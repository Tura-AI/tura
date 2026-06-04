use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;
use crate::tool_router::execute_tool::ToolExecutionResult;

use super::constants::{
    APPLY_DIFF_TOOL, APPROVAL_POLICY_ENV, COMMAND_RUN_TOOL, DELETE_FILE_TOOL, WRITE_FILE_TOOL,
};

pub(super) fn permission_denial_for_tool(
    tool_name: &str,
    arguments: &serde_json::Value,
    session: &SessionManagement,
    runtime: &RuntimeManagement,
) -> Option<ToolExecutionResult> {
    if !should_request_permission(tool_name) {
        return None;
    }

    let outcome = tokio::runtime::Runtime::new()
        .map_err(|error| format!("failed to create permission runtime: {error}"))
        .and_then(|runtime_handle| {
            runtime_handle.block_on(request_permission_for_tool(
                tool_name, arguments, session, runtime,
            ))
        });

    match outcome {
        Ok(true) => None,
        Ok(false) => Some(blocked_tool_result(
            tool_name,
            arguments.clone(),
            "permission denied by user".to_string(),
        )),
        Err(error) => Some(blocked_tool_result(tool_name, arguments.clone(), error)),
    }
}

pub(super) fn request_command_run_sandbox_bypass(
    arguments: &serde_json::Value,
    session: &SessionManagement,
    runtime: &RuntimeManagement,
    reason: &str,
) -> Result<bool, String> {
    tokio::runtime::Runtime::new()
        .map_err(|error| format!("failed to create permission runtime: {error}"))
        .and_then(|runtime_handle| {
            runtime_handle.block_on(request_permission(
                "command_run:sandbox_bypass",
                serde_json::json!({
                    "tool_name": COMMAND_RUN_TOOL,
                    "arguments": arguments,
                    "runtime_id": &runtime.runtime_id,
                    "reason": reason,
                }),
                session,
            ))
        })
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
    matches!(
        tool_name,
        COMMAND_RUN_TOOL | APPLY_DIFF_TOOL | DELETE_FILE_TOOL | WRITE_FILE_TOOL
    )
}

async fn request_permission_for_tool(
    tool_name: &str,
    arguments: &serde_json::Value,
    session: &SessionManagement,
    runtime: &RuntimeManagement,
) -> Result<bool, String> {
    request_permission(
        &format!("tool:{tool_name}"),
        serde_json::json!({
            "tool_name": tool_name,
            "arguments": arguments,
            "runtime_id": &runtime.runtime_id,
        }),
        session,
    )
    .await
}

async fn request_permission(
    permission: &str,
    args: serde_json::Value,
    session: &SessionManagement,
) -> Result<bool, String> {
    let _ = (permission, args, session);
    Err("runtime cannot request gateway permissions directly; route permission decisions through gateway/router before dispatch".to_string())
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
