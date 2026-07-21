use crate::tool_router::execute_tool::ToolExecutionResult;
use lifecycle::RuntimeAggregate;

use crate::manas::constants::{APPROVAL_POLICY_ENV, COMMAND_RUN_TOOL};

pub(crate) fn permission_denial_for_tool(
    tool_name: &str,
    arguments: &serde_json::Value,
    runtime: &RuntimeAggregate,
) -> Option<ToolExecutionResult> {
    if !should_request_permission(tool_name) {
        return None;
    }

    Some(blocked_tool_result(
        tool_name,
        arguments.clone(),
        format!(
            "runtime cannot request gateway permissions directly for tool `{tool_name}` in runtime `{}`; route permission decisions through gateway/router before dispatch",
            runtime.runtime_id
        ),
    ))
}

pub(crate) fn request_command_run_sandbox_bypass(
    runtime: &RuntimeAggregate,
    reason: &str,
) -> Result<bool, String> {
    Err(format!(
        "runtime cannot request gateway permission `{COMMAND_RUN_TOOL}:sandbox_bypass` for runtime `{}`: {reason}",
        runtime.runtime_id
    ))
}

fn should_request_permission(tool_name: &str) -> bool {
    match approval_policy().as_deref() {
        Some("always") => true,
        Some("on-request") | Some("on_request") | Some("untrusted") => is_high_risk_tool(tool_name),
        _ => false,
    }
}

fn approval_policy() -> Option<String> {
    std::env::var(APPROVAL_POLICY_ENV)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty() && value != "never")
}

fn is_high_risk_tool(tool_name: &str) -> bool {
    tool_name == COMMAND_RUN_TOOL
}

fn blocked_tool_result(
    tool_name: &str,
    arguments: serde_json::Value,
    error: String,
) -> ToolExecutionResult {
    ToolExecutionResult {
        tool_name: tool_name.to_string(),
        arguments,
        result: serde_json::json!({
            "ok": false,
            "blocked": true,
            "error": error,
        }),
        success: false,
        error: Some(error),
    }
}
