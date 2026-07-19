use chrono::{TimeZone, Utc};
use gateway::{SessionStatus as GatewaySessionStatus, SessionStore};
use lifecycle::{RuntimeState, SessionState};
use runtime::context::{
    accumulate_message, accumulate_tool_result, build_messages_from_session,
    user_input_content_value,
};
use runtime::state_machine::session_management::{SessionInput, SessionManagement};
use serde_json::{json, Value};
use session_log::SessionLogStore;
use session_log_contract::{
    GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest, UpsertSessionRequest,
};
use std::path::PathBuf;

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
    let workspace = tempfile::tempdir()?;
    let store = SessionStore::new();
    let session = store.create_session(
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
    store.update_session_status(&session.id, GatewaySessionStatus::Busy);
    let first = store.append_user_command(&session.id, " first queued input ");
    let second = store.append_user_command(&session.id, "second queued input");
    let taken = store.take_user_commands_for_session(&session.id);
    let empty = store.user_commands_for_session(&session.id);
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
    store.upsert_session(UpsertSessionRequest {
        session: json!({
            "id": "session-store-fixed",
            "name": "Fixed Store Session",
            "directory": workspace_text,
            "created_at": 10,
            "updated_at": 20,
            "status": "busy",
            "task_management": {"status": "doing"},
            "management": {
                "session_id": "session-store-fixed",
                "session_name": "Fixed Store Session",
                "state": "running"
            }
        }),
        parent_id: None,
        messages: vec![
            json!({"id": "message-user-fixed", "role": "user", "created_at": 11, "updated_at": 11, "parts": [{"type": "text", "text": "fixed input"}]}),
            json!({"id": "message-assistant-fixed", "role": "assistant", "created_at": 12, "updated_at": 12, "parts": [{"type": "text", "text": "fixed output"}]}),
        ],
        todos: vec![json!({"id": "todo-fixed", "status": "doing"})],
    })?;
    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: "session-store-fixed".to_string(),
        })?
        .ok_or("fixed session missing")?;
    let (page, sessions) = store.list_sessions(ListSessionsRequest {
        workspace: workspace_text,
        page: 0,
        page_size: 10,
    })?;
    let (records_page, records) = store.list_session_records(ListSessionRecordsRequest {
        session_id: "session-store-fixed".to_string(),
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
            "items": records.into_iter().map(|record| json!({
                "message_id": record.message_id,
                "role": record.role,
                "record": record.record,
            })).collect::<Vec<_>>(),
        }
    }))
}
