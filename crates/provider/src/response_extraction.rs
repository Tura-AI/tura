//! Provider response normalization: extract plain text and tool calls from the
//! already-`normalize_response_content`-shaped payload, absorbing the per-
//! provider format differences (OpenAI `tool_calls`, Google
//! `parts/functionCall + thoughtSignature`, `<thought>` blocks, etc.).
//!
//! Binding rule: every per-provider format branch is concentrated in this
//! module (inside the provider crate). The upstream runtime only ever sees
//! the normalized `ProviderToolCall` and the plain text; it must contain no
//! provider-specific if/else.
//!
//! The same applies to provider capability probes (whether prompt-cache key
//! is accepted, whether SSE usage is supported, etc.): they are exposed here
//! so the runtime does not have to make per-provider decisions itself.

use serde_json::Value;

use crate::utils::normalize_command_run_tool_input;

/// Normalized tool call consumed directly by the runtime; the runtime does not
/// need to know which provider produced it.
#[derive(Debug, Clone)]
pub struct ProviderToolCall {
    pub tool_name: String,
    pub arguments: Value,
    /// Pass-through provider-specific metadata (e.g. Google's
    /// `thoughtSignature`). The runtime only echoes this back on
    /// replay/serialization without interpreting its content.
    pub provider_metadata: Option<Value>,
}

/// Extract plain text from the normalized response content. Handles three
/// shapes: OpenAI string, `text` field, and concatenated Google `parts[].text`.
pub fn extract_response_text(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    if let Some(text) = content.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = content.get("content").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(parts) = content.get("parts").and_then(Value::as_array) {
        return text_from_parts(parts);
    }
    if let Some(parts) = content
        .get("content")
        .and_then(|value| value.get("parts"))
        .and_then(Value::as_array)
    {
        return text_from_parts(parts);
    }
    None
}

fn text_from_parts(parts: &[Value]) -> Option<String> {
    let text = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

/// Extract tool calls from the normalized response content. Handles two
/// shapes: OpenAI `tool_calls` array and Google `parts[].functionCall`.
pub fn extract_tool_calls(content: &Value) -> Vec<ProviderToolCall> {
    let mut calls = Vec::new();

    if let Some(tool_calls) = content.get("tool_calls").and_then(Value::as_array) {
        for call in tool_calls {
            if let Some(function) = call.get("function") {
                if let Some(name) = function.get("name").and_then(Value::as_str) {
                    let arguments = function.get("arguments").cloned().unwrap_or(Value::Null);
                    calls.push(ProviderToolCall {
                        tool_name: name.to_string(),
                        arguments: normalize_command_run_tool_input(
                            name,
                            parse_arguments(arguments),
                        ),
                        provider_metadata: openai_tool_call_metadata(call),
                    });
                }
            }
        }
    }

    if let Some(parts) = content.get("parts").and_then(Value::as_array) {
        for part in parts {
            if let Some(function_call) = part.get("functionCall") {
                if let Some(name) = function_call.get("name").and_then(Value::as_str) {
                    calls.push(ProviderToolCall {
                        tool_name: name.to_string(),
                        arguments: normalize_command_run_tool_input(
                            name,
                            function_call.get("args").cloned().unwrap_or(Value::Null),
                        ),
                        provider_metadata: google_function_call_metadata(part),
                    });
                }
            }
        }
    }

    calls
}

fn google_function_call_metadata(part: &Value) -> Option<Value> {
    let signature = part
        .get("thoughtSignature")
        .or_else(|| part.get("thought_signature"))
        .and_then(Value::as_str)?;
    Some(serde_json::json!({
        "google_thought_signature": signature,
    }))
}

fn openai_tool_call_metadata(call: &Value) -> Option<Value> {
    let mut metadata = call
        .get("provider_metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for key in ["id", "call_id", "type"] {
        if let Some(value) = call.get(key) {
            metadata
                .entry(key.to_string())
                .or_insert_with(|| value.clone());
        }
    }

    (!metadata.is_empty()).then_some(Value::Object(metadata))
}

fn parse_arguments(arguments: Value) -> Value {
    match arguments {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        other => other,
    }
}

/// Strip `<thought>...</thought>` blocks (some providers leak the thought block into visible text).
pub fn strip_thought_blocks(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(start) = lower.find("<thought>") else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..start]);
        let after_start = &rest[start + "<thought>".len()..];
        let lower_after_start = after_start.to_ascii_lowercase();
        let Some(end) = lower_after_start.find("</thought>") else {
            break;
        };
        rest = &after_start[end + "</thought>".len()..];
    }
    output.trim().to_string()
}

/// Whether the provider supports OpenAI-style `prompt_cache_key`. Can be
/// globally disabled via `TURA_DISABLE_PROMPT_CACHE`. The runtime must call
/// this function rather than inspect provider name / base_url itself.
pub fn prompt_cache_key_supported(provider: &str, base_url: &str) -> bool {
    if std::env::var("TURA_DISABLE_PROMPT_CACHE")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
    {
        return false;
    }
    if matches!(
        provider.to_ascii_lowercase().as_str(),
        "openai" | "openai-api" | "chatgpt" | "codex"
    ) {
        return true;
    }
    base_url.contains("api.openai.com")
}

/// Whether the provider supports OpenAI-compatible SSE `stream_options.include_usage`.
pub fn openai_compatible_usage_stream_supported(provider: &str, base_url: &str) -> bool {
    if provider.eq_ignore_ascii_case("openai")
        || provider.eq_ignore_ascii_case("minimax")
        || provider.eq_ignore_ascii_case("qwen")
        || provider.eq_ignore_ascii_case("openrouter")
    {
        return true;
    }
    base_url.contains("api.openai.com")
        || base_url.contains("api.minimax.io")
        || base_url.contains("dashscope")
        || base_url.contains("openrouter.ai")
}

#[cfg(test)]
mod tests {
    use super::extract_tool_calls;
    use serde_json::json;

    #[test]
    fn extract_tool_calls_preserves_provider_metadata() {
        let calls = extract_tool_calls(&json!({
            "tool_calls": [{
                "id": "toolu_1",
                "type": "function",
                "function": {
                    "name": "command_run",
                    "arguments": {"commands": []}
                },
                "provider_metadata": {"id": "toolu_1"}
            }]
        }));

        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0]
                .provider_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("id"))
                .and_then(serde_json::Value::as_str),
            Some("toolu_1")
        );
    }

    #[test]
    fn extract_tool_calls_preserves_top_level_openai_id_as_metadata() {
        let calls = extract_tool_calls(&json!({
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "command_run",
                    "arguments": {"commands": []}
                }
            }]
        }));

        assert_eq!(calls.len(), 1);
        let metadata = calls[0]
            .provider_metadata
            .as_ref()
            .expect("top-level id should become provider metadata");
        assert_eq!(
            metadata.get("id").and_then(serde_json::Value::as_str),
            Some("call_1")
        );
        assert_eq!(
            metadata.get("type").and_then(serde_json::Value::as_str),
            Some("function")
        );
    }
}
