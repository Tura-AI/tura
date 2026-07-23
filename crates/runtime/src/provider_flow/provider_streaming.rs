use chrono::{DateTime, Utc};
use lifecycle::RuntimeState;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tracing::error;

use crate::gateway_events::{frontend_session_id, publish_streamed_agent_text};
use crate::provider_flow::call::flush_runtime_events;
use crate::provider_flow::command_run_streaming::{
    apply_cancelled_streamed_command_run_result, apply_patch_failed_streamed_command_run_result,
    apply_startup_apply_patch_discarded_streamed_command_run_result,
    ensure_streamed_command_run_tool_record, spawn_streamed_command_run_task,
    SpawnStreamedCommandRunTask, StreamedCommandRunState,
};
use crate::provider_flow::errors::{
    finish_provider_call_failure, finish_runtime_failure, runtime_timeout,
};
use crate::provider_flow::provider_response::apply_provider_response_with_options;
use crate::provider_flow::streamed_command_run::{
    command_run_stream_events_from_provider_content, streamed_command_run_call_id,
};
use crate::provider_flow::usage::usage_report_from_metrics;
use crate::runtime_event_writer::RuntimeEventWriter;
use lifecycle::RuntimeAggregate;

pub(crate) struct RuntimeStreamingInput {
    pub(crate) messages: Vec<serde_json::Value>,
    pub(crate) options: tura_llm_rust::CallOptions,
    pub(crate) session_directory: PathBuf,
    pub(crate) allowed_command_run_commands: Option<BTreeSet<String>>,
    pub(crate) require_startup_task_state: bool,
}

pub(crate) async fn call_runtime_streaming(
    runtime: &mut RuntimeAggregate,
    route_config: &tura_llm_rust::RouteConfig,
    tura_config: &Arc<tura_llm_rust::TuraConfig>,
    input: RuntimeStreamingInput,
    mut runtime_event_writer: Option<&mut RuntimeEventWriter>,
) -> Result<(), String> {
    let started_at = Utc::now();
    let timeout_duration = runtime_timeout(runtime);
    let feed_publisher = runtime_event_writer
        .as_deref_mut()
        .map(|writer| {
            writer.feed_publisher(
                &runtime.runtime_id,
                &frontend_session_id(&runtime.session_id),
            )
        })
        .transpose()?;
    let (stream_tx, stream_rx) = mpsc::channel::<tura_llm_rust::ProviderStreamEvent>();
    let final_response_stream_tx = stream_tx.clone();
    let text_delta_runtime = runtime.clone();
    let text_feed_publisher = feed_publisher.clone();

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
            if let Err(error) =
                publish_streamed_agent_text(&text_delta_runtime, text, text_feed_publisher.as_ref())
            {
                tracing::warn!(error = %error, "failed to queue assistant text feed event");
            }
        }
        let _ = stream_tx.send(event);
    });

    let gateway_session_id = runtime.session_id.clone();
    let gateway_runtime_id = runtime.runtime_id.clone();
    let gateway_provider = serde_json::to_value(&runtime.provider).unwrap_or(Value::Null);
    let gateway_call_id = streamed_command_run_call_id(&gateway_runtime_id);
    let command_task = spawn_streamed_command_run_task(SpawnStreamedCommandRunTask {
        stream_rx,
        session_directory: input.session_directory,
        allowed_command_run_commands: input.allowed_command_run_commands,
        session_id: gateway_session_id,
        runtime_id: gateway_runtime_id.clone(),
        provider: gateway_provider,
        call_id: gateway_call_id,
        started_at,
        state: command_state.clone(),
        runtime_status: runtime.lifecycle_projection(),
        feed_publisher,
        require_startup_task_state: input.require_startup_task_state,
    });

    let route_config_for_task = route_config.clone();
    let tura_config_for_task = Arc::clone(tura_config);
    let provider_task = tokio::spawn(async move {
        route_config_for_task
            .run_with_stream_events(
                tura_config_for_task.as_ref(),
                input.messages,
                input.options,
                Some(sink),
            )
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
                }))?;
                flush_runtime_events(&mut runtime_event_writer, runtime)?;
                finish_runtime_failure(
                    runtime,
                    finished_at,
                    "CALL_TIMED_OUT",
                    message,
                    RuntimeState::TimedOut,
                )?;
                flush_runtime_events(&mut runtime_event_writer, runtime)?;
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
                        }))?;
                        flush_runtime_events(&mut runtime_event_writer, runtime)?;
                        finish_provider_call_failure(runtime, finished_at, &e, RuntimeState::Failed)?;
                        flush_runtime_events(&mut runtime_event_writer, runtime)?;
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
                        }))?;
                        flush_runtime_events(&mut runtime_event_writer, runtime)?;
                        finish_runtime_failure(
                            runtime,
                            finished_at,
                            "CALL_FAILED",
                            message,
                            RuntimeState::Failed,
                        )?;
                        flush_runtime_events(&mut runtime_event_writer, runtime)?;
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
                    )?;
                    let first_token_at = first_stream_output_or(&first_stream_output_at, finished_at);
                    runtime
                        .mark_first_token(first_token_at)
                        .map_err(|e| format!("failed to mark first token: {e}"))?;
                    flush_runtime_events(&mut runtime_event_writer, runtime)?;
                    finish_runtime_failure(
                        runtime,
                        finished_at,
                        "COMMAND_RUN_CANCELLED",
                        "streamed command_run cancelled after an infrastructure failure".to_string(),
                        RuntimeState::Cancelled,
                    )?;
                    flush_runtime_events(&mut runtime_event_writer, runtime)?;
                    provider_task.abort();
                    let _ = (&mut provider_task).await;
                    drop(final_response_stream_tx);
                    let _ = command_task.join();
                    return Ok(());
                }
                if command_state.should_finish_after_apply_patch_failure() {
                    let finished_at = Utc::now();
                    let snapshot = command_state.snapshot();
                    apply_patch_failed_streamed_command_run_result(
                        runtime,
                        &snapshot.commands,
                        &snapshot.events,
                        &snapshot.results,
                        finished_at,
                    )?;
                    let first_token_at = first_stream_output_or(&first_stream_output_at, finished_at);
                    runtime
                        .mark_first_token(first_token_at)
                        .map_err(|e| format!("failed to mark first token: {e}"))?;
                    flush_runtime_events(&mut runtime_event_writer, runtime)?;
                    runtime
                        .finish_success(finished_at, None)
                        .map_err(|e| format!("failed to finish runtime success: {e}"))?;
                    flush_runtime_events(&mut runtime_event_writer, runtime)?;
                    provider_task.abort();
                    let _ = (&mut provider_task).await;
                    drop(final_response_stream_tx);
                    let _ = command_task.join();
                    return Ok(());
                }
                if command_state.should_finish_startup_apply_patch_discard() {
                    let finished_at = Utc::now();
                    let snapshot = command_state.snapshot();
                    apply_startup_apply_patch_discarded_streamed_command_run_result(
                        runtime,
                        &snapshot.commands,
                        &snapshot.events,
                        &snapshot.results,
                        finished_at,
                    )?;
                    let first_token_at = first_stream_output_or(&first_stream_output_at, finished_at);
                    runtime
                        .mark_first_token(first_token_at)
                        .map_err(|e| format!("failed to mark first token: {e}"))?;
                    flush_runtime_events(&mut runtime_event_writer, runtime)?;
                    runtime
                        .finish_success(finished_at, None)
                        .map_err(|e| format!("failed to finish runtime success: {e}"))?;
                    flush_runtime_events(&mut runtime_event_writer, runtime)?;
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

    if command_state.was_cancelled() {
        apply_cancelled_streamed_command_run_result(
            runtime,
            &snapshot.commands,
            &snapshot.events,
            &streamed_command_results,
            finished_at,
        )?;
        let first_token_at = first_stream_output_or(&first_stream_output_at, finished_at);
        runtime
            .mark_first_token(first_token_at)
            .map_err(|e| format!("failed to mark first token: {e}"))?;
        flush_runtime_events(&mut runtime_event_writer, runtime)?;
        finish_runtime_failure(
            runtime,
            finished_at,
            "COMMAND_RUN_CANCELLED",
            "streamed command_run cancelled after an infrastructure failure".to_string(),
            RuntimeState::Cancelled,
        )?;
        flush_runtime_events(&mut runtime_event_writer, runtime)?;
        return Ok(());
    }

    let apply_patch_failed = command_state.apply_patch_failed();
    let startup_apply_patch_discarded = command_state.startup_apply_patch_discarded();
    let has_streamed_command_run_result =
        startup_apply_patch_discarded || !streamed_command_results.is_empty();
    let mut runtime_output = response.content.clone();
    if has_streamed_command_run_result {
        runtime_output = serde_json::json!({
            "streamed_command_run_result": {
                "commands": snapshot.commands,
                "command_events": snapshot.events,
                "results": streamed_command_results,
            }
        });
        if apply_patch_failed {
            runtime_output["streamed_command_run_result"]["early_finish_reason"] =
                Value::String("apply_patch_failed".to_string());
        }
        if !startup_apply_patch_discarded {
            runtime_output["provider_content"] = tool_dispatch_content.clone();
        }
    }
    runtime.set_output(runtime_output)?;
    apply_provider_response_with_options(
        runtime,
        &tool_dispatch_content,
        finished_at,
        has_streamed_command_run_result,
    )?;
    if has_streamed_command_run_result {
        ensure_streamed_command_run_tool_record(runtime, &snapshot.commands, finished_at)?;
    }

    if let Some(stream) = response.content.get("stream").and_then(|s| s.as_array()) {
        for chunk in stream {
            if let Some(text) = chunk.get("text").and_then(|t| t.as_str()) {
                runtime.append_text(text)?;
            }
        }
    }

    let first_token_at = first_stream_output_or(&first_stream_output_at, finished_at);

    runtime
        .mark_first_token(first_token_at)
        .map_err(|e| format!("failed to mark first token: {e}"))?;

    let usage =
        usage_report_from_metrics(response.metrics, started_at, finished_at, first_token_at);

    flush_runtime_events(&mut runtime_event_writer, runtime)?;
    runtime
        .finish_success(finished_at, usage)
        .map_err(|e| format!("failed to finish runtime success: {e}"))?;
    flush_runtime_events(&mut runtime_event_writer, runtime)?;

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
