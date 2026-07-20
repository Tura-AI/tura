use super::connection::{init_workspace_db, with_connection};
use super::helpers::{
    apply_lifecycle_projection, i64_at, legacy_session_aggregate, management_task_management,
    millis_at, path_text, remove_sqlite_files, session_state_from_management, session_state_text,
    set_object_i64, set_object_string, set_object_value, string_at,
};
use super::SessionLogStore;
use crate::path::{normalize_workspace, workspace_session_log_db};
use anyhow::{Context, Result};
use lifecycle::{SessionAggregate, SessionCommand, SessionQuery, SessionState};
use rusqlite::{params, params_from_iter, OptionalExtension};
use session_log_contract::{
    CommandCheckpoint, DeleteSessionRequest, DeleteWorkspaceRequest, ExecuteSessionCommandRequest,
    MarkSessionInterruptedRequest, UpsertSessionRequest,
};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

fn profile_timings_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("TURA_PROFILE_TURN_TIMINGS")
            .or_else(|_| std::env::var("TURA_PROFILE_TIMINGS"))
            .ok()
            .map(|value| {
                let value = value.trim().to_ascii_lowercase();
                !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
            })
            .unwrap_or(false)
    })
}

fn profile_log(label: &str, elapsed: Option<Duration>, fields: serde_json::Value) {
    if !profile_timings_enabled() {
        return;
    }
    let mut payload = fields.as_object().cloned().unwrap_or_default();
    payload.insert(
        "label".to_string(),
        serde_json::Value::String(label.to_string()),
    );
    if let Some(elapsed) = elapsed {
        payload.insert(
            "elapsed_us".to_string(),
            serde_json::Value::Number((elapsed.as_micros() as u64).into()),
        );
        payload.insert(
            "elapsed_ms".to_string(),
            serde_json::Value::Number((elapsed.as_millis() as u64).into()),
        );
    }
    eprintln!("TURA_PROFILE_TIMING {}", serde_json::Value::Object(payload));
}

fn session_log_omitted_entries(management: &serde_json::Value) -> u64 {
    management
        .get("session_log_retention")
        .and_then(|value| value.get("omitted_entries"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default()
}

impl SessionLogStore {
    pub fn mark_session_interrupted(&self, request: MarkSessionInterruptedRequest) -> Result<bool> {
        self.mark_session_interrupted_by_id(&request.session_id)
    }

    pub fn mark_session_interrupted_by_id(&self, session_id: &str) -> Result<bool> {
        self.interrupt_session_if_recoverable(session_id)
    }

    pub fn mark_stale_running_sessions_interrupted(&self, max_idle: Duration) -> Result<u64> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let cutoff_ms = now_ms.saturating_sub(max_idle.as_millis() as i64);
        let candidates = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT session_id
                 FROM sessions
                 WHERE state IN ('running', 'paused')
                   AND updated_at <= ?1",
            )?;
            let rows = stmt
                .query_map(params![cutoff_ms], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;

        let mut affected = 0;
        for session_id in candidates {
            affected += u64::from(self.interrupt_session_if_recoverable(&session_id)?);
        }
        Ok(affected)
    }

    pub fn upsert_session(&self, request: UpsertSessionRequest) -> Result<()> {
        let total_start = Instant::now();
        let UpsertSessionRequest {
            mut session,
            parent_id,
            messages,
            todos,
        } = request;
        let session_id = string_at(&session, &["id"])
            .or_else(|| string_at(&session, &["management", "session_id"]))
            .context("session id missing")?;
        let workspace = string_at(&session, &["directory"])
            .or_else(|| string_at(&session, &["management", "session_directory"]))
            .unwrap_or_default();
        let workspace = normalize_workspace(&workspace);
        let workspace_db = workspace_session_log_db(&workspace);
        let workspace_db_text = path_text(&workspace_db);

        let mut management = session
            .get("management")
            .cloned()
            .context("session management missing")?;
        let state = session_state_from_management(&management, &session_id)?;
        let state_text = session_state_text(state)?;
        let status = state.ui_status();
        set_object_string(&mut session, "status", status);
        let task_management = session
            .get("task_management")
            .cloned()
            .or_else(|| management_task_management(&management))
            .unwrap_or_else(|| serde_json::json!({}));
        let created_at = i64_at(&session, &["created_at"])
            .or_else(|| millis_at(&management, &["session_created_at"]))
            .unwrap_or_default();
        let updated_at = i64_at(&session, &["updated_at"])
            .or_else(|| millis_at(&management, &["session_last_update_at"]))
            .unwrap_or(created_at);
        let last_user_message_at = i64_at(&session, &["last_user_message_at"])
            .or_else(|| millis_at(&management, &["session_last_user_message_at"]));
        if let Some(last_user_message_at) = last_user_message_at {
            set_object_i64(&mut session, "last_user_message_at", last_user_message_at);
        }
        let requested_message_count = messages.len() as i64;
        let name =
            string_at(&session, &["name"]).or_else(|| string_at(&management, &["session_name"]));
        let parent_id = parent_id.or_else(|| string_at(&session, &["parent_id"]));
        let initial_aggregate = legacy_session_aggregate(
            &session_id,
            state,
            parent_id.clone(),
            &management,
            Some(&task_management),
        )?;
        let serialize_start = Instant::now();
        let task_management_json = serde_json::to_string(&task_management)?;
        let management_json = serde_json::to_string(&management)?;
        let session_json = serde_json::to_string(&session)?;
        let todos_json = serde_json::to_string(&todos)?;
        let lifecycle_json = serde_json::to_string(&initial_aggregate)?;
        profile_log(
            "session_log_store.upsert_session.serialize_session_fields",
            Some(serialize_start.elapsed()),
            serde_json::json!({
                "session_id": session_id,
                "requested_message_count": requested_message_count,
                "task_management_bytes": task_management_json.len(),
                "management_bytes": management_json.len(),
                "session_bytes": session_json.len(),
                "todos_bytes": todos_json.len(),
            }),
        );

        let workspace_write_start = Instant::now();
        let (message_count, parent_id, state_text, status, task_management_json, management_json) =
            self.with_workspace_connection(&workspace_db, |conn| {
                let transaction_start = Instant::now();
                let tx = conn.transaction()?;
                let session_row_start = Instant::now();
                let session_exists = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id = ?1)",
                    params![session_id],
                    |row| row.get::<_, bool>(0),
                )?;
                tx.execute(
                    "INSERT INTO sessions(
                    session_id, workspace, name, parent_id, created_at, updated_at,
                    last_user_message_at, state, status, message_count, task_management_json,
                    management_json, session_json, todos_json, lifecycle_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                ON CONFLICT(session_id) DO UPDATE SET
                    workspace=excluded.workspace,
                    name=excluded.name,
                    created_at=excluded.created_at,
                    updated_at=excluded.updated_at,
                    last_user_message_at=excluded.last_user_message_at,
                    message_count=excluded.message_count,
                    management_json=excluded.management_json,
                    session_json=excluded.session_json,
                    todos_json=excluded.todos_json",
                    params![
                        session_id,
                        workspace,
                        name,
                        parent_id,
                        created_at,
                        updated_at,
                        last_user_message_at,
                        state_text,
                        status,
                        requested_message_count,
                        task_management_json,
                        management_json,
                        session_json,
                        todos_json,
                        lifecycle_json,
                    ],
                )?;
                let lifecycle_json: String = tx.query_row(
                    "SELECT lifecycle_json FROM sessions WHERE session_id = ?1",
                    params![session_id],
                    |row| row.get(0),
                )?;
                let aggregate: SessionAggregate = serde_json::from_str(&lifecycle_json)
                    .with_context(|| format!("invalid lifecycle_json for session {session_id}"))?;
                let projection = aggregate.query(SessionQuery::Lifecycle);
                let state_text = session_state_text(projection.state)?;
                let status = projection.state.ui_status().to_string();
                let projected_task_management =
                    apply_lifecycle_projection(&mut management, &mut session, &projection)?;
                let persisted_task_management = if session_exists {
                    projected_task_management
                } else {
                    set_object_value(&mut session, "task_management", task_management.clone());
                    task_management.clone()
                };
                let task_management_json = serde_json::to_string(&persisted_task_management)?;
                let management_json = serde_json::to_string(&management)?;
                let session_json = serde_json::to_string(&session)?;
                tx.execute(
                    "UPDATE sessions
                 SET parent_id = ?2, state = ?3, status = ?4,
                     task_management_json = ?5, management_json = ?6, session_json = ?7
                 WHERE session_id = ?1",
                    params![
                        session_id,
                        projection.parent_id,
                        state_text,
                        status,
                        task_management_json,
                        management_json,
                        session_json,
                    ],
                )?;
                profile_log(
                    "session_log_store.upsert_session.workspace_session_row",
                    Some(session_row_start.elapsed()),
                    serde_json::json!({
                        "session_id": session_id,
                        "workspace_db": workspace_db_text,
                    }),
                );

                {
                    let records_start = Instant::now();
                    let mut stmt = tx.prepare(
                        "INSERT INTO session_records(
                        session_id, message_id, role, created_at, updated_at, record_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ON CONFLICT(session_id, message_id) DO UPDATE SET
                        role=excluded.role,
                        created_at=excluded.created_at,
                        updated_at=excluded.updated_at,
                        record_json=excluded.record_json",
                    )?;
                    let mut message_ids = Vec::new();
                    let mut record_json_bytes = 0usize;
                    let mut record_serialize_us = 0u128;
                    let mut record_execute_us = 0u128;
                    for message in messages {
                        let created = i64_at(&message, &["created_at"]).unwrap_or_default();
                        let message_id = string_at(&message, &["id"])
                            .unwrap_or_else(|| format!("{session_id}:{created}"));
                        message_ids.push(message_id.clone());
                        let role = string_at(&message, &["role"]).unwrap_or_default();
                        let updated = i64_at(&message, &["updated_at"]).unwrap_or(created);
                        let record_serialize_start = Instant::now();
                        let record_json = serde_json::to_string(&message)?;
                        record_serialize_us += record_serialize_start.elapsed().as_micros();
                        record_json_bytes += record_json.len();
                        let record_execute_start = Instant::now();
                        stmt.execute(params![
                            session_id,
                            message_id,
                            role,
                            created,
                            updated,
                            record_json,
                        ])?;
                        record_execute_us += record_execute_start.elapsed().as_micros();
                    }
                    drop(stmt);
                    let cleanup_start = Instant::now();
                    let preserve_unlisted_records = session_log_omitted_entries(&management) > 0;
                    if preserve_unlisted_records {
                        // Compacted runtime snapshots only send the retained session_log tail.
                        // Keep older session_records so UI/history reads do not collapse to the tail.
                    } else if message_ids.is_empty() {
                        tx.execute(
                            "DELETE FROM session_records WHERE session_id = ?1",
                            params![session_id],
                        )?;
                    } else {
                        let placeholders = std::iter::repeat_n("?", message_ids.len())
                            .collect::<Vec<_>>()
                            .join(",");
                        let sql = format!(
                            "DELETE FROM session_records
                         WHERE session_id = ? AND message_id NOT IN ({placeholders})"
                        );
                        let params = std::iter::once(session_id.clone()).chain(message_ids);
                        tx.execute(&sql, params_from_iter(params))?;
                    }
                    let cleanup_elapsed = cleanup_start.elapsed();
                    profile_log(
                        "session_log_store.upsert_session.workspace_records",
                        Some(records_start.elapsed()),
                        serde_json::json!({
                            "session_id": session_id,
                            "record_json_bytes": record_json_bytes,
                            "record_serialize_us": record_serialize_us,
                            "record_execute_us": record_execute_us,
                            "cleanup_us": cleanup_elapsed.as_micros(),
                        }),
                    );
                }
                let count_start = Instant::now();
                let message_count = tx.query_row(
                    "SELECT COUNT(*) FROM session_records WHERE session_id = ?1",
                    params![session_id],
                    |row| row.get::<_, i64>(0),
                )?;
                tx.execute(
                    "UPDATE sessions SET message_count = ?2 WHERE session_id = ?1",
                    params![session_id, message_count],
                )?;
                profile_log(
                    "session_log_store.upsert_session.workspace_count_update",
                    Some(count_start.elapsed()),
                    serde_json::json!({
                        "session_id": session_id,
                        "message_count": message_count,
                    }),
                );
                let commit_start = Instant::now();
                tx.commit()?;
                profile_log(
                    "session_log_store.upsert_session.workspace_commit",
                    Some(commit_start.elapsed()),
                    serde_json::json!({
                        "session_id": session_id,
                        "message_count": message_count,
                    }),
                );
                profile_log(
                    "session_log_store.upsert_session.workspace_transaction",
                    Some(transaction_start.elapsed()),
                    serde_json::json!({
                        "session_id": session_id,
                        "message_count": message_count,
                    }),
                );
                Ok((
                    message_count,
                    projection.parent_id,
                    state_text,
                    status,
                    task_management_json,
                    management_json,
                ))
            })?;
        profile_log(
            "session_log_store.upsert_session.workspace_total",
            Some(workspace_write_start.elapsed()),
            serde_json::json!({
                "session_id": session_id,
                "message_count": message_count,
            }),
        );

        let index_write_start = Instant::now();
        self.with_index_connection(|conn| {
            conn.execute(
                "INSERT INTO sessions(
                    session_id, workspace, workspace_db_path, name, parent_id, created_at,
                    updated_at, last_user_message_at, state, status, message_count, task_management_json, management_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(session_id) DO UPDATE SET
                    workspace=excluded.workspace,
                    workspace_db_path=excluded.workspace_db_path,
                    name=excluded.name,
                    parent_id=excluded.parent_id,
                    created_at=excluded.created_at,
                    updated_at=excluded.updated_at,
                    last_user_message_at=excluded.last_user_message_at,
                    state=excluded.state,
                    status=excluded.status,
                    message_count=excluded.message_count,
                    task_management_json=excluded.task_management_json,
                    management_json=excluded.management_json",
                params![
                    session_id,
                    workspace,
                    workspace_db_text,
                    name,
                    parent_id,
                    created_at,
                    updated_at,
                    last_user_message_at,
                    state_text,
                    status,
                    message_count,
                    task_management_json,
                    management_json,
                ],
            )?;
            Ok(())
        })?;
        profile_log(
            "session_log_store.upsert_session.index_total",
            Some(index_write_start.elapsed()),
            serde_json::json!({
                "session_id": session_id,
                "message_count": message_count,
            }),
        );

        profile_log(
            "session_log_store.upsert_session.total",
            Some(total_start.elapsed()),
            serde_json::json!({
                "session_id": session_id,
                "requested_message_count": requested_message_count,
                "message_count": message_count,
            }),
        );
        Ok(())
    }

    pub fn apply_command_checkpoint(&self, checkpoint: CommandCheckpoint) -> Result<()> {
        let idempotency_key = checkpoint.idempotency_key();
        let payload_json = serde_json::to_string(&checkpoint)?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        self.with_index_connection(|conn| {
            conn.execute(
                "INSERT INTO session_write_queue(
                    idempotency_key, session_id, turn_id, runtime_worker_id,
                    command_run_id, command_id, event_seq, event_type, payload_json,
                    status, retry_count, created_at, applied_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'applied', 0, ?10, ?10)
                ON CONFLICT(idempotency_key) DO UPDATE SET
                    payload_json=excluded.payload_json,
                    status='applied',
                    applied_at=excluded.applied_at,
                    last_error=NULL",
                params![
                    idempotency_key,
                    checkpoint.session_id,
                    checkpoint.turn_id,
                    checkpoint.runtime_worker_id,
                    checkpoint.command_run_id,
                    checkpoint.command_id,
                    checkpoint.event_seq,
                    checkpoint.status,
                    payload_json,
                    now_ms,
                ],
            )?;
            Ok(())
        })
    }

    pub fn mark_running_sessions_interrupted(&self) -> Result<u64> {
        let candidates = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare("SELECT session_id FROM sessions")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;

        let mut affected: u64 = 0;
        for session_id in candidates {
            affected += u64::from(self.interrupt_session_if_recoverable(&session_id)?);
        }
        Ok(affected)
    }

    fn interrupt_session_if_recoverable(&self, session_id: &str) -> Result<bool> {
        let Some(snapshot) = self.get_session_without_stale_sweep(session_id)? else {
            return Ok(false);
        };
        let projection = snapshot.lifecycle_projection.with_context(|| {
            format!("canonical lifecycle projection missing for session {session_id}")
        })?;
        if !projection.state.is_recoverable_running() {
            return Ok(false);
        }
        let result = self.execute_session_command(ExecuteSessionCommandRequest {
            session_id: session_id.to_string(),
            session_command: SessionCommand::InterruptSession,
        })?;
        Ok(result.projection.state == SessionState::Interrupted)
    }

    pub fn delete_session(&self, request: DeleteSessionRequest) -> Result<()> {
        let workspace_db_path = self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT workspace_db_path FROM sessions WHERE session_id = ?1",
                params![request.session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        if let Some(path) = workspace_db_path.as_deref().map(Path::new) {
            if path.exists() {
                with_connection(path, init_workspace_db, |conn| {
                    conn.execute(
                        "DELETE FROM sessions WHERE session_id = ?1",
                        params![request.session_id],
                    )?;
                    Ok(())
                })?;
            }
        }
        self.delete_index_session(&request.session_id)
    }

    pub fn delete_workspace(&self, request: DeleteWorkspaceRequest) -> Result<()> {
        let workspace = normalize_workspace(&request.workspace);
        let db_paths = self.with_index_connection(|conn| {
            let mut stmt = conn
                .prepare("SELECT DISTINCT workspace_db_path FROM sessions WHERE workspace = ?1")?;
            let paths = stmt
                .query_map(params![workspace], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            conn.execute(
                "DELETE FROM sessions WHERE workspace = ?1",
                params![workspace],
            )?;
            Ok(paths)
        })?;
        for path in db_paths {
            remove_sqlite_files(Path::new(&path))?;
        }
        Ok(())
    }
}
