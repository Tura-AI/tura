//! Session API handlers

use crate::api::product::current_user_snapshot;
use crate::api::types::*;
use crate::mock::global_store;
use crate::session::config::{load_config, merge_config, TuraSessionConfig};
use crate::session::{
    process_cleanup::kill_processes_in_directory, session_store, MessageRole as SessionMessageRole,
    SessionStatus as SessionStatusMano,
};
use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::time::Duration;

use runtime::state_machine::session_management::StartCondition;

// ============================================================================
// Session List & Create
// ============================================================================

pub async fn list_sessions(
    headers: HeaderMap,
    Query(params): Query<SessionListParams>,
) -> Json<Vec<Session>> {
    let directory = params
        .directory
        .clone()
        .or(params.workspace.clone())
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .or_else(|| global_store().get_current_directory());

    session_store().hydrate_directory(directory.clone());

    let mut sessions = filter_list_sessions(
        session_store().list_sessions(),
        &params,
        directory.as_deref(),
    );
    sessions.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    if let Some(limit) = params.limit.filter(|limit| *limit > 0) {
        sessions.truncate(limit);
    }

    Json(sessions)
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct SessionListParams {
    pub directory: Option<String>,
    pub workspace: Option<String>,
    pub roots: Option<bool>,
    #[serde(default, alias = "includeChildren")]
    pub include_children: bool,
    pub start: Option<i64>,
    pub search: Option<String>,
    pub limit: Option<usize>,
}

fn filter_list_sessions(
    sessions: Vec<Session>,
    params: &SessionListParams,
    directory: Option<&str>,
) -> Vec<Session> {
    let directory_key = directory
        .map(workspace_key)
        .filter(|value| !value.is_empty());
    let search = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    sessions
        .into_iter()
        .filter(|session| {
            if let Some(directory_key) = &directory_key {
                if workspace_key(session.directory.as_deref().unwrap_or_default()) != *directory_key
                {
                    return false;
                }
            }
            if (params.roots == Some(true) || !params.include_children)
                && session.parent_id.is_some()
            {
                return false;
            }
            if let Some(start) = params.start {
                if session.updated_at < start {
                    return false;
                }
            }
            if let Some(search) = &search {
                let title = session
                    .name
                    .as_deref()
                    .unwrap_or("New Session")
                    .to_ascii_lowercase();
                if !title.contains(search) && !session.id.to_ascii_lowercase().contains(search) {
                    return false;
                }
            }
            true
        })
        .collect()
}

fn workspace_key(directory: &str) -> String {
    let value = directory.trim().replace('\\', "/");
    if value.is_empty() {
        return String::new();
    }
    if value.len() == 3
        && value.as_bytes()[1] == b':'
        && value.ends_with('/')
        && value.as_bytes()[0].is_ascii_alphabetic()
    {
        return value;
    }
    if value.chars().all(|ch| ch == '/') {
        return "/".to_string();
    }
    value.trim_end_matches('/').to_string()
}

pub async fn get_session(Path(session_id): Path<String>) -> Json<Session> {
    session_store()
        .get_session(&session_id)
        .map(Json)
        .unwrap_or_else(|| {
            Json(Session {
                id: session_id,
                name: None,
                parent_id: None,
                created_at: 0,
                updated_at: 0,
                directory: None,
                model: None,
                agent: None,
                session_type: Some("coding".to_string()),
                auto_session_name: true,
                kill_processes_on_start: false,
                validator_enabled: false,
                force_planning: false,
                disable_permission_restrictions: false,
                model_variant: None,
                model_acceleration_enabled: false,
                status: SessionStatus::Idle,
                message_count: 0,
                task_management: serde_json::json!({}),
                plan_summary: None,
                session_display_name: None,
            })
        })
}

pub async fn create_session(
    headers: HeaderMap,
    Query(params): Query<SessionDirectoryParams>,
    payload: Option<Json<CreateSessionRequest>>,
) -> Json<Session> {
    let payload = payload.map(|Json(payload)| payload).unwrap_or_default();
    let directory = payload
        .directory
        .or(params.directory)
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .or_else(|| global_store().get_current_directory());
    let persisted_config = directory.as_deref().map(load_config).unwrap_or_default();
    let kill_processes_on_start = payload
        .kill_processes_on_start
        .or(persisted_config.kill_processes_on_start)
        .unwrap_or(false);
    let validator_enabled = payload
        .validator_enabled
        .or(persisted_config.validator_enabled)
        .unwrap_or(false);
    let force_planning = payload
        .force_planning
        .or(persisted_config.force_planning)
        .unwrap_or(false);
    let model_variant = payload.model_variant.or(persisted_config.model_variant);
    let model_acceleration_enabled = payload
        .model_acceleration_enabled
        .or(persisted_config.model_acceleration_enabled)
        .unwrap_or(false);
    let disable_permission_restrictions = payload.disable_permission_restrictions.unwrap_or(false);
    let auto_session_name = payload.auto_session_name.unwrap_or(true);
    if kill_processes_on_start {
        if let Some(directory) = directory.as_deref() {
            let cleanup = tokio::task::spawn_blocking({
                let directory = directory.to_string();
                move || {
                    if let Err(err) = kill_processes_in_directory(&directory) {
                        tracing::warn!(directory, error = %err, "session startup process cleanup failed");
                    }
                }
            })
            .await;
            if let Err(err) = cleanup {
                tracing::warn!(error = %err, "session startup process cleanup task failed");
            }
        }
    }
    let requested_session_type = payload
        .session_type
        .clone()
        .or(persisted_config.session_type.clone())
        .unwrap_or_else(|| "coding".to_string());
    let requested_agent = payload
        .agent
        .clone()
        .or(persisted_config.active_agent.clone());
    let mut session = session_store().create_session(
        directory,
        payload.model.or(persisted_config.model),
        requested_agent,
        Some(requested_session_type),
        kill_processes_on_start,
        validator_enabled,
        force_planning,
        model_variant,
        model_acceleration_enabled,
        disable_permission_restrictions,
    );
    if auto_session_name != session.auto_session_name {
        if let Some(updated) =
            session_store().update_session_auto_session_name(&session.id, auto_session_name)
        {
            session = updated;
        }
    }
    if let Some(task_management) = payload.task_management {
        if let Some(updated) = session_store().update_session(
            &session.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(task_management),
        ) {
            session = updated;
        }
    }
    session_store().push_event(GlobalEvent::SessionCreated {
        properties: SessionCreatedProperties {
            session_id: session.id.clone(),
            info: session.clone(),
        },
    });
    Json(session)
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CreateSessionRequest {
    pub directory: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: Option<String>,
    pub kill_processes_on_start: Option<bool>,
    pub validator_enabled: Option<bool>,
    pub force_planning: Option<bool>,
    pub model_variant: Option<String>,
    pub model_acceleration_enabled: Option<bool>,
    pub disable_permission_restrictions: Option<bool>,
    #[serde(alias = "autoSessionName")]
    pub auto_session_name: Option<bool>,
    pub task_management: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionDirectoryParams {
    pub directory: Option<String>,
}

pub async fn get_session_config(
    headers: HeaderMap,
    Query(params): Query<SessionDirectoryParams>,
) -> Json<TuraSessionConfig> {
    let directory = params
        .directory
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .or_else(|| global_store().get_current_directory());
    Json(directory.as_deref().map(load_config).unwrap_or_default())
}

pub async fn patch_session_config(
    headers: HeaderMap,
    Query(params): Query<SessionDirectoryParams>,
    Json(payload): Json<TuraSessionConfig>,
) -> Json<TuraSessionConfig> {
    let directory = params
        .directory
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .or_else(|| global_store().get_current_directory());
    let Some(directory) = directory else {
        return Json(TuraSessionConfig::default());
    };
    match merge_config(directory, payload) {
        Ok(config) => Json(config),
        Err(err) => {
            tracing::warn!(error = %err, "failed to patch session config");
            Json(TuraSessionConfig::default())
        }
    }
}

fn repo_root_for_router() -> Option<PathBuf> {
    let mut starts = Vec::new();
    if let Ok(current) = std::env::current_dir() {
        starts.push(current);
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }

    starts.into_iter().find_map(|start| {
        start
            .ancestors()
            .find(|candidate| {
                candidate
                    .join("crates")
                    .join("router")
                    .join("Cargo.toml")
                    .exists()
            })
            .map(PathBuf::from)
    })
}

fn encoded_header(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?;
    Some(percent_decode(value))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                output.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

// ============================================================================
// Session Operations
// ============================================================================

pub async fn delete_session(Path(session_id): Path<String>) -> Json<bool> {
    let info = session_store().get_session(&session_id);
    let deleted = session_store().delete_session(&session_id);
    if deleted {
        if let Some(info) = info {
            session_store().push_event(GlobalEvent::SessionDeleted {
                properties: SessionDeletedProperties { session_id, info },
            });
        }
    }
    Json(deleted)
}

pub async fn update_session(
    Path(session_id): Path<String>,
    Json(payload): Json<UpdateSessionRequest>,
) -> Json<Session> {
    if let Some(auto_session_name) = payload.auto_session_name {
        let _ = session_store().update_session_auto_session_name(&session_id, auto_session_name);
    }
    let session = session_store()
        .update_session(
            &session_id,
            payload.title.or(payload.name),
            payload.model,
            payload.agent,
            payload.session_type,
            None,
            payload.validator_enabled,
            payload.force_planning,
            payload.disable_permission_restrictions,
            payload.task_management,
        )
        .unwrap_or_else(|| Session {
            id: session_id.clone(),
            name: None,
            parent_id: None,
            created_at: 0,
            updated_at: 0,
            directory: None,
            model: None,
            agent: Some("thinking-planning".to_string()),
            session_type: Some("coding".to_string()),
            auto_session_name: true,
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
            model_variant: None,
            model_acceleration_enabled: false,
            disable_permission_restrictions: false,
            status: SessionStatus::Idle,
            message_count: 0,
            task_management: serde_json::json!({}),
            plan_summary: None,
            session_display_name: None,
        });

    session_store().push_event(GlobalEvent::SessionUpdated {
        properties: SessionUpdatedProperties {
            session_id: session.id.clone(),
            info: session.clone(),
        },
    });

    Json(session)
}

pub async fn update_session_task_management(
    Path(session_id): Path<String>,
    Json(payload): Json<UpdateSessionTaskManagementRequest>,
) -> Json<Session> {
    let session = session_store()
        .update_session(
            &session_id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(payload.task_management),
        )
        .unwrap_or_else(|| Session {
            id: session_id.clone(),
            name: None,
            parent_id: None,
            created_at: 0,
            updated_at: 0,
            directory: None,
            model: None,
            agent: Some("thinking-planning".to_string()),
            session_type: Some("coding".to_string()),
            auto_session_name: true,
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
            model_variant: None,
            model_acceleration_enabled: false,
            disable_permission_restrictions: false,
            status: SessionStatus::Idle,
            message_count: 0,
            task_management: serde_json::json!({}),
            plan_summary: None,
            session_display_name: None,
        });
    session_store().push_event(GlobalEvent::SessionUpdated {
        properties: SessionUpdatedProperties {
            session_id: session.id.clone(),
            info: session.clone(),
        },
    });
    Json(session)
}

#[derive(Debug, Deserialize)]
pub struct UpdateSessionTaskManagementRequest {
    pub task_management: serde_json::Value,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct UpdateSessionRequest {
    pub title: Option<String>,
    pub name: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: Option<String>,
    pub kill_processes_on_start: Option<bool>,
    pub validator_enabled: Option<bool>,
    pub force_planning: Option<bool>,
    pub disable_permission_restrictions: Option<bool>,
    #[serde(alias = "autoSessionName")]
    pub auto_session_name: Option<bool>,
    pub task_management: Option<serde_json::Value>,
}

pub async fn abort_session(Path(session_id): Path<String>) -> Json<AbortResponse> {
    let aborted_sessions = session_store().cancellation_scope_session_ids(&session_id);

    let directories = aborted_sessions
        .iter()
        .filter_map(|id| session_store().get_session(id))
        .filter_map(|session| session.directory)
        .map(PathBuf::from)
        .filter(|directory| directory.exists())
        .collect::<BTreeSet<_>>();
    let mut cleanups = Vec::new();
    for directory in directories {
        if let Some(cleanup) =
            tokio::task::spawn_blocking(move || kill_processes_in_directory(directory))
                .await
                .ok()
                .and_then(Result::ok)
        {
            cleanups.push(cleanup);
        }
    }

    for id in &aborted_sessions {
        session_store().mark_cancelled(id);
        session_store().finish_todos(id, false);
        session_store().update_session_status(id, SessionStatusMano::Idle);
    }

    Json(AbortResponse {
        aborted: true,
        sessions: aborted_sessions,
        cleanup: cleanups.first().cloned(),
        cleanups,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AbortResponse {
    pub aborted: bool,
    pub sessions: Vec<String>,
    pub cleanup: Option<crate::session::process_cleanup::DirectoryProcessCleanup>,
    pub cleanups: Vec<crate::session::process_cleanup::DirectoryProcessCleanup>,
}

pub async fn fork_session(
    Path(session_id): Path<String>,
    Json(payload): Json<ForkSessionRequest>,
) -> Json<Session> {
    let original = session_store().get_session(&session_id);
    let new_session = session_store().create_session(
        payload
            .directory
            .or_else(|| original.as_ref().and_then(|s| s.directory.clone())),
        payload
            .model
            .or_else(|| original.as_ref().and_then(|s| s.model.clone())),
        payload
            .agent
            .or_else(|| original.as_ref().and_then(|s| s.agent.clone())),
        original.as_ref().and_then(|s| s.session_type.clone()),
        original
            .as_ref()
            .map(|session| session.kill_processes_on_start)
            .unwrap_or(false),
        original
            .as_ref()
            .map(|session| session.validator_enabled)
            .unwrap_or(false),
        original
            .as_ref()
            .map(|session| session.force_planning)
            .unwrap_or(false),
        original
            .as_ref()
            .and_then(|session| session.model_variant.clone()),
        original
            .as_ref()
            .map(|session| session.model_acceleration_enabled)
            .unwrap_or(false),
        original
            .as_ref()
            .map(|session| session.disable_permission_restrictions)
            .unwrap_or(false),
    );
    Json(new_session)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ForkSessionRequest {
    pub directory: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
}

pub async fn session_status() -> Json<std::collections::HashMap<String, serde_json::Value>> {
    let sessions = session_store().list_sessions();
    let statuses = sessions
        .into_iter()
        .map(|s| {
            (
                s.id.clone(),
                serde_json::json!({
                    "status": session_status_value(&s.status),
                    "task_management": s.task_management,
                    "plan_summary": s.plan_summary,
                    "session_display_name": s.session_display_name,
                }),
            )
        })
        .collect();
    Json(statuses)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionStatusResponse {
    pub session_id: String,
    pub status: SessionStatus,
    pub task_management: serde_json::Value,
    pub plan_summary: Option<String>,
    pub session_display_name: Option<String>,
}

pub(crate) fn session_status_value(status: &SessionStatus) -> serde_json::Value {
    match status {
        SessionStatus::Idle => serde_json::json!({ "type": "idle" }),
        SessionStatus::Busy => serde_json::json!({ "type": "busy" }),
        SessionStatus::Error => serde_json::json!({ "type": "error" }),
    }
}

pub async fn share_session(Path(session_id): Path<String>) -> Json<ShareResponse> {
    Json(ShareResponse {
        url: format!("https://share.example.com/session/{}", session_id),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ShareResponse {
    pub url: String,
}

// ============================================================================
// Session Children
// ============================================================================

pub async fn session_children(Path(session_id): Path<String>) -> Json<Vec<Session>> {
    Json(session_store().list_child_sessions(&session_id))
}

pub async fn session_user_commands(Path(session_id): Path<String>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "session_id": session_id,
        "commands": session_store().take_user_commands_for_session(&session_id),
    }))
}

pub async fn append_session_user_command(
    Path(session_id): Path<String>,
    Json(payload): Json<AppendUserCommandRequest>,
) -> Json<serde_json::Value> {
    let commands = session_store().append_user_command(&session_id, payload.command);
    Json(serde_json::json!({
        "ok": true,
        "session_id": session_id,
        "commands": commands,
    }))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AppendUserCommandRequest {
    pub command: String,
}

pub async fn register_child_session(
    Path(session_id): Path<String>,
    Json(payload): Json<RegisterChildSessionRequest>,
) -> Json<Session> {
    let session = session_store().register_child_session(
        &session_id,
        &payload.child_session_id,
        Some(payload.directory.clone()),
        Some(payload.name.clone()),
        Some(payload.task_instruction.clone()),
    );
    session_store().push_event(GlobalEvent::SessionCreated {
        properties: SessionCreatedProperties {
            session_id: session.id.clone(),
            info: session.clone(),
        },
    });
    Json(session)
}

pub async fn update_session_status_for_runtime(
    Path(session_id): Path<String>,
    Json(payload): Json<RuntimeSessionStatusRequest>,
) -> Json<SessionStatusResponse> {
    let status = match payload.status.as_str() {
        "busy" | "running" => SessionStatusMano::Busy,
        "error" | "failed" => SessionStatusMano::Error,
        _ => SessionStatusMano::Idle,
    };
    session_store().update_session_status(&session_id, status);
    let api_status = match status {
        SessionStatusMano::Idle => SessionStatus::Idle,
        SessionStatusMano::Busy => SessionStatus::Busy,
        SessionStatusMano::Error => SessionStatus::Error,
    };
    let session = session_store().get_session(&session_id);
    Json(SessionStatusResponse {
        session_id,
        status: api_status,
        task_management: session
            .as_ref()
            .map(|session| session.task_management.clone())
            .unwrap_or_else(|| serde_json::json!({})),
        plan_summary: session
            .as_ref()
            .and_then(|session| session.plan_summary.clone()),
        session_display_name: session.and_then(|session| session.session_display_name),
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RuntimeSessionStatusRequest {
    pub status: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegisterChildSessionRequest {
    pub child_session_id: String,
    pub directory: String,
    pub name: String,
    pub task_instruction: String,
}

// ============================================================================
// Message Operations
// ============================================================================

#[path = "session_messages.rs"]
mod session_messages;
#[cfg(test)]
use session_messages::{agent_message_content, agent_message_metadata, planning_todos};
pub use session_messages::{
    get_message, get_message_part, get_todos, list_messages, send_agent_message, send_message,
    session_command, update_todos, SendAgentMedia, SendAgentMessageRequest,
    SendAgentMessageResponse, SendAgentToolCall,
};
pub async fn revert_session(Path(session_id): Path<String>) -> Json<bool> {
    Json(
        apply_session_change_records(&session_id, ChangeDirection::Revert).unwrap_or_else(
            |error| {
                tracing::warn!(session_id, error = %error, "session revert failed");
                false
            },
        ),
    )
}

pub async fn unrevert_session(Path(session_id): Path<String>) -> Json<bool> {
    Json(
        apply_session_change_records(&session_id, ChangeDirection::Unrevert).unwrap_or_else(
            |error| {
                tracing::warn!(session_id, error = %error, "session unrevert failed");
                false
            },
        ),
    )
}

#[derive(Debug, Clone, Copy)]
enum ChangeDirection {
    Revert,
    Unrevert,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SessionChangeRecord {
    path: String,
    before_exists: bool,
    before_content: Option<String>,
    after_exists: bool,
    after_content: Option<String>,
    #[serde(default)]
    reverted: bool,
}

fn apply_session_change_records(
    session_id: &str,
    direction: ChangeDirection,
) -> Result<bool, String> {
    let directory = session_store()
        .get_session(session_id)
        .and_then(|session| session.directory)
        .ok_or_else(|| format!("session {session_id} has no directory"))?;
    let tracker_path = session_change_tracker_path(&directory, session_id);
    let content = fs::read_to_string(&tracker_path).map_err(|error| {
        format!(
            "failed to read change tracker {}: {error}",
            tracker_path.display()
        )
    })?;
    let mut records: Vec<SessionChangeRecord> = serde_json::from_str(&content)
        .map_err(|error| format!("failed to parse change tracker: {error}"))?;
    let mut changed_any = false;
    let mut skipped = false;

    match direction {
        ChangeDirection::Revert => {
            for record in records.iter_mut().rev().filter(|record| !record.reverted) {
                match apply_single_change(record, true) {
                    Ok(true) => {
                        record.reverted = true;
                        changed_any = true;
                    }
                    Ok(false) => skipped = true,
                    Err(error) => {
                        tracing::warn!(path = %record.path, error = %error, "failed to revert tracked file change");
                        skipped = true;
                    }
                }
            }
        }
        ChangeDirection::Unrevert => {
            for record in records.iter_mut().filter(|record| record.reverted) {
                match apply_single_change(record, false) {
                    Ok(true) => {
                        record.reverted = false;
                        changed_any = true;
                    }
                    Ok(false) => skipped = true,
                    Err(error) => {
                        tracing::warn!(path = %record.path, error = %error, "failed to unrevert tracked file change");
                        skipped = true;
                    }
                }
            }
        }
    }

    let updated = serde_json::to_string_pretty(&records)
        .map_err(|error| format!("failed to serialize change tracker: {error}"))?;
    fs::write(&tracker_path, updated).map_err(|error| {
        format!(
            "failed to write change tracker {}: {error}",
            tracker_path.display()
        )
    })?;
    Ok(changed_any && !skipped)
}

fn apply_single_change(record: &SessionChangeRecord, revert: bool) -> Result<bool, String> {
    let path = PathBuf::from(&record.path);
    let expected = if revert {
        record.after_content.as_deref()
    } else {
        record.before_content.as_deref()
    };
    let current = fs::read_to_string(&path).ok();
    if current.as_deref() != expected {
        return Ok(false);
    }

    let target_exists = if revert {
        record.before_exists
    } else {
        record.after_exists
    };
    let target_content = if revert {
        record.before_content.as_deref()
    } else {
        record.after_content.as_deref()
    };

    if target_exists {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&path, target_content.unwrap_or_default()).map_err(|error| error.to_string())?;
    } else if path.exists() {
        fs::remove_file(&path).map_err(|error| error.to_string())?;
    }
    Ok(true)
}

fn session_change_tracker_path(directory: &str, session_id: &str) -> PathBuf {
    PathBuf::from(directory)
        .join(".tura")
        .join("session_changes")
        .join(format!("{session_id}.json"))
}

// ============================================================================
// Session Summarize
// ============================================================================

#[path = "session_summary.rs"]
mod session_summary;
pub use session_summary::{summarize_session, SummaryResponse};

// ============================================================================
// Session Shell
// ============================================================================

#[path = "session_shell.rs"]
mod session_shell;
use session_shell::{run_session_shell_command, truncate_summary_text};
pub use session_shell::{session_shell, ShellRequest, ShellResponse};

// ============================================================================
// Async Prompt
// ============================================================================

#[path = "session_prompt.rs"]
mod session_prompt;
use session_prompt::{final_agent_message, frontend_safe_reply_message, run_mano_for_prompt};
#[cfg(test)]
use session_prompt::{
    first_prompt_part_id, prompt_message_id, prompt_model_acceleration, prompt_model_variant,
    prompt_text, user_facing_completion_fallback,
};
pub use session_prompt::{prompt_async, start_task_scheduler};
#[path = "session_format.rs"]
mod session_format;
pub(crate) use session_format::api_message_from_store;
#[cfg(test)]
use session_format::frontend_safe_value;
use session_format::{message_with_parts_from_store, part_json};

#[path = "session_tui.rs"]
mod session_tui;
pub use session_tui::tui_action;

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
