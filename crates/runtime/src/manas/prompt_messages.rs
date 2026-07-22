use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    time::Instant,
};

use crate::context::user_input_content_value;
use crate::profile_timings;
use crate::prompt_style::{
    context_blocks, runtime_prompt_manual, tail_injection, task_status, user_new_command,
    PromptBuilder,
};
use lifecycle::ContextTokenStats;
use lifecycle::SessionManagement;
use lifecycle::{PlanStatus, StartCondition, TaskStep};

pub(crate) struct TurnMessages {
    pub messages: Vec<serde_json::Value>,
    pub context_tokens: ContextTokenStats,
}

pub(crate) fn messages_for_turn_with_context_limit(
    current_messages: &[serde_json::Value],
    session: &mut SessionManagement,
    _original_user_task: &str,
    context_limit_tokens: u64,
) -> TurnMessages {
    let total_start = Instant::now();
    let profiling = profile_timings::enabled();
    let clone_start = Instant::now();
    let mut messages = current_messages.to_vec();
    profile_timings::log_elapsed(
        "messages_for_turn.clone_current_messages",
        clone_start,
        serde_json::json!({
            "session_id": session.session_id,
            "input_message_count": current_messages.len(),
            "output_message_count": messages.len(),
            "input_messages_bytes": if profiling {
                profile_timings::json_vec_bytes(current_messages)
            } else {
                0
            },
        }),
    );
    let user_command_start = Instant::now();
    let user_commands = fetch_user_commands(&session.session_id);
    if !user_commands.is_empty() {
        record_user_new_commands(session, &user_commands);
        append_user_new_command_messages(&mut messages, &user_commands);
    }
    let user_command = user_new_command_message_from_commands(&user_commands);
    profile_timings::log_elapsed(
        "messages_for_turn.user_new_command_message",
        user_command_start,
        serde_json::json!({
            "session_id": session.session_id,
            "has_user_command": user_command.is_some(),
        }),
    );
    if let Some(content) = user_command {
        tail_injection::append_tail_prompt(
            &mut messages,
            tail_injection::TailPrompt::developer(content),
        );
    }
    profile_timings::log_elapsed(
        "messages_for_turn.total",
        total_start,
        serde_json::json!({
            "session_id": session.session_id,
            "message_count": messages.len(),
            "context_limit_tokens": context_limit_tokens,
        }),
    );
    TurnMessages {
        messages,
        context_tokens: ContextTokenStats {
            input: session.context_tokens.input,
            limit: context_limit_tokens,
        },
    }
}

fn user_new_command_message_from_commands(commands: &[String]) -> Option<String> {
    if commands.is_empty() {
        return None;
    }
    let commands = commands
        .iter()
        .enumerate()
        .map(|(index, command)| format!("{}. {}", index + 1, command))
        .collect::<Vec<_>>()
        .join("\n");
    Some(
        PromptBuilder::new()
            .part(user_new_command::USER_NEW_COMMAND)
            .section("user_new_commands", commands)
            .render(),
    )
}

fn append_user_new_command_messages(messages: &mut Vec<serde_json::Value>, commands: &[String]) {
    for command in commands.iter().map(|command| command.trim()) {
        if command.is_empty() {
            continue;
        }
        messages.push(serde_json::json!({
            "role": "developer",
            "content": user_input_content_value(command),
        }));
    }
}

fn record_user_new_commands(session: &mut SessionManagement, commands: &[String]) {
    for command in commands.iter().map(|command| command.trim()) {
        if command.is_empty() {
            continue;
        }
        let now = chrono::Utc::now();
        let record = serde_json::json!({
            "type": "user",
            "role": "developer",
            "content": user_input_content_value(command),
            "created_at": now.timestamp_millis(),
            "updated_at": now.timestamp_millis(),
            "timestamp": now.to_rfc3339(),
        });
        session.push_log(
            serde_json::to_string(&record).unwrap_or_else(|_| "message: user".to_string()),
            now,
        );
    }
}

pub(super) fn fetch_user_commands(session_id: &str) -> Vec<String> {
    fetch_user_commands_from_router(session_id).unwrap_or_default()
}

fn fetch_user_commands_from_router(session_id: &str) -> Option<Vec<String>> {
    let addr = std::env::var("TURA_ROUTER_ADDR")
        .ok()
        .and_then(|addr| addr.trim().parse::<SocketAddr>().ok())?;
    let target_session_id = user_command_router_session_id(session_id);
    let request = serde_json::json!({
        "request_id": format!("runtime-user-commands-{}-{}", std::process::id(), chrono::Utc::now().timestamp_millis()),
        "kind": "call",
        "method": "session.take_user_commands",
        "payload": {
            "session_id": target_session_id,
            "root_session_id": target_session_id,
        },
        "deadline_ms": user_command_fetch_timeout().as_millis() as u64,
    });
    let timeout = user_command_fetch_timeout();
    let mut stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    stream
        .write_all(format!("{request}\n").as_bytes())
        .and_then(|_| stream.flush())
        .ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let response = serde_json::from_str::<serde_json::Value>(line.trim()).ok()?;
    if !response.get("ok").and_then(serde_json::Value::as_bool)? {
        return None;
    }
    Some(
        response
            .pointer("/payload/commands")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToString::to_string)
            .collect(),
    )
}

fn user_command_router_session_id(session_id: &str) -> String {
    std::env::var("TURA_PARENT_SESSION_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| session_id.to_string())
}

fn user_command_fetch_timeout() -> std::time::Duration {
    std::env::var("TURA_USER_COMMAND_FETCH_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(std::time::Duration::from_millis)
        .unwrap_or_else(|| std::time::Duration::from_millis(100))
}

pub(crate) fn push_no_tool_task_status_retry_message(
    messages: &mut Vec<serde_json::Value>,
    session: &SessionManagement,
) {
    let operation_manual = runtime_prompt_manual::active_operation_manual_text(session);
    let content = task_status::no_tool_retry(
        &planning_objective_block(session),
        operation_manual.as_deref(),
    );
    tail_injection::append_tail_prompt(messages, tail_injection::TailPrompt::developer(content));
}

pub(crate) fn planning_objective_block(session: &SessionManagement) -> String {
    let overall = session.current_objective.trim();
    let Some((_index, task)) = current_planning_task(session) else {
        return context_blocks::current_objective_block(overall, None);
    };
    let current_task = planning_current_task_text(task);
    context_blocks::current_objective_block(overall, Some(current_task))
}

pub(crate) fn planning_current_task_text(task: &TaskStep) -> &str {
    context_blocks::current_task_text(&task.task_summary)
}

fn current_planning_task(session: &SessionManagement) -> Option<(usize, &TaskStep)> {
    session
        .task_plan
        .detailed_tasks
        .iter()
        .enumerate()
        .find(|(_, task)| {
            task.status == PlanStatus::Doing
                || (task.status == PlanStatus::Todo
                    && task.start_condition == StartCondition::UserAction)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        messages_for_turn_with_context_limit, planning_objective_block,
        push_no_tool_task_status_retry_message, record_user_new_commands,
    };
    use crate::context::{build_messages_from_session, compact_session_context};
    use chrono::Utc;
    use lifecycle::{PlanStatus, SessionInput, SessionManagement, StartCondition, TaskStep};
    use std::path::PathBuf;

    fn test_session(user_input: &str) -> SessionManagement {
        let now = Utc::now();
        SessionManagement::new(
            "sess-prompt-messages".to_string(),
            "prompt messages".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: user_input.to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            user_input.to_string(),
            now,
        )
    }

    #[test]
    fn planning_objective_block_without_task_only_includes_overall_objective() {
        let mut session = test_session("ship the feature");
        session.current_objective = "ship the feature".to_string();

        let content = planning_objective_block(&session);

        assert_eq!(content, "[current objective]:\nship the feature");
        assert!(!content.contains("[current task]:"));
    }

    #[test]
    fn planning_objective_block_with_task_includes_overall_and_current_task() {
        let mut session = test_session("ship the feature");
        session.current_objective = "ship the feature".to_string();
        let mut task_plan = session.task_plan.clone();
        task_plan.detailed_tasks.push(TaskStep {
            task_id: "task-1".to_string(),
            step: 1,
            task_summary: "Patch parser".to_string(),
            step_deliverable_description: "Parser accepts fixture flags.".to_string(),
            status: PlanStatus::Doing,
            start_condition: StartCondition::UserAction,
            ..TaskStep::default()
        });
        session.replace_task_plan(task_plan, Utc::now());

        let content = planning_objective_block(&session);

        assert_eq!(
            content,
            "[current objective]:\nship the feature\n\nPatch parser"
        );
        assert!(!content.contains("[current task]:"));
        assert!(!content.contains("Deliverable:"));
    }

    #[test]
    fn no_tool_retry_injects_active_goal_instead_of_last_user_message() {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "sess-no-tool-retry".to_string(),
            "no tool retry".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "ORIGINAL HUGE PROMPT".repeat(10),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "ORIGINAL HUGE PROMPT".to_string(),
            now,
        );
        session.current_objective = "STATE MACHINE OBJECTIVE".to_string();

        let mut messages = Vec::new();
        push_no_tool_task_status_retry_message(&mut messages, &session);
        let content = messages[0]["content"]
            .as_str()
            .expect("prompt message content should be a string");

        assert!(content.contains("Continue working toward the active thread user goal"));
        assert!(content.contains("active_goal:\n[current objective]:\nSTATE MACHINE OBJECTIVE"));
        assert!(content.contains("Do not infer the objective from the last user message"));
        assert!(
            !content.contains("The last user message in the conversation is the current objective")
        );
        assert!(!content.contains("ORIGINAL HUGE PROMPT"));
        assert!(!content.contains("original_user_task:"));
    }

    #[test]
    fn no_tool_retry_injects_current_task_when_present() {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "sess-no-tool-task".to_string(),
            "no tool task".to_string(),
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
        );
        session.current_objective = "fix the task".to_string();
        let mut task_plan = session.task_plan.clone();
        task_plan.detailed_tasks.push(TaskStep {
            task_id: "task-1".to_string(),
            step: 1,
            task_summary: "Patch parser".to_string(),
            status: PlanStatus::Doing,
            start_condition: StartCondition::UserAction,
            ..TaskStep::default()
        });
        session.replace_task_plan(task_plan, now);

        let mut messages = Vec::new();
        push_no_tool_task_status_retry_message(&mut messages, &session);
        let content = messages[0]["content"]
            .as_str()
            .expect("prompt message content should be a string");

        assert!(
            content.contains("active_goal:\n[current objective]:\nfix the task\n\nPatch parser")
        );
        assert!(
            !content.contains("The last user message in the conversation is the current objective")
        );
        assert!(!content.contains("original_user_task:"));
    }

    #[test]
    fn messages_for_turn_preserves_provider_context_tokens_without_estimating() {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "sess-compact-threshold".to_string(),
            "compact threshold".to_string(),
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
        );
        session.context_tokens.input = 1234;

        let turn = messages_for_turn_with_context_limit(
            &[serde_json::json!({
                "role": "user",
                "content": "x".repeat(100)
            })],
            &mut session,
            "fix the task",
            10,
        );
        let joined = turn
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!joined.contains("Context checkpoint required"));
        assert!(!joined.contains("command_type\":\"compact_context"));
        assert_eq!(turn.context_tokens.limit, 10);
        assert_eq!(turn.context_tokens.input, 1234);
    }

    #[test]
    fn messages_for_turn_does_not_append_original_user_task_tail() {
        let mut session = test_session("tail task");
        let messages = messages_for_turn_with_context_limit(
            &[
                serde_json::json!({"role": "system", "content": "fixed system prefix"}),
                serde_json::json!({"role": "user", "content": "x".repeat(100)}),
            ],
            &mut session,
            "tail task",
            10,
        )
        .messages;

        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "fixed system prefix");
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|message| {
            message.get("content").and_then(serde_json::Value::as_str) != Some("tail task")
        }));
    }

    #[test]
    fn user_new_commands_record_as_user_type_developer_messages_for_compaction() {
        let mut session = test_session("original task");
        record_user_new_commands(
            &mut session,
            &[String::from("change direction and inspect the new failure")],
        );

        let log_entry = session
            .session_log
            .last()
            .expect("user new command should be recorded");
        let value: serde_json::Value = serde_json::from_str(log_entry).expect("json log");
        assert_eq!(value["type"], "user");
        assert_eq!(value["role"], "developer");
        assert_eq!(
            value["content"],
            "change direction and inspect the new failure"
        );

        compact_session_context(&mut session, "handoff").expect("compact should succeed");
        let joined = build_messages_from_session(&session)
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("current_run/user: change direction and inspect the new failure"),
            "{joined}"
        );
    }
}
