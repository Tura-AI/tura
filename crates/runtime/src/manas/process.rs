use super::constants::{COMMAND_RUN_TOOL, TASK_STATUS_COMMAND};
use super::final_response::user_visible_runtime_text;
use super::gateway_events::{
    publish_gateway_agent_message, publish_runtime_failure_message, publish_runtime_usage_record,
};
use super::prompt_messages::{
    messages_for_turn, planning_current_task_text, push_no_tool_task_status_retry_message,
    push_task_status_nudge,
};
use super::runtime_turn::execute_turn;
use super::tool_catalog::{command_run_commands_for_agent, planning_child_depth};
use super::tool_execution::execute_tool_calls;
use super::validator_feedback::apply_validator_reliability_feedback;
use chrono::Utc;
use std::{
    collections::HashSet,
    io::Write,
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};
use tracing::{info, warn};
use tura_llm_rust::{
    provider_media_fallback, replace_unsupported_content_type_in_messages, ProviderMediaFallback,
};

use crate::context::{
    accumulate_tool_result_with_provider_metadata, build_context, compact_session_context,
    ContextInput,
};
use crate::manas::ManasOverrides;
use crate::mano::persist_gateway_session;
use crate::state_machine::agent_management::{AgentManagement, AgentState};
use crate::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeId, RuntimeManagement,
};
use crate::state_machine::session_management::TaskStatus;
use crate::state_machine::session_management::{SessionManagement, SessionState};
use crate::tool_router::execute_tool::ToolExecutionResult;

#[cfg(test)]
use super::agent_prompts::load_agent_prompt_messages;
#[cfg(test)]
use super::constants::PLANNING_TOOL;
#[cfg(test)]
use super::tool_arguments::{normalize_tool_arguments, normalize_tool_arguments_for_tool};
#[cfg(test)]
use super::tool_catalog::{
    filter_tools_for_turn, remove_tool, require_planning_tool_for_planning_mode,
};
#[cfg(test)]
use tura_llm_rust::provider_unsupported_content_type;

pub struct ManasInput<'a> {
    pub agents: &'a mut [AgentManagement],
    pub session: &'a mut SessionManagement,
    pub initial_messages: Vec<serde_json::Value>,
    pub redis_url: &'a str,
}

pub struct ManasResult {
    pub agents: Vec<AgentManagement>,
    pub session: SessionManagement,
    pub final_runtime: RuntimeManagement,
}

pub fn process_manas_internal(
    input: ManasInput,
    overrides: ManasOverrides,
) -> Result<ManasResult, String> {
    let ManasInput {
        agents,
        session,
        initial_messages,
        redis_url,
    } = input;
    let mut loaded_agents;
    let agents = if agents.is_empty() {
        if let Some(agent_loader) = overrides.agent_loader {
            loaded_agents = agent_loader(session)?;
            loaded_agents.as_mut_slice()
        } else {
            agents
        }
    } else {
        agents
    };

    let now = Utc::now();

    session.transition(SessionState::Running, now)?;
    persist_session_checkpoint(session, "running");

    let mut current_messages = initial_messages.clone();
    let mut last_runtime_id: Option<RuntimeId> = None;
    let original_user_task = session.input.user_input.clone();
    let mut turn = 0_u64;
    let mut provider_timeout_retries = 0_u8;
    let mut no_tool_retries = 0_u8;
    let mut terminal_task_status_seen = false;
    let mut final_session_state = SessionState::Completed;
    let supports_task_status = agents
        .first()
        .map(command_run_commands_for_agent)
        .is_some_and(|commands| commands.contains(TASK_STATUS_COMMAND));
    // Count consecutive command_run turns that neither wrote (apply_patch) nor
    // settled task state (task_status). After the threshold, inject the
    // task_status nudge so a model stuck re-running read-only/verification
    // commands is reminded to mark done or ask a question.
    let mut no_write_command_run_turns = 0_u64;
    loop {
        turn = turn.saturating_add(1);
        info!(
            session_id = %session.session_id,
            turn = turn,
            "starting turn"
        );

        let runtime_result = match execute_turn(
            agents,
            session,
            &messages_for_turn(&current_messages, session, &original_user_task),
            redis_url,
            turn == 1,
            false,
            false,
        ) {
            Ok(result) => result,
            Err(error) => {
                warn!(
                    session_id = %session.session_id,
                    turn = turn,
                    error = %error,
                    "runtime failed during turn; publishing visible fallback"
                );
                let runtime_id = last_runtime_id
                    .clone()
                    .unwrap_or_else(|| format!("runtime-error-{}", session.session_id));
                publish_runtime_failure_message(session, &runtime_id, &error);
                if env_flag("TURA_FAIL_ON_RUNTIME_ERROR") {
                    return Err(error);
                }
                final_session_state = SessionState::Failed;
                break;
            }
        };

        let runtime = runtime_result.0;
        let tool_calls = runtime_result.1;

        last_runtime_id = Some(runtime.runtime_id.clone());

        accumulate_session_from_runtime(session, &runtime, true)?;
        apply_validator_reliability_feedback(&runtime);
        publish_runtime_usage_record(session, &runtime);
        session.increment_turn(now);
        persist_session_checkpoint(session, "runtime");

        if runtime.call_result_status == RuntimeCallResultStatus::TimedOut
            || runtime_failure_allows_retry(&runtime)
        {
            let error_text = runtime_failure_text(&runtime)
                .unwrap_or_else(|| "Provider runtime failed before producing output.".to_string());
            if let Some(wait_duration) = provider_timeout_retry_wait(provider_timeout_retries) {
                if let Some(ProviderMediaFallback::UnsupportedRequiredContent { content_type }) =
                    provider_media_fallback(&error_text)
                {
                    warn!(
                        session_id = %session.session_id,
                        turn = turn,
                        runtime_id = %runtime.runtime_id,
                        content_type = content_type,
                        error = %error_text,
                        "provider rejected required media content; not retrying without media"
                    );
                    publish_runtime_failure_message(
                        session,
                        &runtime.runtime_id,
                        &format!(
                            "Provider/model does not support `{content_type}` media input for this request. Use an image-capable model or a route whose model metadata includes that input modality. Original provider error: {error_text}"
                        ),
                    );
                    final_session_state = SessionState::Failed;
                    break;
                }
                let removed_media = provider_media_fallback(&error_text)
                    .and_then(ProviderMediaFallback::retry_content_type)
                    .map(|content_type| {
                        let removed = replace_unsupported_content_type_in_messages(
                            &mut current_messages,
                            content_type,
                        );
                        (content_type, removed)
                    })
                    .filter(|(_, removed)| *removed > 0);
                provider_timeout_retries = provider_timeout_retries.saturating_add(1);
                warn!(
                    session_id = %session.session_id,
                    turn = turn,
                    runtime_id = %runtime.runtime_id,
                    status = ?runtime.call_result_status,
                    error = %error_text,
                    retry = provider_timeout_retries,
                    wait_ms = wait_duration.as_millis(),
                    "provider runtime failed transiently; waiting before retrying with full tool set"
                );
                thread::sleep(wait_duration);
                if let Some((content_type, removed)) = removed_media {
                    current_messages.push(serde_json::json!({
                        "role": "system",
                        "content": format!(
                            "The provider rejected `{content_type}` media content. {removed} item(s) were omitted from the next request and replaced with text placeholders; continue using the remaining text and supported media."
                        )
                    }));
                }
                current_messages.push(serde_json::json!({
                    "role": "system",
                    "content": format!("Provider failure while waiting for the model response: {error_text}. This is transient provider failure retry {} of 3, not task completion. Retry the current task with the normal command_run tool unless the requested edits and validation are actually complete.", provider_timeout_retries)
                }));
                continue;
            }

            warn!(
                session_id = %session.session_id,
                turn = turn,
                runtime_id = %runtime.runtime_id,
                status = ?runtime.call_result_status,
                error = %error_text,
                retries = provider_timeout_retries,
                "provider runtime failed transiently after retries; publishing visible failure"
            );
            publish_runtime_failure_message(
                session,
                &runtime.runtime_id,
                &format!(
                    "Provider runtime failed after 3 retries before completing the task: {error_text}"
                ),
            );
            final_session_state = SessionState::Failed;
            break;
        }
        if runtime.call_result_status == RuntimeCallResultStatus::Failed {
            let error_text = runtime_failure_text(&runtime)
                .unwrap_or_else(|| "Provider runtime failed before producing output.".to_string());
            warn!(
                session_id = %session.session_id,
                turn = turn,
                runtime_id = %runtime.runtime_id,
                error = %error_text,
                "provider runtime failed"
            );
            publish_runtime_failure_message(session, &runtime.runtime_id, &error_text);
            if env_flag("TURA_FAIL_ON_RUNTIME_ERROR") {
                return Err(error_text);
            }
            final_session_state = SessionState::Failed;
            break;
        }

        if !tool_calls.is_empty() {
            if let Some(content) = user_visible_runtime_text(&runtime.text)
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
            {
                if let Err(error) = publish_gateway_agent_message(
                    &session.session_id,
                    &runtime.runtime_id,
                    content,
                    String::new(),
                ) {
                    warn!(
                        session_id = %session.session_id,
                        runtime_id = %runtime.runtime_id,
                        error = %error,
                        "failed to publish assistant text before tool execution"
                    );
                }
            }
            provider_timeout_retries = 0;
            no_tool_retries = 0;
            let mut tool_results =
                execute_tool_calls(&tool_calls, agents.first(), session, &runtime, redis_url)?;
            apply_compact_context_results(session, &mut tool_results)?;
            let terminal_task_status = tool_results
                .iter()
                .find_map(|result| command_run_result_terminal_task_status(&result.result));
            terminal_task_status_seen = terminal_task_status.is_some();

            // Track consecutive command_run turns with no write/state command.
            if command_run_turn_has_write_or_status(&tool_calls) || terminal_task_status_seen {
                no_write_command_run_turns = 0;
            } else if tool_calls
                .iter()
                .any(|tool_call| tool_call.tool_name == COMMAND_RUN_TOOL)
            {
                no_write_command_run_turns = no_write_command_run_turns.saturating_add(1);
            }

            for (index, tool_result) in tool_results.iter().enumerate() {
                if command_run_results_empty(&tool_result.result) {
                    continue;
                }
                accumulate_tool_result_with_provider_metadata(
                    session,
                    &tool_result.tool_name,
                    tool_result.arguments.clone(),
                    tool_result.result.clone(),
                    tool_result.success,
                    tool_result.error.clone(),
                    tool_calls
                        .get(index)
                        .and_then(|tool_call| tool_call.provider_metadata.clone()),
                )?;
            }
            persist_session_checkpoint(session, "tool_results");

            let context_output = build_context(ContextInput {
                session: session.clone(),
                runtime: runtime.clone(),
                additional_messages: Vec::new(),
            })?;

            current_messages = messages_with_initial_context_prefix(
                &initial_messages,
                context_output.messages,
                &original_user_task,
            );
            let next_task = if terminal_task_status.as_deref() == Some("done") {
                active_todo_task_user_message(session)
            } else {
                active_task_user_message(session)
            };
            if let Some(next_task) = next_task {
                record_task_focus_message_for_terminal_done(
                    session,
                    &next_task,
                    terminal_task_status.as_deref() == Some("done"),
                );
                persist_session_checkpoint(session, "task_focus");
                current_messages.push(next_task);
            } else if matches!(terminal_task_status.as_deref(), Some("done" | "question")) {
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    status = terminal_task_status.as_deref().unwrap_or("unknown"),
                    "terminal task_status returned and no next executable task exists; ending loop"
                );
                break;
            }

            // The model keeps running command_run without writing or settling
            // task state; remind it to mark done or ask a question.
            if supports_task_status
                && no_write_command_run_turns >= NO_WRITE_COMMAND_RUN_NUDGE_THRESHOLD
            {
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    no_write_turns = no_write_command_run_turns,
                    "injecting task_status nudge after consecutive no-write command_run turns"
                );
                push_task_status_nudge(&mut current_messages);
            }
        } else {
            let context_output = build_context(ContextInput {
                session: session.clone(),
                runtime: runtime.clone(),
                additional_messages: Vec::new(),
            })?;

            current_messages = messages_with_initial_context_prefix(
                &initial_messages,
                context_output.messages,
                &original_user_task,
            );
            if planning_child_depth() > 0 {
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    "planning child turn completed without tool calls, ending child session without synthesized user receipt"
                );
                break;
            }

            if no_tool_retries < no_tool_retry_limit() {
                no_tool_retries = no_tool_retries.saturating_add(1);
                push_no_tool_task_status_retry_message(&mut current_messages, session);
                if let Some(next_task) = active_task_user_message(session) {
                    record_task_focus_message(session, &next_task);
                    persist_session_checkpoint(session, "task_focus");
                    current_messages.push(next_task);
                }
                warn!(
                    session_id = %session.session_id,
                    turn = turn,
                    runtime_id = %runtime.runtime_id,
                    no_tool_retries = no_tool_retries,
                    "non-final turn returned no tool calls; retrying with normal tool set"
                );
                continue;
            }

            info!(
                session_id = %session.session_id,
                turn = turn,
                "turn completed without command_run after retries, ending session"
            );
            break;
        }
    }

    session.transition(final_session_state, now)?;
    persist_session_checkpoint(
        session,
        if final_session_state == SessionState::Failed {
            "failed"
        } else {
            "completed"
        },
    );

    for agent in agents.iter_mut() {
        agent.state = if final_session_state == SessionState::Failed {
            AgentState::Failed
        } else {
            AgentState::Completed
        };
        agent.updated_at = Utc::now();
    }

    let final_runtime = create_dummy_runtime(last_runtime_id.unwrap_or_default(), session);

    Ok(ManasResult {
        agents: agents.to_vec(),
        session: session.clone(),
        final_runtime,
    })
}

/// Inject the task_status nudge once this many consecutive command_run turns
/// have produced no write (`apply_patch`) and no task state change
/// (`task_status`).
const NO_WRITE_COMMAND_RUN_NUDGE_THRESHOLD: u64 = 3;

/// True if any command_run tool call in this turn includes a write command
/// (`apply_patch`) or a task-state command (`task_status`).
fn command_run_turn_has_write_or_status(
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

fn command_run_result_terminal_task_status(result: &serde_json::Value) -> Option<String> {
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
        .filter(|status| matches!(*status, "done" | "question"))
        .map(ToString::to_string)
}

fn apply_compact_context_results(
    session: &mut SessionManagement,
    tool_results: &mut [ToolExecutionResult],
) -> Result<(), String> {
    for tool_result in tool_results.iter_mut() {
        if tool_result.tool_name != COMMAND_RUN_TOOL {
            continue;
        }
        let Some(summary) = compact_context_summary_from_command_run(&tool_result.result) else {
            continue;
        };
        compact_session_context(session, &summary)?;
        strip_compact_context_from_command_run(&mut tool_result.arguments, &mut tool_result.result);
        tool_result.success = command_run_result_success_value(&tool_result.result);
        tool_result.error = command_run_result_error_value(&tool_result.result);
    }
    Ok(())
}

fn compact_context_summary_from_command_run(result: &serde_json::Value) -> Option<String> {
    result
        .get("results")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|item| {
            item.get("command_type")
                .or_else(|| item.get("command"))
                .and_then(serde_json::Value::as_str)
                == Some("compact_context")
                && item.get("success").and_then(serde_json::Value::as_bool) == Some(true)
        })
        .and_then(|item| {
            item.get("output")
                .and_then(|output| {
                    output
                        .get("compact_context")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| output.as_str())
                })
                .map(ToString::to_string)
        })
}

fn strip_compact_context_from_command_run(
    arguments: &mut serde_json::Value,
    result: &mut serde_json::Value,
) {
    if let Some(commands) = arguments
        .get_mut("commands")
        .and_then(serde_json::Value::as_array_mut)
    {
        commands.retain(|command| {
            command
                .get("command_type")
                .or_else(|| command.get("command"))
                .and_then(serde_json::Value::as_str)
                .map(canonical_command_name)
                .as_deref()
                != Some("compact_context")
        });
    }
    if let Some(results) = result
        .get_mut("results")
        .and_then(serde_json::Value::as_array_mut)
    {
        results.retain(|item| {
            item.get("command_type")
                .or_else(|| item.get("command"))
                .and_then(serde_json::Value::as_str)
                != Some("compact_context")
        });
    }
}

fn canonical_command_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('-', "_")
}

fn command_run_results_empty(result: &serde_json::Value) -> bool {
    result
        .get("results")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|results| results.is_empty())
}

fn command_run_result_success_value(result: &serde_json::Value) -> bool {
    result
        .get("results")
        .and_then(serde_json::Value::as_array)
        .map(|results| {
            results
                .iter()
                .all(|item| item.get("success").and_then(serde_json::Value::as_bool) == Some(true))
        })
        .unwrap_or(true)
}

fn command_run_result_error_value(result: &serde_json::Value) -> Option<String> {
    if command_run_result_success_value(result) {
        return None;
    }
    result
        .get("results")
        .and_then(serde_json::Value::as_array)
        .and_then(|results| {
            results.iter().find_map(|item| {
                if item.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
                    item.get("error")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string)
                } else {
                    None
                }
            })
        })
}

fn active_task_user_message(session: &SessionManagement) -> Option<serde_json::Value> {
    task_user_message_by(session, task_is_executable)
}

fn active_todo_task_user_message(session: &SessionManagement) -> Option<serde_json::Value> {
    task_user_message_by(session, task_is_user_action_todo)
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
    let current_task = planning_current_task_text(task);
    Some(serde_json::json!({
        "role": "user",
        "content": format!(
            "[current objective]:\n{}\n\n{}",
            session.current_objective.trim(),
            current_task
        )
    }))
}

fn record_task_focus_message(session: &mut SessionManagement, message: &serde_json::Value) {
    record_task_focus_message_for_terminal_done(session, message, false);
}

fn record_task_focus_message_for_terminal_done(
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
    let task_id = task.task_id.clone();
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

fn task_is_user_action_todo(task: &crate::state_machine::session_management::TaskStep) -> bool {
    task.status == TaskStatus::Todo
        && task.start_condition
            == crate::state_machine::session_management::StartCondition::UserAction
}

fn planned_tasks_incomplete(session: &SessionManagement) -> bool {
    session
        .task_plan
        .detailed_tasks
        .iter()
        .any(|task| task.status != TaskStatus::Done)
}

fn persist_session_checkpoint(session: &SessionManagement, stage: &str) {
    if let Err(err) = persist_gateway_session(session) {
        warn!(
            session_id = %session.session_id,
            stage,
            error = %err,
            "failed to persist gateway session checkpoint"
        );
    }
    emit_cli_live_checkpoint(session, stage);
}

fn emit_cli_live_checkpoint(session: &SessionManagement, stage: &str) {
    if !env_flag("TURA_CLI_LIVE_JSONL") {
        return;
    }
    if matches!(stage, "completed") {
        return;
    }
    static EMITTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let emitted = EMITTED.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut emitted) = emitted.lock() else {
        return;
    };
    if !emitted.insert(session.session_id.clone()) {
        return;
    }
    let event = serde_json::json!({
        "type": "item.completed",
        "item": {
            "id": "item_live_0",
            "type": "agent_message",
            "text": "Runtime session is active; detailed command events will follow."
        }
    });
    println!("{event}");
    let _ = std::io::stdout().flush();
}

fn messages_with_initial_context_prefix(
    initial_messages: &[serde_json::Value],
    session_messages: Vec<serde_json::Value>,
    _original_user_task: &str,
) -> Vec<serde_json::Value> {
    let mut messages = initial_messages
        .iter()
        .filter(|message| {
            let role = message.get("role").and_then(|role| role.as_str());
            role == Some("developer")
        })
        .cloned()
        .collect::<Vec<_>>();
    messages.extend(session_messages);
    messages
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn no_tool_retry_limit() -> u8 {
    std::env::var("TURA_NO_TOOL_RETRY_LIMIT")
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(20)
}

fn accumulate_session_from_runtime(
    session: &mut SessionManagement,
    runtime: &RuntimeManagement,
    publish_runtime_text: bool,
) -> Result<(), String> {
    let now = Utc::now();

    if let Some(usage) = &runtime.usage {
        session.push_log(
            serde_json::json!({
                "type": "runtime_usage",
                "runtime_id": runtime.runtime_id,
                "usage": usage,
                "status": format!("{:?}", runtime.call_result_status),
                "cache_diagnostics": runtime_cache_diagnostics(runtime),
                "timestamp": now.to_rfc3339(),
            })
            .to_string(),
            now,
        );
    }

    if !publish_runtime_text {
        return Ok(());
    }

    if let Some(content) = user_visible_runtime_text(&runtime.text) {
        session.push_log(
            serde_json::json!({
                "role": "assistant",
                "content": content,
            })
            .to_string(),
            now,
        );
    }

    Ok(())
}

fn runtime_cache_diagnostics(runtime: &RuntimeManagement) -> serde_json::Value {
    let input = runtime.input.as_ref();
    let messages = input
        .and_then(|input| input.get("messages"))
        .and_then(serde_json::Value::as_array);
    let tools = input
        .and_then(|input| input.get("tools"))
        .and_then(serde_json::Value::as_array);
    let options = input.and_then(|input| input.get("options"));
    serde_json::json!({
        "input_hash": input.map(stable_json_hash).unwrap_or_default(),
        "message_count": messages.map(|messages| messages.len()).unwrap_or_default(),
        "tool_count": tools.map(|tools| tools.len()).unwrap_or_default(),
        "first_message_hash": messages
            .and_then(|messages| messages.first())
            .map(stable_json_hash)
            .unwrap_or_default(),
        "last_message_hash": messages
            .and_then(|messages| messages.last())
            .map(stable_json_hash)
            .unwrap_or_default(),
        "tools_hash": tools
            .map(|tools| stable_json_hash(&serde_json::Value::Array(tools.clone())))
            .unwrap_or_default(),
        "prompt_cache_key": options
            .and_then(|options| options.get("prompt_cache_key"))
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    })
}

fn stable_json_hash(value: &serde_json::Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in serialized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn provider_timeout_retry_wait(retry_count: u8) -> Option<Duration> {
    match retry_count {
        0 => Some(Duration::from_secs(5)),
        1 => Some(Duration::from_secs(15)),
        2 => Some(Duration::from_secs(45)),
        _ => None,
    }
}

fn runtime_failure_allows_retry(runtime: &RuntimeManagement) -> bool {
    runtime.call_result_status == RuntimeCallResultStatus::Failed
        && runtime
            .error
            .as_ref()
            .map(|error| error.retry_allowed)
            .unwrap_or(false)
}

fn runtime_failure_text(runtime: &RuntimeManagement) -> Option<String> {
    runtime
        .error
        .as_ref()
        .and_then(|error| error.error_text.clone())
        .or_else(|| {
            runtime
                .output
                .as_ref()
                .and_then(|output| output.get("error"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
}

fn create_dummy_runtime(runtime_id: RuntimeId, session: &SessionManagement) -> RuntimeManagement {
    let now = Utc::now();
    let provider_name = crate::agent_router::coding_agent_provider_name();

    let runtime_provider_config = crate::state_machine::runtime_management::RuntimeProviderConfig {
        base: crate::state_machine::agent_management::ProviderConfig {
            tura_llm_name: provider_name.clone(),
            stream: true,
            temperature: 0.5,
            max_tokens: 0,
            tool_choice: crate::state_machine::agent_management::ToolChoice::Auto,
            time_out_ms: 120_000,
        },
        thinking: false,
        provider_name: provider_name.clone(),
        model_name: String::new(),
        provider_url_name: String::new(),
        llm_provider_name: provider_name,
    };

    let mut runtime = RuntimeManagement::new(
        runtime_id,
        session.session_id.clone(),
        session.session_id.clone(),
        runtime_provider_config,
        now,
    );

    let _ = runtime.finish_success(now, None);

    runtime
}

#[cfg(test)]
mod tests {
    use super::{
        command_run_turn_has_write_or_status, filter_tools_for_turn, load_agent_prompt_messages,
        messages_with_initial_context_prefix, normalize_tool_arguments,
        normalize_tool_arguments_for_tool, provider_timeout_retry_wait,
        provider_unsupported_content_type, remove_tool,
        replace_unsupported_content_type_in_messages, require_planning_tool_for_planning_mode,
        runtime_failure_allows_retry, runtime_failure_text, user_visible_runtime_text,
        COMMAND_RUN_TOOL, PLANNING_TOOL,
    };
    use crate::context::build_messages_from_session;
    use crate::runtime::types::ToolCallData;
    use crate::state_machine::agent_management::{
        AgentCapabilityItem, AgentManagement, AgentPromptItem, ProviderConfig, ToolChoice,
        ValidatorConfig,
    };
    use crate::state_machine::runtime_management::{
        RuntimeCallResultStatus, RuntimeError, RuntimeManagement, RuntimeProviderConfig,
    };
    use crate::state_machine::session_management::{
        PlanStatus, SessionInput, SessionManagement, StartCondition, TaskStep,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashSet;
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
        assert!(!command_run_turn_has_write_or_status(&[command_run_call(
            &["shell_command"]
        )]));
        assert!(!command_run_turn_has_write_or_status(&[command_run_call(
            &["shell_command", "read_media"]
        )]));
        // A write (apply_patch) or a task state command (task_status) counts.
        assert!(command_run_turn_has_write_or_status(&[command_run_call(
            &["shell_command", "apply_patch"]
        )]));
        assert!(command_run_turn_has_write_or_status(&[command_run_call(
            &["task_status"]
        )]));
        // Alias spelling normalizes too.
        assert!(command_run_turn_has_write_or_status(&[command_run_call(
            &["apply-patch"]
        )]));
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
                            "task_summary": "finished"
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
        assert!(message["content"].as_str().unwrap().contains("Task 2"));
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

        let content = message["content"].as_str().unwrap();
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

    #[test]
    fn provider_timeout_retry_waits_use_three_step_backoff() {
        assert_eq!(
            provider_timeout_retry_wait(0),
            Some(std::time::Duration::from_secs(5))
        );
        assert_eq!(
            provider_timeout_retry_wait(1),
            Some(std::time::Duration::from_secs(15))
        );
        assert_eq!(
            provider_timeout_retry_wait(2),
            Some(std::time::Duration::from_secs(45))
        );
        assert_eq!(provider_timeout_retry_wait(3), None);
    }

    #[test]
    fn provider_schema_error_removes_rejected_media_content_type() {
        let error = "http status 400: Invalid value: 'input_file'. Supported values are: 'input_text', 'input_image'";
        assert_eq!(provider_unsupported_content_type(error), Some("input_file"));

        let mut messages = vec![json!({
            "type": "function_call_output",
            "call_id": "call_1",
            "output": [
                { "type": "input_text", "text": "kept" },
                { "type": "input_file", "filename": "tone.mp3", "file_data": "data:audio/mpeg;base64,QUJD" },
                { "type": "input_image", "image_url": "data:image/jpeg;base64,AAA" }
            ]
        })];

        let removed = replace_unsupported_content_type_in_messages(&mut messages, "input_file");
        assert_eq!(removed, 1);
        let serialized = serde_json::to_string(&messages).expect("serialize");
        assert!(serialized.contains("Unsupported media omitted"));
        assert!(serialized.contains("input_image"));
        assert!(!serialized.contains("file_data"));
        assert!(!serialized.contains("tone.mp3"));
    }

    #[test]
    fn initial_context_prefix_keeps_only_developer_prefix_without_replaying_history() {
        let initial_messages = vec![
            json!({"role": "developer", "content": "permissions"}),
            json!({"role": "assistant", "content": "old answer"}),
            json!({"type": "function_call", "name": "command_run", "call_id": "call_old"}),
            json!({"type": "function_call_output", "call_id": "call_old", "output": "old output"}),
            json!({"role": "user", "content": "new task"}),
        ];
        let session_messages = vec![
            json!({"role": "user", "content": "new task"}),
            json!({"role": "assistant", "content": "new answer"}),
        ];

        let messages =
            messages_with_initial_context_prefix(&initial_messages, session_messages, "new task");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["role"], "developer");
        assert!(!messages.iter().any(|message| {
            message.get("call_id").and_then(serde_json::Value::as_str) == Some("call_old")
        }));
    }

    #[test]
    fn retry_allowed_failed_runtime_uses_provider_retry_path() {
        let mut runtime = runtime_for_retry_test("retryable-runtime");
        runtime.call_result_status = RuntimeCallResultStatus::Failed;
        runtime.error = Some(RuntimeError {
            error_code: Some("CALL_FAILED".to_string()),
            error_text: Some(
                "all providers failed: openai:gpt-5.1 => network error: error decoding response body"
                    .to_string(),
            ),
            retry_allowed: true,
            fallback_allowed: true,
            fallback_to_id: None,
        });

        assert!(runtime_failure_allows_retry(&runtime));
        assert_eq!(
            runtime_failure_text(&runtime).as_deref(),
            Some(
                "all providers failed: openai:gpt-5.1 => network error: error decoding response body"
            )
        );
    }

    #[test]
    fn non_retryable_failed_runtime_does_not_use_provider_retry_path() {
        let mut runtime = runtime_for_retry_test("non-retryable-runtime");
        runtime.call_result_status = RuntimeCallResultStatus::Failed;
        runtime.error = Some(RuntimeError {
            error_code: Some("CALL_FAILED".to_string()),
            error_text: Some("provider rejected invalid request".to_string()),
            retry_allowed: false,
            fallback_allowed: false,
            fallback_to_id: None,
        });

        assert!(!runtime_failure_allows_retry(&runtime));
        assert_eq!(
            runtime_failure_text(&runtime).as_deref(),
            Some("provider rejected invalid request")
        );
    }

    fn runtime_for_retry_test(runtime_id: &str) -> RuntimeManagement {
        let now = Utc::now();
        let provider = "openai".to_string();
        RuntimeManagement::new(
            runtime_id.to_string(),
            "session-for-retry-test".to_string(),
            "session-for-retry-test".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: provider.clone(),
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: false,
                provider_name: provider.clone(),
                model_name: "gpt-5.1".to_string(),
                provider_url_name: provider.clone(),
                llm_provider_name: provider,
            },
            now,
        )
    }

    #[test]
    fn planning_mode_still_exposes_only_command_run() {
        let tools = vec![
            json!({
                "type": "function",
                "function": { "name": "read_block", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "grep", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "find_definition", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "write_file", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "delete_file", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "apply_diff", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "command_run", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": PLANNING_TOOL, "parameters": { "type": "object" } }
            }),
        ];

        let filtered = filter_tools_for_turn(tools, false, false, false, true)
            .expect("planning filtering should succeed");
        let names = filtered
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .collect::<HashSet<_>>();

        assert_eq!(names.len(), 1);
        assert!(!names.contains("read_block"));
        assert!(!names.contains("grep"));
        assert!(!names.contains("find_definition"));
        assert!(!names.contains("write_file"));
        assert!(!names.contains("delete_file"));
        assert!(!names.contains("apply_diff"));
        assert!(names.contains("command_run"));
        assert!(!names.contains(PLANNING_TOOL));
    }

    #[test]
    fn planning_mode_requires_planning_command() {
        let tools = vec![
            json!({
                "type": "function",
                "function": { "name": "read_block", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "find_definition", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "write_file", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "command_run", "parameters": { "type": "object" } }
            }),
        ];

        let error = require_planning_tool_for_planning_mode(tools)
            .expect_err("planning should be required");

        assert!(error.contains("planning mode requested but planning is unavailable"));
    }

    #[test]
    fn default_non_final_turn_keeps_development_tools_without_planning() {
        let tools = vec![
            json!({
                "type": "function",
                "function": { "name": "read_block", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "write_file", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "delete_file", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "apply_diff", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "command_run", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": PLANNING_TOOL, "parameters": { "type": "object" } }
            }),
        ];

        let filtered = filter_tools_for_turn(tools, false, false, false, false)
            .expect("default filtering should succeed");
        let names = filtered
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .collect::<HashSet<_>>();

        assert_eq!(names.len(), 1);
        assert!(!names.contains("read_block"));
        assert!(!names.contains("write_file"));
        assert!(!names.contains("delete_file"));
        assert!(!names.contains("apply_diff"));
        assert!(names.contains("command_run"));
        assert!(!names.contains(PLANNING_TOOL));
    }

    #[test]
    fn planning_child_turn_keeps_development_tools_and_removes_planning_command() {
        let tools = vec![
            json!({
                "type": "function",
                "function": { "name": "write_file", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": "command_run", "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": PLANNING_TOOL, "parameters": { "type": "object" } }
            }),
        ];

        let filtered = filter_tools_for_turn(tools, false, false, true, true)
            .expect("child filtering should succeed");
        let names = filtered
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .collect::<HashSet<_>>();

        assert_eq!(names.len(), 1);
        assert!(!names.contains("write_file"));
        assert!(names.contains("command_run"));
        assert!(!names.contains(PLANNING_TOOL));
    }

    #[test]
    fn remove_tool_filters_matching_provider_schema() {
        let tools = vec![
            json!({
                "type": "function",
                "function": { "name": PLANNING_TOOL, "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": COMMAND_RUN_TOOL, "parameters": { "type": "object" } }
            }),
        ];

        let filtered = remove_tool(tools, PLANNING_TOOL);
        let names = filtered
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .collect::<HashSet<_>>();

        assert_eq!(names, HashSet::from([COMMAND_RUN_TOOL]));
    }

    #[test]
    fn tool_argument_normalization_removes_runtime_reporting_fields() {
        let normalized = normalize_tool_arguments(json!({
            "reply_message": "done",
            "new_learning": "state changed",
            "step_summary": "summarize"
        }));

        assert_eq!(normalized["reply_message"], "done");
        assert_eq!(normalized["new_learning"], "state changed");
        assert!(normalized.get("step_summary").is_none());
        assert!(normalized.get("last_tool_call_status").is_none());
        assert!(normalized.get("last_tool_call_summary").is_none());
    }

    #[test]
    fn tool_argument_normalization_unwraps_batch_requests() {
        let normalized = normalize_tool_arguments(json!({
            "requests": [
                { "pattern": "*.rs", "directory": "." }
            ],
            "step_summary": "list files"
        }));

        assert_eq!(normalized, json!([{ "pattern": "*.rs", "directory": "." }]));
    }

    #[test]
    fn command_run_tool_keeps_runtime_reporting_fields() {
        let arguments = json!({
            "commands": [
                { "command": "shell_command", "command_line": "pwd" },
                { "command": "shell_command", "command_line": "Write-Output 2" },
                { "command": "shell_command", "command_line": "Write-Output 3" },
                { "command": "shell_command", "command_line": "Write-Output 4" },
                { "command": "shell_command", "command_line": "Write-Output 5" }
            ],
            "step_summary": "Run pwd."
        });

        let normalized = normalize_tool_arguments_for_tool(
            COMMAND_RUN_TOOL,
            arguments.clone(),
            std::path::Path::new("C:/workspace"),
        );

        assert_eq!(normalized, arguments);
    }

    #[test]
    fn command_run_legacy_steps_are_normalized_to_commands() {
        let normalized = normalize_tool_arguments_for_tool(
            COMMAND_RUN_TOOL,
            json!({
                "steps": [
                    {
                        "step": 2,
                        "tool_package_name": "shell_command",
                        "command_code": "Get-ChildItem",
                        "timeout_secs": 15
                    },
                    {
                        "tool_name": "shell_command",
                        "command_code": "Get-Content src/lib.rs -TotalCount 5"
                    }
                ],
                "step_summary": "legacy provider shape"
            }),
            std::path::Path::new("C:/workspace"),
        );

        assert!(normalized.get("steps").is_none());
        assert_eq!(normalized["step_summary"], "legacy provider shape");
        assert_eq!(normalized["commands"][0]["command_type"], "shell_command");
        assert_eq!(normalized["commands"][0]["command_line"], "Get-ChildItem");
        assert_eq!(normalized["commands"][0]["step"], 2);
        assert_eq!(normalized["commands"][0]["timeout_secs"], 15);
        assert_eq!(normalized["commands"][1]["command_type"], "shell_command");
        assert_eq!(normalized["commands"][1]["step"], 1);
    }

    #[test]
    fn user_visible_runtime_text_extracts_reply_message_from_tool_payload() {
        let text = json!({
            "error": null,
            "input": {
                "reply_message": "final answer",
                "new_learning": "",
                "runtime_id": "runtime-1"
            }
        })
        .to_string();

        assert_eq!(
            user_visible_runtime_text(&text).as_deref(),
            Some("final answer")
        );
    }

    #[test]
    fn user_visible_runtime_text_hides_raw_tool_argument_payload() {
        let text = json!({
            "requests": [{
                "path": "services/sd-text-to-image/main.py",
                "start_line": 1,
                "end_line": 250
            }],
            "step_summary": "Read the Stable Diffusion image service main.py to find the port it runs on."
        })
        .to_string();

        assert_eq!(user_visible_runtime_text(&text), None);
    }

    #[test]
    fn prompt_loading_only_includes_agent_prompt() {
        let now = chrono::Utc::now();
        let unique = format!(
            "mano-prompt-test-{:x}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(unique);
        let agent_prompt_dir = root.join("agent-prompts");
        let tool_dir = root.join("tools");
        std::fs::create_dir_all(&agent_prompt_dir).expect("agent prompt dir should be created");
        std::fs::write(agent_prompt_dir.join("prompt.md"), "agent prompt")
            .expect("agent prompt should be written");

        let provider = ProviderConfig {
            tura_llm_name: "test".to_string(),
            stream: false,
            temperature: 0.0,
            max_tokens: 0,
            tool_choice: ToolChoice::Auto,
            time_out_ms: 1_000,
        };
        let validator = ValidatorConfig {
            need_validator: false,
            validator_name: None,
        };
        let mut agent = AgentManagement::new(
            "agent-id".to_string(),
            "agent".to_string(),
            root.clone(),
            None,
            true,
            false,
            provider,
            validator,
            now,
        );
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "agent".to_string(),
                prompt_directory: agent_prompt_dir,
            },
            now,
        );
        agent.add_capability(
            AgentCapabilityItem {
                capability_name: COMMAND_RUN_TOOL.to_string(),
                capability_directory: tool_dir,
            },
            now,
        );

        let messages = load_agent_prompt_messages(&agent).expect("prompt loading should succeed");
        let content = messages
            .iter()
            .filter_map(|message| message.get("content").and_then(|content| content.as_str()))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(content.contains("agent prompt"));
        assert!(!content.contains("command_run prompt"));

        let _ = std::fs::remove_dir_all(root);
    }
}
