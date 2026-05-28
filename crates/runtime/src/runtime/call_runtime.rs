use chrono::{DateTime, Utc};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};
use tracing::error;

use crate::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeManagement, RuntimeState, ToolCallRecord,
};
use crate::state_machine::session_management::SessionId;

use super::runtime_receive::command_run_stream_event_command;
use super::types::{RuntimeQueueItem, ToolCallData};

const COMMAND_RUN_TOOL_NAME: &str = "command_run";
const DEFAULT_STREAMED_COMMAND_RUN_POST_RESULT_TIMEOUT_MS: u64 = 15_000;

pub struct CallRuntimeInput {
    pub runtime: RuntimeManagement,
    pub messages: Vec<serde_json::Value>,
    pub tools: Vec<serde_json::Value>,
    pub provider_name: String,
    pub stream: bool,
    pub max_tokens: u32,
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
        max_tokens: session_max_tokens(input.max_tokens),
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
            "max_tokens": call_options.max_tokens,
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

fn session_max_tokens(agent_max_tokens: u32) -> Option<u64> {
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
    settings.provider_base_url(provider)
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
                        provider_metadata: None,
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

fn parse_arguments(arguments: Value) -> Value {
    match arguments {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        other => other,
    }
}

fn strip_thought_blocks(text: &str) -> String {
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

#[cfg(test)]
mod provider_message_tests {
    use super::{normalize_provider_messages, strip_thought_blocks};

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

    #[test]
    fn strip_thought_blocks_removes_visible_reasoning_text() {
        assert_eq!(
            strip_thought_blocks("<thought>hidden</thought>visible"),
            "visible"
        );
        assert_eq!(strip_thought_blocks("a<THOUGHT>hidden</THOUGHT>b"), "ab");
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
    let (stream_tx, stream_rx) = mpsc::channel::<tura_llm_rust::ProviderStreamEvent>();
    let first_stream_output_at: Arc<Mutex<Option<DateTime<Utc>>>> = Arc::new(Mutex::new(None));
    let streamed_command_results: Arc<Mutex<Vec<serde_json::Value>>> =
        Arc::new(Mutex::new(Vec::new()));
    let last_streamed_command_result_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let streamed_command_run_cancelled = Arc::new(AtomicBool::new(false));
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
    let command_results_for_task = Arc::clone(&streamed_command_results);
    let last_command_result_for_task = Arc::clone(&last_streamed_command_result_at);
    let command_cancelled_for_task = Arc::clone(&streamed_command_run_cancelled);
    let command_task = std::thread::spawn(move || {
        let mut executor =
            code_tools::command_run::StreamingCommandRunExecutor::new(command_session_directory);
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(_) => return Vec::new(),
        };
        let mut results = Vec::new();
        let mut live_item_index = 0usize;
        while let Ok(event) = stream_rx.recv() {
            let Some(command) = command_run_stream_event_command(event) else {
                continue;
            };
            let completed = runtime.block_on(executor.push_command_value(command));
            emit_cli_live_command_run_results(&completed, &mut live_item_index);
            if !completed.is_empty() {
                {
                    let mut shared = command_results_for_task
                        .lock()
                        .unwrap_or_else(|err| err.into_inner());
                    shared.extend(completed.clone());
                }
                *last_command_result_for_task
                    .lock()
                    .unwrap_or_else(|err| err.into_inner()) = Some(Instant::now());
            }
            results.extend(completed);
            if executor.is_halted() {
                command_cancelled_for_task.store(true, Ordering::SeqCst);
                break;
            }
        }
        let halted_before_finish = executor.is_halted();
        let completed = runtime.block_on(executor.finish());
        emit_cli_live_command_run_results(&completed, &mut live_item_index);
        if !completed.is_empty() {
            {
                let mut shared = command_results_for_task
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                shared.extend(completed.clone());
            }
            *last_command_result_for_task
                .lock()
                .unwrap_or_else(|err| err.into_inner()) = Some(Instant::now());
        }
        results.extend(completed);
        if halted_before_finish {
            command_cancelled_for_task.store(true, Ordering::SeqCst);
        }
        results
    });

    let post_command_result_timeout = streamed_command_run_post_result_timeout();
    let route_config_for_task = route_config.clone();
    let tura_config_for_task = Arc::clone(tura_config);
    let provider_task = tokio::spawn(async move {
        route_config_for_task
            .run_with_stream_events(tura_config_for_task.as_ref(), messages, options, Some(sink))
            .await
    });
    tokio::pin!(provider_task);
    let timeout_sleep = tokio::time::sleep(timeout_duration);
    tokio::pin!(timeout_sleep);
    let response = loop {
        tokio::select! {
            _ = &mut timeout_sleep => {
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
            provider_task.abort();
            return Ok(());
            }
            response = &mut provider_task => {
                break match response {
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
                    Err(e) => {
                        let finished_at = Utc::now();
                        let message = format!("runtime provider task failed: {e}");
                        error!(error = %message, "runtime provider task failed");
                        runtime.set_output(serde_json::json!({
                            "error": message
                        }));
                        finish_runtime_failure(
                            runtime,
                            finished_at,
                            "CALL_FAILED",
                            message,
                            RuntimeCallResultStatus::Failed,
                        )?;
                        return Ok(());
                    }
                };
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                let should_cancel_from_streamed_command_run =
                    streamed_command_run_cancelled.load(Ordering::SeqCst)
                        && !streamed_command_results
                            .lock()
                            .unwrap_or_else(|err| err.into_inner())
                            .is_empty();
                if should_cancel_from_streamed_command_run {
                    let finished_at = Utc::now();
                    let results = streamed_command_results
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .clone();
                    runtime.set_output(serde_json::json!({
                        "streamed_command_run_result": {
                            "results": results,
                            "early_finish_reason": "apply_patch_failed",
                            "cancelled": true,
                        }
                    }));
                    runtime.push_tool_call(ToolCallRecord {
                        tool_called_name: COMMAND_RUN_TOOL_NAME.to_string(),
                        tool_called_input: serde_json::json!({}),
                        provider_metadata: None,
                        tool_received_at: finished_at,
                        tool_executed_at: finished_at,
                        tool_calldata_received_at: finished_at,
                        tool_reported_success: false,
                        agent_reported_success: false,
                        agent_reported_helpful: false,
                        agent_reported_summary: String::new(),
                        validator_reported_success: None,
                    });
                    runtime
                        .mark_first_token(
                            first_stream_output_at
                                .lock()
                                .unwrap_or_else(|err| err.into_inner())
                                .unwrap_or(finished_at),
                        )
                        .map_err(|e| format!("failed to mark first token: {}", e))?;
                    finish_runtime_failure(
                        runtime,
                        finished_at,
                        "COMMAND_RUN_CANCELLED",
                        "apply_patch failed; runtime stream cancelled after command_run result"
                            .to_string(),
                        RuntimeCallResultStatus::Cancelled,
                    )?;
                    provider_task.abort();
                    let _ = command_task.join();
                    return Ok(());
                }
                let should_finish_from_task_status_done = {
                    let results = streamed_command_results
                        .lock()
                        .unwrap_or_else(|err| err.into_inner());
                    command_run_results_contain_task_status_done(&results)
                };
                if should_finish_from_task_status_done {
                    let finished_at = Utc::now();
                    let results = streamed_command_results
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .clone();
                    runtime.set_output(serde_json::json!({
                        "provider_content": {
                            "text": "command_run reported task_status done; advancing with completed command_run results."
                        },
                        "streamed_command_run_result": {
                            "results": results,
                            "early_finish_reason": "task_status_done",
                        }
                    }));
                    runtime.push_tool_call(ToolCallRecord {
                        tool_called_name: COMMAND_RUN_TOOL_NAME.to_string(),
                        tool_called_input: serde_json::json!({}),
                        provider_metadata: None,
                        tool_received_at: finished_at,
                        tool_executed_at: finished_at,
                        tool_calldata_received_at: finished_at,
                        tool_reported_success: false,
                        agent_reported_success: false,
                        agent_reported_helpful: false,
                        agent_reported_summary: String::new(),
                        validator_reported_success: None,
                    });
                    runtime
                        .mark_first_token(
                            first_stream_output_at
                                .lock()
                                .unwrap_or_else(|err| err.into_inner())
                                .unwrap_or(finished_at),
                        )
                        .map_err(|e| format!("failed to mark first token: {}", e))?;
                    runtime
                        .finish_success(finished_at, None)
                        .map_err(|e| format!("failed to finish runtime success: {}", e))?;
                    provider_task.abort();
                    return Ok(());
                }
                let should_finish_from_streamed_command_run = {
                    let has_results = !streamed_command_results
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .is_empty();
                    let last_result_at = *last_streamed_command_result_at
                        .lock()
                        .unwrap_or_else(|err| err.into_inner());
                    has_results
                        && last_result_at
                            .map(|last| last.elapsed() >= post_command_result_timeout)
                            .unwrap_or(false)
                };
                if should_finish_from_streamed_command_run {
                    let finished_at = Utc::now();
                    let results = streamed_command_results
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .clone();
                    runtime.set_output(serde_json::json!({
                        "provider_content": {
                            "text": "Provider stream did not finish after streamed command_run completed; advancing with completed command_run results."
                        },
                        "streamed_command_run_result": {
                            "results": results,
                            "early_finish_reason": "post_command_run_stream_timeout",
                            "post_result_timeout_ms": post_command_result_timeout.as_millis(),
                        }
                    }));
                    runtime.push_tool_call(ToolCallRecord {
                        tool_called_name: COMMAND_RUN_TOOL_NAME.to_string(),
                        tool_called_input: serde_json::json!({}),
                        provider_metadata: None,
                        tool_received_at: finished_at,
                        tool_executed_at: finished_at,
                        tool_calldata_received_at: finished_at,
                        tool_reported_success: false,
                        agent_reported_success: false,
                        agent_reported_helpful: false,
                        agent_reported_summary: String::new(),
                        validator_reported_success: None,
                    });
                    runtime
                        .mark_first_token(
                            first_stream_output_at
                                .lock()
                                .unwrap_or_else(|err| err.into_inner())
                                .unwrap_or(finished_at),
                        )
                        .map_err(|e| format!("failed to mark first token: {}", e))?;
                    runtime
                        .finish_success(finished_at, None)
                        .map_err(|e| format!("failed to finish runtime success: {}", e))?;
                    provider_task.abort();
                    return Ok(());
                }
            }
        }
    };
    let finished_at = Utc::now();
    let streamed_command_results = command_task.join().unwrap_or_default();

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

fn streamed_command_run_post_result_timeout() -> Duration {
    let millis = std::env::var("TURA_STREAMED_COMMAND_RUN_POST_RESULT_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_STREAMED_COMMAND_RUN_POST_RESULT_TIMEOUT_MS);
    Duration::from_millis(millis.max(1))
}

fn command_run_results_contain_task_status_done(results: &[serde_json::Value]) -> bool {
    results.iter().any(|item| {
        item.get("success").and_then(serde_json::Value::as_bool) == Some(true)
            && item
                .get("command_type")
                .or_else(|| item.get("command"))
                .and_then(serde_json::Value::as_str)
                == Some("task_status")
            && item
                .get("output")
                .and_then(|output| output.get("task_status"))
                .and_then(|status| status.get("status"))
                .and_then(serde_json::Value::as_str)
                == Some("done")
    })
}

fn emit_cli_live_command_run_results(results: &[serde_json::Value], item_index: &mut usize) {
    if !env_flag("TURA_CLI_LIVE_JSONL") {
        return;
    }
    for event in cli_live_command_run_events(results, item_index) {
        println!("{event}");
    }
    let _ = std::io::stdout().flush();
}

fn cli_live_command_run_events(
    results: &[serde_json::Value],
    item_index: &mut usize,
) -> Vec<serde_json::Value> {
    let mut events = Vec::new();
    for result in results {
        let command_type = result
            .get("command_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("command");
        let success = result
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let status = if success { "completed" } else { "failed" };
        let aggregated_output = cli_live_command_aggregated_output(command_type, result);
        let item_type = if command_type == "apply_patch" {
            "file_change"
        } else {
            "command_execution"
        };
        events.push(serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": format!("item_live_command_{}", *item_index),
                "type": item_type,
                "command": command_type,
                "aggregated_output": aggregated_output,
                "status": status,
            }
        }));
        *item_index += 1;
    }
    events
}

fn cli_live_command_aggregated_output(command_type: &str, result: &serde_json::Value) -> String {
    result
        .get("output")
        .map(|output| {
            let output = if command_type == "read_media" {
                redacted_read_media_output(output)
            } else {
                output.clone()
            };
            output.as_str().map(ToString::to_string).unwrap_or_else(|| {
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| output.to_string())
            })
        })
        .or_else(|| {
            result
                .get("error")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_default()
}

fn redacted_read_media_output(output: &serde_json::Value) -> serde_json::Value {
    let mut redacted = output.clone();
    redact_media_payload_data(&mut redacted);
    redacted
}

fn redact_media_payload_data(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            let preview_count = object
                .get("visual_preview_count")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if object.contains_key("visual_previews") {
                object.insert(
                    "visual_previews".to_string(),
                    serde_json::json!({
                        "redacted_from_cli_log": true,
                        "count": preview_count,
                        "reason": "media payload is sent through the provider media channel"
                    }),
                );
            }
            let audio_count = object
                .get("audio_preview_count")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if object.contains_key("audio_previews") {
                object.insert(
                    "audio_previews".to_string(),
                    serde_json::json!({
                        "redacted_from_cli_log": true,
                        "count": audio_count,
                        "reason": "media payload is sent through the provider media channel"
                    }),
                );
            }
            let file_count = object
                .get("file_attachment_count")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if object.contains_key("file_attachments") {
                object.insert(
                    "file_attachments".to_string(),
                    serde_json::json!({
                        "redacted_from_cli_log": true,
                        "count": file_count,
                        "reason": "file payload is sent through the provider file channel"
                    }),
                );
            }
            if let Some(serde_json::Value::String(url)) = object.get_mut("url") {
                if is_base64_data_url(url) {
                    *url = "[redacted media data URL]".to_string();
                }
            }
            if let Some(serde_json::Value::String(data)) = object.get_mut("data_base64") {
                if !data.is_empty() {
                    *data = "[redacted base64 file payload]".to_string();
                }
            }
            for child in object.values_mut() {
                redact_media_payload_data(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_media_payload_data(item);
            }
        }
        _ => {}
    }
}

fn is_base64_data_url(value: &str) -> bool {
    value.starts_with("data:") && value.contains(";base64,")
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
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
        input_tokens: m.usage.input_tokens.unwrap_or(0),
        output_tokens: m.usage.output_tokens.unwrap_or(0),
        total_tokens: m.usage.total_tokens.unwrap_or(0),
        cached_input_tokens: m.usage.cached_input_tokens.unwrap_or(0),
        cache_write_tokens: m.usage.cache_write_tokens.unwrap_or(0),
        reasoning_tokens: m.usage.reasoning_tokens.unwrap_or(0),
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

    if let Some(text) = extract_response_text(&content).map(|text| strip_thought_blocks(&text)) {
        runtime.append_text(&text);
    }

    for tool_call in extract_tool_calls(&content) {
        if suppress_command_run_tool_calls && tool_call.tool_name == COMMAND_RUN_TOOL_NAME {
            continue;
        }
        runtime.push_tool_call(ToolCallRecord {
            tool_called_name: tool_call.tool_name,
            tool_called_input: tool_call.arguments,
            provider_metadata: tool_call.provider_metadata,
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
    use super::{
        cli_live_command_run_events, prompt_cache_key, session_reasoning_effort,
        session_service_tier, stream_options,
    };
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    const REASONING_ENV: &str = "TURA_SESSION_REASONING_EFFORT";
    const ACCEL_ENV: &str = "TURA_SESSION_ACCELERATION_ENABLED";
    const DISABLE_CACHE_ENV: &str = "TURA_DISABLE_PROMPT_CACHE";

    fn with_env<T>(name: &str, value: Option<&str>, run: impl FnOnce() -> T) -> T {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("prompt cache env lock poisoned");
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
    fn cli_live_command_run_events_emit_per_completed_command() {
        let mut item_index = 0;
        let events = cli_live_command_run_events(
            &[serde_json::json!({
                "command_type": "apply_patch",
                "success": false,
                "output": {
                    "error_type": "ContextMismatch",
                    "message": "patch context not found"
                }
            })],
            &mut item_index,
        );

        assert_eq!(item_index, 1);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "item.completed");
        assert_eq!(events[0]["item"]["type"], "file_change");
        assert_eq!(events[0]["item"]["status"], "failed");
        assert!(events[0]["item"]["aggregated_output"]
            .as_str()
            .is_some_and(|text| text.contains("ContextMismatch")));
    }

    #[test]
    fn cli_live_command_run_events_redact_read_media_payloads() {
        let mut item_index = 0;
        let events = cli_live_command_run_events(
            &[serde_json::json!({
                "command_type": "read_media",
                "success": true,
                "output": {
                    "summary_markdown": "- reference/desktop.png: image, 1 visual preview",
                    "visual_preview_count": 1,
                    "visual_previews": [{
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/jpeg;base64,AAA"
                        }
                    }],
                    "media_results": [{
                        "path": "reference/desktop.png",
                        "visual_preview_count": 1,
                        "visual_previews": [{
                            "type": "image_url",
                            "image_url": {
                                "url": "data:image/jpeg;base64,BBB"
                            }
                        }],
                        "file_attachment_count": 1,
                        "file_attachments": [{
                            "data_base64": "QUJD"
                        }]
                    }]
                }
            })],
            &mut item_index,
        );

        let output = events[0]["item"]["aggregated_output"]
            .as_str()
            .expect("aggregated output is text");
        assert!(output.contains("reference/desktop.png"));
        assert!(output.contains("redacted_from_cli_log"));
        assert!(!output.contains("data:image/jpeg;base64"));
        assert!(!output.contains("QUJD"));
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
                model: "gpt-5.1-codex-mini".to_string(),
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
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_a),
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_b)
            );
            assert_eq!(
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_a),
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_b)
            );
            assert!(
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &tools_a)
                    .expect("prompt cache key should be generated")
                    .starts_with("turaosv2:tura-coder:sess-a:")
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
        let route = tura_llm_rust::RouteConfig {
            default_temperature: 0.2,
            providers: vec![tura_llm_rust::ProviderConfig {
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5.1-codex-mini".to_string(),
                temperature: 0.2,
            }],
        };
        with_env(DISABLE_CACHE_ENV, Some("true"), || {
            assert_eq!(
                prompt_cache_key(&route, "flagship_thinking", &"sess-a".to_string(), &[]),
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
                model: "gpt-5.1-codex-mini".to_string(),
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
