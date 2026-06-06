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
