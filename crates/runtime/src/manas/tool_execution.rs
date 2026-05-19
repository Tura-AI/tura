use chrono::Utc;
use tracing::error;

use crate::runtime::types::ToolCallData;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::{SessionManagement, TaskStatus, TaskStep};
use crate::tool_router::execute_tool::{execute_tool, ExecuteToolInput, ToolExecutionResult};

use super::change_tracker::{append_successful_changes, capture_pending_changes};
use super::constants::{COMMAND_RUN_TOOL, PLANNING_TOOL};
use super::gateway_events::{
    publish_step_summary, publish_task_plan_todos, publish_tool_call_record,
    publish_tool_call_started,
};
use super::permission_gate::{permission_denial_for_tool, request_command_run_sandbox_bypass};
use super::tool_arguments::normalize_tool_arguments_for_tool;
use super::tool_catalog::project_directory_with_tools;

pub(super) fn execute_tool_calls(
    tool_calls: &[ToolCallData],
    session: &mut SessionManagement,
    runtime: &RuntimeManagement,
    _redis_url: &str,
) -> Result<Vec<ToolExecutionResult>, String> {
    let mut results = Vec::new();
    let project_directory = project_directory_with_tools()?;
    let tools_directory = project_directory.join("crates").join("tools").join("src");

    for tool_call in tool_calls {
        let tool_started_at = Utc::now();
        let normalized_arguments = normalize_tool_arguments_for_tool(
            &tool_call.tool_name,
            tool_call.arguments.clone(),
            &session.session_directory,
        );
        publish_tool_call_started(
            session,
            runtime,
            tool_call,
            normalized_arguments.clone(),
            tool_started_at,
        );
        publish_step_summary(session, runtime, tool_call);
        if let Some(blocked_result) = permission_denial_for_tool(
            &tool_call.tool_name,
            &normalized_arguments,
            session,
            runtime,
        ) {
            publish_tool_call_record(
                session,
                runtime,
                tool_call,
                normalized_arguments,
                &blocked_result.result,
                false,
                blocked_result.error.as_deref(),
                tool_started_at,
            );
            results.push(blocked_result);
            continue;
        }
        let pending_changes = capture_pending_changes(&tool_call.tool_name, &normalized_arguments);
        let execute_input = ExecuteToolInput {
            tool_name: tool_call.tool_name.clone(),
            arguments: normalized_arguments.clone(),
            session_id: session.session_id.clone(),
            runtime_id: runtime.runtime_id.clone(),
            session_directory: session.session_directory.clone(),
            tools_directory: tools_directory.clone(),
            disable_permission_restrictions: session.disable_permission_restrictions,
        };

        let mut result = tokio::runtime::Runtime::new()
            .map_err(|e| format!("failed to create runtime: {}", e))?
            .block_on(execute_tool(execute_input.clone()));
        if command_run_hit_workspace_sandbox(&tool_call.tool_name, &result)
            && !session.disable_permission_restrictions
        {
            let reason = result
                .as_ref()
                .ok()
                .and_then(|execution_result| execution_result.error.as_deref())
                .unwrap_or("command_run requested access outside the session workspace");
            match request_command_run_sandbox_bypass(
                &normalized_arguments,
                session,
                runtime,
                reason,
            ) {
                Ok(true) => {
                    let mut approved_input = execute_input.clone();
                    approved_input.disable_permission_restrictions = true;
                    result = tokio::runtime::Runtime::new()
                        .map_err(|e| format!("failed to create runtime: {}", e))?
                        .block_on(execute_tool(approved_input));
                }
                Ok(false) => {
                    result = Ok(ToolExecutionResult {
                        tool_name: tool_call.tool_name.clone(),
                        arguments: normalized_arguments.clone(),
                        result: serde_json::json!({
                            "ok": false,
                            "blocked": true,
                            "error": "permission denied by user",
                        }),
                        success: false,
                        error: Some("permission denied by user".to_string()),
                    });
                }
                Err(error) => {
                    let error_message = error.clone();
                    result = Ok(ToolExecutionResult {
                        tool_name: tool_call.tool_name.clone(),
                        arguments: normalized_arguments.clone(),
                        result: serde_json::json!({
                            "ok": false,
                            "blocked": true,
                            "error": error_message,
                        }),
                        success: false,
                        error: Some(error),
                    });
                }
            }
        }

        match result {
            Ok(execution_result) => {
                if apply_tool_result_session_state_update(
                    session,
                    &tool_call.tool_name,
                    &execution_result.result,
                ) {
                    publish_task_plan_todos(session);
                }
                append_successful_changes(
                    &session.session_directory,
                    &session.session_id,
                    &runtime.runtime_id,
                    pending_changes,
                    &execution_result,
                );
                publish_tool_call_record(
                    session,
                    runtime,
                    tool_call,
                    normalized_arguments,
                    &execution_result.result,
                    execution_result.success,
                    execution_result.error.as_deref(),
                    tool_started_at,
                );
                results.push(execution_result);
            }
            Err(e) => {
                error!(tool_name = %tool_call.tool_name, error = %e, "tool execution failed");
                publish_tool_call_record(
                    session,
                    runtime,
                    tool_call,
                    normalized_arguments,
                    &serde_json::Value::Null,
                    false,
                    Some(e.as_str()),
                    tool_started_at,
                );
                results.push(ToolExecutionResult {
                    tool_name: tool_call.tool_name.clone(),
                    arguments: tool_call.arguments.clone(),
                    result: serde_json::Value::Null,
                    success: false,
                    error: Some(e),
                });
            }
        }
    }

    Ok(results)
}

fn command_run_hit_workspace_sandbox(
    tool_name: &str,
    result: &Result<ToolExecutionResult, String>,
) -> bool {
    if tool_name != COMMAND_RUN_TOOL {
        return false;
    }
    let text = match result {
        Ok(result) => format!(
            "{} {}",
            result.error.as_deref().unwrap_or_default(),
            result.result
        ),
        Err(error) => error.clone(),
    };
    text.contains("command denied by sandbox policy") || text.contains("path outside workspace")
}

fn apply_tool_result_session_state_update(
    session: &mut SessionManagement,
    tool_name: &str,
    result: &serde_json::Value,
) -> bool {
    if tool_name != PLANNING_TOOL {
        return false;
    }
    let Some(plan) = planning_output_from_tool_result(result) else {
        return false;
    };
    session.task_plan.summary = plan
        .get("user_task")
        .and_then(|value| value.as_str())
        .unwrap_or(&session.user_goal)
        .to_string();
    let steps = plan
        .get("steps")
        .and_then(|value| value.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    session.task_plan.detailed_tasks = steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| task_step_from_planning_step(index, step))
        .collect();
    session.session_last_update_at = Utc::now();
    true
}

fn planning_output_from_tool_result(result: &serde_json::Value) -> Option<serde_json::Value> {
    let first = result
        .get("results")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| result.clone());
    if first.get("steps").is_some() {
        Some(first)
    } else {
        None
    }
}

fn task_step_from_planning_step(index: usize, value: &serde_json::Value) -> Option<TaskStep> {
    let object = value.as_object()?;
    let task_instruction = object
        .get("task_instruction")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let step_goal = object
        .get("step_goal")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let task_name = if !step_goal.trim().is_empty() {
        step_goal.clone()
    } else if !task_instruction.trim().is_empty() {
        task_instruction.clone()
    } else {
        format!("Step {}", index + 1)
    };
    let status = if object.get("ok").and_then(|value| value.as_bool()) == Some(true) {
        TaskStatus::Completed
    } else if object.get("ok").and_then(|value| value.as_bool()) == Some(false) {
        TaskStatus::Cancelled
    } else {
        TaskStatus::Pending
    };
    Some(TaskStep {
        task_name,
        status,
        task_summary: step_goal.clone(),
        step_task: task_instruction,
        step_turn: object
            .get("child_session_turns")
            .and_then(|value| value.as_u64())
            .unwrap_or_default(),
        step_memory: String::new(),
        step_tool: object
            .get("tool_needed")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        step_context: serde_json::to_string(value).unwrap_or_default(),
        step_agent_name: object
            .get("child_agent_names")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        step_deliverable_description: object
            .get("deliverable")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        step_deliverable_path: std::path::PathBuf::new(),
    })
}
