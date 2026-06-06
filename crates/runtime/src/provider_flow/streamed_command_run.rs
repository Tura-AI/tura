//! Streamed command_run handling for provider responses.

use chrono::{DateTime, Utc};
use serde_json::Value;
use tracing::warn;

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

pub fn should_replay_final_response_command_run(streamed_command_seen: bool) -> bool {
    !streamed_command_seen
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
        tura_llm_rust::ProviderStreamEvent::ProviderOutputStarted => None,
    }
}

pub fn streamed_command_event_record(
    status: &str,
    runtime_id: &str,
    tool_call_id: &str,
    command_index: usize,
    command: &Value,
    result: Option<&Value>,
) -> Value {
    serde_json::json!({
        "status": status,
        "runtime_id": runtime_id,
        "provider_tool_call_id": tool_call_id,
        "command_index": command_index,
        "step": command.get("step").cloned().unwrap_or(Value::Null),
        "command_type": command
            .get("command_type")
            .or_else(|| command.get("command"))
            .cloned()
            .unwrap_or(Value::Null),
        "command_line": command.get("command_line").cloned().unwrap_or(Value::Null),
        "command": command,
        "result": result.cloned().unwrap_or(Value::Null),
        "timestamp": Utc::now().to_rfc3339(),
    })
}

pub fn streamed_command_result_record(
    status: &str,
    runtime_id: &str,
    result_index: usize,
    result: &Value,
) -> Value {
    serde_json::json!({
        "status": status,
        "runtime_id": runtime_id,
        "result_index": result_index,
        "step": result.get("step").cloned().unwrap_or(Value::Null),
        "command_type": result
            .get("command_type")
            .or_else(|| result.get("command"))
            .cloned()
            .unwrap_or(Value::Null),
        "success": result.get("success").cloned().unwrap_or(Value::Null),
        "result": result,
        "timestamp": Utc::now().to_rfc3339(),
    })
}

pub fn streamed_command_run_call_id(runtime_id: &str) -> String {
    format!("{runtime_id}-streamed-command-run")
}

pub fn command_run_live_delta_result(command: &Value, stdout: &str, stderr: &str) -> Value {
    let command_type = command
        .get("command_type")
        .or_else(|| command.get("command"))
        .and_then(Value::as_str)
        .unwrap_or(COMMAND_RUN_TOOL_NAME);
    let command_line = command
        .get("command_line")
        .or_else(|| command.get("command"))
        .and_then(Value::as_str)
        .unwrap_or(command_type);
    let step = command
        .get("step")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let mut output_text = String::from("Output:\n");
    output_text.push_str(stdout);
    if !stderr.is_empty() {
        output_text.push_str("\nStderr:\n");
        output_text.push_str(stderr);
    }
    serde_json::json!({
        "step": step,
        "command_type": command_type,
        "command_line": command_line,
        "status": "running",
        "success": null,
        "output": {
            "stdout": stdout,
            "stderr": stderr,
            "text": output_text,
        },
    })
}

pub struct StreamedCommandRunUpdate<'a> {
    pub session_id: &'a str,
    pub runtime_id: &'a str,
    pub provider: &'a Value,
    pub call_id: &'a str,
    pub commands: &'a [Value],
    pub results: &'a [Value],
    pub status: &'a str,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

pub async fn publish_streamed_command_run_update(update: StreamedCommandRunUpdate<'_>) {
    if gateway_callbacks_disabled() {
        return;
    }

    let target_session_id = gateway_callback_session_id(update.session_id);
    let endpoint = format!(
        "{}/session/{target_session_id}/message/agent",
        gateway_callback_base_url()
    );
    let input = serde_json::json!({ "commands": update.commands });
    let output = serde_json::json!({
        "streamed_command_run_result": {
            "results": update.results,
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
        "input": input,
        "output": output,
        "success": success,
        "error": error_value,
        "runtime_id": update.runtime_id,
        "session_id": update.session_id,
        "provider": update.provider,
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
        "streamed_command_run_result": {
            "results": update.results,
        },
        "title": if update.status == "completed" {
            "Called `command_run`"
        } else {
            "Calling `command_run`"
        },
        "metadata": metadata,
        "time": time,
    });
    let payload = serde_json::json!({
        "reply_message": "",
        "new_learning": "",
        "media": [],
        "runtime_id": update.runtime_id,
        "tool_call": {
            "tool_name": COMMAND_RUN_TOOL_NAME,
            "call_id": update.call_id,
            "state": state,
            "metadata": metadata,
        }
    });

    let result = reqwest::Client::new()
        .post(endpoint)
        .json(&payload)
        .send()
        .await;
    match result {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(
                session_id = %update.session_id,
                runtime_id = %update.runtime_id,
                gateway_status = %status,
                body = %body,
                "failed to publish streamed command_run update"
            );
        }
        Err(error) => {
            warn!(
                session_id = %update.session_id,
                runtime_id = %update.runtime_id,
                error = %error,
                "failed to call gateway for streamed command_run update"
            );
        }
    }
}

fn gateway_callbacks_disabled() -> bool {
    std::env::var("TURA_DISABLE_GATEWAY_CALLBACKS")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn gateway_callback_base_url() -> String {
    std::env::var("TURA_GATEWAY_URL")
        .or_else(|_| std::env::var("GATEWAY_BASE_URL"))
        .unwrap_or_else(|_| {
            let port = std::env::var("TURA_GATEWAY_PORT")
                .or_else(|_| std::env::var("PORT"))
                .unwrap_or_else(|_| "4096".to_string());
            format!("http://127.0.0.1:{port}")
        })
        .trim_end_matches('/')
        .to_string()
}

fn gateway_callback_session_id(session_id: &str) -> String {
    if planning_child_depth_from_env() > 0 {
        if let Ok(parent_session_id) = std::env::var("TURA_PARENT_SESSION_ID") {
            let parent_session_id = parent_session_id.trim();
            if !parent_session_id.is_empty() {
                return parent_session_id.to_string();
            }
        }
    }
    session_id.to_string()
}

fn planning_child_depth_from_env() -> usize {
    std::env::var("TURA_PLANNING_DEPTH")
        .or_else(|_| std::env::var("TURA_EXECUTE_TOOLS_DEPTH"))
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        command_run_stream_event_command, command_run_stream_events_from_provider_content,
        should_replay_final_response_command_run,
    };

    #[test]
    fn final_response_command_run_replay_is_skipped_after_streamed_command_seen() {
        assert!(!should_replay_final_response_command_run(true));
        assert!(should_replay_final_response_command_run(false));
    }

    #[test]
    fn final_response_command_run_events_still_extract_when_provider_did_not_stream() {
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
        assert_eq!(event.tool_call_id, "call_command_run_0");
        assert_eq!(event.command["command_type"], "apply_patch");
    }
}
