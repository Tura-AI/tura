//! Native Anthropic Messages API compatibility layer for the `claude-code`
//! provider.
//!
//! `claude-code` can authenticate in two distinct ways and this module handles
//! both behind one entry point:
//!
//! * **OAuth subscription route** — the token is a Claude Code subscription
//!   token (`sk-ant-oat...`). Anthropic only accepts it on `/v1/messages` with a
//!   `Bearer` token, the `anthropic-beta: oauth-2025-04-20` header, and a system
//!   prompt whose first line is exactly [`CLAUDE_CODE_SYSTEM_PROMPT`]. Any other
//!   shape is rejected (HTTP 401/429).
//! * **API-key route** — the token is a normal Anthropic API key
//!   (`sk-ant-api...`). It uses the `x-api-key` header and imposes none of the
//!   subscription identity constraints.
//!
//! Both routes speak the native Messages API (not the OpenAI-compatible shim),
//! so this module converts the OpenAI-shaped messages/tools that the rest of
//! tura produces into Anthropic blocks, and converts the Anthropic response back
//! into the OpenAI-shaped `tool_calls` content the runtime state machine
//! consumes.

use serde_json::{json, Map, Value};

use crate::tura_llm::{
    CallMetrics, CallOptions, ProviderResponse, ProviderStreamEvent, ProviderStreamEventSink,
    TuraError,
};
use crate::utils::{
    anthropic_blocks_from_canonical, anthropic_tool_result_content_from_canonical,
    emit_command_run_stream_events_from_content, extract_xml_tool_calls,
    normalize_command_run_tool_input, strip_json_fence, strip_xml_tool_calls, to_anthropic_tools,
};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const OAUTH_BETA: &str = "oauth-2025-04-20";

/// Required system identity for Claude Code subscription OAuth tokens. Anthropic
/// rejects any request from a subscription token whose system prompt does not
/// start with this exact line, so it is always prepended on the OAuth route.
pub(crate) const CLAUDE_CODE_SYSTEM_PROMPT: &str =
    "You are Claude Code, Anthropic's official CLI for Claude.";

const DEFAULT_MAX_TOKENS: u64 = 1024;

/// `true` when the token is a Claude Code subscription OAuth token rather than a
/// standard Anthropic API key.
fn is_oauth_subscription_token(token: &str) -> bool {
    token.starts_with("sk-ant-oat")
}

pub async fn call_with_stream_events(
    base_url: &str,
    model: &str,
    access_token: &str,
    messages: &[Value],
    options: &CallOptions,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    let oauth = is_oauth_subscription_token(access_token);
    let payload = build_payload(model, messages, options, oauth);

    let client = reqwest::Client::builder()
        .build()
        .map_err(|err| TuraError::Network {
            message: err.to_string(),
        })?;
    let url = format!("{}/messages", base_url.trim_end_matches('/'));

    let mut request = client
        .post(url)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .json(&payload);
    request = if oauth {
        request
            .bearer_auth(access_token)
            .header("anthropic-beta", OAUTH_BETA)
    } else {
        request.header("x-api-key", access_token)
    };

    if let Some(sink) = stream_events.as_ref() {
        sink(ProviderStreamEvent::ProviderOutputStarted);
    }

    let resp = request.send().await.map_err(|err| TuraError::Network {
        message: err.to_string(),
    })?;
    let status = resp.status();
    let req_id = resp
        .headers()
        .get("request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = resp.text().await.map_err(|err| TuraError::Network {
        message: err.to_string(),
    })?;
    if !status.is_success() {
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body,
        });
    }
    let data: Value = serde_json::from_str(&body).map_err(|err| TuraError::Network {
        message: err.to_string(),
    })?;

    let content = normalize_response_content(&data);
    emit_command_run_stream_events_from_content(&content, stream_events.as_ref());
    let mut metrics = extract_metrics(&data);
    metrics.provider_request_id = req_id;

    Ok(ProviderResponse {
        content,
        raw: data,
        metrics: Some(metrics),
    })
}

/// Build the native Anthropic Messages payload from OpenAI-shaped inputs.
fn build_payload(model: &str, messages: &[Value], options: &CallOptions, oauth: bool) -> Value {
    let (system, converted) = convert_messages(messages, oauth);

    let max_tokens = options
        .max_tokens
        .filter(|value| *value > 0)
        .or_else(|| options.max_completion_tokens.filter(|value| *value > 0))
        .unwrap_or(DEFAULT_MAX_TOKENS);

    let mut payload = Map::new();
    payload.insert("model".to_string(), Value::String(model.to_string()));
    payload.insert("max_tokens".to_string(), Value::from(max_tokens));
    if !system.trim().is_empty() {
        // Anthropic's OAuth (Claude subscription) channel rejects large,
        // *uncached* requests with a `rate_limit_error` (HTTP 429) even when
        // plenty of quota remains — small requests slip under the threshold,
        // large ones get an instant reject. The real `claude-code` CLI avoids
        // this by sending `system` as typed blocks with a prompt-cache
        // breakpoint (`cache_control: ephemeral`). Replicate that shape so big
        // prompts are accepted. Verified by ablation: the cache breakpoint is
        // the single decisive factor (betas/stream/temperature are irrelevant).
        payload.insert("system".to_string(), Value::Array(system_blocks(&system)));
    }
    payload.insert("messages".to_string(), Value::Array(converted));
    payload.insert("stream".to_string(), Value::Bool(false));

    if let Some(tools) = options.tools.as_ref().filter(|tools| !tools.is_empty()) {
        let anthropic_tools = to_anthropic_tools(tools);
        if !anthropic_tools.is_empty() {
            payload.insert("tools".to_string(), Value::Array(anthropic_tools));
            if let Some(choice) = convert_tool_choice(options.tool_choice.as_ref()) {
                payload.insert("tool_choice".to_string(), choice);
            }
        }
    }

    // `temperature` is deliberately never forwarded: current Claude models
    // (e.g. opus-4-8) reject it as a deprecated parameter, and the runtime
    // always supplies one. Extended thinking is the only sampling control we
    // pass through.
    if let Some(budget) = thinking_budget(options, max_tokens) {
        payload.insert(
            "thinking".to_string(),
            json!({ "type": "enabled", "budget_tokens": budget }),
        );
    }

    if let Some(top_p) = options.top_p {
        payload.insert("top_p".to_string(), json!(top_p));
    }
    if let Some(stop) = stop_sequences(options.stop.as_ref()) {
        payload.insert("stop_sequences".to_string(), stop);
    }

    Value::Object(payload)
}

/// Split the system prompt into typed Anthropic blocks with a prompt-cache
/// breakpoint, mirroring the real `claude-code` CLI. The leading
/// [`CLAUDE_CODE_SYSTEM_PROMPT`] prefix is emitted as its own block carrying
/// `cache_control: ephemeral`; any remaining content follows as a plain block.
/// Without this breakpoint the OAuth channel 429s on large prompts.
fn system_blocks(system: &str) -> Vec<Value> {
    let cache = json!({ "type": "ephemeral" });
    if let Some(rest) = system.strip_prefix(CLAUDE_CODE_SYSTEM_PROMPT) {
        let mut blocks = vec![json!({
            "type": "text",
            "text": CLAUDE_CODE_SYSTEM_PROMPT,
            "cache_control": cache,
        })];
        let rest = rest.trim_start_matches(['\n', '\r']);
        if !rest.trim().is_empty() {
            blocks.push(json!({ "type": "text", "text": rest }));
        }
        blocks
    } else {
        // No recognized prefix (e.g. non-OAuth path): cache the whole block.
        vec![json!({
            "type": "text",
            "text": system,
            "cache_control": cache,
        })]
    }
}

/// Convert OpenAI chat/Responses-style messages into an Anthropic `system`
/// string plus a list of `messages` with typed content blocks. Adjacent
/// same-role messages are merged so the result always alternates roles, which
/// Anthropic requires.
fn convert_messages(messages: &[Value], oauth: bool) -> (String, Vec<Value>) {
    let mut system = if oauth {
        CLAUDE_CODE_SYSTEM_PROMPT.to_string()
    } else {
        String::new()
    };
    let mut blocks: Vec<(String, Vec<Value>)> = Vec::new();

    let mut push = |role: &str, block: Value| {
        if let Some((last_role, last_blocks)) = blocks.last_mut() {
            if last_role == role {
                last_blocks.push(block);
                return;
            }
        }
        blocks.push((role.to_string(), vec![block]));
    };

    for message in messages {
        // Responses-API tool items carry a `type` instead of a `role`.
        match message.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                push("assistant", responses_tool_use_block(message));
                continue;
            }
            Some("function_call_output") => {
                push("user", responses_tool_result_block(message));
                continue;
            }
            _ => {}
        }

        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");

        match role {
            "system" | "developer" => {
                if let Some(text) = message_text(message.get("content")) {
                    if !text.trim().is_empty() {
                        if !system.is_empty() {
                            system.push_str("\n\n");
                        }
                        system.push_str(&text);
                    }
                }
            }
            "tool" => {
                push("user", chat_tool_result_block(message));
            }
            "assistant" => {
                if let Some(text) = message_text(message.get("content")) {
                    if !text.trim().is_empty() {
                        push("assistant", json!({ "type": "text", "text": text }));
                    }
                }
                for tool_use in chat_tool_use_blocks(message) {
                    push("assistant", tool_use);
                }
            }
            _ => {
                let blocks = anthropic_blocks_from_canonical(message.get("content"))
                    .unwrap_or_else(|| vec![json!({ "type": "text", "text": "" })]);
                for block in blocks {
                    push("user", block);
                }
            }
        }
    }

    let mut converted: Vec<Value> = blocks
        .into_iter()
        .map(|(role, content)| json!({ "role": role, "content": content }))
        .collect();

    // Anthropic requires the conversation to begin with a user turn and to be
    // non-empty.
    if converted.is_empty() {
        converted.push(json!({ "role": "user", "content": [{ "type": "text", "text": "" }] }));
    }

    (system, converted)
}

fn responses_tool_use_block(message: &Value) -> Value {
    let id = message
        .get("call_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("call_command_run");
    let name = message
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("command_run");
    let input = parse_arguments(message.get("arguments"));
    json!({ "type": "tool_use", "id": id, "name": name, "input": input })
}

fn responses_tool_result_block(message: &Value) -> Value {
    let id = message
        .get("call_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("call_command_run");
    let output = anthropic_tool_result_content_from_canonical(
        message.get("output").or_else(|| message.get("content")),
    );
    json!({ "type": "tool_result", "tool_use_id": id, "content": output })
}

fn chat_tool_result_block(message: &Value) -> Value {
    let id = message
        .get("tool_call_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("call_command_run");
    let output = anthropic_tool_result_content_from_canonical(message.get("content"));
    json!({ "type": "tool_result", "tool_use_id": id, "content": output })
}

fn chat_tool_use_blocks(message: &Value) -> Vec<Value> {
    let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) else {
        return Vec::new();
    };
    tool_calls
        .iter()
        .enumerate()
        .filter_map(|(index, call)| {
            let function = call.get("function")?;
            let name = function.get("name").and_then(Value::as_str)?;
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("call_{index}"));
            let input = parse_arguments(function.get("arguments"));
            Some(json!({ "type": "tool_use", "id": id, "name": name, "input": input }))
        })
        .collect()
}

/// OpenAI tool arguments arrive either as a JSON string or an object; Anthropic
/// `input` must be an object.
fn parse_arguments(value: Option<&Value>) -> Value {
    match value {
        Some(Value::String(text)) => serde_json::from_str(text).unwrap_or_else(|_| json!({})),
        Some(obj @ Value::Object(_)) => obj.clone(),
        _ => json!({}),
    }
}

fn convert_tool_choice(choice: Option<&Value>) -> Option<Value> {
    match choice? {
        Value::String(text) => match text.as_str() {
            "required" | "any" => Some(json!({ "type": "any" })),
            "none" => None,
            _ => Some(json!({ "type": "auto" })),
        },
        Value::Object(object) => {
            let name = object
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)?;
            Some(json!({ "type": "tool", "name": name }))
        }
        _ => None,
    }
}

fn thinking_budget(options: &CallOptions, max_tokens: u64) -> Option<u64> {
    let effort = options
        .reasoning_effort
        .as_deref()?
        .trim()
        .to_ascii_lowercase();
    if effort.is_empty() || effort == "default" || effort == "none" || effort == "minimal" {
        return None;
    }
    let budget = match effort.as_str() {
        "low" => 1024,
        "medium" => 4096,
        "high" | "highest" | "xhigh" => 8192,
        _ => 1024,
    };
    // Anthropic requires max_tokens to exceed the thinking budget; if the caller
    // capped max_tokens too low for the requested budget, skip thinking rather
    // than emit a request the API will reject.
    (max_tokens > budget).then_some(budget)
}

fn stop_sequences(stop: Option<&Value>) -> Option<Value> {
    match stop? {
        Value::String(text) if !text.trim().is_empty() => Some(json!([text])),
        Value::Array(items) if !items.is_empty() => Some(Value::Array(items.clone())),
        _ => None,
    }
}

/// Convert an Anthropic response into the OpenAI-shaped content the runtime
/// state machine consumes: a bare string for plain text, `{ tool_calls }` for a
/// pure tool turn, or `{ content, tool_calls }` when both are present.
fn normalize_response_content(data: &Value) -> Value {
    let mut text = String::new();
    let mut tool_calls = Vec::new();

    if let Some(blocks) = data.get("content").and_then(Value::as_array) {
        for block in blocks {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(value) = block.get("text").and_then(Value::as_str) {
                        text.push_str(value);
                        tool_calls.extend(extract_xml_tool_calls(value));
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("tool_use")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let input = normalize_command_run_tool_input(
                        &name,
                        block.get("input").cloned().unwrap_or_else(|| json!({})),
                    );
                    tool_calls.push(json!({
                        "id": id,
                        "type": "function",
                        "function": { "name": name, "arguments": input },
                    }));
                }
                _ => {}
            }
        }
    }

    let stripped = strip_xml_tool_calls(&strip_json_fence(&text));
    if !tool_calls.is_empty() && !stripped.trim().is_empty() {
        json!({ "content": stripped, "tool_calls": tool_calls })
    } else if !tool_calls.is_empty() {
        json!({ "tool_calls": tool_calls })
    } else {
        Value::String(stripped)
    }
}

fn extract_metrics(data: &Value) -> CallMetrics {
    let mut metrics = CallMetrics::default();
    metrics.usage.input_tokens = data.pointer("/usage/input_tokens").and_then(Value::as_u64);
    metrics.usage.output_tokens = data.pointer("/usage/output_tokens").and_then(Value::as_u64);
    metrics.usage.cached_input_tokens = data
        .pointer("/usage/cache_read_input_tokens")
        .and_then(Value::as_u64);
    metrics.usage.cache_write_tokens = data
        .pointer("/usage/cache_creation_input_tokens")
        .and_then(Value::as_u64);
    // A non-zero `cache_read_input_tokens` means the prompt-cache breakpoint hit
    // on this request; surface it on the metrics flags the same way the OpenAI
    // and Google paths do so cache reporting is consistent across providers.
    let cached = metrics.usage.cached_input_tokens.unwrap_or(0);
    metrics.cache_hit = cached > 0;
    metrics.cache_triggered_at_input_tokens = metrics.usage.cached_input_tokens;
    metrics.finish_reason = data
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(str::to_string);
    metrics.tool_call_count = data
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
                .count()
        })
        .unwrap_or(0);
    metrics.raw_usage = data.get("usage").cloned();
    metrics
}

fn message_text(content: Option<&Value>) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn oauth_route_prepends_claude_code_system_prompt() {
        let messages = vec![json!({ "role": "user", "content": "hi" })];
        let payload = build_payload("claude-opus-4-8", &messages, &CallOptions::default(), true);
        // System is emitted as typed blocks; the prefix block carries the
        // prompt-cache breakpoint required to avoid OAuth 429s on big prompts.
        let blocks = payload["system"].as_array().unwrap();
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
        let blocks = payload["system"].as_array().unwrap();
        assert_eq!(blocks[0]["text"], CLAUDE_CODE_SYSTEM_PROMPT);
        assert_eq!(blocks[0]["cache_control"]["type"], "ephemeral");
        let merged: String = blocks
            .iter()
            .map(|b| b["text"].as_str().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n\n");
        assert!(merged.contains("Be terse."));
        assert_eq!(payload["messages"].as_array().unwrap().len(), 1);
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
        assert_eq!(payload["messages"].as_array().unwrap().len(), 1);
        assert_eq!(
            payload["messages"][0]["content"].as_array().unwrap().len(),
            2
        );
    }

    #[test]
    fn tools_and_tool_choice_convert() {
        let mut options = CallOptions::default();
        options.tools = Some(vec![json!({
            "type": "function",
            "function": { "name": "grep", "description": "search", "parameters": {"type":"object"} }
        })]);
        options.tool_choice = Some(json!({ "type": "function", "function": { "name": "grep" } }));
        let messages = vec![json!({ "role": "user", "content": "hi" })];
        let payload = build_payload("claude-opus-4-8", &messages, &options, true);
        assert_eq!(payload["tools"][0]["name"], "grep");
        assert_eq!(payload["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(payload["tool_choice"]["type"], "tool");
        assert_eq!(payload["tool_choice"]["name"], "grep");
    }

    #[test]
    fn thinking_enabled_omits_temperature() {
        let mut options = CallOptions::default();
        options.reasoning_effort = Some("low".to_string());
        options.temperature = Some(0.0);
        options.max_tokens = Some(8192);
        let messages = vec![json!({ "role": "user", "content": "hi" })];
        let payload = build_payload("claude-opus-4-8", &messages, &options, true);
        assert_eq!(payload["thinking"]["type"], "enabled");
        assert_eq!(payload["thinking"]["budget_tokens"], 1024);
        assert!(payload.get("temperature").is_none());
    }

    #[test]
    fn thinking_skipped_when_budget_exceeds_max_tokens() {
        let mut options = CallOptions::default();
        options.reasoning_effort = Some("high".to_string());
        options.max_tokens = Some(2048);
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
}
