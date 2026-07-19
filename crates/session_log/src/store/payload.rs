use super::connection::{init_workspace_db, with_connection};
use super::helpers::{
    parse_json_field, session_state_text, set_object_i64, set_object_string,
    transition_management_to_interrupted,
};
use anyhow::Result;
use lifecycle::SessionState;
use rusqlite::{params, OptionalExtension, Row};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct IndexSessionRow {
    pub(super) session_id: String,
    pub(super) workspace_db_path: PathBuf,
}

pub(super) struct WorkspacePayload {
    pub(super) workspace: String,
    pub(super) name: Option<String>,
    pub(super) parent_id: Option<String>,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
    pub(super) last_user_message_at: Option<i64>,
    pub(super) state: Option<String>,
    pub(super) status: Option<String>,
    pub(super) message_count: i64,
    pub(super) task_management: Value,
    pub(super) management: Value,
    pub(super) session: Value,
    pub(super) todos: Vec<Value>,
}

pub(super) struct WorkspaceSessionSummaryPayload {
    pub(super) workspace: String,
    pub(super) name: Option<String>,
    pub(super) parent_id: Option<String>,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
    pub(super) last_user_message_at: Option<i64>,
    pub(super) state: Option<String>,
    pub(super) status: Option<String>,
    pub(super) message_count: i64,
    pub(super) task_management: Value,
}

pub(super) fn index_session_from_row(row: &Row<'_>) -> rusqlite::Result<IndexSessionRow> {
    Ok(IndexSessionRow {
        session_id: row.get(0)?,
        workspace_db_path: PathBuf::from(row.get::<_, String>(1)?),
    })
}

pub(super) fn load_workspace_session_payload(
    workspace_db_path: &Path,
    session_id: &str,
) -> Result<Option<WorkspacePayload>> {
    if !workspace_db_path.exists() {
        return Ok(None);
    }
    let payload = with_connection(workspace_db_path, init_workspace_db, |conn| {
        conn.query_row(
            "SELECT workspace, name, parent_id, created_at, updated_at, last_user_message_at, state, status,
                    message_count, task_management_json, management_json, session_json, todos_json
             FROM sessions
             WHERE session_id = ?1",
            params![session_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                ))
            },
        )
        .optional()
        .map_err(Into::into)
    })?;
    payload
        .map(
            |(
                workspace,
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
                session_json,
                todos_json,
            )| {
                Ok(WorkspacePayload {
                    workspace,
                    name,
                    parent_id,
                    created_at,
                    updated_at,
                    last_user_message_at,
                    state,
                    status,
                    message_count,
                    task_management: parse_json_field(
                        &task_management_json,
                        "task_management_json",
                        Some(session_id),
                    )?,
                    management: parse_json_field(
                        &management_json,
                        "management_json",
                        Some(session_id),
                    )?,
                    session: parse_json_field(&session_json, "session_json", Some(session_id))?,
                    todos: parse_json_field(&todos_json, "todos_json", Some(session_id))?,
                })
            },
        )
        .transpose()
}

pub(super) fn load_workspace_session_summary_payload(
    workspace_db_path: &Path,
    session_id: &str,
) -> Result<Option<WorkspaceSessionSummaryPayload>> {
    if !workspace_db_path.exists() {
        return Ok(None);
    }
    let payload = with_connection(workspace_db_path, init_workspace_db, |conn| {
        conn.query_row(
            "SELECT workspace, name, parent_id, created_at, updated_at, last_user_message_at, state, status,
                    message_count, task_management_json
             FROM sessions
             WHERE session_id = ?1",
            params![session_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        )
        .optional()
        .map_err(Into::into)
    })?;
    payload
        .map(
            |(
                workspace,
                name,
                parent_id,
                created_at,
                updated_at,
                last_user_message_at,
                state,
                status,
                message_count,
                task_management_json,
            )| {
                Ok(WorkspaceSessionSummaryPayload {
                    workspace,
                    name,
                    parent_id,
                    created_at,
                    updated_at,
                    last_user_message_at,
                    state,
                    status,
                    message_count,
                    task_management: parse_json_field(
                        &task_management_json,
                        "task_management_json",
                        Some(session_id),
                    )?,
                })
            },
        )
        .transpose()
}

pub(super) fn mark_workspace_session_interrupted(
    workspace_db_path: &Path,
    session_id: &str,
    now_ms: i64,
) -> Result<Option<Value>> {
    if !workspace_db_path.exists() {
        return Ok(None);
    }
    with_connection(workspace_db_path, init_workspace_db, |conn| {
        let payload = conn
            .query_row(
                "SELECT session_json, management_json FROM sessions WHERE session_id = ?1",
                params![session_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((session_json, management_json)) = payload else {
            return Ok(None);
        };
        let mut session: Value = parse_json_field(&session_json, "session_json", Some(session_id))?;
        let mut management: Value =
            parse_json_field(&management_json, "management_json", Some(session_id))?;
        if !transition_management_to_interrupted(&mut management, session_id, now_ms)? {
            return Ok(None);
        }
        let state_text = session_state_text(SessionState::Interrupted)?;
        let status = SessionState::Interrupted.ui_status();
        set_object_string(&mut session, "status", status);
        set_object_i64(&mut session, "updated_at", now_ms);
        conn.execute(
            "UPDATE sessions
             SET state = ?2,
                 status = ?3,
                 updated_at = MAX(updated_at, ?4),
                 session_json = ?5,
                 management_json = ?6
             WHERE session_id = ?1",
            params![
                session_id,
                state_text,
                status,
                now_ms,
                serde_json::to_string(&session)?,
                serde_json::to_string(&management)?,
            ],
        )?;
        Ok(Some(management))
    })
}
