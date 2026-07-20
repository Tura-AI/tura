use chrono::Utc;
use lifecycle::SessionState;
use runtime::context::{
    accumulate_message, accumulate_tool_result, accumulate_tool_result_with_provider_metadata,
    build_context, build_messages_from_session, user_input_content_matches,
    user_input_content_value, ContextInput,
};
use runtime::state_machine::agent_management::{ProviderConfig, ToolChoice};
use runtime::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
use runtime::state_machine::session_management::{SessionInput, SessionManagement};
use serde_json::{json, Value};
use std::path::PathBuf;

fn business_session(user_input: &str) -> SessionManagement {
    let now = Utc::now();
    SessionManagement::new(
        "session-runtime-context-business".to_string(),
        "Context business flow".to_string(),
        PathBuf::from("C:/workspace/runtime-context-business"),
        false,
        "coding".to_string(),
        SessionInput {
            user_input: user_input.to_string(),
            file_input: Vec::new(),
            agent: Some("context-agent".to_string()),
            runtime_context: Some("{\"source\":\"business-test\"}".to_string()),
            planning_mode_override: None,
        },
        user_input.to_string(),
        now,
    )
}

fn business_runtime(session: &SessionManagement) -> RuntimeManagement {
    let provider_name = runtime::agent_router::coding_agent_provider_name();
    RuntimeManagement::new(
        "runtime-context-business".to_string(),
        session.session_id.clone(),
        "agent-context-business".to_string(),
        RuntimeProviderConfig {
            base: ProviderConfig {
                tura_llm_name: provider_name.clone(),
                default_model_tier: None,
                current_model: None,
                stream: true,
                temperature: 0.0,
                max_tokens: 4096,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 120_000,
            },
            thinking: false,
            provider_name: provider_name.clone(),
            model_name: "local-context-model".to_string(),
            provider_url_name: "http://127.0.0.1:1/v1".to_string(),
            llm_provider_name: provider_name,
        },
        Utc::now(),
    )
}

fn message_texts(messages: &[Value]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| message.get("content"))
        .map(|content| {
            content
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| content.to_string())
        })
        .collect()
}

#[test]
fn context_business_flow_injects_initial_user_once_and_preserves_dialog_order() {
    let mut session = business_session("inspect the workspace");
    accumulate_message(&mut session, "system", json!("system guardrail"))
        .expect("system message should log");
    accumulate_message(&mut session, "user", json!("inspect the workspace"))
        .expect("matching user message should log once");
    accumulate_message(&mut session, "assistant", json!("I will inspect it"))
        .expect("assistant message should log");

    let output = build_context(ContextInput {
        runtime: business_runtime(&session),
        session,
        additional_messages: vec![json!({"role": "system", "content": "late instruction"})],
    })
    .expect("context should build");

    let texts = message_texts(&output.messages);
    assert_eq!(
        texts
            .iter()
            .filter(|text| text.as_str() == "inspect the workspace")
            .count(),
        1,
        "initial user input must not be duplicated when history already has it: {texts:?}"
    );
    assert_eq!(texts[0], "system guardrail");
    assert!(texts.iter().any(|text| text == "I will inspect it"));
    assert_eq!(texts.last().map(String::as_str), Some("late instruction"));
    assert_eq!(output.context_state.messages, output.messages);
    assert_eq!(
        output.session.state,
        SessionState::Created,
        "context building must not mutate the session FSM"
    );
}

#[test]
fn context_business_flow_converts_image_markers_into_structured_user_content() {
    let input = "look at [MEDIA:data:image/png;base64,AAAA:MEDIA] and summarize";
    let session = business_session(input);

    let messages = build_messages_from_session(&session);
    let content = &messages[0]["content"];

    assert!(
        content.is_array(),
        "image marker should become content parts: {content}"
    );
    assert!(user_input_content_matches(content, input));
    assert_eq!(content[0]["type"], "input_text");
    assert_eq!(content[1]["type"], "input_image");
    assert_eq!(content[1]["image_url"], "data:image/png;base64,AAAA");
    assert_eq!(content[2]["type"], "input_text");
    assert_eq!(user_input_content_value("plain text"), json!("plain text"));
}

#[test]
fn context_business_flow_keeps_invalid_media_markers_as_plain_user_text() {
    let input = "do not parse [MEDIA:file:///tmp/image.png:MEDIA] as provider image";
    let session = business_session(input);
    let messages = build_messages_from_session(&session);

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], input);
    assert!(user_input_content_matches(&messages[0]["content"], input));
}

#[test]
fn context_business_flow_accumulates_tool_result_with_metadata_and_strips_reporting_fields() {
    let mut session = business_session("run the verifier");
    accumulate_tool_result_with_provider_metadata(
        &mut session,
        "command_run",
        json!({
            "commands": [{
                "command": "shell_command",
                "command_line": "node verify.mjs",
                "status": "model-only-status",
                "summary": "model-only-summary",
                "success": false
            }]
        }),
        json!({
            "results": [{
                "step": 1,
                "command_type": "shell_command",
                "success": true,
                "output": "VERIFIER_OK"
            }]
        }),
        true,
        None,
        Some("runtime-context-1"),
        Some(json!({
            "id": "call_context_1",
            "provider": "local",
            "request_id": "req-context-1"
        })),
    )
    .expect("tool result should log");

    let messages = build_messages_from_session(&session);
    let serialized = messages
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(serialized.contains("VERIFIER_OK"), "{serialized}");
    assert!(serialized.contains("command_run"), "{serialized}");
    assert!(serialized.contains("model-only-status"), "{serialized}");
    assert!(!serialized.contains("model-only-summary"), "{serialized}");
    assert!(!serialized.contains("\"success\":false"), "{serialized}");

    let raw_log = session.session_log.join("\n");
    assert!(
        raw_log.contains("\"runtime_id\":\"runtime-context-1\""),
        "{raw_log}"
    );

    let stored = session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .find(|entry| entry.get("type").and_then(Value::as_str) == Some("tool_result"))
        .expect("tool result should be stored");
    assert_eq!(stored["sequence"], 1);
    assert_eq!(stored["provider_metadata"]["request_id"], "req-context-1");
    assert!(stored.get("context_cache").is_some());
    assert!(stored.get("context_messages").is_some());
}

#[test]
fn context_business_flow_tracks_tool_result_sequence_and_last_response() {
    let mut session = business_session("run two tools");
    session.use_last_tool_call_response = true;
    accumulate_tool_result(
        &mut session,
        "command_run",
        json!({"commands": [{"command": "task_status", "command_line": "{\"status\":\"done\"}"}]}),
        json!({"results": [{"command_type": "task_status", "success": true, "output": {"task_status": {"status": "done"}}}]}),
        true,
        None,
    )
    .expect("first tool result");
    accumulate_tool_result(
        &mut session,
        "command_run",
        json!({"commands": [{"command": "shell_command", "command_line": "echo second"}]}),
        json!({"results": [{"command_type": "shell_command", "success": false, "error": "boom"}]}),
        false,
        Some("boom".to_string()),
    )
    .expect("second tool result");

    let output = build_context(ContextInput {
        runtime: business_runtime(&session),
        session,
        additional_messages: Vec::new(),
    })
    .expect("context should build");

    assert_eq!(output.context_state.tool_results.len(), 0);
    let stored_tool_results = output
        .session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("tool_result"))
        .collect::<Vec<_>>();
    assert_eq!(stored_tool_results[0]["sequence"], 1);
    assert_eq!(stored_tool_results[1]["sequence"], 2);
    assert!(
        output.context_state.last_tool_call_response.is_some(),
        "last tool response should be cached when enabled"
    );
    let joined = output
        .messages
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("boom"), "{joined}");
    assert!(joined.contains("task_status"), "{joined}");
}

#[test]
fn context_business_flow_uses_runtime_text_and_reasoning_when_session_history_is_empty() {
    let session = business_session("");
    let mut runtime = business_runtime(&session);
    runtime.text = "assistant fallback answer".to_string();
    runtime.reasoning = Some("private reasoning summary".to_string());

    let output = build_context(ContextInput {
        runtime,
        session,
        additional_messages: Vec::new(),
    })
    .expect("context should build");

    assert_eq!(output.context_state.reasoning_history.len(), 1);
    assert_eq!(output.messages[0]["role"], "system");
    assert_eq!(output.messages[0]["type"], "reasoning");
    assert_eq!(output.messages[0]["content"], "private reasoning summary");
    assert_eq!(output.messages[1]["role"], "assistant");
    assert_eq!(output.messages[1]["content"], "assistant fallback answer");
}

#[test]
fn context_business_flow_keeps_runtime_reasoning_out_of_non_empty_messages_but_records_state() {
    let mut session = business_session("continue");
    accumulate_message(&mut session, "assistant", json!("previous response"))
        .expect("assistant history");
    let mut runtime = business_runtime(&session);
    runtime.text = "new answer should not be injected over existing history".to_string();
    runtime.reasoning = Some("reasoning retained for state only".to_string());

    let output = build_context(ContextInput {
        runtime,
        session,
        additional_messages: Vec::new(),
    })
    .expect("context should build");

    let joined = output
        .messages
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("previous response"), "{joined}");
    assert!(
        !joined.contains("new answer should not be injected"),
        "{joined}"
    );
    assert!(
        !joined.contains("reasoning retained for state only"),
        "{joined}"
    );
    assert_eq!(
        output.context_state.reasoning_history,
        vec!["reasoning retained for state only".to_string()]
    );
}
