use std::time::Duration;

use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;
use crate::tool_router::execute_tool::ToolExecutionResult;

use super::constants::{
    APPLY_DIFF_TOOL, APPROVAL_POLICY_ENV, COMMAND_RUN_TOOL, DELETE_FILE_TOOL,
    PERMISSION_WAIT_SECONDS_ENV, WRITE_FILE_TOOL,
};
use super::gateway_events::gateway_callback_base_url;

#[derive(Debug, serde::Deserialize)]
struct PermissionCreateResponse {
    id: String,
}

#[derive(Debug, serde::Deserialize)]
struct PermissionStatusResponse {
    responded: bool,
    approve: Option<bool>,
}

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
    let gateway_base = gateway_callback_base_url();
    let client = reqwest::Client::new();
    let endpoint = format!("{gateway_base}/session/{}/permissions", session.session_id);
    let payload = serde_json::json!({
        "permission": permission,
        "args": args,
    });
    let created = client
        .post(endpoint)
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("failed to create permission request: {error}"))?;
    if !created.status().is_success() {
        let status = created.status();
        let body = created.text().await.unwrap_or_default();
        return Err(format!(
            "gateway permission request failed with {status}: {body}"
        ));
    }
    let request: PermissionCreateResponse = created
        .json()
        .await
        .map_err(|error| format!("failed to decode permission request: {error}"))?;

    let timeout = Duration::from_secs(permission_wait_seconds());
    let poll_started = std::time::Instant::now();
    loop {
        if poll_started.elapsed() >= timeout {
            return Err(format!(
                "permission request {} timed out after {} seconds",
                request.id,
                timeout.as_secs()
            ));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let status_endpoint = format!(
            "{gateway_base}/session/{}/permissions/{}/reply",
            session.session_id, request.id
        );
        let response = client
            .get(status_endpoint)
            .send()
            .await
            .map_err(|error| format!("failed to poll permission request: {error}"))?;
        if !response.status().is_success() {
            continue;
        }
        let status: PermissionStatusResponse = response
            .json()
            .await
            .map_err(|error| format!("failed to decode permission status: {error}"))?;
        if status.responded {
            return Ok(status.approve.unwrap_or(false));
        }
    }
}

fn permission_wait_seconds() -> u64 {
    std::env::var(PERMISSION_WAIT_SECONDS_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(300)
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
