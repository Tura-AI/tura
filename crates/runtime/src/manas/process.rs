use crate::gateway_events::{
    publish_gateway_agent_message, publish_runtime_failure_message, publish_runtime_usage_record,
};
use crate::manas::prompt_messages::{
    messages_for_turn, push_no_tool_task_status_retry_message, push_task_status_nudge,
};
use crate::manas::runtime_turn::execute_turn;
use crate::manas::tool_catalog::{command_run_commands_for_agent, planning_child_depth};
use crate::manas::{user_visible_runtime_output_text, user_visible_runtime_text};
use crate::manas::{COMMAND_RUN_TOOL, TASK_STATUS_COMMAND};
use crate::tool_flow::execute::execute_tool_calls;
use chrono::Utc;
use std::thread;
use tracing::{info, warn};
use tura_llm_rust::{
    provider_media_fallback, replace_unsupported_content_type_in_messages, ProviderMediaFallback,
};

use crate::checkpoint::session_snapshot::persist_session_checkpoint;
use crate::context::{accumulate_tool_result_with_provider_metadata, build_context, ContextInput};
use crate::manas::ManasOverrides;
use crate::provider_flow::errors::{
    provider_timeout_retry_wait, runtime_failure_allows_retry, runtime_failure_text,
};
use crate::state_machine::agent_management::{AgentManagement, AgentState};
use crate::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeId, RuntimeManagement,
};
use crate::state_machine::session_management::{SessionManagement, SessionState};
use crate::turn_loop::finalization::create_dummy_runtime;
use crate::turn_loop::no_tool_policy::no_tool_retry_limit;
use crate::turn_loop::provider_step::accumulate_session_from_runtime;
use crate::turn_loop::retry_policy::env_flag;
use crate::turn_loop::task_progress::{
    active_task_user_message, active_todo_task_user_message,
    command_run_result_terminal_task_status, command_run_turn_has_write_or_status,
    record_task_focus_message, record_task_focus_message_for_terminal_done,
    terminal_task_status_final_message, NO_WRITE_COMMAND_RUN_NUDGE_THRESHOLD,
};
use crate::turn_loop::tool_step::{apply_compact_context_results, command_run_results_empty};

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
            let terminal_task_status_seen = terminal_task_status.is_some();

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
                let final_response_published = run_terminal_final_response_turn(
                    agents,
                    session,
                    &current_messages,
                    redis_url,
                    &original_user_task,
                )?;
                if !final_response_published {
                    if let Some(content) = terminal_task_status_final_message(
                        session,
                        terminal_task_status.as_deref().unwrap_or("done"),
                        &original_user_task,
                    ) {
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
                                "failed to publish terminal task_status assistant message"
                            );
                        }
                    }
                }
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

fn run_terminal_final_response_turn(
    agents: &[AgentManagement],
    session: &mut SessionManagement,
    current_messages: &[serde_json::Value],
    redis_url: &str,
    original_user_task: &str,
) -> Result<bool, String> {
    let mut final_messages = messages_for_turn(current_messages, session, original_user_task);
    final_messages.push(serde_json::json!({
        "role": "system",
        "content": "The task was marked done. Now send the user-facing assistant reply directly, without calling tools and without mentioning task_status, command_run, or internal status updates.",
    }));
    let (runtime, _tool_calls) = execute_turn(
        agents,
        session,
        &final_messages,
        redis_url,
        false,
        true,
        true,
    )?;
    let visible_text = user_visible_runtime_text(&runtime.text).or_else(|| {
        runtime
            .output
            .as_ref()
            .and_then(user_visible_runtime_output_text)
    });
    let has_visible_text = visible_text
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    if let Some(content) = visible_text.filter(|text| !text.trim().is_empty()) {
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
                "failed to publish terminal final response assistant message"
            );
        }
    }
    accumulate_session_from_runtime(session, &runtime, true)?;
    publish_runtime_usage_record(session, &runtime);
    session.increment_turn(Utc::now());
    persist_session_checkpoint(session, "terminal_final_response");
    Ok(has_visible_text)
}

#[cfg(test)]
mod tests {
    use super::messages_with_initial_context_prefix;
    use serde_json::json;

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
}
