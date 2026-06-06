use crate::state_machine::session_management::SessionId;
use tura_llm_rust::{openai_compatible_usage_stream_supported, prompt_cache_key_supported};

const COMMAND_RUN_TOOL_NAME: &str = "command_run";

pub(crate) fn normalize_provider_messages(
    messages: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut normalized = Vec::new();

    for message in messages {
        if matches!(
            message.get("type").and_then(serde_json::Value::as_str),
            Some("function_call" | "function_call_output")
        ) {
            normalized.push(message);
            continue;
        }
        let role = message
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("user");
        if role == "assistant" {
            if let Some(tool_calls) = message
                .get("tool_calls")
                .filter(|value| value.as_array().is_some_and(|calls| !calls.is_empty()))
                .cloned()
            {
                let mut item = serde_json::json!({
                    "role": "assistant",
                    "content": message_content_value(&message),
                    "tool_calls": tool_calls,
                });
                if let Some(name) = message.get("name").and_then(serde_json::Value::as_str) {
                    item["name"] = serde_json::Value::String(name.to_string());
                }
                normalized.push(item);
                continue;
            }
        }
        if role == "tool" {
            if let Some(tool_call_id) = message
                .get("tool_call_id")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                let mut item = serde_json::json!({
                    "role": "tool",
                    "content": message_content_value(&message),
                    "tool_call_id": tool_call_id,
                });
                if let Some(name) = message.get("name").and_then(serde_json::Value::as_str) {
                    item["name"] = serde_json::Value::String(name.to_string());
                }
                normalized.push(item);
                continue;
            }
        }
        let content = message_content_value(&message);
        if content_is_empty(&content) {
            continue;
        }

        let (role, content) = match role {
            "system" | "developer" | "user" | "assistant" | "tool" => (role, content),
            other => (
                "user",
                serde_json::Value::String(format!(
                    "Runtime context ({other}):\n{}",
                    message_content_text(&content)
                )),
            ),
        };
        normalized.push(serde_json::json!({
            "role": role,
            "content": content
        }));
    }
    normalized
}

fn message_content_value(message: &serde_json::Value) -> serde_json::Value {
    if let Some(content) = message.get("content") {
        return content.clone();
    }
    if let Some(text) = message.get("text").and_then(serde_json::Value::as_str) {
        return serde_json::Value::String(text.to_string());
    }
    serde_json::Value::String(String::new())
}

fn content_is_empty(content: &serde_json::Value) -> bool {
    match content {
        serde_json::Value::String(text) => text.trim().is_empty(),
        serde_json::Value::Array(items) => items.is_empty(),
        serde_json::Value::Null => true,
        _ => false,
    }
}

fn message_content_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(text) => text.to_string(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| item.get("content").and_then(serde_json::Value::as_str))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other if other.is_null() => String::new(),
        other => other.to_string(),
    }
}

pub(crate) fn session_reasoning_effort() -> Option<String> {
    std::env::var("TURA_SESSION_REASONING_EFFORT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
}

pub(crate) fn prompt_cache_key(
    route_config: &tura_llm_rust::RouteConfig,
    route_name: &str,
    session_id: &SessionId,
    tools: &[serde_json::Value],
) -> Option<String> {
    let provider = route_config.providers.first()?;
    if !prompt_cache_key_supported(&provider.provider, &provider.base_url) {
        return None;
    }
    let mut tool_names = tools
        .iter()
        .filter_map(tool_name)
        .filter(|name| name != COMMAND_RUN_TOOL_NAME)
        .collect::<Vec<_>>();
    tool_names.sort();
    let tool_sig = tool_names.join(",");
    let hash_input = format!(
        "{}\n{}\n{}\n{}\n{}",
        route_name, session_id, provider.provider, provider.model, tool_sig
    );
    Some(format!(
        "turaosv2:{}:{}:{}",
        short_key_part(route_name),
        short_key_part(session_id),
        fnv1a64_hex(&hash_input)
    ))
}

pub(crate) fn stream_options(
    route_config: &tura_llm_rust::RouteConfig,
    stream: bool,
) -> Option<serde_json::Value> {
    if !stream {
        return None;
    }
    let provider = route_config.providers.first()?;
    if !openai_compatible_usage_stream_supported(&provider.provider, &provider.base_url) {
        return None;
    }
    Some(serde_json::json!({ "include_usage": true }))
}

fn tool_name(tool: &serde_json::Value) -> Option<String> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| tool.get("name").and_then(serde_json::Value::as_str))
        .map(ToString::to_string)
}

fn short_key_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(24)
        .collect()
}

fn fnv1a64_hex(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub(crate) fn parallel_tool_calls_enabled(
    route_config: &tura_llm_rust::RouteConfig,
    has_tools: bool,
) -> Option<bool> {
    if !has_tools {
        return None;
    }
    if let Ok(value) = std::env::var("TURA_PARALLEL_TOOL_CALLS") {
        let value = value.trim().to_ascii_lowercase();
        if matches!(value.as_str(), "0" | "false" | "no" | "off") {
            return Some(false);
        }
        if matches!(value.as_str(), "1" | "true" | "yes" | "on") {
            return Some(true);
        }
    }

    route_config.providers.first().map(|_| false)
}

pub(crate) fn session_service_tier() -> Option<String> {
    let enabled = std::env::var("TURA_SESSION_ACCELERATION_ENABLED")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    enabled.then(|| "priority".to_string())
}

pub(crate) fn session_max_tokens(agent_max_tokens: u32) -> Option<u64> {
    std::env::var("TURA_SESSION_MAX_TOKENS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .or_else(|| (agent_max_tokens > 0).then_some(u64::from(agent_max_tokens)))
}

pub fn route_by_name<'a>(
    settings: &'a tura_llm_rust::Settings,
    provider_name: &str,
) -> Option<&'a tura_llm_rust::RouteConfig> {
    settings.route_by_name(provider_name)
}

pub(crate) fn session_model_override_route(
    settings: &tura_llm_rust::Settings,
    fallback: &tura_llm_rust::RouteConfig,
) -> Option<tura_llm_rust::RouteConfig> {
    let value = std::env::var("TURA_SESSION_MODEL_OVERRIDE").ok()?;
    let (provider, model) = value.trim().split_once('/')?;
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    let base_url = provider_base_url(settings, provider)?;
    let temperature = fallback
        .providers
        .first()
        .map(|item| item.temperature)
        .unwrap_or(fallback.default_temperature);
    Some(tura_llm_rust::RouteConfig {
        default_temperature: fallback.default_temperature,
        providers: vec![tura_llm_rust::ProviderConfig {
            provider: provider.to_string(),
            base_url,
            model: tura_llm_rust::Settings::normalize_model_name(provider, model),
            temperature,
        }],
    })
}

fn provider_base_url(settings: &tura_llm_rust::Settings, provider: &str) -> Option<String> {
    settings.provider_base_url(provider)
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_provider_messages, prompt_cache_key, session_reasoning_effort,
        session_service_tier, stream_options,
    };
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};
    use tura_llm_rust::strip_thought_blocks;

    const REASONING_ENV: &str = "TURA_SESSION_REASONING_EFFORT";
    const ACCEL_ENV: &str = "TURA_SESSION_ACCELERATION_ENABLED";
    const DISABLE_CACHE_ENV: &str = "TURA_DISABLE_PROMPT_CACHE";

    fn with_env<T>(name: &str, value: Option<&str>, run: impl FnOnce() -> T) -> T {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("provider flow env lock poisoned");
        let previous: Option<OsString> = std::env::var_os(name);
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
        let result = run();
        match previous {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
        result
    }

    #[test]
    fn reasoning_default_is_omitted_when_unset_empty_or_default() {
        with_env(REASONING_ENV, None, || {
            assert_eq!(session_reasoning_effort(), None)
        });
        with_env(REASONING_ENV, Some("   "), || {
            assert_eq!(session_reasoning_effort(), None)
        });
        with_env(REASONING_ENV, Some(" default "), || {
            assert_eq!(session_reasoning_effort(), None)
        });
    }

    #[test]
    fn reasoning_uses_selected_effort_when_set() {
        with_env(REASONING_ENV, Some(" high "), || {
            assert_eq!(session_reasoning_effort().as_deref(), Some("high"));
        });
    }

    #[test]
    fn service_tier_is_omitted_when_acceleration_is_not_enabled() {
        with_env(ACCEL_ENV, None, || assert_eq!(session_service_tier(), None));
        with_env(ACCEL_ENV, Some("0"), || {
            assert_eq!(session_service_tier(), None)
        });
    }

    #[test]
    fn service_tier_uses_priority_when_acceleration_is_enabled() {
        with_env(ACCEL_ENV, Some("true"), || {
            assert_eq!(session_service_tier().as_deref(), Some("priority"));
        });
    }

    #[test]
    fn prompt_cache_key_is_stable_for_openai_toolsets() {
        let route = openai_route();
        let tools_a = vec![
            serde_json::json!({"type":"function","function":{"name":"command_run","parameters":{"type":"object"}}}),
        ];
        let tools_b = vec![
            serde_json::json!({"type":"function","function":{"name":"command_run","parameters":{"type":"object"}}}),
        ];

        with_env(DISABLE_CACHE_ENV, None, || {
            assert_eq!(
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_a),
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_b)
            );
            assert!(
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_a)
                    .expect("prompt cache key should be generated")
                    .starts_with("turaosv2:flagship-thinking:sess-a:")
            );
            assert_ne!(
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_a),
                prompt_cache_key(&route, "flagship_thinking", &"sess-b".to_string(), &tools_a)
            );
        });
    }

    #[test]
    fn prompt_cache_key_is_omitted_for_non_openai_providers() {
        let route = tura_llm_rust::RouteConfig {
            default_temperature: 0.2,
            providers: vec![tura_llm_rust::ProviderConfig {
                provider: "minimax".to_string(),
                base_url: "https://api.minimax.io/v1".to_string(),
                model: "abab".to_string(),
                temperature: 0.2,
            }],
        };
        with_env(DISABLE_CACHE_ENV, None, || {
            assert_eq!(
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &[]),
                None
            );
        });
    }

    #[test]
    fn prompt_cache_key_can_be_disabled() {
        let route = openai_route();
        with_env(DISABLE_CACHE_ENV, Some("true"), || {
            assert_eq!(
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &[]),
                None
            );
        });
    }

    #[test]
    fn stream_options_request_openai_compatible_usage_when_streaming() {
        let minimax_route = tura_llm_rust::RouteConfig {
            default_temperature: 0.2,
            providers: vec![tura_llm_rust::ProviderConfig {
                provider: "minimax".to_string(),
                base_url: "https://api.minimax.io/v1".to_string(),
                model: "abab".to_string(),
                temperature: 0.2,
            }],
        };
        assert_eq!(
            stream_options(&openai_route(), true),
            Some(serde_json::json!({ "include_usage": true }))
        );
        assert_eq!(stream_options(&openai_route(), false), None);
        assert_eq!(
            stream_options(&minimax_route, true),
            Some(serde_json::json!({ "include_usage": true }))
        );
    }

    #[test]
    fn normalize_provider_messages_preserves_message_boundaries_for_cache() {
        let normalized = normalize_provider_messages(vec![
            serde_json::json!({"role": "system", "content": "## Input rules\nstable"}),
            serde_json::json!({"role": "system", "content": "# COMMAND_RUN Tool Guide\nstable"}),
            serde_json::json!({"role": "system", "content": "Dynamic runtime state:\ncurrent_directory: C:/tmp"}),
            serde_json::json!({"role": "user", "content": "do the task"}),
            serde_json::json!({"role": "system", "content": "Recent tool callback result from `command_run`:\nok"}),
            serde_json::json!({"role": "debug", "content": "unknown role text"}),
            serde_json::json!({"role": "user", "content": ""}),
        ]);

        assert_eq!(normalized.len(), 6);
        assert_eq!(
            normalized[5]["content"],
            "Runtime context (debug):\nunknown role text"
        );
    }

    #[test]
    fn normalize_provider_messages_preserves_chat_tool_envelope() {
        let normalized = normalize_provider_messages(vec![
            serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "command_run", "arguments": "{\"commands\":[]}"}}]
            }),
            serde_json::json!({"role": "tool", "tool_call_id": "call_1", "content": "{\"ok\":true}"}),
        ]);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["tool_calls"][0]["id"], "call_1");
        assert_eq!(normalized[1]["tool_call_id"], "call_1");
    }

    #[test]
    fn normalize_provider_messages_preserves_structured_media_content() {
        let normalized = normalize_provider_messages(vec![serde_json::json!({
            "role": "user",
            "content": [
                { "type": "input_text", "text": "inspect this" },
                { "type": "input_image", "image_url": "data:image/png;base64,AAA" }
            ]
        })]);

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0]["content"][0]["type"], "input_text");
        assert_eq!(normalized[0]["content"][1]["type"], "input_image");
    }

    #[test]
    fn strip_thought_blocks_removes_visible_reasoning_text() {
        assert_eq!(
            strip_thought_blocks("<thought>hidden</thought>visible"),
            "visible"
        );
        assert_eq!(strip_thought_blocks("a<THOUGHT>hidden</THOUGHT>b"), "ab");
    }

    fn openai_route() -> tura_llm_rust::RouteConfig {
        tura_llm_rust::RouteConfig {
            default_temperature: 0.2,
            providers: vec![tura_llm_rust::ProviderConfig {
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5.1-codex-mini".to_string(),
                temperature: 0.2,
            }],
        }
    }
}
