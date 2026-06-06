use chrono::Utc;
use tracing::warn;

use crate::manas::constants::DISABLE_GATEWAY_CALLBACKS_ENV;
use crate::manas::tool_catalog::env_flag;
use crate::prompt_style::{runtime_fallback, tool_progress};
use crate::runtime::types::ToolCallData;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;

use super::agent_message::{
    gateway_callback_base_url, gateway_callback_session_id, publish_gateway_agent_message,
};

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
    runtime: &RuntimeManagement,
    tool_call: &ToolCallData,
) {
    let Some(step_summary) = extract_tool_argument_string(&tool_call.arguments, "step_summary")
    else {
        return;
    };

    let message = tool_progress::step_summary(&step_summary);
    if env_flag("TURA_CLI_PROGRESS") && !env_flag("TURA_CLI_LIVE_JSONL") {
        eprintln!("step: {}", step_summary.trim());
    }

    if let Err(error) = publish_gateway_agent_message(
        &session.session_id,
        &runtime.runtime_id,
        message,
        tool_progress::calling_tool(&tool_call.tool_name),
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
    runtime: &RuntimeManagement,
) {
    let target_session_id = gateway_callback_session_id(&session.session_id);
    let metadata = serde_json::json!({
        "kind": "mano_runtime_usage",
        "runtime_id": runtime.runtime_id,
        "session_id": session.session_id,
        "target_session_id": target_session_id,
        "provider": runtime.provider,
        "usage": runtime.usage,
        "status": format!("{:?}", runtime.call_result_status),
        "input": runtime.input.clone(),
        "output": runtime.output.clone(),
    });
    let state = serde_json::json!({
        "status": "completed",
        "input": runtime.input.clone().unwrap_or(serde_json::Value::Null),
        "output": runtime.output.clone().unwrap_or(serde_json::Value::Null),
        "title": "Runtime usage",
        "metadata": metadata,
        "time": {
            "start": runtime.called_at.unwrap_or(runtime.created_at).timestamp_millis(),
            "end": runtime.call_finished_at.unwrap_or_else(Utc::now).timestamp_millis()
        }
    });

    if let Err(error) = publish_gateway_tool_message(
        &target_session_id,
        &runtime.runtime_id,
        "runtime",
        runtime.runtime_id.clone(),
        state,
        metadata,
    ) {
        warn!(
            session_id = %session.session_id,
            target_session_id = %target_session_id,
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
    runtime: &RuntimeManagement,
    tool_call: &ToolCallData,
    input: serde_json::Value,
    output: &serde_json::Value,
    success: bool,
    error: Option<&str>,
    started_at: chrono::DateTime<Utc>,
) {
    let ended_at = Utc::now();
    let call_id = stable_tool_call_id(&runtime.runtime_id, &tool_call.tool_name, &input);
    let output_text = tool_output_text(output, error);
    let metadata = serde_json::json!({
        "kind": "mano_tool_call",
        "tool": tool_call.tool_name,
        "input": input,
        "output": output,
        "success": success,
        "error": error,
        "summary": extract_tool_argument_string(&tool_call.arguments, "step_summary"),
        "runtime_id": runtime.runtime_id,
        "session_id": session.session_id,
        "provider": runtime.provider,
    });
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

    if let Err(error) = publish_gateway_tool_message(
        &gateway_callback_session_id(&session.session_id),
        &runtime.runtime_id,
        &tool_call.tool_name,
        call_id,
        state,
        metadata,
    ) {
        warn!(
            session_id = %session.session_id,
            tool_name = %tool_call.tool_name,
            error = %error,
            "failed to publish tool call record"
        );
    }
}

pub(crate) fn publish_tool_call_started(
    session: &SessionManagement,
    runtime: &RuntimeManagement,
    tool_call: &ToolCallData,
    input: serde_json::Value,
    started_at: chrono::DateTime<Utc>,
) {
    let call_id = stable_tool_call_id(&runtime.runtime_id, &tool_call.tool_name, &input);
    let metadata = serde_json::json!({
        "kind": "mano_tool_call",
        "tool": tool_call.tool_name,
        "input": input,
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

    if let Err(error) = publish_gateway_tool_message(
        &gateway_callback_session_id(&session.session_id),
        &runtime.runtime_id,
        &tool_call.tool_name,
        call_id,
        state,
        metadata,
    ) {
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

fn publish_gateway_tool_message(
    session_id: &str,
    runtime_id: &str,
    tool_name: &str,
    call_id: String,
    state: serde_json::Value,
    metadata: serde_json::Value,
) -> Result<(), String> {
    if env_flag(DISABLE_GATEWAY_CALLBACKS_ENV) {
        return Ok(());
    }

    let target_session_id = gateway_callback_session_id(session_id);
    let gateway_base = gateway_callback_base_url();
    let endpoint = format!("{gateway_base}/session/{target_session_id}/message/agent");
    let payload = serde_json::json!({
        "reply_message": "",
        "new_learning": "",
        "media": [],
        "runtime_id": runtime_id,
        "tool_call": {
            "tool_name": tool_name,
            "call_id": call_id,
            "state": state,
            "metadata": metadata,
        }
    });

    tokio::runtime::Runtime::new()
        .map_err(|err| format!("failed to create gateway callback runtime: {err}"))?
        .block_on(async {
            let response = reqwest::Client::new()
                .post(endpoint)
                .json(&payload)
                .send()
                .await
                .map_err(|err| format!("failed to call gateway: {err}"))?;
            if response.status().is_success() {
                Ok(())
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Err(format!("gateway returned {status}: {body}"))
            }
        })
}

pub(crate) fn publish_task_plan_todos(session: &SessionManagement) {
    if env_flag(DISABLE_GATEWAY_CALLBACKS_ENV) || session.task_plan.detailed_tasks.is_empty() {
        return;
    }

    let todos = session
        .task_plan
        .detailed_tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let status = match task.status {
                crate::state_machine::session_management::TaskStatus::Todo => "todo",
                crate::state_machine::session_management::TaskStatus::WaitingUser => "waiting_user",
                crate::state_machine::session_management::TaskStatus::Doing => "doing",
                crate::state_machine::session_management::TaskStatus::Question => "question",
                crate::state_machine::session_management::TaskStatus::Done => "done",
                crate::state_machine::session_management::TaskStatus::Archived => "archived",
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

    let target_session_id = gateway_callback_session_id(&session.session_id);
    let endpoint = format!(
        "{}/session/{target_session_id}/todo",
        gateway_callback_base_url()
    );
    let _ = tokio::runtime::Runtime::new()
        .map_err(|_| ())
        .and_then(|runtime| {
            runtime.block_on(async {
                reqwest::Client::new()
                    .post(endpoint)
                    .json(&todos)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|_| ())
            })
        });
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
