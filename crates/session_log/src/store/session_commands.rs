use super::helpers::{
    apply_lifecycle_projection, i64_at, millis_to_rfc3339, path_text, session_state_text,
    string_at, task_management_value,
};
use super::SessionLogStore;
use crate::path::{normalize_workspace, workspace_session_log_db};
use anyhow::{Context, Result};
use lifecycle::{SessionAggregate, SessionCommand, SessionEvent, SessionQuery};
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::{json, Value};
use session_log_contract::{
    CreateSessionRequest, ExecuteSessionCommandRequest, PersistSessionPayloadRequest,
    SessionCommandResult,
};

impl SessionLogStore {
    pub fn create_session(&self, request: CreateSessionRequest) -> Result<SessionCommandResult> {
        let workspace = normalize_workspace(&request.workspace);
        let workspace_db = workspace_session_log_db(&workspace);
        let workspace_db_text = path_text(&workspace_db);
        if !matches!(
            &request.creation_command,
            SessionCommand::CreateSession { .. }
                | SessionCommand::ForkSession { .. }
                | SessionCommand::RegisterChildSession { .. }
        ) {
            anyhow::bail!("create_session requires a creation command");
        }
        let mut aggregate = SessionAggregate::new(request.session_id.clone());
        let event = aggregate.execute(request.creation_command.clone())?;
        let projection = aggregate.query(SessionQuery::Lifecycle);
        let state_text = session_state_text(projection.state)?;
        let status = projection.state.ui_status();
        let timestamp = millis_to_rfc3339(request.created_at)?;
        let task_management = task_management_value(&projection.task_plan);
        let management = json!({
            "session_id": &request.session_id,
            "session_name": &request.name,
            "auto_session_name": request.auto_session_name,
            "session_directory": &request.session_directory,
            "session_uses_docker": false,
            "task_type": [],
            "session_capabilities": [],
            "session_current_turn": 0,
            "session_log": [],
            "session_log_retention": { "omitted_entries": 0 },
            "session_created_at": timestamp,
            "session_last_update_at": timestamp,
            "session_last_user_message_at": timestamp,
            "session_started_at": timestamp,
            "input": {
                "user_input": "",
                "file_input": [],
                "agent": &request.agent,
                "runtime_context": null,
                "planning_mode_override": null
            },
            "user_goal": "",
            "current_objective": "",
            "task_plan": &projection.task_plan,
            "state": projection.state,
            "use_last_tool_call_response": request.use_last_tool_call_response,
            "is_child_session": projection.parent_id.is_some(),
            "disable_permission_restrictions": request.disable_permission_restrictions,
            "planning_enabled": false,
            "reflection_enabled": false,
            "op_manual_enabled": true,
            "no_op_manual": false,
            "goal_mode": false,
            "last_goal_user_input": "",
            "context_tokens": { "input": 0, "limit": 260000 },
            "runtime_usage": null
        });
        let directory = if workspace.is_empty() {
            Value::Null
        } else {
            Value::String(workspace.clone())
        };
        let session = json!({
            "id": &request.session_id,
            "created_at": request.created_at,
            "updated_at": request.created_at,
            "last_user_message_at": request.created_at,
            "directory": directory,
            "model": &request.model,
            "agent": &request.agent,
            "session_type": &request.session_type,
            "kill_processes_on_start": request.kill_processes_on_start,
            "validator_enabled": request.validator_enabled,
            "force_planning": request.force_planning,
            "model_variant": &request.model_variant,
            "model_acceleration_enabled": request.model_acceleration_enabled,
            "disable_permission_restrictions": request.disable_permission_restrictions,
            "use_last_tool_call_response": request.use_last_tool_call_response,
            "status": status,
            "message_count": 0,
            "management": management,
            "task_management": task_management
        });
        let lifecycle_json = serde_json::to_string(&aggregate)?;
        let management_json = serde_json::to_string(&management)?;
        let session_json = serde_json::to_string(&session)?;
        let task_management_json = serde_json::to_string(&task_management)?;

        self.with_workspace_connection(&workspace_db, |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO sessions(
                    session_id, workspace, name, parent_id, created_at, updated_at,
                    last_user_message_at, state, status, message_count, task_management_json,
                    management_json, session_json, todos_json, lifecycle_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5, ?6, ?7, 0, ?8, ?9, ?10, '[]', ?11)",
                params![
                    request.session_id,
                    workspace,
                    request.name,
                    projection.parent_id,
                    request.created_at,
                    state_text,
                    status,
                    task_management_json,
                    management_json,
                    session_json,
                    lifecycle_json,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })?;
        self.upsert_session_index(
            &request.session_id,
            &workspace,
            &workspace_db_text,
            Some(&request.name),
            projection.parent_id.as_deref(),
            request.created_at,
            request.created_at,
            Some(request.created_at),
            &state_text,
            status,
            0,
            &task_management_json,
            &management_json,
        )?;
        Ok(SessionCommandResult {
            event,
            projection,
            session_name: Some(request.name),
        })
    }

    pub fn execute_session_command(
        &self,
        request: ExecuteSessionCommandRequest,
    ) -> Result<SessionCommandResult> {
        let workspace_db_path = self
            .workspace_db_path_for_session(&request.session_id)?
            .with_context(|| format!("session {} not found", request.session_id))?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let command = request.session_command;
        let auto_name = task_summary_for_auto_name(&command);
        let result = self.with_workspace_connection(&workspace_db_path, |conn| {
            let tx = conn.transaction()?;
            let row = load_command_row(&tx, &request.session_id)?
                .with_context(|| format!("session {} not found", request.session_id))?;
            let mut aggregate: SessionAggregate = serde_json::from_str(&row.lifecycle_json)
                .with_context(|| {
                    format!("invalid lifecycle_json for session {}", request.session_id)
                })?;
            let previous_task_plan = aggregate.task_plan.clone();
            let event = aggregate.execute(command)?;
            let projection = aggregate.query(SessionQuery::Lifecycle);
            if matches!(event, SessionEvent::SessionDeleted) {
                tx.execute(
                    "DELETE FROM sessions WHERE session_id = ?1",
                    params![request.session_id],
                )?;
                tx.commit()?;
                return Ok(CommandTransactionResult::Deleted(SessionCommandResult {
                    event,
                    projection,
                    session_name: None,
                }));
            }

            let mut management: Value =
                serde_json::from_str(&row.management_json).with_context(|| {
                    format!("invalid management_json for session {}", request.session_id)
                })?;
            let mut session: Value =
                serde_json::from_str(&row.session_json).with_context(|| {
                    format!("invalid session_json for session {}", request.session_id)
                })?;
            let timestamp = millis_to_rfc3339(now_ms)?;
            let state_text = session_state_text(projection.state)?;
            let status = projection.state.ui_status();
            set_json_field(
                &mut management,
                "session_last_update_at",
                Value::String(timestamp),
            );
            if let Some(name) = auto_name
                .filter(|_| projection.task_plan != previous_task_plan)
                .filter(|_| management["auto_session_name"].as_bool().unwrap_or(true))
            {
                set_json_field(&mut management, "session_name", Value::String(name.clone()));
                set_json_field(&mut session, "name", Value::String(name));
            }
            set_json_field(&mut session, "updated_at", Value::Number(now_ms.into()));
            let task_management =
                apply_lifecycle_projection(&mut management, &mut session, &projection)?;
            let lifecycle_json = serde_json::to_string(&aggregate)?;
            let management_json = serde_json::to_string(&management)?;
            let session_json = serde_json::to_string(&session)?;
            let task_management_json = serde_json::to_string(&task_management)?;
            let name = management["session_name"].as_str().map(ToString::to_string);
            tx.execute(
                "UPDATE sessions
                 SET name = ?2, parent_id = ?3, updated_at = MAX(updated_at, ?4),
                     state = ?5, status = ?6, task_management_json = ?7,
                     management_json = ?8, session_json = ?9, lifecycle_json = ?10
                 WHERE session_id = ?1",
                params![
                    request.session_id,
                    name,
                    projection.parent_id,
                    now_ms,
                    state_text,
                    status,
                    task_management_json,
                    management_json,
                    session_json,
                    lifecycle_json,
                ],
            )?;
            tx.commit()?;
            Ok(CommandTransactionResult::Updated {
                result: SessionCommandResult {
                    event,
                    projection,
                    session_name: name.clone(),
                },
                name,
                state_text,
                status: status.to_string(),
                task_management_json,
                management_json,
                workspace: row.workspace,
                created_at: row.created_at,
                last_user_message_at: row.last_user_message_at,
                message_count: row.message_count,
            })
        })?;

        match result {
            CommandTransactionResult::Deleted(result) => {
                self.delete_index_session(&request.session_id)?;
                Ok(result)
            }
            CommandTransactionResult::Updated {
                result,
                name,
                state_text,
                status,
                task_management_json,
                management_json,
                workspace,
                created_at,
                last_user_message_at,
                message_count,
            } => {
                self.upsert_session_index(
                    &request.session_id,
                    &workspace,
                    &path_text(&workspace_db_path),
                    name.as_deref(),
                    result.projection.parent_id.as_deref(),
                    created_at,
                    now_ms,
                    last_user_message_at,
                    &state_text,
                    &status,
                    message_count,
                    &task_management_json,
                    &management_json,
                )?;
                Ok(result)
            }
        }
    }

    pub fn persist_session_payload(&self, request: PersistSessionPayloadRequest) -> Result<()> {
        let workspace_db_path = self
            .workspace_db_path_for_session(&request.session_id)?
            .with_context(|| format!("session {} not found", request.session_id))?;
        let session_id = request.session_id;
        let todos_json = serde_json::to_string(&request.todos)?;
        let message_count = self.with_workspace_connection(&workspace_db_path, |conn| {
            let tx = conn.transaction()?;
            let session_exists = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id = ?1)",
                params![session_id],
                |row| row.get::<_, bool>(0),
            )?;
            if !session_exists {
                anyhow::bail!("session {session_id} not found");
            }

            tx.execute(
                "DELETE FROM session_records WHERE session_id = ?1",
                params![session_id],
            )?;
            {
                let mut statement = tx.prepare(
                    "INSERT INTO session_records(
                        session_id, message_id, role, created_at, updated_at, record_json
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?;
                for record in request.records {
                    let message_id = string_at(&record, &["id"])
                        .filter(|value| !value.trim().is_empty())
                        .with_context(|| format!("record id missing for session {session_id}"))?;
                    let role = string_at(&record, &["role"])
                        .filter(|value| !value.trim().is_empty())
                        .with_context(|| format!("record role missing for session {session_id}"))?;
                    let created_at = i64_at(&record, &["created_at"]).unwrap_or_default();
                    let updated_at = i64_at(&record, &["updated_at"]).unwrap_or(created_at);
                    let record_json = serde_json::to_string(&record)?;
                    statement.execute(params![
                        session_id,
                        message_id,
                        role,
                        created_at,
                        updated_at,
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
                "UPDATE sessions SET message_count = ?2, todos_json = ?3 WHERE session_id = ?1",
                params![session_id, message_count, todos_json],
            )?;
            tx.commit()?;
            Ok(message_count)
        })?;
        self.with_index_connection(|conn| {
            let changed = conn.execute(
                "UPDATE sessions SET message_count = ?2 WHERE session_id = ?1",
                params![session_id, message_count],
            )?;
            if changed == 0 {
                anyhow::bail!("session {session_id} not found in index");
            }
            Ok(())
        })
    }

    fn workspace_db_path_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<std::path::PathBuf>> {
        self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT workspace_db_path FROM sessions WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.map(Into::into))
            .map_err(Into::into)
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "index projection mirrors its SQL row"
    )]
    fn upsert_session_index(
        &self,
        session_id: &str,
        workspace: &str,
        workspace_db_path: &str,
        name: Option<&str>,
        parent_id: Option<&str>,
        created_at: i64,
        updated_at: i64,
        last_user_message_at: Option<i64>,
        state: &str,
        status: &str,
        message_count: i64,
        task_management_json: &str,
        management_json: &str,
    ) -> Result<()> {
        self.with_index_connection(|conn| {
            conn.execute(
                "INSERT INTO sessions(
                    session_id, workspace, workspace_db_path, name, parent_id, created_at,
                    updated_at, last_user_message_at, state, status, message_count,
                    task_management_json, management_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(session_id) DO UPDATE SET
                    workspace=excluded.workspace, workspace_db_path=excluded.workspace_db_path,
                    name=excluded.name, parent_id=excluded.parent_id,
                    created_at=excluded.created_at, updated_at=excluded.updated_at,
                    last_user_message_at=excluded.last_user_message_at,
                    state=excluded.state, status=excluded.status,
                    message_count=excluded.message_count,
                    task_management_json=excluded.task_management_json,
                    management_json=excluded.management_json",
                params![
                    session_id,
                    workspace,
                    workspace_db_path,
                    name,
                    parent_id,
                    created_at,
                    updated_at,
                    last_user_message_at,
                    state,
                    status,
                    message_count,
                    task_management_json,
                    management_json,
                ],
            )?;
            Ok(())
        })
    }
}

struct CommandRow {
    workspace: String,
    created_at: i64,
    last_user_message_at: Option<i64>,
    message_count: i64,
    management_json: String,
    session_json: String,
    lifecycle_json: String,
}

fn load_command_row(tx: &Transaction<'_>, session_id: &str) -> Result<Option<CommandRow>> {
    tx.query_row(
        "SELECT workspace, created_at, last_user_message_at, message_count,
                management_json, session_json, lifecycle_json
         FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok(CommandRow {
                workspace: row.get(0)?,
                created_at: row.get(1)?,
                last_user_message_at: row.get(2)?,
                message_count: row.get(3)?,
                management_json: row.get(4)?,
                session_json: row.get(5)?,
                lifecycle_json: row.get(6)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

enum CommandTransactionResult {
    Deleted(SessionCommandResult),
    Updated {
        result: SessionCommandResult,
        name: Option<String>,
        state_text: String,
        status: String,
        task_management_json: String,
        management_json: String,
        workspace: String,
        created_at: i64,
        last_user_message_at: Option<i64>,
        message_count: i64,
    },
}

fn task_summary_for_auto_name(command: &SessionCommand) -> Option<String> {
    match command {
        SessionCommand::ApplyTaskPatch { patch, .. } => patch.task_summary.clone(),
        SessionCommand::ApplyTaskPatches { tasks, .. } => tasks
            .iter()
            .rev()
            .find_map(|patch| patch.task_summary.clone()),
        SessionCommand::ApplyTaskPlanPatch { patch } => patch
            .task
            .as_ref()
            .and_then(|patch| patch.task_summary.clone())
            .or_else(|| {
                patch.tasks.as_ref().and_then(|tasks| {
                    tasks
                        .iter()
                        .rev()
                        .find_map(|patch| patch.task_summary.clone())
                })
            }),
        _ => None,
    }
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
}

fn set_json_field(value: &mut Value, key: &str, field: Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), field);
    }
}
