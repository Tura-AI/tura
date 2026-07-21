use chrono::{DateTime, Utc};
use serde_json::Value;

use lifecycle::{RuntimeAggregate, ToolCallRecord};

const COMMAND_RUN_TOOL_NAME: &str = "command_run";

pub(crate) fn apply_provider_response(
    runtime: &mut RuntimeAggregate,
    content: &Value,
    now: DateTime<Utc>,
) -> Result<(), String> {
    apply_provider_response_with_options(runtime, content, now, false)
}

pub(crate) fn apply_provider_response_with_options(
    runtime: &mut RuntimeAggregate,
    content: &Value,
    now: DateTime<Utc>,
    suppress_command_run_tool_calls: bool,
) -> Result<(), String> {
    let content = tura_llm_rust::normalize_response_content(content);

    if let Some(text) = tura_llm_rust::extract_response_text(&content)
        .map(|text| tura_llm_rust::strip_thought_blocks(&text))
    {
        runtime.append_text(text)?;
    }

    for tool_call in tura_llm_rust::extract_tool_calls(&content) {
        if suppress_command_run_tool_calls && tool_call.tool_name == COMMAND_RUN_TOOL_NAME {
            continue;
        }
        runtime.push_tool_call(ToolCallRecord {
            tool_called_name: tool_call.tool_name,
            tool_called_input: tool_call.arguments,
            provider_metadata: tool_call.provider_metadata,
            tool_received_at: now,
            tool_executed_at: now,
            tool_calldata_received_at: now,
            tool_reported_success: false,
            agent_reported_success: false,
            agent_reported_helpful: false,
            agent_reported_summary: String::new(),
            validator_reported_success: None,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::apply_provider_response_with_options;
    use chrono::Utc;
    use lifecycle::{ProviderConfig, ToolChoice};
    use lifecycle::{RuntimeAggregate, RuntimeProviderConfig};
    use serde_json::json;

    fn runtime() -> RuntimeAggregate {
        RuntimeAggregate::new(
            "runtime-provider-response-test".to_string(),
            "session-provider-response-test".to_string(),
            "agent-provider-response-test".to_string(),
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
            },
            Utc::now(),
        )
    }

    #[test]
    fn provider_response_can_suppress_raw_command_run_tool_calls() {
        let mut runtime = runtime();
        let now = runtime.created_at;
        let content = json!({
            "text": "visible text",
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "command_run",
                    "arguments": {
                        "commands": [{ "command_type": "apply_patch", "command_line": "ignored patch body" }]
                    }
                }
            }]
        });

        apply_provider_response_with_options(&mut runtime, &content, now, true)
            .expect("provider response should apply");

        assert_eq!(runtime.text, "visible text");
        assert!(runtime.tool_call.is_empty());
    }
}
