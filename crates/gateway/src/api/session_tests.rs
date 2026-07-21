use super::{
    api_message_from_store, apply_single_change, config_model_override, filter_list_sessions,
    first_prompt_part_id, frontend_safe_reply_message, frontend_safe_value,
    inactive_sessions_from_probe, prompt_command_run_shell, prompt_message_id,
    prompt_model_acceleration, prompt_model_variant, prompt_text, workspace_key,
    SessionChangeRecord, SessionListParams,
};
use crate::contracts::{Session, SessionContextTokens, SessionStatus};
use crate::session::config::TuraSessionConfig;
use crate::session_store;
use crate::test_support::SessionDbTestService;
use axum::{
    extract::{Path, Query},
    http::HeaderMap,
    Json,
};
use std::fs;

async fn create_canonical_test_session(directory: String) -> Session {
    super::create_session_value(
        super::SessionDirectoryParams { directory: None },
        super::CreateSessionRequest {
            directory: Some(directory),
            session_type: Some("chat".to_string()),
            ..super::CreateSessionRequest::default()
        },
        None,
    )
    .await
    .expect("canonical test session should be created")
}

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
fn prompt_payload_extracts_documented_command_run_shell_surfaces() {
    let zsh = serde_json::json!({ "command_run_shell": "zsh" });
    let shel = serde_json::json!({ "command_run_shell": "shel" });
    let shell_command = serde_json::json!({ "commandRunShell": "shell_command" });
    let typo = serde_json::json!({ "command_run_shell": "zash" });

    assert_eq!(prompt_command_run_shell(&zsh).as_deref(), Some("zsh"));
    assert_eq!(
        prompt_command_run_shell(&shel).as_deref(),
        Some("shell_command")
    );
    assert_eq!(
        prompt_command_run_shell(&shell_command).as_deref(),
        Some("shell_command")
    );
    assert_eq!(prompt_command_run_shell(&typo), None);
}

#[test]
fn prompt_payload_treats_default_model_variant_as_unset() {
    let payload = serde_json::json!({
        "variant": " default ",
    });

    assert_eq!(prompt_model_variant(&payload), None);
}

#[test]
fn session_config_defaults_priority_routing_off() {
    assert_eq!(
        TuraSessionConfig::default().model_acceleration_enabled,
        Some(false)
    );
}

#[test]
fn session_config_model_override_keeps_tier_names_out_of_specific_model_display() {
    let config = TuraSessionConfig {
        model: Some("thinking".to_string()),
        active_provider: None,
        active_model: None,
        ..TuraSessionConfig::default()
    };

    assert_eq!(config_model_override(&config), None);
}

fn test_session(id: &str, directory: &str, parent_id: Option<&str>, updated_at: i64) -> Session {
    Session {
        id: id.to_string(),
        name: Some(id.to_string()),
        parent_id: parent_id.map(ToString::to_string),
        created_at: updated_at - 1,
        updated_at,
        last_user_message_at: None,
        task_start_at: Some(updated_at - 1),
        directory: Some(directory.to_string()),
        model: None,
        agent: None,
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
        context_tokens: SessionContextTokens::default(),
        usage: Default::default(),
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

    let filtered = filter_list_sessions(sessions, &SessionListParams::default(), Some("C:/repo"));

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

#[tokio::test]
async fn session_list_orders_by_latest_user_message_not_runtime_update() {
    let _service = SessionDbTestService::start();
    let directory = std::env::temp_dir()
        .join(format!(
            "tura-session-user-message-order-{}",
            uuid::Uuid::new_v4()
        ))
        .to_string_lossy()
        .to_string();

    let assistant_updated_later = create_canonical_test_session(directory.clone()).await;
    let user_sent_later = create_canonical_test_session(directory.clone()).await;

    super::update_session_task_management_value(
        assistant_updated_later.id.clone(),
        super::UpdateSessionTaskManagementRequest {
            task_management: serde_json::json!({
                "task_summary": "Assistant updated later",
                "start_at": "2026-06-25T12:00:00Z"
            }),
        },
    )
    .expect("first task management patch should succeed");
    super::update_session_task_management_value(
        user_sent_later.id.clone(),
        super::UpdateSessionTaskManagementRequest {
            task_management: serde_json::json!({
                "task_summary": "User sent later",
                "start_at": "2026-06-25T10:00:00Z"
            }),
        },
    )
    .expect("second task management patch should succeed");

    session_store().add_message(
        &assistant_updated_later.id,
        crate::session::store::MessageRole::User,
        "older user prompt".to_string(),
    );
    std::thread::sleep(std::time::Duration::from_millis(2));
    let newer_user_message = session_store()
        .add_message(
            &user_sent_later.id,
            crate::session::store::MessageRole::User,
            "newer user prompt".to_string(),
        )
        .expect("newer user message should be stored");
    std::thread::sleep(std::time::Duration::from_millis(2));
    session_store().add_message(
        &assistant_updated_later.id,
        crate::session::store::MessageRole::Assistant,
        "later assistant reply".to_string(),
    );

    let Json(listed) = super::list_sessions(
        HeaderMap::new(),
        Query(SessionListParams {
            directory: Some(directory.clone()),
            include_children: true,
            ..SessionListParams::default()
        }),
    )
    .await;

    assert_eq!(
        listed.first().map(|session| session.id.as_str()),
        Some(user_sent_later.id.as_str())
    );
    assert_eq!(
        listed
            .first()
            .and_then(|session| session.last_user_message_at),
        Some(newer_user_message.updated_at)
    );

    let _ = fs::remove_dir_all(directory);
}

#[test]
fn workspace_key_normalizes_slashes_and_trailing_separator() {
    assert_eq!(workspace_key(r"C:\repo\"), "C:/repo");
    assert_eq!(workspace_key("C:/"), "C:/");
    assert_eq!(workspace_key("///"), "/");
}

#[tokio::test]
async fn session_status_includes_task_management_display_fields() {
    let _service = SessionDbTestService::start();
    let directory = std::env::temp_dir()
        .join(format!("tura-session-status-{}", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let session = create_canonical_test_session(directory).await;
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
    assert_eq!(status["session_display_name"], "Status task");
}

#[tokio::test]
async fn create_session_accepts_task_management_and_serializes_session_fields() {
    let _service = SessionDbTestService::start();
    let directory = std::env::temp_dir()
        .join(format!("tura-create-session-plan-{}", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let session = super::create_session_value(
        super::SessionDirectoryParams { directory: None },
        super::CreateSessionRequest {
            directory: Some(directory.clone()),
            model: None,
            agent: None,
            session_type: Some("chat".to_string()),
            kill_processes_on_start: Some(false),
            validator_enabled: Some(false),
            force_planning: Some(false),
            model_variant: None,
            model_acceleration_enabled: Some(false),
            disable_permission_restrictions: Some(false),
            auto_session_name: None,
            task_management: Some(serde_json::json!({
                "plan_summary": "Create Route Plan",
                "task_summary": "Create route task"
            })),
        },
        None,
    )
    .await
    .expect("canonical session creation should succeed");

    assert_eq!(session.directory.as_deref(), Some(directory.as_str()));
    assert_eq!(session.plan_summary.as_deref(), Some("Create Route Plan"));
    assert_eq!(
        session.session_display_name.as_deref(),
        Some("Create route task")
    );
    assert_eq!(session.task_management["task_summary"], "Create route task");

    let value = serde_json::to_value(&session).expect("session should serialize");
    assert!(value["name"].as_str().is_some_and(|name| !name.is_empty()));
    assert!(value["task_management"].get("status").is_none());
    assert_eq!(value["task_management"]["start_condition"], "user_action");
    assert_eq!(value["plan_summary"], "Create Route Plan");
    assert_eq!(value["session_display_name"], "Create route task");
    assert_eq!(value["auto_session_name"], true);
    assert_eq!(value["context_tokens"]["input"], 0);
    assert!(value["context_tokens"]["limit"].as_u64().is_some());
    assert_eq!(value["usage"]["context_tokens"]["input"], 0);
    assert!(value["usage"]["context_tokens"]["limit"].as_u64().is_some());
    assert_eq!(
        value["last_user_message_at"].as_i64(),
        session.last_user_message_at
    );
    let object = value.as_object().expect("session JSON should be an object");
    assert_eq!(object.len(), 25);

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
    let _service = SessionDbTestService::start();
    let directory = std::env::temp_dir()
        .join(format!(
            "tura-task-management-route-{}",
            uuid::Uuid::new_v4()
        ))
        .to_string_lossy()
        .to_string();
    let session = create_canonical_test_session(directory.clone()).await;

    let updated = super::update_session_task_management_value(
        session.id.clone(),
        super::UpdateSessionTaskManagementRequest {
            task_management: serde_json::json!({
                "plan_summary": "Dedicated Patch Route",
                "task_summary": "Patch task",
                "status": "question",
                "start_at": "2026-05-25T08:30:00Z"
            }),
        },
    )
    .expect("task management patch should succeed");

    assert_eq!(
        updated.plan_summary.as_deref(),
        Some("Dedicated Patch Route")
    );
    assert_eq!(updated.session_display_name.as_deref(), Some("Patch task"));
    assert_eq!(updated.task_management["status"], "question");
    assert_eq!(updated.task_management["start_condition"], "scheduled_task");

    let value = serde_json::to_value(&updated).expect("session should serialize");
    assert_eq!(value["task_management"]["status"], "question");
    assert_eq!(
        value["task_management"]["start_condition"],
        "scheduled_task"
    );
    assert_eq!(value["plan_summary"], "Dedicated Patch Route");
    assert_eq!(value["session_display_name"], "Patch task");
    assert_eq!(value["auto_session_name"], true);
    assert_eq!(value["context_tokens"]["input"], 0);
    assert!(value["context_tokens"]["limit"].as_u64().is_some());
    assert_eq!(value["usage"]["context_tokens"]["input"], 0);
    assert!(value["usage"]["context_tokens"]["limit"].as_u64().is_some());
    assert_eq!(
        value["last_user_message_at"].as_i64(),
        updated.last_user_message_at
    );
    let object = value.as_object().expect("session JSON should be an object");
    assert_eq!(object.len(), 25);

    let Json(fetched) = super::get_session(Path(session.id)).await;
    assert_eq!(fetched.task_management["status"], "question");
    assert_eq!(fetched.task_management["start_condition"], "scheduled_task");

    let _ = fs::remove_dir_all(directory);
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

    let value =
        serde_json::to_value(api_message_from_store(message)).expect("message should serialize");

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
fn apply_single_change_reports_target_directory_context() {
    let temp = tempfile::tempdir().expect("tempdir");
    let blocking_parent = temp.path().join("blocked");
    std::fs::write(&blocking_parent, "file blocks child directory").expect("write blocking file");
    let target = blocking_parent.join("child.txt");
    let record = SessionChangeRecord {
        path: target.to_string_lossy().to_string(),
        before_exists: true,
        before_content: Some("before".to_string()),
        after_exists: true,
        after_content: None,
        reverted: false,
    };

    let error = apply_single_change(&record, true)
        .expect_err("blocked parent path should fail directory creation");

    let message = &error;
    assert!(
        message.contains("failed to create change target directory"),
        "error should describe the failed operation: {message}"
    );
    assert!(
        message.contains(&blocking_parent.to_string_lossy().to_string()),
        "error should include the target directory path: {message}"
    );
}

#[test]
fn inactive_sessions_from_probe_keeps_active_sessions() {
    let expected = vec![
        "active".to_string(),
        "queued".to_string(),
        "running".to_string(),
        "worker".to_string(),
    ];
    let inactive = inactive_sessions_from_probe(
        &expected,
        &serde_json::json!({
            "sessions": [
                { "session_id": "active", "status": "active" },
                { "session_id": "queued", "status": "queued" },
                { "session_id": "running", "status": "running" },
                { "session_id": "worker", "worker_alive": true }
            ]
        }),
    );

    assert!(inactive.is_empty());
}

#[test]
fn inactive_sessions_from_probe_marks_missing_or_inactive_sessions() {
    let expected = vec![
        "inactive".to_string(),
        "missing".to_string(),
        "active".to_string(),
    ];
    let inactive = inactive_sessions_from_probe(
        &expected,
        &serde_json::json!({
            "sessions": [
                { "session_id": "inactive", "status": "inactive" },
                { "session_id": "active", "active_turn": true }
            ]
        }),
    );

    assert_eq!(
        inactive,
        vec!["inactive".to_string(), "missing".to_string()]
    );
}
