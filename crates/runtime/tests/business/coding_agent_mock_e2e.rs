use std::sync::atomic::Ordering;

use runtime::mano;
use runtime::state_machine::session_management::{SessionInput, SessionState};
use serde_json::Value;

#[path = "../support/session_db_support.rs"]
mod session_db_support;

#[path = "helpers/coding_agent_mock.rs"]
mod helpers;
use helpers::*;
#[test]
fn coding_agent_can_call_command_run_tool_e2e() {
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_command_run();
    let llm_config = write_llm_config(&workspace, provider.addr);
    let router_addr = mock_command_run_router_addr();
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        ("OPENAI_API_KEY", "test-key"),
        ("TURA_ROUTER_ADDR", router_addr.as_str()),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "5"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-run-command-tool".to_string(),
        SessionInput {
            user_input: "Run pwd with command_run, then patch src/lib.rs with command_run apply_patch, verify it with shell_command, and finish with normal assistant text."
                .to_string(),
            file_input: vec![],
            agent: Some("direct".to_string()),
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace.clone(),
    )
    .expect("coding agent should complete the command_run e2e flow");

    assert_eq!(result.agents.len(), 1);
    assert_eq!(result.agents[0].agent_name, "direct");
    assert_eq!(
        result.session.state,
        SessionState::Completed,
        "final_error={:?}; session log: {:#?}",
        result.final_error,
        result.session.session_log
    );

    let tool_results = tool_results(&result.session.session_log);
    assert_tool_success(&tool_results, "command_run");
    assert!(!tool_results
        .iter()
        .any(|result| result.get("tool_name").and_then(Value::as_str)
            == Some("send_message_to_user")));
    assert!(result
        .session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .any(|entry| entry.get("role").and_then(Value::as_str) == Some("assistant")));

    let run_output = tool_results
        .iter()
        .find(|result| result.get("tool_name").and_then(Value::as_str) == Some("command_run"))
        .and_then(|result| result.get("output"))
        .cloned()
        .unwrap_or(Value::Null);
    let first_command_output = run_output
        .pointer("/results/0/output")
        .expect("first command_run result should expose structured output");
    assert_eq!(first_command_output["exit_code"].as_i64(), Some(0));
    assert!(first_command_output["stdout"]
        .as_str()
        .is_some_and(|stdout| !stdout.trim().is_empty()));
    assert_eq!(first_command_output["stderr"].as_str(), Some(""));
    assert!(run_output.pointer("/results/0/exit_code").is_none());
    assert!(run_output.pointer("/results/0/display_command").is_none());

    let patched_content = std::fs::read_to_string(workspace.join("src/lib.rs"))
        .expect("patched file should be readable");
    assert!(
        patched_content.contains("processed verified {input}"),
        "patched file should contain verified output; tool_results={tool_results:#?}"
    );

    let requests = provider
        .requests
        .lock()
        .expect("mock provider requests lock");
    let first_tools = requests
        .iter()
        .find(|request| request.get("tools").and_then(Value::as_array).is_some())
        .and_then(|request| request.get("tools"))
        .and_then(Value::as_array)
        .expect("at least one provider request should include tools");
    let first_tool_names = first_tools
        .iter()
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .or_else(|| tool.get("name"))
                .and_then(Value::as_str)
        })
        .collect::<Vec<_>>();
    assert!(first_tool_names.contains(&"command_run"));
}

#[test]
fn coding_agent_executes_command_run_command_before_stream_finishes() {
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_codex_streaming_probe(workspace.clone());
    let llm_config = write_codex_llm_config(&workspace);
    let endpoint = format!("http://{}", provider.addr);
    let router_addr = mock_command_run_router_addr();
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        ("OPENAI_LOGIN", "oauth"),
        ("OPENAI_API_KEY", "test-key"),
        ("OPENAI_TOKEN_EXPIRES", MOCK_OPENAI_TOKEN_EXPIRES),
        ("OPENAI_CODEX_ENDPOINT", endpoint.as_str()),
        ("TURA_ROUTER_ADDR", router_addr.as_str()),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "2"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_MULTI_COMMAND_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_MULTI_COMMAND_STREAM_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-stream-command-before-message-done".to_string(),
        SessionInput {
            user_input: "Use command_run in this code file workspace to create streamed-first.txt, then create streamed-second.txt."
                .to_string(),
            file_input: vec![],
            agent: Some("direct".to_string()),
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace.clone(),
    )
    .expect("coding agent should complete the streaming command_run e2e flow");

    assert!(
        provider
            .first_command_observed_before_response_finished
            .load(Ordering::SeqCst),
        "first streamed command did not execute before the provider finished sending the response; requests={:#?}; first_exists={}; second_exists={}",
        provider.requests.lock().expect("mock provider requests lock"),
        workspace.join("streamed-first.txt").exists(),
        workspace.join("streamed-second.txt").exists()
    );
    assert_eq!(
        result.session.state,
        SessionState::Completed,
        "final_error={:?}; session log: {:#?}",
        result.final_error,
        result.session.session_log
    );
    assert!(
        workspace.join("streamed-first.txt").exists(),
        "first streamed command should create streamed-first.txt"
    );
    assert!(
        workspace.join("streamed-second.txt").exists(),
        "second streamed command should create streamed-second.txt"
    );
}

#[test]
fn streamed_single_task_status_is_backfilled_when_final_response_lacks_tool_call() {
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_codex_streaming_single_task_status_missing_final_tool_call();
    let llm_config = write_codex_llm_config(&workspace);
    let endpoint = format!("http://{}", provider.addr);
    let router_addr = mock_command_run_router_addr();
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        ("OPENAI_LOGIN", "oauth"),
        ("OPENAI_API_KEY", "test-key"),
        ("OPENAI_TOKEN_EXPIRES", MOCK_OPENAI_TOKEN_EXPIRES),
        ("OPENAI_CODEX_ENDPOINT", endpoint.as_str()),
        ("TURA_ROUTER_ADDR", router_addr.as_str()),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "3"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-stream-single-task-status-backfill".to_string(),
        SessionInput {
            user_input: "Create a GUI avatar effect and continue after task status setup."
                .to_string(),
            file_input: vec![],
            agent: Some("direct".to_string()),
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace,
    )
    .expect("streamed task_status-only first turn should continue to a follow-up provider turn");

    assert_eq!(
        result.session.state,
        SessionState::Completed,
        "final_error={:?}; session log: {:#?}",
        result.final_error,
        result.session.session_log
    );
    assert_eq!(
        result.session.task_type,
        vec!["visual".to_string(), "frontend".to_string()]
    );
    let requests = provider
        .requests
        .lock()
        .expect("mock provider requests lock");
    assert!(
        requests.len() >= 2,
        "streamed task_status-only first turn should not be lost before backfill; requests={requests:#?}"
    );
    let second_request = requests
        .get(1)
        .expect("second provider request should exist");
    let serialized = second_request.to_string();
    assert!(
        serialized.contains("function_call_output"),
        "second provider request should replay streamed command_run output: {second_request:#?}"
    );
    assert!(
        serialized.contains("call_stream_task_status_only"),
        "second provider request should bind output to the streamed provider call id: {second_request:#?}"
    );
    assert!(
        serialized.contains("GUI avatar effect") && serialized.contains("task_status"),
        "second provider request should include the streamed task_status result: {second_request:#?}"
    );
    assert!(
        serialized.contains("Frontend Operation Manual")
            && serialized.contains("Visual Operation Manual"),
        "second provider request should include manuals activated by streamed task_type: {second_request:#?}"
    );
}

#[test]
fn non_planning_agent_visible_reply_with_task_status_doing_is_backfilled_on_followup_turn() {
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_task_status_doing_with_visible_reply();
    let llm_config = write_llm_config(&workspace, provider.addr);
    let router_addr = mock_command_run_router_addr();
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        ("OPENAI_API_KEY", "test-key"),
        ("TURA_ROUTER_ADDR", router_addr.as_str()),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "4"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-nonplanning-doing-visible-reply".to_string(),
        SessionInput {
            user_input: "Answer directly, mark the task status, and stop.".to_string(),
            file_input: vec![],
            agent: Some("direct".to_string()),
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace,
    )
    .expect("non-planning task_status doing session should backfill before completion");

    assert_eq!(
        result.session.state,
        SessionState::Completed,
        "final_error={:?}; session log: {:#?}",
        result.final_error,
        result.session.session_log
    );
    let requests = provider
        .requests
        .lock()
        .expect("mock provider requests lock");
    assert!(
        requests.len() >= 2,
        "single doing task_status must be backfilled before the runtime loop can end; requests={requests:#?}"
    );
    let second_request = requests
        .get(1)
        .expect("second provider request should exist");
    let serialized = second_request.to_string();
    assert!(
        serialized.contains("call_task_status_doing") && serialized.contains("task_status"),
        "second provider request should replay the doing task_status output: {second_request:#?}"
    );
    assert!(result
        .session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .any(
            |entry| entry.get("role").and_then(Value::as_str) == Some("assistant")
                && entry
                    .get("content")
                    .and_then(Value::as_str)
                    .is_some_and(|content| content.contains("Done."))
        ));
    assert_eq!(
        provider
            .requests
            .lock()
            .expect("mock provider requests lock")
            .len(),
        1,
        "runtime must not do a second LLM turn just to recover from stale task_status doing"
    );
}

#[test]
fn task_status_only_first_turn_is_backfilled_with_manuals_on_followup_turn() {
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_task_status_only_then_final();
    let llm_config = write_llm_config(&workspace, provider.addr);
    let router_addr = mock_command_run_router_addr();
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        ("OPENAI_API_KEY", "test-key"),
        ("TURA_ROUTER_ADDR", router_addr.as_str()),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "4"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-task-status-only-backfill".to_string(),
        SessionInput {
            user_input: "Create a GUI avatar effect and continue after task status setup."
                .to_string(),
            file_input: vec![],
            agent: Some("direct".to_string()),
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace,
    )
    .expect("task_status-only first turn should continue to a follow-up provider turn");

    assert_eq!(
        result.session.state,
        SessionState::Completed,
        "final_error={:?}; session log: {:#?}",
        result.final_error,
        result.session.session_log
    );
    assert_eq!(
        result.session.task_type,
        vec!["visual".to_string(), "frontend".to_string()]
    );
    let requests = provider
        .requests
        .lock()
        .expect("mock provider requests lock");
    assert!(
        requests.len() >= 2,
        "task_status-only first turn should not end the runtime loop before backfill; requests={requests:#?}"
    );
    let second_request = requests
        .get(1)
        .expect("second provider request should exist");
    let serialized = second_request.to_string();
    assert!(
        serialized.contains("function_call_output"),
        "second provider request should replay command_run function output: {second_request:#?}"
    );
    assert!(
        serialized.contains("call_task_status_only"),
        "second provider request should bind the tool output to the original provider call id: {second_request:#?}"
    );
    assert!(
        serialized.contains("GUI avatar effect") && serialized.contains("task_status"),
        "second provider request should include the task_status result content: {second_request:#?}"
    );
    assert!(
        serialized.contains("Frontend Operation Manual")
            && serialized.contains("Visual Operation Manual"),
        "second provider request should include manuals activated by task_type: {second_request:#?}"
    );
}

#[test]
fn single_done_task_status_with_long_visible_reply_completes_without_backfill_turn() {
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_task_status_done_with_long_visible_reply();
    let llm_config = write_llm_config(&workspace, provider.addr);
    let router_addr = mock_command_run_router_addr();
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        ("OPENAI_API_KEY", "test-key"),
        ("TURA_ROUTER_ADDR", router_addr.as_str()),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "4"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-single-done-task-status-long-reply".to_string(),
        SessionInput {
            user_input: "Finish with a long assistant response and a done task status.".to_string(),
            file_input: vec![],
            agent: Some("direct".to_string()),
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace,
    )
    .expect("single done task_status with a long visible reply should complete");

    assert_eq!(result.session.state, SessionState::Completed);
    assert_eq!(
        result
            .session
            .task_plan
            .detailed_tasks
            .first()
            .map(|task| task.status),
        Some(runtime::state_machine::session_management::PlanStatus::Done),
        "done task_status should still update task state; log={:#?}",
        result.session.session_log
    );
    assert!(result
        .session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .any(
            |entry| entry.get("role").and_then(Value::as_str) == Some("assistant")
                && entry
                    .get("content")
                    .and_then(Value::as_str)
                    .is_some_and(|content| content.len() > 1_000)
        ));
    assert_eq!(
        provider
            .requests
            .lock()
            .expect("mock provider requests lock")
            .len(),
        1,
        "runtime must not send the single done task_status tool result back to the provider when the assistant message already exceeds 1000 characters"
    );
}

#[test]
fn single_done_task_status_with_short_visible_reply_is_backfilled() {
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_task_status_done_with_short_visible_reply();
    let llm_config = write_llm_config(&workspace, provider.addr);
    let router_addr = mock_command_run_router_addr();
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        ("OPENAI_API_KEY", "test-key"),
        ("TURA_ROUTER_ADDR", router_addr.as_str()),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "4"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-single-done-task-status-short-reply".to_string(),
        SessionInput {
            user_input: "Finish with a short assistant response and a done task status."
                .to_string(),
            file_input: vec![],
            agent: Some("direct".to_string()),
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace,
    )
    .expect("single done task_status with a short visible reply should backfill");

    assert_eq!(result.session.state, SessionState::Completed);
    let requests = provider
        .requests
        .lock()
        .expect("mock provider requests lock");
    assert!(
        requests.len() >= 2,
        "runtime must send a short single done task_status result back to the provider before ending; requests={requests:#?}"
    );
    let second_request = requests
        .get(1)
        .expect("second provider request should exist");
    let serialized = second_request.to_string();
    assert!(
        serialized.contains("function_call_output"),
        "second provider request should replay command_run function output: {second_request:#?}"
    );
    assert!(
        serialized.contains("call_task_status_done_short"),
        "second provider request should bind the tool output to the original provider call id: {second_request:#?}"
    );
}

#[test]
fn coding_agent_provider_retry_exhaustion_preserves_provider_error() {
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_rate_limit();
    let llm_config = write_llm_config(&workspace, provider.addr);
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        ("OPENAI_API_KEY", "test-key"),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "6"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_RETRY_BACKOFF_MS", "0,0,0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-provider-retry-exhausted".to_string(),
        SessionInput {
            user_input: "Trigger a provider rate limit and report the real error.".to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace,
    )
    .expect("provider failures should be captured in the session result");

    assert_eq!(result.session.state, SessionState::Failed);
    let final_error = result
        .final_error
        .as_deref()
        .expect("final provider error should be preserved");
    assert!(
        final_error.contains("rate_limit_exceeded"),
        "provider error should survive retries; got {final_error}"
    );
    assert!(
        final_error.contains("Provider runtime failed after 3 retries"),
        "retry exhaustion context should be visible; got {final_error}"
    );
    let request_count = provider
        .requests
        .lock()
        .expect("mock provider requests lock")
        .len();
    assert_eq!(
        request_count, 4,
        "initial provider call plus three retries should be attempted"
    );
}
