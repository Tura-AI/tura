//! Tool-driven task status and planning state helpers.

use chrono::Utc;
use uuid::Uuid;

use crate::prompt_style::runtime_prompt_manual;
use crate::state_machine::session_management::SessionManagement;
use lifecycle::{PlanStatus, StartCondition, TaskStep};

const COMMAND_RUN_TOOL: &str = "command_run";
const PLANNING_TOOL: &str = "planning";
const TASK_STATUS_COMMAND: &str = "task_status";

pub(crate) fn apply_tool_result_session_state_update(
    session: &mut SessionManagement,
    tool_name: &str,
    result: &mut serde_json::Value,
) -> bool {
    let mut changed = false;
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
        let requested_group = status
            .get("task_group")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        if let Some(group) = requested_group {
            changed |= apply_task_group(session, group);
        }
        if let Some(task_type) = status.get("task_type") {
            let task_type = runtime_prompt_manual::task_type_ids_from_value(task_type);
            let before = session.task_type.clone();
            session.replace_task_type(task_type);
            if session.task_type != before {
                changed = true;
            }
            if !session.no_op_manual && !session.op_manual_enabled {
                session.op_manual_enabled = true;
                changed = true;
            }
            changed |= runtime_prompt_manual::append_missing_runtime_prompt_manuals(session, None)
                .unwrap_or(false);
        }
        match status.get("status").and_then(serde_json::Value::as_str) {
            Some("doing") => changed |= mark_active_task_doing(session),
            Some("done") => changed |= complete_active_task(session),
            Some("question") => changed |= question_active_task(session),
            _ => {}
        }
    }
    changed
}

fn apply_task_group(session: &mut SessionManagement, group: String) -> bool {
    let mut changed = false;
    if session.auto_session_name && session.session_name.trim() != group.trim() {
        session.session_name = group.clone();
        changed = true;
    }
    if session.task_plan.plan_summary.trim() != group.trim() {
        session.task_plan.plan_summary = group.clone();
        changed = true;
    }
    if session.task_plan.detailed_tasks.is_empty() {
        ensure_single_task(session, Utc::now());
    }
    if let Some(task) = session.task_plan.detailed_tasks.iter_mut().find(|task| {
        matches!(
            task.status,
            PlanStatus::Doing | PlanStatus::Todo | PlanStatus::Question
        )
    }) {
        if task.task_summary.trim() != group.trim() {
            task.task_summary = group;
            changed = true;
        }
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

fn mark_active_task_doing(session: &mut SessionManagement) -> bool {
    ensure_single_task(session, Utc::now());
    if let Some(current) = session.task_plan.detailed_tasks.iter_mut().find(|task| {
        matches!(
            task.status,
            PlanStatus::Doing | PlanStatus::Todo | PlanStatus::Question
        )
    }) {
        if current.status == PlanStatus::Doing {
            return false;
        }
        current.status = PlanStatus::Doing;
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
        step_goal
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
    use super::apply_tool_result_session_state_update;
    const COMMAND_RUN_TOOL: &str = "command_run";
    use crate::context::compact_session_context;
    use crate::prompt_style::runtime_prompt_manual::RUNTIME_PROMPT_MANUAL_RECORD_TYPE;
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
                        "task_group": "商城前端",
                        "status": "done"
                    }
                }
            }]
        });

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(session.task_plan.plan_summary, "商城前端");
        assert_eq!(session.session_name, "商城前端");
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
                        "task_group": "Rust CLI rebuild",
                        "status": "done"
                    }
                }
            }]
        });

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(session.task_plan.detailed_tasks[0].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Todo);
    }

    #[test]
    fn task_group_does_not_rename_session_when_auto_name_disabled() {
        let mut session = session();
        session.session_name = "Manual title".to_string();
        session.auto_session_name = false;
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "status": {
                        "task_group": "订单清结算微服务"
                    }
                }
            }]
        });

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(session.task_plan.plan_summary, "订单清结算微服务");
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

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(
            session.task_plan.detailed_tasks[0].status,
            PlanStatus::Question
        );
        assert_eq!(session.task_plan.plan_summary, "Fix startup crash");
    }

    #[test]
    fn status_doing_marks_current_task_doing() {
        let mut session = session();
        session.task_plan.plan_summary = "Continue implementation".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "nonce-1".to_string(),
            step: 1,
            task_summary: "Continue implementation".to_string(),
            status: PlanStatus::Todo,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "status": "doing"
                    }
                }
            }]
        });

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(
            session.task_plan.detailed_tasks[0].status,
            PlanStatus::Doing
        );
    }

    #[test]
    fn task_type_injects_manuals_without_goal_mode_and_compact_rebuilds_current() {
        let mut session = session();
        assert!(!session.goal_mode);
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_type": ["debug", "frontend"]
                    }
                }
            }]
        });

        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &mut result,
        ));
        assert_eq!(session.task_type, vec!["debug", "visual", "frontend"]);
        assert_eq!(
            runtime_prompt_manual_log_ids(&session),
            vec!["debug", "visual", "frontend"]
        );
        let original_manual_positions = runtime_prompt_manual_log_positions(&session);
        let mut repeated = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_type": ["debug", "frontend"]
                    }
                }
            }]
        });

        assert!(!apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &mut repeated,
        ));
        assert_eq!(
            runtime_prompt_manual_log_ids(&session),
            vec!["debug", "visual", "frontend"]
        );
        assert_eq!(
            runtime_prompt_manual_log_positions(&session),
            original_manual_positions
        );

        let mut expanded = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_type": ["debug", "frontend", "interactive_and_3d"]
                    }
                }
            }]
        });

        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &mut expanded,
        ));
        assert_eq!(
            runtime_prompt_manual_log_ids(&session),
            vec!["debug", "visual", "frontend", "interactive_and_3d"]
        );
        let expanded_manual_positions = runtime_prompt_manual_log_positions(&session);
        assert_eq!(
            expanded_manual_positions
                .iter()
                .take(original_manual_positions.len())
                .cloned()
                .collect::<Vec<_>>(),
            original_manual_positions
        );

        let mut updated = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_type": ["frontend"]
                    }
                }
            }]
        });

        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &mut updated,
        ));
        assert_eq!(session.task_type, vec!["visual", "frontend"]);
        assert_eq!(
            runtime_prompt_manual_log_ids(&session),
            vec!["debug", "visual", "frontend", "interactive_and_3d"]
        );

        compact_session_context(&mut session, "handoff").expect("compact should succeed");

        assert_eq!(
            runtime_prompt_manual_log_ids_since_last_compact(&session),
            vec!["visual", "frontend"]
        );
    }

    #[test]
    fn task_type_repairs_missing_manual_when_state_already_contains_type() {
        let mut session = session();
        assert!(!session.goal_mode);
        session.task_type = vec!["visual".to_string()];
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_type": ["visual"]
                    }
                }
            }]
        });

        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &mut result,
        ));
        assert_eq!(session.task_type, vec!["visual"]);
        assert_eq!(runtime_prompt_manual_log_ids(&session), vec!["visual"]);
    }

    #[test]
    fn task_type_from_status_enables_manual_injection_for_direct_agents() {
        let mut session = session();
        session.op_manual_enabled = false;
        assert!(!session.no_op_manual);
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_type": ["frontend", "visual"]
                    }
                }
            }]
        });

        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &mut result,
        ));

        assert!(session.op_manual_enabled);
        assert_eq!(session.task_type, vec!["visual", "frontend"]);
        assert_eq!(
            runtime_prompt_manual_log_ids(&session),
            vec!["visual", "frontend"]
        );
    }

    #[test]
    fn task_type_from_status_respects_no_op_manual() {
        let mut session = session();
        session.op_manual_enabled = false;
        session.no_op_manual = true;
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_type": ["visual"]
                    }
                }
            }]
        });

        assert!(apply_tool_result_session_state_update(
            &mut session,
            COMMAND_RUN_TOOL,
            &mut result,
        ));

        assert!(!session.op_manual_enabled);
        assert_eq!(session.task_type, vec!["visual"]);
        assert!(runtime_prompt_manual_log_ids(&session).is_empty());
    }

    #[test]
    fn task_group_refreshes_auto_session_name_after_summary_exists() {
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
                        "task_group": "pdf编辑制作"
                    }
                }
            }]
        });

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(session.task_plan.plan_summary, "pdf编辑制作");
        assert_eq!(session.session_name, "pdf编辑制作");
        assert_eq!(
            session.task_plan.detailed_tasks[0].task_summary,
            "pdf编辑制作"
        );
        assert!(
            result["results"][0]["output"]["status"]
                .get("warning")
                .is_none(),
            "task_group refresh should not emit a warning: {result}"
        );
    }

    #[test]
    fn task_group_update_reaches_fsm_when_task_type_already_exists() {
        let mut session = session();
        session.auto_session_name = false;
        session.session_name = "Manual title".to_string();
        session.task_type = vec!["debug".to_string()];
        session.task_plan.plan_summary = "Previous runtime work".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "nonce-1".to_string(),
            step: 1,
            task_summary: "Previous runtime work".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_group": "runtime message handling",
                        "task_type": ["debug"],
                        "status": "doing"
                    }
                }
            }]
        });

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(session.session_name, "Manual title");
        assert_eq!(session.task_type, vec!["debug"]);
        assert_eq!(session.task_plan.plan_summary, "runtime message handling");
        assert_eq!(
            session.task_plan.detailed_tasks[0].task_summary,
            "runtime message handling"
        );
    }

    #[test]
    fn task_group_refresh_does_not_block_incremental_task_type_manuals_and_capabilities() {
        let mut session = session();
        session.task_plan.plan_summary = "Existing visual work".to_string();
        session.session_name = "Existing visual work".to_string();
        session.task_type = vec!["visual".to_string()];
        assert!(
            crate::prompt_style::runtime_prompt_manual::append_missing_runtime_prompt_manuals(
                &mut session,
                None,
            )
            .expect("visual manual should append")
        );
        assert!(session.has_session_capability("read_media"));
        assert!(!session.has_session_capability("apply_patch"));
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "nonce-1".to_string(),
            step: 1,
            task_summary: "Existing visual work".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut result = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": {
                    "task_status": {
                        "task_group": "frontend polish",
                        "task_type": ["frontend"]
                    }
                }
            }]
        });

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(session.session_name, "frontend polish");
        assert_eq!(session.task_plan.plan_summary, "frontend polish");
        assert_eq!(
            session.task_plan.detailed_tasks[0].task_summary,
            "frontend polish"
        );
        assert_eq!(session.task_type, vec!["visual", "frontend"]);
        assert_eq!(
            runtime_prompt_manual_log_ids(&session),
            vec!["visual", "frontend"]
        );
        assert!(session.has_session_capability("apply_patch"));
        assert!(session.has_session_capability(code_tools::commands::active_shell_command_name()));
        assert!(
            result["results"][0]["output"]["task_status"]
                .get("warning")
                .is_none(),
            "task_group + task_type update should not emit a warning: {result}"
        );
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

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

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

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

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

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

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

        let changed =
            apply_tool_result_session_state_update(&mut session, COMMAND_RUN_TOOL, &mut result);

        assert!(changed);
        assert_eq!(session.task_plan.detailed_tasks[0].status, PlanStatus::Done);
        assert_eq!(session.task_plan.detailed_tasks[1].status, PlanStatus::Todo);
    }

    fn runtime_prompt_manual_log_ids(session: &SessionManagement) -> Vec<String> {
        session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .filter(|value| {
                value.get("type").and_then(serde_json::Value::as_str)
                    == Some(RUNTIME_PROMPT_MANUAL_RECORD_TYPE)
            })
            .filter_map(|value| {
                value
                    .get("task_type")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
            })
            .collect()
    }

    fn runtime_prompt_manual_log_positions(session: &SessionManagement) -> Vec<usize> {
        session
            .session_log
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let value = serde_json::from_str::<serde_json::Value>(entry).ok()?;
                (value.get("type").and_then(serde_json::Value::as_str)
                    == Some(RUNTIME_PROMPT_MANUAL_RECORD_TYPE))
                .then_some(index)
            })
            .collect()
    }

    fn runtime_prompt_manual_log_ids_since_last_compact(
        session: &SessionManagement,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        for entry in session.session_log.iter().rev() {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(entry) else {
                continue;
            };
            if value.get("type").and_then(serde_json::Value::as_str) == Some("context_compaction") {
                break;
            }
            if value.get("type").and_then(serde_json::Value::as_str)
                == Some(RUNTIME_PROMPT_MANUAL_RECORD_TYPE)
            {
                if let Some(id) = value.get("task_type").and_then(serde_json::Value::as_str) {
                    ids.push(id.to_string());
                }
            }
        }
        ids.reverse();
        ids
    }
}
