#![allow(dead_code)]

use anyhow::{bail, Context, Result};
use lifecycle::{SessionCommand, SessionInput, SessionManagement, TaskPlan};
use session_log_contract::{
    CreateSessionRequest, ExecuteSessionCommandRequest, GetSessionRequest,
    PersistSessionDeltaRequest, ReadContextSliceRequest, SessionContextRecord, SessionDeltaEntry,
    SessionLogCommand, SessionLogResponse, SessionRecordProjection,
};

pub fn create_request(
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
    creation_command: SessionCommand,
) -> CreateSessionRequest {
    CreateSessionRequest {
        command_id: format!("create:{session_id}"),
        session_id: session_id.to_string(),
        creation_command,
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
    }
}

pub fn root_create_request(
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
) -> CreateSessionRequest {
    create_request(
        session_id,
        workspace,
        name,
        created_at,
        SessionCommand::CreateSession {
            task_plan: TaskPlan::default(),
        },
    )
}

pub fn message_entry(sequence: u64, record: serde_json::Value) -> Result<SessionDeltaEntry> {
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
    let session_id = record
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .context("typed test message is missing string session_id")?
        .to_string();
    Ok(SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: serde_json::json!({ "id": message_id, "role": role }).to_string(),
        },
        projection: Some(SessionRecordProjection {
            session_id,
            message_id,
            role,
            created_at,
            updated_at,
            record,
        }),
    })
}

pub fn entries_from_messages(
    start_sequence: u64,
    messages: Vec<serde_json::Value>,
) -> Result<Vec<SessionDeltaEntry>> {
    messages
        .into_iter()
        .enumerate()
        .map(|(offset, message)| message_entry(start_sequence + offset as u64, message))
        .collect()
}

pub fn enqueue_create(request: CreateSessionRequest) -> Result<()> {
    session_log_contract::client::enqueue_command(&SessionLogCommand::CreateSession(request))?;
    Ok(())
}

pub fn enqueue_execute(session_id: &str, session_command: SessionCommand) -> Result<()> {
    session_log_contract::client::enqueue_command(&SessionLogCommand::ExecuteSessionCommand(
        ExecuteSessionCommandRequest {
            command_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            session_command,
            message_projection: None,
        },
    ))?;
    Ok(())
}

pub fn enqueue_delta(
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
    entries: Vec<SessionDeltaEntry>,
) -> Result<()> {
    let persisted = if session_log_contract::client::service_is_running() {
        Some((
            management_via_service(session_id)?,
            context_via_service(session_id)?,
        ))
    } else {
        None
    };
    let (management_sequence, previous_management, management) = match persisted {
        Some((management, context)) => (
            context.next_management_sequence,
            Some(management.clone()),
            management,
        ),
        None => (
            0,
            None,
            initial_management(session_id, workspace, name, created_at),
        ),
    };
    enqueue_delta_from_management(
        session_id,
        management_sequence,
        previous_management.as_ref(),
        &management,
        entries,
    )
}

pub fn enqueue_delta_from_management(
    session_id: &str,
    management_sequence: u64,
    previous_management: Option<&SessionManagement>,
    management: &SessionManagement,
    entries: Vec<SessionDeltaEntry>,
) -> Result<()> {
    session_log_contract::client::enqueue_command(&SessionLogCommand::PersistSessionDelta(
        Box::new(delta_request(
            session_id,
            management_sequence,
            previous_management,
            management,
            entries,
        )),
    ))?;
    Ok(())
}

pub fn create_via_service(request: CreateSessionRequest) -> Result<()> {
    match session_log_contract::client::call_service(&SessionLogCommand::CreateSession(request))? {
        SessionLogResponse::SessionCommandApplied { .. } => Ok(()),
        SessionLogResponse::Error { error } => bail!("create session failed: {error}"),
        other => bail!("unexpected create session response: {other:?}"),
    }
}

pub fn execute_via_service(session_id: &str, session_command: SessionCommand) -> Result<()> {
    match session_log_contract::client::call_service(&SessionLogCommand::ExecuteSessionCommand(
        ExecuteSessionCommandRequest {
            command_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            session_command,
            message_projection: None,
        },
    ))? {
        SessionLogResponse::SessionCommandApplied { .. } => Ok(()),
        SessionLogResponse::Error { error } => bail!("execute session command failed: {error}"),
        other => bail!("unexpected execute session command response: {other:?}"),
    }
}

pub fn persist_via_service(session_id: &str, entries: Vec<SessionDeltaEntry>) -> Result<u64> {
    let management = management_via_service(session_id)?;
    let context = context_via_service(session_id)?;
    let previous_management = (context.next_management_sequence > 0).then_some(&management);
    match session_log_contract::client::call_service(&SessionLogCommand::PersistSessionDelta(
        Box::new(delta_request(
            session_id,
            context.next_management_sequence,
            previous_management,
            &management,
            entries,
        )),
    ))? {
        SessionLogResponse::SessionDeltaPersisted { next_sequence, .. } => Ok(next_sequence),
        SessionLogResponse::Error { error } => bail!("persist session delta failed: {error}"),
        other => bail!("unexpected persist session delta response: {other:?}"),
    }
}

pub fn initial_management(
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
) -> SessionManagement {
    let now = chrono::DateTime::from_timestamp_millis(created_at).unwrap_or_else(chrono::Utc::now);
    let mut management = SessionManagement::new(
        session_id.to_string(),
        name.to_string(),
        workspace.into(),
        false,
        Vec::<String>::new(),
        SessionInput {
            user_input: String::new(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        String::new(),
        now,
    );
    management.auto_session_name = false;
    management.use_last_tool_call_response = false;
    management
}

fn management_via_service(session_id: &str) -> Result<SessionManagement> {
    match session_log_contract::client::call_service(&SessionLogCommand::GetSession(
        GetSessionRequest {
            session_id: session_id.to_string(),
        },
    ))? {
        SessionLogResponse::Session {
            session: Some(session),
        } => serde_json::from_value(session.management)
            .with_context(|| format!("invalid management for session {session_id}")),
        SessionLogResponse::Session { session: None } => bail!("session {session_id} not found"),
        SessionLogResponse::Error { error } => bail!("get session failed: {error}"),
        other => bail!("unexpected get session response: {other:?}"),
    }
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

fn delta_request(
    session_id: &str,
    management_sequence: u64,
    previous_management: Option<&SessionManagement>,
    management: &SessionManagement,
    entries: Vec<SessionDeltaEntry>,
) -> PersistSessionDeltaRequest {
    let mut management = management.clone();
    management.session_log.clear();
    management.session_log_retention.omitted_entries = 0;
    PersistSessionDeltaRequest {
        session_id: session_id.to_string(),
        management_sequence,
        management_delta: SessionManagement::persistence_delta(previous_management, &management),
        retained_from_sequence: 0,
        entries,
    }
}
