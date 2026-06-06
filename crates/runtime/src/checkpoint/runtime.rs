//! Runtime turn checkpoint helpers.

use crate::state_machine::runtime_management::RuntimeManagement;

use super::command_run::{checkpoint_runtime_event, RuntimeCheckpoint};

fn checkpoint_runtime_state_event(
    runtime: &RuntimeManagement,
    event_type: &str,
    provider_call_id: Option<&str>,
    command_run_id: Option<&str>,
    command_id: Option<&str>,
    event_seq: Option<i64>,
) -> Result<(), String> {
    if let Err(error) = checkpoint_runtime_event(RuntimeCheckpoint {
        session_id: &runtime.session_id,
        turn_id: &runtime.runtime_id,
        runtime_worker_id: &runtime.runtime_id,
        provider_call_id,
        command_run_id,
        command_id,
        event_seq,
        event_type,
        payload: serde_json::json!({
            "event_type": event_type,
            "runtime_state": runtime.state,
            "call_result_status": runtime.call_result_status,
        }),
    }) {
        tracing::warn!(
            session_id = %runtime.session_id,
            runtime_id = %runtime.runtime_id,
            event_type,
            error = %error,
            "failed to persist runtime state checkpoint"
        );
    }
    Ok(())
}

pub fn checkpoint_turn_started(runtime: &RuntimeManagement) -> Result<(), String> {
    checkpoint_runtime_state_event(runtime, "turn_started", None, None, None, Some(1))
}

pub fn checkpoint_provider_call_started(runtime: &RuntimeManagement) -> Result<(), String> {
    checkpoint_runtime_state_event(
        runtime,
        "provider_call_started",
        Some(&runtime.runtime_id),
        None,
        None,
        Some(2),
    )
}

pub fn checkpoint_provider_call_finished(runtime: &RuntimeManagement) -> Result<(), String> {
    checkpoint_runtime_state_event(
        runtime,
        "provider_call_finished",
        Some(&runtime.runtime_id),
        None,
        None,
        Some(3),
    )
}

pub fn checkpoint_turn_finished(runtime: &RuntimeManagement) -> Result<(), String> {
    checkpoint_runtime_state_event(runtime, "turn_finished", None, None, None, Some(4))
}

pub fn checkpoint_turn_failed(runtime: &RuntimeManagement) -> Result<(), String> {
    checkpoint_runtime_state_event(runtime, "turn_failed", None, None, None, Some(4))
}

pub fn checkpoint_turn_interrupted(runtime: &RuntimeManagement) -> Result<(), String> {
    checkpoint_runtime_state_event(runtime, "turn_interrupted", None, None, None, Some(4))
}
