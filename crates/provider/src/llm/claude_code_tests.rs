use super::*;
use std::sync::{Arc, Mutex};

#[test]
fn oauth_token_detected_by_prefix() {
    assert!(is_oauth_subscription_token("sk-ant-oat01-abc"));
    assert!(!is_oauth_subscription_token("sk-ant-api03-abc"));
}

#[test]
fn cache_read_tokens_set_cache_hit_flag() {
    let data = json!({
        "content": [{ "type": "text", "text": "ok" }],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 5,
            "output_tokens": 2,
            "cache_read_input_tokens": 4096,
            "cache_creation_input_tokens": 0
        }
    });
    let metrics = extract_metrics(&data);
    assert!(metrics.cache_hit);
    assert_eq!(metrics.cache_triggered_at_input_tokens, Some(4096));
    assert_eq!(metrics.usage.cached_input_tokens, Some(4096));
}

#[test]
fn no_cache_read_leaves_cache_hit_false() {
    let data = json!({
        "content": [{ "type": "text", "text": "ok" }],
        "usage": { "input_tokens": 5, "output_tokens": 2 }
    });
    let metrics = extract_metrics(&data);
    assert!(!metrics.cache_hit);
    assert_eq!(metrics.cache_triggered_at_input_tokens, None);
}

#[test]
fn anthropic_stream_collects_text_and_usage() {
    let mut state = AnthropicStreamState::default();
    process_anthropic_sse_line(
            r#"data: {"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","usage":{"input_tokens":3}}}"#,
            &mut state,
            None,
        )
        .expect("process message_start");
    process_anthropic_sse_line(
            r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            &mut state,
            None,
        )
        .expect("process content_block_start");
    process_anthropic_sse_line(
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hello"}}"#,
            &mut state,
            None,
        )
        .expect("process text_delta");
    process_anthropic_sse_line(
            r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}"#,
            &mut state,
            None,
        )
        .expect("process message_delta");

    let data = state.into_message();
    assert_eq!(
        normalize_response_content(&data),
        Value::String("hello".to_string())
    );
    let metrics = extract_metrics(&data);
    assert_eq!(metrics.usage.input_tokens, Some(3));
    assert_eq!(metrics.usage.output_tokens, Some(2));
    assert_eq!(metrics.finish_reason.as_deref(), Some("end_turn"));
}

#[test]
fn anthropic_stream_emits_command_run_when_tool_block_stops() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&events);
    let sink: ProviderStreamEventSink = Arc::new(move |event| {
        captured.lock().expect("capture stream event").push(event);
    });
    let mut state = AnthropicStreamState::default();

    process_anthropic_sse_line(
            r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"command_run","input":{}}}"#,
            &mut state,
            Some(&sink),
        )
        .expect("process tool block start");
    process_anthropic_sse_line(
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"commands\":[{\"command_type\":\"exec\",\"command_line\":\""}}"#,
            &mut state,
            Some(&sink),
        )
        .expect("process first tool input delta");
    process_anthropic_sse_line(
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"echo ok\"}]}"}}"#,
            &mut state,
            Some(&sink),
        )
        .expect("process second tool input delta");
    process_anthropic_sse_line(
        r#"data: {"type":"content_block_stop","index":0}"#,
        &mut state,
        Some(&sink),
    )
    .expect("process tool block stop");

    let captured = events.lock().expect("captured stream events");
    assert!(matches!(
        captured.first(),
        Some(ProviderStreamEvent::ProviderOutputStarted)
    ));
    let ready = captured
        .iter()
        .find_map(|event| match event {
            ProviderStreamEvent::CommandRunCommandReady {
                tool_call_id,
                command_index,
                command,
            } => Some((tool_call_id, command_index, command)),
            ProviderStreamEvent::ProviderOutputStarted => None,
        })
        .expect("command_run ready event");
    assert_eq!(ready.0, "toolu_1");
    assert_eq!(*ready.1, 0);
    assert_eq!(ready.2["command_line"], "echo ok");

    let data = state.into_message();
    assert_eq!(
        data["content"][0]["input"]["commands"][0]["command_line"],
        "echo ok"
    );
    let content = normalize_response_content(&data);
    assert_eq!(content["tool_calls"][0]["function"]["name"], "command_run");
}

#[test]
fn oauth_route_prepends_claude_code_system_prompt() {
    let messages = vec![json!({ "role": "user", "content": "hi" })];
    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), true);
    // System is emitted as typed blocks; the prefix block carries the
    // prompt-cache breakpoint required to avoid OAuth 429s on big prompts.
    let blocks = payload["system"].as_array().expect("system blocks");
    assert_eq!(blocks[0]["text"], CLAUDE_CODE_SYSTEM_PROMPT);
    assert_eq!(blocks[0]["cache_control"]["type"], "ephemeral");
}

#[test]
fn api_route_does_not_force_system_prompt() {
    let messages = vec![json!({ "role": "user", "content": "hi" })];
    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), false);
    assert!(payload.get("system").is_none());
    assert_eq!(payload["messages"][0]["role"], "user");
}

#[test]
fn system_messages_merge_into_system_string() {
    let messages = vec![
        json!({ "role": "system", "content": "Be terse." }),
        json!({ "role": "user", "content": "hi" }),
    ];
    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), true);
    let blocks = payload["system"].as_array().expect("system blocks");
    assert_eq!(blocks[0]["text"], CLAUDE_CODE_SYSTEM_PROMPT);
    assert_eq!(blocks[0]["cache_control"]["type"], "ephemeral");
    let merged: String = blocks
        .iter()
        .map(|b| b["text"].as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n\n");
    assert!(merged.contains("Be terse."));
    assert_eq!(payload["messages"].as_array().expect("messages").len(), 1);
}

#[test]
fn native_anthropic_system_blocks_keep_position_and_cache_control() {
    let messages = vec![
        json!({
            "role": "system",
            "content": [{
                "type": "text",
                "text": "Native cached instructions",
                "cache_control": {"type": "ephemeral"}
            }]
        }),
        json!({ "role": "user", "content": "hi" }),
    ];

    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), false);
    let blocks = payload["system"].as_array().expect("system blocks");

    assert_eq!(blocks[0]["type"], "text");
    assert_eq!(blocks[0]["text"], "Native cached instructions");
    assert_eq!(blocks[0]["cache_control"]["type"], "ephemeral");
    assert_eq!(payload["messages"][0]["role"], "user");
}

#[test]
fn assistant_tool_calls_become_tool_use_blocks() {
    let messages = vec![json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": "call_1",
            "type": "function",
            "function": { "name": "grep", "arguments": "{\"pattern\":\"foo\"}" }
        }]
    })];
    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), true);
    let block = &payload["messages"][0]["content"][0];
    assert_eq!(block["type"], "tool_use");
    assert_eq!(block["id"], "call_1");
    assert_eq!(block["name"], "grep");
    assert_eq!(block["input"]["pattern"], "foo");
}

#[test]
fn tool_role_message_becomes_tool_result_block() {
    let messages = vec![json!({
        "role": "tool",
        "tool_call_id": "call_1",
        "content": "result text"
    })];
    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), true);
    let block = &payload["messages"][0]["content"][0];
    assert_eq!(payload["messages"][0]["role"], "user");
    assert_eq!(block["type"], "tool_result");
    assert_eq!(block["tool_use_id"], "call_1");
    assert_eq!(block["content"], "result text");
}

#[test]
fn responses_function_items_convert_to_blocks() {
    let messages = vec![
        json!({ "type": "function_call", "call_id": "c1", "name": "ls", "arguments": "{\"path\":\".\"}" }),
        json!({ "type": "function_call_output", "call_id": "c1", "output": "a\nb" }),
    ];
    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), true);
    assert_eq!(payload["messages"][0]["role"], "assistant");
    assert_eq!(payload["messages"][0]["content"][0]["type"], "tool_use");
    assert_eq!(payload["messages"][0]["content"][0]["input"]["path"], ".");
    assert_eq!(payload["messages"][1]["role"], "user");
    assert_eq!(payload["messages"][1]["content"][0]["type"], "tool_result");
    assert_eq!(payload["messages"][1]["content"][0]["content"], "a\nb");
}

#[test]
fn responses_function_output_media_converts_to_anthropic_image_block() {
    let messages = vec![json!({
        "type": "function_call_output",
        "call_id": "c_media",
        "output": [
            { "type": "input_text", "text": "read_media returned image" },
            { "type": "input_image", "image_url": "data:image/jpeg;base64,AAA" }
        ]
    })];

    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), true);
    let content = &payload["messages"][0]["content"][0]["content"];

    assert_eq!(payload["messages"][0]["role"], "user");
    assert_eq!(payload["messages"][0]["content"][0]["type"], "tool_result");
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["media_type"], "image/jpeg");
    assert_eq!(content[1]["source"]["data"], "AAA");
}

#[test]
fn adjacent_same_role_messages_merge() {
    let messages = vec![
        json!({ "role": "user", "content": "one" }),
        json!({ "role": "user", "content": "two" }),
    ];
    let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), true);
    assert_eq!(payload["messages"].as_array().expect("messages").len(), 1);
    assert_eq!(
        payload["messages"][0]["content"]
            .as_array()
            .expect("message content")
            .len(),
        2
    );
}

#[test]
fn tools_and_tool_choice_convert() {
    let options = CallOptions {
        tools: Some(vec![json!({
            "type": "function",
            "function": { "name": "grep", "description": "search", "parameters": {"type":"object"} }
        })]),
        tool_choice: Some(json!({ "type": "function", "function": { "name": "grep" } })),
        ..Default::default()
    };
    let messages = vec![json!({ "role": "user", "content": "hi" })];
    let payload = build_payload("claude-opus-4-8", &messages, &options, true);
    assert_eq!(payload["tools"][0]["name"], "grep");
    assert_eq!(payload["tools"][0]["input_schema"]["type"], "object");
    assert_eq!(payload["tool_choice"]["type"], "tool");
    assert_eq!(payload["tool_choice"]["name"], "grep");
}

#[test]
fn extra_body_preserves_native_claude_code_request_fields() {
    let options = CallOptions {
        extra_body: Some(json!({
            "betas": ["context-management-2025-06-27"],
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "output_config": {"effort": "medium"},
            "speed": "fast"
        })),
        ..Default::default()
    };
    let messages = vec![json!({ "role": "user", "content": "hi" })];

    let payload = build_payload("claude-opus-4-8", &messages, &options, true);

    assert_eq!(payload["betas"][0], "context-management-2025-06-27");
    assert_eq!(payload["context_management"]["edits"][0]["keep"], "all");
    assert_eq!(payload["output_config"]["effort"], "medium");
    assert_eq!(payload["speed"], "fast");
}

#[test]
fn thinking_enabled_omits_temperature() {
    let options = CallOptions {
        reasoning_effort: Some("low".to_string()),
        temperature: Some(0.0),
        max_tokens: Some(8192),
        ..Default::default()
    };
    let messages = vec![json!({ "role": "user", "content": "hi" })];
    let payload = build_payload("claude-opus-4-8", &messages, &options, true);
    assert_eq!(payload["thinking"]["type"], "enabled");
    assert_eq!(payload["thinking"]["budget_tokens"], 1024);
    assert!(payload.get("temperature").is_none());
}

#[test]
fn thinking_skipped_when_budget_exceeds_max_tokens() {
    let options = CallOptions {
        reasoning_effort: Some("high".to_string()),
        max_tokens: Some(2048),
        ..Default::default()
    };
    let messages = vec![json!({ "role": "user", "content": "hi" })];
    let payload = build_payload("claude-opus-4-8", &messages, &options, true);
    assert!(payload.get("thinking").is_none());
}

#[test]
fn response_text_only_returns_string() {
    let data = json!({
        "content": [{ "type": "text", "text": "hello" }],
        "stop_reason": "end_turn",
        "usage": { "input_tokens": 3, "output_tokens": 1 }
    });
    let content = normalize_response_content(&data);
    assert_eq!(content, Value::String("hello".to_string()));
    let metrics = extract_metrics(&data);
    assert_eq!(metrics.usage.input_tokens, Some(3));
    assert_eq!(metrics.tool_call_count, 0);
    assert_eq!(metrics.finish_reason.as_deref(), Some("end_turn"));
}

#[test]
fn response_tool_use_returns_openai_tool_calls() {
    let data = json!({
        "content": [
            { "type": "text", "text": "let me check" },
            { "type": "tool_use", "id": "tu_1", "name": "grep", "input": { "pattern": "x" } }
        ],
        "stop_reason": "tool_use",
        "usage": { "input_tokens": 10, "output_tokens": 5 }
    });
    let content = normalize_response_content(&data);
    assert_eq!(content["content"], "let me check");
    assert_eq!(content["tool_calls"][0]["id"], "tu_1");
    assert_eq!(content["tool_calls"][0]["type"], "function");
    assert_eq!(content["tool_calls"][0]["function"]["name"], "grep");
    assert_eq!(
        content["tool_calls"][0]["function"]["arguments"]["pattern"],
        "x"
    );
    assert_eq!(extract_metrics(&data).tool_call_count, 1);
}

#[test]
fn response_pure_tool_use_omits_content_field() {
    let data = json!({
        "content": [
            { "type": "tool_use", "id": "tu_1", "name": "ls", "input": {} }
        ],
        "stop_reason": "tool_use"
    });
    let content = normalize_response_content(&data);
    assert!(content.get("content").is_none());
    assert_eq!(content["tool_calls"][0]["function"]["name"], "ls");
}
