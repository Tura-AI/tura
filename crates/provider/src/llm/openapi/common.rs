//! Shared helpers for the OpenAI-compatible provider tiers (Chat Completions
//! and Responses). Extracted so both `openapi_chat` and `openapi_response`
//! can reuse the option-normalization and content-flattening utilities
//! without duplicating logic.

use serde_json::Value;

use crate::tura_llm::CallOptions;

/// Flatten a message `content` field (string or array-of-parts) into a single
/// text blob, returning `None` when there is nothing meaningful.
pub(crate) fn message_content_text(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(value) => Some(value.clone()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("content").and_then(Value::as_str))
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
        other if other.is_null() => None,
        other => Some(other.to_string()),
    }
}

/// Insert `key` into `payload` only when `value` is `Some`.
pub(crate) fn insert_opt(payload: &mut Value, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        payload[key] = value;
    }
}

/// `service_tier` is an OpenAI-only acceleration knob; only forward it for the
/// OpenAI GPT/o-series/codex model families.
pub(crate) fn should_pass_service_tier(provider: &str, model: &str) -> bool {
    if !provider.eq_ignore_ascii_case("openai") {
        return false;
    }
    let model = model.to_ascii_lowercase();
    model.starts_with("gpt-") || model.starts_with("o") || model.contains("codex")
}

pub(crate) fn normalized_reasoning_effort(options: &CallOptions) -> Option<String> {
    normalized_non_default_option(options.reasoning_effort.as_deref()).map(|value| {
        if value.eq_ignore_ascii_case("highest") {
            "xhigh".to_string()
        } else {
            value
        }
    })
}

pub(crate) fn normalized_service_tier(options: &CallOptions) -> Option<String> {
    normalized_non_default_option(options.service_tier.as_deref())
}

pub(crate) fn normalized_non_default_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
        .map(ToString::to_string)
}
