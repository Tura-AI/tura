//! Task-progress helpers for the turn loop.

use chrono::Utc;

use crate::prompt_style::context_blocks;
use crate::state_machine::session_management::{SessionManagement, TaskStatus};

const COMMAND_RUN_TOOL: &str = "command_run";
const TASK_STATUS_COMMAND: &str = "task_status";

/// Inject the task_status nudge once this many consecutive command_run turns
/// have produced no write (`apply_patch`) and no task state change
/// (`task_status`).
pub(crate) const NO_WRITE_COMMAND_RUN_NUDGE_THRESHOLD: u64 = 3;

/// True if any command_run tool call in this turn includes a write command
/// (`apply_patch`) or a task-state command (`task_status`).
pub(crate) fn command_run_turn_has_write_or_status(
    tool_calls: &[crate::runtime::types::ToolCallData],
) -> bool {
    tool_calls.iter().any(|tool_call| {
        if tool_call.tool_name != COMMAND_RUN_TOOL {
            return false;
        }
        tool_call
            .arguments
            .get("commands")
            .and_then(|commands| commands.as_array())
            .is_some_and(|commands| {
                commands.iter().any(|command| {
                    let command_type = command
                        .get("command_type")
                        .or_else(|| command.get("command"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase()
                        .replace('-', "_");
                    matches!(command_type.as_str(), "apply_patch" | "task_status")
                })
            })
    })
}

pub(crate) fn command_run_result_terminal_task_status(
    result: &serde_json::Value,
) -> Option<String> {
    let result = result.get("streamed_command_run_result").unwrap_or(result);
    result
        .get("results")
        .and_then(|value| value.as_array())
        .and_then(|items| items.iter().find_map(command_run_item_terminal_task_status))
}

fn command_run_item_terminal_task_status(item: &serde_json::Value) -> Option<String> {
    let command_type = item
        .get("command_type")
        .or_else(|| item.get("command"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_");
    if command_type != TASK_STATUS_COMMAND {
        return None;
    }
    if item.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
        return None;
    }
    item.get("output")
        .and_then(|output| output.get("task_status"))
        .and_then(|status| status.get("status"))
        .and_then(serde_json::Value::as_str)
        .filter(|status| matches!(*status, "doing" | "done" | "question"))
        .map(ToString::to_string)
}

pub(crate) fn command_run_result_has_non_status_command(result: &serde_json::Value) -> bool {
    let result = result.get("streamed_command_run_result").unwrap_or(result);
    result
        .get("results")
        .and_then(|value| value.as_array())
        .is_some_and(|items| {
            items.iter().any(|item| {
                let command_type = item
                    .get("command_type")
                    .or_else(|| item.get("command"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase()
                    .replace('-', "_");
                !command_type.is_empty() && command_type != TASK_STATUS_COMMAND
            })
        })
}

pub(crate) fn active_task_user_message(session: &SessionManagement) -> Option<serde_json::Value> {
    task_user_message_by(session, task_is_executable)
}

#[cfg(test)]
pub(crate) fn active_todo_task_user_message(
    session: &SessionManagement,
) -> Option<serde_json::Value> {
    task_user_message_by(session, task_is_user_action_todo)
}

pub(crate) fn active_doing_task_user_message(
    session: &SessionManagement,
) -> Option<serde_json::Value> {
    task_user_message_by(session, task_is_doing)
}

fn task_user_message_by(
    session: &SessionManagement,
    predicate: fn(&crate::state_machine::session_management::TaskStep) -> bool,
) -> Option<serde_json::Value> {
    let (_index, task) = session
        .task_plan
        .detailed_tasks
        .iter()
        .enumerate()
        .find(|(_, task)| predicate(task))?;
    let current_task = context_blocks::current_task_text(&task.task_summary);
    Some(serde_json::json!({
        "role": "user",
        "content": context_blocks::current_objective_block(
            &session.current_objective,
            Some(current_task),
        )
    }))
}

pub(crate) fn record_task_focus_message(
    session: &mut SessionManagement,
    message: &serde_json::Value,
) {
    record_task_focus_message_for_terminal_done(session, message, false);
}

pub(crate) fn record_task_focus_message_for_terminal_done(
    session: &mut SessionManagement,
    message: &serde_json::Value,
    only_todo: bool,
) {
    let Some(task) = session.task_plan.detailed_tasks.iter().find(|task| {
        if only_todo {
            task_is_user_action_todo(task)
        } else {
            task_is_executable(task)
        }
    }) else {
        return;
    };
    let task_id = task.task_id.as_str();
    if session.session_log.iter().rev().any(|entry| {
        serde_json::from_str::<serde_json::Value>(entry)
            .ok()
            .filter(|value| value.get("type").and_then(|kind| kind.as_str()) == Some("task_focus"))
            .and_then(|value| {
                value
                    .get("task_id")
                    .and_then(serde_json::Value::as_str)
                    .map(|seen| seen == task_id)
            })
            .unwrap_or(false)
    }) {
        return;
    }
    let now = Utc::now();
    session.push_log(
        serde_json::json!({
            "type": "task_focus",
            "task_id": task.task_id,
            "step": task.step,
            "task_summary": task.task_summary,
            "deliverable": task.step_deliverable_description,
            "content": message.get("content").cloned().unwrap_or(serde_json::Value::Null),
            "timestamp": now.to_rfc3339(),
        })
        .to_string(),
        now,
    );
}

fn task_is_executable(task: &crate::state_machine::session_management::TaskStep) -> bool {
    task.status == TaskStatus::Doing
        || (task.status == TaskStatus::Todo
            && task.start_condition
                == crate::state_machine::session_management::StartCondition::UserAction)
}

fn task_is_doing(task: &crate::state_machine::session_management::TaskStep) -> bool {
    task.status == TaskStatus::Doing
}

fn task_is_user_action_todo(task: &crate::state_machine::session_management::TaskStep) -> bool {
    task.status == TaskStatus::Todo
        && task.start_condition
            == crate::state_machine::session_management::StartCondition::UserAction
}

#[cfg(test)]
mod tests {
    const COMMAND_RUN_TOOL: &str = "command_run";
    use crate::context::build_messages_from_session;
    use crate::runtime::types::ToolCallData;
    use crate::state_machine::session_management::{
        PlanStatus, SessionInput, SessionManagement, StartCondition, TaskStep,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::path::PathBuf;

    fn command_run_call(command_types: &[&str]) -> ToolCallData {
        ToolCallData {
            tool_name: COMMAND_RUN_TOOL.to_string(),
            arguments: json!({
                "commands": command_types
                    .iter()
                    .map(|ct| json!({ "command_type": ct, "command_line": "x" }))
                    .collect::<Vec<_>>()
            }),
            provider_metadata: None,
        }
    }

    #[test]
    fn no_write_detection_drives_task_status_nudge_counter() {
        // Read-only / verification-only command_run turns are "no write".
        assert!(!super::command_run_turn_has_write_or_status(&[
            command_run_call(&["shell_command"])
        ]));
        assert!(!super::command_run_turn_has_write_or_status(&[
            command_run_call(&["shell_command", "read_media"])
        ]));
        // A write (apply_patch) or a task state command (task_status) counts.
        assert!(super::command_run_turn_has_write_or_status(&[
            command_run_call(&["shell_command", "apply_patch"])
        ]));
        assert!(super::command_run_turn_has_write_or_status(&[
            command_run_call(&["task_status"])
        ]));
        // Alias spelling normalizes too.
        assert!(super::command_run_turn_has_write_or_status(&[
            command_run_call(&["apply-patch"])
        ]));
    }

    #[test]
    fn task_status_detection_accepts_streamed_command_run_results() {
        let result = json!({
            "streamed_command_run_result": {
                "results": [{
                    "command_type": "task_status",
                    "output": {
                        "task_status": {
                            "status": "done",
                            "task_detail": "finished"
                        }
                    }
                }]
            }
        });

        assert_eq!(
            super::command_run_result_terminal_task_status(&result).as_deref(),
            Some("done")
        );

        let question = json!({
            "results": [{
                "command_type": "task_status",
                "output": {
                    "task_status": {
                        "status": "question",
                        "content": "Need API key."
                    }
                }
            }]
        });
        assert_eq!(
            super::command_run_result_terminal_task_status(&question).as_deref(),
            Some("question")
        );

        let doing = json!({
            "results": [{
                "command_type": "task_status",
                "output": {
                    "task_status": {
                        "status": "doing"
                    }
                }
            }]
        });
        assert_eq!(
            super::command_run_result_terminal_task_status(&doing).as_deref(),
            Some("doing")
        );
    }

    #[test]
    fn command_run_result_detects_real_commands_before_terminal_status() {
        let result = json!({
            "streamed_command_run_result": {
                "results": [
                    { "command_type": "shell_command", "success": true, "output": "ok" },
                    {
                        "command_type": "task_status",
                        "success": true,
                        "output": { "task_status": { "status": "done" } }
                    }
                ]
            }
        });
        assert!(super::command_run_result_has_non_status_command(&result));

        let status_only = json!({
            "results": [{
                "command_type": "task_status",
                "success": true,
                "output": { "task_status": { "status": "done" } }
            }]
        });
        assert!(!super::command_run_result_has_non_status_command(
            &status_only
        ));
    }

    fn session_with_tasks(tasks: Vec<TaskStep>) -> SessionManagement {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "sess-executable-task".to_string(),
            "task routing".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "finish queued work".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "finish queued work".to_string(),
            now,
        );
        session.task_plan.detailed_tasks = tasks;
        session
    }

    fn task(step: u64, status: PlanStatus, start_condition: StartCondition) -> TaskStep {
        TaskStep {
            task_id: format!("task-{step}"),
            step,
            task_summary: format!("Task {step}"),
            step_deliverable_description: format!("Deliverable {step}"),
            status,
            start_condition,
            ..TaskStep::default()
        }
    }

    #[test]
    fn terminal_task_status_continues_when_gateway_added_task_is_executable() {
        let session = session_with_tasks(vec![
            task(1, PlanStatus::Done, StartCondition::UserAction),
            task(2, PlanStatus::Todo, StartCondition::UserAction),
        ]);

        let message =
            super::active_todo_task_user_message(&session).expect("todo task is executable");
        assert!(message["content"]
            .as_str()
            .expect("message content")
            .contains("Task 2"));
    }

    #[test]
    fn terminal_task_status_done_only_continues_for_todo_user_action_task() {
        let session = session_with_tasks(vec![
            task(1, PlanStatus::Done, StartCondition::UserAction),
            task(2, PlanStatus::Doing, StartCondition::UserAction),
        ]);

        assert!(super::active_todo_task_user_message(&session).is_none());
    }

    #[test]
    fn terminal_task_status_done_focuses_nearest_todo_not_existing_doing() {
        let mut session = session_with_tasks(vec![
            task(1, PlanStatus::Done, StartCondition::UserAction),
            task(2, PlanStatus::Doing, StartCondition::UserAction),
            task(3, PlanStatus::Todo, StartCondition::UserAction),
        ]);
        session.current_objective = "Original user objective".to_string();

        let message =
            super::active_todo_task_user_message(&session).expect("todo task should be selected");
        super::record_task_focus_message_for_terminal_done(&mut session, &message, true);

        let content = message["content"].as_str().expect("message content");
        assert!(content.contains("[current objective]:\nOriginal user objective"));
        assert!(!content.contains("[current task]:"));
        assert!(content.ends_with("\n\nTask 3"));
        assert_eq!(session.current_objective, "Original user objective");
        let focus_event = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .find(|value| {
                value.get("type").and_then(serde_json::Value::as_str) == Some("task_focus")
            })
            .expect("task focus should be recorded");
        assert_eq!(focus_event["task_id"], "task-3");
    }

    #[test]
    fn task_focus_is_audited_without_entering_model_context() {
        let mut session = session_with_tasks(vec![
            task(1, PlanStatus::Done, StartCondition::UserAction),
            task(2, PlanStatus::Todo, StartCondition::UserAction),
        ]);
        let message = super::active_task_user_message(&session).expect("todo task is executable");

        super::record_task_focus_message(&mut session, &message);
        super::record_task_focus_message(&mut session, &message);

        let focus_events = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .filter(|value| {
                value.get("type").and_then(serde_json::Value::as_str) == Some("task_focus")
            })
            .collect::<Vec<_>>();
        assert_eq!(focus_events.len(), 1);
        assert_eq!(focus_events[0]["task_id"], "task-2");
        let context_messages = build_messages_from_session(&session);
        assert!(!context_messages.iter().any(|value| {
            value
                .get("content")
                .map(|content| content.to_string().contains("[current objective]"))
                .unwrap_or(false)
        }));
    }

    #[test]
    fn terminal_task_status_ends_when_only_scheduled_or_completed_tasks_remain() {
        let session = session_with_tasks(vec![
            task(1, PlanStatus::Done, StartCondition::UserAction),
            task(2, PlanStatus::Todo, StartCondition::ScheduledTask),
        ]);

        assert!(super::active_todo_task_user_message(&session).is_none());
        assert!(super::active_task_user_message(&session).is_none());
    }
}
