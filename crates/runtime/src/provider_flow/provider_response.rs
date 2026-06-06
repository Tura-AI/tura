use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::state_machine::runtime_management::{RuntimeManagement, ToolCallRecord};

const COMMAND_RUN_TOOL_NAME: &str = "command_run";

pub(crate) fn apply_provider_response(
    runtime: &mut RuntimeManagement,
    content: &Value,
    now: DateTime<Utc>,
) {
    apply_provider_response_with_options(runtime, content, now, false);
}

pub(crate) fn apply_provider_response_with_options(
    runtime: &mut RuntimeManagement,
    content: &Value,
    now: DateTime<Utc>,
    suppress_command_run_tool_calls: bool,
) {
    let content = tura_llm_rust::normalize_response_content(content);

    if let Some(text) = tura_llm_rust::extract_response_text(&content)
        .map(|text| tura_llm_rust::strip_thought_blocks(&text))
    {
        runtime.append_text(&text);
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
        });
    }
}
