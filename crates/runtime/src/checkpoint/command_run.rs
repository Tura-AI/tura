//! Command-run checkpoint helpers.

use chrono::{DateTime, Utc};
use serde_json::Value;
use session_log::CommandCheckpoint;

use super::client::CheckpointClient;

#[derive(Debug, Clone)]
pub struct StreamedCommandCheckpoint<'a> {
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub runtime_worker_id: &'a str,
    pub command_run_id: &'a str,
    pub index: usize,
    pub result: &'a Value,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeCheckpoint<'a> {
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub runtime_worker_id: &'a str,
    pub provider_call_id: Option<&'a str>,
    pub command_run_id: Option<&'a str>,
    pub command_id: Option<&'a str>,
    pub event_seq: Option<i64>,
    pub event_type: &'a str,
    pub payload: Value,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

pub(super) fn checkpoint_runtime_event(input: RuntimeCheckpoint<'_>) -> Result<(), String> {
    let operation = input.event_type;
    let checkpoint = CommandCheckpoint {
        session_id: input.session_id.to_string(),
        turn_id: input.turn_id.to_string(),
        runtime_worker_id: Some(input.runtime_worker_id.to_string()),
        provider_call_id: input.provider_call_id.map(str::to_string),
        command_run_id: input.command_run_id.map(str::to_string),
        command_id: input.command_id.map(str::to_string),
        event_seq: input.event_seq,
        command_type: None,
        command_line: None,
        status: input.event_type.to_string(),
        output_summary: None,
        changes: input.payload,
        started_at: input.started_at.map(|value| value.to_rfc3339()),
        finished_at: input.finished_at.map(|value| value.to_rfc3339()),
    };
    write_checkpoint(operation, checkpoint)
}

pub fn checkpoint_command_run_started(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    started_at: DateTime<Utc>,
) -> Result<(), String> {
    checkpoint_runtime_event(RuntimeCheckpoint {
        session_id,
        turn_id: runtime_id,
        runtime_worker_id: runtime_id,
        provider_call_id: Some(runtime_id),
        command_run_id: Some(command_run_id),
        command_id: None,
        event_seq: Some(10),
        event_type: "command_run_started",
        payload: serde_json::json!({ "event_type": "command_run_started" }),
        started_at: Some(started_at),
        finished_at: None,
    })
}

pub fn checkpoint_command_ready(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    command_id: &str,
    command_index: usize,
    command: &Value,
    ready_at: DateTime<Utc>,
) -> Result<(), String> {
    checkpoint_runtime_event(RuntimeCheckpoint {
        session_id,
        turn_id: runtime_id,
        runtime_worker_id: runtime_id,
        provider_call_id: Some(runtime_id),
        command_run_id: Some(command_run_id),
        command_id: Some(command_id),
        event_seq: Some(20 + command_index as i64),
        event_type: "command_ready",
        payload: serde_json::json!({
            "event_type": "command_ready",
            "command": command,
        }),
        started_at: Some(ready_at),
        finished_at: None,
    })
}

pub fn checkpoint_command_started(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    command_id: &str,
    command_index: usize,
    command: &Value,
    started_at: DateTime<Utc>,
) -> Result<(), String> {
    checkpoint_runtime_event(RuntimeCheckpoint {
        session_id,
        turn_id: runtime_id,
        runtime_worker_id: runtime_id,
        provider_call_id: Some(runtime_id),
        command_run_id: Some(command_run_id),
        command_id: Some(command_id),
        event_seq: Some(30 + command_index as i64),
        event_type: "command_started",
        payload: serde_json::json!({
            "event_type": "command_started",
            "command": command,
        }),
        started_at: Some(started_at),
        finished_at: None,
    })
}

pub fn checkpoint_command_run_finished(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    status: &str,
    result_count: usize,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> Result<(), String> {
    checkpoint_runtime_event(RuntimeCheckpoint {
        session_id,
        turn_id: runtime_id,
        runtime_worker_id: runtime_id,
        provider_call_id: Some(runtime_id),
        command_run_id: Some(command_run_id),
        command_id: None,
        event_seq: Some(90),
        event_type: "command_run_finished",
        payload: serde_json::json!({
            "event_type": "command_run_finished",
            "status": status,
            "result_count": result_count,
        }),
        started_at: Some(started_at),
        finished_at: Some(finished_at),
    })
}

pub fn checkpoint_streamed_command_finished(
    input: StreamedCommandCheckpoint<'_>,
) -> Result<(), String> {
    let result = input.result;
    let command_id = result
        .get("id")
        .or_else(|| result.get("command_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("command-{}", input.index));
    let command_type = result
        .get("command_type")
        .or_else(|| result.get("command"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let command_line = result
        .get("command_line")
        .or_else(|| result.get("display_command"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let success = result
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            result
                .get("exit_code")
                .and_then(Value::as_i64)
                .map(|code| code == 0)
                .unwrap_or(true)
        });
    let output_summary = result
        .get("stdout")
        .or_else(|| result.get("output"))
        .and_then(Value::as_str)
        .map(|text| text.chars().take(4000).collect::<String>());
    let status = if success {
        "command_finished"
    } else {
        "command_failed"
    };
    let checkpoint = CommandCheckpoint {
        session_id: input.session_id.to_string(),
        turn_id: input.turn_id.to_string(),
        runtime_worker_id: Some(input.runtime_worker_id.to_string()),
        provider_call_id: None,
        command_run_id: Some(input.command_run_id.to_string()),
        command_id: Some(command_id),
        event_seq: Some(input.index as i64),
        command_type,
        command_line,
        status: status.to_string(),
        output_summary,
        changes: result.get("changes").cloned().unwrap_or(Value::Null),
        started_at: None,
        finished_at: Some(input.finished_at.to_rfc3339()),
    };
    write_checkpoint(status, checkpoint)
}

fn write_checkpoint(operation: &str, checkpoint: CommandCheckpoint) -> Result<(), String> {
    CheckpointClient::discover()
        .map_err(|error| checkpoint_error("discover", operation, &error.to_string()))?
        .checkpoint_command_finished(checkpoint)
        .map_err(|error| checkpoint_error("write", operation, &error.to_string()))
}

fn checkpoint_error(stage: &str, operation: &str, error: &str) -> String {
    format!("failed to {stage} runtime checkpoint for {operation}: {error}")
}

#[cfg(test)]
mod tests {
    use super::checkpoint_error;

    #[test]
    fn checkpoint_error_keeps_stage_operation_and_source_error() {
        let error = checkpoint_error("write", "command_ready", "sqlite busy");

        assert!(error.contains("failed to write runtime checkpoint"));
        assert!(error.contains("command_ready"));
        assert!(error.contains("sqlite busy"));
    }
}
