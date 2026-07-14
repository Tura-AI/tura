use super::*;

#[test]
fn openai_compatible_function_output_media_gets_sidecar_user_image() {
    let messages = vec![
        json!({
            "type": "function_call",
            "name": "command_run",
            "call_id": "call_media",
            "arguments": "{\"commands\":[]}"
        }),
        json!({
            "type": "function_call_output",
            "call_id": "call_media",
            "output": [
                { "type": "input_text", "text": "read_media returned image" },
                { "type": "input_image", "image_url": "data:image/jpeg;base64,AAA" }
            ]
        }),
    ];

    let normalized = normalize_messages_for_provider("openrouter", &messages);

    assert_eq!(normalized[1]["role"], "tool");
    assert_eq!(normalized[1]["content"], "read_media returned image");
    assert_eq!(normalized[2]["role"], "user");
    assert_eq!(normalized[2]["content"][1]["type"], "image_url");
    assert_eq!(
        normalized[2]["content"][1]["image_url"]["url"],
        "data:image/jpeg;base64,AAA"
    );
}

#[test]
fn openrouter_qwen_thinking_omits_object_tool_choice() {
    let payload = build_chat_payload(
        "openrouter",
        "qwen3.7-max",
        &[json!({"role": "user", "content": "hi"})],
        &CallOptions {
            tools: Some(vec![json!({
                "type": "function",
                "function": {
                    "name": "command_run",
                    "parameters": {"type": "object"}
                }
            })]),
            tool_choice: Some(json!({
                "type": "function",
                "function": {"name": "command_run"}
            })),
            reasoning_effort: Some("low".to_string()),
            ..Default::default()
        },
    );

    assert!(payload.get("tool_choice").is_none());
    assert!(payload.get("tools").is_some());
    assert_eq!(payload["model"], "qwen/qwen3.7-max");
}

#[test]
fn openrouter_user_facing_models_are_mapped_to_router_ids() {
    let payload = build_chat_payload(
        "openrouter",
        "deepseek-v4-pro",
        &[json!({"role": "user", "content": "hi"})],
        &CallOptions::default(),
    );

    assert_eq!(payload["model"], "deepseek/deepseek-v4-pro");

    let legacy_payload = build_chat_payload(
        "openrouter",
        "qwen/qwen3.6-flash",
        &[json!({"role": "user", "content": "hi"})],
        &CallOptions::default(),
    );

    assert_eq!(legacy_payload["model"], "qwen/qwen3.6-flash");
}
