use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::state_machine::session_management::SessionId;

use super::types::ToolRouterQueueItem;

#[derive(Debug, Clone)]
pub struct ExecuteToolInput {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub session_id: SessionId,
    pub runtime_id: String,
    pub session_directory: PathBuf,
    pub tools_directory: PathBuf,
    pub disable_permission_restrictions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub result: serde_json::Value,
    pub success: bool,
    pub error: Option<String>,
}

pub async fn execute_tool(input: ExecuteToolInput) -> Result<ToolExecutionResult, String> {
    let execution_tool_name = canonical_tool_file_name(&input.tool_name);
    let interface_path = input
        .tools_directory
        .join(execution_tool_name)
        .join("schema.json");

    if !interface_path.exists() {
        return Err(format!(
            "tool interface not found: {}",
            interface_path.display()
        ));
    }

    let interface_content = fs::read_to_string(&interface_path)
        .map_err(|e| format!("failed to read tool interface: {}", e))?;

    let _interface: serde_json::Value = serde_json::from_str(&interface_content)
        .map_err(|e| format!("failed to parse tool interface: {}", e))?;

    if execution_tool_name == "command_run" {
        let output_value = code_tools::command_run::execute_async_value(
            input.arguments.clone(),
            input.session_directory.clone(),
        )
        .await;
        return Ok(ToolExecutionResult {
            tool_name: input.tool_name.clone(),
            arguments: input.arguments,
            success: tool_output_success(&output_value),
            error: tool_output_error(&output_value),
            result: output_value,
        });
    }
    if input.tool_name == "multiple_tasks" {
        let output_value = code_tools::commands::multiple_tasks::execute_value(
            input.arguments.clone(),
            &input.session_directory,
        );
        return Ok(ToolExecutionResult {
            tool_name: input.tool_name.clone(),
            arguments: input.arguments,
            success: output_value.success,
            error: (!output_value.success).then_some(output_value.stderr.clone()),
            result: output_value.output,
        });
    }
    if execution_tool_name == "task_delivered" {
        let delivered = input
            .arguments
            .get("task_delivered")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        return Ok(ToolExecutionResult {
            tool_name: input.tool_name.clone(),
            arguments: input.arguments,
            success: delivered,
            error: (!delivered).then(|| "task_delivered must be true".to_string()),
            result: serde_json::json!({ "task_delivered": delivered }),
        });
    }

    Err(format!(
        "unsupported tool `{}`: this runtime exposes only command_run",
        input.tool_name
    ))
}

fn tool_output_success(output: &serde_json::Value) -> bool {
    if let Some(results) = output.get("results").and_then(serde_json::Value::as_array) {
        return results.iter().all(|result| {
            result.get("success").and_then(serde_json::Value::as_bool) == Some(true)
        });
    }
    output
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

fn tool_output_error(output: &serde_json::Value) -> Option<String> {
    if tool_output_success(output) {
        return None;
    }
    if let Some(message) = output
        .get("results")
        .and_then(serde_json::Value::as_array)
        .and_then(|results| {
            results.iter().find_map(|result| {
                if result.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
                    result
                        .get("error")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| result.get("output").and_then(serde_json::Value::as_str))
                } else {
                    None
                }
            })
        })
    {
        return Some(message.to_string());
    }
    output
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .and_then(|errors| errors.first())
        .and_then(|error| {
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| error.as_str())
        })
        .map(ToString::to_string)
}

fn canonical_tool_file_name(tool_name: &str) -> &str {
    match tool_name {
        "multiple_tasks" => "commands/multiple_tasks",
        _ => tool_name,
    }
}

#[cfg(test)]
mod tests {
    use super::{tool_output_error, tool_output_success};

    #[test]
    fn tool_output_success_follows_top_level_ok_field() {
        let failed = serde_json::json!({
            "ok": false,
            "errors": [{ "message": "command failed" }],
        });
        let succeeded = serde_json::json!({ "ok": true, "results": [] });
        let legacy = serde_json::json!({ "raw_output": "done" });

        assert!(!tool_output_success(&failed));
        assert_eq!(
            tool_output_error(&failed).as_deref(),
            Some("command failed")
        );
        assert!(tool_output_success(&succeeded));
        assert!(tool_output_error(&succeeded).is_none());
        assert!(tool_output_success(&legacy));
    }

    #[test]
    fn tool_output_success_follows_current_style_command_run_results() {
        let failed = serde_json::json!({
            "results": [
                { "step": 1, "command": "shell_command", "success": true, "output": {} },
                { "step": 2, "command": "apply_patch", "success": false, "error": "patch context not found" }
            ]
        });
        let succeeded = serde_json::json!({
            "results": [
                { "step": 1, "command": "shell_command", "success": true, "output": {} },
                { "step": 2, "command": "apply_patch", "success": true, "output": {} }
            ]
        });

        assert!(!tool_output_success(&failed));
        assert_eq!(
            tool_output_error(&failed).as_deref(),
            Some("patch context not found")
        );
        assert!(tool_output_success(&succeeded));
        assert!(tool_output_error(&succeeded).is_none());
    }
}

pub async fn dequeue_tool_call(
    session_id: &SessionId,
    redis_url: &str,
) -> Result<Option<ToolRouterQueueItem>, String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("tool_router:queue:{}", session_id);

    let result: Option<String> = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async(&mut con)
        .await
        .map_err(|e| format!("failed to dequeue tool call: {}", e))?;

    match result {
        Some(payload) => {
            let item: ToolRouterQueueItem = serde_json::from_str(&payload)
                .map_err(|e| format!("failed to deserialize tool call: {}", e))?;
            Ok(Some(item))
        }
        None => Ok(None),
    }
}

pub async fn enqueue_tool_result(
    result: &ToolExecutionResult,
    session_id: &SessionId,
    redis_url: &str,
) -> Result<(), String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("tool_result:queue:{}", session_id);

    let payload = serde_json::to_string(result)
        .map_err(|e| format!("failed to serialize tool result: {}", e))?;

    redis::cmd("RPUSH")
        .arg(&queue_key)
        .arg(&payload)
        .query_async::<_, ()>(&mut con)
        .await
        .map_err(|e| format!("failed to enqueue tool result: {}", e))?;

    Ok(())
}
