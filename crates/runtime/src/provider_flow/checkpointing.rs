use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::checkpoint::StreamedCommandCheckpoint;
use crate::state_machine::runtime_management::{RuntimeCallResultStatus, RuntimeManagement};

pub(crate) fn turn_started(runtime: &RuntimeManagement) -> Result<(), String> {
    crate::checkpoint::checkpoint_turn_started(runtime)
}

pub(crate) fn provider_call_started(runtime: &RuntimeManagement) -> Result<(), String> {
    crate::checkpoint::checkpoint_provider_call_started(runtime)
}

pub(crate) fn provider_call_finished(runtime: &RuntimeManagement) -> Result<(), String> {
    crate::checkpoint::checkpoint_provider_call_finished(runtime)
}

pub(crate) fn terminal_turn(runtime: &RuntimeManagement) -> Result<(), String> {
    match runtime.call_result_status {
        RuntimeCallResultStatus::Failed | RuntimeCallResultStatus::TimedOut => {
            crate::checkpoint::checkpoint_turn_failed(runtime)
        }
        RuntimeCallResultStatus::Cancelled => {
            crate::checkpoint::checkpoint_turn_interrupted(runtime)
        }
        _ => crate::checkpoint::checkpoint_turn_finished(runtime),
    }
}

pub(crate) fn best_effort_turn_failed(runtime: &RuntimeManagement) {
    let _ = crate::checkpoint::checkpoint_turn_failed(runtime);
}

pub(crate) fn command_run_started(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    started_at: DateTime<Utc>,
) -> Result<(), String> {
    crate::checkpoint::checkpoint_command_run_started(
        session_id,
        runtime_id,
        command_run_id,
        started_at,
    )
}

pub(crate) fn command_ready(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    command_id: &str,
    command_index: usize,
    command: &Value,
    ready_at: DateTime<Utc>,
) -> Result<(), String> {
    crate::checkpoint::checkpoint_command_ready(
        session_id,
        runtime_id,
        command_run_id,
        command_id,
        command_index,
        command,
        ready_at,
    )
}

pub(crate) fn command_started(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    command_id: &str,
    command_index: usize,
    command: &Value,
    started_at: DateTime<Utc>,
) -> Result<(), String> {
    crate::checkpoint::checkpoint_command_started(
        session_id,
        runtime_id,
        command_run_id,
        command_id,
        command_index,
        command,
        started_at,
    )
}

pub(crate) fn streamed_command_finished(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    index: usize,
    result: &Value,
    finished_at: DateTime<Utc>,
) -> Result<(), String> {
    crate::checkpoint::checkpoint_streamed_command_finished(StreamedCommandCheckpoint {
        session_id,
        turn_id: runtime_id,
        runtime_worker_id: runtime_id,
        command_run_id,
        index,
        result,
        finished_at,
    })
}

pub(crate) fn command_run_finished(
    session_id: &str,
    runtime_id: &str,
    command_run_id: &str,
    status: &str,
    result_count: usize,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> Result<(), String> {
    crate::checkpoint::checkpoint_command_run_finished(
        session_id,
        runtime_id,
        command_run_id,
        status,
        result_count,
        started_at,
        finished_at,
    )
}
