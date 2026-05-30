//! The OpenAI-compatible **Chat Completions** tier (`/chat/completions`).
//!
//! This is the default route for providers that don't speak the Responses API
//! (minimax, deepseek, moonshot, openrouter, anthropic-compatible, …). It
//! preserves native message roles, supports streaming with tool-call
//! reassembly, and includes the MiniMax XML tool-call shim.

use regex::Regex;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;

use super::common::{
    insert_opt, message_content_text, normalized_reasoning_effort, normalized_service_tier,
    should_pass_service_tier,
};
use crate::metrics::{extract_openapi_metrics, fill_missing_estimated_usage};
use crate::streaming::{
    next_provider_stream_chunk, read_provider_response_body, send_provider_request_first_response,
};
use crate::tura_llm::{
    default_client, estimate_context_utilization, normalize_response_content, CallMetrics,
    CallOptions, CostDetails, ProviderResponse, ProviderStreamEventSink, TuraError, UsageDetails,
};
use crate::utils::{deep_merge_json, strip_json_fence};

pub async fn embed(
    base_url: &str,
    model: &str,
    api_key: &str,
    text: &str,
) -> Result<Vec<f32>, TuraError> {
    let client = default_client(api_key)?;
    let url = format!("{}/embeddings", base_url.trim_end_matches('/'));
    let payload = json!({
        "model": model,
        "input": text,
    });
    let resp = send_provider_request_first_response(client.post(url).json(&payload)).await?;
    let status = resp.status();
    let data: Value = read_provider_response_body(resp.json()).await?;
    if !status.is_success() {
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body: data.to_string(),
        });
    }
    let embedding = data
        .pointer("/data/0/embedding")
        .and_then(Value::as_array)
        .ok_or_else(|| TuraError::ProviderRequest {
            provider: "openai-compatible".into(),
            message: "missing embedding vector".into(),
        })?;
    Ok(embedding
        .iter()
        .filter_map(Value::as_f64)
        .map(|v| v as f32)
        .collect())
}

pub async fn call(
    base_url: &str,
    model: &str,
    provider: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
) -> Result<ProviderResponse, TuraError> {
    call_with_stream_events(base_url, model, provider, api_key, messages, options, None).await
}

pub async fn call_with_stream_events(
    base_url: &str,
    model: &str,
    provider: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
    _stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    let client = default_client(api_key)?;
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let payload = build_chat_payload(provider, model, messages, options);

    if options.stream.unwrap_or(false) {
        return stream_call(base_url, &client, url, payload, options.context_window).await;
    }

    let resp = send_provider_request_first_response(client.post(url).json(&payload)).await?;
    let status = resp.status();
    let req_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let data: Value = read_provider_response_body(resp.json()).await?;
    if !status.is_success() {
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body: data.to_string(),
        });
    }

    let mut content = normalize_response_content(&data);
    if let Some(text) = content.as_str() {
        content = Value::String(strip_json_fence(text));
    }

    let mut metrics = extract_openapi_metrics(&data, options.context_window);
    metrics.provider_request_id = req_id;

    Ok(ProviderResponse {
        content,
        raw: data.clone(),
        metrics: Some(metrics),
    })
}

pub(crate) fn build_chat_payload(
    provider: &str,
    model: &str,
    messages: &[Value],
    options: &CallOptions,
) -> Value {
    let normalized_messages = normalize_messages_for_provider(provider, messages);

    let mut payload = json!({
        "model": model,
        "messages": normalized_messages,
        "temperature": options.temperature.unwrap_or(0.2),
    });

    if let Some(tools) = &options.tools {
        payload["tools"] = Value::Array(tools.clone());
    }
    if options.search {
        let tools = payload
            .get("tools")
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        let mut arr = tools.as_array().cloned().unwrap_or_default();
        if !arr
            .iter()
            .any(|t| t.get("type").and_then(Value::as_str) == Some("web_search"))
        {
            arr.push(json!({"type":"web_search"}));
        }
        payload["tools"] = Value::Array(arr);
    }

    insert_opt(&mut payload, "top_p", options.top_p.map(Value::from));
    insert_opt(&mut payload, "n", options.n.map(Value::from));
    insert_opt(&mut payload, "stop", options.stop.clone());
    insert_opt(
        &mut payload,
        "max_completion_tokens",
        options.max_completion_tokens.map(Value::from),
    );
    insert_opt(
        &mut payload,
        "max_tokens",
        options.max_tokens.map(Value::from),
    );
    insert_opt(
        &mut payload,
        "presence_penalty",
        options.presence_penalty.map(Value::from),
    );
    insert_opt(
        &mut payload,
        "frequency_penalty",
        options.frequency_penalty.map(Value::from),
    );
    insert_opt(&mut payload, "logit_bias", options.logit_bias.clone());
    insert_opt(&mut payload, "logprobs", options.logprobs.map(Value::from));
    insert_opt(
        &mut payload,
        "top_logprobs",
        options.top_logprobs.map(Value::from),
    );
    insert_opt(&mut payload, "seed", options.seed.map(Value::from));
    insert_opt(&mut payload, "user", options.user.clone().map(Value::from));
    insert_opt(
        &mut payload,
        "safety_identifier",
        options.safety_identifier.clone().map(Value::from),
    );
    insert_opt(
        &mut payload,
        "prompt_cache_key",
        options.prompt_cache_key.clone().map(Value::from),
    );
    insert_opt(
        &mut payload,
        "reasoning_effort",
        normalized_reasoning_effort(options).map(Value::from),
    );
    insert_opt(&mut payload, "prediction", options.prediction.clone());
    insert_opt(
        &mut payload,
        "modalities",
        options.modalities.clone().map(|v| json!(v)),
    );
    insert_opt(&mut payload, "audio", options.audio.clone());
    insert_opt(&mut payload, "stream", options.stream.map(Value::from));
    insert_opt(
        &mut payload,
        "stream_options",
        options.stream_options.clone(),
    );
    insert_opt(&mut payload, "store", options.store.map(Value::from));
    insert_opt(
        &mut payload,
        "metadata",
        options.metadata.clone().map(|v| json!(v)),
    );
    if should_pass_service_tier(provider, model) {
        insert_opt(
            &mut payload,
            "service_tier",
            normalized_service_tier(options).map(Value::from),
        );
    }
    insert_opt(
        &mut payload,
        "verbosity",
        options.verbosity.clone().map(Value::from),
    );
    insert_opt(
        &mut payload,
        "web_search_options",
        options.web_search_options.clone(),
    );
    insert_opt(&mut payload, "tool_choice", options.tool_choice.clone());
    insert_opt(
        &mut payload,
        "parallel_tool_calls",
        options.parallel_tool_calls.map(Value::from),
    );

    if let Some(extra_body) = &options.extra_body {
        deep_merge_json(&mut payload, extra_body.clone());
    }

    payload
}

async fn stream_call(
    _base_url: &str,
    client: &reqwest::Client,
    url: String,
    payload: Value,
    context_window: Option<u64>,
) -> Result<ProviderResponse, TuraError> {
    let resp = send_provider_request_first_response(client.post(url).json(&payload)).await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.map_err(|e| TuraError::Network {
            message: e.to_string(),
        })?;
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body,
        });
    }
    let mut stream = resp.bytes_stream();

    let mut full_content = String::new();
    let mut tool_calls = Vec::new();
    let mut stream_state = OpenAiCompatibleStreamState::default();
    let mut pending = String::new();
    let mut saw_output = false;
    let mut last_output_at = Instant::now();

    while let Some(chunk) =
        next_provider_stream_chunk(&mut stream, saw_output, last_output_at).await?
    {
        let data = chunk.map_err(|e| TuraError::Network {
            message: e.to_string(),
        })?;
        pending.push_str(&String::from_utf8_lossy(&data));

        while let Some(line_end) = pending.find('\n') {
            let line = pending[..line_end].trim_end_matches('\r').to_string();
            pending.drain(..=line_end);
            if process_openai_compatible_stream_line(
                &line,
                &mut full_content,
                &mut tool_calls,
                &mut stream_state,
            ) {
                saw_output = true;
                last_output_at = Instant::now();
            }
            if stream_state.stream_done {
                break;
            }
        }
        if stream_state.stream_done {
            break;
        }
    }
    if !pending.trim().is_empty() && !stream_state.stream_done {
        let line = pending.trim_end_matches('\r').to_string();
        let _ = process_openai_compatible_stream_line(
            &line,
            &mut full_content,
            &mut tool_calls,
            &mut stream_state,
        );
    }

    let content = if !full_content.is_empty() && !tool_calls.is_empty() {
        json!({
            "text": full_content,
            "tool_calls": tool_calls
        })
    } else if !full_content.is_empty() {
        Value::String(full_content)
    } else if !tool_calls.is_empty() {
        json!({ "tool_calls": tool_calls })
    } else if !stream_state.reasoning_buffer.is_empty() {
        // The model only emitted reasoning (no content / tool calls). Surface
        // it so the runtime sees a non-empty assistant turn instead of treating
        // the response as an empty failure.
        Value::String(std::mem::take(&mut stream_state.reasoning_buffer))
    } else {
        Value::Null
    };

    let mut metrics = if let Some(usage) = stream_state.stream_usage.clone() {
        let mut metrics = extract_openapi_metrics(&json!({ "usage": usage }), context_window);
        metrics.tool_call_count = tool_calls.len();
        metrics.finish_reason = stream_state.finish_reason.clone();
        metrics
    } else {
        CallMetrics {
            usage: UsageDetails {
                context_window,
                ..Default::default()
            },
            cost: CostDetails {
                currency: Some("USD".to_string()),
                ..Default::default()
            },
            cache_hit: false,
            cache_triggered_at_input_tokens: None,
            tool_call_count: tool_calls.len(),
            finish_reason: stream_state.finish_reason.clone(),
            provider_request_id: None,
            raw_usage: None,
        }
    };
    if stream_state.stream_usage.is_none() {
        fill_missing_estimated_usage(
            &mut metrics,
            &payload,
            &content,
            "codex_oauth_stream_returned_before_provider_usage",
        );
    }
    estimate_context_utilization(&mut metrics);

    Ok(ProviderResponse {
        content,
        raw: json!({ "tool_calls": tool_calls, "usage": stream_state.stream_usage }),
        metrics: Some(metrics),
    })
}

#[derive(Default)]
struct OpenAiCompatibleStreamState {
    tool_call_buffers: BTreeMap<String, StreamingToolCall>,
    finish_reason: Option<String>,
    completed_tool_call: bool,
    stream_usage: Option<Value>,
    stream_done: bool,
    /// Accumulated `delta.reasoning` text emitted by reasoning models
    /// (OpenRouter/DeepSeek-style `reasoning`, some providers use
    /// `reasoning_content`). Used both to keep the stream alive while the model
    /// is thinking (so the first-output timeout doesn't trip on a silent
    /// reasoning phase) and as a fallback when no `content`/tool calls arrive.
    reasoning_buffer: String,
}

/// Test-only: feed a single SSE `data:` line through the chat stream parser and
/// report whether it counted as output activity plus any accumulated reasoning.
#[cfg(test)]
pub(crate) fn process_chat_stream_line_for_test(line: &str) -> (bool, String, String) {
    let mut full = String::new();
    let mut tools = Vec::new();
    let mut state = OpenAiCompatibleStreamState::default();
    let event = process_openai_compatible_stream_line(line, &mut full, &mut tools, &mut state);
    (event, full, state.reasoning_buffer)
}

fn process_openai_compatible_stream_line(
    line: &str,
    full_content: &mut String,
    tool_calls: &mut Vec<Value>,
    state: &mut OpenAiCompatibleStreamState,
) -> bool {
    let Some(line) = line.trim_start().strip_prefix("data:") else {
        return false;
    };
    let line = line.trim_start();
    if line == "[DONE]" {
        state.stream_done = true;
        return false;
    }
    let Ok(delta) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    let mut output_event = false;
    if let Some(usage) = delta.get("usage").filter(|usage| !usage.is_null()) {
        state.stream_usage = Some(usage.clone());
    }
    if let Some(choice) = delta
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
    {
        if let Some(delta_content) = choice.get("delta").and_then(|d| d.get("content")) {
            if let Some(text) = delta_content
                .as_str()
                .filter(|_| !state.completed_tool_call)
            {
                if !text.is_empty() {
                    output_event = true;
                }
                full_content.push_str(text);
                if emit_minimax_streaming_tool_call(
                    full_content,
                    &mut state.tool_call_buffers,
                    tool_calls,
                ) {
                    state.completed_tool_call = true;
                }
            }
        }
        if let Some(reasoning) = choice
            .get("delta")
            .and_then(|d| d.get("reasoning").or_else(|| d.get("reasoning_content")))
            .and_then(Value::as_str)
        {
            // Reasoning tokens are real provider activity. Counting them keeps
            // the liveness timeout governed by the idle window (instead of the
            // long first-output window) so heavy "thinking" models such as
            // minimax-m2.7 and glm-5.1 are not retried into a hard timeout
            // while they reason before emitting content or tool calls.
            if !reasoning.is_empty() {
                output_event = true;
                state.reasoning_buffer.push_str(reasoning);
            }
        }
        if let Some(tool_calls_delta) = choice.get("delta").and_then(|d| d.get("tool_calls")) {
            if let Some(calls) = tool_calls_delta.as_array() {
                if !calls.is_empty() {
                    output_event = true;
                }
                for call in calls {
                    let key = call
                        .get("index")
                        .and_then(Value::as_u64)
                        .map(|index| index.to_string())
                        .or_else(|| {
                            call.get("id")
                                .and_then(Value::as_str)
                                .map(ToString::to_string)
                        })
                        .unwrap_or_else(|| "0".to_string());
                    let buffer = state.tool_call_buffers.entry(key).or_default();
                    if let Some(id) = call
                        .get("id")
                        .and_then(Value::as_str)
                        .filter(|id| !id.trim().is_empty())
                    {
                        buffer.id = Some(id.to_string());
                    }
                    if let Some(name) = call
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(Value::as_str)
                        .filter(|name| !name.trim().is_empty())
                    {
                        buffer.name = Some(name.to_string());
                    }
                    if let Some(args) = call
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(Value::as_str)
                    {
                        buffer.arguments.push_str(args);
                    }
                    if emit_completed_tool_call(buffer, tool_calls) {
                        state.completed_tool_call = true;
                    }
                }
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            if reason == "tool_calls" || reason == "stop" {
                for buffer in state.tool_call_buffers.values_mut() {
                    emit_completed_tool_call(buffer, tool_calls);
                }
            }
            state.finish_reason = Some(reason.to_string());
        }
    }
    output_event
}

#[derive(Default)]
pub(crate) struct StreamingToolCall {
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) arguments: String,
    pub(crate) emitted: bool,
}

pub(crate) fn emit_completed_tool_call(
    buffer: &mut StreamingToolCall,
    tool_calls: &mut Vec<Value>,
) -> bool {
    if buffer.arguments.trim().is_empty() {
        return false;
    }
    let Some(name) = buffer
        .name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
    else {
        return false;
    };

    if buffer.emitted {
        return false;
    }
    let Ok(arguments) = serde_json::from_str::<Value>(&buffer.arguments) else {
        return false;
    };

    push_streaming_tool_call(buffer, tool_calls, name, arguments);
    buffer.emitted = true;
    true
}

fn push_streaming_tool_call(
    buffer: &StreamingToolCall,
    tool_calls: &mut Vec<Value>,
    name: &str,
    arguments: Value,
) {
    let id = buffer
        .id
        .clone()
        .unwrap_or_else(|| format!("stream_tool_call_{}", tool_calls.len()));
    tool_calls.push(json!({
        "id": id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments
        }
    }));
}

fn emit_minimax_streaming_tool_call(
    content: &str,
    buffers: &mut BTreeMap<String, StreamingToolCall>,
    tool_calls: &mut Vec<Value>,
) -> bool {
    let Some((name, arguments)) = last_complete_minimax_invoke(content) else {
        return false;
    };
    let buffer = buffers.entry(format!("minimax_xml_{name}")).or_default();
    buffer.id = Some(format!("minimax_stream_tool_call_{}", tool_calls.len()));
    buffer.name = Some(name);
    buffer.arguments = arguments.to_string();
    emit_completed_tool_call(buffer, tool_calls)
}

pub(crate) fn last_complete_minimax_invoke(text: &str) -> Option<(String, Value)> {
    if !text.contains("<invoke") {
        return None;
    }
    let invoke_re = Regex::new(r#"(?s)<invoke\s+name=["']([^"']+)["']\s*>(.*?)</invoke>"#).ok()?;
    let param_re =
        Regex::new(r#"(?s)<parameter\s+name=["']([^"']+)["']\s*>(.*?)</parameter>"#).ok()?;
    let capture = invoke_re.captures_iter(text).last()?;
    let name = xml_unescape(
        capture
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default(),
    );
    if name.trim().is_empty() {
        return None;
    }
    let body = capture
        .get(2)
        .map(|value| value.as_str())
        .unwrap_or_default();
    let mut arguments = serde_json::Map::new();
    for parameter in param_re.captures_iter(body) {
        let key = xml_unescape(
            parameter
                .get(1)
                .map(|value| value.as_str())
                .unwrap_or_default(),
        );
        let value = xml_unescape(
            parameter
                .get(2)
                .map(|value| value.as_str())
                .unwrap_or_default(),
        )
        .trim()
        .to_string();
        arguments.insert(key, parse_minimax_parameter_value(&value));
    }
    Some((name, Value::Object(arguments)))
}

fn parse_minimax_parameter_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()))
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

pub(crate) async fn force_search(
    base_url: &str,
    model: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
) -> Result<ProviderResponse, TuraError> {
    let client = default_client(api_key)?;
    let url = format!("{}/responses", base_url.trim_end_matches('/'));
    let input = messages
        .iter()
        .map(|m| {
            format!(
                "{}: {}",
                m.get("role").and_then(Value::as_str).unwrap_or("user"),
                m.get("content").map(|v| v.to_string()).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut tools = options.tools.clone().unwrap_or_default();
    if !tools
        .iter()
        .any(|t| t.get("type").and_then(Value::as_str) == Some("web_search"))
    {
        tools.push(json!({"type":"web_search"}));
    }

    let mut payload = json!({
        "model": model,
        "input": input,
        "tools": tools,
    });
    insert_opt(
        &mut payload,
        "web_search_options",
        options.web_search_options.clone(),
    );
    insert_opt(&mut payload, "tool_choice", options.tool_choice.clone());
    insert_opt(
        &mut payload,
        "parallel_tool_calls",
        options.parallel_tool_calls.map(Value::from),
    );
    if let Some(extra_body) = &options.extra_body {
        deep_merge_json(&mut payload, extra_body.clone());
    }

    let resp = send_provider_request_first_response(client.post(url).json(&payload)).await?;
    let status = resp.status();
    let req_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let data: Value = read_provider_response_body(resp.json()).await?;
    if !status.is_success() {
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body: data.to_string(),
        });
    }

    let content = data.get("output").cloned().unwrap_or_else(|| data.clone());
    let mut metrics = extract_openapi_metrics(&data, options.context_window);
    metrics.provider_request_id = req_id;
    Ok(ProviderResponse {
        content,
        raw: data,
        metrics: Some(metrics),
    })
}

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
        if let Some(item) = normalize_responses_tool_item_for_chat(message) {
            normalized.push(item);
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

        let content = message_content_text(message.get("content"))
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());

        match role {
            "assistant" => {
                if tool_calls.is_none() && content.is_none() {
                    continue;
                }
                let mut item = json!({
                    "role": "assistant",
                    "content": content.unwrap_or_default(),
                });
                if let Some(tool_calls) = tool_calls {
                    item["tool_calls"] = tool_calls;
                }
                normalized.push(item);
            }
            "tool" => {
                let Some(content) = content else {
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
                let Some(content) = content else {
                    continue;
                };
                normalized.push(json!({
                    "role": "system",
                    "content": content,
                }));
            }
            _ => {
                let Some(content) = content else {
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

fn normalize_responses_tool_item_for_chat(message: &Value) -> Option<Value> {
    match message.get("type").and_then(Value::as_str)? {
        "function_call" => {
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
            Some(json!({
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
            }))
        }
        "function_call_output" => {
            let call_id = message
                .get("call_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("call_command_run");
            let output = message_content_text(message.get("output"))
                .or_else(|| message_content_text(message.get("content")))
                .unwrap_or_default();
            Some(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": output,
            }))
        }
        _ => None,
    }
}
