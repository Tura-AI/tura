use chrono::{TimeZone, Utc};
use gateway::SessionStore;
use lifecycle::{
    PlanStatus, RuntimeState, SessionCommand, SessionEvent, SessionState, TaskPlan, TaskStep,
};
use lifecycle::{SessionInput, SessionManagement};
use runtime::context::{
    accumulate_message, accumulate_tool_result, build_messages_from_session,
    user_input_content_value,
};
use serde_json::{json, Value};
use session_log::SessionLogStore;
use session_log_contract::{
    CreateSessionRequest, ExecuteSessionCommandRequest, GetSessionRequest,
    ListSessionRecordsRequest, ListSessionsRequest, PersistSessionDeltaRequest,
    SessionContextRecord, SessionDeltaEntry, SessionRecordProjection,
};
use std::{
    ffi::OsString,
    io,
    path::PathBuf,
    thread::JoinHandle,
    time::{Duration, Instant},
};

struct EquivalenceSessionDb {
    _root: tempfile::TempDir,
    previous_environment: Vec<(&'static str, Option<OsString>)>,
    handle: Option<JoinHandle<Option<String>>>,
}

impl EquivalenceSessionDb {
    fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let previous_environment = ["TURA_HOME", "SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"]
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect();
        let root = tempfile::tempdir()?;
        let home = root.path().join("home");
        std::fs::create_dir_all(&home)?;
        let store = SessionLogStore::open(root.path())?;
        std::env::set_var("TURA_HOME", &home);
        std::env::set_var("SESSION_LOG_DB_ROOT", root.path());
        std::env::remove_var("TURA_DB_ROOT");

        let handle = std::thread::spawn(move || {
            session_log::ipc::serve_blocking(store)
                .err()
                .map(|error| format!("{error:#}"))
        });
        let mut service = Self {
            _root: root,
            previous_environment,
            handle: Some(handle),
        };
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            if service.handle.as_ref().is_some_and(JoinHandle::is_finished) {
                let detail = service
                    .join()?
                    .unwrap_or_else(|| "service exited before publishing its address".to_string());
                return Err(io::Error::other(detail).into());
            }
            if session_log_contract::client::service_is_running() {
                return Ok(service);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "session DB did not become reachable for equivalence capture",
        )
        .into())
    }

    fn join(&mut self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        if let Some(handle) = self.handle.take() {
            return handle
                .join()
                .map_err(|_| io::Error::other("session DB thread panicked").into());
        }
        Ok(None)
    }
}

impl Drop for EquivalenceSessionDb {
    fn drop(&mut self) {
        let shutdown_requested = session_log_contract::client::call_service(
            &session_log_contract::SessionLogCommand::Shutdown,
        )
        .is_ok();
        let finished = self.handle.as_ref().is_some_and(JoinHandle::is_finished);
        if shutdown_requested || finished {
            let _ = self.join();
        }
        for (key, value) in self.previous_environment.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = json!({
        "schema": "tura.runtime-session-equivalence.v1",
        "runtime_transitions": runtime_transition_capture(),
        "session_transitions": session_transition_capture(),
        "context": context_capture()?,
        "gateway_busy_input": gateway_busy_input_capture()?,
        "store": store_capture()?,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn runtime_transition_capture() -> Value {
    let states = [
        RuntimeState::Created,
        RuntimeState::Dispatching,
        RuntimeState::WaitingFirstToken,
        RuntimeState::Streaming,
        RuntimeState::Finished,
        RuntimeState::Failed,
        RuntimeState::TimedOut,
        RuntimeState::Cancelled,
    ];
    Value::Array(
        states
            .iter()
            .flat_map(|from| {
                states.iter().map(move |to| {
                    json!({
                        "from": from,
                        "to": to,
                        "allowed": from.can_transition_to(*to),
                        "from_status": from.call_result_status(),
                        "from_live": from.is_live(),
                    })
                })
            })
            .collect(),
    )
}

fn session_transition_capture() -> Value {
    let states = [
        SessionState::Created,
        SessionState::Running,
        SessionState::Paused,
        SessionState::Completed,
        SessionState::Failed,
        SessionState::Cancelled,
        SessionState::Interrupted,
    ];
    Value::Array(
        states
            .iter()
            .flat_map(|from| {
                states.iter().map(move |to| {
                    json!({
                        "from": from,
                        "to": to,
                        "allowed": from.can_transition_to(*to),
                        "ui_status": from.ui_status(),
                        "recoverable_running": from.is_recoverable_running(),
                    })
                })
            })
            .collect(),
    )
}

fn fixed_session(workspace: PathBuf) -> SessionManagement {
    let now = Utc
        .with_ymd_and_hms(2026, 7, 19, 0, 0, 0)
        .single()
        .expect("fixed timestamp");
    SessionManagement::new(
        "session-phase0-fixed".to_string(),
        "Phase 0 fixed session".to_string(),
        workspace,
        false,
        Vec::<String>::new(),
        SessionInput {
            user_input: "Inspect [MEDIA:data:image/png;base64,QUJD:MEDIA] now.".to_string(),
            file_input: Vec::new(),
            agent: Some("direct".to_string()),
            runtime_context: None,
            planning_mode_override: None,
        },
        "Preserve runtime and session behavior".to_string(),
        now,
    )
}

fn context_capture() -> Result<Value, Box<dyn std::error::Error>> {
    let workspace = tempfile::tempdir()?;
    let mut session = fixed_session(workspace.path().to_path_buf());
    accumulate_message(&mut session, "system", json!("fixed-system"))?;
    accumulate_message(&mut session, "assistant", json!("ordinary assistant text"))?;
    accumulate_tool_result(
        &mut session,
        "command_run",
        json!({
            "commands": [
                {"step": 1, "command_type": "task_status", "command_line": "{\"status\":\"doing\"}"},
                {"step": 1, "command_type": "shell_command", "command_line": "exit 7"}
            ]
        }),
        json!({
            "results": [
                {"step": 1, "command_type": "task_status", "success": true, "output": {"task_status": {"status": "doing"}}},
                {"step": 1, "command_type": "shell_command", "success": false, "error": "fixed failure"}
            ]
        }),
        false,
        Some("fixed partial failure".to_string()),
    )?;
    let before_compact = build_messages_from_session(&session);
    session.session_log.push(
        json!({
            "type": "context_compaction",
            "category": "compact_context",
            "content": "fixed compact summary",
            "workspace_snapshot": "<WORKSPACE_SNAPSHOT>\nfixed\n</WORKSPACE_SNAPSHOT>",
            "environment_context": "<environment_context>fixed</environment_context>",
            "timestamp": "2026-07-19T00:00:00+00:00"
        })
        .to_string(),
    );
    let after_compact = build_messages_from_session(&session);
    Ok(json!({
        "media_input": user_input_content_value(&session.input.user_input),
        "before_compact": before_compact,
        "after_compact": after_compact,
        "tool_result_count": session.session_log.iter().filter(|entry| entry.contains("\"type\":\"tool_result\"")).count(),
    }))
}

fn gateway_busy_input_capture() -> Result<Value, Box<dyn std::error::Error>> {
    let _service = EquivalenceSessionDb::start()?;
    let workspace = tempfile::tempdir()?;
    let store = SessionStore::new();
    let info = store.build_session_info(
        Some(workspace.path().to_string_lossy().to_string()),
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
    let task_plan = info.management.task_plan.clone();
    let session = store
        .create_canonical_session(info, SessionCommand::CreateSession { task_plan })
        .map_err(io::Error::other)?;
    store
        .execute_canonical_session_command(
            &session.id,
            SessionCommand::RuntimeStarted {
                runtime_id: "runtime-busy-input".to_string(),
            },
        )
        .map_err(io::Error::other)?;
    let first = store
        .execute_canonical_session_command(
            &session.id,
            SessionCommand::QueueUserInputWhileBusy {
                input: " first queued input ".to_string(),
            },
        )
        .map_err(io::Error::other)?
        .projection
        .pending_user_inputs;
    let second = store
        .execute_canonical_session_command(
            &session.id,
            SessionCommand::QueueUserInputWhileBusy {
                input: "second queued input".to_string(),
            },
        )
        .map_err(io::Error::other)?
        .projection
        .pending_user_inputs;
    let consumed = store
        .execute_canonical_session_command(&session.id, SessionCommand::ConsumeQueuedUserInputs)
        .map_err(io::Error::other)?;
    let taken = match consumed.event {
        SessionEvent::QueuedUserInputsConsumed { inputs } => inputs,
        event => {
            return Err(io::Error::other(format!("unexpected consume event: {event:?}")).into())
        }
    };
    let empty = consumed.projection.pending_user_inputs;
    Ok(json!({
        "state_while_busy": store.get_session_info(&session.id).map(|info| info.management.state),
        "first": first,
        "second": second,
        "taken": taken,
        "empty_after_take": empty,
    }))
}

fn store_capture() -> Result<Value, Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let workspace_root = tempfile::tempdir()?;
    let workspace = workspace_root.path().join("fixed-workspace");
    std::fs::create_dir_all(&workspace)?;
    let workspace_text = workspace.to_string_lossy().replace('\\', "/");
    let store = SessionLogStore::open(root.path())?;
    let session_id = "session-store-fixed";
    let task_plan = TaskPlan {
        plan_summary: "Fixed store task".to_string(),
        detailed_tasks: vec![TaskStep {
            task_id: "task-fixed".to_string(),
            step: 1,
            start_at: Utc
                .with_ymd_and_hms(2026, 7, 19, 0, 0, 0)
                .single()
                .expect("fixed task timestamp"),
            status: PlanStatus::Doing,
            task_summary: "Fixed store task".to_string(),
            step_task: "Capture canonical store state".to_string(),
            ..TaskStep::default()
        }],
    };
    store.create_session(CreateSessionRequest {
        command_id: format!("create:{session_id}"),
        session_id: session_id.to_string(),
        creation_command: SessionCommand::CreateSession { task_plan },
        copy_context: false,
        workspace: workspace_text.clone(),
        session_directory: workspace_text.clone(),
        name: "Fixed Store Session".to_string(),
        created_at: 10,
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
    })?;
    store.execute_session_command(ExecuteSessionCommandRequest {
        command_id: format!("start:{session_id}"),
        session_id: session_id.to_string(),
        session_command: SessionCommand::StartUserTurn,
        message_projection: None,
    })?;
    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: session_id.to_string(),
        })?
        .ok_or("fixed session missing before delta")?;
    let mut management: SessionManagement = serde_json::from_value(snapshot.management)?;
    management.session_log.clear();
    management.session_log_retention.omitted_entries = 0;
    let management_delta = SessionManagement::persistence_delta(None, &management);
    let messages = [
        json!({"id": "message-user-fixed", "session_id": session_id, "role": "user", "created_at": 11, "updated_at": 11, "parts": [{"type": "text", "text": "fixed input"}]}),
        json!({"id": "message-assistant-fixed", "session_id": session_id, "role": "assistant", "created_at": 12, "updated_at": 12, "parts": [{"type": "text", "text": "fixed output"}]}),
    ];
    store.persist_session_delta(PersistSessionDeltaRequest {
        session_id: session_id.to_string(),
        management_sequence: 0,
        management_delta,
        retained_from_sequence: 0,
        entries: messages
            .into_iter()
            .enumerate()
            .map(|(sequence, record)| {
                let message_id = record["id"].as_str().expect("fixed message id").to_string();
                let role = record["role"]
                    .as_str()
                    .expect("fixed message role")
                    .to_string();
                let created_at = record["created_at"].as_i64().expect("fixed created_at");
                let updated_at = record["updated_at"].as_i64().expect("fixed updated_at");
                SessionDeltaEntry {
                    context: SessionContextRecord {
                        sequence: sequence as u64,
                        raw_record: json!({ "id": message_id, "role": role }).to_string(),
                    },
                    projection: Some(SessionRecordProjection {
                        session_id: session_id.to_string(),
                        message_id,
                        role,
                        created_at,
                        updated_at,
                        record,
                    }),
                }
            })
            .collect(),
    })?;
    store.execute_session_command(ExecuteSessionCommandRequest {
        command_id: format!("interrupt:{session_id}"),
        session_id: session_id.to_string(),
        session_command: SessionCommand::InterruptSession,
        message_projection: None,
    })?;
    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: session_id.to_string(),
        })?
        .ok_or("fixed session missing")?;
    let (page, sessions) = store.list_sessions(ListSessionsRequest {
        workspace: workspace_text,
        page: 0,
        page_size: 10,
    })?;
    let (records_page, records) = store.list_session_records(ListSessionRecordsRequest {
        session_id: session_id.to_string(),
        page: 0,
        page_size: 10,
    })?;
    Ok(json!({
        "snapshot": {
            "session_id": snapshot.session_id,
            "name": snapshot.name,
            "state": snapshot.state,
            "status": snapshot.status,
            "message_count": snapshot.message_count,
            "task_management": snapshot.task_management,
            "todos": snapshot.todos,
        },
        "list": {
            "page": page,
            "session_ids": sessions.into_iter().map(|session| session.session_id).collect::<Vec<_>>(),
        },
        "records": {
            "page": records_page,
            "items": records.into_iter().map(|record| {
                let mut payload = record.record;
                if let Some(object) = payload.as_object_mut() {
                    object.remove("session_id");
                }
                json!({
                    "message_id": record.message_id,
                    "role": record.role,
                    "record": payload,
                })
            }).collect::<Vec<_>>(),
        }
    }))
}
