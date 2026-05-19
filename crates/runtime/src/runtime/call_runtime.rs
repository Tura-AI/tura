use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::error;

use crate::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeManagement, RuntimeState, ToolCallRecord,
};
use crate::state_machine::session_management::SessionId;

use super::runtime_recieve::command_run_stream_event_command;
use super::types::{RuntimeQueueItem, ToolCallData};

const COMMAND_RUN_TOOL_NAME: &str = "command_run";

pub struct CallRuntimeInput {
    pub runtime: RuntimeManagement,
    pub messages: Vec<serde_json::Value>,
    pub tools: Vec<serde_json::Value>,
    pub provider_name: String,
    pub stream: bool,
    pub tool_choice: Option<serde_json::Value>,
    pub session_directory: PathBuf,
}

pub async fn call_runtime(
    input: CallRuntimeInput,
    tura_settings: Arc<tura_llm_rust::Settings>,
    tura_config: Arc<tura_llm_rust::TuraConfig>,
) -> Result<RuntimeManagement, String> {
    let mut runtime = input.runtime;
    let now = Utc::now();
    let provider_messages = normalize_provider_messages(input.messages);
    let input_messages = provider_messages.clone();
    let input_tools = input.tools.clone();

    runtime
        .transition(RuntimeState::Dispatching)
        .map_err(|e| format!("failed to transition runtime to Dispatching: {}", e))?;
    runtime
        .mark_called(now)
        .map_err(|e| format!("failed to mark runtime called: {}", e))?;
    runtime
        .mark_waiting_first_token()
        .map_err(|e| format!("failed to mark runtime waiting for first token: {}", e))?;

    let configured_route = route_by_name(tura_settings.as_ref(), &input.provider_name)
        .ok_or_else(|| format!("unknown provider route: {}", input.provider_name))?;
    let override_route = session_model_override_route(tura_settings.as_ref(), configured_route);
    let route_config = override_route.as_ref().unwrap_or(configured_route);

    let prompt_cache_key = prompt_cache_key(
        route_config,
        &input.provider_name,
        &runtime.session_id,
        &input_tools,
    );
    let call_options = tura_llm_rust::CallOptions {
        tools: if input.tools.is_empty() {
            None
        } else {
            Some(input.tools)
        },
        stream: Some(input.stream),
        parallel_tool_calls: parallel_tool_calls_enabled(route_config, !input_tools.is_empty()),
        prompt_cache_key,
        stream_options: stream_options(route_config, input.stream),
        reasoning_effort: session_reasoning_effort(),
        service_tier: session_service_tier(),
        store: Some(false),
        tool_choice: input.tool_choice.clone(),
        ..Default::default()
    };
    runtime.set_input(serde_json::json!({
        "messages": input_messages,
        "tools": input_tools,
        "options": {
            "stream": input.stream,
            "parallel_tool_calls": call_options.parallel_tool_calls,
            "prompt_cache_key": call_options.prompt_cache_key.clone(),
            "stream_options": call_options.stream_options.clone(),
            "reasoning_effort": call_options.reasoning_effort.clone(),
            "service_tier": call_options.service_tier.clone(),
            "store": call_options.store,
            "tool_choice": call_options.tool_choice.clone(),
        }
    }));

    if input.stream {
        call_runtime_streaming(
            &mut runtime,
            route_config,
            &tura_config,
            provider_messages,
            call_options,
            input.session_directory.clone(),
        )
        .await?;
    } else {
        call_runtime_non_streaming(
            &mut runtime,
            route_config,
            &tura_config,
            provider_messages,
            call_options,
        )
        .await?;
    }

    Ok(runtime)
}

fn normalize_provider_messages(messages: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
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
        let content = message_content(&message);
        if content.trim().is_empty() {
            continue;
        }

        let (role, content) = match role {
            "system" | "developer" | "user" | "assistant" | "tool" => (role, content),
            other => ("user", format!("Runtime context ({other}):\n{content}")),
        };
        normalized.push(serde_json::json!({
            "role": role,
            "content": content
        }));
    }
    normalized
}

fn message_content(message: &serde_json::Value) -> String {
    if let Some(content) = message.get("content").and_then(serde_json::Value::as_str) {
        return content.to_string();
    }
    if let Some(text) = message.get("text").and_then(serde_json::Value::as_str) {
        return text.to_string();
    }
    String::new()
}

fn session_reasoning_effort() -> Option<String> {
    std::env::var("TURA_SESSION_REASONING_EFFORT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
}

fn prompt_cache_key(
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

fn prompt_cache_key_supported(provider: &str, base_url: &str) -> bool {
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
    if provider.eq_ignore_ascii_case("openai") {
        return true;
    }
    base_url.contains("api.openai.com")
}

fn stream_options(
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

fn openai_compatible_usage_stream_supported(provider: &str, base_url: &str) -> bool {
    if provider.eq_ignore_ascii_case("openai") || provider.eq_ignore_ascii_case("minimax") {
        return true;
    }
    base_url.contains("api.openai.com") || base_url.contains("api.minimax.io")
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

fn parallel_tool_calls_enabled(
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

fn session_service_tier() -> Option<String> {
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

pub fn route_by_name<'a>(
    settings: &'a tura_llm_rust::Settings,
    provider_name: &str,
) -> Option<&'a tura_llm_rust::RouteConfig> {
    match provider_name {
        "tura_general" => Some(&settings.tura_general),
        "tura_office" => Some(&settings.tura_office),
        "tura_creative" => Some(&settings.tura_creative),
        "tura_translator" => Some(&settings.tura_translator),
        "tura_validator" => Some(&settings.tura_validator),
        "tura_validator_advanced" => Some(&settings.tura_validator_advanced),
        "tura_classifier" => Some(&settings.tura_classifier),
        "tura_embedding" => Some(&settings.tura_embedding),
        "tura_coder" => Some(&settings.tura_coder),
        "tura_coder_advanced" => Some(&settings.tura_coder_advanced),
        "tura_planner" => Some(&settings.tura_planner),
        "tura_planner_advanced" => Some(&settings.tura_planner_advanced),
        "tura_roleplay" => Some(&settings.tura_roleplay),
        "tura_professional" => Some(&settings.tura_professional),
        "tura_math" => Some(&settings.tura_math),
        "tura_academic" => Some(&settings.tura_academic),
        _ => None,
    }
}

fn session_model_override_route(
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
    for route in [
        &settings.tura_general,
        &settings.tura_office,
        &settings.tura_creative,
        &settings.tura_translator,
        &settings.tura_validator,
        &settings.tura_validator_advanced,
        &settings.tura_classifier,
        &settings.tura_embedding,
        &settings.tura_coder,
        &settings.tura_coder_advanced,
        &settings.tura_planner,
        &settings.tura_planner_advanced,
        &settings.tura_roleplay,
        &settings.tura_professional,
        &settings.tura_math,
        &settings.tura_academic,
    ] {
        if let Some(config) = route
            .providers
            .iter()
            .find(|item| item.provider == provider)
        {
            return Some(config.base_url.clone());
        }
    }
    match provider {
        "antigravity" => Some("https://antigravity.google.com/v1".to_string()),
        "anthropic" => Some("https://api.anthropic.com/v1".to_string()),
        "minimax" => Some("https://api.minimax.io/v1".to_string()),
        "openai" => Some("https://api.openai.com/v1".to_string()),
        _ => None,
    }
}

async fn call_runtime_non_streaming(
    runtime: &mut RuntimeManagement,
    route_config: &tura_llm_rust::RouteConfig,
    tura_config: &Arc<tura_llm_rust::TuraConfig>,
    messages: Vec<serde_json::Value>,
    options: tura_llm_rust::CallOptions,
) -> Result<(), String> {
    let started_at = Utc::now();
    let timeout_duration = runtime_timeout(runtime);

    match tokio::time::timeout(
        timeout_duration,
        route_config.run(tura_config.as_ref(), messages, options),
    )
    .await
    {
        Err(_) => {
            let finished_at = Utc::now();
            let message = format!(
                "runtime call timed out after {} ms",
                timeout_duration.as_millis()
            );
            error!(error = %message, "runtime call timed out");
            runtime.set_output(serde_json::json!({
                "error": message
            }));
            finish_runtime_failure(
                runtime,
                finished_at,
                "CALL_TIMED_OUT",
                message,
                RuntimeCallResultStatus::TimedOut,
            )?;
        }
        Ok(Ok(response)) => {
            let finished_at = Utc::now();
            runtime.set_output(response.content.clone());
            apply_provider_response(runtime, &response.content, finished_at);

            runtime
                .mark_first_token(finished_at)
                .map_err(|e| format!("failed to mark first token: {}", e))?;

            let usage =
                usage_report_from_metrics(response.metrics, started_at, finished_at, finished_at);

            runtime
                .finish_success(finished_at, usage)
                .map_err(|e| format!("failed to finish runtime success: {}", e))?;
        }
        Ok(Err(e)) => {
            let finished_at = Utc::now();
            error!(error = %e, "runtime call failed");
            runtime.set_output(serde_json::json!({
                "error": e.to_string()
            }));
            finish_runtime_failure(
                runtime,
                finished_at,
                "CALL_FAILED",
                e.to_string(),
                RuntimeCallResultStatus::Failed,
            )?;
        }
    }

    Ok(())
}

fn runtime_timeout(runtime: &RuntimeManagement) -> Duration {
    Duration::from_millis(runtime.provider.base.time_out_ms.max(1_000))
}

fn finish_runtime_failure(
    runtime: &mut RuntimeManagement,
    finished_at: chrono::DateTime<Utc>,
    error_code: &str,
    error_text: String,
    status: RuntimeCallResultStatus,
) -> Result<(), String> {
    let err = crate::state_machine::runtime_management::RuntimeError {
        error_code: Some(error_code.to_string()),
        error_text: Some(error_text),
        retry_allowed: true,
        fallback_allowed: true,
        fallback_to_id: None,
    };
    runtime
        .finish_failure(finished_at, err, status, None)
        .map_err(|e| format!("failed to finish runtime failure: {}", e))
}

fn extract_response_text(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    if let Some(text) = content.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(parts) = content.get("parts").and_then(Value::as_array) {
        let text = parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("");
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn extract_tool_calls(content: &Value) -> Vec<ToolCallData> {
    let mut calls = Vec::new();

    if let Some(tool_calls) = content.get("tool_calls").and_then(Value::as_array) {
        for call in tool_calls {
            if let Some(function) = call.get("function") {
                if let Some(name) = function.get("name").and_then(Value::as_str) {
                    let arguments = function.get("arguments").cloned().unwrap_or(Value::Null);
                    calls.push(ToolCallData {
                        tool_name: name.to_string(),
                        arguments: parse_arguments(arguments),
                    });
                }
            }
        }
    }

    if let Some(parts) = content.get("parts").and_then(Value::as_array) {
        for part in parts {
            if let Some(function_call) = part.get("functionCall") {
                if let Some(name) = function_call.get("name").and_then(Value::as_str) {
                    calls.push(ToolCallData {
                        tool_name: name.to_string(),
                        arguments: function_call.get("args").cloned().unwrap_or(Value::Null),
                    });
                }
            }
        }
    }

    calls
}

fn parse_arguments(arguments: Value) -> Value {
    match arguments {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        other => other,
    }
}

#[cfg(test)]
mod provider_message_tests {
    use super::normalize_provider_messages;

    #[test]
    fn normalize_provider_messages_preserves_message_boundaries_for_cache() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "## Input rules\nstable"}),
            serde_json::json!({"role": "system", "content": "# COMMAND_RUN Tool Guide\nstable"}),
            serde_json::json!({"role": "system", "content": "Dynamic runtime state:\ncurrent_directory: C:/tmp"}),
            serde_json::json!({"role": "user", "content": "do the task"}),
            serde_json::json!({"role": "system", "content": "Recent tool callback result from `command_run`:\nok"}),
            serde_json::json!({"role": "debug", "content": "unknown role text"}),
            serde_json::json!({"role": "user", "content": ""}),
        ];

        let normalized = normalize_provider_messages(messages);

        assert_eq!(normalized.len(), 6);
        assert_eq!(normalized[0]["role"], "system");
        assert_eq!(normalized[1]["role"], "system");
        assert_eq!(normalized[2]["role"], "system");
        assert_eq!(normalized[3]["role"], "user");
        assert_eq!(normalized[4]["role"], "system");
        assert_eq!(normalized[5]["role"], "user");
        assert_eq!(normalized[0]["content"], "## Input rules\nstable");
        assert_eq!(normalized[1]["content"], "# COMMAND_RUN Tool Guide\nstable");
        assert_eq!(
            normalized[5]["content"],
            "Runtime context (debug):\nunknown role text"
        );
    }
}

async fn call_runtime_streaming(
    runtime: &mut RuntimeManagement,
    route_config: &tura_llm_rust::RouteConfig,
    tura_config: &Arc<tura_llm_rust::TuraConfig>,
    messages: Vec<serde_json::Value>,
    options: tura_llm_rust::CallOptions,
    session_directory: PathBuf,
) -> Result<(), String> {
    let started_at = Utc::now();
    let timeout_duration = runtime_timeout(runtime);
    let (stream_tx, mut stream_rx) =
        tokio::sync::mpsc::unbounded_channel::<tura_llm_rust::ProviderStreamEvent>();
    let first_stream_output_at: Arc<Mutex<Option<DateTime<Utc>>>> = Arc::new(Mutex::new(None));
    let first_stream_output_for_sink = Arc::clone(&first_stream_output_at);
    let sink: tura_llm_rust::ProviderStreamEventSink = Arc::new(move |event| {
        if matches!(
            event,
            tura_llm_rust::ProviderStreamEvent::ProviderOutputStarted
        ) {
            let mut first = first_stream_output_for_sink
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            if first.is_none() {
                *first = Some(Utc::now());
            }
        }
        let _ = stream_tx.send(event);
    });
    let command_session_directory = session_directory.clone();
    let command_task = tokio::spawn(async move {
        let mut executor =
            code_tools::command_run::StreamingCommandRunExecutor::new(command_session_directory);
        while let Some(event) = stream_rx.recv().await {
            let Some(command) = command_run_stream_event_command(event) else {
                continue;
            };
            executor.push_command_value(command).await;
        }
        executor.finish().await
    });

    let response = match tokio::time::timeout(
        timeout_duration,
        route_config.run_with_stream_events(tura_config.as_ref(), messages, options, Some(sink)),
    )
    .await
    {
        Err(_) => {
            let finished_at = Utc::now();
            let message = format!(
                "runtime call timed out after {} ms",
                timeout_duration.as_millis()
            );
            error!(error = %message, "runtime call timed out");
            runtime.set_output(serde_json::json!({
                "error": message
            }));
            finish_runtime_failure(
                runtime,
                finished_at,
                "CALL_TIMED_OUT",
                message,
                RuntimeCallResultStatus::TimedOut,
            )?;
            return Ok(());
        }
        Ok(Ok(response)) => response,
        Ok(Err(e)) => {
            let finished_at = Utc::now();
            error!(error = %e, "runtime call failed");
            runtime.set_output(serde_json::json!({
                "error": e.to_string()
            }));
            finish_runtime_failure(
                runtime,
                finished_at,
                "CALL_FAILED",
                e.to_string(),
                RuntimeCallResultStatus::Failed,
            )?;
            return Ok(());
        }
    };
    let finished_at = Utc::now();
    let streamed_command_results = command_task.await.unwrap_or_default();

    let mut runtime_output = response.content.clone();
    if !streamed_command_results.is_empty() {
        runtime_output = serde_json::json!({
            "provider_content": response.content.clone(),
            "streamed_command_run_result": {
                "results": streamed_command_results,
            }
        });
    }
    runtime.set_output(runtime_output);
    apply_provider_response_with_options(runtime, &response.content, finished_at, false);

    if let Some(stream) = response.content.get("stream").and_then(|s| s.as_array()) {
        for chunk in stream {
            if let Some(text) = chunk.get("text").and_then(|t| t.as_str()) {
                runtime.append_text(text);
            }
        }
    }

    let first_token_at = first_stream_output_at
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .unwrap_or(finished_at);

    runtime
        .mark_first_token(first_token_at)
        .map_err(|e| format!("failed to mark first token: {}", e))?;

    let usage =
        usage_report_from_metrics(response.metrics, started_at, finished_at, first_token_at);

    runtime
        .finish_success(finished_at, usage)
        .map_err(|e| format!("failed to finish runtime success: {}", e))?;

    Ok(())
}

fn usage_report_from_metrics(
    metrics: Option<tura_llm_rust::CallMetrics>,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    first_token_at: DateTime<Utc>,
) -> Option<crate::state_machine::runtime_management::UsageReport> {
    let latency_ms = finished_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    let time_to_first_token_ms = first_token_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    metrics.map(|m| crate::state_machine::runtime_management::UsageReport {
        input_tokens: m.usage.input_tokens.unwrap_or(0) as u64,
        output_tokens: m.usage.output_tokens.unwrap_or(0) as u64,
        total_tokens: m.usage.total_tokens.unwrap_or(0) as u64,
        cached_input_tokens: m.usage.cached_input_tokens.unwrap_or(0) as u64,
        cache_write_tokens: m.usage.cache_write_tokens.unwrap_or(0) as u64,
        reasoning_tokens: m.usage.reasoning_tokens.unwrap_or(0) as u64,
        attachment_input_tokens: 0,
        input_cost: m.cost.input_cost.unwrap_or(0.0),
        output_cost: m.cost.output_cost.unwrap_or(0.0),
        total_cost: m.cost.total_cost.unwrap_or(0.0),
        currency: m.cost.currency.unwrap_or_else(|| "USD".to_string()),
        pricing_source: "provider".to_string(),
        latency_ms,
        time_to_first_token_ms,
        token_per_second: tokens_per_second(m.usage.output_tokens.unwrap_or(0), latency_ms),
    })
}

fn tokens_per_second(output_tokens: u64, latency_ms: u64) -> f64 {
    if output_tokens == 0 || latency_ms == 0 {
        return 0.0;
    }
    output_tokens as f64 / (latency_ms as f64 / 1000.0)
}

fn apply_provider_response(
    runtime: &mut RuntimeManagement,
    content: &Value,
    now: chrono::DateTime<Utc>,
) {
    apply_provider_response_with_options(runtime, content, now, false);
}

fn apply_provider_response_with_options(
    runtime: &mut RuntimeManagement,
    content: &Value,
    now: chrono::DateTime<Utc>,
    suppress_command_run_tool_calls: bool,
) {
    let content = tura_llm_rust::normalize_response_content(content);

    if let Some(text) = extract_response_text(&content) {
        runtime.append_text(&text);
    }

    for tool_call in extract_tool_calls(&content) {
        if suppress_command_run_tool_calls && tool_call.tool_name == COMMAND_RUN_TOOL_NAME {
            continue;
        }
        runtime.push_tool_call(ToolCallRecord {
            tool_called_name: tool_call.tool_name,
            tool_called_input: tool_call.arguments,
            tool_received_at: now,
            tool_executed_at: now,
            tool_calldata_received_at: now,
            tool_reported_success: false,
            agent_reported_success: false,
            agent_reported_helpful: false,
            agent_reported_summary: String::new(),
            validator_reported_success: None,
        });
    }
}

pub async fn dequeue_runtime(
    session_id: &SessionId,
    redis_url: &str,
) -> Result<Option<RuntimeQueueItem>, String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("runtime:queue:{}", session_id);

    let result: Option<String> = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async(&mut con)
        .await
        .map_err(|e| format!("failed to dequeue runtime: {}", e))?;

    match result {
        Some(payload) => {
            let item: RuntimeQueueItem = serde_json::from_str(&payload)
                .map_err(|e| format!("failed to deserialize queue item: {}", e))?;
            Ok(Some(item))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{prompt_cache_key, session_reasoning_effort, session_service_tier, stream_options};
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    const REASONING_ENV: &str = "TURA_SESSION_REASONING_EFFORT";
    const ACCEL_ENV: &str = "TURA_SESSION_ACCELERATION_ENABLED";
    const DISABLE_CACHE_ENV: &str = "TURA_DISABLE_PROMPT_CACHE";

    fn with_env<T>(name: &str, value: Option<&str>, run: impl FnOnce() -> T) -> T {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
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
            assert_eq!(session_reasoning_effort(), None);
        });
        with_env(REASONING_ENV, Some("   "), || {
            assert_eq!(session_reasoning_effort(), None);
        });
        with_env(REASONING_ENV, Some(" default "), || {
            assert_eq!(session_reasoning_effort(), None);
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
        with_env(ACCEL_ENV, None, || {
            assert_eq!(session_service_tier(), None);
        });
        with_env(ACCEL_ENV, Some("0"), || {
            assert_eq!(session_service_tier(), None);
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
        let route = tura_llm_rust::RouteConfig {
            default_temperature: 0.2,
            providers: vec![tura_llm_rust::ProviderConfig {
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5.3-codex-spark".to_string(),
                temperature: 0.2,
            }],
        };
        let tools_a = vec![
            serde_json::json!({"type":"function","function":{"name":"command_run","parameters":{"type":"object"}}}),
        ];
        let tools_b = vec![
            serde_json::json!({"type":"function","function":{"name":"command_run","parameters":{"type":"object"}}}),
        ];

        with_env(DISABLE_CACHE_ENV, None, || {
            assert_eq!(
                prompt_cache_key(&route, "tura_coder", &"sess-a".to_string(), &tools_a),
                prompt_cache_key(&route, "tura_coder", &"sess-a".to_string(), &tools_b)
            );
            assert_eq!(
                prompt_cache_key(&route, "tura_coder", &"sess-a".to_string(), &tools_a),
                prompt_cache_key(&route, "tura_coder", &"sess-a".to_string(), &tools_b)
            );
            assert!(
                prompt_cache_key(&route, "tura_coder", &"sess-a".to_string(), &tools_a)
                    .unwrap()
                    .starts_with("turaosv2:tura-coder:sess-a:")
            );
            assert_ne!(
                prompt_cache_key(&route, "tura_coder", &"sess-a".to_string(), &tools_a),
                prompt_cache_key(&route, "tura_coder", &"sess-b".to_string(), &tools_a)
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
                prompt_cache_key(&route, "tura_coder", &"sess-a".to_string(), &[]),
                None
            );
        });
    }

    #[test]
    fn prompt_cache_key_can_be_disabled() {
        let route = tura_llm_rust::RouteConfig {
            default_temperature: 0.2,
            providers: vec![tura_llm_rust::ProviderConfig {
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5.3-codex-spark".to_string(),
                temperature: 0.2,
            }],
        };
        with_env(DISABLE_CACHE_ENV, Some("true"), || {
            assert_eq!(
                prompt_cache_key(&route, "tura_coder", &"sess-a".to_string(), &[]),
                None
            );
        });
    }

    #[test]
    fn stream_options_request_openai_compatible_usage_when_streaming() {
        let openai_route = tura_llm_rust::RouteConfig {
            default_temperature: 0.2,
            providers: vec![tura_llm_rust::ProviderConfig {
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5.3-codex-spark".to_string(),
                temperature: 0.2,
            }],
        };
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
            stream_options(&openai_route, true),
            Some(serde_json::json!({ "include_usage": true }))
        );
        assert_eq!(stream_options(&openai_route, false), None);
        assert_eq!(
            stream_options(&minimax_route, true),
            Some(serde_json::json!({ "include_usage": true }))
        );
    }
}
