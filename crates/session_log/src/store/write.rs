use super::connection::{init_workspace_db, with_connection};
use super::helpers::{
    i64_at, management_task_management, millis_at, path_text, remove_sqlite_files,
    session_state_from_management, session_state_text, set_object_string, string_at,
};
use super::payload::mark_workspace_session_interrupted;
use super::SessionLogStore;
use crate::checkpoint::CommandCheckpoint;
use crate::path::{normalize_workspace, workspace_session_log_db};
use crate::protocol::{DeleteSessionRequest, DeleteWorkspaceRequest, UpsertSessionRequest};
use crate::SessionState;
use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

impl SessionLogStore {
    pub fn upsert_session(&self, request: UpsertSessionRequest) -> Result<()> {
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

        let management = session
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
        let requested_message_count = messages.len() as i64;
        let name =
            string_at(&session, &["name"]).or_else(|| string_at(&management, &["session_name"]));
        let parent_id = parent_id.or_else(|| string_at(&session, &["parent_id"]));
        let task_management_json = serde_json::to_string(&task_management)?;
        let management_json = serde_json::to_string(&management)?;
        let session_json = serde_json::to_string(&session)?;
        let todos_json = serde_json::to_string(&todos)?;

        let message_count = self.with_workspace_connection(&workspace_db, |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO sessions(
                    session_id, workspace, name, parent_id, created_at, updated_at,
                    state, status, message_count, task_management_json, management_json,
                    session_json, todos_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(session_id) DO UPDATE SET
                    workspace=excluded.workspace,
                    name=excluded.name,
                    parent_id=excluded.parent_id,
                    created_at=excluded.created_at,
                    updated_at=excluded.updated_at,
                    state=excluded.state,
                    status=excluded.status,
                    message_count=excluded.message_count,
                    task_management_json=excluded.task_management_json,
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
                    state_text,
                    status,
                    requested_message_count,
                    task_management_json,
                    management_json,
                    session_json,
                    todos_json,
                ],
            )?;

            {
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
                for message in messages {
                    let created = i64_at(&message, &["created_at"]).unwrap_or_default();
                    let message_id = string_at(&message, &["id"])
                        .unwrap_or_else(|| format!("{session_id}:{created}"));
                    let role = string_at(&message, &["role"]).unwrap_or_default();
                    let updated = i64_at(&message, &["updated_at"]).unwrap_or(created);
                    let record_json = serde_json::to_string(&message)?;
                    stmt.execute(params![
                        session_id,
                        message_id,
                        role,
                        created,
                        updated,
                        record_json,
                    ])?;
                }
            }
            let message_count = tx.query_row(
                "SELECT COUNT(*) FROM session_records WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, i64>(0),
            )?;
            tx.execute(
                "UPDATE sessions SET message_count = ?2 WHERE session_id = ?1",
                params![session_id, message_count],
            )?;
            tx.commit()?;
            Ok(message_count)
        })?;

        self.with_index_connection(|conn| {
            conn.execute(
                "INSERT INTO sessions(
                    session_id, workspace, workspace_db_path, name, parent_id, created_at,
                    updated_at, state, status, message_count, task_management_json, management_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ON CONFLICT(session_id) DO UPDATE SET
                    workspace=excluded.workspace,
                    workspace_db_path=excluded.workspace_db_path,
                    name=excluded.name,
                    parent_id=excluded.parent_id,
                    created_at=excluded.created_at,
                    updated_at=excluded.updated_at,
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
                    state_text,
                    status,
                    message_count,
                    task_management_json,
                    management_json,
                ],
            )?;
            Ok(())
        })?;

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
        let now_ms = chrono::Utc::now().timestamp_millis();
        let candidates = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare("SELECT session_id, workspace_db_path FROM sessions")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;

        let mut affected: u64 = 0;
        for (session_id, workspace_db_path) in candidates {
            let Some(management) = mark_workspace_session_interrupted(
                Path::new(&workspace_db_path),
                &session_id,
                now_ms,
            )?
            else {
                continue;
            };
            let state_text = session_state_text(SessionState::Interrupted)?;
            let status = SessionState::Interrupted.ui_status();
            let management_json = serde_json::to_string(&management)?;
            self.with_index_connection(|conn| {
                conn.execute(
                    "UPDATE sessions
                     SET state = ?2,
                         status = ?3,
                         updated_at = MAX(updated_at, ?4),
                         management_json = ?5
                     WHERE session_id = ?1",
                    params![session_id, state_text, status, now_ms, management_json],
                )?;
                Ok(())
            })?;
            affected += 1;
        }
        Ok(affected)
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
