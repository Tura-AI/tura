use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tracing::error;

use crate::gateway_events::publish_streamed_agent_text;
use crate::provider_flow::command_run_streaming::{
    apply_cancelled_streamed_command_run_result, spawn_streamed_command_run_task,
    SpawnStreamedCommandRunTask, StreamedCommandRunState,
};
use crate::provider_flow::errors::{finish_runtime_failure, runtime_timeout};
use crate::provider_flow::provider_response::apply_provider_response_with_options;
use crate::provider_flow::streamed_command_run::{
    command_run_stream_events_from_provider_content, streamed_command_run_call_id,
};
use crate::provider_flow::usage::usage_report_from_metrics;
use crate::state_machine::runtime_management::{RuntimeCallResultStatus, RuntimeManagement};

pub(crate) async fn call_runtime_streaming(
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
    let text_delta_runtime = runtime.clone();
    let _text_delta_thread = std::thread::spawn(move || {
        let Ok(async_runtime) = tokio::runtime::Runtime::new() else {
            return;
        };
        while let Ok(delta) = text_delta_rx.recv() {
            async_runtime.block_on(publish_streamed_agent_text(
                &text_delta_session_id,
                &text_delta_runtime,
                &delta,
            ));
        }
    });

    let first_stream_output_at: Arc<Mutex<Option<DateTime<Utc>>>> = Arc::new(Mutex::new(None));
    let command_state = StreamedCommandRunState::new();
    let first_stream_output_for_sink = Arc::clone(&first_stream_output_at);
    let command_state_for_sink = command_state.clone();
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
            command_state_for_sink.mark_seen();
        }
        if let tura_llm_rust::ProviderStreamEvent::TextDelta { text } = &event {
            let _ = text_delta_tx.send(text.clone());
        }
        let _ = stream_tx.send(event);
    });

    let gateway_session_id = runtime.session_id.clone();
    let gateway_runtime_id = runtime.runtime_id.clone();
    let gateway_provider = serde_json::to_value(&runtime.provider).unwrap_or(Value::Null);
    let gateway_call_id = streamed_command_run_call_id(&gateway_runtime_id);
    let command_task = spawn_streamed_command_run_task(SpawnStreamedCommandRunTask {
        stream_rx,
        session_directory,
        allowed_command_run_commands,
        session_id: gateway_session_id,
        runtime_id: gateway_runtime_id.clone(),
        provider: gateway_provider,
        call_id: gateway_call_id,
        started_at,
        state: command_state.clone(),
        runtime_status: runtime.session_sync_status(),
    });

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
                if command_state.should_cancel_after_results() {
                    let finished_at = Utc::now();
                    let snapshot = command_state.snapshot();
                    apply_cancelled_streamed_command_run_result(
                        runtime,
                        &snapshot.commands,
                        &snapshot.events,
                        &snapshot.results,
                        finished_at,
                    );
                    let first_token_at = first_stream_output_or(&first_stream_output_at, finished_at);
                    runtime
                        .mark_first_token(first_token_at)
                        .map_err(|e| format!("failed to mark first token: {e}"))?;
                    finish_runtime_failure(
                        runtime,
                        finished_at,
                        "COMMAND_RUN_CANCELLED",
                        "apply_patch failed; runtime stream cancelled after command_run result"
                            .to_string(),
                        RuntimeCallResultStatus::Cancelled,
                    )?;
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
    let tool_dispatch_content =
        response_content_for_tool_dispatch(&response.content, &response.raw);
    for event in command_run_stream_events_from_provider_content(&tool_dispatch_content) {
        let _ = final_response_stream_tx.send(event);
    }
    drop(final_response_stream_tx);
    let joined_command_results = command_task.join().unwrap_or_default();
    let snapshot = command_state.snapshot();
    let streamed_command_results = if joined_command_results.is_empty() {
        snapshot.results
    } else {
        joined_command_results
    };

    let mut runtime_output = response.content.clone();
    if !streamed_command_results.is_empty() {
        runtime_output = serde_json::json!({
            "provider_content": tool_dispatch_content,
            "streamed_command_run_result": {
                "commands": snapshot.commands,
                "command_events": snapshot.events,
                "results": streamed_command_results,
            }
        });
    }
    runtime.set_output(runtime_output);
    apply_provider_response_with_options(runtime, &tool_dispatch_content, finished_at, false);

    if let Some(stream) = response.content.get("stream").and_then(|s| s.as_array()) {
        for chunk in stream {
            if let Some(text) = chunk.get("text").and_then(|t| t.as_str()) {
                runtime.append_text(text);
            }
        }
    }

    let first_token_at = first_stream_output_or(&first_stream_output_at, finished_at);

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

fn response_content_for_tool_dispatch(content: &Value, raw: &Value) -> Value {
    let normalized_content = tura_llm_rust::normalize_response_content(content);
    if !tura_llm_rust::extract_tool_calls(&normalized_content).is_empty() {
        return content.clone();
    }

    let normalized_raw = tura_llm_rust::normalize_response_content(raw);
    if !tura_llm_rust::extract_tool_calls(&normalized_raw).is_empty() {
        return normalized_raw;
    }

    content.clone()
}

fn first_stream_output_or(
    first_stream_output_at: &Arc<Mutex<Option<DateTime<Utc>>>>,
    fallback: DateTime<Utc>,
) -> DateTime<Utc> {
    first_stream_output_at
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::response_content_for_tool_dispatch;
    use serde_json::json;

    #[test]
    fn tool_dispatch_content_recovers_command_run_from_raw_response_events() {
        let content = json!("I will inspect before editing.");
        let raw = json!({
            "object": "response",
            "output": [],
            "output_text": "I will inspect before editing.",
            "events": [
                {
                    "type": "response.output_item.done",
                    "item": {
                        "type": "message",
                        "id": "msg_1",
                        "content": [{
                            "type": "output_text",
                            "text": "I will inspect before editing."
                        }]
                    }
                },
                {
                    "type": "response.output_item.added",
                    "item": {
                        "type": "function_call",
                        "id": "fc_1",
                        "call_id": "call_1",
                        "name": "command_run",
                        "arguments": ""
                    }
                },
                {
                    "type": "response.function_call_arguments.done",
                    "item_id": "fc_1",
                    "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"pwd\"}]}"
                },
                {
                    "type": "response.output_item.done",
                    "item": {
                        "type": "function_call",
                        "id": "fc_1",
                        "call_id": "call_1",
                        "name": "command_run",
                        "status": "completed",
                        "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"pwd\"}]}"
                    }
                }
            ]
        });

        let dispatch_content = response_content_for_tool_dispatch(&content, &raw);
        let commands = dispatch_content["tool_calls"][0]["function"]["arguments"]["commands"]
            .as_array()
            .expect("command_run commands");

        assert_eq!(dispatch_content["text"], "I will inspect before editing.");
        assert_eq!(commands[0]["command_line"], "pwd");
    }
}
