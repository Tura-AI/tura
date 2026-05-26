use chrono::Utc;
use tracing::error;

use crate::runtime::types::ToolCallData;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::{PlanStatus, SessionManagement, TaskStep};
use crate::tool_router::execute_tool::{execute_tool, ExecuteToolInput, ToolExecutionResult};

use super::change_tracker::{append_successful_changes, capture_pending_changes};
use super::constants::{COMMAND_RUN_TOOL, MULTIPLE_TASKS_TOOL, TASK_STATUS_COMMAND};
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
                let mut execution_result = execution_result;
                if apply_tool_result_session_state_update(
                    session,
                    &tool_call.tool_name,
                    &execution_result.arguments,
                    &mut execution_result.result,
                ) {
                    publish_task_plan_todos(session);
                    execution_result.success = command_run_result_success(&execution_result.result);
                    execution_result.error = command_run_result_error(&execution_result.result);
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
            Ok(mut execution_result) => {
                if apply_tool_result_session_state_update(
                    session,
                    &tool_call.tool_name,
                    &execution_result.arguments,
                    &mut execution_result.result,
                ) {
                    publish_task_plan_todos(session);
                    execution_result.success = command_run_result_success(&execution_result.result);
                    execution_result.error = command_run_result_error(&execution_result.result);
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
    result: &mut serde_json::Value,
) -> bool {
    let mut changed = false;
    let _ = arguments;
    if tool_name == COMMAND_RUN_TOOL || tool_name == MULTIPLE_TASKS_TOOL {
        let plan = if tool_name == MULTIPLE_TASKS_TOOL && result.get("steps").is_some() {
            Some(result.clone())
        } else {
            multiple_tasks_output_from_tool_result(result)
        };
        if let Some(plan) = plan {
            if !session.task_plan.detailed_tasks.is_empty() {
                mark_multiple_tasks_update_rejected(result);
                return true;
            }
            session.task_plan.plan_summary = plan
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
                first.status = PlanStatus::Doing;
            }
            changed = true;
        }
    }
    if tool_name == COMMAND_RUN_TOOL {
        changed |= apply_status_result(session, result);
    }
    if changed {
        session.session_last_update_at = Utc::now();
    }
    changed
}

fn mark_multiple_tasks_update_rejected(result: &mut serde_json::Value) {
    if let Some(item) = result
        .get_mut("results")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|items| {
            items.iter_mut().find(|item| {
                item.get("command_type")
                    .or_else(|| item.get("command"))
                    .and_then(serde_json::Value::as_str)
                    == Some(MULTIPLE_TASKS_TOOL)
            })
        })
    {
        item["success"] = serde_json::Value::Bool(false);
        item["error"] = serde_json::Value::String(
            "planning state already exists; multiple_tasks update ignored unless the user clearly changes the task".to_string(),
        );
    }
}

fn apply_status_result(session: &mut SessionManagement, result: &mut serde_json::Value) -> bool {
    let Some(items) = result
        .get_mut("results")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };
    let mut changed = false;
    for item in items {
        if item.get("success").and_then(serde_json::Value::as_bool) != Some(true)
            || item
                .get("command_type")
                .or_else(|| item.get("command"))
                .and_then(serde_json::Value::as_str)
                != Some(TASK_STATUS_COMMAND)
        {
            continue;
        }
        let Some(status) = item.get_mut("output").and_then(|output| {
            output
                .get_mut("status")
                .and_then(serde_json::Value::as_object_mut)
        }) else {
            continue;
        };
        let requested_summary = status
            .get("task_summary")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        if let Some(summary) = requested_summary {
            changed |= apply_task_summary(session, status, summary);
        }
        match status.get("status").and_then(serde_json::Value::as_str) {
            Some("done") => changed |= complete_active_task(session),
            Some("question") => changed |= question_active_task(session),
            _ => {}
        }
    }
    changed
}

fn apply_task_summary(
    session: &mut SessionManagement,
    output: &mut serde_json::Map<String, serde_json::Value>,
    summary: String,
) -> bool {
    if session.task_plan.plan_summary.trim().is_empty() {
        session.task_plan.plan_summary = summary.clone();
        ensure_single_task(session, Utc::now());
        if let Some(task) = session.task_plan.detailed_tasks.first_mut() {
            task.task_name = summary.clone();
            task.task_summary = summary;
        }
        return true;
    }
    if session.task_plan.plan_summary.trim() != summary.trim() {
        output.insert(
            "warning".to_string(),
            serde_json::Value::String(
                "task_summary rename ignored because the task already has a name; no other task-management parameter needs updating for this rename".to_string(),
            ),
        );
    }
    false
}

fn complete_active_task(session: &mut SessionManagement) -> bool {
    ensure_single_task(session, Utc::now());
    if let Some(current) = session.task_plan.detailed_tasks.iter_mut().find(|task| {
        matches!(
            task.status,
            PlanStatus::Doing | PlanStatus::Todo | PlanStatus::Question
        )
    }) {
        current.status = PlanStatus::Done;
        if let Some(next) = session
            .task_plan
            .detailed_tasks
            .iter_mut()
            .find(|task| task.status == PlanStatus::Todo)
        {
            next.status = PlanStatus::Doing;
        }
        return true;
    }
    false
}

fn question_active_task(session: &mut SessionManagement) -> bool {
    ensure_single_task(session, Utc::now());
    if let Some(current) = session
        .task_plan
        .detailed_tasks
        .iter_mut()
        .find(|task| matches!(task.status, PlanStatus::Doing | PlanStatus::Todo))
    {
        current.status = PlanStatus::Question;
        return true;
    }
    false
}

fn ensure_single_task(session: &mut SessionManagement, now: chrono::DateTime<Utc>) {
    if !session.task_plan.detailed_tasks.is_empty() {
        return;
    }
    let summary = if session.task_plan.plan_summary.trim().is_empty() {
        session.user_goal.clone()
    } else {
        session.task_plan.plan_summary.clone()
    };
    session.task_plan.detailed_tasks.push(TaskStep {
        nonce_id: format!("{}:0", session.session_id),
        step: 0,
        start_at: now,
        task_name: summary.clone(),
        task_summary: summary.clone(),
        step_deliverable_description: summary,
        status: PlanStatus::Doing,
        ..TaskStep::default()
    });
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
        .or_else(|| object.get("delivery"))
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
        PlanStatus::Done
    } else if object.get("ok").and_then(|value| value.as_bool()) == Some(false) {
        PlanStatus::Archived
    } else {
        PlanStatus::Todo
    };
    Some(TaskStep {
        nonce_id: object
            .get("nonce_id")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("task-{index}")),
        step: object
            .get("step")
            .and_then(|value| value.as_u64())
            .unwrap_or(index as u64),
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
            .get("delivery")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        step_deliverable_path: std::path::PathBuf::new(),
        ..TaskStep::default()
    })
}

#[cfg(test)]
mod tests {
    use super::{apply_tool_result_session_state_update, COMMAND_RUN_TOOL};
    use crate::state_machine::session_management::{
        PlanStatus, SessionInput, SessionManagement, TaskStep,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::path::PathBuf;

    fn session() -> SessionManagement {
        let now = Utc::now();
        SessionManagement::new(
            "sess-task-status".to_string(),
            "task status".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "fix the task".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            "fix the task".to_string(),
            now,
        )
    }

    #[test]
    fn status_done_creates_single_task_and_marks_done() {
        let mut session = session();
        let mut result = json!({
            "results": [{
                "command_type": "status",
                "success": true,
                "output": {
                    "status": {
                        "task_summary": "Fix startup crash",
                        "status": "done"
                    }
                }
            }]
        });

        let changed = apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut result,
        );

        assert!(changed);
        assert_eq!(session.task_plan.plan_summary, "Fix startup crash");
        assert_eq!(session.task_plan.detailed_tasks.len(), 1);
        assert_eq!(session.task_plan.detailed_tasks[0].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[0].step, 0);
        assert!(!session.task_plan.detailed_tasks[0].nonce_id.is_empty());
    }

    #[test]
    fn status_question_marks_current_task_question() {
        let mut session = session();
        session.task_plan.plan_summary = "Fix startup crash".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            nonce_id: "nonce-1".to_string(),
            step: 0,
            task_summary: "Fix startup crash".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "status",
                "success": true,
                "output": {
                    "status": {
                        "status": "question"
                    }
                }
            }]
        });

        let changed = apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut result,
        );

        assert!(changed);
        assert_eq!(
            session.task_plan.detailed_tasks[0].status,
            PlanStatus::Question
        );
        assert_eq!(session.task_plan.plan_summary, "Fix startup crash");
    }

    #[test]
    fn status_rename_is_rejected_after_summary_exists() {
        let mut session = session();
        session.task_plan.plan_summary = "Existing task".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            nonce_id: "nonce-1".to_string(),
            step: 0,
            task_summary: "Existing task".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "status",
                "success": true,
                "output": {
                    "status": {
                        "task_summary": "New task name"
                    }
                }
            }]
        });

        let changed = apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut result,
        );

        assert!(!changed);
        assert_eq!(session.task_plan.plan_summary, "Existing task");
        assert_eq!(
            session.task_plan.detailed_tasks[0].task_summary,
            "Existing task"
        );
        assert!(result["results"][0]["output"]["status"]["warning"]
            .as_str()
            .is_some_and(|text| text.contains("rename ignored")));
    }

    #[test]
    fn multiple_tasks_plan_preserves_parallel_steps_and_delivery() {
        let mut session = session();
        let mut result = json!({
            "results": [{
                "command_type": "multiple_tasks",
                "success": true,
                "output": {
                    "steps": [
                        {
                            "nonce_id": "server",
                            "step": 1,
                            "task_summary": "Server module",
                            "delivery": "Read services/server/ACCEPTANCE.md and implement server requirements."
                        },
                        {
                            "nonce_id": "frontend",
                            "step": 1,
                            "task_summary": "Frontend module",
                            "delivery": "Read apps/frontend/ACCEPTANCE.md and implement frontend requirements."
                        },
                        {
                            "nonce_id": "e2e",
                            "step": 2,
                            "task_summary": "E2E acceptance",
                            "delivery": "Read docs/acceptance/E2E.md and validate the full flow."
                        }
                    ]
                }
            }]
        });

        let changed = apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut result,
        );

        assert!(changed);
        assert_eq!(session.task_plan.detailed_tasks.len(), 3);
        assert_eq!(
            session.task_plan.detailed_tasks[0].status,
            PlanStatus::Doing
        );
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Todo);
        assert_eq!(session.task_plan.detailed_tasks[0].step, 1);
        assert_eq!(session.task_plan.detailed_tasks[1].step, 1);
        assert_eq!(session.task_plan.detailed_tasks[2].step, 2);
        assert_eq!(
            session.task_plan.detailed_tasks[0].step_deliverable_description,
            "Read services/server/ACCEPTANCE.md and implement server requirements."
        );
    }

    #[test]
    fn multiple_tasks_update_is_rejected_when_plan_exists() {
        let mut session = session();
        session.task_plan.plan_summary = "Existing task".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            nonce_id: "nonce-1".to_string(),
            step: 0,
            task_summary: "Existing task".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "multiple_tasks",
                "success": true,
                "output": {
                    "steps": [
                        { "nonce_id": "new-1", "task_summary": "New one" },
                        { "nonce_id": "new-2", "task_summary": "New two" }
                    ]
                }
            }]
        });

        let changed = apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut result,
        );

        assert!(changed);
        assert_eq!(session.task_plan.plan_summary, "Existing task");
        assert_eq!(session.task_plan.detailed_tasks.len(), 1);
        assert_eq!(result["results"][0]["success"], false);
        assert!(result["results"][0]["error"]
            .as_str()
            .is_some_and(|text| text.contains("planning state already exists")));
    }
}
