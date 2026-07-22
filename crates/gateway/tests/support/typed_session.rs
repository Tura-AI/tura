use anyhow::{bail, Context, Result};
use lifecycle::{SessionCommand, SessionManagement, TaskPlan};
use session_log_contract::{
    CreateSessionRequest, GetSessionRequest, PersistSessionDeltaRequest, ReadContextSliceRequest,
    SessionContextRecord, SessionDeltaEntry, SessionLogCommand, SessionLogResponse,
    SessionRecordProjection,
};

pub fn create_request(
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
    task_plan: TaskPlan,
) -> CreateSessionRequest {
    CreateSessionRequest {
        command_id: format!("create:{session_id}"),
        session_id: session_id.to_string(),
        creation_command: SessionCommand::CreateSession { task_plan },
        copy_context: false,
        workspace: workspace.to_string(),
        session_directory: workspace.to_string(),
        name: name.to_string(),
        created_at,
        model: None,
        agent: None,
        session_type: "coding".to_string(),
        kill_processes_on_start: false,
        validator_enabled: false,
        force_planning: false,
        model_variant: None,
        model_acceleration_enabled: false,
        disable_permission_restrictions: false,
        use_last_tool_call_response: false,
        auto_session_name: false,
        initial_task_plan_patch: None,
    }
}

pub fn create_via_service(
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
    task_plan: TaskPlan,
) -> Result<()> {
    match session_log_contract::client::call_service(&SessionLogCommand::CreateSession(
        create_request(session_id, workspace, name, created_at, task_plan),
    ))? {
        SessionLogResponse::SessionCommandApplied { .. } => Ok(()),
        SessionLogResponse::Error { error } => bail!("create session failed: {error}"),
        other => bail!("unexpected create session response: {other:?}"),
    }
}

pub fn persist_messages_via_service(
    session_id: &str,
    messages: Vec<serde_json::Value>,
) -> Result<u64> {
    let management = management_via_service(session_id)?;
    let context = context_via_service(session_id)?;
    let entries = messages
        .into_iter()
        .enumerate()
        .map(|(offset, record)| {
            message_entry(context.next_sequence + offset as u64, session_id, record)
        })
        .collect::<Result<Vec<_>>>()?;
    persist_entries_via_service(session_id, management, context, entries)
}

fn context_via_service(session_id: &str) -> Result<session_log_contract::ContextSlice> {
    match session_log_contract::client::call_service(&SessionLogCommand::ReadContextSlice(
        ReadContextSliceRequest {
            session_id: session_id.to_string(),
            max_estimated_tokens: u64::MAX,
        },
    ))? {
        SessionLogResponse::ContextSlice { context } => Ok(context),
        SessionLogResponse::Error { error } => bail!("read context failed: {error}"),
        other => bail!("unexpected read context response: {other:?}"),
    }
}

fn management_via_service(session_id: &str) -> Result<SessionManagement> {
    match session_log_contract::client::call_service(&SessionLogCommand::GetSession(
        GetSessionRequest {
            session_id: session_id.to_string(),
        },
    ))? {
        SessionLogResponse::Session {
            session: Some(session),
        } => Ok(session.management),
        SessionLogResponse::Session { session: None } => bail!("session {session_id} not found"),
        SessionLogResponse::Error { error } => bail!("get session failed: {error}"),
        other => bail!("unexpected get session response: {other:?}"),
    }
}

fn message_entry(
    sequence: u64,
    session_id: &str,
    mut record: serde_json::Value,
) -> Result<SessionDeltaEntry> {
    let message_id = record
        .get("id")
        .and_then(serde_json::Value::as_str)
        .context("typed test message is missing string id")?
        .to_string();
    let role = record
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("runtime")
        .to_string();
    let created_at = record
        .get("created_at")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    let updated_at = record
        .get("updated_at")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(created_at);
    let record_session_id = record
        .as_object_mut()
        .context("typed test message is not an object")?
        .entry("session_id")
        .or_insert_with(|| serde_json::Value::String(session_id.to_string()));
    if record_session_id.as_str() != Some(session_id) {
        bail!("typed test message session_id does not match {session_id}");
    }
    Ok(SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: serde_json::json!({ "id": message_id, "role": role }).to_string(),
        },
        projection: Some(SessionRecordProjection {
            session_id: session_id.to_string(),
            message_id,
            role,
            created_at,
            updated_at,
            record,
        }),
    })
}

fn persist_entries_via_service(
    session_id: &str,
    mut management: SessionManagement,
    context: session_log_contract::ContextSlice,
    entries: Vec<SessionDeltaEntry>,
) -> Result<u64> {
    let previous_management = (context.next_management_sequence > 0).then_some(management.clone());
    management.session_log.clear();
    management.session_log_retention.omitted_entries = 0;
    match session_log_contract::client::call_service(&SessionLogCommand::PersistSessionDelta(
        Box::new(PersistSessionDeltaRequest {
            session_id: session_id.to_string(),
            management_sequence: context.next_management_sequence,
            management_delta: SessionManagement::persistence_delta(
                previous_management.as_ref(),
                &management,
            ),
            retained_from_sequence: 0,
            entries,
        }),
    ))? {
        SessionLogResponse::SessionDeltaPersisted { next_sequence, .. } => Ok(next_sequence),
        SessionLogResponse::Error { error } => bail!("persist session delta failed: {error}"),
        other => bail!("unexpected persist session delta response: {other:?}"),
    }
}
