use chrono::Utc;
use lifecycle::RuntimeProjection;
use std::time::Instant;
use tracing::warn;

use crate::manas::tool_catalog::env_flag;
use crate::profile_timings;
use crate::prompt_style::{runtime_fallback, tool_progress};
use crate::runtime::types::ToolCallData;
use crate::runtime_event_writer::RuntimeFeedPublisher;
use crate::tool_callback_sanitizer::sanitize_tool_callback_output;
use lifecycle::SessionManagement;
use lifecycle::{ContextTokenStats, RuntimeAggregate, UsageReport};
use session_log_contract::SessionFeedEvent;

use super::agent_message::{publish_agent_message, publish_feed_event};
use super::{runtime_message_id, runtime_tool_part_id};

pub(crate) fn summarize_single_tool_output(tool_name: &str, output: &serde_json::Value) -> String {
    if let Some(markdown) = first_summary_markdown(output) {
        return markdown.lines().take(10).collect::<Vec<_>>().join("\n");
    }
    if tool_name == "command_run" {
        if let Some(summary) = summarize_command_run_output(output) {
            return summary;
        }
    }

    if matches!(tool_name, "find" | "glob") {
        let mut matched_paths = Vec::new();
        if let Some(results) = output.get("results").and_then(|value| value.as_array()) {
            for result in results {
                if let Some(matches) = result.get("matches").and_then(|value| value.as_array()) {
                    for item in matches {
                        if let Some(path) = item.get("path").and_then(|value| value.as_str()) {
                            matched_paths.push(path.to_string());
                        }
                    }
                    continue;
                }
                if let Some(paths) = result
                    .get("matched_paths")
                    .and_then(|value| value.as_array())
                {
                    for path in paths {
                        if let Some(path) = path.as_str() {
                            matched_paths.push(path.to_string());
                        }
                    }
                }
            }
        }

        if !matched_paths.is_empty() {
            let preview = matched_paths
                .iter()
                .take(5)
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join(", ");
            return runtime_fallback::glob_match_summary(&preview, matched_paths.len());
        }
    }

    if let Some(raw_output) = output.get("raw_output").and_then(|value| value.as_str()) {
        let trimmed = raw_output.trim();
        if !trimmed.is_empty() {
            return trimmed.lines().take(6).collect::<Vec<_>>().join("\n");
        }
    }

    serde_json::to_string_pretty(output)
        .unwrap_or_else(|_| output.to_string())
        .lines()
        .take(8)
        .collect::<Vec<_>>()
        .join("\n")
}

fn summarize_command_run_output(output: &serde_json::Value) -> Option<String> {
    let results = output
        .get("results")
        .and_then(serde_json::Value::as_array)
        .filter(|results| !results.is_empty())?;
    let mut lines = Vec::new();
    for result in results.iter().take(5) {
        let command = result
            .get("command_type")
            .or_else(|| result.get("command"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("command");
        let status = if result.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
            "failed"
        } else {
            "ok"
        };
        let detail = result
            .get("output")
            .and_then(serde_json::Value::as_str)
            .map(compact_command_output)
            .or_else(|| {
                result
                    .get("output")
                    .and_then(|value| value.get("task_status"))
                    .and_then(|value| value.get("status"))
                    .and_then(serde_json::Value::as_str)
                    .map(|status| format!("task_status {status}"))
            })
            .filter(|value| !value.is_empty());
        lines.push(match detail {
            Some(detail) => format!("{command} {status}: {detail}"),
            None => format!("{command} {status}"),
        });
    }
    Some(lines.join("; "))
}

fn compact_command_output(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("Exit code:")
                && !line.starts_with("Wall time:")
                && *line != "Output:"
        })
        .take(3)
        .collect::<Vec<_>>()
        .join(" / ")
}

pub(crate) fn publish_step_summary(
    session: &SessionManagement,
    runtime: &RuntimeAggregate,
    tool_call: &ToolCallData,
    publisher: Option<&RuntimeFeedPublisher>,
) {
    let Some(step_summary) = extract_tool_argument_string(&tool_call.arguments, "step_summary")
    else {
        return;
    };

    let message = tool_progress::step_summary(&step_summary);
    if env_flag("TURA_CLI_PROGRESS") && !env_flag("TURA_CLI_LIVE_JSONL") {
        eprintln!("step: {}", step_summary.trim());
    }

    if let Err(error) = publish_agent_message(
        &session.session_id,
        &runtime.runtime_id,
        message,
        tool_progress::calling_tool(&tool_call.tool_name),
        publisher,
    ) {
        warn!(
            session_id = %session.session_id,
            tool_name = %tool_call.tool_name,
            error = %error,
            "failed to publish step summary"
        );
    }
}

pub(crate) fn publish_runtime_usage_record(
    session: &SessionManagement,
    runtime: &RuntimeAggregate,
    publisher: Option<&RuntimeFeedPublisher>,
) {
    let runtime_output = runtime
        .output
        .as_ref()
        .map(sanitize_tool_callback_output)
        .unwrap_or(serde_json::Value::Null);
    let metadata = serde_json::json!({
        "runtime_id": runtime.runtime_id,
        "session_id": session.session_id,
        "provider": runtime.provider,
        "usage": runtime.usage,
        "status": format!("{:?}", runtime.call_result_status()),
    });
    let state = serde_json::json!({
        "status": "completed",
        "input": runtime.input.clone().unwrap_or(serde_json::Value::Null),
        "output": runtime_output,
        "title": "Runtime usage",
        "metadata": metadata,
        "time": {
            "start": runtime.called_at.unwrap_or(runtime.created_at).timestamp_millis(),
            "end": runtime.call_finished_at
                .unwrap_or_else(|| runtime.called_at.unwrap_or(runtime.created_at))
                .timestamp_millis()
        }
    });
    let (created_at, updated_at) = runtime.assistant_message_timestamps();

    if let Err(error) = publish_gateway_tool_message(GatewayToolMessage {
        session_id: &session.session_id,
        runtime_id: &runtime.runtime_id,
        tool_name: "runtime",
        call_id: runtime.runtime_id.clone(),
        state,
        metadata,
        runtime_status: Some(runtime.lifecycle_projection()),
        context_tokens: Some(session.context_tokens),
        usage: runtime.usage.clone(),
        created_at,
        updated_at,
        publisher,
    }) {
        warn!(
            session_id = %session.session_id,
            runtime_id = %runtime.runtime_id,
            error = %error,
            "failed to publish runtime usage record"
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "gateway event payload assembly keeps call timing and result fields explicit"
)]
pub(crate) fn publish_tool_call_record(
    session: &SessionManagement,
    runtime: &RuntimeAggregate,
    tool_call: &ToolCallData,
    input: serde_json::Value,
    output: &serde_json::Value,
    success: bool,
    error: Option<&str>,
    started_at: chrono::DateTime<Utc>,
    publisher: Option<&RuntimeFeedPublisher>,
) {
    let total_start = Instant::now();
    let sanitized_output = sanitize_tool_callback_output(output);
    let profiling = profile_timings::enabled();
    let input_bytes = if profiling {
        profile_timings::json_bytes(&input)
    } else {
        0
    };
    let output_bytes = if profiling {
        profile_timings::json_bytes(&sanitized_output)
    } else {
        0
    };
    let ended_at = Utc::now();
    let (message_created_at, _) = runtime.assistant_message_timestamps();
    let call_id_start = Instant::now();
    let call_id = stable_tool_call_id(&runtime.runtime_id, &tool_call.tool_name, &input);
    profile_timings::log_elapsed(
        "publish_tool_call_record.stable_tool_call_id",
        call_id_start,
        serde_json::json!({
            "session_id": session.session_id,
            "runtime_id": runtime.runtime_id,
            "tool_name": tool_call.tool_name,
            "input_bytes": input_bytes,
        }),
    );
    let output_text_start = Instant::now();
    let output_text = tool_output_text(&sanitized_output, error);
    let output_text_len = output_text.len();
    profile_timings::log_elapsed(
        "publish_tool_call_record.tool_output_text",
        output_text_start,
        serde_json::json!({
            "session_id": session.session_id,
            "runtime_id": runtime.runtime_id,
            "tool_name": tool_call.tool_name,
            "output_bytes": output_bytes,
            "output_text_len": output_text_len,
        }),
    );
    let metadata_start = Instant::now();
    let metadata = serde_json::json!({
        "kind": "mano_tool_call",
        "tool": tool_call.tool_name,
        "success": success,
        "error": error,
        "summary": extract_tool_argument_string(&tool_call.arguments, "step_summary"),
        "runtime_id": runtime.runtime_id,
        "session_id": session.session_id,
        "provider": runtime.provider,
    });
    profile_timings::log_elapsed(
        "publish_tool_call_record.metadata",
        metadata_start,
        serde_json::json!({
            "session_id": session.session_id,
            "runtime_id": runtime.runtime_id,
            "tool_name": tool_call.tool_name,
            "metadata_bytes": if profiling {
                profile_timings::json_bytes(&metadata)
            } else {
                0
            },
        }),
    );
    let state_start = Instant::now();
    let state = if success {
        serde_json::json!({
            "status": "completed",
            "input": input,
            "output": output_text,
            "title": format!("Called `{}`", tool_call.tool_name),
            "metadata": metadata,
            "time": {
                "start": started_at.timestamp_millis(),
                "end": ended_at.timestamp_millis()
            }
        })
    } else {
        serde_json::json!({
            "status": "error",
            "input": input,
            "error": error.unwrap_or("Tool execution failed"),
            "metadata": metadata,
            "time": {
                "start": started_at.timestamp_millis(),
                "end": ended_at.timestamp_millis()
            }
        })
    };
    profile_timings::log_elapsed(
        "publish_tool_call_record.state",
        state_start,
        serde_json::json!({
            "session_id": session.session_id,
            "runtime_id": runtime.runtime_id,
            "tool_name": tool_call.tool_name,
            "state_bytes": if profiling {
                profile_timings::json_bytes(&state)
            } else {
                0
            },
        }),
    );

    let publish_start = Instant::now();
    let publish_result = publish_gateway_tool_message(GatewayToolMessage {
        session_id: &session.session_id,
        runtime_id: &runtime.runtime_id,
        tool_name: &tool_call.tool_name,
        call_id,
        state,
        metadata,
        runtime_status: None,
        context_tokens: Some(runtime.context_tokens),
        usage: runtime.usage.clone(),
        created_at: message_created_at,
        updated_at: ended_at.timestamp_millis(),
        publisher,
    });
    profile_timings::log_elapsed(
        "publish_tool_call_record.publish_gateway_tool_message",
        publish_start,
        serde_json::json!({
            "session_id": session.session_id,
            "runtime_id": runtime.runtime_id,
            "tool_name": tool_call.tool_name,
            "success": publish_result.is_ok(),
        }),
    );
    if let Err(error) = publish_result {
        warn!(
            session_id = %session.session_id,
            tool_name = %tool_call.tool_name,
            error = %error,
            "failed to publish tool call record"
        );
    }
    profile_timings::log_elapsed(
        "publish_tool_call_record.total",
        total_start,
        serde_json::json!({
            "session_id": session.session_id,
            "runtime_id": runtime.runtime_id,
            "tool_name": tool_call.tool_name,
            "input_bytes": input_bytes,
            "output_bytes": output_bytes,
            "output_text_len": output_text_len,
        }),
    );
}

pub(crate) fn publish_tool_call_started(
    session: &SessionManagement,
    runtime: &RuntimeAggregate,
    tool_call: &ToolCallData,
    input: serde_json::Value,
    started_at: chrono::DateTime<Utc>,
    publisher: Option<&RuntimeFeedPublisher>,
) {
    let (message_created_at, _) = runtime.assistant_message_timestamps();
    let call_id = stable_tool_call_id(&runtime.runtime_id, &tool_call.tool_name, &input);
    let metadata = serde_json::json!({
        "kind": "mano_tool_call",
        "tool": tool_call.tool_name,
        "success": null,
        "error": null,
        "summary": extract_tool_argument_string(&tool_call.arguments, "step_summary"),
        "runtime_id": runtime.runtime_id,
        "session_id": session.session_id,
        "provider": runtime.provider,
    });
    let state = serde_json::json!({
        "status": "running",
        "input": input,
        "title": format!("Calling `{}`", tool_call.tool_name),
        "metadata": metadata,
        "time": {
            "start": started_at.timestamp_millis()
        }
    });

    if let Err(error) = publish_gateway_tool_message(GatewayToolMessage {
        session_id: &session.session_id,
        runtime_id: &runtime.runtime_id,
        tool_name: &tool_call.tool_name,
        call_id,
        state,
        metadata,
        runtime_status: None,
        context_tokens: Some(runtime.context_tokens),
        usage: runtime.usage.clone(),
        created_at: message_created_at,
        updated_at: started_at.timestamp_millis(),
        publisher,
    }) {
        warn!(
            session_id = %session.session_id,
            tool_name = %tool_call.tool_name,
            error = %error,
            "failed to publish running tool call record"
        );
    }
}

fn stable_tool_call_id(runtime_id: &str, tool_name: &str, arguments: &serde_json::Value) -> String {
    format!(
        "{runtime_id}-{}",
        stable_tool_call_suffix(tool_name, arguments)
    )
}

fn stable_tool_call_suffix(tool_name: &str, arguments: &serde_json::Value) -> String {
    let source = format!("{tool_name}:{arguments}");
    let mut hash: u64 = 14_695_981_039_346_656_037;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    format!("{hash:016x}")
}

fn tool_output_text(output: &serde_json::Value, error: Option<&str>) -> String {
    if let Some(error) = error {
        return error.to_string();
    }
    if let Some(markdown) = first_summary_markdown(output) {
        return markdown;
    }
    serde_json::to_string_pretty(output).unwrap_or_else(|_| output.to_string())
}

fn first_summary_markdown(output: &serde_json::Value) -> Option<String> {
    output
        .get("results")
        .and_then(|value| value.as_array())?
        .iter()
        .filter_map(|result| {
            result
                .get("summary_markdown")
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn extract_tool_argument_string(arguments: &serde_json::Value, key: &str) -> Option<String> {
    let value = arguments
        .get(key)
        .or_else(|| arguments.get("requests")?.as_array()?.first()?.get(key))?;
    value
        .as_str()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

struct GatewayToolMessage<'a> {
    session_id: &'a str,
    runtime_id: &'a str,
    tool_name: &'a str,
    call_id: String,
    state: serde_json::Value,
    metadata: serde_json::Value,
    runtime_status: Option<RuntimeProjection>,
    context_tokens: Option<ContextTokenStats>,
    usage: Option<UsageReport>,
    created_at: i64,
    updated_at: i64,
    publisher: Option<&'a RuntimeFeedPublisher>,
}

fn publish_gateway_tool_message(message: GatewayToolMessage<'_>) -> Result<(), String> {
    let total_start = Instant::now();
    let profiling = profile_timings::enabled();
    let payload_start = Instant::now();
    let event = SessionFeedEvent::ToolCallUpdated {
        message_id: runtime_message_id(message.runtime_id),
        part_id: runtime_tool_part_id(message.runtime_id, message.tool_name),
        tool_name: message.tool_name.to_string(),
        call_id: message.call_id,
        state: message.state,
        metadata: Some(message.metadata),
        runtime_status: message.runtime_status,
        context_tokens: message.context_tokens,
        usage: message.usage,
        command_updates: Vec::new(),
        created_at: message.created_at,
        updated_at: message.updated_at,
    };
    let payload_bytes = if profiling {
        serde_json::to_value(&event)
            .map(|value| profile_timings::json_bytes(&value))
            .unwrap_or(0)
    } else {
        0
    };
    profile_timings::log_elapsed(
        "publish_gateway_tool_message.payload",
        payload_start,
        serde_json::json!({
            "target_session_id": message.session_id,
            "runtime_id": message.runtime_id,
            "tool_name": message.tool_name,
            "payload_bytes": payload_bytes,
        }),
    );

    let runtime_id = message.runtime_id.to_string();
    publish_feed_event(message.publisher, event)?;
    profile_timings::log_elapsed(
        "publish_gateway_tool_message.total",
        total_start,
        serde_json::json!({
            "runtime_id": runtime_id,
            "tool_name": message.tool_name,
            "payload_bytes": payload_bytes,
        }),
    );
    Ok(())
}

pub(crate) fn publish_task_plan_todos(
    session: &SessionManagement,
    publisher: Option<&RuntimeFeedPublisher>,
) {
    if session.task_plan.detailed_tasks.is_empty() {
        return;
    }

    let todos = session
        .task_plan
        .detailed_tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let status = match task.status {
                lifecycle::TaskStatus::Todo => "todo",
                lifecycle::TaskStatus::WaitingUser => "waiting_user",
                lifecycle::TaskStatus::Doing => "doing",
                lifecycle::TaskStatus::Question => "question",
                lifecycle::TaskStatus::Done => "done",
                lifecycle::TaskStatus::Archived => "archived",
            };
            let content = first_non_empty([
                task.task_summary.as_str(),
                task.step_task.as_str(),
                task.step_deliverable_description.as_str(),
            ])
            .unwrap_or_else(|| format!("Step {}", index + 1));
            serde_json::json!({
                "id": format!("task-plan-{}", index + 1),
                "content": content,
                "status": status,
                "priority": "medium",
            })
        })
        .collect::<Vec<_>>();

    if let Err(error) = publish_feed_event(
        publisher,
        SessionFeedEvent::TodosUpdated {
            todos,
            updated_at: Utc::now().timestamp_millis(),
        },
    ) {
        warn!(session_id = %session.session_id, error = %error, "failed to publish task plan todos");
    }
}

fn first_non_empty<const N: usize>(items: [&str; N]) -> Option<String> {
    items
        .into_iter()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::stable_tool_call_id;

    #[test]
    fn stable_tool_call_id_uses_normalized_execution_arguments() {
        let raw_arguments = serde_json::json!({
            "step_summary": "inspect files",
            "requests": [{ "command": "pwd" }]
        });
        let normalized_arguments = serde_json::json!({
            "commands": [{ "command": "shell_command", "command_line": "Get-Location" }]
        });

        let from_normalized =
            stable_tool_call_id("runtime-1", "command_run", &normalized_arguments);
        let from_raw = stable_tool_call_id("runtime-1", "command_run", &raw_arguments);

        assert_ne!(from_normalized, from_raw);
        assert_eq!(
            from_normalized,
            stable_tool_call_id("runtime-1", "command_run", &normalized_arguments)
        );
    }
}
