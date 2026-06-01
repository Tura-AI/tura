//! Session API handlers

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
use std::path::{Path as StdPath, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::Duration;

use code_tools_suite::state_machine::session_management::StartCondition;

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
                force_multiple_tasks: false,
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
    let force_multiple_tasks = payload
        .force_multiple_tasks
        .or(persisted_config.force_multiple_tasks)
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
        force_multiple_tasks,
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
    pub force_multiple_tasks: Option<bool>,
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

fn router_executable_candidates(root: &StdPath) -> Vec<PathBuf> {
    let executable = if cfg!(windows) {
        "tura_router.exe"
    } else {
        "tura_router"
    };
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join(executable));
        }
    }
    candidates.push(root.join("target").join("release").join(executable));
    candidates.push(root.join("target").join("debug").join(executable));
    candidates
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
            payload.force_multiple_tasks,
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
            agent: Some("coding_agent_planning".to_string()),
            session_type: Some("coding".to_string()),
            auto_session_name: true,
            kill_processes_on_start: false,
            validator_enabled: false,
            force_multiple_tasks: false,
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
            agent: Some("coding_agent_planning".to_string()),
            session_type: Some("coding".to_string()),
            auto_session_name: true,
            kill_processes_on_start: false,
            validator_enabled: false,
            force_multiple_tasks: false,
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
    pub force_multiple_tasks: Option<bool>,
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
            .map(|session| session.force_multiple_tasks)
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
        "commands": session_store().user_commands_for_session(&session_id),
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

pub async fn list_messages(Path(session_id): Path<String>) -> Json<Vec<serde_json::Value>> {
    let messages = session_store().get_messages(&session_id);
    let api_messages: Vec<serde_json::Value> = messages
        .into_iter()
        .map(message_with_parts_from_store)
        .collect();
    Json(api_messages)
}

pub async fn send_message(
    Path(session_id): Path<String>,
    Json(payload): Json<SendMessageRequest>,
) -> Json<Message> {
    session_store().add_message(
        &session_id,
        SessionMessageRole::User,
        payload.content.clone(),
    );
    session_store().update_session_status(&session_id, SessionStatusMano::Busy);
    let before_count = session_store().get_messages(&session_id).len();
    run_mano_for_prompt(
        session_id.clone(),
        serde_json::json!({
            "parts": [{
                "type": "text",
                "text": payload.content,
            }]
        }),
    );

    if let Some(msg) = final_agent_message(&session_id, before_count) {
        return Json(api_message_from_store(msg));
    }

    Json(Message {
        id: "error".to_string(),
        session_id,
        role: MessageRole::Assistant,
        parts: vec![],
        created_at: 0,
        updated_at: 0,
        parent_id: None,
    })
}

pub async fn send_agent_message(
    Path(session_id): Path<String>,
    Json(payload): Json<SendAgentMessageRequest>,
) -> Json<SendAgentMessageResponse> {
    let content = agent_message_content(&payload);
    let message = if content.trim().is_empty() {
        None
    } else {
        session_store().add_message_with_metadata(
            &session_id,
            SessionMessageRole::Assistant,
            content,
            agent_message_metadata(&payload),
        )
    };
    let tool_message = payload.tool_call.as_ref().and_then(|tool_call| {
        if let Some(todos) = multiple_tasks_todos(tool_call) {
            session_store().set_todos(&session_id, todos);
        }
        session_store().add_tool_message(
            &session_id,
            tool_call.tool_name.clone(),
            tool_call.call_id.clone(),
            tool_call.state.clone(),
            tool_call.metadata.clone(),
        )
    });
    sync_auto_session_name_from_agent_tool_call(&session_id, payload.tool_call.as_ref());

    match message.or(tool_message) {
        Some(message) => Json(SendAgentMessageResponse {
            ok: true,
            session_id: session_id.clone(),
            message_id: Some(message.id.clone()),
            event: {
                let info = api_message_from_store(message.clone());
                let event = GlobalEvent::MessageUpdated {
                    properties: MessageUpdatedProperties {
                        session_id: session_id.clone(),
                        info,
                    },
                };
                session_store().push_event(event.clone());
                Some(event)
            },
            error: None,
        }),
        None => Json(SendAgentMessageResponse {
            ok: false,
            session_id,
            message_id: None,
            event: None,
            error: Some("failed to store agent message".to_string()),
        }),
    }
}

fn sync_auto_session_name_from_agent_tool_call(
    session_id: &str,
    tool_call: Option<&SendAgentToolCall>,
) {
    let Some(summary) = tool_call.and_then(last_task_summary_from_tool_call) else {
        return;
    };
    let Some(current_session) = session_store().get_session(session_id) else {
        return;
    };
    let default_name = current_session
        .name
        .as_deref()
        .is_none_or(|name| name.trim().is_empty() || name.starts_with("Session-"));
    if !current_session.auto_session_name && !default_name {
        return;
    }
    let Some(session) = session_store().update_session(
        session_id,
        Some(summary),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ) else {
        return;
    };
    session_store().push_event(GlobalEvent::SessionUpdated {
        properties: SessionUpdatedProperties {
            session_id: session.id.clone(),
            info: session,
        },
    });
}

fn last_task_summary_from_tool_call(tool_call: &SendAgentToolCall) -> Option<String> {
    let mut summaries = Vec::new();
    if let Some(output) = tool_call
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("output"))
    {
        collect_task_summaries(output, &mut summaries);
    }
    if summaries.is_empty() {
        if let Some(output) = tool_call
            .state
            .get("metadata")
            .and_then(|metadata| metadata.get("output"))
            .or_else(|| tool_call.state.get("output"))
        {
            collect_task_summaries(output, &mut summaries);
        }
    }
    if summaries.is_empty() {
        collect_task_summaries(&tool_call.state, &mut summaries);
    }
    summaries.pop()
}

fn collect_task_summaries(value: &serde_json::Value, summaries: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(summary) = object
                .get("task_summary")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                summaries.push(summary.to_string());
            }
            for child in object.values() {
                collect_task_summaries(child, summaries);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_task_summaries(item, summaries);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SendAgentMessageRequest {
    pub reply_message: String,
    pub new_learning: String,
    pub step_summary: Option<String>,
    #[serde(default)]
    pub media: Vec<SendAgentMedia>,
    pub runtime_id: Option<String>,
    pub tool_call: Option<SendAgentToolCall>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SendAgentToolCall {
    pub tool_name: String,
    pub call_id: String,
    pub state: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SendAgentMedia {
    pub path: String,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SendAgentMessageResponse {
    pub ok: bool,
    pub session_id: String,
    pub message_id: Option<String>,
    pub event: Option<GlobalEvent>,
    pub error: Option<String>,
}

fn agent_message_content(payload: &SendAgentMessageRequest) -> String {
    if payload.tool_call.is_some()
        && payload.reply_message.trim().is_empty()
        && payload.media.is_empty()
    {
        return String::new();
    }

    let mut content = frontend_safe_reply_message(&payload.reply_message);

    if !payload.media.is_empty() {
        if !content.trim().is_empty() {
            content.push_str("\n\n");
        }
        for item in &payload.media {
            content.push_str("[MEDIA:");
            content.push_str(&item.path);
            content.push_str(":MEDIA]\n");
        }
    }

    content
}

fn agent_message_metadata(payload: &SendAgentMessageRequest) -> Option<serde_json::Value> {
    let step_summary = payload
        .step_summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    step_summary?;

    let mut metadata = serde_json::Map::new();
    if let Some(step_summary) = step_summary {
        metadata.insert(
            "step_summary".to_string(),
            serde_json::Value::String(step_summary.to_string()),
        );
    }
    Some(serde_json::Value::Object(metadata))
}

fn multiple_tasks_todos(tool_call: &SendAgentToolCall) -> Option<Vec<serde_json::Value>> {
    if tool_call.tool_name != "multiple_tasks" {
        return None;
    }

    let input = tool_call
        .state
        .get("input")
        .or_else(|| tool_call.metadata.as_ref()?.get("input"))?;
    let steps = input.get("steps")?.as_array()?;
    if steps.is_empty() {
        return None;
    }

    let status = tool_call
        .state
        .get("status")
        .and_then(|value| value.as_str());
    let output_steps = multiple_tasks_output_steps(tool_call);
    let running_step = if status == Some("running") {
        steps
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let number = index + 1;
                !output_steps.iter().any(|item| {
                    item.get("index").and_then(|value| value.as_u64()) == Some(number as u64)
                })
            })
            .map(|(index, step)| multiple_tasks_step_value(step, index + 1))
            .min()
    } else {
        None
    };

    Some(
        steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let number = index + 1;
                let step_value = multiple_tasks_step_value(step, number);
                let output_step = output_steps.iter().find(|item| {
                    item.get("index").and_then(|value| value.as_u64()) == Some(number as u64)
                });
                let status = match output_step {
                    Some(item)
                        if item.get("ok").and_then(|value| value.as_bool()) == Some(true) =>
                    {
                        "completed"
                    }
                    Some(_) => "cancelled",
                    None if status == Some("running") && Some(step_value) == running_step => {
                        "in_progress"
                    }
                    None if status == Some("pending") => "pending",
                    None if matches!(status, Some("completed" | "error")) => "cancelled",
                    None => "pending",
                };
                serde_json::json!({
                    "id": format!("{}:{number}", tool_call.call_id),
                    "content": todo_content(step, number),
                    "status": status,
                    "priority": "medium",
                })
            })
            .collect(),
    )
}

fn multiple_tasks_step_value(step: &serde_json::Value, fallback: usize) -> usize {
    step.get("step")
        .and_then(|value| value.as_u64())
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(fallback)
}

fn multiple_tasks_output_steps(tool_call: &SendAgentToolCall) -> Vec<serde_json::Value> {
    let raw = tool_call
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("output"))
        .or_else(|| tool_call.state.get("output"));
    let Some(output) = raw.and_then(parse_json_value) else {
        return Vec::new();
    };
    let result = output
        .get("results")
        .and_then(|results| results.as_array())
        .and_then(|results| results.iter().find(|value| value.is_object()))
        .unwrap_or(&output);

    result
        .get("steps")
        .and_then(|steps| steps.as_array())
        .cloned()
        .unwrap_or_default()
}

fn parse_json_value(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::String(text) => serde_json::from_str(text).ok(),
        value if value.is_object() => Some(value.clone()),
        _ => None,
    }
}

fn todo_content(step: &serde_json::Value, number: usize) -> String {
    step.get("step_goal")
        .and_then(|value| value.as_str())
        .or_else(|| {
            step.get("task_instruction")
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("Step {number}"))
}

pub async fn get_message(
    Path((session_id, message_id)): Path<(String, String)>,
) -> Json<serde_json::Value> {
    let messages = session_store().get_messages(&session_id);
    let message = messages
        .into_iter()
        .find(|m| m.id == message_id)
        .map(message_with_parts_from_store)
        .unwrap_or_else(|| {
            serde_json::json!({
                "info": {
                    "id": message_id,
                    "sessionID": session_id,
                    "role": "user",
                    "time": { "created": 0 },
                    "parts": [],
                },
                "parts": [],
            })
        });
    Json(message)
}

pub async fn get_message_part(
    Path((session_id, message_id, part_id)): Path<(String, String, String)>,
) -> Json<serde_json::Value> {
    let messages = session_store().get_messages(&session_id);
    let message = messages.into_iter().find(|m| m.id == message_id);

    let part = message
        .and_then(|m| m.parts.into_iter().find(|p| p.id == part_id))
        .map(|p| part_json(&session_id, &message_id, p))
        .unwrap_or_else(|| {
            serde_json::json!({
                "id": part_id,
                "sessionID": session_id,
                "messageID": message_id,
                "type": "text",
                "text": "",
            })
        });
    Json(part)
}

// ============================================================================
// Session Permissions
// ============================================================================

pub async fn list_permissions(Path(session_id): Path<String>) -> Json<Vec<PermissionRequest>> {
    Json(global_store().list_permissions(&session_id))
}

pub async fn create_permission(
    Path(session_id): Path<String>,
    Json(payload): Json<PermissionCreateRequest>,
) -> Json<PermissionRequest> {
    Json(global_store().create_permission(session_id, payload.permission, payload.args))
}

pub async fn reply_permission(
    Path(request_id): Path<String>,
    Json(payload): Json<PermissionReplyRequest>,
) -> Json<PermissionReplyResponse> {
    Json(PermissionReplyResponse {
        success: global_store().reply_permission(&request_id, payload.approve),
    })
}

pub async fn get_permission_reply(
    Path(permission_id): Path<String>,
) -> Json<PermissionStatusResponse> {
    let approve = global_store().permission_reply(&permission_id);
    Json(PermissionStatusResponse {
        responded: approve.is_some(),
        approve,
    })
}

pub async fn list_session_permission_by_id(
    Path((session_id, permission_id)): Path<(String, String)>,
) -> Json<PermissionRequest> {
    let permissions = global_store().list_permissions(&session_id);
    let found = permissions.into_iter().find(|p| p.id == permission_id);
    Json(found.unwrap_or_else(|| PermissionRequest {
        id: permission_id,
        session_id,
        permission: "not_found".to_string(),
        args: std::collections::HashMap::new(),
    }))
}

// ============================================================================
// Session Commands
// ============================================================================

pub async fn session_command(
    Path(session_id): Path<String>,
    Json(payload): Json<CommandRequest>,
) -> Json<CommandResponse> {
    let directory = session_store()
        .get_session(&session_id)
        .and_then(|session| session.directory)
        .unwrap_or_else(|| ".".to_string());
    let output = run_session_shell_command(&directory, &payload.command)
        .unwrap_or_else(|error| format!("failed to run session command: {error}"));
    Json(CommandResponse { output })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandResponse {
    pub output: String,
}

// ============================================================================
// Session Todo
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
}

pub async fn get_todos(Path(session_id): Path<String>) -> Json<Vec<serde_json::Value>> {
    Json(session_store().get_todos(&session_id))
}

pub async fn update_todos(
    Path(session_id): Path<String>,
    Json(payload): Json<Vec<serde_json::Value>>,
) -> Json<Vec<serde_json::Value>> {
    Json(session_store().set_todos(&session_id, payload))
}

// ============================================================================
// Session Diff
// ============================================================================

pub async fn get_session_diff(Path(session_id): Path<String>) -> Json<Vec<FileDiff>> {
    let directory = session_store()
        .get_session(&session_id)
        .and_then(|session| session.directory);
    Json(crate::api::misc::git_diff_for_directory(
        directory.as_deref(),
    ))
}

// ============================================================================
// Session Revert / Unrevert
// ============================================================================

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

pub async fn summarize_session(Path(session_id): Path<String>) -> Json<SummaryResponse> {
    let messages = session_store().get_messages(&session_id);
    let message_count = messages.len();
    let mut user_count = 0;
    let mut assistant_count = 0;
    let mut tool_count = 0;
    let mut snippets = Vec::new();

    for message in messages.iter().rev().take(8).rev() {
        match message.role {
            SessionMessageRole::User => user_count += 1,
            SessionMessageRole::Assistant => assistant_count += 1,
            SessionMessageRole::System => {}
        }
        for part in &message.parts {
            if part.tool.is_some() {
                tool_count += 1;
            }
            let text = part
                .text
                .as_deref()
                .or(part.content.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some(text) = text {
                snippets.push(format!(
                    "{}: {}",
                    match message.role {
                        SessionMessageRole::User => "User",
                        SessionMessageRole::Assistant => "Assistant",
                        SessionMessageRole::System => "System",
                    },
                    truncate_summary_text(text, 180)
                ));
            }
        }
    }

    let summary = if snippets.is_empty() {
        format!("Session {session_id} has {message_count} stored messages and no textual summary content yet.")
    } else {
        format!(
            "Session {session_id}: {message_count} messages ({user_count} user, {assistant_count} assistant), {tool_count} tool parts. Recent context:\n{}",
            snippets.join("\n")
        )
    };

    Json(SummaryResponse { summary })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SummaryResponse {
    pub summary: String,
}

// ============================================================================
// Session Shell
// ============================================================================

pub async fn session_shell(
    Path(session_id): Path<String>,
    Json(payload): Json<ShellRequest>,
) -> Json<ShellResponse> {
    let directory = session_store()
        .get_session(&session_id)
        .and_then(|session| session.directory)
        .unwrap_or_else(|| ".".to_string());
    let output = run_session_shell_command(&directory, &payload.input)
        .unwrap_or_else(|error| format!("failed to run shell command: {error}"));
    Json(ShellResponse { output })
}

fn truncate_summary_text(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in value.chars().take(max_chars) {
        output.push(ch);
    }
    if value.chars().count() > max_chars {
        output.push_str("...");
    }
    output.replace('\n', " ")
}

fn run_session_shell_command(directory: &str, input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let mut command = if cfg!(windows) {
        let mut command = ProcessCommand::new("powershell");
        command.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            trimmed,
        ]);
        command
    } else {
        let mut command = ProcessCommand::new("sh");
        command.args(["-lc", trimmed]);
        command
    };
    command.current_dir(directory);
    let output = command.output().map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut combined = String::new();
    if !stdout.trim().is_empty() {
        combined.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(stderr.trim_end());
    }
    if !output.status.success() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&format!("exit status: {}", output.status));
    }
    Ok(combined)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ShellRequest {
    pub input: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ShellResponse {
    pub output: String,
}

// ============================================================================
// Async Prompt
// ============================================================================

pub async fn prompt_async(
    Path(session_id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    session_store().clear_cancelled(&session_id);
    let content = prompt_text(&payload).unwrap_or_else(|| "Prompt submitted".to_string());
    let session = session_store().get_session(&session_id);
    if session
        .as_ref()
        .is_some_and(|session| matches!(session.status, SessionStatus::Busy))
    {
        let _ = session_store().add_message_with_ids(
            &session_id,
            SessionMessageRole::User,
            content.clone(),
            prompt_message_id(&payload),
            first_prompt_part_id(&payload),
            Some(serde_json::json!({
                "kind": "user_new_command",
            })),
        );
        session_store().append_user_command(&session_id, content);
        return StatusCode::NO_CONTENT;
    }
    let _ = session_store().add_message_with_ids(
        &session_id,
        SessionMessageRole::User,
        content,
        prompt_message_id(&payload),
        first_prompt_part_id(&payload),
        None,
    );
    session_store().update_session_status(&session_id, SessionStatusMano::Busy);
    session_store().set_todos(
        &session_id,
        vec![serde_json::json!({
            "id": format!("{session_id}:multiple_tasks"),
            "content": "规划执行步骤",
            "status": "in_progress",
            "priority": "medium",
        })],
    );
    watch_direct_mano_messages(
        session_id.clone(),
        session_store().get_messages(&session_id).len(),
    );
    let session_id_for_task = session_id.clone();
    let payload_for_task = payload.clone();
    tokio::task::spawn_blocking(move || {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_mano_for_prompt(session_id_for_task.clone(), payload_for_task);
        }))
        .is_err()
        {
            tracing::error!(session_id = %session_id_for_task, "MANO prompt task panicked");
            session_store().update_session_status(&session_id_for_task, SessionStatusMano::Error);
            session_store().finish_todos(&session_id_for_task, false);
            add_agent_fallback_message(
                &session_id_for_task,
                "MANO failed while processing this prompt: background task panicked before completion.".to_string(),
            );
        }
    });
    StatusCode::NO_CONTENT
}

pub fn start_task_scheduler() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            run_due_task_scheduler_tick();
        }
    });
}

fn run_due_task_scheduler_tick() {
    for run in session_store().claim_due_task_runs(chrono::Utc::now()) {
        let prompt = scheduler_prompt_payload(&run.task_summary, run.start_condition);
        let content = prompt_text(&prompt).unwrap_or_else(|| run.task_summary.clone());
        let initial_count = session_store().get_messages(&run.session_id).len();
        let _ = session_store().add_message_with_metadata(
            &run.session_id,
            SessionMessageRole::User,
            content,
            Some(serde_json::json!({
                "kind": "task_scheduler",
                "start_condition": run.start_condition,
            })),
        );
        session_store().set_todos(
            &run.session_id,
            vec![serde_json::json!({
                "id": format!("{}:scheduled-task", run.session_id),
                "content": run.task_summary,
                "status": "in_progress",
                "priority": "medium",
            })],
        );
        watch_direct_mano_messages(run.session_id.clone(), initial_count);
        std::thread::spawn(move || {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_mano_for_prompt(run.session_id.clone(), prompt);
                if matches!(run.start_condition, StartCondition::PollingTask) {
                    reset_polling_task_after_run(&run.session_id);
                }
            }))
            .is_err()
            {
                tracing::error!(session_id = %run.session_id, "scheduled task panicked");
                session_store().update_session_status(&run.session_id, SessionStatusMano::Error);
                session_store().finish_todos(&run.session_id, false);
                add_agent_fallback_message(
                    &run.session_id,
                    "Scheduled task failed before completion.".to_string(),
                );
            }
        });
    }
}

fn scheduler_prompt_payload(
    task_summary: &str,
    start_condition: StartCondition,
) -> serde_json::Value {
    let trigger = match start_condition {
        StartCondition::SessionIdle => "session became idle",
        StartCondition::ScheduledTask => "scheduled start time arrived",
        StartCondition::PollingTask => "polling interval became due",
        StartCondition::UserAction => "user action",
    };
    serde_json::json!({
        "parts": [{
            "id": format!("part_scheduler_{}", uuid::Uuid::new_v4()),
            "type": "text",
            "text": format!("Continue the pending task because the {trigger}: {task_summary}")
        }],
        "source": "task_scheduler"
    })
}

fn reset_polling_task_after_run(session_id: &str) {
    let Some(session) = session_store().get_session(session_id) else {
        return;
    };
    let current_status = session
        .task_management
        .get("status")
        .and_then(serde_json::Value::as_str);
    if matches!(current_status, Some("done" | "archived")) {
        return;
    }
    let _ = session_store().update_session(
        session_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(serde_json::json!({ "status": "todo" })),
    );
}

fn watch_direct_mano_messages(session_id: String, initial_count: usize) {
    std::thread::spawn(move || {
        let mut seen = initial_count;
        for _ in 0..1200 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let messages = session_store().get_messages(&session_id);
            if messages.len() > seen {
                for message in messages.iter().skip(seen).cloned() {
                    session_store().push_event(GlobalEvent::MessageUpdated {
                        properties: MessageUpdatedProperties {
                            session_id: session_id.clone(),
                            info: api_message_from_store(message),
                        },
                    });
                }
                seen = messages.len();
            }
        }
    });
}

fn prompt_text(payload: &serde_json::Value) -> Option<String> {
    let parts = payload.get("parts")?.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| {
            if part.get("type")?.as_str()? != "text" {
                return None;
            }
            part.get("text")?.as_str()
        })
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

fn prompt_message_id(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("messageID")
        .or_else(|| payload.get("message_id"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn first_prompt_part_id(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("parts")?
        .as_array()?
        .iter()
        .find(|part| part.get("type").and_then(|value| value.as_str()) == Some("text"))?
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn prompt_runtime_context(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("system")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// 通过 router CLI 转发一次 prompt：gateway 仅做转发 + 状态收尾，
/// runtime 行为全部经 router→runtime worker 子进程执行，事件经现有回调通道回报。
fn run_mano_for_prompt(session_id: String, payload: serde_json::Value) {
    let content = prompt_text(&payload).unwrap_or_else(|| "Prompt submitted".to_string());
    let before_count = session_store().get_messages(&session_id).len();
    let session = session_store().get_session(&session_id);

    let agent = prompt_agent_override(&payload)
        .or_else(|| session.as_ref().and_then(|session| session.agent.clone()));
    let runtime_context = prompt_runtime_context(&payload);
    let force_multiple_tasks = session
        .as_ref()
        .map(|session| session.force_multiple_tasks)
        .unwrap_or(false);
    let model_override = prompt_model_override(&payload)
        .or_else(|| session.as_ref().and_then(|session| session.model.clone()))
        .and_then(normalize_model_override);
    let agent_runtime_settings = agent
        .as_deref()
        .and_then(agent_runtime_settings)
        .unwrap_or_default();
    let reasoning_effort = prompt_model_variant(&payload)
        .or(agent_runtime_settings.reasoning_effort)
        .or_else(|| {
            session
                .as_ref()
                .and_then(|session| session.model_variant.clone())
                .filter(|value| !value.trim().is_empty())
        });
    let acceleration_enabled = prompt_model_acceleration(&payload)
        .or(agent_runtime_settings.acceleration_enabled)
        .or_else(|| {
            session
                .as_ref()
                .map(|session| session.model_acceleration_enabled)
        })
        .unwrap_or(false);
    let directory = session
        .as_ref()
        .and_then(|session| {
            session
                .directory
                .clone()
                .map(|directory| directory.trim().to_string())
                .filter(|directory| !directory.is_empty())
        })
        .or_else(|| global_store().get_current_directory());
    let command_run_stall_guard = directory
        .as_deref()
        .map(load_config)
        .map(|config| config.command_run_stall_guard())
        .unwrap_or_else(|| TuraSessionConfig::default().command_run_stall_guard());

    // worker env 契约：取代旧的进程级 with_*_env 注入，由 router 注入 runtime worker 子进程。
    let mut worker_env: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if force_multiple_tasks {
        worker_env.insert(
            "TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS".to_string(),
            "1".to_string(),
        );
    }
    if let Some(reasoning) = reasoning_effort
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
    {
        worker_env.insert(
            "TURA_SESSION_REASONING_EFFORT".to_string(),
            reasoning.to_string(),
        );
    }
    if acceleration_enabled {
        worker_env.insert(
            "TURA_SESSION_ACCELERATION_ENABLED".to_string(),
            "1".to_string(),
        );
    }
    worker_env.insert(
        "TURA_COMMAND_RUN_STALL_CHECK_SECS".to_string(),
        command_run_stall_guard.check_secs.to_string(),
    );
    worker_env.insert(
        "TURA_COMMAND_RUN_STALL_IDENTICAL_CHECKS".to_string(),
        command_run_stall_guard.identical_checks.to_string(),
    );

    let body = serde_json::json!({
        "session_id": session_id,
        "directory": directory,
        "model": model_override,
        "agent": agent,
        "prompt": content,
        "runtime_context": runtime_context,
        "worker_env": worker_env,
    });

    let result = forward_run_agent_to_router(&body);

    if session_store().is_cancelled(&session_id) {
        session_store().finish_todos(&session_id, false);
        session_store().update_session_status(&session_id, SessionStatusMano::Idle);
        return;
    }

    match result {
        Ok(()) => {
            session_store().update_session_status(&session_id, SessionStatusMano::Idle);
            session_store().finish_todos(&session_id, true);
            if let Some(message) = final_agent_message(&session_id, before_count) {
                session_store().push_event(GlobalEvent::MessageUpdated {
                    properties: MessageUpdatedProperties {
                        session_id: session_id.clone(),
                        info: api_message_from_store(message),
                    },
                });
            } else {
                add_agent_fallback_message(
                    &session_id,
                    "MANO completed without a user-facing message.".to_string(),
                );
            }
        }
        Err(error) => {
            session_store().update_session_status(&session_id, SessionStatusMano::Error);
            session_store().finish_todos(&session_id, false);
            add_agent_fallback_message(
                &session_id,
                format!("MANO failed while processing this prompt: {error}"),
            );
        }
    }
}

/// 阻塞式通过 router CLI 派发 runtime worker（在 spawn_blocking / std thread 内调用）。
fn forward_run_agent_to_router(body: &serde_json::Value) -> Result<(), String> {
    let root = repo_root_for_router()
        .ok_or_else(|| "failed to locate repository root for router CLI".to_string())?;
    let router = router_executable_candidates(&root)
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| "router binary not found in target/{release,debug}".to_string())?;
    let mut command = ProcessCommand::new(&router);
    command
        .arg("run-agent")
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Ok(port) = std::env::var("PORT") {
        command.env("TURA_GATEWAY_PORT", port);
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to start router CLI {}: {err}", router.display()))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let encoded = serde_json::to_vec(body)
            .map_err(|err| format!("failed to encode router run-agent payload: {err}"))?;
        stdin
            .write_all(&encoded)
            .map_err(|err| format!("failed to write router run-agent payload: {err}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("router CLI run-agent failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|err| {
        format!(
            "router CLI run-agent returned invalid response: {err}; output={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })?;
    if value
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
    {
        Ok(())
    } else {
        Err(value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("router CLI run-agent returned failure")
            .to_string())
    }
}

fn prompt_model_override(payload: &serde_json::Value) -> Option<String> {
    let model = payload.get("model")?;
    if let Some(value) = model.as_str() {
        return Some(value.to_string());
    }
    let provider = model
        .get("providerID")
        .or_else(|| model.get("provider_id"))
        .and_then(|value| value.as_str())?;
    let model_id = model
        .get("modelID")
        .or_else(|| model.get("model_id"))
        .and_then(|value| value.as_str())?;
    Some(format!("{provider}/{model_id}"))
}

fn prompt_model_variant(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("variant")
        .or_else(|| payload.get("model_variant"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
        .map(ToString::to_string)
}

#[derive(Default)]
struct AgentRuntimeSettings {
    reasoning_effort: Option<String>,
    acceleration_enabled: Option<bool>,
}

fn agent_runtime_settings(agent_id: &str) -> Option<AgentRuntimeSettings> {
    let root = repo_root_for_router()?;
    let agent = tura_agents::store::load_agent(&root, agent_id)?;
    let provider = agent.config.provider.as_object()?;
    Some(AgentRuntimeSettings {
        reasoning_effort: provider_string(
            provider,
            &[
                "model_reasoning_effort",
                "reasoning_effort",
                "model_variant",
            ],
        ),
        acceleration_enabled: provider_bool(provider, "model_acceleration_enabled").or_else(|| {
            provider_string(provider, &["service_tier"])
                .map(|value| value.eq_ignore_ascii_case("priority"))
        }),
    })
}

fn provider_string(
    provider: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .filter_map(|key| provider.get(*key))
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
        .map(ToString::to_string)
        .next()
}

fn provider_bool(provider: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<bool> {
    provider.get(key).and_then(serde_json::Value::as_bool)
}

fn prompt_agent_override(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("agent")
        .or_else(|| payload.get("agent_id"))
        .or_else(|| payload.get("agentID"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn prompt_model_acceleration(payload: &serde_json::Value) -> Option<bool> {
    payload
        .get("model_acceleration_enabled")
        .or_else(|| payload.get("modelAccelerationEnabled"))
        .or_else(|| payload.get("accelerated"))
        .and_then(|value| value.as_bool())
}

fn normalize_model_override(value: String) -> Option<String> {
    let trimmed = value.trim();
    let (provider, model) = trimmed.split_once('/')?;
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    let provider = match provider {
        "openai-api" => "openai",
        "anthropic-api" => "anthropic",
        "antigravity-api" => "antigravity",
        other => other,
    };
    Some(format!("{provider}/{model}"))
}

fn final_agent_message(
    session_id: &str,
    before_count: usize,
) -> Option<crate::session::store::Message> {
    session_store()
        .get_messages(session_id)
        .into_iter()
        .skip(before_count)
        .find(|message| {
            message.role == SessionMessageRole::Assistant
                && message
                    .parts
                    .iter()
                    .filter_map(|part| part.text.as_deref().or(part.content.as_deref()))
                    .any(is_meaningful_final_message)
        })
}

fn is_meaningful_final_message(text: &str) -> bool {
    let trimmed = text.trim_start();
    if trimmed.starts_with("Step summary:")
        || trimmed.starts_with("MANO failed while processing this prompt:")
    {
        return false;
    }
    if looks_like_tool_payload(trimmed) {
        return false;
    }

    !strip_runtime_markup(
        trimmed
            .replace("<think>", "")
            .replace("</think>", "")
            .trim(),
    )
    .trim()
    .is_empty()
}

fn strip_runtime_markup(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_runtime_markup_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_runtime_markup_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "<invoke>" | "</invoke>" | "<tool_call>" | "</tool_call>" | "<tool>" | "</tool>"
    ) {
        return true;
    }

    if lower.starts_with('<') && lower.ends_with('>') {
        return true;
    }

    if lower.starts_with("command_run:") && (lower.contains('{') || lower.contains('[')) {
        return true;
    }

    (lower.starts_with("<invoke") && lower.ends_with('>'))
        || (lower.starts_with("</invoke") && lower.ends_with('>'))
        || (lower.starts_with("<tool_call") && lower.ends_with('>'))
        || (lower.starts_with("</tool_call") && lower.ends_with('>'))
}

fn frontend_safe_reply_message(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(reply_message) = extract_reply_message_from_json(trimmed) {
        return reply_message;
    }
    if looks_like_tool_payload(trimmed) {
        return String::new();
    }
    trimmed.to_string()
}

fn extract_reply_message_from_json(text: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|value| find_reply_message(&value))
}

fn find_reply_message(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(message) = object
                .get("reply_message")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Some(message.to_string());
            }
            object.values().find_map(find_reply_message)
        }
        serde_json::Value::Array(items) => items.iter().find_map(find_reply_message),
        _ => None,
    }
}

fn looks_like_tool_payload(text: &str) -> bool {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('{') {
        return false;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return json_looks_like_tool_payload(&value);
    }

    trimmed.contains("\"reply_message\"")
        || trimmed.contains("\"new_learning\"")
        || trimmed.contains("\"tool_calls\"")
        || trimmed.contains("\"input\"")
        || trimmed.contains("\"last_tool_call_status\"")
        || trimmed.contains("\"last_tool_call_summary\"")
        || trimmed.contains("\"step_summary\"")
}

fn json_looks_like_tool_payload(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            let has_reporting_fields = object.contains_key("last_tool_call_status")
                || object.contains_key("last_tool_call_summary")
                || object.contains_key("step_summary");
            let has_tool_shape = object.contains_key("requests")
                || object.contains_key("reply_message")
                || object.contains_key("new_learning")
                || object.contains_key("tool_calls")
                || object.contains_key("input")
                || object.contains_key("command_code")
                || object.contains_key("environment");

            (has_reporting_fields && has_tool_shape)
                || object.contains_key("tool_calls")
                || object.values().any(json_looks_like_tool_payload)
        }
        serde_json::Value::Array(items) => items.iter().any(json_looks_like_tool_payload),
        _ => false,
    }
}

fn add_agent_fallback_message(session_id: &str, content: String) {
    if let Some(message) =
        session_store().add_message(session_id, SessionMessageRole::Assistant, content)
    {
        session_store().push_event(GlobalEvent::MessageUpdated {
            properties: MessageUpdatedProperties {
                session_id: session_id.to_string(),
                info: api_message_from_store(message),
            },
        });
    }
}

pub(crate) fn api_message_from_store(message: crate::session::store::Message) -> Message {
    Message {
        id: message.id,
        session_id: message.session_id,
        role: match message.role {
            SessionMessageRole::User => MessageRole::User,
            SessionMessageRole::Assistant => MessageRole::Assistant,
            SessionMessageRole::System => MessageRole::System,
        },
        parts: message
            .parts
            .into_iter()
            .map(|part| MessagePart {
                id: part.id.clone(),
                part_type: part.part_type.clone(),
                content: part.content.clone(),
                text: part.text.clone(),
                metadata: frontend_safe_part_value(&part, part.metadata.clone()),
                call_id: part.call_id.clone(),
                tool: part.tool.clone(),
                state: frontend_safe_part_value(&part, part.state.clone()),
            })
            .collect(),
        created_at: message.created_at,
        updated_at: message.updated_at,
        parent_id: message.parent_id,
    }
}

fn message_with_parts_from_store(message: crate::session::store::Message) -> serde_json::Value {
    let session_id = message.session_id.clone();
    let message_id = message.id.clone();
    let parts: Vec<_> = message
        .parts
        .iter()
        .cloned()
        .map(|part| part_json(&session_id, &message_id, part))
        .collect();
    let mut info = serde_json::to_value(api_message_from_store(message))
        .unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = info.as_object_mut() {
        object.insert("parts".to_string(), serde_json::Value::Array(parts.clone()));
    }
    serde_json::json!({
        "info": info,
        "parts": parts,
    })
}

fn part_json(
    session_id: &str,
    message_id: &str,
    part: crate::session::store::MessagePart,
) -> serde_json::Value {
    serde_json::json!({
        "id": part.id.clone(),
        "sessionID": session_id,
        "messageID": message_id,
        "type": part.part_type.clone(),
        "text": part.text.clone().or(part.content.clone()).unwrap_or_default(),
        "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
        "callID": part.call_id.clone(),
        "tool": part.tool.clone(),
        "state": frontend_safe_part_value(&part, part.state.clone()),
    })
}

fn frontend_safe_value(value: Option<serde_json::Value>) -> Option<serde_json::Value> {
    value.map(sanitize_frontend_value)
}

fn frontend_safe_part_value(
    part: &crate::session::store::MessagePart,
    value: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    if part.part_type == "tool" && part.tool.as_deref() == Some("runtime") {
        return value;
    }
    frontend_safe_value(value)
}

fn sanitize_frontend_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let object = object
                .into_iter()
                .filter(|(key, _)| !matches!(key.as_str(), "new_learning" | "runtime_id"))
                .map(|(key, value)| (key, sanitize_frontend_value(value)))
                .collect();
            serde_json::Value::Object(object)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sanitize_frontend_value).collect())
        }
        value => value,
    }
}

pub async fn tui_action(payload: Option<Json<serde_json::Value>>) -> Json<serde_json::Value> {
    let payload = payload
        .map(|Json(payload)| payload)
        .unwrap_or(serde_json::Value::Null);
    let action = payload
        .get("action")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("submit-prompt");
    if matches!(
        action,
        "submit-prompt" | "append-prompt" | "execute-command"
    ) {
        let content = payload
            .get("prompt")
            .or_else(|| payload.get("input"))
            .or_else(|| payload.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Prompt submitted")
            .to_string();
        let session_id = payload
            .get("sessionID")
            .or_else(|| payload.get("session_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                session_store()
                    .create_session(
                        global_store().get_current_directory(),
                        None,
                        None,
                        Some("coding".to_string()),
                        false,
                        false,
                        false,
                        None,
                        false,
                        false,
                    )
                    .id
            });
        let prompt_payload = serde_json::json!({
            "parts": [{
                "id": format!("tui-part-{}", uuid::Uuid::new_v4()),
                "type": "text",
                "text": content
            }]
        });
        session_store().clear_cancelled(&session_id);
        let _ = session_store().add_message(&session_id, SessionMessageRole::User, content);
        session_store().update_session_status(&session_id, SessionStatusMano::Busy);
        let session_id_for_task = session_id.clone();
        tokio::task::spawn_blocking(move || {
            run_mano_for_prompt(session_id_for_task, prompt_payload);
        });
        return Json(serde_json::json!({
            "ok": true,
            "action": action,
            "sessionID": session_id,
            "status": "submitted"
        }));
    }
    if action == "clear-prompt" {
        return Json(serde_json::json!({
            "ok": true,
            "action": action,
            "prompt": ""
        }));
    }
    Json(serde_json::json!({
        "ok": true,
        "action": action,
        "payload": payload,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        agent_message_content, agent_message_metadata, api_message_from_store,
        filter_list_sessions, first_prompt_part_id, frontend_safe_reply_message,
        frontend_safe_value, multiple_tasks_todos, prompt_message_id, prompt_model_acceleration,
        prompt_model_variant, prompt_text, workspace_key, SendAgentMedia, SendAgentMessageRequest,
        SendAgentToolCall, SessionListParams,
    };
    use crate::api::types::{Session, SessionStatus};
    use crate::session_store;
    use axum::{
        extract::{Path, Query},
        http::HeaderMap,
        Json,
    };
    use std::fs;

    #[test]
    fn prompt_payload_keeps_frontend_message_and_part_ids() {
        let payload = serde_json::json!({
            "messageID": "msg_frontend_1",
            "parts": [
                { "id": "part_text_1", "type": "text", "text": "Read README.md" },
                { "id": "part_file_1", "type": "file", "url": "file:///README.md" }
            ]
        });

        assert_eq!(
            prompt_message_id(&payload).as_deref(),
            Some("msg_frontend_1")
        );
        assert_eq!(
            first_prompt_part_id(&payload).as_deref(),
            Some("part_text_1")
        );
        assert_eq!(prompt_text(&payload).as_deref(), Some("Read README.md"));
    }

    #[test]
    fn prompt_payload_extracts_model_runtime_options() {
        let payload = serde_json::json!({
            "variant": "high",
            "model_acceleration_enabled": true,
        });

        assert_eq!(prompt_model_variant(&payload).as_deref(), Some("high"));
        assert_eq!(prompt_model_acceleration(&payload), Some(true));
    }

    #[test]
    fn prompt_payload_treats_default_model_variant_as_unset() {
        let payload = serde_json::json!({
            "variant": " default ",
        });

        assert_eq!(prompt_model_variant(&payload), None);
    }

    fn test_session(
        id: &str,
        directory: &str,
        parent_id: Option<&str>,
        updated_at: i64,
    ) -> Session {
        Session {
            id: id.to_string(),
            name: Some(id.to_string()),
            parent_id: parent_id.map(ToString::to_string),
            created_at: updated_at - 1,
            updated_at,
            directory: Some(directory.to_string()),
            model: None,
            agent: None,
            session_type: Some("coding".to_string()),
            auto_session_name: true,
            kill_processes_on_start: false,
            validator_enabled: false,
            force_multiple_tasks: false,
            model_variant: None,
            model_acceleration_enabled: false,
            disable_permission_restrictions: false,
            status: SessionStatus::Idle,
            message_count: 0,
            task_management: serde_json::json!({}),
            plan_summary: None,
            session_display_name: None,
        }
    }

    #[test]
    fn session_list_filters_requested_directory_and_roots() {
        let sessions = vec![
            test_session("root-a", r"C:\repo", None, 10),
            test_session("child-a", r"C:\repo", Some("root-a"), 11),
            test_session("root-b", r"C:\other", None, 12),
        ];
        let params = SessionListParams {
            roots: Some(true),
            ..SessionListParams::default()
        };

        let filtered = filter_list_sessions(sessions, &params, Some("C:/repo/"));

        assert_eq!(
            filtered
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            vec!["root-a"]
        );
    }

    #[test]
    fn session_list_hides_children_by_default() {
        let sessions = vec![
            test_session("root-a", r"C:\repo", None, 10),
            test_session("child-a", r"C:\repo", Some("root-a"), 11),
        ];

        let filtered =
            filter_list_sessions(sessions, &SessionListParams::default(), Some("C:/repo"));

        assert_eq!(
            filtered
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            vec!["root-a"]
        );
    }

    #[test]
    fn session_list_can_include_children_when_requested() {
        let sessions = vec![
            test_session("root-a", r"C:\repo", None, 10),
            test_session("child-a", r"C:\repo", Some("root-a"), 11),
        ];
        let params = SessionListParams {
            include_children: true,
            ..SessionListParams::default()
        };

        let filtered = filter_list_sessions(sessions, &params, Some("C:/repo"));

        assert_eq!(
            filtered
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            vec!["root-a", "child-a"]
        );
    }

    #[test]
    fn workspace_key_normalizes_slashes_and_trailing_separator() {
        assert_eq!(workspace_key(r"C:\repo\"), "C:/repo");
        assert_eq!(workspace_key("C:/"), "C:/");
        assert_eq!(workspace_key("///"), "/");
    }

    #[tokio::test]
    async fn session_status_includes_task_management_display_fields() {
        let directory = std::env::temp_dir()
            .join(format!("tura-session-status-{}", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        let session = session_store().create_session(
            Some(directory),
            None,
            None,
            Some("coding".to_string()),
            false,
            false,
            false,
            None,
            false,
            false,
        );
        session_store()
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "plan_summary": "Status Contract",
                    "task_summary": "Status task",
                    "status": "question"
                })),
            )
            .expect("session task management should update");

        let Json(statuses) = super::session_status().await;
        let status = statuses
            .get(&session.id)
            .expect("status map should include new session");

        assert_eq!(status["task_management"]["status"], "question");
        assert_eq!(status["plan_summary"], "Status Contract");
        assert_eq!(status["session_display_name"], "Status Contract");
    }

    #[tokio::test]
    async fn create_session_accepts_task_management_and_serializes_session_fields() {
        let directory = std::env::temp_dir()
            .join(format!("tura-create-session-plan-{}", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        let Json(session) = super::create_session(
            HeaderMap::new(),
            Query(super::SessionDirectoryParams { directory: None }),
            Some(Json(super::CreateSessionRequest {
                directory: Some(directory.clone()),
                model: None,
                agent: None,
                session_type: Some("chat".to_string()),
                kill_processes_on_start: Some(false),
                validator_enabled: Some(false),
                force_multiple_tasks: Some(false),
                model_variant: None,
                model_acceleration_enabled: Some(false),
                disable_permission_restrictions: Some(false),
                auto_session_name: None,
                task_management: Some(serde_json::json!({
                    "plan_summary": "Create Route Plan",
                    "task_summary": "Create route task"
                })),
            })),
        )
        .await;

        assert_eq!(session.directory.as_deref(), Some(directory.as_str()));
        assert_eq!(session.plan_summary.as_deref(), Some("Create Route Plan"));
        assert_eq!(
            session.session_display_name.as_deref(),
            Some("Create Route Plan")
        );
        assert_eq!(session.task_management["task_summary"], "Create route task");

        let value = serde_json::to_value(&session).expect("session should serialize");
        assert!(value["name"].as_str().is_some_and(|name| !name.is_empty()));
        assert!(value["task_management"].get("status").is_none());
        assert_eq!(value["task_management"]["start_condition"], "user_action");
        assert_eq!(value["plan_summary"], "Create Route Plan");
        assert_eq!(value["session_display_name"], "Create Route Plan");
        assert_eq!(value["auto_session_name"], true);
        let object = value.as_object().expect("session JSON should be an object");
        assert_eq!(object.len(), 21);

        let Json(listed) = super::list_sessions(
            HeaderMap::new(),
            Query(SessionListParams {
                directory: Some(directory.clone()),
                include_children: true,
                ..SessionListParams::default()
            }),
        )
        .await;
        assert!(listed.iter().any(|item| item.id == session.id
            && item.task_management.get("status").is_none()
            && item.task_management["start_condition"] == "user_action"));

        let _ = fs::remove_dir_all(directory);
    }

    #[tokio::test]
    async fn task_management_route_patches_session_and_returns_session_fields() {
        let directory = std::env::temp_dir()
            .join(format!(
                "tura-task-management-route-{}",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .to_string();
        let session = session_store().create_session(
            Some(directory.clone()),
            None,
            None,
            Some("chat".to_string()),
            false,
            false,
            false,
            None,
            false,
            false,
        );

        let Json(updated) = super::update_session_task_management(
            Path(session.id.clone()),
            Json(super::UpdateSessionTaskManagementRequest {
                task_management: serde_json::json!({
                    "plan_summary": "Dedicated Patch Route",
                    "task_summary": "Patch task",
                    "status": "question",
                    "start_at": "2026-05-25T08:30:00Z"
                }),
            }),
        )
        .await;

        assert_eq!(
            updated.plan_summary.as_deref(),
            Some("Dedicated Patch Route")
        );
        assert_eq!(
            updated.session_display_name.as_deref(),
            Some("Dedicated Patch Route")
        );
        assert_eq!(updated.task_management["status"], "question");
        assert_eq!(updated.task_management["start_condition"], "scheduled_task");

        let value = serde_json::to_value(&updated).expect("session should serialize");
        assert_eq!(value["task_management"]["status"], "question");
        assert_eq!(
            value["task_management"]["start_condition"],
            "scheduled_task"
        );
        assert_eq!(value["plan_summary"], "Dedicated Patch Route");
        assert_eq!(value["session_display_name"], "Dedicated Patch Route");
        assert_eq!(value["auto_session_name"], true);
        let object = value.as_object().expect("session JSON should be an object");
        assert_eq!(object.len(), 21);

        let Json(fetched) = super::get_session(Path(session.id)).await;
        assert_eq!(fetched.task_management["status"], "question");
        assert_eq!(fetched.task_management["start_condition"], "scheduled_task");

        let _ = fs::remove_dir_all(directory);
    }

    #[tokio::test]
    async fn agent_tool_callback_updates_auto_session_name_from_last_task_summary() {
        let directory = std::env::temp_dir()
            .join(format!("auto-session-name-{}", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        let session = session_store().create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            false,
            false,
            false,
            None,
            false,
            false,
        );

        let Json(response) = super::send_agent_message(
            Path(session.id.clone()),
            Json(super::SendAgentMessageRequest {
                reply_message: String::new(),
                new_learning: String::new(),
                step_summary: None,
                media: vec![],
                runtime_id: Some("runtime-1".to_string()),
                tool_call: Some(super::SendAgentToolCall {
                    tool_name: "command_run".to_string(),
                    call_id: "call-1".to_string(),
                    state: serde_json::json!({
                        "status": "completed",
                        "metadata": {
                            "output": {
                                "results": [
                                    { "output": { "task_status": { "task_summary": "First summary" } } },
                                    { "output": { "status": { "task_summary": "Last summary" } } }
                                ]
                            }
                        }
                    }),
                    metadata: None,
                }),
            }),
        )
        .await;

        assert!(response.ok);
        let Json(updated) = super::get_session(Path(session.id)).await;
        assert_eq!(updated.name.as_deref(), Some("Last summary"));
        assert_eq!(
            updated.session_display_name.as_deref(),
            Some("Last summary")
        );

        let _ = fs::remove_dir_all(directory);
    }

    #[tokio::test]
    async fn agent_tool_callback_keeps_manual_session_name_when_auto_disabled() {
        let directory = std::env::temp_dir()
            .join(format!("manual-session-title-{}", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        let session = session_store().create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            false,
            false,
            false,
            None,
            false,
            false,
        );
        session_store()
            .update_session_auto_session_name(&session.id, false)
            .expect("session auto mode should update");
        session_store()
            .update_session(
                &session.id,
                Some("Manual title".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("session title should update");

        let Json(response) = super::send_agent_message(
            Path(session.id.clone()),
            Json(super::SendAgentMessageRequest {
                reply_message: String::new(),
                new_learning: String::new(),
                step_summary: None,
                media: vec![],
                runtime_id: Some("runtime-1".to_string()),
                tool_call: Some(super::SendAgentToolCall {
                    tool_name: "command_run".to_string(),
                    call_id: "call-1".to_string(),
                    state: serde_json::json!({
                        "status": "completed",
                        "metadata": {
                            "output": {
                                "results": [{
                                    "output": {
                                        "status": { "task_summary": "Generated summary" }
                                    }
                                }]
                            }
                        }
                    }),
                    metadata: None,
                }),
            }),
        )
        .await;

        assert!(response.ok);
        let Json(updated) = super::get_session(Path(session.id)).await;
        assert_eq!(updated.name.as_deref(), Some("Manual title"));
        assert!(!updated.auto_session_name);

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn multiple_tasks_tool_call_derives_todos_from_steps_and_output() {
        let tool_call = SendAgentToolCall {
            tool_name: "multiple_tasks".to_string(),
            call_id: "call-1".to_string(),
            state: serde_json::json!({
                "status": "completed",
                "input": {
                    "steps": [
                        { "step_goal": "Inspect wiring", "task_instruction": "Read the code" },
                        { "task_instruction": "Patch the flow" }
                    ]
                },
                "output": {
                    "results": [{
                        "steps": [
                            { "index": 1, "ok": true },
                            { "index": 2, "ok": false }
                        ]
                    }]
                }
            }),
            metadata: None,
        };

        let todos = multiple_tasks_todos(&tool_call).expect("multiple_tasks should produce todos");
        assert_eq!(todos[0]["content"], "Inspect wiring");
        assert_eq!(todos[0]["status"], "completed");
        assert_eq!(todos[1]["content"], "Patch the flow");
        assert_eq!(todos[1]["status"], "cancelled");
    }

    #[test]
    fn multiple_tasks_running_call_marks_next_step_in_progress() {
        let tool_call = SendAgentToolCall {
            tool_name: "multiple_tasks".to_string(),
            call_id: "call-1".to_string(),
            state: serde_json::json!({
                "status": "running",
                "input": {
                    "steps": [
                        { "step_goal": "Plan" },
                        { "step_goal": "Execute" }
                    ]
                }
            }),
            metadata: None,
        };

        let todos = multiple_tasks_todos(&tool_call).expect("multiple_tasks should produce todos");
        assert_eq!(todos[0]["status"], "in_progress");
        assert_eq!(todos[1]["status"], "pending");
    }

    #[test]
    fn multiple_tasks_running_call_marks_same_step_group_in_progress() {
        let tool_call = SendAgentToolCall {
            tool_name: "multiple_tasks".to_string(),
            call_id: "call-1".to_string(),
            state: serde_json::json!({
                "status": "running",
                "input": {
                    "steps": [
                        { "step": 2, "step_goal": "Snake game" },
                        { "step": 2, "step_goal": "Tetris game" },
                        { "step": 3, "step_goal": "Verify games" }
                    ]
                }
            }),
            metadata: None,
        };

        let todos = multiple_tasks_todos(&tool_call).expect("multiple_tasks should produce todos");
        assert_eq!(todos[0]["status"], "in_progress");
        assert_eq!(todos[1]["status"], "in_progress");
        assert_eq!(todos[2]["status"], "pending");
    }

    #[test]
    fn frontend_safe_value_strips_tool_internal_fields_recursively() {
        let value = frontend_safe_value(Some(serde_json::json!({
            "input": {
                "reply_message": "done",
                "new_learning": "private",
                "nested": [{ "runtime_id": "runtime-1", "ok": true }]
            },
            "runtime_id": "runtime-2"
        })))
        .expect("value should remain present");

        let serialized = serde_json::to_string(&value).expect("value should serialize");
        assert!(!serialized.contains("new_learning"));
        assert!(!serialized.contains("runtime_id"));
        assert!(serialized.contains("reply_message"));
    }

    #[test]
    fn runtime_tool_part_keeps_exact_input_output_payloads() {
        let message = crate::session::store::Message {
            id: "message-1".to_string(),
            session_id: "session-1".to_string(),
            role: crate::session::store::MessageRole::Assistant,
            parent_id: None,
            parts: vec![crate::session::store::MessagePart {
                id: "part-1".to_string(),
                part_type: "tool".to_string(),
                content: None,
                text: None,
                metadata: None,
                call_id: Some("runtime-1".to_string()),
                tool: Some("runtime".to_string()),
                state: Some(serde_json::json!({
                    "status": "completed",
                    "input": {
                        "messages": [{ "role": "user", "content": "ACTUAL_CONTEXT_MARKER" }],
                        "runtime_id": "request-runtime-id"
                    },
                    "output": {
                        "text": "FULL_PROVIDER_OUTPUT_MARKER",
                        "runtime_id": "response-runtime-id"
                    }
                })),
            }],
            created_at: 1,
            updated_at: 2,
        };

        let value = serde_json::to_value(api_message_from_store(message))
            .expect("message should serialize");

        assert_eq!(
            value["parts"][0]["state"]["input"]["messages"][0]["content"],
            "ACTUAL_CONTEXT_MARKER"
        );
        assert_eq!(
            value["parts"][0]["state"]["input"]["runtime_id"],
            "request-runtime-id"
        );
        assert_eq!(
            value["parts"][0]["state"]["output"]["text"],
            "FULL_PROVIDER_OUTPUT_MARKER"
        );
        assert_eq!(
            value["parts"][0]["state"]["output"]["runtime_id"],
            "response-runtime-id"
        );
    }

    #[test]
    fn frontend_safe_reply_message_extracts_reply_from_raw_tool_payload() {
        let text = serde_json::json!({
            "error": null,
            "input": {
                "reply_message": "final answer",
                "new_learning": "",
                "runtime_id": "runtime-1"
            }
        })
        .to_string();

        assert_eq!(frontend_safe_reply_message(&text), "final answer");
    }

    #[test]
    fn frontend_safe_reply_message_hides_raw_tool_argument_payload() {
        let text = serde_json::json!({
            "requests": [{
                "path": "services/sd-text-to-image/main.py",
                "start_line": 1,
                "end_line": 250
            }],
            "step_summary": "Read the Stable Diffusion image service main.py to find the port it runs on."
        })
        .to_string();

        assert_eq!(frontend_safe_reply_message(&text), "");
    }

    #[test]
    fn agent_message_metadata_keeps_step_summary_for_frontend() {
        let metadata = agent_message_metadata(&SendAgentMessageRequest {
            reply_message: "done".to_string(),
            new_learning: String::new(),
            step_summary: Some("send final response".to_string()),
            media: vec![],
            runtime_id: Some("runtime-1".to_string()),
            tool_call: None,
        })
        .expect("feedback metadata should be present");

        assert_eq!(metadata["step_summary"], "send final response");
        let sanitized = frontend_safe_value(Some(metadata))
            .expect("metadata should survive frontend sanitizing");
        assert_eq!(sanitized["step_summary"], "send final response");
    }

    #[test]
    fn agent_message_content_renders_media_as_rich_tokens() {
        let content = agent_message_content(&SendAgentMessageRequest {
            reply_message: "screens".to_string(),
            new_learning: String::new(),
            step_summary: None,
            media: vec![SendAgentMedia {
                path: r"C:\Users\liuliu\Documents\tura\shot.png".to_string(),
                media_type: Some("image/png".to_string()),
            }],
            runtime_id: Some("runtime-1".to_string()),
            tool_call: None,
        });

        assert_eq!(
            content,
            "screens\n\n[MEDIA:C:\\Users\\liuliu\\Documents\\tura\\shot.png:MEDIA]\n"
        );
    }
}
