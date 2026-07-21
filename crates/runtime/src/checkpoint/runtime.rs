//! Runtime turn checkpoint helpers.

use lifecycle::RuntimeAggregate;
use session_log_contract::CheckpointType;

use super::command_run::{checkpoint_runtime_event, RuntimeCheckpoint};

fn checkpoint_runtime_state_event(
    runtime: &RuntimeAggregate,
    checkpoint_type: CheckpointType,
    provider_call_id: Option<&str>,
    command_run_id: Option<&str>,
    command_id: Option<&str>,
    event_seq: Option<i64>,
) -> Result<(), String> {
    let event_type = checkpoint_type.as_str();
    let payload = runtime_state_checkpoint_payload(runtime, checkpoint_type);
    if let Err(error) = checkpoint_runtime_event(RuntimeCheckpoint {
        session_id: &runtime.session_id,
        runtime_id: &runtime.runtime_id,
        runtime_worker_id: &runtime.runtime_id,
        provider_call_id,
        command_run_id,
        command_id,
        event_seq,
        checkpoint_type,
        payload,
        started_at: None,
        finished_at: None,
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

fn runtime_state_checkpoint_payload(
    runtime: &RuntimeAggregate,
    checkpoint_type: CheckpointType,
) -> serde_json::Value {
    let event_type = checkpoint_type.as_str();
    let mut payload = serde_json::json!({
        "event_type": event_type,
        "runtime_state": runtime.state,
        "call_result_status": runtime.call_result_status(),
    });
    if event_includes_usage(checkpoint_type) {
        payload["usage"] = serde_json::to_value(&runtime.usage).unwrap_or(serde_json::Value::Null);
    }
    payload
}

fn event_includes_usage(checkpoint_type: CheckpointType) -> bool {
    matches!(
        checkpoint_type,
        CheckpointType::ProviderCallFinished
            | CheckpointType::TurnFinished
            | CheckpointType::TurnFailed
            | CheckpointType::TurnInterrupted
    )
}

pub fn checkpoint_turn_started(runtime: &RuntimeAggregate) -> Result<(), String> {
    checkpoint_runtime_state_event(
        runtime,
        CheckpointType::TurnStarted,
        None,
        None,
        None,
        Some(1),
    )
}

pub fn checkpoint_provider_call_started(runtime: &RuntimeAggregate) -> Result<(), String> {
    checkpoint_runtime_state_event(
        runtime,
        CheckpointType::ProviderCallStarted,
        Some(&runtime.runtime_id),
        None,
        None,
        Some(2),
    )
}

pub fn checkpoint_provider_call_finished(runtime: &RuntimeAggregate) -> Result<(), String> {
    checkpoint_runtime_state_event(
        runtime,
        CheckpointType::ProviderCallFinished,
        Some(&runtime.runtime_id),
        None,
        None,
        Some(3),
    )
}

pub fn checkpoint_turn_finished(runtime: &RuntimeAggregate) -> Result<(), String> {
    checkpoint_runtime_state_event(
        runtime,
        CheckpointType::TurnFinished,
        None,
        None,
        None,
        Some(4),
    )
}

pub fn checkpoint_turn_failed(runtime: &RuntimeAggregate) -> Result<(), String> {
    checkpoint_runtime_state_event(
        runtime,
        CheckpointType::TurnFailed,
        None,
        None,
        None,
        Some(4),
    )
}

pub fn checkpoint_turn_interrupted(runtime: &RuntimeAggregate) -> Result<(), String> {
    checkpoint_runtime_state_event(
        runtime,
        CheckpointType::TurnInterrupted,
        None,
        None,
        None,
        Some(4),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use lifecycle::{ProviderConfig, ToolChoice};
    use lifecycle::{RuntimeProviderConfig, UsageReport};

    #[test]
    fn event_usage_inclusion_matches_terminal_checkpoint_contract() {
        for event in [
            CheckpointType::TurnStarted,
            CheckpointType::ProviderCallStarted,
            CheckpointType::CommandReady,
        ] {
            assert!(!event_includes_usage(event), "{event}");
        }
        for event in [
            CheckpointType::ProviderCallFinished,
            CheckpointType::TurnFinished,
            CheckpointType::TurnFailed,
            CheckpointType::TurnInterrupted,
        ] {
            assert!(event_includes_usage(event), "{event}");
        }
    }

    #[test]
    fn runtime_state_payload_uses_canonical_pascal_case_states_and_statuses() {
        let mut runtime = runtime_fixture();
        runtime
            .mark_called(runtime.created_at)
            .expect("mark called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        runtime
            .mark_first_token(runtime.created_at)
            .expect("mark first token");

        let payload =
            runtime_state_checkpoint_payload(&runtime, CheckpointType::ProviderCallStarted);

        assert_eq!(payload["event_type"], "provider_call_started");
        assert_eq!(payload["runtime_state"], "Streaming");
        assert_eq!(payload["call_result_status"], "Streaming");
        assert!(payload.get("usage").is_none());
    }

    #[test]
    fn terminal_runtime_state_payload_includes_usage_snapshot_or_null() {
        let mut runtime = runtime_fixture();
        runtime
            .mark_called(runtime.created_at)
            .expect("mark called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        let usage = UsageReport {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
            cached_input_tokens: 1,
            cache_write_tokens: 2,
            reasoning_tokens: 3,
            attachment_input_tokens: 4,
            input_cost: 0.01,
            output_cost: 0.02,
            total_cost: 0.03,
            currency: "USD".to_string(),
            pricing_source: "test".to_string(),
            latency_ms: 123,
            time_to_first_token_ms: 45,
            token_per_second: 9.5,
        };
        runtime
            .finish_success(runtime.created_at, Some(usage))
            .expect("finish runtime");

        let finished = runtime_state_checkpoint_payload(&runtime, CheckpointType::TurnFinished);
        assert_eq!(finished["usage"]["input_tokens"], 10);
        assert_eq!(finished["usage"]["total_tokens"], 15);
        assert_eq!(finished["usage"]["currency"], "USD");

        let failed =
            runtime_state_checkpoint_payload(&runtime_fixture(), CheckpointType::TurnFailed);
        assert_eq!(failed["usage"], serde_json::Value::Null);
    }

    #[test]
    fn checkpoint_wrappers_are_best_effort_when_session_db_is_unavailable() {
        let runtime = runtime_fixture();

        assert_eq!(checkpoint_turn_started(&runtime), Ok(()));
        assert_eq!(checkpoint_provider_call_started(&runtime), Ok(()));
        assert_eq!(checkpoint_provider_call_finished(&runtime), Ok(()));
        assert_eq!(checkpoint_turn_finished(&runtime), Ok(()));
        assert_eq!(checkpoint_turn_failed(&runtime), Ok(()));
        assert_eq!(checkpoint_turn_interrupted(&runtime), Ok(()));
    }

    fn runtime_fixture() -> RuntimeAggregate {
        RuntimeAggregate::new(
            "runtime-1".to_string(),
            "session-1".to_string(),
            "agent-1".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "flagship".to_string(),
                    default_model_tier: None,
                    current_model: None,
                    stream: true,
                    temperature: 0.2,
                    max_tokens: 4096,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: true,
                provider_name: "OpenAI".to_string(),
                model_name: "gpt-test".to_string(),
                provider_url_name: "openai".to_string(),
                llm_provider_name: "openai".to_string(),
            },
            Utc::now(),
        )
    }
}
