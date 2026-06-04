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

use std::collections::BTreeMap;
use std::time::Instant;

use serde_json::{json, Map, Value};

use crate::streaming::{next_provider_stream_chunk, read_provider_response_body};
use crate::tura_llm::{
    CallMetrics, CallOptions, ProviderResponse, ProviderStreamEvent, ProviderStreamEventSink,
    TuraError,
};
use crate::utils::{
    anthropic_blocks_from_canonical, anthropic_tool_result_content_from_canonical, deep_merge_json,
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
    let mut payload = build_payload(model, messages, options, oauth);
    let should_stream = stream_events.is_some() || options.stream.unwrap_or(false);
    if should_stream {
        payload["stream"] = Value::Bool(true);
    }

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

    let resp = request.send().await.map_err(|err| TuraError::Network {
        message: err.to_string(),
    })?;
    let status = resp.status();
    let req_id = resp
        .headers()
        .get("request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if should_stream && status.is_success() {
        let mut response = parse_anthropic_stream(resp, stream_events).await?;
        if let Some(metrics) = response.metrics.as_mut() {
            metrics.provider_request_id = req_id;
        }
        return Ok(response);
    }

    let body = read_provider_response_body(resp.text()).await?;
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

async fn parse_anthropic_stream(
    resp: reqwest::Response,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    let mut stream = resp.bytes_stream();
    let mut pending = String::new();
    let mut state = AnthropicStreamState::default();
    let mut saw_output = false;
    let mut last_output_at = Instant::now();

    while let Some(chunk) =
        next_provider_stream_chunk(&mut stream, saw_output, last_output_at).await?
    {
        let chunk = chunk.map_err(|err| TuraError::Network {
            message: err.to_string(),
        })?;
        pending.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = pending.find('\n') {
            let line = pending[..line_end].trim_end_matches('\r').to_string();
            pending.drain(..=line_end);
            if process_anthropic_sse_line(&line, &mut state, stream_events.as_ref())? {
                saw_output = true;
                last_output_at = Instant::now();
            }
        }
    }

    if !pending.trim().is_empty() {
        let _ = process_anthropic_sse_line(&pending, &mut state, stream_events.as_ref())?;
    }

    let data = state.into_message();
    let content = normalize_response_content(&data);
    let metrics = extract_metrics(&data);

    Ok(ProviderResponse {
        content,
        raw: data,
        metrics: Some(metrics),
    })
}

#[derive(Default)]
struct AnthropicStreamState {
    message: Map<String, Value>,
    content: BTreeMap<usize, Value>,
    tool_input_buffers: BTreeMap<usize, String>,
    saw_output_started: bool,
}

fn process_anthropic_sse_line(
    line: &str,
    state: &mut AnthropicStreamState,
    stream_events: Option<&ProviderStreamEventSink>,
) -> Result<bool, TuraError> {
    let line = line.trim_start();
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(false);
    };
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return Ok(false);
    }

    let value: Value = serde_json::from_str(data).map_err(TuraError::Json)?;
    Ok(state.push_event(&value, stream_events))
}

impl AnthropicStreamState {
    fn push_event(
        &mut self,
        event: &Value,
        stream_events: Option<&ProviderStreamEventSink>,
    ) -> bool {
        let mut output_started = false;
        match event.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                if let Some(message) = event.get("message").and_then(Value::as_object) {
                    self.message = message.clone();
                }
            }
            Some("content_block_start") => {
                if let (Some(index), Some(block)) = (
                    event.get("index").and_then(Value::as_u64),
                    event.get("content_block"),
                ) {
                    self.content.insert(index as usize, block.clone());
                    output_started = true;
                }
            }
            Some("content_block_delta") => {
                if let Some(index) = event.get("index").and_then(Value::as_u64) {
                    if let Some(delta) = event.get("delta") {
                        self.apply_delta(index as usize, delta);
                        output_started = true;
                    }
                }
            }
            Some("content_block_stop") => {
                if let Some(index) = event.get("index").and_then(Value::as_u64) {
                    self.finish_block(index as usize, stream_events);
                }
            }
            Some("message_delta") => {
                if let Some(delta) = event.get("delta").and_then(Value::as_object) {
                    for (key, value) in delta {
                        self.message.insert(key.clone(), value.clone());
                    }
                }
                if let Some(usage) = event.get("usage") {
                    merge_usage(&mut self.message, usage);
                }
            }
            _ => {}
        }

        if output_started && !self.saw_output_started {
            self.saw_output_started = true;
            if let Some(sink) = stream_events {
                sink(ProviderStreamEvent::ProviderOutputStarted);
            }
        }
        output_started
    }

    fn apply_delta(&mut self, index: usize, delta: &Value) {
        match delta.get("type").and_then(Value::as_str) {
            Some("text_delta") => {
                if let Some(text) = delta.get("text").and_then(Value::as_str) {
                    let block = self
                        .content
                        .entry(index)
                        .or_insert_with(|| json!({ "type": "text", "text": "" }));
                    append_string_field(block, "text", text);
                }
            }
            Some("input_json_delta") => {
                if let Some(partial) = delta.get("partial_json").and_then(Value::as_str) {
                    self.tool_input_buffers
                        .entry(index)
                        .or_default()
                        .push_str(partial);
                }
            }
            Some("thinking_delta") => {
                if let Some(text) = delta.get("thinking").and_then(Value::as_str) {
                    let block = self
                        .content
                        .entry(index)
                        .or_insert_with(|| json!({ "type": "thinking", "thinking": "" }));
                    append_string_field(block, "thinking", text);
                }
            }
            _ => {}
        }
    }

    fn finish_block(&mut self, index: usize, stream_events: Option<&ProviderStreamEventSink>) {
        let Some(block) = self.content.get_mut(&index) else {
            return;
        };
        if let Some(buffer) = self.tool_input_buffers.remove(&index) {
            if let Ok(input) = serde_json::from_str::<Value>(&buffer) {
                block["input"] = input;
            }
        }
        for event in command_run_events_from_anthropic_tool_block(block) {
            if let Some(sink) = stream_events {
                sink(event);
            }
        }
    }

    fn into_message(self) -> Value {
        let mut message = self.message;
        if !message.contains_key("type") {
            message.insert("type".to_string(), Value::String("message".to_string()));
        }
        if !message.contains_key("role") {
            message.insert("role".to_string(), Value::String("assistant".to_string()));
        }
        message.insert(
            "content".to_string(),
            Value::Array(self.content.into_values().collect()),
        );
        Value::Object(message)
    }
}

fn append_string_field(block: &mut Value, field: &str, text: &str) {
    if let Some(object) = block.as_object_mut() {
        let current = object
            .entry(field.to_string())
            .or_insert_with(|| Value::String(String::new()));
        if let Some(value) = current.as_str() {
            *current = Value::String(format!("{value}{text}"));
        }
    }
}

fn merge_usage(message: &mut Map<String, Value>, usage_delta: &Value) {
    let usage = message
        .entry("usage".to_string())
        .or_insert_with(|| json!({}));
    let Some(target) = usage.as_object_mut() else {
        return;
    };
    if let Some(source) = usage_delta.as_object() {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn command_run_events_from_anthropic_tool_block(block: &Value) -> Vec<ProviderStreamEvent> {
    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
        return Vec::new();
    }
    let name = block.get("name").and_then(Value::as_str).unwrap_or("");
    if name != "command_run" {
        return Vec::new();
    }
    let input = normalize_command_run_tool_input(
        name,
        block.get("input").cloned().unwrap_or_else(|| json!({})),
    );
    let Some(commands) = input.get("commands").and_then(Value::as_array) else {
        return Vec::new();
    };
    let tool_call_id = block
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("tool_use")
        .to_string();
    commands
        .iter()
        .cloned()
        .enumerate()
        .map(
            |(command_index, command)| ProviderStreamEvent::CommandRunCommandReady {
                tool_call_id: tool_call_id.clone(),
                command_index,
                command,
            },
        )
        .collect()
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
    if !system.is_empty() {
        // Anthropic's OAuth (Claude subscription) channel rejects large,
        // *uncached* requests with a `rate_limit_error` (HTTP 429) even when
        // plenty of quota remains — small requests slip under the threshold,
        // large ones get an instant reject. The real `claude-code` CLI avoids
        // this by sending `system` as typed blocks with a prompt-cache
        // breakpoint (`cache_control: ephemeral`). Replicate that shape so big
        // prompts are accepted. Verified by ablation: the cache breakpoint is
        // the single decisive factor (betas/stream/temperature are irrelevant).
        payload.insert("system".to_string(), Value::Array(system));
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

    let mut payload = Value::Object(payload);
    if let Some(extra_body) = &options.extra_body {
        deep_merge_json(&mut payload, extra_body.clone());
    }
    payload
}

/// Build typed Anthropic system blocks with a prompt-cache breakpoint,
/// mirroring the real `claude-code` CLI's system position.
fn oauth_system_prefix_block() -> Value {
    let cache = json!({ "type": "ephemeral" });
    json!({
        "type": "text",
        "text": CLAUDE_CODE_SYSTEM_PROMPT,
        "cache_control": cache,
    })
}

fn system_blocks_from_content(content: Option<&Value>) -> Vec<Value> {
    match content {
        Some(Value::Array(items)) => {
            let mut blocks = Vec::new();
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some("text")
                    && item.get("text").and_then(Value::as_str).is_some()
                {
                    blocks.push(item.clone());
                } else if let Some(text) = item
                    .get("text")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("content").and_then(Value::as_str))
                    .filter(|text| !text.trim().is_empty())
                {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
            }
            blocks
        }
        Some(Value::String(text)) if !text.trim().is_empty() => {
            vec![json!({ "type": "text", "text": text })]
        }
        Some(other) if !other.is_null() => {
            vec![json!({ "type": "text", "text": other.to_string() })]
        }
        _ => Vec::new(),
    }
}

/// Convert OpenAI chat/Responses-style messages into an Anthropic `system`
/// string plus a list of `messages` with typed content blocks. Adjacent
/// same-role messages are merged so the result always alternates roles, which
/// Anthropic requires.
fn convert_messages(messages: &[Value], oauth: bool) -> (Vec<Value>, Vec<Value>) {
    let mut system = if oauth {
        vec![oauth_system_prefix_block()]
    } else {
        Vec::new()
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
                system.extend(system_blocks_from_content(message.get("content")));
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
                        "provider_metadata": { "id": id },
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
        let captured = events.clone();
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
}
