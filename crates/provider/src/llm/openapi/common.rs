//! Shared helpers for the OpenAI-compatible provider tiers (Chat Completions
//! and Responses). Extracted so both `openapi_chat` and `openapi_response`
//! can reuse the option-normalization and content-flattening utilities
//! without duplicating logic.

use serde_json::Value;

use crate::tura_llm::CallOptions;
use crate::utils::text_from_content;

/// Flatten a message `content` field (string or array-of-parts) into a single
/// text blob, returning `None` when there is nothing meaningful.
pub(crate) fn message_content_text(content: Option<&Value>) -> Option<String> {
    text_from_content(content)
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

pub(crate) fn normalized_reasoning_effort(options: &CallOptions, model: &str) -> Option<String> {
    normalized_non_default_option(options.reasoning_effort.as_deref()).map(|value| {
        if value.eq_ignore_ascii_case("highest")
            || (value.eq_ignore_ascii_case("max") && !model_supports_max_reasoning(model))
        {
            "xhigh".to_string()
        } else if value.eq_ignore_ascii_case("max") {
            "max".to_string()
        } else {
            value
        }
    })
}

fn model_supports_max_reasoning(model: &str) -> bool {
    let model = model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .to_ascii_lowercase();
    matches!(
        model.as_str(),
        "gpt-5.6-sol" | "gpt-5.6-terra" | "gpt-5.6-luna"
    )
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
