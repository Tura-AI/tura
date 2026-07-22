use chrono::Utc;

use crate::gateway_events::{runtime_message_id, runtime_text_part_id};
use crate::manas::{user_visible_runtime_output_text, user_visible_runtime_text};
use crate::provider_flow::usage::runtime_cache_diagnostics;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;

pub(crate) fn accumulate_session_from_runtime(
    session: &mut SessionManagement,
    runtime: &RuntimeManagement,
    publish_runtime_text: bool,
) -> Result<(), String> {
    let now = Utc::now();

    session.runtime_usage = runtime
        .usage
        .as_ref()
        .map(|usage| serde_json::to_value(usage).unwrap_or(serde_json::Value::Null))
        .unwrap_or(serde_json::Value::Null);

    if let Some(usage) = &runtime.usage {
        session.push_log(
            serde_json::json!({
                "type": "runtime_usage",
                "runtime_id": runtime.runtime_id,
                "usage": usage,
                "status": format!("{:?}", runtime.call_result_status),
                "cache_diagnostics": runtime_cache_diagnostics(runtime),
                "timestamp": now.to_rfc3339(),
            })
            .to_string(),
            now,
        );
    }

    if !publish_runtime_text {
        return Ok(());
    }

    let visible_text = user_visible_runtime_text(&runtime.text).or_else(|| {
        runtime
            .output
            .as_ref()
            .and_then(user_visible_runtime_output_text)
    });

    if let Some(content) = visible_text {
        let (created_at, updated_at) = runtime.assistant_message_timestamps();
        let message_timestamp = runtime
            .call_finished_at
            .or(runtime.first_token_at)
            .or(runtime.called_at)
            .unwrap_or(runtime.created_at);
        session.push_log(
            serde_json::json!({
                "id": runtime_message_id(&runtime.runtime_id),
                "role": "assistant",
                "content": content,
                "part_id": runtime_text_part_id(&runtime.runtime_id),
                "runtime_id": runtime.runtime_id,
                "created_at": created_at,
                "updated_at": updated_at,
                "timestamp": message_timestamp.to_rfc3339(),
            })
            .to_string(),
            message_timestamp,
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::accumulate_session_from_runtime;
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{
        RuntimeManagement, RuntimeProviderConfig, UsageReport,
    };
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use chrono::{Duration, Utc};
    use std::path::PathBuf;

    #[test]
    fn assistant_session_log_reuses_runtime_message_ids_and_timestamps() {
        let session_created_at = Utc::now();
        let mut session = SessionManagement::new(
            "session-provider-step".to_string(),
            "provider step".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "hello".to_string(),
                file_input: Vec::new(),
                agent: Some("direct".to_string()),
                runtime_context: None,
                planning_mode_override: None,
            },
            "hello".to_string(),
            session_created_at,
        );
        let mut runtime = RuntimeManagement::new(
            "runtime-provider-step".to_string(),
            session.session_id.clone(),
            "agent-provider-step".to_string(),
            provider_config(),
            session_created_at + Duration::milliseconds(5),
        );
        let called_at = runtime.created_at + Duration::milliseconds(10);
        let first_token_at = called_at + Duration::milliseconds(20);
        let finished_at = first_token_at + Duration::milliseconds(30);
        runtime.mark_called(called_at).expect("mark called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        runtime
            .mark_first_token(first_token_at)
            .expect("mark first token");
        runtime.append_text("Visible assistant reply");
        runtime.finish_success(finished_at, None).expect("finish");

        accumulate_session_from_runtime(&mut session, &runtime, true).expect("accumulate");

        let entry = session
            .session_log
            .last()
            .expect("assistant message should be appended");
        let value: serde_json::Value = serde_json::from_str(entry).expect("assistant log json");
        assert_eq!(value["id"], "runtime-provider-step.message");
        assert_eq!(value["part_id"], "runtime-provider-step.message");
        assert_eq!(value["created_at"], first_token_at.timestamp_millis());
        assert_eq!(value["updated_at"], finished_at.timestamp_millis());
        assert_eq!(value["timestamp"], finished_at.to_rfc3339());
    }

    #[test]
    fn runtime_usage_updates_session_snapshot_usage() {
        let session_created_at = Utc::now();
        let mut session = SessionManagement::new(
            "session-runtime-usage".to_string(),
            "runtime usage".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "hello".to_string(),
                file_input: Vec::new(),
                agent: Some("direct".to_string()),
                runtime_context: None,
                planning_mode_override: None,
            },
            "hello".to_string(),
            session_created_at,
        );
        let mut runtime = RuntimeManagement::new(
            "runtime-provider-usage".to_string(),
            session.session_id.clone(),
            "agent-provider-step".to_string(),
            provider_config(),
            session_created_at + Duration::milliseconds(5),
        );
        let called_at = runtime.created_at + Duration::milliseconds(10);
        let first_token_at = called_at + Duration::milliseconds(20);
        let finished_at = first_token_at + Duration::milliseconds(30);
        runtime.mark_called(called_at).expect("mark called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        runtime
            .mark_first_token(first_token_at)
            .expect("mark first token");
        runtime
            .finish_success(
                finished_at,
                Some(UsageReport {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                    cached_input_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                    attachment_input_tokens: 0,
                    input_cost: 0.01,
                    output_cost: 0.02,
                    total_cost: 0.03,
                    currency: "USD".to_string(),
                    pricing_source: "test".to_string(),
                    routing_ms: 10,
                    provider_queue_ms: 5,
                    latency_ms: 100,
                    time_to_first_token_ms: 25,
                    token_per_second: 50.0,
                }),
            )
            .expect("finish");

        accumulate_session_from_runtime(&mut session, &runtime, true).expect("accumulate");

        assert_eq!(session.runtime_usage["total_tokens"], 15);
        assert_eq!(session.runtime_usage["total_cost"], 0.03);
        assert_eq!(session.runtime_usage["currency"], "USD");
    }

    fn provider_config() -> RuntimeProviderConfig {
        RuntimeProviderConfig {
            base: ProviderConfig {
                tura_llm_name: "fast".to_string(),
                default_model_tier: None,
                current_model: None,
                stream: true,
                temperature: 0.0,
                max_tokens: 1024,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 30_000,
            },
            thinking: false,
            provider_name: "openai".to_string(),
            model_name: "gpt-test".to_string(),
            provider_url_name: "openai".to_string(),
            llm_provider_name: "openai".to_string(),
        }
    }
}
