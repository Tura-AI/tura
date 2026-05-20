use chrono::Utc;
use tracing::error;

use crate::runtime::types::ToolCallData;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::{SessionManagement, TaskStatus, TaskStep};
use crate::tool_router::execute_tool::{execute_tool, ExecuteToolInput, ToolExecutionResult};

use super::change_tracker::{append_successful_changes, capture_pending_changes};
use super::constants::{COMMAND_RUN_TOOL, MULTIPLE_TASKS_TOOL, TASK_DELIVERED_TOOL};
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
        if tool_call.tool_name == COMMAND_RUN_TOOL {
            if let Some(streamed_result) = streamed_command_run_result(runtime) {
                let execution_result = ToolExecutionResult {
                    tool_name: tool_call.tool_name.clone(),
                    arguments: normalized_arguments.clone(),
                    success: command_run_result_success(&streamed_result),
                    error: command_run_result_error(&streamed_result),
                    result: streamed_result,
                };
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
                continue;
            }
        }
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
                    &execution_result.arguments,
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

fn streamed_command_run_result(runtime: &RuntimeManagement) -> Option<serde_json::Value> {
    runtime
        .output
        .as_ref()?
        .get("streamed_command_run_result")
        .cloned()
}

fn command_run_result_success(output: &serde_json::Value) -> bool {
    output
        .get("results")
        .and_then(serde_json::Value::as_array)
        .map(|results| {
            results.iter().all(|result| {
                result.get("success").and_then(serde_json::Value::as_bool) == Some(true)
            })
        })
        .unwrap_or(true)
}

fn command_run_result_error(output: &serde_json::Value) -> Option<String> {
    if command_run_result_success(output) {
        return None;
    }
    output
        .get("results")
        .and_then(serde_json::Value::as_array)
        .and_then(|results| {
            results.iter().find_map(|result| {
                if result.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
                    result
                        .get("error")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| result.get("output").and_then(serde_json::Value::as_str))
                        .map(ToString::to_string)
                } else {
                    None
                }
            })
        })
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
    arguments: &serde_json::Value,
    result: &serde_json::Value,
) -> bool {
    let mut changed = false;
    if tool_name == COMMAND_RUN_TOOL || tool_name == MULTIPLE_TASKS_TOOL {
        let plan = if tool_name == MULTIPLE_TASKS_TOOL && result.get("steps").is_some() {
            Some(result.clone())
        } else {
            multiple_tasks_output_from_tool_result(result)
        };
        if let Some(plan) = plan {
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
                .filter_map(|(index, step)| task_step_from_multiple_tasks_step(index, step))
                .collect();
            if let Some(first) = session.task_plan.detailed_tasks.first_mut() {
                first.status = TaskStatus::InProgress;
            }
            changed = true;
        }
    }
    if tool_name == COMMAND_RUN_TOOL && command_run_contains_task_delivered(result) {
        changed |= complete_active_task(session);
    }
    if tool_name == TASK_DELIVERED_TOOL && task_delivered_arguments_true(arguments) {
        changed |= complete_active_task(session);
    }
    if changed {
        session.session_last_update_at = Utc::now();
    }
    changed
}

fn task_delivered_arguments_true(arguments: &serde_json::Value) -> bool {
    arguments
        .get("task_delivered")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

fn command_run_contains_task_delivered(result: &serde_json::Value) -> bool {
    result
        .get("results")
        .and_then(|value| value.as_array())
        .map(|items| {
            items.iter().any(|item| {
                item.get("success").and_then(serde_json::Value::as_bool) == Some(true)
                    && item
                        .get("command_type")
                        .or_else(|| item.get("command"))
                        .and_then(serde_json::Value::as_str)
                        == Some(TASK_DELIVERED_TOOL)
            })
        })
        .unwrap_or(false)
}

fn complete_active_task(session: &mut SessionManagement) -> bool {
    if let Some(current) = session
        .task_plan
        .detailed_tasks
        .iter_mut()
        .find(|task| matches!(task.status, TaskStatus::InProgress | TaskStatus::Pending))
    {
        current.status = TaskStatus::Completed;
        if let Some(next) = session
            .task_plan
            .detailed_tasks
            .iter_mut()
            .find(|task| task.status == TaskStatus::Pending)
        {
            next.status = TaskStatus::InProgress;
        }
        return true;
    }
    false
}

fn multiple_tasks_output_from_tool_result(result: &serde_json::Value) -> Option<serde_json::Value> {
    result
        .get("results")
        .and_then(|value| value.as_array())
        .and_then(|items| {
            items.iter().find_map(|item| {
                (item
                    .get("command_type")
                    .or_else(|| item.get("command"))
                    .and_then(|value| value.as_str())
                    == Some("multiple_tasks"))
                .then(|| item.get("output").cloned())
                .flatten()
            })
        })
        .filter(|value| value.get("steps").is_some())
}

fn task_step_from_multiple_tasks_step(index: usize, value: &serde_json::Value) -> Option<TaskStep> {
    let object = value.as_object()?;
    let task_instruction = object
        .get("task_instruction")
        .or_else(|| object.get("deliverble"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let step_goal = object
        .get("step_goal")
        .or_else(|| object.get("task_summary"))
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
            .or_else(|| object.get("deliverble"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        step_deliverable_path: std::path::PathBuf::new(),
    })
}
