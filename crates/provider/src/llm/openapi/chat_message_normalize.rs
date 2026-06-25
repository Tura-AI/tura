use serde_json::{json, Value};

use super::super::common::message_content_text;
use crate::utils::{openai_chat_content_from_canonical, openai_chat_media_content_from_canonical};

pub(crate) fn normalize_messages_for_provider(provider: &str, messages: &[Value]) -> Vec<Value> {
    if needs_chat_tool_result_messages(provider) {
        return normalize_openai_compatible_chat_messages(messages);
    }
    messages
        .iter()
        .map(|m| {
            let mut msg = m.clone();
            if msg.get("role").and_then(Value::as_str) == Some("assistant")
                && msg.get("content").is_some_and(Value::is_null)
            {
                msg["content"] = Value::String(String::new());
            }
            msg
        })
        .collect()
}

fn needs_chat_tool_result_messages(provider: &str) -> bool {
    !(provider.eq_ignore_ascii_case("openai") || provider.eq_ignore_ascii_case("anthropic"))
}

/// Normalize a conversation for an OpenAI-compatible Chat Completions endpoint
/// while **preserving native roles**. Earlier revisions collapsed every turn
/// into a `user` message (folding `system`/`tool` into prose), which threw away
/// the instruction/tool-call structure these providers natively understand.
///
/// The rules now:
/// * `system` stays `system`.
/// * `assistant` stays `assistant`, keeping any `tool_calls` array intact even
///   when the textual content is empty (the Chat API requires the assistant
///   turn that issued the calls to precede their `tool` results).
/// * `tool` stays `tool`, carrying its `tool_call_id` so the result binds to the
///   originating call.
/// * Responses-API items (`function_call` / `function_call_output`) are
///   translated into the equivalent assistant `tool_calls` / `tool` messages via
///   [`normalize_responses_tool_item_for_chat`].
fn normalize_openai_compatible_chat_messages(messages: &[Value]) -> Vec<Value> {
    let mut normalized = Vec::new();

    for message in messages {
        let items = normalize_responses_tool_item_for_chat(message);
        if !items.is_empty() {
            normalized.extend(items);
            continue;
        }

        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");

        let tool_calls = message
            .get("tool_calls")
            .filter(|value| value.as_array().is_some_and(|calls| !calls.is_empty()))
            .cloned();

        let content_text = message_content_text(message.get("content"))
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        let chat_content = openai_chat_content_from_canonical(message.get("content"));

        match role {
            "assistant" => {
                if tool_calls.is_none() && content_text.is_none() {
                    continue;
                }
                let mut item = json!({
                    "role": "assistant",
                    "content": content_text.unwrap_or_default(),
                });
                if let Some(tool_calls) = tool_calls {
                    item["tool_calls"] = tool_calls;
                }
                normalized.push(item);
            }
            "tool" => {
                let Some(content) = content_text else {
                    continue;
                };
                let mut item = json!({
                    "role": "tool",
                    "content": content,
                });
                if let Some(call_id) = message
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                {
                    item["tool_call_id"] = json!(call_id);
                }
                normalized.push(item);
            }
            "system" | "developer" => {
                let Some(content) = content_text else {
                    continue;
                };
                normalized.push(json!({
                    "role": "system",
                    "content": content,
                }));
            }
            _ => {
                let Some(content) = chat_content else {
                    continue;
                };
                normalized.push(json!({
                    "role": "user",
                    "content": content,
                }));
            }
        }
    }

    if normalized.is_empty() {
        normalized.push(json!({
            "role": "user",
            "content": "Continue.",
        }));
    }

    normalized
}

fn normalize_responses_tool_item_for_chat(message: &Value) -> Vec<Value> {
    match message.get("type").and_then(Value::as_str) {
        Some("function_call") => {
            let call_id = message
                .get("call_id")
                .or_else(|| message.get("id"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("call_command_run");
            let name = message
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("command_run");
            let arguments = message
                .get("arguments")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    message
                        .get("arguments")
                        .cloned()
                        .unwrap_or(Value::Object(Default::default()))
                        .to_string()
                });
            vec![json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": arguments,
                    },
                }],
            })]
        }
        Some("function_call_output") => {
            let call_id = message
                .get("call_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("call_command_run");
            let output_value = message.get("output").or_else(|| message.get("content"));
            let output = message_content_text(output_value).unwrap_or_default();
            let mut items = vec![json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": output,
            })];
            if let Some(media_content) = openai_chat_media_content_from_canonical(output_value) {
                let mut content = vec![json!({
                    "type": "text",
                    "text": "Media payload from the preceding read_media tool result:",
                })];
                if let Some(parts) = media_content.as_array() {
                    content.extend(parts.iter().cloned());
                }
                items.push(json!({
                    "role": "user",
                    "content": content,
                }));
            }
            items
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_routing_keeps_openai_and_anthropic_native_but_fixes_assistant_null_content() {
        let messages = vec![
            json!({"role": "assistant", "content": null, "tool_calls": [{"id": "call_1"}]}),
            json!({"role": "user", "content": [{"type": "input_text", "text": "hi"}]}),
        ];

        for provider in ["openai", "OpenAI", "anthropic", "ANTHROPIC"] {
            let normalized = normalize_messages_for_provider(provider, &messages);

            assert_eq!(normalized.len(), 2, "{provider}");
            assert_eq!(normalized[0]["role"], "assistant", "{provider}");
            assert_eq!(normalized[0]["content"], "", "{provider}");
            assert_eq!(normalized[0]["tool_calls"][0]["id"], "call_1", "{provider}");
            assert_eq!(normalized[1], messages[1], "{provider}");
        }
    }

    #[test]
    fn openai_compatible_chat_normalization_skips_empty_messages_and_falls_back_to_continue() {
        let messages = vec![
            json!({"role": "assistant", "content": null}),
            json!({"role": "tool", "tool_call_id": "call_empty", "content": ""}),
            json!({"role": "system", "content": "   "}),
            json!({"role": "user", "content": []}),
            json!({"role": "unknown"}),
        ];

        let normalized = normalize_openai_compatible_chat_messages(&messages);

        assert_eq!(
            normalized,
            vec![json!({
                "role": "user",
                "content": "Continue.",
            })]
        );
    }

    #[test]
    fn openai_compatible_chat_normalization_maps_roles_and_content_shapes() {
        let messages = vec![
            json!({"role": "developer", "content": [{"type": "input_text", "text": "Dev rules"}]}),
            json!({"role": "system", "content": "System rules"}),
            json!({"role": "assistant", "content": [{"type": "output_text", "text": "Done"}]}),
            json!({"role": "tool", "tool_call_id": "call_1", "content": [{"type": "output_text", "text": "Tool output"}]}),
            json!({"role": "user", "content": [
                {"type": "input_text", "text": "Look"},
                {"type": "input_image", "image_url": "data:image/png;base64,AAA"}
            ]}),
            json!({"content": "missing role defaults to user"}),
        ];

        let normalized = normalize_openai_compatible_chat_messages(&messages);

        assert_eq!(
            normalized[0],
            json!({"role": "system", "content": "Dev rules"})
        );
        assert_eq!(
            normalized[1],
            json!({"role": "system", "content": "System rules"})
        );
        assert_eq!(
            normalized[2],
            json!({"role": "assistant", "content": "Done"})
        );
        assert_eq!(
            normalized[3],
            json!({"role": "tool", "tool_call_id": "call_1", "content": "Tool output"})
        );
        assert_eq!(normalized[4]["role"], "user");
        assert_eq!(
            normalized[4]["content"][0],
            json!({"type": "text", "text": "Look"})
        );
        assert_eq!(normalized[4]["content"][1]["type"], "image_url");
        assert_eq!(
            normalized[5],
            json!({"role": "user", "content": "missing role defaults to user"})
        );
    }

    #[test]
    fn function_call_items_default_missing_fields_and_stringify_object_arguments() {
        let defaulted = normalize_responses_tool_item_for_chat(&json!({
            "type": "function_call"
        }));
        assert_eq!(defaulted.len(), 1);
        assert_eq!(defaulted[0]["role"], "assistant");
        assert_eq!(defaulted[0]["content"], "");
        assert_eq!(defaulted[0]["tool_calls"][0]["id"], "call_command_run");
        assert_eq!(
            defaulted[0]["tool_calls"][0]["function"]["name"],
            "command_run"
        );
        assert_eq!(defaulted[0]["tool_calls"][0]["function"]["arguments"], "{}");

        let object_args = normalize_responses_tool_item_for_chat(&json!({
            "type": "function_call",
            "id": "item_1",
            "name": "custom_tool",
            "arguments": {
                "z": 2,
                "a": 1
            }
        }));
        assert_eq!(object_args[0]["tool_calls"][0]["id"], "item_1");
        assert_eq!(
            object_args[0]["tool_calls"][0]["function"]["name"],
            "custom_tool"
        );
        let arguments = object_args[0]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .expect("arguments string");
        let parsed: Value = serde_json::from_str(arguments).expect("arguments JSON");
        assert_eq!(parsed, json!({"a": 1, "z": 2}));
    }

    #[test]
    fn function_call_output_defaults_call_id_and_uses_content_when_output_is_missing() {
        let normalized = normalize_responses_tool_item_for_chat(&json!({
            "type": "function_call_output",
            "content": [{"type": "output_text", "text": "fallback content"}]
        }));

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0]["role"], "tool");
        assert_eq!(normalized[0]["tool_call_id"], "call_command_run");
        assert_eq!(normalized[0]["content"], "fallback content");
    }

    #[test]
    fn function_call_output_adds_sidecar_media_user_message_after_tool_result() {
        let normalized = normalize_responses_tool_item_for_chat(&json!({
            "type": "function_call_output",
            "call_id": "call_media",
            "output": [
                {"type": "output_text", "text": "media summary"},
                {"type": "input_image", "image_url": "data:image/jpeg;base64,AAA"},
                {"type": "input_file", "filename": "report.pdf", "file_data": "data:application/pdf;base64,BBB"}
            ]
        }));

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["role"], "tool");
        assert_eq!(normalized[0]["tool_call_id"], "call_media");
        assert_eq!(normalized[0]["content"], "media summary");
        assert_eq!(normalized[1]["role"], "user");
        assert_eq!(
            normalized[1]["content"][0]["text"],
            "Media payload from the preceding read_media tool result:"
        );
        assert_eq!(normalized[1]["content"][1]["type"], "image_url");
        assert!(
            normalized[1]["content"]
                .as_array()
                .expect("sidecar content")
                .len()
                == 2
        );
    }

    #[test]
    fn non_response_items_are_not_translated_as_tool_items() {
        assert!(normalize_responses_tool_item_for_chat(&json!({
            "type": "message",
            "role": "user",
            "content": "hello"
        }))
        .is_empty());
        assert!(normalize_responses_tool_item_for_chat(&json!({})).is_empty());
    }
}
