//! Streamed command_run handling for provider responses.

use chrono::{DateTime, Utc};
use lifecycle::RuntimeProjection;
use serde_json::Value;
use session_log_contract::{SessionFeedCommandUpdate, SessionFeedEvent};

use crate::gateway_events::{publish_feed_event, runtime_message_id, runtime_tool_part_id};
use crate::runtime_event_writer::RuntimeFeedPublisher;
use crate::tool_callback_sanitizer::{
    sanitize_tool_callback_output, sanitize_tool_callback_result,
};

const COMMAND_RUN_TOOL_NAME: &str = "command_run";

pub fn command_run_stream_events_from_provider_content(
    content: &Value,
) -> Vec<tura_llm_rust::ProviderStreamEvent> {
    tura_llm_rust::extract_tool_calls(content)
        .into_iter()
        .enumerate()
        .filter(|(_, tool_call)| tool_call.tool_name == COMMAND_RUN_TOOL_NAME)
        .flat_map(|(tool_index, tool_call)| {
            let tool_call_id = tool_call
                .provider_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("id"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("call_command_run_{tool_index}"));
            tool_call
                .arguments
                .get("commands")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(move |(command_index, command)| {
                    tura_llm_rust::ProviderStreamEvent::CommandRunCommandReady {
                        tool_call_id: tool_call_id.clone(),
                        command_index,
                        command,
                    }
                })
        })
        .collect()
}

pub struct StreamedCommandEvent {
    pub tool_call_id: String,
    pub command_index: usize,
    pub command: Value,
}

pub fn command_run_stream_event_command(
    event: tura_llm_rust::ProviderStreamEvent,
) -> Option<StreamedCommandEvent> {
    match event {
        tura_llm_rust::ProviderStreamEvent::CommandRunCommandReady {
            tool_call_id,
            command_index,
            command,
        } => Some(StreamedCommandEvent {
            tool_call_id,
            command_index,
            command,
        }),
        tura_llm_rust::ProviderStreamEvent::ProviderOutputStarted
        | tura_llm_rust::ProviderStreamEvent::TextDelta { .. } => None,
    }
}

pub fn streamed_command_event_record(
    status: &str,
    runtime_id: &str,
    tool_call_id: &str,
    command_index: usize,
    command: &Value,
    result: Option<&Value>,
    timestamp: DateTime<Utc>,
) -> Value {
    serde_json::json!({
        "status": status,
        "runtime_id": runtime_id,
        "command_id": command.get("command_id").cloned().unwrap_or(Value::Null),
        "command_run_id": command.get("command_run_id").cloned().unwrap_or(Value::Null),
        "provider_tool_call_id": tool_call_id,
        "command_index": command_index,
        "step": command.get("step").cloned().unwrap_or(Value::Null),
        "command_type": command.get("command_type").cloned().unwrap_or(Value::Null),
        "command_line": command.get("command_line").cloned().unwrap_or(Value::Null),
        "command": command,
        "result": result
            .map(sanitize_tool_callback_result)
            .unwrap_or(Value::Null),
        "timestamp": timestamp.to_rfc3339(),
    })
}

pub fn streamed_command_result_record(
    status: &str,
    runtime_id: &str,
    result_index: usize,
    result: &Value,
    timestamp: DateTime<Utc>,
) -> Value {
    let result = sanitize_tool_callback_result(result);
    serde_json::json!({
        "status": status,
        "runtime_id": runtime_id,
        "command_id": result.get("command_id").cloned().unwrap_or(Value::Null),
        "command_run_id": result.get("command_run_id").cloned().unwrap_or(Value::Null),
        "provider_tool_call_id": result.get("provider_tool_call_id").cloned().unwrap_or(Value::Null),
        "command_index": result.get("command_index").cloned().unwrap_or(Value::Null),
        "result_index": result_index,
        "step": result.get("step").cloned().unwrap_or(Value::Null),
        "command_type": result
            .get("command_type")
            .or_else(|| result.get("command"))
            .cloned()
            .unwrap_or(Value::Null),
        "success": result.get("success").cloned().unwrap_or(Value::Null),
        "result": result,
        "timestamp": timestamp.to_rfc3339(),
    })
}

pub fn streamed_command_run_call_id(runtime_id: &str) -> String {
    runtime_tool_part_id(runtime_id, COMMAND_RUN_TOOL_NAME)
}

pub fn command_run_live_delta_result(
    command: &Value,
    stdout: &str,
    stderr: &str,
    started_at: DateTime<Utc>,
) -> Value {
    let command_type = command
        .get("command_type")
        .and_then(Value::as_str)
        .unwrap_or(COMMAND_RUN_TOOL_NAME);
    let command_line = command
        .get("command_line")
        .and_then(Value::as_str)
        .unwrap_or("");
    let step = command
        .get("step")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let output = sanitize_tool_callback_output(&serde_json::json!({
        "stdout": stdout,
        "stderr": stderr,
    }));
    serde_json::json!({
        "command_id": command.get("command_id").cloned().unwrap_or(Value::Null),
        "command_run_id": command.get("command_run_id").cloned().unwrap_or(Value::Null),
        "provider_tool_call_id": command.get("provider_tool_call_id").cloned().unwrap_or(Value::Null),
        "command_index": command.get("command_index").cloned().unwrap_or(Value::Null),
        "step": step,
        "command_type": command_type,
        "command_line": command_line,
        "status": "running",
        "success": null,
        "started_at": started_at.timestamp_millis(),
        "command": command,
        "output": output,
    })
}

pub(crate) struct StreamedCommandRunUpdate<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) runtime_id: &'a str,
    pub(crate) provider: &'a Value,
    pub(crate) call_id: &'a str,
    pub(crate) commands: &'a [Value],
    pub(crate) results: &'a [Value],
    pub(crate) status: &'a str,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) ended_at: Option<DateTime<Utc>>,
    pub(crate) runtime_status: RuntimeProjection,
    pub(crate) publisher: Option<&'a RuntimeFeedPublisher>,
}

pub(crate) fn publish_streamed_command_run_update(
    update: StreamedCommandRunUpdate<'_>,
) -> Result<(), String> {
    let updated_at = update
        .ended_at
        .unwrap_or(update.started_at)
        .timestamp_millis();
    let created_at = update.started_at.timestamp_millis();
    let sanitized_results = update
        .results
        .iter()
        .map(sanitize_tool_callback_result)
        .collect::<Vec<_>>();
    let command_updates = command_update_payloads(
        update.runtime_id,
        update.call_id,
        update.commands,
        &sanitized_results,
        update.status,
        created_at,
        updated_at,
    );
    let input = serde_json::json!({ "commands": update.commands });
    let output = serde_json::json!({
        "streamed_command_run_result": {
            "results": sanitized_results,
        }
    });
    let success = match update.status {
        "completed" => Value::Bool(true),
        "error" => Value::Bool(false),
        _ => Value::Null,
    };
    let error_value = if update.status == "error" {
        Value::String("command_run stream halted".to_string())
    } else {
        Value::Null
    };
    let metadata = serde_json::json!({
        "kind": "mano_tool_call",
        "tool": COMMAND_RUN_TOOL_NAME,
        "success": success,
        "error": error_value,
        "runtime_id": update.runtime_id,
        "session_id": update.session_id,
        "provider": update.provider,
        "runtime_status": &update.runtime_status,
        "transient": true,
        "streaming_partial": update.status != "completed" && update.status != "error",
    });
    let mut time = serde_json::Map::new();
    time.insert(
        "start".to_string(),
        Value::Number(update.started_at.timestamp_millis().into()),
    );
    if let Some(ended_at) = update.ended_at {
        time.insert(
            "end".to_string(),
            Value::Number(ended_at.timestamp_millis().into()),
        );
    }
    let state = serde_json::json!({
        "status": update.status,
        "input": input,
        "output": output,
        "title": if update.status == "completed" {
            "Called `command_run`"
        } else {
            "Calling `command_run`"
        },
        "metadata": metadata,
        "time": time,
    });
    publish_feed_event(
        update.publisher,
        SessionFeedEvent::ToolCallUpdated {
            message_id: runtime_message_id(update.runtime_id),
            part_id: runtime_tool_part_id(update.runtime_id, COMMAND_RUN_TOOL_NAME),
            tool_name: COMMAND_RUN_TOOL_NAME.to_string(),
            call_id: update.call_id.to_string(),
            state,
            metadata: Some(metadata),
            runtime_status: Some(update.runtime_status),
            context_tokens: None,
            usage: None,
            command_updates,
            created_at,
            updated_at,
        },
    )
}

fn command_update_payloads(
    runtime_id: &str,
    command_run_id: &str,
    commands: &[Value],
    results: &[Value],
    status: &str,
    created_at: i64,
    updated_at: i64,
) -> Vec<SessionFeedCommandUpdate> {
    let mut updates = Vec::new();
    for command in commands {
        let command_id =
            command_identity(command, command_run_id).unwrap_or_else(|| command_run_id.to_string());
        updates.push(SessionFeedCommandUpdate {
            message_id: runtime_message_id(runtime_id),
            part_id: runtime_tool_part_id(runtime_id, COMMAND_RUN_TOOL_NAME),
            command_run_id: command_run_id.to_string(),
            command_id,
            provider_tool_call_id: command
                .get("provider_tool_call_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            command_index: command.get("command_index").and_then(Value::as_u64),
            event_seq: Some(command_event_seq(command, 20)),
            status: if status == "completed" || status == "error" {
                status.to_string()
            } else {
                "ready".to_string()
            },
            command: command.clone(),
            result: Value::Null,
            created_at,
            updated_at,
        });
    }
    for result in results {
        let command = result.get("command").unwrap_or(&Value::Null);
        let command_id = command_identity(result, command_run_id)
            .or_else(|| command_identity(command, command_run_id))
            .unwrap_or_else(|| command_run_id.to_string());
        let result_status = command_result_status(result, status);
        updates.push(SessionFeedCommandUpdate {
            message_id: runtime_message_id(runtime_id),
            part_id: runtime_tool_part_id(runtime_id, COMMAND_RUN_TOOL_NAME),
            command_run_id: command_run_id.to_string(),
            command_id,
            provider_tool_call_id: result
                .get("provider_tool_call_id")
                .or_else(|| command.get("provider_tool_call_id"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            command_index: result
                .get("command_index")
                .or_else(|| command.get("command_index"))
                .and_then(Value::as_u64),
            event_seq: Some(command_event_seq(result, status_event_rank(&result_status))),
            status: result_status,
            command: if command.is_null() {
                Value::Null
            } else {
                command.clone()
            },
            result: result.clone(),
            created_at,
            updated_at,
        });
    }
    updates
}

fn command_identity(value: &Value, fallback_run_id: &str) -> Option<String> {
    value
        .get("command_id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            let provider = value.get("provider_tool_call_id").and_then(Value::as_str)?;
            let index = value.get("command_index").and_then(Value::as_u64)?;
            Some(format!("{fallback_run_id}:{provider}:{index}"))
        })
}

fn command_event_seq(value: &Value, rank: i64) -> i64 {
    value
        .get("command_index")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .saturating_mul(100)
        .saturating_add(rank)
}

fn command_result_status(result: &Value, fallback: &str) -> String {
    if result.get("success").and_then(Value::as_bool) == Some(false) {
        return "failed".to_string();
    }
    if let Some(status) = result.get("status").and_then(Value::as_str) {
        return if status == "in_progress" {
            "running".to_string()
        } else {
            status.to_string()
        };
    }
    if result.get("success").and_then(Value::as_bool) == Some(true) {
        return "completed".to_string();
    }
    if fallback == "completed" {
        "completed".to_string()
    } else {
        "running".to_string()
    }
}

fn status_event_rank(status: &str) -> i64 {
    match status {
        "failed" | "error" => 50,
        "completed" => 40,
        "running" => 30,
        "ready" => 20,
        _ => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        command_run_live_delta_result, command_run_stream_event_command,
        command_run_stream_events_from_provider_content, streamed_command_event_record,
        streamed_command_result_record,
    };
    use chrono::Utc;

    #[test]
    fn final_response_command_run_events_extract_for_replay() {
        let content = serde_json::json!({
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "command_run",
                    "arguments": {
                        "commands": [{
                            "command_type": "apply_patch",
                            "command_line": "*** Begin Patch\n*** Add File: probe.txt\n+ok\n*** End Patch"
                        }]
                    }
                }
            }]
        });

        let events = command_run_stream_events_from_provider_content(&content);

        assert_eq!(events.len(), 1);
        let event = command_run_stream_event_command(events[0].clone())
            .expect("command_run event should contain a command");
        assert_eq!(event.tool_call_id, "call_1");
        assert_eq!(event.command["command_type"], "apply_patch");
    }

    #[test]
    fn command_event_records_do_not_use_command_text_as_command_type() {
        let command = serde_json::json!({
            "command": "large file scan",
            "step": 1
        });

        let started_at = Utc::now();
        let record = streamed_command_event_record(
            "ready",
            "runtime-1",
            "call-1",
            0,
            &command,
            None,
            started_at,
        );
        let live = command_run_live_delta_result(&command, "", "", started_at);

        assert_eq!(record["command_type"], serde_json::Value::Null);
        assert_eq!(record["command_line"], serde_json::Value::Null);
        assert_eq!(live["command_type"], "command_run");
        assert_eq!(live["command_line"], "");
        assert_eq!(live["started_at"], started_at.timestamp_millis());
    }

    #[test]
    fn streamed_command_records_truncate_large_result_output() {
        let large_output = "match line\n".repeat(2_000);
        let result = serde_json::json!({
            "command_id": "cmd-1",
            "command_run_id": "run-1",
            "provider_tool_call_id": "call-1",
            "command_index": 0,
            "step": 1,
            "command_type": "shell_command",
            "success": true,
            "output": large_output,
        });

        let event = streamed_command_event_record(
            "completed",
            "runtime-1",
            "call-1",
            0,
            &serde_json::json!({"command_type": "shell_command"}),
            Some(&result),
            Utc::now(),
        );
        let record =
            streamed_command_result_record("completed", "runtime-1", 0, &result, Utc::now());

        for output in [
            event["result"]["output"].as_str().expect("event output"),
            record["result"]["output"].as_str().expect("record output"),
        ] {
            assert!(output.contains("characters truncated"), "{output}");
            assert!(output.len() < large_output.len(), "{output}");
        }
    }
}
