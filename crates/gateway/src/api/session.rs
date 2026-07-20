//! Session API handlers

use crate::api::product::current_user_snapshot;
use crate::contracts::*;
use crate::mock::global_store;
use crate::router_client::RouterClient;
use crate::session::config::{load_config, merge_config, TuraSessionConfig};
use crate::session::{session_store, MessageRole as SessionMessageRole};
use crate::session_db_client::SessionDbClient;
use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::time::Duration;

use lifecycle::{SessionAggregate, SessionCommand, SessionEvent, StartCondition};

// ============================================================================
// Session List & Create
// ============================================================================

pub async fn list_sessions(
    headers: HeaderMap,
    Query(params): Query<SessionListParams>,
) -> Json<Vec<Session>> {
    Json(list_sessions_value(params, encoded_header(&headers, "x-opencode-directory")).await)
}

pub async fn list_sessions_value(
    params: SessionListParams,
    header_directory: Option<String>,
) -> Vec<Session> {
    let directory = params
        .directory
        .clone()
        .or(params.workspace.clone())
        .or(header_directory)
        .or_else(|| global_store().get_current_directory());

    session_store().hydrate_directory(directory.clone());

    let listed = filter_list_sessions(
        session_store().list_sessions(),
        &params,
        directory.as_deref(),
    );
    refresh_busy_session_liveness(&listed).await;

    let mut sessions = filter_list_sessions(
        session_store().list_sessions(),
        &params,
        directory.as_deref(),
    );
    sessions.sort_by(|a, b| {
        session_sort_time(b)
            .cmp(&session_sort_time(a))
            .then_with(|| a.id.cmp(&b.id))
    });

    if let Some(limit) = params.limit.filter(|limit| *limit > 0) {
        sessions.truncate(limit);
    }

    fn session_sort_time(session: &Session) -> i64 {
        session.last_user_message_at.unwrap_or(0)
    }

    sessions
}

async fn refresh_busy_session_liveness(sessions: &[Session]) {
    let busy_session_ids = sessions
        .iter()
        .filter(|session| session.status == SessionStatus::Busy)
        .map(|session| session.id.clone())
        .collect::<Vec<_>>();
    if busy_session_ids.is_empty() {
        return;
    }

    let inactive_session_ids = match RouterClient::global().probe_sessions(&busy_session_ids) {
        Ok(payload) => inactive_sessions_from_probe(&busy_session_ids, &payload),
        Err(error) => {
            tracing::warn!(
                error = %error,
                sessions = ?busy_session_ids,
                "runtime liveness probe failed; marking busy sessions interrupted"
            );
            busy_session_ids
        }
    };

    for session_id in inactive_session_ids {
        mark_session_interrupted_from_gateway_probe(&session_id).await;
    }
}

fn inactive_sessions_from_probe(
    expected_session_ids: &[String],
    payload: &serde_json::Value,
) -> Vec<String> {
    let active = payload
        .get("sessions")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let session_id = entry
                .get("session_id")
                .and_then(serde_json::Value::as_str)?;
            let active = entry
                .get("status")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| matches!(value, "active" | "queued" | "running"))
                || entry
                    .get("active_turn")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
                || entry
                    .get("worker_alive")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
            active.then_some(session_id.to_string())
        })
        .collect::<std::collections::HashSet<_>>();

    expected_session_ids
        .iter()
        .filter(|session_id| !active.contains(*session_id))
        .cloned()
        .collect()
}

async fn mark_session_interrupted_from_gateway_probe(session_id: &str) {
    if let Err(error) = session_store()
        .execute_canonical_session_command(session_id, SessionCommand::InterruptSession)
    {
        tracing::warn!(
            session_id,
            error,
            "failed to apply runtime liveness interruption"
        );
        return;
    }
    let Some(session) = session_store().get_session(session_id) else {
        return;
    };
    session_store().finish_todos(session_id, false);
    session_store().push_event(GlobalEvent::SessionUpdated {
        properties: SessionUpdatedProperties {
            session_id: session_id.to_string(),
            info: session,
        },
    });
    session_store().push_current_session_status_event(session_id);
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
    Json(get_session_value(session_id))
}

pub fn get_session_value(session_id: String) -> Session {
    session_store()
        .get_session(&session_id)
        .unwrap_or_else(|| Session {
            id: session_id,
            name: None,
            parent_id: None,
            created_at: 0,
            updated_at: 0,
            last_user_message_at: None,
            task_start_at: None,
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
            context_tokens: SessionContextTokens::default(),
            usage: Default::default(),
            plan_summary: None,
            session_display_name: None,
        })
}

pub async fn create_session(
    headers: HeaderMap,
    Query(params): Query<SessionDirectoryParams>,
    payload: Option<Json<CreateSessionRequest>>,
) -> impl IntoResponse {
    let payload = payload.map(|Json(payload)| payload).unwrap_or_default();
    match create_session_value(
        params,
        payload,
        encoded_header(&headers, "x-opencode-directory"),
    )
    .await
    {
        Ok(session) => Json(session).into_response(),
        Err(error) => session_mutation_error(error),
    }
}

pub async fn create_session_value(
    params: SessionDirectoryParams,
    payload: CreateSessionRequest,
    header_directory: Option<String>,
) -> Result<Session, String> {
    let directory = payload
        .directory
        .clone()
        .or(params.directory)
        .or(header_directory)
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
    let model_variant = payload
        .model_variant
        .clone()
        .or(persisted_config.model_variant);
    let model_acceleration_enabled = payload
        .model_acceleration_enabled
        .or(persisted_config.model_acceleration_enabled)
        .unwrap_or(false);
    let disable_permission_restrictions = payload.disable_permission_restrictions.unwrap_or(false);
    let auto_session_name = payload.auto_session_name.unwrap_or(true);
    if kill_processes_on_start {
        tracing::info!(
            "kill_processes_on_start requested; workspace-wide process scanning is disabled, router-owned workers are stopped by session id"
        );
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
    let mut info = session_store().build_session_info(
        directory,
        payload.model.clone().or(persisted_config.model),
        requested_agent,
        Some(requested_session_type),
        kill_processes_on_start,
        validator_enabled,
        force_planning,
        model_variant,
        model_acceleration_enabled,
        disable_permission_restrictions,
    );
    info.management.auto_session_name = auto_session_name;
    if let Some(task_management) = payload.task_management {
        session_store().apply_initial_task_management(&mut info, task_management)?;
    }
    let task_plan = info.management.task_plan.clone();
    let session = session_store()
        .create_canonical_session(info, SessionCommand::CreateSession { task_plan })?;
    session_store().push_event(GlobalEvent::SessionCreated {
        properties: SessionCreatedProperties {
            session_id: session.id.clone(),
            info: session.clone(),
        },
    });
    Ok(session)
}

fn session_mutation_error(error: String) -> axum::response::Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({
            "ok": false,
            "error": "session_service_mutation_failed",
            "message": error,
        })),
    )
        .into_response()
}

pub async fn get_session_config(
    headers: HeaderMap,
    Query(params): Query<SessionDirectoryParams>,
) -> Json<TuraSessionConfig> {
    let directory = params
        .directory
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .or_else(|| global_store().get_current_directory());
    Json(get_session_config_value(directory))
}

pub fn get_session_config_value(directory: Option<String>) -> TuraSessionConfig {
    directory.as_deref().map(load_config).unwrap_or_default()
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
    Json(patch_session_config_value(directory, payload))
}

pub fn patch_session_config_value(
    directory: Option<String>,
    payload: TuraSessionConfig,
) -> TuraSessionConfig {
    let Some(directory) = directory else {
        return TuraSessionConfig::default();
    };
    match merge_config(directory, payload) {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(error = %err, "failed to patch session config");
            TuraSessionConfig::default()
        }
    }
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
    Json(delete_session_value(session_id).await)
}

pub async fn delete_session_value(session_id: String) -> bool {
    let info = session_store().get_session(&session_id);
    let abort = abort_session_value(&session_id);
    tracing::info!(
        session_id,
        aborted_sessions = ?abort.sessions,
        "aborted session scope before delete"
    );
    let delete_result = session_store()
        .execute_canonical_session_command(&session_id, SessionCommand::DeleteSession);
    if let Err(error) = &delete_result {
        tracing::warn!(session_id, error, "failed to delete canonical session");
    }
    if delete_result.is_ok() {
        if let Some(info) = info {
            session_store().push_event(GlobalEvent::SessionDeleted {
                properties: SessionDeletedProperties { session_id, info },
            });
        }
    }
    delete_result.is_ok()
}

pub async fn update_session(
    Path(session_id): Path<String>,
    Json(payload): Json<UpdateSessionRequest>,
) -> impl IntoResponse {
    match update_session_value(session_id, payload) {
        Ok(session) => Json(session).into_response(),
        Err(error) => session_mutation_error(error),
    }
}

pub fn update_session_value(
    session_id: String,
    mut payload: UpdateSessionRequest,
) -> Result<Session, String> {
    if let Some(task_management) = payload.task_management.take() {
        session_store().execute_task_management_patch(&session_id, task_management)?;
    }
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
            None,
        )
        .ok_or_else(|| format!("session {session_id} not found"))?;

    session_store().push_event(GlobalEvent::SessionUpdated {
        properties: SessionUpdatedProperties {
            session_id: session.id.clone(),
            info: session.clone(),
        },
    });

    Ok(session)
}

pub async fn update_session_task_management(
    Path(session_id): Path<String>,
    Json(payload): Json<UpdateSessionTaskManagementRequest>,
) -> impl IntoResponse {
    match update_session_task_management_value(session_id, payload) {
        Ok(session) => Json(session).into_response(),
        Err(error) => session_mutation_error(error),
    }
}

pub fn update_session_task_management_value(
    session_id: String,
    payload: UpdateSessionTaskManagementRequest,
) -> Result<Session, String> {
    let session =
        session_store().execute_task_management_patch(&session_id, payload.task_management)?;
    session_store().push_event(GlobalEvent::SessionUpdated {
        properties: SessionUpdatedProperties {
            session_id: session.id.clone(),
            info: session.clone(),
        },
    });
    Ok(session)
}

pub async fn abort_session(Path(session_id): Path<String>) -> Json<AbortResponse> {
    Json(abort_session_value(&session_id))
}

pub fn abort_session_value(session_id: &str) -> AbortResponse {
    let aborted_sessions = session_store().cancellation_scope_session_ids(session_id);
    let mut cleanups = Vec::new();
    let router = RouterClient::global();
    for id in &aborted_sessions {
        let lifecycle_error = session_store()
            .execute_canonical_session_command(id, SessionCommand::CancelSession)
            .err();
        cleanups.push(match (router.kill_session_workers(id), lifecycle_error) {
            (Ok(payload), None) => AbortCleanup {
                session_id: id.clone(),
                status: payload
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                stopped_worker: payload
                    .get("stopped_worker")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                error: None,
            },
            (Ok(payload), Some(error)) => AbortCleanup {
                session_id: id.clone(),
                status: payload
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                stopped_worker: payload
                    .get("stopped_worker")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                error: Some(error),
            },
            (Err(error), lifecycle_error) => AbortCleanup {
                session_id: id.clone(),
                status: "error".to_string(),
                stopped_worker: false,
                error: Some(match lifecycle_error {
                    Some(lifecycle_error) => {
                        format!("{error}; session cancellation failed: {lifecycle_error}")
                    }
                    None => error.to_string(),
                }),
            },
        });
    }

    AbortResponse {
        aborted: cleanups.iter().all(|cleanup| cleanup.error.is_none()),
        sessions: aborted_sessions,
        cleanup: cleanups.first().cloned(),
        cleanups,
    }
}

pub async fn fork_session(
    Path(session_id): Path<String>,
    Json(payload): Json<ForkSessionRequest>,
) -> impl IntoResponse {
    match fork_session_value(session_id, payload).await {
        Ok(session) => Json(session).into_response(),
        Err(error) => session_mutation_error(error),
    }
}

pub async fn fork_session_value(
    session_id: String,
    payload: ForkSessionRequest,
) -> Result<Session, String> {
    let original = session_store().get_session(&session_id);
    let info = session_store().build_session_info(
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
    let new_session = session_store().create_canonical_session(
        info,
        SessionCommand::ForkSession {
            parent_id: session_id.clone(),
        },
    )?;
    if payload.copy_context.unwrap_or(true) {
        let _ = session_store().copy_session_context(&session_id, &new_session.id);
    }
    let new_session = session_store()
        .get_session(&new_session.id)
        .unwrap_or(new_session);
    match session_store().session_payload_request(&new_session.id) {
        Ok(request) => {
            if let Err(error) = SessionDbClient::discover()
                .and_then(|client| client.persist_session_payload(request))
            {
                tracing::warn!(
                    session_id = %new_session.id,
                    parent_session_id = %session_id,
                    error = %error,
                    "failed to persist forked session payload"
                );
            }
        }
        Err(error) => tracing::warn!(
            session_id = %new_session.id,
            parent_session_id = %session_id,
            error,
            "failed to build forked session payload"
        ),
    }
    session_store().push_event(GlobalEvent::SessionCreated {
        properties: SessionCreatedProperties {
            session_id: new_session.id.clone(),
            info: new_session.clone(),
        },
    });
    Ok(new_session)
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
                    "context_tokens": s.context_tokens,
                    "usage": s.usage,
                    "plan_summary": s.plan_summary,
                    "session_display_name": s.session_display_name,
                }),
            )
        })
        .collect();
    Json(statuses)
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
        url: format!("https://share.example.com/session/{session_id}"),
    })
}

// ============================================================================
// Session Children
// ============================================================================

pub async fn session_children(Path(session_id): Path<String>) -> Json<Vec<Session>> {
    Json(session_store().list_child_sessions(&session_id))
}

pub async fn session_user_commands(Path(session_id): Path<String>) -> impl IntoResponse {
    let root_session_id = session_store().root_session_id(&session_id);
    let result = session_store().execute_canonical_session_command(
        &root_session_id,
        SessionCommand::ConsumeQueuedUserInputs,
    );
    match result {
        Ok(result) => match result.event {
            SessionEvent::QueuedUserInputsConsumed { inputs } => Json(serde_json::json!({
                "session_id": session_id,
                "commands": inputs,
            }))
            .into_response(),
            event => session_mutation_error(format!(
                "session service returned unexpected queued-input event: {event:?}"
            )),
        },
        Err(error) => session_mutation_error(error),
    }
}

pub(crate) fn append_user_command_for_runtime(
    session_id: &str,
    command: String,
) -> Result<Vec<String>, String> {
    let root_session_id = session_store().root_session_id(session_id);
    let result = session_store().execute_canonical_session_command(
        &root_session_id,
        SessionCommand::QueueUserInputWhileBusy { input: command },
    )?;
    Ok(result.projection.pending_user_inputs)
}

pub async fn append_session_user_command(
    Path(session_id): Path<String>,
    Json(payload): Json<AppendUserCommandRequest>,
) -> impl IntoResponse {
    match append_user_command_for_runtime(&session_id, payload.command) {
        Ok(commands) => Json(serde_json::json!({
            "ok": true,
            "session_id": session_id,
            "commands": commands,
        }))
        .into_response(),
        Err(error) => session_mutation_error(error),
    }
}

pub async fn register_child_session(
    Path(session_id): Path<String>,
    Json(payload): Json<RegisterChildSessionRequest>,
) -> impl IntoResponse {
    let session = session_store().register_canonical_child_session(
        &session_id,
        &payload.child_session_id,
        Some(payload.directory.clone()),
        Some(payload.name.clone()),
        Some(payload.task_instruction.clone()),
    );
    let session = match session {
        Ok(session) => session,
        Err(error) => return session_mutation_error(error),
    };
    session_store().push_event(GlobalEvent::SessionCreated {
        properties: SessionCreatedProperties {
            session_id: session.id.clone(),
            info: session.clone(),
        },
    });
    Json(session).into_response()
}

pub async fn update_session_status_for_runtime(
    Path(session_id): Path<String>,
    Json(payload): Json<RuntimeSessionStatusRequest>,
) -> impl IntoResponse {
    if session_store().get_session(&session_id).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "ok": false,
                "error": "session_not_found",
                "session_id": session_id,
            })),
        )
            .into_response();
    }
    let (command, api_status) = match payload.status.as_str() {
        "idle" => (SessionCommand::RuntimeCompleted, SessionStatus::Idle),
        "busy" => (SessionCommand::RuntimeStarted, SessionStatus::Busy),
        "error" => (SessionCommand::RuntimeFailed, SessionStatus::Error),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "ok": false,
                    "error": "invalid_session_status",
                    "allowed": ["idle", "busy", "error"],
                })),
            )
                .into_response();
        }
    };
    if let Some(projection) = session_store().session_lifecycle_projection(&session_id) {
        let aggregate = SessionAggregate {
            session_id: projection.session_id,
            state: projection.state,
            parent_id: projection.parent_id,
            task_plan: projection.task_plan,
            pending_user_inputs: projection.pending_user_inputs,
            cancelled: projection.cancelled,
        };
        if aggregate.decide(command.clone()).is_err() {
            let session = session_store().get_session(&session_id);
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "ok": false,
                    "error": "session_status_transition_rejected",
                    "requested": payload.status,
                    "actual": session.as_ref().map(|session| session.status.clone()),
                })),
            )
                .into_response();
        }
    }
    if let Err(error) =
        session_store().execute_canonical_session_command_with_status_event(&session_id, command)
    {
        return session_mutation_error(error);
    }
    let session = session_store().get_session(&session_id);
    if session
        .as_ref()
        .is_none_or(|session| session.status != api_status)
    {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "ok": false,
                "error": "session_status_transition_rejected",
                "requested": payload.status,
                "actual": session.as_ref().map(|session| session.status.clone()),
            })),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        Json(
            serde_json::to_value(SessionStatusResponse {
                session_id,
                status: api_status,
                task_management: session
                    .as_ref()
                    .map(|session| session.task_management.clone())
                    .unwrap_or_else(|| serde_json::json!({})),
                context_tokens: session
                    .as_ref()
                    .map(|session| session.context_tokens)
                    .unwrap_or_default(),
                usage: session
                    .as_ref()
                    .map(|session| session.usage.clone())
                    .unwrap_or_default(),
                plan_summary: session
                    .as_ref()
                    .and_then(|session| session.plan_summary.clone()),
                session_display_name: session.and_then(|session| session.session_display_name),
            })
            .unwrap_or_else(|_| {
                serde_json::json!({
                    "ok": false,
                    "error": "session_status_response_encode_failed",
                })
            }),
        ),
    )
        .into_response()
}

// ============================================================================
// Message Operations
// ============================================================================

#[path = "session_messages.rs"]
mod session_messages;
pub use crate::contracts::{
    MessageListParams, SendAgentMedia, SendAgentMessageRequest, SendAgentMessageResponse,
    SendAgentToolCall, SessionCommandRequest, SessionCommandResponse, StreamAgentTextRequest,
};
#[cfg(test)]
use session_messages::{agent_message_content, agent_message_metadata, planning_todos};
pub use session_messages::{
    get_message, get_message_part, get_todos, list_messages, list_messages_value,
    send_agent_message, send_agent_message_payload, send_message, session_command,
    stream_agent_message, stream_agent_message_payload, update_todos,
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
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create change target directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        fs::write(&path, target_content.unwrap_or_default()).map_err(|error| {
            format!(
                "failed to write change target file {}: {error}",
                path.display()
            )
        })?;
    } else if path.exists() {
        fs::remove_file(&path).map_err(|error| {
            format!(
                "failed to remove change target file {}: {error}",
                path.display()
            )
        })?;
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
pub use crate::contracts::SummaryResponse;
pub use session_summary::summarize_session;

// ============================================================================
// Session Shell
// ============================================================================

#[path = "session_shell.rs"]
mod session_shell;
pub use crate::contracts::{ShellRequest, ShellResponse};
pub use session_shell::session_shell;
use session_shell::{run_session_shell_command, truncate_summary_text};

// ============================================================================
// Async Prompt
// ============================================================================

#[path = "session_prompt.rs"]
mod session_prompt;
#[cfg(test)]
use session_prompt::{
    config_model_override, first_prompt_part_id, prompt_command_run_shell, prompt_message_id,
    prompt_model_acceleration, prompt_model_variant, prompt_text,
};
use session_prompt::{final_agent_message, frontend_safe_reply_message, run_mano_for_prompt};
pub use session_prompt::{prompt_async, prompt_async_value, start_task_scheduler};
#[cfg(any(feature = "business-tests", feature = "os-tests"))]
pub use session_prompt::{
    run_due_task_scheduler_tick_for_business_test,
    run_due_task_scheduler_tick_for_store_business_test,
};
#[path = "session_format.rs"]
mod session_format;
#[cfg(test)]
use crate::session::store::frontend_safe_value;
pub(crate) use session_format::api_message_from_store;
use session_format::{message_with_parts_from_store, part_json};

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
