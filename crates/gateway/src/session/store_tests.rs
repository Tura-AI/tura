use super::*;
use crate::test_support::SessionDbTestService;
use session_log_contract::{
    GetSessionRequest, PersistSessionDeltaRequest, ReadContextSliceRequest, SessionContextRecord,
    SessionDeltaEntry, SessionLogCommand, SessionLogResponse, SessionRecordProjection,
};

fn create_canonical_test_session(store: &SessionStore, directory: String) -> ApiSession {
    let info = store.build_session_info(
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
    let task_plan = info.projection.task_plan.clone();
    store
        .create_canonical_session(info, SessionCommand::CreateSession { task_plan })
        .expect("canonical test session should be created")
}

fn execute_test_command(store: &SessionStore, session_id: &str, command: SessionCommand) {
    store
        .execute_canonical_session_command_with_status_event(session_id, command)
        .expect("canonical test command should succeed");
}

fn message_record(
    session_id: &str,
    message_id: &str,
    role: &str,
    text: &str,
    created_at: i64,
) -> serde_json::Value {
    serde_json::json!({
        "id": message_id,
        "session_id": session_id,
        "role": role,
        "parent_id": null,
        "parts": [{
            "id": format!("{message_id}:part"),
            "type": "text",
            "content": text,
            "text": text,
            "metadata": null,
            "call_id": null,
            "tool": null,
            "state": null
        }],
        "created_at": created_at,
        "updated_at": created_at
    })
}

fn persist_runtime_owned_session_for_test(
    store: &SessionStore,
    session_id: &str,
    _parent_id: Option<String>,
) {
    let messages = store
        .get_messages(session_id)
        .into_iter()
        .map(|message| serde_json::to_value(message).expect("message json"))
        .collect::<Vec<_>>();
    persist_session_messages_for_test(store, session_id, None, messages);
}

fn persist_session_messages_for_test(
    _store: &SessionStore,
    session_id: &str,
    _parent_id: Option<String>,
    messages: Vec<serde_json::Value>,
) {
    let snapshot = match session_log_contract::client::call_service(&SessionLogCommand::GetSession(
        GetSessionRequest {
            session_id: session_id.to_string(),
        },
    ))
    .expect("session_log get should reach test service")
    {
        SessionLogResponse::Session {
            session: Some(session),
        } => session,
        SessionLogResponse::Session { session: None } => {
            panic!("session {session_id} should exist before test DB delta")
        }
        SessionLogResponse::Error { error } => panic!("session_log get failed: {error}"),
        other => panic!("unexpected session_log get response: {other:?}"),
    };
    let mut management = snapshot
        .into_management()
        .expect("test snapshot must contain one canonical lifecycle projection");
    management.session_log.clear();
    management.session_log_retention.omitted_entries = 0;
    let context = match session_log_contract::client::call_service(
        &SessionLogCommand::ReadContextSlice(ReadContextSliceRequest {
            session_id: session_id.to_string(),
            max_estimated_tokens: u64::MAX,
        }),
    )
    .expect("session_log context read should reach test service")
    {
        SessionLogResponse::ContextSlice { context } => context,
        SessionLogResponse::Error { error } => panic!("session_log context read failed: {error}"),
        other => panic!("unexpected session_log context response: {other:?}"),
    };
    let previous_management = (context.next_management_sequence > 0).then_some(&management);
    let entries = messages
        .into_iter()
        .enumerate()
        .map(|(sequence, record)| test_delta_entry(sequence as u64, record))
        .collect();
    let response = session_log_contract::client::call_service(
        &SessionLogCommand::PersistSessionDelta(Box::new(PersistSessionDeltaRequest {
            session_id: session_id.to_string(),
            management_sequence: context.next_management_sequence,
            management_delta: lifecycle::SessionManagement::persistence_delta(
                previous_management,
                &management,
            ),
            retained_from_sequence: 0,
            entries,
        })),
    )
    .expect("session_log delta should reach test service");
    match response {
        SessionLogResponse::SessionDeltaPersisted { .. } => {}
        SessionLogResponse::Error { error } => {
            panic!("session_log delta failed: {error}")
        }
        other => panic!("unexpected session_log delta response: {other:?}"),
    }
}

fn test_delta_entry(sequence: u64, record: serde_json::Value) -> SessionDeltaEntry {
    let message_id = record["id"].as_str().expect("test message id").to_string();
    let role = record["role"].as_str().unwrap_or("runtime").to_string();
    let created_at = record["created_at"].as_i64().unwrap_or_default();
    let updated_at = record["updated_at"].as_i64().unwrap_or(created_at);
    let session_id = record["session_id"]
        .as_str()
        .expect("test message session id")
        .to_string();
    SessionDeltaEntry {
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
    }
}

#[test]
fn update_session_status_updates_stored_status() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    assert!(store.update_session_context_tokens(
        &session.id,
        crate::contracts::SessionContextTokens {
            input: 12_345,
            limit: 76_800,
        },
    ));
    assert!(store.update_session_runtime_usage(
        &session.id,
        serde_json::json!({
            "total_tokens": 99,
            "total_cost": 0.034,
            "currency": "USD",
        })
    ));
    let mut cursor = store.event_cursor();

    execute_test_command(
        &store,
        &session.id,
        SessionCommand::RuntimeStarted {
            runtime_id: "runtime-status-event".to_string(),
        },
    );
    let updated = store
        .get_session(&session.id)
        .expect("session should exist");
    assert_eq!(updated.status, ApiSessionStatus::Busy);
    let updated_event = store
        .next_event(&mut cursor)
        .expect("session update event should exist");
    assert!(matches!(updated_event, GlobalEvent::SessionUpdated { .. }));
    let status_event = store
        .next_event(&mut cursor)
        .expect("status event should follow the session update");
    match status_event {
        GlobalEvent::SessionStatus { properties } => {
            assert_eq!(properties.session_id, session.id);
            assert_eq!(properties.updated_at, updated.updated_at);
            assert_eq!(properties.context_tokens.input, 12_345);
            assert_eq!(properties.context_tokens.limit, 76_800);
            assert_eq!(properties.usage.context_tokens.input, 12_345);
            assert_eq!(properties.usage.context_tokens.limit, 76_800);
            assert_eq!(properties.usage.tokens["total_tokens"], 99);
            assert_eq!(properties.usage.cost, Some(0.034));
            assert_eq!(properties.usage.currency.as_deref(), Some("USD"));
        }
        other => panic!("unexpected event: {other:?}"),
    }

    execute_test_command(
        &store,
        &session.id,
        SessionCommand::RuntimeCompleted {
            runtime_id: "runtime-status-event".to_string(),
        },
    );
    let updated = store
        .get_session(&session.id)
        .expect("session should exist");
    assert_eq!(updated.status, ApiSessionStatus::Idle);
}

#[test]
fn add_tool_message_updates_existing_call_id() {
    let store = SessionStore::new();
    let session = store.create_session(
        Some("C:/workspace".to_string()),
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

    let first = store
        .add_tool_message(
            &session.id,
            "grep".to_string(),
            "call-1".to_string(),
            serde_json::json!({
                "status": "running",
                "input": { "pattern": "foo" },
                "time": { "start": 1 }
            }),
            None,
        )
        .expect("running tool message should be stored");

    let second = store
        .add_tool_message(
            &session.id,
            "grep".to_string(),
            "call-1".to_string(),
            serde_json::json!({
                "status": "completed",
                "input": { "pattern": "foo" },
                "output": "matched",
                "title": "Called `grep`",
                "metadata": {},
                "time": { "start": 1, "end": 2 }
            }),
            None,
        )
        .expect("completed tool message should update stored message");

    assert_eq!(first.id, second.id);
    let messages = store.get_messages(&session.id);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].parts.len(), 1);
    assert_eq!(
        messages[0].parts[0]
            .state
            .as_ref()
            .and_then(|state| state.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("completed")
    );
}

#[test]
fn transient_tool_message_emits_events_without_storing_messages() {
    let store = SessionStore::new();
    let session = store.create_session(
        Some("C:/workspace".to_string()),
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
    while store.pop_event().is_some() {}

    let message = store.emit_transient_tool_message_with_ids(
        &session.id,
        "command_run".to_string(),
        "runtime-1.tool.command_run".to_string(),
        serde_json::json!({
            "status": "running",
            "input": { "commands": [{ "command_type": "shell_command", "command_line": "npm test" }] },
            "metadata": { "kind": "mano_tool_call", "runtime_id": "runtime-1", "transient": true, "streaming_partial": true },
            "time": { "start": 1 }
        }),
        Some(serde_json::json!({
            "kind": "mano_tool_call",
            "runtime_id": "runtime-1",
            "transient": true,
            "streaming_partial": true
        })),
        "runtime-1.message".to_string(),
        "runtime-1.tool.command_run".to_string(),
    );

    assert_eq!(message.id, "runtime-1.message");
    assert!(store.get_messages(&session.id).is_empty());
    assert!(matches!(
        store.pop_event(),
        Some(GlobalEvent::MessageUpdated { .. })
    ));
    assert!(matches!(
        store.pop_event(),
        Some(GlobalEvent::MessagePartUpdated { .. })
    ));
    assert!(store.get_messages(&session.id).is_empty());
}

#[test]
fn add_tool_message_normalizes_running_state_with_final_output_metadata() {
    let store = SessionStore::new();
    let session = store.create_session(
        Some("C:/workspace".to_string()),
        None,
        None,
        Some("general".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );

    store
        .add_tool_message(
            &session.id,
            "command_run".to_string(),
            "call-1".to_string(),
            serde_json::json!({
                "status": "running",
                "input": { "commands": [] },
                "metadata": {
                    "kind": "mano_tool_call",
                    "output": {
                        "ok": false,
                        "errors": [{ "message": "bad command" }]
                    }
                },
                "time": { "start": 1 }
            }),
            Some(serde_json::json!({
                "kind": "mano_tool_call",
                "output": {
                    "ok": false,
                    "errors": [{ "message": "bad command" }]
                },
                "error": "bad command"
            })),
        )
        .expect("tool message should be stored");

    let messages = store.get_messages(&session.id);
    let state = messages[0].parts[0]
        .state
        .as_ref()
        .expect("part should have state");
    assert_eq!(
        state.get("status").and_then(serde_json::Value::as_str),
        Some("error")
    );
    assert_eq!(
        state.get("error").and_then(serde_json::Value::as_str),
        Some("bad command")
    );
    assert!(state
        .get("time")
        .and_then(|time| time.get("end"))
        .and_then(serde_json::Value::as_i64)
        .is_some());
}

#[test]
fn user_commands_are_shared_from_parent_to_child_sessions() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let child_id = format!("child-{}", Uuid::new_v4());
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    store
        .register_canonical_child_session(
            &session.id,
            &child_id,
            Some("C:/workspace".to_string()),
            Some("Subtask".to_string()),
            Some("read files".to_string()),
        )
        .expect("canonical child should be registered");
    execute_test_command(
        &store,
        &session.id,
        SessionCommand::RuntimeStarted {
            runtime_id: "runtime-child-cancel".to_string(),
        },
    );
    execute_test_command(
        &store,
        &session.id,
        SessionCommand::QueueUserInputWhileBusy {
            input: "focus on tests".to_string(),
        },
    );

    assert_eq!(
        store
            .session_lifecycle_projection(&session.id)
            .expect("root projection")
            .pending_user_inputs,
        vec!["focus on tests"]
    );
    assert!(
        store
            .session_lifecycle_projection(&child_id)
            .expect("child projection")
            .pending_user_inputs
            .is_empty(),
        "the child cache must not hold a second queue"
    );

    let root_id = store.root_session_id(&child_id);
    assert_eq!(root_id, session.id);
    execute_test_command(
        &store,
        &root_id,
        SessionCommand::QueueUserInputWhileBusy {
            input: "also update docs".to_string(),
        },
    );
    let consumed = store
        .execute_canonical_session_command(&root_id, SessionCommand::ConsumeQueuedUserInputs)
        .expect("root queue should be consumed atomically");
    assert_eq!(
        consumed.event,
        SessionEvent::QueuedUserInputsConsumed {
            inputs: vec!["focus on tests".to_string(), "also update docs".to_string()]
        }
    );
    assert!(consumed.projection.pending_user_inputs.is_empty());
}

#[test]
fn hydrated_child_session_keeps_parent_mapping() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-child-session-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let parent = store.create_session(
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

    store
        .register_canonical_child_session(
            &parent.id,
            "child-1",
            Some(directory.clone()),
            Some("Subtask".to_string()),
            Some("read files".to_string()),
        )
        .expect("canonical child should be created");
    persist_runtime_owned_session_for_test(&store, "child-1", Some(parent.id.clone()));

    let hydrated = SessionStore::new();
    hydrated.hydrate_directory(Some(directory));
    let child = hydrated
        .get_session("child-1")
        .expect("child should hydrate");

    assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
    assert_eq!(hydrated.list_child_session_ids(&parent.id), vec!["child-1"]);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn child_session_derives_workspace_and_task_instruction_context() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-child-context-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let child_id = format!("child-{}", Uuid::new_v4());
    let parent = store.create_session(
        Some(directory.clone()),
        None,
        None,
        Some("coding".to_string()),
        false,
        false,
        true,
        None,
        false,
        true,
    );

    let child = store
        .register_canonical_child_session(
            &parent.id,
            &child_id,
            parent.directory.clone(),
            Some("Backend subtask".to_string()),
            Some("Read docs/backend/ACCEPTANCE.md and implement the backend module.".to_string()),
        )
        .expect("canonical child should be created");
    let child_info = store
        .get_session_info(&child_id)
        .expect("child session info should exist");
    let messages = store.get_messages(&child_id);

    assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
    assert_eq!(child.directory.as_deref(), Some(directory.as_str()));
    assert_eq!(child_info.session_directory, PathBuf::from(&directory));
    assert!(child_info.disable_permission_restrictions);
    assert!(messages.iter().any(|message| {
        message.role == MessageRole::User
            && message.parts.iter().any(|part| {
                part.text
                    .as_deref()
                    .is_some_and(|text| text.contains("docs/backend/ACCEPTANCE.md"))
            })
    }));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cancellation_scope_includes_root_and_descendants_from_child() {
    let _service = SessionDbTestService::start();
    let root_directory =
        std::env::temp_dir().join(format!("tura-cancellation-scope-{}", Uuid::new_v4()));
    let directory = root_directory.to_string_lossy().to_string();
    let store = SessionStore::new();
    let child_id = format!("child-{}", uuid::Uuid::new_v4());
    let grandchild_id = format!("grandchild-{}", uuid::Uuid::new_v4());
    let root = store.create_session(
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

    store
        .register_canonical_child_session(
            &root.id,
            &child_id,
            Some(directory.clone()),
            Some("Subtask 1".to_string()),
            Some("first".to_string()),
        )
        .expect("canonical child should be created");
    store
        .register_canonical_child_session(
            &child_id,
            &grandchild_id,
            Some(directory),
            Some("Subtask 1.1".to_string()),
            Some("nested".to_string()),
        )
        .expect("canonical grandchild should be created");

    assert_eq!(
        store.cancellation_scope_session_ids(&child_id),
        vec![root.id, child_id, grandchild_id]
    );
    let _ = std::fs::remove_dir_all(root_directory);
}

#[test]
fn update_session_title_persists_to_management_name() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    let updated = store
        .update_session(
            &session.id,
            Some("修复登录流程".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("session should update");

    assert_eq!(updated.name.as_deref(), Some("修复登录流程"));
    let info = store.sessions.read();
    let stored = info.get(&session.id).expect("session should remain stored");
    assert_eq!(stored.name, "修复登录流程");
}

#[test]
fn update_session_task_management_persists_and_lists_status() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    let updated = store
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
                "plan_summary": "计划入口名称",
                "task_summary": "执行状态机名称",
                "status": "question",
                "start_at": "2026-05-25T08:30:00Z",
                "poll_interval": { "m": 0, "d": 1, "h": 2, "s": 3 },
                "sub_session_id": "sub-1",
                "step": 2
            })),
        )
        .expect("session should update");

    assert_eq!(updated.plan_summary.as_deref(), Some("计划入口名称"));
    assert_eq!(
        updated.task_management["status"],
        serde_json::json!("question")
    );
    assert_eq!(updated.task_management["start_condition"], "polling_task");
    assert_eq!(updated.task_management["step"], serde_json::json!(2));
    assert_eq!(updated.name.as_deref(), Some("执行状态机名称"));

    let listed = store
        .list_sessions()
        .into_iter()
        .find(|item| item.id == session.id)
        .expect("session should be listed");
    assert_eq!(
        listed.session_display_name.as_deref(),
        Some("执行状态机名称")
    );
    assert_eq!(listed.task_management["sub_session_id"], "sub-1");
}

#[test]
fn session_display_name_prefers_auto_session_name_over_plan_summary() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    let planned = store
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
                "plan_summary": "Original plan title",
                "task_summary": "First generated task"
            })),
        )
        .expect("session should accept initial task patch");
    assert_eq!(
        planned.session_display_name.as_deref(),
        Some("First generated task")
    );

    let renamed = store
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
                "task_summary": "Latest agent task detail",
                "status": "doing"
            })),
        )
        .expect("session should update auto-generated task name");

    assert_eq!(renamed.name.as_deref(), Some("Latest agent task detail"));
    assert_eq!(
        renamed.session_display_name.as_deref(),
        Some("Latest agent task detail")
    );
    assert_eq!(renamed.plan_summary.as_deref(), Some("Original plan title"));
}

#[test]
fn auto_session_name_can_be_disabled_for_task_summary_patches() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    let updated = store
        .update_session_auto_session_name(&session.id, false)
        .expect("auto session name should update");
    assert!(!updated.auto_session_name);

    let updated = store
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
            Some(serde_json::json!({
                "task_summary": "Generated title",
                "status": "doing"
            })),
        )
        .expect("session should update");

    assert!(!updated.auto_session_name);
    assert_eq!(updated.name.as_deref(), Some("Manual title"));
    assert_eq!(updated.task_management["task_summary"], "Generated title");
}

#[test]
fn scheduled_task_patch_clears_previous_polling_interval() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    store
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
                "task_summary": "轮询待办工单",
                "start_at": "2026-05-25T08:30:00Z",
                "poll_interval": { "m": 0, "d": 0, "h": 1, "s": 0 }
            })),
        )
        .expect("polling task should update");

    let updated = store
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
                "start_at": "2026-05-26T09:45:00Z",
                "poll_interval": { "m": 0, "d": 0, "h": 0, "s": 0 }
            })),
        )
        .expect("scheduled task should update");

    assert_eq!(
        updated.task_management["poll_interval"],
        serde_json::json!({ "m": 0, "d": 0, "h": 0, "s": 0 })
    );
    assert_eq!(
        updated.task_management["start_at"],
        serde_json::json!("2026-05-26T09:45:00Z")
    );
}

#[test]
fn single_task_patch_defaults_nonce_to_session_step_zero() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    let updated = store
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
                "plan_summary": "Single task contract",
                "task_summary": "Run one task"
            })),
        )
        .expect("session should update");

    let task_id = updated.task_management["task_id"]
        .as_str()
        .expect("task_id should be present");
    assert_eq!(task_id.len(), 8);
    assert!(task_id.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_eq!(updated.task_management["step"], serde_json::json!(1));
}

#[test]
fn multi_task_patch_matches_task_id_and_creates_defaulted_tasks() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    let planned = store
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
                "plan_summary": "Multi task contract",
                "tasks": [
                    {
                        "task_id": "inspect",
                        "step": 1,
                        "task_summary": "Inspect wiring",
                        "deliverable": "Find the files."
                    },
                    {
                        "task_id": "verify",
                        "step": 2,
                        "task_summary": "Verify wiring",
                        "deliverable": "Delivery spelling.",
                        "status": "question"
                    }
                ]
            })),
        )
        .expect("initial multi-task patch should update");

    assert_eq!(
        planned.task_management["tasks"]
            .as_array()
            .expect("task_management.tasks should be an array")
            .len(),
        2
    );

    let updated = store
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
                "tasks": [
                    {
                        "task_id": "inspect",
                        "status": "done"
                    },
                    {
                        "task_summary": "Generated follow-up"
                    }
                ]
            })),
        )
        .expect("follow-up multi-task patch should update");

    let tasks = updated.task_management["tasks"]
        .as_array()
        .expect("multi-task state should serialize as tasks array");
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0]["task_id"], "inspect");
    assert_eq!(tasks[0]["status"], "done");
    assert_eq!(tasks[1]["task_id"], "verify");
    assert_eq!(tasks[1]["status"], "question");
    assert_eq!(tasks[1]["deliverable"], "Delivery spelling.");
    let generated_task_id = tasks[2]["task_id"]
        .as_str()
        .expect("generated task_id should be present");
    assert_eq!(generated_task_id.len(), 8);
    assert!(generated_task_id.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_eq!(tasks[2]["step"], 3);
    assert_eq!(tasks[2]["task_summary"], "Generated follow-up");
    assert!(tasks[2].get("status").is_none());
    assert_eq!(tasks[2]["start_condition"], "user_action");
}

#[test]
fn multi_task_patch_reorders_tasks_by_request_order() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    store
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
                "tasks": [
                    { "task_id": "alpha", "step": 1, "task_summary": "Alpha" },
                    { "task_id": "bravo", "step": 2, "task_summary": "Bravo" },
                    { "task_id": "charlie", "step": 3, "task_summary": "Charlie" }
                ]
            })),
        )
        .expect("initial multi-task patch should update");

    let updated = store
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
                "tasks": [
                    { "task_id": "charlie", "task_summary": "Charlie" },
                    { "task_id": "alpha", "task_summary": "Alpha" }
                ]
            })),
        )
        .expect("reorder patch should update");

    let tasks = updated.task_management["tasks"]
        .as_array()
        .expect("multi-task state should serialize as tasks array");
    let order: Vec<_> = tasks
        .iter()
        .map(|task| {
            (
                task["task_id"].as_str().expect("task_id should be text"),
                task["step"].as_u64().expect("step should be numeric"),
            )
        })
        .collect();
    assert_eq!(order, vec![("charlie", 1), ("alpha", 2), ("bravo", 3)]);
}

#[test]
fn task_management_patch_accepts_all_contract_enums() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());

    for status in [
        "todo",
        "waiting_user",
        "doing",
        "question",
        "done",
        "archived",
    ] {
        let updated = store
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
                Some(serde_json::json!({ "status": status })),
            )
            .expect("session should update");
        if status == "todo" {
            assert!(updated.task_management.get("status").is_none());
        } else {
            assert_eq!(updated.task_management["status"], status);
        }
    }

    for start_condition in [
        "session_idle",
        "user_action",
        "scheduled_task",
        "polling_task",
    ] {
        let updated = store
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
                    "status": "todo",
                    "start_condition": start_condition
                })),
            )
            .expect("session should update");
        assert_eq!(updated.task_management["start_condition"], start_condition);
        assert!(updated.task_management.get("status").is_none());
    }
}

#[test]
fn invalid_task_management_patch_keeps_previous_state() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-invalid-task-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, directory.clone());
    let valid = store
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
                "plan_summary": "Stable plan",
                "task_summary": "Stable task",
                "status": "todo",
                "start_condition": "user_action",
                "start_at": "2026-05-25T08:30:00Z",
                "poll_interval": { "m": 0, "d": 0, "h": 1, "s": 0 }
            })),
        )
        .expect("valid patch should update");
    let previous_task_management = valid.task_management.clone();
    let previous_plan_summary = valid.plan_summary;

    let invalid_status = store
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
                "plan_summary": "Should not leak",
                "task_summary": "Should not leak",
                "status": "blocked"
            })),
        )
        .expect("invalid patch remains non-fatal");
    assert_eq!(invalid_status.task_management, previous_task_management);
    assert_eq!(invalid_status.plan_summary, previous_plan_summary);

    let invalid_date = store
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
                "status": "done",
                "start_at": "not-a-date"
            })),
        )
        .expect("invalid date remains non-fatal");
    assert_eq!(invalid_date.task_management, previous_task_management);
    persist_runtime_owned_session_for_test(&store, &session.id, None);

    let hydrated = SessionStore::new();
    hydrated.hydrate_directory(Some(directory));
    let persisted = hydrated
        .get_session(&session.id)
        .expect("persisted session should hydrate");
    assert_eq!(persisted.task_management, previous_task_management);
    assert_eq!(persisted.plan_summary, previous_plan_summary);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn session_display_name_falls_back_to_new_session() {
    let mut info = SessionManager::create_session(
        Some("C:/workspace".to_string()),
        None,
        None,
        Some("coding".to_string()),
    );
    info.name.clear();
    info.projection.task_plan.plan_summary.clear();

    let session = api_session_from_info(&info, None);

    assert_eq!(session.session_display_name.as_deref(), Some("New Session"));
}

#[test]
fn user_messages_update_only_the_gateway_message_projection() {
    let store = SessionStore::new();
    let session = store.create_session(
        Some("C:/workspace".to_string()),
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

    let before = store
        .get_session_info(&session.id)
        .expect("session info should exist before message");
    let message = store
        .add_message(&session.id, MessageRole::User, "补充新的约束".to_string())
        .expect("message should be stored");
    let after = store
        .get_session_info(&session.id)
        .expect("session info should exist");
    assert_eq!(after.projection, before.projection);
    assert_eq!(after.name, before.name);
    assert_eq!(after.message_count, before.message_count + 1);
    assert_eq!(message.role, MessageRole::User);
    assert_eq!(message.parts[0].text.as_deref(), Some("补充新的约束"));
    assert_eq!(store.get_messages(&session.id).last(), Some(&message));
}

#[test]
fn reopened_session_hydrates_frontend_user_message() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-reopen-user-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, directory.clone());

    store
        .add_message_with_ids(
            &session.id,
            MessageRole::User,
            "关闭再打开后应该还能看到这条用户消息".to_string(),
            Some("msg_tui_reopen_user".to_string()),
            Some("part_tui_reopen_user".to_string()),
            None,
        )
        .expect("user message should be stored");
    store
        .add_message_with_ids(
            &session.id,
            MessageRole::Assistant,
            "收到".to_string(),
            Some("runtime-reopen.message".to_string()),
            Some("runtime-reopen.message".to_string()),
            None,
        )
        .expect("assistant message should be stored");
    persist_runtime_owned_session_for_test(&store, &session.id, None);

    let reopened = SessionStore::new();
    reopened.hydrate_directory(Some(directory));
    let messages = reopened.get_frontend_messages(&session.id);
    let user = messages
        .iter()
        .find(|message| message.role == MessageRole::User)
        .expect("hydrated messages should include the user prompt");
    assert_eq!(user.id, "msg_tui_reopen_user");
    assert_eq!(
        user.parts.first().map(|part| part.id.as_str()),
        Some("part_tui_reopen_user")
    );
    assert_eq!(
        user.parts.first().and_then(|part| part.text.as_deref()),
        Some("关闭再打开后应该还能看到这条用户消息")
    );
    assert!(messages.iter().any(|message| {
        message.role == MessageRole::Assistant
            && message.parts.first().and_then(|part| part.text.as_deref()) == Some("收到")
    }));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn frontend_messages_filter_system_role_from_session_db_projection() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-system-filter-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, directory.clone());

    store
        .add_message_with_ids(
            &session.id,
            MessageRole::System,
            "internal runtime prompt".to_string(),
            Some("msg_system_prompt".to_string()),
            Some("part_system_prompt".to_string()),
            None,
        )
        .expect("system message should be stored internally");
    store
        .add_message_with_ids(
            &session.id,
            MessageRole::User,
            "visible user request".to_string(),
            Some("msg_visible_user".to_string()),
            Some("part_visible_user".to_string()),
            None,
        )
        .expect("user message should be stored");
    store
        .add_message_with_ids(
            &session.id,
            MessageRole::Assistant,
            "visible assistant reply".to_string(),
            Some("msg_visible_assistant".to_string()),
            Some("part_visible_assistant".to_string()),
            None,
        )
        .expect("assistant message should be stored");
    persist_runtime_owned_session_for_test(&store, &session.id, None);

    let reopened = SessionStore::new();
    reopened.hydrate_directory(Some(directory));
    let frontend_messages = reopened.get_frontend_messages(&session.id);

    assert!(
        frontend_messages
            .iter()
            .all(|message| message.role != MessageRole::System),
        "frontend messages must not expose system prompts: {frontend_messages:#?}"
    );
    assert!(frontend_messages
        .iter()
        .any(|message| message.id == "msg_visible_user"));
    assert!(frontend_messages
        .iter()
        .any(|message| message.id == "msg_visible_assistant"));
    assert!(reopened
        .get_messages(&session.id)
        .iter()
        .any(|message| message.role == MessageRole::System));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn idle_status_refreshes_session_db_message_cache_after_runtime_write() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-stale-session-db-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, directory);
    execute_test_command(
        &store,
        &session.id,
        SessionCommand::RuntimeStarted {
            runtime_id: "runtime-message-refresh".to_string(),
        },
    );

    let user = message_record(
        &session.id,
        "msg_runtime_user_before_completion",
        "user",
        "visible user request before runtime completion",
        1,
    );
    persist_session_messages_for_test(&store, &session.id, None, vec![user]);
    store
        .refresh_messages_from_session_db(&session.id)
        .expect("seed stale cache from session DB");

    let cached_before_completion = store.get_frontend_messages(&session.id);
    assert_eq!(cached_before_completion.len(), 1);
    assert_eq!(cached_before_completion[0].role, MessageRole::User);

    let assistant = message_record(
        &session.id,
        "msg_runtime_assistant_after_completion",
        "assistant",
        "visible assistant reply after runtime completion",
        2,
    );
    let assistant = serde_json::from_value(assistant).expect("typed assistant message");
    store
        .execute_canonical_session_command_with_message(
            &session.id,
            SessionCommand::RuntimeCompleted {
                runtime_id: "runtime-message-refresh".to_string(),
            },
            assistant,
        )
        .expect("atomically complete runtime with assistant message");

    let refreshed_after_idle = store.get_frontend_messages(&session.id);
    assert!(
        refreshed_after_idle.iter().any(|message| {
            message.id == "msg_runtime_assistant_after_completion"
                && message.role == MessageRole::Assistant
                && message.parts.first().and_then(|part| part.text.as_deref())
                    == Some("visible assistant reply after runtime completion")
        }),
        "idle completion must refresh the stale session_db message cache: {refreshed_after_idle:#?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn user_messages_preserve_and_hydrate_pending_task_management_state() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-message-task-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let session = create_canonical_test_session(&store, directory.clone());
    let start_at = (Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    let scheduled = store
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
                "plan_summary": "Pending scheduled plan",
                "task_summary": "Ask before continuing",
                "status": "question",
                "start_condition": "scheduled_task",
                "start_at": start_at,
                "poll_interval": { "m": 5, "d": 0, "h": 1, "s": 30 }
            })),
        )
        .expect("scheduled task state should update");
    let previous_task_management = scheduled.task_management;

    store
        .add_message(
            &session.id,
            MessageRole::User,
            "用户补充：保持计划等待，不要自动改状态".to_string(),
        )
        .expect("message should be stored");

    let after_message = store
        .get_session(&session.id)
        .expect("session should remain available");
    assert_eq!(after_message.task_management, previous_task_management);
    persist_runtime_owned_session_for_test(&store, &session.id, None);

    let hydrated = SessionStore::new();
    hydrated.hydrate_directory(Some(directory));
    let persisted = hydrated
        .get_session(&session.id)
        .expect("hydrated session should exist");
    assert_eq!(persisted.task_management, previous_task_management);
    assert!(hydrated
        .get_frontend_messages(&session.id)
        .iter()
        .any(|message| message.parts.iter().any(|part| {
            part.text
                .as_deref()
                .is_some_and(|text| text.contains("保持计划等待"))
        })));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scheduler_claims_due_idle_tasks_and_skips_ineligible_tasks() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-scheduled-task-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let now = Utc::now();
    let due = (now - chrono::Duration::minutes(5)).to_rfc3339();
    let future = (now + chrono::Duration::minutes(5)).to_rfc3339();
    let scheduled = create_canonical_test_session(&store, directory.clone());
    let busy = create_canonical_test_session(&store, directory.clone());
    let done = create_canonical_test_session(&store, directory.clone());
    let user_action = create_canonical_test_session(&store, directory.clone());
    let future_scheduled = create_canonical_test_session(&store, directory.clone());
    let idle = create_canonical_test_session(&store, directory);

    store.update_session(
        &scheduled.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(serde_json::json!({
            "task_summary": "due scheduled",
            "status": "todo",
            "start_at": due
        })),
    );
    store.update_session(
        &busy.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(serde_json::json!({
            "task_summary": "busy scheduled",
            "status": "todo",
            "start_at": due
        })),
    );
    execute_test_command(
        &store,
        &busy.id,
        SessionCommand::RuntimeStarted {
            runtime_id: "runtime-busy-scheduled".to_string(),
        },
    );
    store.update_session(
        &done.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(serde_json::json!({
            "task_summary": "done scheduled",
            "status": "done",
            "start_at": due
        })),
    );
    store.update_session(
        &user_action.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(serde_json::json!({
            "task_summary": "manual only",
            "status": "todo"
        })),
    );
    store.update_session(
        &future_scheduled.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(serde_json::json!({
            "task_summary": "future scheduled",
            "status": "todo",
            "start_at": future
        })),
    );
    store.update_session(
        &idle.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(serde_json::json!({
            "task_summary": "idle pending",
            "status": "todo",
            "start_condition": "session_idle"
        })),
    );

    let claimed = store.claim_due_task_runs(now);
    let mut claimed_ids = claimed
        .iter()
        .map(|run| run.session_id.as_str())
        .collect::<Vec<_>>();
    claimed_ids.sort_unstable();
    let mut expected_ids = vec![scheduled.id.as_str()];
    expected_ids.sort_unstable();

    assert_eq!(claimed_ids, expected_ids);
    let (_, durable_records) = SessionDbClient::discover()
        .expect("session DB client")
        .list_session_records(scheduled.id.clone(), 0, 10)
        .expect("read durable scheduler message");
    assert_eq!(
        durable_records.len(),
        1,
        "a successful scheduler claim must commit its user message to Session DB"
    );
    let (durable_feed, durable_cursor) = SessionDbClient::discover()
        .expect("session DB client")
        .read_session_feed(scheduled.id.clone(), 0, 10)
        .expect("read durable scheduler feed");
    assert_eq!(
        durable_feed.last().map(|entry| entry.cursor),
        Some(durable_cursor)
    );
    assert!(durable_feed.iter().any(|entry| matches!(
        entry.event,
        session_log_contract::SessionFeedEvent::MessageUpserted { .. }
    )));
    let scheduled_messages = store.get_messages(&scheduled.id);
    assert_eq!(
        scheduled_messages.len(),
        1,
        "a successful scheduler claim must already include its durable user message"
    );
    let scheduled_message = &scheduled_messages[0];
    assert_eq!(scheduled_message.role, MessageRole::User);
    assert!(scheduled_message.parts.iter().any(|part| {
        part.text
            .as_deref()
            .is_some_and(|text| text.contains("due scheduled"))
            && part.metadata.as_ref().is_some_and(|metadata| {
                metadata.get("kind") == Some(&serde_json::json!("task_scheduler"))
                    && metadata.get("start_condition") == Some(&serde_json::json!("scheduled_task"))
            })
    }));
    assert_eq!(
        store
            .get_session(&scheduled.id)
            .expect("scheduled should exist")
            .task_management["status"],
        "doing"
    );
    execute_test_command(
        &store,
        &scheduled.id,
        SessionCommand::RuntimeStarted {
            runtime_id: "runtime-scheduled-complete".to_string(),
        },
    );
    execute_test_command(
        &store,
        &scheduled.id,
        SessionCommand::RuntimeCompleted {
            runtime_id: "runtime-scheduled-complete".to_string(),
        },
    );
    assert!(
        store
            .claim_due_task_runs(now + chrono::Duration::minutes(1))
            .iter()
            .all(|run| run.session_id != scheduled.id),
        "scheduled task should not be claimed again after it is already doing"
    );
    assert_eq!(
        store
            .get_session(&idle.id)
            .expect("idle should exist")
            .task_management["status"],
        "waiting_user"
    );
    assert_eq!(
        store
            .get_session(&done.id)
            .expect("done should exist")
            .task_management["status"],
        "done"
    );
    assert_eq!(
        store
            .get_session(&future_scheduled.id)
            .expect("future should exist")
            .task_management
            .get("status"),
        None
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn new_session_idle_task_added_while_idle_waits_for_user_action() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let now = Utc::now();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());
    store.update_session(
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
            "task_summary": "idle-added queued task",
            "status": "todo",
            "start_condition": "session_idle"
        })),
    );

    let updated = store
        .get_session(&session.id)
        .expect("session should exist");
    assert_eq!(updated.task_management["status"], "waiting_user");
    assert!(
        store.claim_due_task_runs(now).is_empty(),
        "session_idle task added while already idle must wait for user action"
    );
}

#[test]
fn new_session_idle_task_added_while_busy_runs_after_idle_edge() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();
    let now = Utc::now();
    let session = create_canonical_test_session(&store, "C:/workspace".to_string());
    execute_test_command(
        &store,
        &session.id,
        SessionCommand::RuntimeStarted {
            runtime_id: "runtime-session-idle".to_string(),
        },
    );
    store.update_session(
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
            "task_summary": "busy-added queued task",
            "status": "todo",
            "start_condition": "session_idle"
        })),
    );

    assert!(store.claim_due_task_runs(now).is_empty());

    execute_test_command(
        &store,
        &session.id,
        SessionCommand::RuntimeCompleted {
            runtime_id: "runtime-session-idle".to_string(),
        },
    );

    let claimed = store.claim_due_task_runs(now);
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].session_id, session.id);
    assert_eq!(claimed[0].task_summary, "busy-added queued task");
}

#[test]
fn scheduler_claim_persists_next_polling_start() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-polling-task-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let now = Utc::now();
    let due = now - chrono::Duration::minutes(30);
    let session = create_canonical_test_session(&store, directory.clone());
    store.update_session(
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
            "task_summary": "poll repo",
            "status": "todo",
            "start_condition": "polling_task",
            "start_at": due.to_rfc3339(),
            "poll_interval": { "m": 0, "d": 0, "h": 1, "s": 0 }
        })),
    );

    let claimed = store.claim_due_task_runs(now);

    assert_eq!(claimed.len(), 1);
    let updated = store
        .get_session(&session.id)
        .expect("session should exist after claim");
    let next_start = DateTime::parse_from_rfc3339(
        updated
            .task_management
            .get("start_at")
            .and_then(serde_json::Value::as_str)
            .expect("start_at should serialize"),
    )
    .expect("start_at should parse")
    .with_timezone(&Utc);
    assert!(next_start > now);
    persist_runtime_owned_session_for_test(&store, &session.id, None);

    let hydrated = SessionStore::new();
    hydrated.hydrate_directory(Some(directory));
    let persisted = hydrated
        .get_session(&session.id)
        .expect("persisted polling session should hydrate");
    assert_eq!(
        persisted.task_management["start_at"],
        updated.task_management["start_at"]
    );
    let runtime_id = "runtime-polling".to_string();
    execute_test_command(
        &store,
        &session.id,
        SessionCommand::RuntimeStarted {
            runtime_id: runtime_id.clone(),
        },
    );
    execute_test_command(
        &store,
        &session.id,
        SessionCommand::RuntimeCompleted { runtime_id },
    );
    assert!(
        store.claim_due_task_runs(now).is_empty(),
        "polling task should not be reclaimed until its next start_at is due"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn api_session_exposes_runtime_context_token_stats() {
    let mut info = SessionManager::create_session(
        Some("C:/workspace/context-token-session".to_string()),
        None,
        Some("fast".to_string()),
        Some("coding".to_string()),
    );
    info.context_tokens = lifecycle::ContextTokenStats {
        input: 12_345,
        limit: 76_800,
    };

    let session = api_session_from_info(&info, None);

    assert_eq!(session.context_tokens.input, 12_345);
    assert_eq!(session.context_tokens.limit, 76_800);
}

#[test]
fn local_then_feed_projection_update_emits_public_events_once() {
    let store = SessionStore::new();
    let session_id = store
        .list_sessions()
        .into_iter()
        .next()
        .expect("default session")
        .id;
    let mut projection = store
        .session_lifecycle_projection(&session_id)
        .expect("default lifecycle projection");
    projection.state = lifecycle::SessionState::Running;
    let mut cursor = store.event_cursor();

    let local = store
        .write_replaced_projection_cache(projection.clone(), None, None)
        .expect("local projection cache write");
    store.publish_session_updated(&local);
    let feed = store
        .write_reduced_projection_cache(projection, None, 123)
        .expect("feed projection cache write");
    store.publish_session_updated(&feed);

    assert!(local.changed);
    assert!(!feed.changed);
    assert_eq!(
        store
            .get_session(&session_id)
            .expect("timestamp-converged session")
            .updated_at,
        123
    );
    let events = std::iter::from_fn(|| store.next_event(&mut cursor)).collect::<Vec<_>>();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GlobalEvent::SessionUpdated { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GlobalEvent::SessionStatus { .. }))
            .count(),
        1
    );
}

#[test]
fn feed_then_local_projection_update_preserves_feed_timestamp_and_emits_once() {
    let store = SessionStore::new();
    let session_id = store
        .list_sessions()
        .into_iter()
        .next()
        .expect("default session")
        .id;
    let mut projection = store
        .session_lifecycle_projection(&session_id)
        .expect("default lifecycle projection");
    projection.state = lifecycle::SessionState::Running;
    let mut cursor = store.event_cursor();

    let feed = store
        .write_reduced_projection_cache(projection.clone(), None, 456)
        .expect("feed projection cache write");
    store.publish_session_updated(&feed);
    let local = store
        .write_replaced_projection_cache(projection, None, None)
        .expect("local projection cache write");
    store.publish_session_updated(&local);

    assert!(feed.changed);
    assert!(!local.changed);
    assert_eq!(local.session.updated_at, 456);
    assert_eq!(
        store
            .get_session(&session_id)
            .expect("feed-owned timestamp")
            .updated_at,
        456
    );
    let events = std::iter::from_fn(|| store.next_event(&mut cursor)).collect::<Vec<_>>();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GlobalEvent::SessionUpdated { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GlobalEvent::SessionStatus { .. }))
            .count(),
        1
    );
}
