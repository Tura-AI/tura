use super::*;

struct EnvRestore {
    keys: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvRestore {
    fn capture(keys: &[&'static str]) -> Self {
        Self {
            keys: keys
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect(),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (key, value) in &self.keys {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

struct SessionDbTestService {
    _guard: std::sync::MutexGuard<'static, ()>,
    _env: EnvRestore,
    _root: tempfile::TempDir,
    handle: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
}

impl SessionDbTestService {
    fn start() -> Self {
        let guard = crate::test_support::env_lock();
        let env = EnvRestore::capture(&["TURA_HOME", "SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"]);
        let root = tempfile::tempdir().expect("session db root");
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).expect("session db home");
        std::env::set_var("TURA_HOME", &home);
        std::env::set_var("SESSION_LOG_DB_ROOT", root.path());
        std::env::remove_var("TURA_DB_ROOT");

        let handle = std::thread::spawn(session_log::service::run_socket_service);
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_secs(10) {
            if handle.is_finished() {
                let detail = match handle.join() {
                    Ok(Ok(())) => "service exited without publishing service.addr".to_string(),
                    Ok(Err(error)) => format!("service exited with error: {error:#}"),
                    Err(_) => "service thread panicked before publishing service.addr".to_string(),
                };
                panic!("session_db test service did not become reachable: {detail}");
            }
            if session_log::ipc::service_is_running() {
                return Self {
                    _guard: guard,
                    _env: env,
                    _root: root,
                    handle: Some(handle),
                };
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!(
            "session_db test service did not become reachable within 10s; addr_path={}",
            session_log::ipc::service_addr_path().display()
        );
    }
}

impl Drop for SessionDbTestService {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&session_log::SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[test]
fn update_session_status_updates_stored_status() {
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

    store.update_session_status(&session.id, SessionStatusMano::Busy);
    let updated = store
        .get_session(&session.id)
        .expect("session should exist");
    assert_eq!(updated.status, ApiSessionStatus::Busy);

    store.update_session_status(&session.id, SessionStatusMano::Idle);
    let updated = store
        .get_session(&session.id)
        .expect("session should exist");
    assert_eq!(updated.status, ApiSessionStatus::Idle);
}

#[test]
fn persist_session_ack_reports_missing_session_id() {
    let _service = SessionDbTestService::start();
    let store = SessionStore::new();

    let error = store
        .persist_session_ack("missing-session-for-ack")
        .expect_err("missing session should fail ACK");

    assert!(
        error.contains("session missing-session-for-ack not found in session_db"),
        "ACK error should include session id and durable store context: {error}"
    );
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
    let store = SessionStore::new();
    let child_id = format!("child-{}", Uuid::new_v4());
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

    store.register_child_session(
        &session.id,
        &child_id,
        Some("C:/workspace".to_string()),
        Some("Subtask".to_string()),
        Some("read files".to_string()),
    );
    store.append_user_command(&session.id, "focus on tests");

    assert_eq!(
        store.user_commands_for_session(&session.id),
        vec!["focus on tests"]
    );
    assert_eq!(
        store.user_commands_for_session(&child_id),
        vec!["focus on tests"]
    );

    store.append_user_command(&child_id, "also update docs");
    assert_eq!(
        store.user_commands_for_session(&session.id),
        vec!["focus on tests", "also update docs"]
    );
    assert_eq!(
        store.user_commands_for_session(&child_id),
        vec!["focus on tests", "also update docs"]
    );
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

    store.register_child_session(
        &parent.id,
        "child-1",
        Some(directory.clone()),
        Some("Subtask".to_string()),
        Some("read files".to_string()),
    );

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
    let store = SessionStore::new();
    let child_id = format!("child-{}", Uuid::new_v4());
    let parent = store.create_session(
        Some("C:/workspace".to_string()),
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

    let child = store.register_child_session(
        &parent.id,
        &child_id,
        parent.directory.clone(),
        Some("Backend subtask".to_string()),
        Some("Read docs/backend/ACCEPTANCE.md and implement the backend module.".to_string()),
    );
    let child_info = store
        .get_session_info(&child_id)
        .expect("child session info should exist");
    let messages = store.get_messages(&child_id);

    assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
    assert_eq!(child.directory.as_deref(), Some("C:/workspace"));
    assert_eq!(
        child_info.management.session_directory,
        PathBuf::from("C:/workspace")
    );
    assert!(child_info.management.disable_permission_restrictions);
    assert!(messages.iter().any(|message| {
        message.role == MessageRole::User
            && message.parts.iter().any(|part| {
                part.text
                    .as_deref()
                    .is_some_and(|text| text.contains("docs/backend/ACCEPTANCE.md"))
            })
    }));
}

#[test]
fn cancellation_scope_includes_root_and_descendants_from_child() {
    let store = SessionStore::new();
    let child_id = format!("child-{}", uuid::Uuid::new_v4());
    let grandchild_id = format!("grandchild-{}", uuid::Uuid::new_v4());
    let root = store.create_session(
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

    store.register_child_session(
        &root.id,
        &child_id,
        Some("C:/workspace".to_string()),
        Some("Subtask 1".to_string()),
        Some("first".to_string()),
    );
    store.register_child_session(
        &child_id,
        &grandchild_id,
        Some("C:/workspace".to_string()),
        Some("Subtask 1.1".to_string()),
        Some("nested".to_string()),
    );

    assert_eq!(
        store.cancellation_scope_session_ids(&child_id),
        vec![root.id, child_id, grandchild_id]
    );
}

#[test]
fn update_session_title_persists_to_management_name() {
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
    assert_eq!(stored.management.session_name, "修复登录流程");
}

#[test]
fn update_session_task_management_persists_and_lists_status() {
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
    assert_eq!(listed.session_display_name.as_deref(), Some("计划入口名称"));
    assert_eq!(listed.task_management["sub_session_id"], "sub-1");
}

#[test]
fn auto_session_name_can_be_disabled_for_task_summary_patches() {
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
    let session = store.create_session(
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
    info.management.session_name.clear();
    info.management.task_plan.plan_summary.clear();

    let session = api_session_from_info(&info, None);

    assert_eq!(session.session_display_name.as_deref(), Some("New Session"));
}

#[test]
fn user_messages_are_recorded_in_session_management_log() {
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

    store
        .add_message(&session.id, MessageRole::User, "补充新的约束".to_string())
        .expect("message should be stored");
    let info = store
        .get_session_info(&session.id)
        .expect("session info should exist");
    assert!(info
        .management
        .session_log
        .iter()
        .any(|entry| entry.contains("补充新的约束")));
}

#[test]
fn user_messages_preserve_and_hydrate_pending_task_management_state() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-message-task-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let session = store.create_session(
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

    let hydrated = SessionStore::new();
    hydrated.hydrate_directory(Some(directory));
    let persisted = hydrated
        .get_session(&session.id)
        .expect("hydrated session should exist");
    assert_eq!(persisted.task_management, previous_task_management);
    let info = hydrated
        .get_session_info(&session.id)
        .expect("hydrated session info should exist");
    assert!(info
        .management
        .session_log
        .iter()
        .any(|entry| entry.contains("保持计划等待")));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scheduler_claims_due_idle_tasks_and_skips_ineligible_tasks() {
    let root = std::env::temp_dir().join(format!("tura-scheduled-task-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let now = Utc::now();
    let due = (now - chrono::Duration::minutes(5)).to_rfc3339();
    let future = (now + chrono::Duration::minutes(5)).to_rfc3339();
    let scheduled = store.create_session(
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
    let busy = store.create_session(
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
    let done = store.create_session(
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
    let user_action = store.create_session(
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
    let future_scheduled = store.create_session(
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
    let idle = store.create_session(
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
    store.update_session_status(&busy.id, SessionStatusMano::Busy);
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
    let mut expected_ids = vec![scheduled.id.as_str(), idle.id.as_str()];
    expected_ids.sort_unstable();

    assert_eq!(claimed_ids, expected_ids);
    assert_eq!(
        store
            .get_session(&scheduled.id)
            .expect("scheduled should exist")
            .task_management["status"],
        "doing"
    );
    store.update_session_status(&scheduled.id, SessionStatusMano::Idle);
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
            .status,
        ApiSessionStatus::Busy
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
fn scheduler_claim_persists_next_polling_start() {
    let _service = SessionDbTestService::start();
    let root = std::env::temp_dir().join(format!("tura-polling-task-{}", Uuid::new_v4()));
    let directory = root.to_string_lossy().to_string();
    let store = SessionStore::new();
    let now = Utc::now();
    let due = now - chrono::Duration::minutes(30);
    let session = store.create_session(
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

    let hydrated = SessionStore::new();
    hydrated.hydrate_directory(Some(directory));
    let persisted = hydrated
        .get_session(&session.id)
        .expect("persisted polling session should hydrate");
    assert_eq!(
        persisted.task_management["start_at"],
        updated.task_management["start_at"]
    );
    store.update_session_status(&session.id, SessionStatusMano::Idle);
    assert!(
        store.claim_due_task_runs(now).is_empty(),
        "polling task should not be reclaimed until its next start_at is due"
    );

    let _ = std::fs::remove_dir_all(root);
}
