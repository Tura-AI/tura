use regex::Regex;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;

use crate::metrics::{extract_openapi_metrics, fill_missing_estimated_usage};
use crate::streaming::{next_provider_stream_chunk, send_provider_request_first_response};
use crate::tura_llm::{
    default_client, estimate_context_utilization, normalize_response_content, CallMetrics,
    CallOptions, CostDetails, ProviderResponse, ProviderStreamEvent, ProviderStreamEventSink,
    TuraError, UsageDetails,
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
    let data: Value = resp.json().await.map_err(|e| TuraError::Network {
        message: e.to_string(),
    })?;
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
    let data: Value = resp.json().await.map_err(|e| TuraError::Network {
        message: e.to_string(),
    })?;
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

fn build_chat_payload(
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

pub(crate) async fn codex_oauth_call(
    model: &str,
    access_token: &str,
    messages: &[Value],
    options: &CallOptions,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|err| TuraError::Network {
            message: err.to_string(),
        })?;
    let payload = build_codex_oauth_payload(model, messages, options);

    let mut request = client
        .post(openai_codex_endpoint())
        .bearer_auth(access_token)
        .header("originator", "codex_cli_rs")
        .header("User-Agent", codex_cli_user_agent())
        .header("session_id", "tura-codex-validation")
        .json(&payload);
    if let Ok(account_id) = std::env::var("OPENAI_ACCOUNT_ID") {
        if !account_id.trim().is_empty() {
            request = request.header("ChatGPT-Account-Id", account_id);
        }
    }

    let resp = send_provider_request_first_response(request).await?;
    let status = resp.status();
    let req_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if !status.is_success() {
        let body = resp.text().await.map_err(|err| TuraError::Network {
            message: err.to_string(),
        })?;
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body,
        });
    }

    let data = parse_codex_response_stream(resp, stream_events).await?;
    let mut content = normalize_codex_response_content(&data);
    if let Some(text) = content.as_str() {
        content = Value::String(strip_json_fence(text));
    }
    let mut metrics = extract_openapi_metrics(&data, options.context_window);
    fill_missing_estimated_usage(
        &mut metrics,
        &payload,
        &content,
        "codex_oauth_stream_returned_before_provider_usage",
    );
    metrics.cost = CostDetails::default();
    metrics.provider_request_id = req_id;
    Ok(ProviderResponse {
        content,
        raw: data,
        metrics: Some(metrics),
    })
}

fn codex_cli_user_agent() -> String {
    format!(
        "codex_cli_rs/{} ({}; {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn build_codex_oauth_payload(model: &str, messages: &[Value], options: &CallOptions) -> Value {
    let mut input = Vec::new();
    let mut instructions = "Follow the user request and answer concisely.".to_string();
    for (index, message) in messages.iter().enumerate() {
        if matches!(
            message.get("type").and_then(Value::as_str),
            Some("function_call" | "function_call_output")
        ) {
            input.push(message.clone());
            continue;
        }
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        let content = message_content_text(message.get("content")).unwrap_or_default();
        if index == 0 && matches!(role, "system" | "developer") && !content.trim().is_empty() {
            instructions = content;
            continue;
        }
        input.push(json!({
            "role": codex_input_role(role),
            "content": content,
        }));
    }
    let mut payload = json!({
        "model": model,
        "instructions": instructions,
        "store": false,
        "stream": true,
        "input": if input.is_empty() {
            vec![json!({"role": "user", "content": ""})]
        } else {
            input
        },
    });
    if let Some(tools) = &options.tools {
        payload["tools"] = Value::Array(tools.iter().map(codex_tool_schema).collect());
    }
    if let Some(reasoning_effort) = normalized_reasoning_effort(options) {
        payload["reasoning"] = json!({ "effort": reasoning_effort });
        payload["include"] = json!(["reasoning.encrypted_content"]);
    }
    payload["tool_choice"] = options
        .tool_choice
        .as_ref()
        .map(normalize_codex_tool_choice)
        .unwrap_or_else(|| Value::String("auto".to_string()));
    insert_opt(
        &mut payload,
        "parallel_tool_calls",
        options.parallel_tool_calls.map(Value::from),
    );
    insert_opt(
        &mut payload,
        "prompt_cache_key",
        options.prompt_cache_key.clone().map(Value::from),
    );
    insert_opt(
        &mut payload,
        "metadata",
        options.metadata.clone().map(|v| json!(v)),
    );
    insert_opt(
        &mut payload,
        "service_tier",
        normalized_service_tier(options).map(Value::from),
    );
    if let Some(extra_body) = &options.extra_body {
        deep_merge_json(&mut payload, extra_body.clone());
    }

    payload
}

async fn parse_codex_response_stream(
    resp: reqwest::Response,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<Value, TuraError> {
    let mut stream = resp.bytes_stream();
    let mut pending = String::new();
    let mut output_text = String::new();
    let mut completed = None;
    let mut events = Vec::new();
    let mut command_collector = CodexCommandRunCommandCollector::default();
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
            if process_codex_sse_line(
                &line,
                &mut output_text,
                &mut completed,
                &mut events,
                &mut command_collector,
                stream_events.as_ref(),
            )? {
                saw_output = true;
                last_output_at = Instant::now();
            }
        }
    }

    if !pending.trim().is_empty() {
        let _ = process_codex_sse_line(
            &pending,
            &mut output_text,
            &mut completed,
            &mut events,
            &mut command_collector,
            stream_events.as_ref(),
        )?;
    }

    Ok(build_codex_stream_root(output_text, completed, events))
}

fn process_codex_sse_line(
    line: &str,
    output_text: &mut String,
    completed: &mut Option<Value>,
    events: &mut Vec<Value>,
    command_collector: &mut CodexCommandRunCommandCollector,
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
    append_codex_stream_text(&value, output_text);
    if let Some(response) = value.get("response") {
        *completed = Some(response.clone());
    }
    let output_event = is_codex_stream_output_start(&value);
    if let Some(sink) = stream_events {
        if output_event {
            sink(ProviderStreamEvent::ProviderOutputStarted);
        }
        for event in command_collector.push_event(&value) {
            sink(event);
        }
    }
    events.push(value);

    Ok(output_event)
}

fn is_codex_stream_output_start(value: &Value) -> bool {
    matches!(
        value.get("type").and_then(Value::as_str),
        Some(
            "response.output_text.delta"
                | "response.function_call_arguments.delta"
                | "response.output_item.added"
                | "response.content_part.added"
        )
    )
}

fn append_codex_stream_text(value: &Value, output_text: &mut String) {
    if matches!(
        value.get("type").and_then(Value::as_str),
        Some(
            "response.function_call_arguments.delta"
                | "response.function_call_arguments.done"
                | "response.output_item.added"
                | "response.output_item.done"
        )
    ) {
        return;
    }

    if let Some(delta) = value
        .get("type")
        .and_then(Value::as_str)
        .filter(|event_type| event_type.ends_with(".delta"))
        .and_then(|_| value.get("delta").and_then(Value::as_str))
    {
        output_text.push_str(delta);
        return;
    }

    if let Some(delta) = value
        .get("delta")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/response/delta").and_then(Value::as_str))
    {
        output_text.push_str(delta);
    }
}

fn build_codex_stream_root(
    output_text: String,
    completed: Option<Value>,
    events: Vec<Value>,
) -> Value {
    let mut root = completed.unwrap_or_else(|| json!({ "output": [] }));
    if !output_text.is_empty() {
        root["output_text"] = Value::String(output_text);
    }
    root["events"] = Value::Array(events);
    root
}

fn openai_codex_endpoint() -> String {
    std::env::var("OPENAI_CODEX_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://chatgpt.com/backend-api/codex/responses".to_string())
}

fn codex_input_role(role: &str) -> &str {
    match role {
        "assistant" => "assistant",
        "system" => "system",
        "developer" => "developer",
        _ => "user",
    }
}

fn normalize_codex_response_content(data: &Value) -> Value {
    let tool_calls = complete_codex_tool_calls(data);
    if !tool_calls.is_empty() {
        let mut object = serde_json::Map::new();
        if let Some(text) = data.get("output_text").and_then(Value::as_str) {
            if !text.trim().is_empty() {
                object.insert("text".to_string(), Value::String(text.to_string()));
            }
        }
        object.insert("tool_calls".to_string(), Value::Array(tool_calls));
        return Value::Object(object);
    }

    if let Some(text) = data.get("output_text").and_then(Value::as_str) {
        return Value::String(text.to_string());
    }
    if let Some(text) = data
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|item| {
            item.get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .find_map(|content| {
            content
                .get("text")
                .and_then(Value::as_str)
                .or_else(|| content.get("content").and_then(Value::as_str))
        })
    {
        return Value::String(text.to_string());
    }
    normalize_response_content(data)
}

fn codex_tool_schema(tool: &Value) -> Value {
    if tool.get("name").and_then(Value::as_str).is_some() {
        return tool.clone();
    }

    let Some(function) = tool.get("function").and_then(Value::as_object) else {
        return tool.clone();
    };
    let mut converted = serde_json::Map::new();
    converted.insert(
        "type".to_string(),
        tool.get("type")
            .cloned()
            .unwrap_or_else(|| Value::String("function".to_string())),
    );
    for key in ["name", "description", "parameters", "strict"] {
        if let Some(value) = function.get(key) {
            converted.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(converted)
}

fn normalize_codex_tool_choice(tool_choice: &Value) -> Value {
    if tool_choice.get("type").and_then(Value::as_str) == Some("function") {
        if let Some(name) = tool_choice
            .get("function")
            .and_then(|function| function.get("name"))
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
        {
            return json!({
                "type": "function",
                "name": name,
            });
        }
    }
    tool_choice.clone()
}

fn extract_codex_tool_calls(data: &Value) -> Vec<Value> {
    let mut calls = Vec::new();
    if let Some(output) = data.get("output").and_then(Value::as_array) {
        for item in output {
            if let Some(call) = codex_output_item_tool_call(item) {
                calls.push(call);
            }
        }
    }
    if let Some(events) = data.get("events").and_then(Value::as_array) {
        calls.extend(codex_event_tool_calls(events));
    }
    dedupe_tool_calls(calls)
}

fn complete_codex_tool_calls(data: &Value) -> Vec<Value> {
    extract_codex_tool_calls(data)
        .into_iter()
        .filter_map(ready_streaming_tool_call)
        .collect()
}

fn ready_streaming_tool_call(call: Value) -> Option<Value> {
    let name = call
        .get("function")
        .and_then(|function| function.get("name"))
        .and_then(Value::as_str)?;
    let arguments = call
        .get("function")
        .and_then(|function| function.get("arguments"))?
        .clone();

    if name == "command_run" {
        let text = match &arguments {
            Value::String(text) => text.as_str(),
            other => return Some(call_with_arguments(call, other.clone())),
        };
        if let Ok(arguments) = serde_json::from_str::<Value>(text) {
            return Some(call_with_arguments(call, arguments));
        }
        return None;
    }

    tool_call_arguments_complete(&arguments).then_some(call)
}

fn call_with_arguments(mut call: Value, arguments: Value) -> Value {
    if let Some(function) = call.get_mut("function").and_then(Value::as_object_mut) {
        function.insert("arguments".to_string(), arguments);
    }
    call
}

fn tool_call_arguments_complete(arguments: &Value) -> bool {
    match arguments {
        Value::String(text) => serde_json::from_str::<Value>(text).is_ok(),
        Value::Object(_) => true,
        _ => false,
    }
}

fn codex_output_item_tool_call(item: &Value) -> Option<Value> {
    let item_type = item.get("type").and_then(Value::as_str)?;
    if !matches!(item_type, "function_call" | "tool_call") {
        return None;
    }
    let name = item.get("name").and_then(Value::as_str)?;
    let arguments = item
        .get("arguments")
        .cloned()
        .or_else(|| item.get("input").cloned())
        .unwrap_or_else(|| json!({}));
    let arguments = match arguments {
        Value::String(text) => Value::String(text),
        other => Value::String(other.to_string()),
    };
    let id = item
        .get("call_id")
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("codex_tool_call");
    Some(codex_tool_call_value(id, name, arguments))
}

fn codex_event_tool_calls(events: &[Value]) -> Vec<Value> {
    let mut collector = CodexToolCallStreamCollector::default();
    let mut calls = Vec::new();
    for event in events {
        calls.extend(collector.push_event(event));
    }
    calls.extend(collector.finish());
    calls
}

#[derive(Default)]
struct CodexToolCallStreamCollector {
    active: Option<String>,
    entries: Vec<CodexToolCallEntry>,
}

#[derive(Default)]
struct CodexToolCallEntry {
    id: String,
    call_id: String,
    name: String,
    arguments: String,
    emitted: bool,
}

impl CodexToolCallStreamCollector {
    fn push_event(&mut self, event: &Value) -> Vec<Value> {
        if let Some(item) = event.get("item") {
            if item.get("type").and_then(Value::as_str) == Some("function_call") {
                self.upsert_item(item);
            }
        }

        match event.get("type").and_then(Value::as_str) {
            Some("response.function_call_arguments.delta") => {
                if let (Some(id), Some(delta)) = (
                    self.event_tool_id(event),
                    event.get("delta").and_then(Value::as_str),
                ) {
                    if let Some(entry) = self.entry_mut(&id) {
                        entry.arguments.push_str(delta);
                    }
                }
                Vec::new()
            }
            Some("response.function_call_arguments.done") => {
                let id = self.event_tool_id(event);
                if let (Some(id), Some(arguments)) =
                    (id, event.get("arguments").and_then(Value::as_str))
                {
                    if let Some(entry) = self.entry_mut(&id) {
                        entry.arguments = arguments.to_string();
                    }
                    return self.emit_ready(&id);
                }
                Vec::new()
            }
            Some("response.output_item.done") => self
                .active
                .clone()
                .map(|id| self.emit_ready(&id))
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn finish(&mut self) -> Vec<Value> {
        let ids = self
            .entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<Vec<_>>();
        ids.into_iter()
            .flat_map(|id| self.emit_ready(&id))
            .collect()
    }

    fn upsert_item(&mut self, item: &Value) {
        let id = item
            .get("id")
            .or_else(|| item.get("call_id"))
            .and_then(Value::as_str)
            .unwrap_or("codex_tool_call")
            .to_string();
        let call_id = item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .unwrap_or(id.as_str())
            .to_string();
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let arguments = item
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.active = Some(id.clone());
        if let Some(entry) = self.entry_mut(&id) {
            if !call_id.is_empty() {
                entry.call_id = call_id;
            }
            if !name.is_empty() {
                entry.name = name;
            }
            if !arguments.is_empty() {
                entry.arguments = arguments;
            }
        } else {
            self.entries.push(CodexToolCallEntry {
                id,
                call_id,
                name,
                arguments,
                emitted: false,
            });
        }
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut CodexToolCallEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id == id || entry.call_id == id)
    }

    fn event_tool_id(&self, event: &Value) -> Option<String> {
        event
            .get("item_id")
            .or_else(|| event.get("call_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| self.active.clone())
    }

    fn emit_ready(&mut self, id: &str) -> Vec<Value> {
        let Some(entry) = self.entry_mut(id) else {
            return Vec::new();
        };
        if entry.emitted
            || entry.name.is_empty()
            || serde_json::from_str::<Value>(&entry.arguments).is_err()
        {
            return Vec::new();
        }
        entry.emitted = true;
        let call = codex_tool_call_value(
            &entry.call_id,
            &entry.name,
            Value::String(entry.arguments.clone()),
        );
        ready_streaming_tool_call(call).into_iter().collect()
    }
}

#[derive(Default)]
struct CodexCommandRunCommandCollector {
    active: Option<String>,
    entries: Vec<CodexCommandRunCommandEntry>,
}

#[derive(Default)]
struct CodexCommandRunCommandEntry {
    id: String,
    call_id: String,
    name: String,
    arguments: String,
    emitted_commands: usize,
}

impl CodexCommandRunCommandCollector {
    fn push_event(&mut self, event: &Value) -> Vec<ProviderStreamEvent> {
        if let Some(item) = event.get("item") {
            if item.get("type").and_then(Value::as_str) == Some("function_call") {
                self.upsert_item(item);
            }
        }

        match event.get("type").and_then(Value::as_str) {
            Some("response.function_call_arguments.delta") => {
                if let (Some(id), Some(delta)) = (
                    self.event_tool_id(event),
                    event.get("delta").and_then(Value::as_str),
                ) {
                    if let Some(entry) = self.entry_mut(&id) {
                        entry.arguments.push_str(delta);
                        return Self::emit_ready_commands(entry);
                    }
                }
                Vec::new()
            }
            Some("response.function_call_arguments.done") => {
                if let (Some(id), Some(arguments)) = (
                    self.event_tool_id(event),
                    event.get("arguments").and_then(Value::as_str),
                ) {
                    if let Some(entry) = self.entry_mut(&id) {
                        entry.arguments = arguments.to_string();
                        return Self::emit_ready_commands(entry);
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn upsert_item(&mut self, item: &Value) {
        let id = item
            .get("id")
            .or_else(|| item.get("call_id"))
            .and_then(Value::as_str)
            .unwrap_or("codex_tool_call")
            .to_string();
        let call_id = item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .unwrap_or(id.as_str())
            .to_string();
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let arguments = item
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.active = Some(id.clone());
        if let Some(entry) = self.entry_mut(&id) {
            if !call_id.is_empty() {
                entry.call_id = call_id;
            }
            if !name.is_empty() {
                entry.name = name;
            }
            if !arguments.is_empty() {
                entry.arguments = arguments;
            }
        } else {
            self.entries.push(CodexCommandRunCommandEntry {
                id,
                call_id,
                name,
                arguments,
                emitted_commands: 0,
            });
        }
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut CodexCommandRunCommandEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id == id || entry.call_id == id)
    }

    fn event_tool_id(&self, event: &Value) -> Option<String> {
        event
            .get("item_id")
            .or_else(|| event.get("call_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| self.active.clone())
    }

    fn emit_ready_commands(entry: &mut CodexCommandRunCommandEntry) -> Vec<ProviderStreamEvent> {
        if entry.name != "command_run" {
            return Vec::new();
        }
        let commands = complete_command_run_command_objects(&entry.arguments);
        if commands.len() <= entry.emitted_commands {
            return Vec::new();
        }
        let start = entry.emitted_commands;
        entry.emitted_commands = commands.len();
        commands
            .into_iter()
            .enumerate()
            .skip(start)
            .map(
                |(command_index, command)| ProviderStreamEvent::CommandRunCommandReady {
                    tool_call_id: entry.call_id.clone(),
                    command_index,
                    command,
                },
            )
            .collect()
    }
}

fn complete_command_run_command_objects(arguments: &str) -> Vec<Value> {
    let Some(array_start) = find_commands_array_start(arguments) else {
        return Vec::new();
    };
    let mut commands = Vec::new();
    let mut in_string = false;
    let mut escape = false;
    let mut depth = 0_i32;
    let mut object_start = None;

    for (offset, ch) in arguments[array_start + 1..].char_indices() {
        let index = array_start + 1 + offset;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    object_start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(start) = object_start.take() {
                            if let Ok(value) =
                                serde_json::from_str::<Value>(&arguments[start..=index])
                            {
                                commands.push(value);
                            }
                        }
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }

    commands
}

fn find_commands_array_start(arguments: &str) -> Option<usize> {
    let mut in_string = false;
    let mut escape = false;
    let mut key_start = None;
    let mut last_key = None::<String>;

    for (index, ch) in arguments.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
                if let Some(start) = key_start.take() {
                    if let Ok(key) = serde_json::from_str::<String>(&arguments[start..=index]) {
                        last_key = Some(key);
                    }
                }
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                key_start = Some(index);
            }
            '[' if last_key.as_deref() == Some("commands") => return Some(index),
            ':' | ' ' | '\n' | '\r' | '\t' => {}
            _ => {
                if ch != ',' {
                    last_key = None;
                }
            }
        }
    }
    None
}

fn codex_tool_call_value(id: &str, name: &str, arguments: Value) -> Value {
    json!({
        "id": id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        }
    })
}

fn dedupe_tool_calls(calls: Vec<Value>) -> Vec<Value> {
    let mut positions = std::collections::HashMap::<String, usize>::new();
    let mut unique = Vec::new();
    for call in calls {
        let key = call
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| call.to_string());
        if let Some(index) = positions.get(&key).copied() {
            if tool_call_argument_score(&call) >= tool_call_argument_score(&unique[index]) {
                unique[index] = call;
            }
        } else {
            positions.insert(key, unique.len());
            unique.push(call);
        }
    }
    unique
}

fn tool_call_argument_score(call: &Value) -> usize {
    let Some(arguments) = call
        .get("function")
        .and_then(|function| function.get("arguments"))
    else {
        return 0;
    };
    match arguments {
        Value::String(text) => text.trim().len(),
        Value::Object(object) => object.len().max(1),
        Value::Array(array) => array.len().max(1),
        _ => 0,
    }
}

fn message_content_text(content: Option<&Value>) -> Option<String> {
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
struct StreamingToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    emitted: bool,
}

fn emit_completed_tool_call(buffer: &mut StreamingToolCall, tool_calls: &mut Vec<Value>) -> bool {
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

fn last_complete_minimax_invoke(text: &str) -> Option<(String, Value)> {
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
    let data: Value = resp.json().await.map_err(|e| TuraError::Network {
        message: e.to_string(),
    })?;
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

fn insert_opt(payload: &mut Value, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        payload[key] = value;
    }
}

fn should_pass_service_tier(provider: &str, model: &str) -> bool {
    if !provider.eq_ignore_ascii_case("openai") {
        return false;
    }
    let model = model.to_ascii_lowercase();
    model.starts_with("gpt-") || model.starts_with("o") || model.contains("codex")
}

fn normalized_reasoning_effort(options: &CallOptions) -> Option<String> {
    normalized_non_default_option(options.reasoning_effort.as_deref()).map(|value| {
        if value.eq_ignore_ascii_case("highest") {
            "xhigh".to_string()
        } else {
            value
        }
    })
}

fn normalized_service_tier(options: &CallOptions) -> Option<String> {
    normalized_non_default_option(options.service_tier.as_deref())
}

fn normalized_non_default_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
        .map(ToString::to_string)
}

fn normalize_messages_for_provider(provider: &str, messages: &[Value]) -> Vec<Value> {
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
        let content = message_content_text(message.get("content"))
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        let Some(content) = content else {
            continue;
        };

        let (role, content) = match role {
            "assistant" => ("assistant", content),
            "tool" => ("user", format!("Tool result:\n{content}")),
            "user" => ("user", content),
            "system" => ("user", format!("System instruction:\n{content}")),
            other => ("user", format!("{other} message:\n{content}")),
        };

        normalized.push(json!({
            "role": role,
            "content": content,
        }));
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

#[cfg(test)]
mod tests {
    use super::{
        build_chat_payload, build_codex_oauth_payload, normalize_messages_for_provider,
        should_pass_service_tier,
    };
    use crate::tura_llm::CallOptions;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{mpsc, OnceLock};
    use tokio::sync::Mutex;

    async fn codex_endpoint_env_lock() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().await
    }

    #[test]
    fn openai_compatible_chat_messages_drop_empty_content_and_fold_system_into_user() {
        let messages = vec![
            json!({"role": "system", "content": "Use tools carefully."}),
            json!({"role": "assistant", "content": null}),
            json!({"role": "user", "content": "Inspect files."}),
        ];

        let normalized = normalize_messages_for_provider("minimax", &messages);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["role"], "user");
        assert_eq!(
            normalized[0]["content"],
            "System instruction:\nUse tools carefully."
        );
        assert_eq!(normalized[1]["role"], "user");
        assert_eq!(normalized[1]["content"], "Inspect files.");
    }

    #[test]
    fn openai_compatible_chat_messages_preserve_tool_call_and_output_pairs() {
        let messages = vec![
            json!({
                "type": "function_call",
                "name": "command_run",
                "call_id": "call_abc",
                "arguments": "{\"commands\":[]}",
                "status": "completed"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_abc",
                "output": "Exit code: 0\nOutput:\nTURA_PROBE_OK\n"
            }),
        ];

        let normalized = normalize_messages_for_provider("minimax", &messages);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["role"], "assistant");
        assert_eq!(normalized[0]["tool_calls"][0]["id"], "call_abc");
        assert_eq!(
            normalized[0]["tool_calls"][0]["function"]["name"],
            "command_run"
        );
        assert_eq!(normalized[1]["role"], "tool");
        assert_eq!(normalized[1]["tool_call_id"], "call_abc");
        let content = normalized[1]["content"]
            .as_str()
            .expect("normalized tool content should be a string");
        assert!(content.contains("TURA_PROBE_OK"));
    }

    #[test]
    fn non_minimax_keeps_assistant_empty_content_for_openai_compatibility() {
        let messages = vec![json!({"role": "assistant", "content": null})];

        let normalized = normalize_messages_for_provider("openai", &messages);

        assert_eq!(normalized[0]["role"], "assistant");
        assert_eq!(normalized[0]["content"], "");
    }

    #[test]
    fn service_tier_is_limited_to_openai_gpt_family_models() {
        assert!(should_pass_service_tier("openai", "gpt-5.2"));
        assert!(should_pass_service_tier("openai", "o3"));
        assert!(should_pass_service_tier("openai", "gpt-5.3-codex"));
        assert!(!should_pass_service_tier("openrouter", "openai/gpt-5.2"));
        assert!(!should_pass_service_tier("minimax", "minimax-m2.5"));
    }

    #[test]
    fn provider_payload_passes_reasoning_and_acceleration_for_openai_gpt() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("high".to_string()),
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };

        let payload = build_chat_payload("openai", "gpt-5.2", &messages, &options);

        assert_eq!(payload["reasoning_effort"], "high");
        assert_eq!(payload["service_tier"], "priority");
    }

    #[test]
    fn provider_payload_maps_highest_reasoning_to_xhigh() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("highest".to_string()),
            ..CallOptions::default()
        };

        let payload = build_chat_payload("openai", "gpt-5.2", &messages, &options);

        assert_eq!(payload["reasoning_effort"], "xhigh");
    }

    #[test]
    fn provider_payload_omits_default_reasoning_and_acceleration() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some(" default ".to_string()),
            service_tier: Some("default".to_string()),
            ..CallOptions::default()
        };

        let payload = build_chat_payload("openai", "gpt-5.2", &messages, &options);

        assert!(payload.get("reasoning_effort").is_none());
        assert!(payload.get("service_tier").is_none());
    }

    #[test]
    fn provider_payload_does_not_pass_acceleration_to_non_openai_models() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("medium".to_string()),
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };

        let payload = build_chat_payload("minimax", "minimax-m2.5", &messages, &options);

        assert_eq!(payload["reasoning_effort"], "medium");
        assert!(payload.get("service_tier").is_none());
    }

    #[tokio::test]
    async fn direct_provider_call_sends_reasoning_and_acceleration() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        let body = String::from_utf8(
                            buffer[body_start..body_start + content_length].to_vec(),
                        )
                        .expect("utf8 body");
                        tx.send(body).expect("send request body");
                        break;
                    }
                }
            }

            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: application/json\r\n",
                "Content-Length: 69\r\n",
                "\r\n",
                r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"total_tokens":1}}"#
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("high".to_string()),
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };

        super::call(
            &format!("http://{addr}"),
            "gpt-5.2",
            "openai",
            "test-key",
            &messages,
            &options,
        )
        .await
        .expect("provider call");

        let body: serde_json::Value =
            serde_json::from_str(&rx.recv().expect("request body")).expect("json body");
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["service_tier"], "priority");
    }

    #[tokio::test]
    async fn streaming_provider_drains_usage_after_tool_arguments_complete() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        break;
                    }
                }
            }

            let first = json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "grep",
                                "arguments": "{\"pattern\""
                            }
                        }]
                    }
                }]
            });
            let second = json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "function": {
                                "arguments": ":\"foo\"}"
                            }
                        }]
                    }
                }]
            });
            let late_text = json!({
                "choices": [{
                    "delta": {
                        "content": "late text after tool call"
                    }
                }]
            });
            let usage = json!({
                "choices": [],
                "usage": {
                    "prompt_tokens": 3000,
                    "completion_tokens": 8,
                    "total_tokens": 3008,
                    "prompt_tokens_details": {"cached_tokens": 2048}
                }
            });
            let body =
                format!("data: {first}\n\ndata: {second}\n\ndata: {late_text}\n\ndata: {usage}\n\ndata: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "search"})];
        let options = CallOptions {
            stream: Some(true),
            ..CallOptions::default()
        };

        let result = super::call(
            &format!("http://{addr}"),
            "gpt-test",
            "openai",
            "test-key",
            &messages,
            &options,
        )
        .await
        .expect("provider call");

        assert_eq!(result.content["tool_calls"][0]["function"]["name"], "grep");
        assert_eq!(
            result.content["tool_calls"][0]["function"]["arguments"]["pattern"],
            "foo"
        );
        assert!(!result
            .content
            .to_string()
            .contains("late text after tool call"));
        let metrics = result.metrics.expect("metrics");
        assert_eq!(metrics.usage.input_tokens, Some(3000));
        assert_eq!(metrics.usage.cached_input_tokens, Some(2048));
        assert!(metrics.cache_hit);
    }

    #[tokio::test]
    async fn streaming_provider_reads_usage_and_cached_tokens() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        let body = String::from_utf8(
                            buffer[body_start..body_start + content_length].to_vec(),
                        )
                        .expect("utf8 body");
                        tx.send(body).expect("send request body");
                        break;
                    }
                }
            }

            let content = json!({
                "choices": [{
                    "delta": {"content": "ok"}
                }]
            });
            let usage = json!({
                "choices": [],
                "usage": {
                    "prompt_tokens": 3000,
                    "completion_tokens": 3,
                    "total_tokens": 3003,
                    "prompt_tokens_details": {"cached_tokens": 2048}
                }
            });
            let body = format!("data: {content}\n\ndata: {usage}\n\ndata: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "cache"})];
        let options = CallOptions {
            stream: Some(true),
            stream_options: Some(json!({ "include_usage": true })),
            ..CallOptions::default()
        };

        let result = super::call(
            &format!("http://{addr}"),
            "gpt-test",
            "openai",
            "test-key",
            &messages,
            &options,
        )
        .await
        .expect("provider call");

        let request_body: serde_json::Value =
            serde_json::from_str(&rx.recv().expect("request body")).expect("json body");
        assert_eq!(request_body["stream_options"]["include_usage"], true);
        let metrics = result.metrics.expect("metrics");
        assert_eq!(metrics.usage.input_tokens, Some(3000));
        assert_eq!(metrics.usage.output_tokens, Some(3));
        assert_eq!(metrics.usage.cached_input_tokens, Some(2048));
        assert!(metrics.cache_hit);
    }

    #[test]
    fn qwen_stream_options_request_usage_for_cache_accounting() {
        let payload = super::build_chat_payload(
            "qwen",
            "qwen3-max-2026-01-23",
            &[json!({"role": "user", "content": "cache"})],
            &CallOptions {
                stream: Some(true),
                stream_options: Some(json!({ "include_usage": true })),
                ..CallOptions::default()
            },
        );

        assert_eq!(payload["stream"], true);
        assert_eq!(payload["stream_options"]["include_usage"], true);
    }

    #[test]
    fn metrics_read_minimax_anthropic_cache_usage_fields() {
        let metrics = crate::metrics::extract_openapi_metrics(
            &json!({
                "usage": {
                    "input_tokens": 108,
                    "output_tokens": 91,
                    "cache_creation_input_tokens": 512,
                    "cache_read_input_tokens": 14813
                }
            }),
            None,
        );

        assert_eq!(metrics.usage.input_tokens, Some(108));
        assert_eq!(metrics.usage.output_tokens, Some(91));
        assert_eq!(metrics.usage.cached_input_tokens, Some(14813));
        assert_eq!(metrics.usage.cache_write_tokens, Some(512));
        assert_eq!(metrics.usage.total_tokens, Some(15524));
        assert!(metrics.cache_hit);
    }

    #[tokio::test]
    async fn codex_oauth_call_sends_responses_reasoning_and_acceleration() {
        let _env_guard = codex_endpoint_env_lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = format!("http://{addr}/backend-api/codex/responses");
        let previous_endpoint = std::env::var_os("OPENAI_CODEX_ENDPOINT");
        std::env::set_var("OPENAI_CODEX_ENDPOINT", &endpoint);
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        let body = String::from_utf8(
                            buffer[body_start..body_start + content_length].to_vec(),
                        )
                        .expect("utf8 body");
                        tx.send(body).expect("send request body");
                        break;
                    }
                }
            }

            let body = concat!(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n",
                "data: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"ok\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n",
                "data: [DONE]\n\n"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("high".to_string()),
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };

        let result =
            super::codex_oauth_call("gpt-5.1-codex", "test-token", &messages, &options, None).await;

        match previous_endpoint {
            Some(value) => std::env::set_var("OPENAI_CODEX_ENDPOINT", value),
            None => std::env::remove_var("OPENAI_CODEX_ENDPOINT"),
        }

        result.expect("codex oauth call");
        let body: serde_json::Value =
            serde_json::from_str(&rx.recv().expect("request body")).expect("json body");
        assert!(body.get("reasoning_effort").is_none());
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["service_tier"], "priority");
    }

    #[tokio::test]
    async fn codex_oauth_stream_reads_completed_usage_after_tool_call() {
        let _env_guard = codex_endpoint_env_lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = format!("http://{addr}/backend-api/codex/responses");
        let previous_endpoint = std::env::var_os("OPENAI_CODEX_ENDPOINT");
        std::env::set_var("OPENAI_CODEX_ENDPOINT", &endpoint);

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        break;
                    }
                }
            }

            let args = r#"{"commands":[{"step":1,"command":"rg","command_line":"rg -n bug ."}]}"#;
            let tool_event = json!({
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "command_run",
                    "arguments": args
                }
            });
            let completed = json!({
                "type": "response.completed",
                "response": {
                    "output": [{
                        "type": "function_call",
                        "call_id": "call_1",
                        "name": "command_run",
                        "arguments": args
                    }],
                    "usage": {
                        "input_tokens": 3000,
                        "input_tokens_details": {"cached_tokens": 2048},
                        "output_tokens": 20,
                        "total_tokens": 3020
                    }
                }
            });
            let body = format!("data: {tool_event}\n\ndata: {completed}\n\ndata: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "run command"})];
        let options = CallOptions {
            tools: Some(vec![json!({
                "type": "function",
                "function": {
                    "name": "command_run",
                    "parameters": {"type": "object"}
                }
            })]),
            ..CallOptions::default()
        };

        let result = super::codex_oauth_call(
            "gpt-5.1-codex-mini",
            "test-token",
            &messages,
            &options,
            None,
        )
        .await;

        match previous_endpoint {
            Some(value) => std::env::set_var("OPENAI_CODEX_ENDPOINT", value),
            None => std::env::remove_var("OPENAI_CODEX_ENDPOINT"),
        }

        let metrics = result.expect("codex oauth call").metrics.expect("metrics");
        assert_eq!(metrics.usage.cached_input_tokens, Some(2048));
        assert!(metrics.cache_hit);
    }

    #[test]
    fn codex_oauth_payload_omits_default_reasoning_and_acceleration() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("default".to_string()),
            service_tier: Some(" default ".to_string()),
            ..CallOptions::default()
        };

        let payload = build_codex_oauth_payload("gpt-5.1-codex", &messages, &options);

        assert!(payload.get("reasoning").is_none());
        assert!(payload.get("reasoning_effort").is_none());
        assert!(payload.get("service_tier").is_none());
    }

    #[test]
    fn codex_oauth_payload_passes_prompt_cache_key_only() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            prompt_cache_key: Some("turaosv2:test:abc".to_string()),
            ..CallOptions::default()
        };

        let payload = build_codex_oauth_payload("gpt-5.1-codex-mini", &messages, &options);

        assert_eq!(payload["prompt_cache_key"], "turaosv2:test:abc");
        assert!(payload.get("prompt_cache_retention").is_none());
    }

    #[test]
    fn codex_oauth_payload_keeps_system_messages_in_input() {
        let messages = vec![
            json!({"role": "system", "content": "You are Tura an agent based on gpt-5.1-codex from LLM provider: openai."}),
            json!({"role": "user", "content": "task"}),
            json!({"role": "system", "content": "dynamic runtime state"}),
            json!({"role": "assistant", "content": "progress"}),
        ];

        let payload =
            build_codex_oauth_payload("gpt-5.1-codex-mini", &messages, &CallOptions::default());

        assert_eq!(
            payload["instructions"],
            "You are Tura an agent based on gpt-5.1-codex from LLM provider: openai."
        );
        assert_eq!(payload["input"][0]["role"], "user");
        assert_eq!(payload["input"][0]["content"], "task");
        assert_eq!(payload["input"][1]["role"], "system");
        assert_eq!(payload["input"][1]["content"], "dynamic runtime state");
        assert_eq!(payload["input"][2]["role"], "assistant");
        assert_eq!(payload["input"][2]["content"], "progress");
        assert_eq!(payload["tool_choice"], "auto");
    }

    #[test]
    fn codex_oauth_usage_falls_back_to_estimate_when_stream_stops_before_usage() {
        let payload = json!({
            "model": "gpt-5.1-codex",
            "input": [{"role": "user", "content": "Run tests"}],
            "tools": [{"type": "function", "name": "command_run"}]
        });
        let content = json!({
            "tool_calls": [{
                "function": {
                    "name": "command_run",
                    "arguments": {"commands": [{"step": 1, "command": "npm", "command_line": "npm test"}]}
                }
            }]
        });
        let mut metrics = crate::metrics::extract_openapi_metrics(&json!({}), None);

        crate::metrics::fill_missing_estimated_usage(
            &mut metrics,
            &payload,
            &content,
            "codex_oauth_stream_returned_before_provider_usage",
        );

        assert!(
            metrics
                .usage
                .input_tokens
                .expect("estimated input tokens should be present")
                > 0
        );
        assert!(
            metrics
                .usage
                .output_tokens
                .expect("estimated output tokens should be present")
                > 0
        );
        assert_eq!(
            metrics
                .raw_usage
                .as_ref()
                .and_then(|usage| usage.get("estimated"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn command_run_streaming_waits_for_complete_json_arguments() {
        let call = json!({
            "type": "function",
            "function": {
                "name": "command_run",
                "arguments": r#"{"commands":[{"step":1,"command":"rg","command_line":"rg -n bug ."},"#
            }
        });

        assert!(super::ready_streaming_tool_call(call).is_none());
    }

    #[test]
    fn command_run_command_streaming_emits_each_complete_command_object() {
        let mut collector = super::CodexCommandRunCommandCollector::default();
        collector.push_event(&json!({
            "type": "response.output_item.added",
            "item": {
                "id": "fc_1",
                "call_id": "call_1",
                "type": "function_call",
                "name": "command_run"
            }
        }));
        let first = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_1",
            "delta": "{\"commands\":[{\"step\":1,\"command_type\":\"shell_command\",\"command_line\":\"echo {one}\"},"
        }));
        assert_eq!(first.len(), 1);
        assert_eq!(command_index_for_test(&first[0]), Some(0));

        let second = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_1",
            "delta": "{\"step\":2,\"command_type\":\"shell_command\",\"command_line\":\"echo two\"}"
        }));
        assert_eq!(second.len(), 1);
        assert_eq!(command_index_for_test(&second[0]), Some(1));

        let done = collector.push_event(&json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_1",
            "arguments": "{\"commands\":[{\"step\":1,\"command_type\":\"shell_command\",\"command_line\":\"echo {one}\"},{\"step\":2,\"command_type\":\"shell_command\",\"command_line\":\"echo two\"}]}"
        }));
        assert!(done.is_empty());
    }

    #[test]
    fn command_run_command_streaming_emits_split_python_command_object() {
        let mut collector = super::CodexCommandRunCommandCollector::default();
        collector.push_event(&json!({
            "type": "response.output_item.added",
            "item": {
                "id": "fc_stream_probe",
                "call_id": "call_stream_probe",
                "type": "function_call",
                "name": "command_run",
                "arguments": ""
            }
        }));
        let open = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_stream_probe",
            "delta": "{\"commands\":["
        }));
        assert!(open.is_empty());
        let first_command = json!({
            "step": 1,
            "command_type": "shell_command",
            "command_line": json!({
                "command": "python -c \"from pathlib import Path; Path('streamed-first.txt').write_text('first')\"",
                "timeout_ms": 20000
            }).to_string()
        })
        .to_string()
            + ",";
        let first = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_stream_probe",
            "delta": first_command
        }));
        assert_eq!(first.len(), 1);
        assert_eq!(command_index_for_test(&first[0]), Some(0));
    }

    fn command_index_for_test(event: &crate::tura_llm::ProviderStreamEvent) -> Option<usize> {
        match event {
            crate::tura_llm::ProviderStreamEvent::CommandRunCommandReady {
                command_index, ..
            } => Some(*command_index),
            crate::tura_llm::ProviderStreamEvent::ProviderOutputStarted => None,
        }
    }

    #[test]
    fn command_run_streaming_emits_complete_json_arguments() {
        let call = json!({
            "type": "function",
            "function": {
                "name": "command_run",
                "arguments": r#"{"commands":[{"step":1,"command":"npm","command_line":"npm test"}]}"#
            }
        });
        let ready = super::ready_streaming_tool_call(call).expect("complete command_run call");

        assert_eq!(
            ready["function"]["arguments"]["commands"]
                .as_array()
                .expect("ready command_run commands should be an array")
                .len(),
            1
        );
        assert_eq!(
            ready["function"]["arguments"]["commands"][0]["command"],
            "npm"
        );
        assert!(ready["function"]["arguments"].get("commands").is_some());
    }

    #[test]
    fn codex_event_tool_calls_accumulates_argument_deltas_before_emit() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "delta": "{\"commands\":[{\"step\":1,"
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "delta": "\"command\":\"shell_command\",\"command_line\":\"pwd\"}]}"
            }),
        ];
        let calls = super::codex_event_tool_calls(&events);
        let ready = calls
            .into_iter()
            .filter_map(super::ready_streaming_tool_call)
            .collect::<Vec<_>>();

        assert_eq!(ready.len(), 1);
        assert_eq!(
            ready[0]["function"]["arguments"]["commands"][0]["command"],
            "shell_command"
        );
    }

    #[test]
    fn codex_stream_collector_emits_on_arguments_done_before_completed_response() {
        let mut collector = super::CodexToolCallStreamCollector::default();
        let added = json!({
            "type": "response.output_item.added",
            "item": {
                "type": "function_call",
                "id": "fc_early",
                "call_id": "call_early",
                "name": "command_run",
                "arguments": ""
            }
        });
        let delta = json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "call_early",
            "delta": "{\"commands\":[{\"command_type\":\"shell_command\","
        });
        let done = json!({
            "type": "response.function_call_arguments.done",
            "item_id": "call_early",
            "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"pwd\"}]}"
        });

        assert!(collector.push_event(&added).is_empty());
        assert!(collector.push_event(&delta).is_empty());
        let ready = collector.push_event(&done);

        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0]["function"]["name"], "command_run");
        assert_eq!(
            ready[0]["function"]["arguments"]["commands"][0]["command_type"],
            "shell_command"
        );
    }

    #[test]
    fn codex_stream_collector_does_not_emit_incomplete_arguments() {
        let mut collector = super::CodexToolCallStreamCollector::default();
        let added = json!({
            "type": "response.output_item.added",
            "item": {
                "type": "function_call",
                "id": "fc_incomplete",
                "call_id": "call_incomplete",
                "name": "command_run",
                "arguments": ""
            }
        });
        let delta = json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "call_incomplete",
            "delta": "{\"commands\":["
        });

        assert!(collector.push_event(&added).is_empty());
        assert!(collector.push_event(&delta).is_empty());
        assert!(collector.finish().is_empty());
    }

    #[test]
    fn codex_responses_stream_tool_call_does_not_pollute_output_text() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "id": "fc_real",
                    "call_id": "call_real",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "item_id": "fc_real",
                "delta": "{\"commands\":[{\"command_type\":\"shell_command\","
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "item_id": "fc_real",
                "delta": "\"command_line\":\"Get-Content -Raw src/app.txt\"}]}"
            }),
            json!({
                "type": "response.function_call_arguments.done",
                "item_id": "fc_real",
                "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"Get-Content -Raw src/app.txt\"}]}"
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "id": "fc_real",
                    "call_id": "call_real",
                    "name": "command_run",
                    "status": "completed",
                    "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"Get-Content -Raw src/app.txt\"}]}"
                }
            }),
        ];

        let mut output_text = String::new();
        for event in &events {
            super::append_codex_stream_text(event, &mut output_text);
        }
        assert!(output_text.is_empty());

        let normalized = super::normalize_codex_response_content(&json!({
            "events": events,
            "output_text": output_text,
        }));
        let tool_calls = normalized["tool_calls"]
            .as_array()
            .expect("Responses function_call events should normalize to tool_calls");

        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "call_real");
        assert_eq!(tool_calls[0]["function"]["name"], "command_run");
        assert_eq!(
            tool_calls[0]["function"]["arguments"]["commands"][0]["command_type"],
            "shell_command"
        );
        assert!(normalized.get("text").is_none());
    }

    #[test]
    fn codex_event_tool_calls_does_not_emit_incomplete_command_run_arguments() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "delta": "{\"commands\":["
            }),
        ];
        let ready = super::codex_event_tool_calls(&events)
            .into_iter()
            .filter_map(super::ready_streaming_tool_call)
            .collect::<Vec<_>>();

        assert!(ready.is_empty());
    }

    #[test]
    fn codex_event_tool_calls_prefers_done_arguments_over_added_empty_arguments() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "id": "fc_1",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.done",
                "item_id": "fc_1",
                "arguments": "{\"commands\":[{\"step\":1,\"command\":\"echo ok\"}]}"
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "id": "fc_1",
                    "name": "command_run",
                    "status": "completed",
                    "arguments": "{\"commands\":[{\"step\":1,\"command\":\"echo ok\"}]}"
                }
            }),
        ];
        let ready = super::complete_codex_tool_calls(&json!({ "events": events }));

        assert_eq!(ready.len(), 1);
        assert_eq!(
            ready[0]["function"]["arguments"]["commands"][0]["command"],
            "echo ok"
        );
    }

    #[test]
    fn streaming_tool_call_buffer_waits_for_complete_json_arguments() {
        let mut buffer = super::StreamingToolCall {
            id: Some("call_1".to_string()),
            name: Some("command_run".to_string()),
            arguments: r#"{"commands":[{"step":1,"command":"rg","command_line":"rg -n bug ."},"#
                .to_string(),
            emitted: false,
        };
        let mut calls = Vec::new();

        assert!(!super::emit_completed_tool_call(&mut buffer, &mut calls));
        assert!(calls.is_empty());
    }

    #[test]
    fn streaming_tool_call_buffer_emits_complete_json_arguments() {
        let mut buffer = super::StreamingToolCall {
            id: Some("call_1".to_string()),
            name: Some("command_run".to_string()),
            arguments: r#"{"commands":[{"step":1,"command":"npm","command_line":"npm test"}]}"#
                .to_string(),
            emitted: false,
        };
        let mut calls = Vec::new();

        assert!(super::emit_completed_tool_call(&mut buffer, &mut calls));
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0]["function"]["arguments"]["commands"][0]["command"],
            "npm"
        );
    }

    #[test]
    fn minimax_xml_streaming_tool_call_supports_complete_command_run() {
        let text = r#"<minimax:tool_call><invoke name="command_run"><parameter name="commands">[{"step":1,"command":"npm","command_line":"npm test"}]</parameter></invoke></minimax:tool_call>"#;
        let (name, arguments) = super::last_complete_minimax_invoke(text).expect("xml tool call");

        assert_eq!(name, "command_run");
        assert_eq!(arguments["commands"][0]["command"], "npm");
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }
}
