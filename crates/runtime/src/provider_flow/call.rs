use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};
use tracing::error;

use crate::gateway_events::{
    emit_cli_live_command_run_results, emit_cli_live_command_run_started,
    publish_streamed_agent_text,
};
use crate::provider_flow::errors::{
    finish_runtime_failure, finish_runtime_failure_with_usage, runtime_timeout,
};
use crate::provider_flow::provider_response::{
    apply_provider_response, apply_provider_response_with_options,
};
pub use crate::provider_flow::request_options::route_by_name;
use crate::provider_flow::request_options::{
    normalize_provider_messages, parallel_tool_calls_enabled, prompt_cache_key, session_max_tokens,
    session_model_override_route, session_reasoning_effort, session_service_tier, stream_options,
};
use crate::provider_flow::streamed_command_run::{
    command_run_live_delta_result, command_run_stream_event_command,
    command_run_stream_events_from_provider_content, publish_streamed_command_run_update,
    should_replay_final_response_command_run, streamed_command_event_record,
    streamed_command_result_record, streamed_command_run_call_id, StreamedCommandEvent,
    StreamedCommandRunUpdate,
};
use crate::provider_flow::usage::{
    estimated_usage_report_for_interrupted_runtime, usage_report_from_metrics,
};
use crate::router_command_run::RouterCommandRunExecutor;
use crate::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeManagement, RuntimeState, ToolCallRecord,
};
use crate::state_machine::session_management::SessionId;

use crate::runtime::types::RuntimeQueueItem;

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
    pub allowed_command_run_commands: Option<BTreeSet<String>>,
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
        .map_err(|e| format!("failed to transition runtime to Dispatching: {e}"))?;
    runtime
        .mark_called(now)
        .map_err(|e| format!("failed to mark runtime called: {e}"))?;
    runtime
        .mark_waiting_first_token()
        .map_err(|e| format!("failed to mark runtime waiting for first token: {e}"))?;
    crate::checkpoint::checkpoint_turn_started(&runtime)?;

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

    crate::checkpoint::checkpoint_provider_call_started(&runtime)?;

    let call_result = if input.stream || !input_tools.is_empty() {
        call_runtime_streaming(
            &mut runtime,
            route_config,
            &tura_config,
            provider_messages,
            call_options,
            input.session_directory.clone(),
            input.allowed_command_run_commands.clone(),
        )
        .await
    } else {
        call_runtime_non_streaming(
            &mut runtime,
            route_config,
            &tura_config,
            provider_messages,
            call_options,
        )
        .await
    };

    match call_result {
        Ok(()) => {
            crate::checkpoint::checkpoint_provider_call_finished(&runtime)?;
            match runtime.call_result_status {
                RuntimeCallResultStatus::Failed | RuntimeCallResultStatus::TimedOut => {
                    crate::checkpoint::checkpoint_turn_failed(&runtime)?
                }
                RuntimeCallResultStatus::Cancelled => {
                    crate::checkpoint::checkpoint_turn_interrupted(&runtime)?
                }
                _ => crate::checkpoint::checkpoint_turn_finished(&runtime)?,
            }
        }
        Err(error) => {
            let _ = crate::checkpoint::checkpoint_turn_failed(&runtime);
            return Err(error);
        }
    }

    Ok(runtime)
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
            let usage = estimated_usage_report_for_interrupted_runtime(
                runtime,
                started_at,
                finished_at,
                finished_at,
                "runtime_estimate_timeout",
            );
            finish_runtime_failure_with_usage(
                runtime,
                finished_at,
                "CALL_TIMED_OUT",
                message,
                RuntimeCallResultStatus::TimedOut,
                Some(usage),
            )?;
        }
        Ok(Ok(response)) => {
            let finished_at = Utc::now();
            runtime.set_output(response.content.clone());
            apply_provider_response(runtime, &response.content, finished_at);

            runtime
                .mark_first_token(finished_at)
                .map_err(|e| format!("failed to mark first token: {e}"))?;

            let usage =
                usage_report_from_metrics(response.metrics, started_at, finished_at, finished_at);

            runtime
                .finish_success(finished_at, usage)
                .map_err(|e| format!("failed to finish runtime success: {e}"))?;
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

async fn call_runtime_streaming(
    runtime: &mut RuntimeManagement,
    route_config: &tura_llm_rust::RouteConfig,
    tura_config: &Arc<tura_llm_rust::TuraConfig>,
    messages: Vec<serde_json::Value>,
    options: tura_llm_rust::CallOptions,
    session_directory: PathBuf,
    allowed_command_run_commands: Option<BTreeSet<String>>,
) -> Result<(), String> {
    let started_at = Utc::now();
    let timeout_duration = runtime_timeout(runtime);
    let (stream_tx, stream_rx) = mpsc::channel::<tura_llm_rust::ProviderStreamEvent>();
    let final_response_stream_tx = stream_tx.clone();
    // Forward incremental assistant text tokens to the gateway on a dedicated
    // thread so the streaming HTTP POSTs never block the provider stream loop.
    let (text_delta_tx, text_delta_rx) = mpsc::channel::<String>();
    let text_delta_session_id = runtime.session_id.clone();
    let text_delta_runtime_id = runtime.runtime_id.clone();
    let _text_delta_thread = std::thread::spawn(move || {
        let Ok(async_runtime) = tokio::runtime::Runtime::new() else {
            return;
        };
        while let Ok(delta) = text_delta_rx.recv() {
            async_runtime.block_on(publish_streamed_agent_text(
                &text_delta_session_id,
                &text_delta_runtime_id,
                &delta,
            ));
        }
    });
    let first_stream_output_at: Arc<Mutex<Option<DateTime<Utc>>>> = Arc::new(Mutex::new(None));
    let streamed_command_results: Arc<Mutex<Vec<serde_json::Value>>> =
        Arc::new(Mutex::new(Vec::new()));
    let streamed_command_inputs: Arc<Mutex<Vec<serde_json::Value>>> =
        Arc::new(Mutex::new(Vec::new()));
    let streamed_command_events: Arc<Mutex<Vec<serde_json::Value>>> =
        Arc::new(Mutex::new(Vec::new()));
    let streamed_command_seen = Arc::new(AtomicBool::new(false));
    let last_streamed_command_result_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let streamed_command_run_cancelled = Arc::new(AtomicBool::new(false));
    let first_stream_output_for_sink = Arc::clone(&first_stream_output_at);
    let streamed_command_seen_for_sink = Arc::clone(&streamed_command_seen);
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
        if matches!(
            event,
            tura_llm_rust::ProviderStreamEvent::CommandRunCommandReady { .. }
        ) {
            streamed_command_seen_for_sink.store(true, Ordering::SeqCst);
        }
        if let tura_llm_rust::ProviderStreamEvent::TextDelta { text } = &event {
            let _ = text_delta_tx.send(text.clone());
        }
        let _ = stream_tx.send(event);
    });
    let command_session_directory = session_directory.clone();
    let command_results_for_task = Arc::clone(&streamed_command_results);
    let command_inputs_for_task = Arc::clone(&streamed_command_inputs);
    let command_events_for_task = Arc::clone(&streamed_command_events);
    let last_command_result_for_task = Arc::clone(&last_streamed_command_result_at);
    let command_cancelled_for_task = Arc::clone(&streamed_command_run_cancelled);
    let gateway_session_id = runtime.session_id.clone();
    let gateway_runtime_id = runtime.runtime_id.clone();
    let gateway_provider = serde_json::to_value(&runtime.provider).unwrap_or(Value::Null);
    let gateway_call_id = streamed_command_run_call_id(&gateway_runtime_id);
    let command_task = std::thread::spawn(move || {
        let mut executor = RouterCommandRunExecutor::new_with_allowed(
            command_session_directory,
            allowed_command_run_commands,
            gateway_session_id.clone(),
            gateway_runtime_id.clone(),
        );
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(_) => return Vec::new(),
        };
        let mut results = Vec::new();
        let mut streamed_commands = Vec::new();
        let mut command_run_started = false;
        let mut live_item_index = 0usize;
        while let Ok(event) = stream_rx.recv() {
            let Some(command_event) = command_run_stream_event_command(event) else {
                continue;
            };
            let StreamedCommandEvent {
                tool_call_id,
                command_index,
                command,
            } = command_event;
            if !command_run_started {
                if let Err(error) = crate::checkpoint::checkpoint_command_run_started(
                    &gateway_session_id,
                    &gateway_runtime_id,
                    &gateway_call_id,
                ) {
                    tracing::warn!(
                        session_id = %gateway_session_id,
                        runtime_id = %gateway_runtime_id,
                        error = %error,
                        "failed to persist command_run_started checkpoint"
                    );
                }
                command_run_started = true;
            }
            streamed_commands.push(command.clone());
            command_inputs_for_task
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .push(command.clone());
            emit_cli_live_command_run_started(&command, &tool_call_id, command_index);
            command_events_for_task
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .push(streamed_command_event_record(
                    "ready",
                    &gateway_runtime_id,
                    &tool_call_id,
                    command_index,
                    &command,
                    None,
                ));
            if let Err(error) = crate::checkpoint::checkpoint_command_ready(
                &gateway_session_id,
                &gateway_runtime_id,
                &gateway_call_id,
                &tool_call_id,
                command_index,
                &command,
            ) {
                tracing::warn!(
                    session_id = %gateway_session_id,
                    runtime_id = %gateway_runtime_id,
                    error = %error,
                    "failed to persist command_ready checkpoint"
                );
            }
            let mut live_results = results.clone();
            live_results.push(command_run_live_delta_result(&command, "", ""));
            runtime.block_on(publish_streamed_command_run_update(
                StreamedCommandRunUpdate {
                    session_id: &gateway_session_id,
                    runtime_id: &gateway_runtime_id,
                    provider: &gateway_provider,
                    call_id: &gateway_call_id,
                    commands: &streamed_commands,
                    results: &live_results,
                    status: "running",
                    started_at,
                    ended_at: None,
                },
            ));
            let delta_poll_stop = Arc::new(AtomicBool::new(false));
            let delta_poll_stop_for_task = Arc::clone(&delta_poll_stop);
            let delta_event_ctx = executor.event_context();
            let delta_event_start = delta_event_ctx.events().len();
            let delta_gateway_session_id = gateway_session_id.clone();
            let delta_gateway_runtime_id = gateway_runtime_id.clone();
            let delta_gateway_provider = gateway_provider.clone();
            let delta_gateway_call_id = gateway_call_id.clone();
            let delta_commands = streamed_commands.clone();
            let delta_base_results = results.clone();
            let delta_command = command.clone();
            if let Err(error) = crate::checkpoint::checkpoint_command_started(
                &gateway_session_id,
                &gateway_runtime_id,
                &gateway_call_id,
                &tool_call_id,
                command_index,
                &command,
            ) {
                tracing::warn!(
                    session_id = %gateway_session_id,
                    runtime_id = %gateway_runtime_id,
                    error = %error,
                    "failed to persist command_started checkpoint"
                );
            }
            let delta_poll_task = std::thread::spawn(move || {
                let Ok(runtime) = tokio::runtime::Runtime::new() else {
                    return;
                };
                let mut seen = delta_event_start;
                let mut stdout = String::new();
                let mut stderr = String::new();
                loop {
                    let events = delta_event_ctx.events();
                    let mut changed = false;
                    for event in events.iter().skip(seen) {
                        if let code_tools::runtime::tool::ToolRuntimeEvent::OutputDelta {
                            stream,
                            text,
                            ..
                        } = event
                        {
                            if stream == "stderr" {
                                stderr.push_str(text);
                            } else {
                                stdout.push_str(text);
                            }
                            changed = true;
                        }
                    }
                    seen = events.len();
                    if changed {
                        let mut live_results = delta_base_results.clone();
                        live_results.push(command_run_live_delta_result(
                            &delta_command,
                            &stdout,
                            &stderr,
                        ));
                        runtime.block_on(publish_streamed_command_run_update(
                            StreamedCommandRunUpdate {
                                session_id: &delta_gateway_session_id,
                                runtime_id: &delta_gateway_runtime_id,
                                provider: &delta_gateway_provider,
                                call_id: &delta_gateway_call_id,
                                commands: &delta_commands,
                                results: &live_results,
                                status: "running",
                                started_at,
                                ended_at: None,
                            },
                        ));
                    }
                    if delta_poll_stop_for_task.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
            });
            let completed = runtime.block_on(executor.push_command_value(command));
            delta_poll_stop.store(true, Ordering::SeqCst);
            let _ = delta_poll_task.join();
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
            for (offset, result) in completed.iter().enumerate() {
                if let Err(error) = crate::checkpoint::checkpoint_streamed_command_finished(
                    crate::checkpoint::StreamedCommandCheckpoint {
                        session_id: &gateway_session_id,
                        turn_id: &gateway_runtime_id,
                        runtime_worker_id: &gateway_runtime_id,
                        command_run_id: &gateway_call_id,
                        index: results.len() + offset,
                        result,
                    },
                ) {
                    error!(
                        session_id = %gateway_session_id,
                        runtime_id = %gateway_runtime_id,
                        error = %error,
                        "session_db command checkpoint ACK failed"
                    );
                    command_cancelled_for_task.store(true, Ordering::SeqCst);
                    break;
                }
                command_events_for_task
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push(streamed_command_result_record(
                        "completed",
                        &gateway_runtime_id,
                        results.len() + offset,
                        result,
                    ));
            }
            results.extend(completed);
            if command_cancelled_for_task.load(Ordering::SeqCst) {
                break;
            }
            runtime.block_on(publish_streamed_command_run_update(
                StreamedCommandRunUpdate {
                    session_id: &gateway_session_id,
                    runtime_id: &gateway_runtime_id,
                    provider: &gateway_provider,
                    call_id: &gateway_call_id,
                    commands: &streamed_commands,
                    results: &results,
                    status: "running",
                    started_at,
                    ended_at: None,
                },
            ));
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
        for (offset, result) in completed.iter().enumerate() {
            if let Err(error) = crate::checkpoint::checkpoint_streamed_command_finished(
                crate::checkpoint::StreamedCommandCheckpoint {
                    session_id: &gateway_session_id,
                    turn_id: &gateway_runtime_id,
                    runtime_worker_id: &gateway_runtime_id,
                    command_run_id: &gateway_call_id,
                    index: results.len() + offset,
                    result,
                },
            ) {
                error!(
                    session_id = %gateway_session_id,
                    runtime_id = %gateway_runtime_id,
                    error = %error,
                    "session_db command checkpoint ACK failed"
                );
                command_cancelled_for_task.store(true, Ordering::SeqCst);
                break;
            }
            command_events_for_task
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .push(streamed_command_result_record(
                    "completed",
                    &gateway_runtime_id,
                    results.len() + offset,
                    result,
                ));
        }
        results.extend(completed);
        let checkpoint_ack_failed = command_cancelled_for_task.load(Ordering::SeqCst);
        if !streamed_commands.is_empty() {
            runtime.block_on(publish_streamed_command_run_update(
                StreamedCommandRunUpdate {
                    session_id: &gateway_session_id,
                    runtime_id: &gateway_runtime_id,
                    provider: &gateway_provider,
                    call_id: &gateway_call_id,
                    commands: &streamed_commands,
                    results: &results,
                    status: if halted_before_finish || checkpoint_ack_failed {
                        "error"
                    } else {
                        "completed"
                    },
                    started_at,
                    ended_at: Some(Utc::now()),
                },
            ));
            let command_run_status = if halted_before_finish || checkpoint_ack_failed {
                "error"
            } else {
                "completed"
            };
            if let Err(error) = crate::checkpoint::checkpoint_command_run_finished(
                &gateway_session_id,
                &gateway_runtime_id,
                &gateway_call_id,
                command_run_status,
                results.len(),
            ) {
                tracing::warn!(
                    session_id = %gateway_session_id,
                    runtime_id = %gateway_runtime_id,
                    error = %error,
                    "failed to persist command_run_finished checkpoint"
                );
            }
        }
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
            let first_token_at = first_stream_output_at
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .unwrap_or(finished_at);
            let usage = estimated_usage_report_for_interrupted_runtime(
                runtime,
                started_at,
                finished_at,
                first_token_at,
                "runtime_estimate_timeout",
            );
            finish_runtime_failure_with_usage(
                runtime,
                finished_at,
                "CALL_TIMED_OUT",
                message,
                RuntimeCallResultStatus::TimedOut,
                Some(usage),
            )?;
            provider_task.abort();
            let _ = (&mut provider_task).await;
            drop(final_response_stream_tx);
            let _ = command_task.join();
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
                        drop(final_response_stream_tx);
                        let _ = command_task.join();
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
                        drop(final_response_stream_tx);
                        let _ = command_task.join();
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
                    let commands = streamed_command_inputs
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .clone();
                    let events = streamed_command_events
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .clone();
                    apply_cancelled_streamed_command_run_result(
                        runtime,
                        &commands,
                        &events,
                        &results,
                        finished_at,
                    );
                    runtime
                        .mark_first_token(
                            first_stream_output_at
                                .lock()
                                .unwrap_or_else(|err| err.into_inner())
                                .unwrap_or(finished_at),
                        )
                        .map_err(|e| format!("failed to mark first token: {e}"))?;
                    let usage = estimated_usage_report_for_interrupted_runtime(
                        runtime,
                        started_at,
                        finished_at,
                        first_stream_output_at
                            .lock()
                            .unwrap_or_else(|err| err.into_inner())
                            .unwrap_or(finished_at),
                        "runtime_estimate_cancelled",
                    );
                    finish_runtime_failure_with_usage(
                        runtime,
                        finished_at,
                        "COMMAND_RUN_CANCELLED",
                        "apply_patch failed; runtime stream cancelled after command_run result"
                            .to_string(),
                        RuntimeCallResultStatus::Cancelled,
                        Some(usage),
                    )?;
                    provider_task.abort();
                    let _ = (&mut provider_task).await;
                    drop(final_response_stream_tx);
                    let _ = command_task.join();
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
                    let commands = streamed_command_inputs
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .clone();
                    let events = streamed_command_events
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .clone();
                    apply_post_result_timeout_streamed_command_run_result(
                        runtime,
                        &commands,
                        &events,
                        &results,
                        post_command_result_timeout,
                        finished_at,
                    );
                    runtime
                        .mark_first_token(
                            first_stream_output_at
                                .lock()
                                .unwrap_or_else(|err| err.into_inner())
                                .unwrap_or(finished_at),
                        )
                        .map_err(|e| format!("failed to mark first token: {e}"))?;
                    let usage = estimated_usage_report_for_interrupted_runtime(
                        runtime,
                        started_at,
                        finished_at,
                        first_stream_output_at
                            .lock()
                            .unwrap_or_else(|err| err.into_inner())
                            .unwrap_or(finished_at),
                        "runtime_estimate_post_command_run_stream_timeout",
                    );
                    runtime
                        .finish_success(finished_at, Some(usage))
                        .map_err(|e| format!("failed to finish runtime success: {e}"))?;
                    provider_task.abort();
                    let _ = (&mut provider_task).await;
                    drop(final_response_stream_tx);
                    let _ = command_task.join();
                    return Ok(());
                }
            }
        }
    };
    let finished_at = Utc::now();
    if should_replay_final_response_command_run(streamed_command_seen.load(Ordering::SeqCst)) {
        for event in command_run_stream_events_from_provider_content(&response.content) {
            let _ = final_response_stream_tx.send(event);
        }
    }
    drop(final_response_stream_tx);
    let streamed_command_results = command_task.join().unwrap_or_default();
    let streamed_command_inputs = streamed_command_inputs
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone();
    let streamed_command_events = streamed_command_events
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone();

    let mut runtime_output = response.content.clone();
    if !streamed_command_results.is_empty() {
        runtime_output = serde_json::json!({
            "provider_content": response.content,
            "streamed_command_run_result": {
                "commands": streamed_command_inputs,
                "command_events": streamed_command_events,
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
        .map_err(|e| format!("failed to mark first token: {e}"))?;

    let usage =
        usage_report_from_metrics(response.metrics, started_at, finished_at, first_token_at);

    runtime
        .finish_success(finished_at, usage)
        .map_err(|e| format!("failed to finish runtime success: {e}"))?;

    Ok(())
}

fn streamed_command_run_post_result_timeout() -> Duration {
    let millis = std::env::var("TURA_STREAMED_COMMAND_RUN_POST_RESULT_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_STREAMED_COMMAND_RUN_POST_RESULT_TIMEOUT_MS);
    Duration::from_millis(millis.max(1))
}

fn apply_cancelled_streamed_command_run_result(
    runtime: &mut RuntimeManagement,
    commands: &[serde_json::Value],
    events: &[serde_json::Value],
    results: &[serde_json::Value],
    finished_at: DateTime<Utc>,
) {
    runtime.set_output(cancelled_streamed_command_run_output(
        commands, events, results,
    ));
    runtime.push_tool_call(streamed_command_run_tool_record(commands, finished_at));
}

fn cancelled_streamed_command_run_output(
    commands: &[serde_json::Value],
    events: &[serde_json::Value],
    results: &[serde_json::Value],
) -> serde_json::Value {
    serde_json::json!({
        "streamed_command_run_result": {
            "commands": commands,
            "command_events": events,
            "results": results,
            "early_finish_reason": "apply_patch_failed",
            "cancelled": true,
        }
    })
}

fn apply_post_result_timeout_streamed_command_run_result(
    runtime: &mut RuntimeManagement,
    commands: &[serde_json::Value],
    events: &[serde_json::Value],
    results: &[serde_json::Value],
    post_command_result_timeout: Duration,
    finished_at: DateTime<Utc>,
) {
    runtime.set_output(post_result_timeout_streamed_command_run_output(
        commands,
        events,
        results,
        post_command_result_timeout,
    ));
    runtime.push_tool_call(streamed_command_run_tool_record(commands, finished_at));
}

fn post_result_timeout_streamed_command_run_output(
    commands: &[serde_json::Value],
    events: &[serde_json::Value],
    results: &[serde_json::Value],
    post_command_result_timeout: Duration,
) -> serde_json::Value {
    serde_json::json!({
        "provider_content": {
            "text": "Provider stream did not finish after streamed command_run completed; advancing with completed command_run results."
        },
        "streamed_command_run_result": {
            "commands": commands,
            "command_events": events,
            "results": results,
            "early_finish_reason": "post_command_run_stream_timeout",
            "post_result_timeout_ms": post_command_result_timeout.as_millis(),
        }
    })
}

fn streamed_command_run_tool_record(
    commands: &[serde_json::Value],
    finished_at: DateTime<Utc>,
) -> ToolCallRecord {
    ToolCallRecord {
        tool_called_name: COMMAND_RUN_TOOL_NAME.to_string(),
        tool_called_input: serde_json::json!({ "commands": commands }),
        provider_metadata: None,
        tool_received_at: finished_at,
        tool_executed_at: finished_at,
        tool_calldata_received_at: finished_at,
        tool_reported_success: false,
        agent_reported_success: false,
        agent_reported_helpful: false,
        agent_reported_summary: String::new(),
        validator_reported_success: None,
    }
}

pub async fn dequeue_runtime(
    session_id: &SessionId,
    redis_url: &str,
) -> Result<Option<RuntimeQueueItem>, String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {e}"))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {e}"))?;

    let queue_key = format!("runtime:queue:{session_id}");

    let result: Option<String> = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async(&mut con)
        .await
        .map_err(|e| format!("failed to dequeue runtime: {e}"))?;

    match result {
        Some(payload) => {
            let item: RuntimeQueueItem = serde_json::from_str(&payload)
                .map_err(|e| format!("failed to deserialize queue item: {e}"))?;
            Ok(Some(item))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_cancelled_streamed_command_run_result,
        apply_post_result_timeout_streamed_command_run_result, call_runtime,
        streamed_command_run_post_result_timeout, CallRuntimeInput,
    };
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{
        RuntimeManagement, RuntimeProviderConfig, RuntimeState,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::collections::{BTreeSet, HashMap};
    use std::sync::Arc;
    use std::time::Duration;
    use tura_llm_rust::{
        ModelCatalog, ProviderConfig as LlmProviderConfig, ProviderEnumCatalog, RouteConfig,
        Settings, TuraConfig,
    };

    fn runtime() -> RuntimeManagement {
        RuntimeManagement::new(
            "runtime-call-test".to_string(),
            "session-call-test".to_string(),
            "agent-call-test".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "fast".to_string(),
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 1024,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 30_000,
                },
                thinking: false,
                provider_name: "openai".to_string(),
                model_name: "gpt-test".to_string(),
                provider_url_name: "openai".to_string(),
                llm_provider_name: "openai".to_string(),
            },
            Utc::now(),
        )
    }

    fn missing_key_settings() -> Arc<Settings> {
        Arc::new(Settings {
            provider_base_url: HashMap::new(),
            routes: HashMap::from([(
                "missing-key-route".to_string(),
                RouteConfig {
                    default_temperature: 0.0,
                    providers: vec![LlmProviderConfig {
                        provider: "definitely_missing_provider_for_call_runtime_test".to_string(),
                        base_url: "http://127.0.0.1:9".to_string(),
                        model: "local-test-model".to_string(),
                        temperature: 0.0,
                    }],
                },
            )]),
            model_catalog: ModelCatalog::default(),
            provider_enums: ProviderEnumCatalog::default(),
        })
    }

    #[test]
    fn cancelled_streamed_command_run_result_marks_output_and_tool_record() {
        let mut runtime = runtime();
        let commands = vec![json!({ "command": "apply_patch failed" })];
        let events = vec![json!({ "status": "completed" })];
        let results = vec![json!({ "success": false, "error": "patch failed" })];
        let finished_at = runtime.created_at;

        apply_cancelled_streamed_command_run_result(
            &mut runtime,
            &commands,
            &events,
            &results,
            finished_at,
        );

        let output = runtime.output.as_ref().expect("output should be set");
        assert_eq!(
            output.pointer("/streamed_command_run_result/early_finish_reason"),
            Some(&json!("apply_patch_failed"))
        );
        assert_eq!(
            output.pointer("/streamed_command_run_result/cancelled"),
            Some(&json!(true))
        );
        assert_eq!(runtime.tool_call.len(), 1);
        assert_eq!(runtime.tool_call[0].tool_called_name, "command_run");
        assert_eq!(
            runtime.tool_call[0].tool_called_input,
            json!({ "commands": commands })
        );
        assert_eq!(runtime.tool_call[0].tool_received_at, finished_at);
        assert_eq!(runtime.state, RuntimeState::Created);
    }

    #[test]
    fn post_result_timeout_streamed_command_run_result_keeps_provider_notice() {
        let mut runtime = runtime();
        let commands = vec![json!({ "command": "echo ok" })];
        let events = vec![json!({ "status": "ready" })];
        let results = vec![json!({ "success": true, "output": "ok" })];
        let finished_at = runtime.created_at;

        apply_post_result_timeout_streamed_command_run_result(
            &mut runtime,
            &commands,
            &events,
            &results,
            Duration::from_millis(25),
            finished_at,
        );

        let output = runtime.output.as_ref().expect("output should be set");
        assert_eq!(
            output.pointer("/provider_content/text"),
            Some(&json!(
                "Provider stream did not finish after streamed command_run completed; advancing with completed command_run results."
            ))
        );
        assert_eq!(
            output.pointer("/streamed_command_run_result/early_finish_reason"),
            Some(&json!("post_command_run_stream_timeout"))
        );
        assert_eq!(
            output.pointer("/streamed_command_run_result/post_result_timeout_ms"),
            Some(&json!(25))
        );
        assert_eq!(runtime.tool_call.len(), 1);
        assert_eq!(
            runtime.tool_call[0].tool_called_input,
            json!({ "commands": commands })
        );
    }

    #[test]
    fn streamed_command_post_result_timeout_default_is_positive() {
        assert!(streamed_command_run_post_result_timeout() >= Duration::from_millis(1));
    }

    #[tokio::test]
    async fn call_runtime_provider_config_failure_finishes_failed_without_network() {
        std::env::remove_var("DEFINITELY_MISSING_PROVIDER_FOR_CALL_RUNTIME_TEST_API_KEY");
        let settings = missing_key_settings();
        let config = Arc::new(TuraConfig::new(".env.missing-for-call-runtime-test"));

        let runtime = call_runtime(
            CallRuntimeInput {
                runtime: runtime(),
                messages: vec![json!({ "role": "user", "content": "hello" })],
                tools: Vec::new(),
                provider_name: "missing-key-route".to_string(),
                stream: false,
                max_tokens: 128,
                tool_choice: None,
                session_directory: std::env::temp_dir(),
                allowed_command_run_commands: Some(BTreeSet::new()),
            },
            settings,
            config,
        )
        .await
        .expect("provider config failure should be captured on the runtime");

        assert_eq!(runtime.state, RuntimeState::Failed);
        assert_eq!(
            runtime.call_result_status,
            crate::state_machine::runtime_management::RuntimeCallResultStatus::Failed
        );
        let output = runtime.output.expect("failure output should be persisted");
        let error = output
            .get("error")
            .and_then(serde_json::Value::as_str)
            .expect("failure output should contain text");
        assert!(
            error.contains("API Key not found"),
            "unexpected failure output: {error}"
        );
        let runtime_error = runtime.error.expect("runtime error should be set");
        assert_eq!(runtime_error.error_code.as_deref(), Some("CALL_FAILED"));
        assert!(runtime_error
            .error_text
            .as_deref()
            .unwrap_or_default()
            .contains("API Key not found"));
    }
}
