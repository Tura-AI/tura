use chrono::Utc;
use tracing::error;
use uuid::Uuid;

use crate::runtime::types::ToolCallData;
use crate::state_machine::agent_management::AgentManagement;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::{
    PlanStatus, SessionManagement, StartCondition, TaskStep,
};
use crate::tool_router::execute_tool::{execute_tool, ExecuteToolInput, ToolExecutionResult};

use super::change_tracker::{append_successful_changes, capture_pending_changes};
use super::constants::{COMMAND_RUN_TOOL, PLANNING_TOOL, TASK_STATUS_COMMAND};
use super::gateway_events::{
    publish_step_summary, publish_task_plan_todos, publish_tool_call_record,
    publish_tool_call_started,
};
use super::permission_gate::{permission_denial_for_tool, request_command_run_sandbox_bypass};
use super::tool_arguments::normalize_tool_arguments_for_tool;
use super::tool_catalog::{command_run_commands_for_agent, project_directory_with_tools};

pub(super) fn execute_tool_calls(
    tool_calls: &[ToolCallData],
    agent: Option<&AgentManagement>,
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
        let has_streamed_command_run_result = tool_call.tool_name == COMMAND_RUN_TOOL
            && streamed_command_run_result(runtime).is_some();
        if !has_streamed_command_run_result {
            publish_tool_call_started(
                session,
                runtime,
                tool_call,
                normalized_arguments.clone(),
                tool_started_at,
            );
            publish_step_summary(session, runtime, tool_call);
        }
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
                let mut streamed_result = streamed_result;
                apply_task_attribution_to_streamed_result(session, &mut streamed_result);
                record_streamed_command_events(session, runtime, &streamed_result);
                let streamed_arguments =
                    streamed_command_run_arguments(&normalized_arguments, &streamed_result);
                let execution_result = ToolExecutionResult {
                    tool_name: tool_call.tool_name.clone(),
                    arguments: streamed_arguments.clone(),
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
                    streamed_arguments,
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
            allowed_command_run_commands: agent.map(command_run_commands_for_agent),
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

fn streamed_command_run_arguments(
    fallback: &serde_json::Value,
    streamed_result: &serde_json::Value,
) -> serde_json::Value {
    streamed_result
        .get("commands")
        .filter(|commands| commands.as_array().is_some_and(|items| !items.is_empty()))
        .map(|commands| {
            serde_json::json!({
                "commands": commands,
            })
        })
        .unwrap_or_else(|| fallback.clone())
}

fn apply_task_attribution_to_streamed_result(
    session: &SessionManagement,
    streamed_result: &mut serde_json::Value,
) {
    let Some(attribution) = current_task_attribution(session) else {
        return;
    };
    if let Some(object) = streamed_result.as_object_mut() {
        object.insert("task_attribution".to_string(), attribution.clone());
        if let Some(events) = object
            .get_mut("command_events")
            .and_then(serde_json::Value::as_array_mut)
        {
            for event in events {
                if let Some(event_object) = event.as_object_mut() {
                    event_object
                        .entry("task_attribution".to_string())
                        .or_insert_with(|| attribution.clone());
                }
            }
        }
    }
}

fn current_task_attribution(session: &SessionManagement) -> Option<serde_json::Value> {
    session
        .task_plan
        .detailed_tasks
        .iter()
        .find(|task| {
            matches!(task.status, PlanStatus::Doing)
                || (task.status == PlanStatus::Todo
                    && task.start_condition == StartCondition::UserAction)
        })
        .map(|task| {
            serde_json::json!({
                "task_id": task.task_id,
                "step": task.step,
                "task_summary": task.task_summary,
                "deliverable": task.step_deliverable_description,
                "status": task.status,
            })
        })
}

fn record_streamed_command_events(
    session: &mut SessionManagement,
    runtime: &RuntimeManagement,
    streamed_result: &serde_json::Value,
) {
    let Some(events) = streamed_result
        .get("command_events")
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    let now = Utc::now();
    for (index, event) in events.iter().enumerate() {
        let mut event = event.clone();
        if !event.is_object() {
            event = serde_json::json!({ "value": event });
        }
        let object = event
            .as_object_mut()
            .expect("streamed command event normalized to object");
        object.insert(
            "type".to_string(),
            serde_json::Value::String("streamed_command_event".to_string()),
        );
        object
            .entry("runtime_id".to_string())
            .or_insert_with(|| serde_json::Value::String(runtime.runtime_id.clone()));
        object
            .entry("session_id".to_string())
            .or_insert_with(|| serde_json::Value::String(session.session_id.clone()));
        object
            .entry("event_index".to_string())
            .or_insert_with(|| serde_json::Value::Number(index.into()));
        object
            .entry("timestamp".to_string())
            .or_insert_with(|| serde_json::Value::String(now.to_rfc3339()));
        session.push_log(event.to_string(), now);
    }
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
    if tool_name == COMMAND_RUN_TOOL {
        changed |= apply_status_result(session, result);
    }
    if tool_name == COMMAND_RUN_TOOL || tool_name == PLANNING_TOOL {
        let plan = if tool_name == PLANNING_TOOL && result.get("steps").is_some() {
            Some(result.clone())
        } else {
            planning_output_from_tool_result(result)
        };
        if let Some(plan) = plan {
            if let Some(user_task) = plan
                .get("user_task")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
            {
                session.task_plan.plan_summary = user_task.to_string();
            } else if session.task_plan.plan_summary.trim().is_empty() {
                session.task_plan.plan_summary = session.user_goal.clone();
            }
            let steps = plan
                .get("steps")
                .and_then(|value| value.as_array())
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let previous_tasks = task_plan_snapshot(session);
            replace_active_task_with_planning(session, steps);
            if session.auto_session_name {
                if let Some(summary) = session
                    .task_plan
                    .detailed_tasks
                    .iter()
                    .rev()
                    .find_map(|task| non_empty_string(&task.task_summary))
                {
                    session.session_name = summary;
                }
            }
            activate_first_planned_task_if_needed(session);
            record_task_topology_applied(session, steps, previous_tasks);
            changed = true;
        }
    }
    if changed {
        session.session_last_update_at = Utc::now();
    }
    changed
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
            if output.get(TASK_STATUS_COMMAND).is_some() {
                output
                    .get_mut(TASK_STATUS_COMMAND)
                    .and_then(serde_json::Value::as_object_mut)
            } else {
                output
                    .get_mut("status")
                    .and_then(serde_json::Value::as_object_mut)
            }
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
    let mut changed = false;
    if session.auto_session_name && session.session_name.trim() != summary.trim() {
        session.session_name = summary.clone();
        changed = true;
    }
    if session.task_plan.plan_summary.trim().is_empty() {
        session.task_plan.plan_summary = summary.clone();
        ensure_single_task(session, Utc::now());
        if let Some(task) = session.task_plan.detailed_tasks.first_mut() {
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
    changed
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
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
        task_id: random_task_id(),
        step: 1,
        start_at: now,
        task_summary: summary.clone(),
        step_deliverable_description: summary,
        status: PlanStatus::Doing,
        ..TaskStep::default()
    });
}

fn replace_active_task_with_planning(session: &mut SessionManagement, steps: &[serde_json::Value]) {
    let mut incoming = steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| task_step_from_planning_step(index, step))
        .collect::<Vec<_>>();
    if incoming.is_empty() {
        return;
    }

    for task in &mut incoming {
        if task.status == PlanStatus::default() {
            task.status = PlanStatus::Todo;
        }
    }

    let replace_index = session
        .task_plan
        .detailed_tasks
        .iter()
        .position(|task| matches!(task.status, PlanStatus::Doing | PlanStatus::Question))
        .or_else(|| {
            session
                .task_plan
                .detailed_tasks
                .iter()
                .position(|task| task.status == PlanStatus::Todo)
        });

    match replace_index {
        Some(index) => {
            session
                .task_plan
                .detailed_tasks
                .splice(index..=index, incoming);
        }
        None => session.task_plan.detailed_tasks.extend(incoming),
    }

    renumber_task_steps(&mut session.task_plan.detailed_tasks);
}

fn activate_first_planned_task_if_needed(session: &mut SessionManagement) {
    let has_active = session
        .task_plan
        .detailed_tasks
        .iter()
        .any(|task| matches!(task.status, PlanStatus::Doing | PlanStatus::Question));
    if has_active {
        return;
    }
    if let Some(first_todo) = session.task_plan.detailed_tasks.iter_mut().find(|task| {
        task.status == PlanStatus::Todo && task.start_condition == StartCondition::UserAction
    }) {
        first_todo.status = PlanStatus::Doing;
    }
}

fn renumber_task_steps(tasks: &mut [TaskStep]) {
    for (index, task) in tasks.iter_mut().enumerate() {
        task.step = (index + 1) as u64;
    }
}

fn record_task_topology_applied(
    session: &mut SessionManagement,
    steps: &[serde_json::Value],
    previous_tasks: serde_json::Value,
) {
    let now = Utc::now();
    session.push_log(
        serde_json::json!({
            "type": "task_topology_applied",
            "input_steps": steps,
            "previous_tasks": previous_tasks,
            "current_tasks": task_plan_snapshot(session),
            "timestamp": now.to_rfc3339(),
        })
        .to_string(),
        now,
    );
}

fn task_plan_snapshot(session: &SessionManagement) -> serde_json::Value {
    serde_json::Value::Array(
        session
            .task_plan
            .detailed_tasks
            .iter()
            .map(|task| {
                serde_json::json!({
                    "task_id": task.task_id,
                    "step": task.step,
                    "task_summary": task.task_summary,
                    "deliverable": task.step_deliverable_description,
                    "status": task.status,
                    "start_condition": task.start_condition,
                })
            })
            .collect(),
    )
}

fn planning_output_from_tool_result(result: &serde_json::Value) -> Option<serde_json::Value> {
    result
        .get("results")
        .and_then(|value| value.as_array())
        .and_then(|items| {
            items.iter().find_map(|item| {
                (item
                    .get("command_type")
                    .or_else(|| item.get("command"))
                    .and_then(|value| value.as_str())
                    == Some("planning"))
                .then(|| item.get("output").cloned())
                .flatten()
            })
        })
        .filter(|value| value.get("steps").is_some())
}

fn task_step_from_planning_step(index: usize, value: &serde_json::Value) -> Option<TaskStep> {
    let object = value.as_object()?;
    let task_instruction = object
        .get("task_instruction")
        .or_else(|| object.get("deliverable"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let step_goal = object
        .get("step_goal")
        .or_else(|| object.get("task_summary"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let task_summary = if !step_goal.trim().is_empty() {
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
        task_id: random_task_id(),
        step: object
            .get("step")
            .and_then(|value| value.as_u64())
            .unwrap_or((index + 1) as u64),
        status,
        task_summary,
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
        ..TaskStep::default()
    })
}

fn random_task_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        apply_task_attribution_to_streamed_result, apply_tool_result_session_state_update,
        record_streamed_command_events, COMMAND_RUN_TOOL,
    };
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
    use crate::state_machine::session_management::{
        PlanStatus, SessionInput, SessionManagement, StartCondition, TaskStep,
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
                planning_mode_override: None,
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
                "command_type": "task_status",
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
        assert_eq!(session.session_name, "Fix startup crash");
        assert_eq!(session.task_plan.detailed_tasks.len(), 1);
        assert_eq!(session.task_plan.detailed_tasks[0].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[0].step, 1);
        assert!(!session.task_plan.detailed_tasks[0].task_id.is_empty());
    }

    #[test]
    fn task_status_output_marks_current_planned_task_done() {
        let mut session = session();
        session.task_plan.plan_summary = "ProgramBench rebuild".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "inspect-contract".to_string(),
            step: 1,
            task_summary: "Inspect available behavior clues".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "rebuild-source".to_string(),
            step: 2,
            task_summary: "Recreate Rust implementation".to_string(),
            status: PlanStatus::Todo,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_summary": "Inspect available behavior clues",
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
        assert_eq!(session.task_plan.detailed_tasks[0].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Todo);
    }

    #[test]
    fn task_summary_does_not_rename_session_when_auto_name_disabled() {
        let mut session = session();
        session.session_name = "Manual title".to_string();
        session.auto_session_name = false;
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "status": {
                        "task_summary": "Generated task"
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
        assert_eq!(session.task_plan.plan_summary, "Generated task");
        assert_eq!(session.session_name, "Manual title");
    }

    #[test]
    fn status_question_marks_current_task_question() {
        let mut session = session();
        session.task_plan.plan_summary = "Fix startup crash".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "nonce-1".to_string(),
            step: 1,
            task_summary: "Fix startup crash".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
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
    fn status_summary_refreshes_auto_session_name_after_summary_exists() {
        let mut session = session();
        session.task_plan.plan_summary = "Existing task".to_string();
        session.session_name = "Existing task".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "nonce-1".to_string(),
            step: 1,
            task_summary: "Existing task".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
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

        assert!(changed);
        assert_eq!(session.task_plan.plan_summary, "Existing task");
        assert_eq!(session.session_name, "New task name");
        assert_eq!(
            session.task_plan.detailed_tasks[0].task_summary,
            "Existing task"
        );
        assert!(result["results"][0]["output"]["status"]["warning"]
            .as_str()
            .is_some_and(|text| text.contains("rename ignored")));
    }

    #[test]
    fn planning_plan_uses_unique_sequential_steps() {
        let mut session = session();
        let mut result = json!({
            "results": [{
                "command_type": "planning",
                "success": true,
                "output": {
                    "steps": [
                        {
                            "step": 1,
                            "task_summary": "Server module"
                        },
                        {
                            "step": 1,
                            "task_summary": "Frontend module"
                        },
                        {
                            "step": 2,
                            "task_summary": "E2E acceptance"
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
        assert_eq!(session.task_plan.detailed_tasks[1].step, 2);
        assert_eq!(session.task_plan.detailed_tasks[2].step, 3);
        assert_eq!(
            session.task_plan.detailed_tasks[0].step_deliverable_description,
            ""
        );
        let topology_event = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .find(|value| {
                value.get("type").and_then(serde_json::Value::as_str)
                    == Some("task_topology_applied")
            })
            .expect("planning topology should be audited");
        assert_eq!(
            topology_event["current_tasks"]
                .as_array()
                .expect("current tasks")
                .len(),
            3
        );
        assert_eq!(
            topology_event["input_steps"]
                .as_array()
                .expect("input steps")
                .len(),
            3
        );
    }

    #[test]
    fn planning_replaces_active_task_and_preserves_queued_tail() {
        let mut session = session();
        session.task_plan.plan_summary = "Existing task".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "a".to_string(),
            step: 1,
            task_summary: "Heavy task".to_string(),
            step_deliverable_description: "Heavy delivery".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "b".to_string(),
            step: 2,
            task_summary: "Queued b".to_string(),
            status: PlanStatus::Todo,
            ..TaskStep::default()
        });
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "c".to_string(),
            step: 3,
            task_summary: "Queued c".to_string(),
            status: PlanStatus::Todo,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "planning",
                "success": true,
                "output": {
                    "steps": [
                        {
                            "step": 1,
                            "task_summary": "Subtask aa"
                        },
                        {
                            "step": 2,
                            "task_summary": "Subtask ab"
                        },
                        {
                            "step": 2,
                            "task_summary": "Subtask ac"
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
        assert_eq!(session.task_plan.plan_summary, "Existing task");
        for task in session.task_plan.detailed_tasks.iter().take(3) {
            assert_eq!(task.task_id.len(), 8);
            assert!(task.task_id.chars().all(|ch| ch.is_ascii_hexdigit()));
        }
        assert_eq!(session.task_plan.detailed_tasks[3].task_id, "b");
        assert_eq!(session.task_plan.detailed_tasks[4].task_id, "c");
        assert_eq!(
            session
                .task_plan
                .detailed_tasks
                .iter()
                .map(|task| task.step)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        assert_eq!(
            session.task_plan.detailed_tasks[0].status,
            PlanStatus::Doing
        );
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Todo);
        assert_eq!(session.task_plan.detailed_tasks[3].task_summary, "Queued b");
        assert_eq!(result["results"][0]["success"], true);

        let mut done_aa = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "status": "done"
                    }
                }
            }]
        });
        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut done_aa,
        ));
        assert_eq!(session.task_plan.detailed_tasks[0].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Todo);

        let mut done_ab = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "status": "done"
                    }
                }
            }]
        });
        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut done_ab,
        ));
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[2].status, PlanStatus::Todo);

        let mut done_ac = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "status": "done"
                    }
                }
            }]
        });
        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut done_ac,
        ));
        assert_eq!(session.task_plan.detailed_tasks[2].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[3].status, PlanStatus::Todo);
    }

    #[test]
    fn command_run_applies_status_before_later_planning_topology() {
        let mut session = session();
        session.task_plan.plan_summary = "Existing task".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "active".to_string(),
            step: 1,
            task_summary: "Active task".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [
                {
                    "command_type": "task_status",
                    "success": true,
                    "output": {
                        "task_status": {
                            "status": "question"
                        }
                    }
                },
                {
                    "command_type": "planning",
                    "success": true,
                    "output": {
                        "steps": [
                            {
                                "step": 1,
                                "task_summary": "First new task"
                            },
                            {
                                "step": 2,
                                "task_summary": "Second new task"
                            }
                        ]
                    }
                }
            ]
        });

        let changed = apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &json!({}),
            &mut result,
        );

        assert!(changed);
        assert_eq!(session.task_plan.detailed_tasks.len(), 2);
        assert_eq!(
            session.task_plan.detailed_tasks[0].task_summary,
            "First new task"
        );
        assert_eq!(
            session.task_plan.detailed_tasks[0].status,
            PlanStatus::Doing
        );
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Todo);
    }

    #[test]
    fn streamed_command_events_are_audited_with_active_task_attribution() {
        let mut session = session();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "task-aa".to_string(),
            step: 7,
            task_summary: "Inspect behavior".to_string(),
            step_deliverable_description: "Read source and fixtures.".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut streamed_result = json!({
            "commands": [{
                "step": 1,
                "command_type": "shell_command",
                "command_line": "rg zip_utils rust-reference/src"
            }],
            "command_events": [
                {
                    "status": "ready",
                    "provider_tool_call_id": "call_provider_1",
                    "command_index": 0,
                    "step": 1,
                    "command_type": "shell_command"
                },
                {
                    "status": "completed",
                    "result_index": 0,
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true
                }
            ],
            "results": [{
                "step": 1,
                "command_type": "shell_command",
                "success": true,
                "output": "ok"
            }]
        });
        let now = Utc::now();
        let runtime = RuntimeManagement::new(
            "runtime-streamed".to_string(),
            session.session_id.clone(),
            session.session_id.clone(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "provider".to_string(),
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: false,
                provider_name: "provider".to_string(),
                model_name: "model".to_string(),
                provider_url_name: "provider".to_string(),
                llm_provider_name: "provider".to_string(),
            },
            now,
        );

        apply_task_attribution_to_streamed_result(&session, &mut streamed_result);
        record_streamed_command_events(&mut session, &runtime, &streamed_result);

        assert_eq!(
            streamed_result["task_attribution"]["task_id"],
            json!("task-aa")
        );
        let events = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .filter(|value| {
                value.get("type").and_then(serde_json::Value::as_str)
                    == Some("streamed_command_event")
            })
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["provider_tool_call_id"], "call_provider_1");
        assert_eq!(events[0]["task_attribution"]["task_id"], "task-aa");
        assert_eq!(events[1]["task_attribution"]["step"], 7);
    }

    #[test]
    fn done_does_not_auto_activate_scheduled_or_polling_task() {
        let mut session = session();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "current".to_string(),
            step: 1,
            task_summary: "Current".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "scheduled".to_string(),
            step: 2,
            task_summary: "Scheduled".to_string(),
            status: PlanStatus::Todo,
            start_condition: StartCondition::ScheduledTask,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
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
        assert_eq!(session.task_plan.detailed_tasks[0].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Todo);
    }
}
