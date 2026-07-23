use anyhow::{bail, Context, Result};
use lifecycle::{SessionCommand, SessionManagement, TaskPlan};
use session_log::SessionLogStore;
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

pub fn message_entry(
    sequence: u64,
    session_id: &str,
    message_id: &str,
    role: &str,
    content: &str,
    timestamp: i64,
) -> SessionDeltaEntry {
    SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: serde_json::json!({ "role": role, "content": content }).to_string(),
        },
        projection: Some(SessionRecordProjection {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            role: role.to_string(),
            created_at: timestamp,
            updated_at: timestamp,
            record: serde_json::json!({
                "id": message_id,
                "session_id": session_id,
                "role": role,
                "content": content,
                "parts": [{ "type": "text", "text": content, "content": content }],
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        }),
    }
}

pub fn delta_request(
    session_id: &str,
    management_sequence: u64,
    previous_management: Option<&SessionManagement>,
    management: &SessionManagement,
    retained_from_sequence: u64,
    entries: Vec<SessionDeltaEntry>,
) -> PersistSessionDeltaRequest {
    let mut management = management.clone();
    management.session_log.clear();
    management.session_log_retention.omitted_entries = retained_from_sequence;
    PersistSessionDeltaRequest {
        session_id: session_id.to_string(),
        management_sequence,
        management_delta: SessionManagement::persistence_delta(previous_management, &management),
        retained_from_sequence,
        entries,
    }
}

pub fn create_in_store(
    store: &SessionLogStore,
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
    task_plan: TaskPlan,
) -> Result<()> {
    store.create_session(create_request(
        session_id, workspace, name, created_at, task_plan,
    ))?;
    Ok(())
}

pub fn persist_in_store(
    store: &SessionLogStore,
    session_id: &str,
    retained_from_sequence: u64,
    entries: Vec<SessionDeltaEntry>,
) -> Result<u64> {
    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: session_id.to_string(),
        })?
        .with_context(|| format!("session {session_id} not found"))?;
    let management = snapshot.into_management().map_err(anyhow::Error::msg)?;
    let context = store.read_context_slice(ReadContextSliceRequest {
        session_id: session_id.to_string(),
        max_estimated_tokens: u64::MAX,
    })?;
    let previous_management = (context.next_management_sequence > 0).then_some(&management);
    store
        .persist_session_delta(delta_request(
            session_id,
            context.next_management_sequence,
            previous_management,
            &management,
            retained_from_sequence,
            entries,
        ))
        .map(|(next_sequence, _)| next_sequence)
}

pub fn create_with_message_in_store(
    store: &SessionLogStore,
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
    role: &str,
    content: &str,
) -> Result<()> {
    create_in_store(
        store,
        session_id,
        workspace,
        name,
        created_at,
        TaskPlan::default(),
    )?;
    persist_in_store(
        store,
        session_id,
        0,
        vec![message_entry(
            0,
            session_id,
            &format!("m-{session_id}"),
            role,
            content,
            created_at,
        )],
    )?;
    Ok(())
}

pub fn create_via_service(
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
    task_plan: TaskPlan,
) -> Result<()> {
    match session_log_contract::client::call_service(&SessionLogCommand::CreateSession(Box::new(
        create_request(session_id, workspace, name, created_at, task_plan),
    )))? {
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

pub fn persist_via_service(
    session_id: &str,
    retained_from_sequence: u64,
    entries: Vec<SessionDeltaEntry>,
) -> Result<u64> {
    let management = match session_log_contract::client::call_service(
        &SessionLogCommand::GetSession(GetSessionRequest {
            session_id: session_id.to_string(),
        }),
    )? {
        SessionLogResponse::Session {
            session: Some(session),
        } => session.management,
        SessionLogResponse::Session { session: None } => bail!("session {session_id} not found"),
        SessionLogResponse::Error { error } => bail!("get session failed: {error}"),
        other => bail!("unexpected get session response: {other:?}"),
    };
    let context = match session_log_contract::client::call_service(
        &SessionLogCommand::ReadContextSlice(ReadContextSliceRequest {
            session_id: session_id.to_string(),
            max_estimated_tokens: u64::MAX,
        }),
    )? {
        SessionLogResponse::ContextSlice { context } => context,
        SessionLogResponse::Error { error } => bail!("read context failed: {error}"),
        other => bail!("unexpected read context response: {other:?}"),
    };
    let previous_management = (context.next_management_sequence > 0).then_some(&management);
    match session_log_contract::client::call_service(&SessionLogCommand::PersistSessionDelta(
        Box::new(delta_request(
            session_id,
            context.next_management_sequence,
            previous_management,
            &management,
            retained_from_sequence,
            entries,
        )),
    ))? {
        SessionLogResponse::SessionDeltaPersisted { next_sequence, .. } => Ok(next_sequence),
        SessionLogResponse::Error { error } => bail!("persist session delta failed: {error}"),
        other => bail!("unexpected persist session delta response: {other:?}"),
    }
}

pub fn create_with_message_via_service(
    session_id: &str,
    workspace: &str,
    name: &str,
    created_at: i64,
    role: &str,
    content: &str,
) -> Result<()> {
    create_via_service(session_id, workspace, name, created_at, TaskPlan::default())?;
    persist_via_service(
        session_id,
        0,
        vec![message_entry(
            0,
            session_id,
            &format!("m-{session_id}"),
            role,
            content,
            created_at,
        )],
    )?;
    Ok(())
}
