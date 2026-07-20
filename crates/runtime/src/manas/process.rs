use crate::gateway_events::{
    publish_gateway_agent_message_from_runtime, publish_runtime_failure_message,
    publish_runtime_usage_record,
};
use crate::manas::constants::PLANNING_TOOL;
use crate::manas::prompt_messages::push_no_tool_task_status_retry_message;
use crate::manas::runtime_turn::execute_turn;
use crate::manas::tool_catalog::{command_run_commands_for_agent, planning_child_depth};
use crate::manas::TASK_STATUS_COMMAND;
use crate::manas::{user_visible_runtime_output_text, user_visible_runtime_text};
use crate::prompt_style::{
    provider_retry, runtime_prompt_manual, tail_injection, terminal_final_response,
};
use crate::tool_callback_sanitizer::sanitize_tool_callback_output;
use crate::tool_flow::execute::execute_tool_calls;
use chrono::Utc;
use std::thread;
use tracing::{info, warn};
use tura_llm_rust::{
    provider_media_fallback, replace_unsupported_content_type_in_messages, ProviderMediaFallback,
};

use crate::checkpoint::session_snapshot::persist_session_checkpoint;
use crate::context::{
    accumulate_tool_result_with_provider_metadata, build_context,
    compact_session_context_automatically_with_capabilities,
    compact_session_context_with_agent_message_and_capabilities, estimated_tokens_from_bytes_u64,
    CompactContextAgentMessage, ContextInput,
};
use crate::manas::ManasOverrides;
use crate::provider_flow::errors::{
    provider_timeout_retry_wait, runtime_failure_allows_retry, runtime_failure_text,
};
use crate::state_machine::agent_management::{AgentManagement, AgentState};
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;
use crate::turn_loop::finalization::create_dummy_runtime;
use crate::turn_loop::no_tool_policy::no_tool_retry_limit;
use crate::turn_loop::provider_step::accumulate_session_from_runtime;
use crate::turn_loop::retry_policy::env_flag;
use crate::turn_loop::task_progress::{
    active_doing_task_user_message, active_task_user_message, command_run_result_has_command,
    command_run_result_is_single_task_status, command_run_result_terminal_task_status,
    record_task_focus_message, record_task_focus_message_for_terminal_done,
};
use crate::turn_loop::tool_step::{command_run_results_empty, extract_compact_context_results};
use lifecycle::{PlanStatus, RuntimeId, RuntimeState, SessionState};

const DEFAULT_MANAS_MAX_TURNS: u64 = 256;
const DONE_TASK_STATUS_LONG_REPLY_BACKFILL_CUTOFF: usize = 1_000;

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
    pub final_error: Option<String>,
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

    let agent_commands = agents.first().map(command_run_commands_for_agent);
    if let Some(commands) = agent_commands.as_ref() {
        session.record_session_capabilities(commands.iter().map(String::as_str));
    }
    session.transition(SessionState::Running, Utc::now())?;
    persist_session_checkpoint(session, "running");

    let active_agent_capabilities = agent_commands
        .as_ref()
        .map(|commands| commands.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut current_messages = initial_messages.clone();
    let mut last_runtime_id: Option<RuntimeId> = None;
    let original_user_task = session.input.user_input.clone();
    let mut turn = 0_u64;
    let mut provider_timeout_retries = 0_u8;
    let mut no_tool_retries = 0_u64;
    let mut final_session_state = SessionState::Completed;
    let mut final_error: Option<String> = None;
    let supports_task_status = agent_commands
        .as_ref()
        .is_some_and(|commands| commands.contains(TASK_STATUS_COMMAND));
    let supports_planning = agent_commands
        .as_ref()
        .is_some_and(|commands| commands.contains(PLANNING_TOOL));
    loop {
        turn = turn.saturating_add(1);
        if turn > manas_max_turns() {
            warn!(
                session_id = %session.session_id,
                turn = turn,
                max_turns = manas_max_turns(),
                "manas turn limit reached; failing session"
            );
            let error = format!(
                "Session stopped after reaching the maximum turn limit of {}.",
                manas_max_turns()
            );
            publish_runtime_failure_message(
                session,
                last_runtime_id.as_deref().unwrap_or_default(),
                &error,
            );
            final_error = Some(error);
            final_session_state = SessionState::Failed;
            break;
        }
        info!(
            session_id = %session.session_id,
            turn = turn,
            "starting turn"
        );
        append_active_runtime_prompt_manual_context(session, &mut current_messages)?;

        let runtime_result = match execute_turn(
            agents,
            session,
            &current_messages,
            &original_user_task,
            None,
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
                if env_flag("TURA_RUNTIME_ERRORS_FATAL") {
                    return Err(error);
                }
                final_error = Some(error);
                final_session_state = SessionState::Failed;
                break;
            }
        };

        let runtime = runtime_result.0;
        let tool_calls = runtime_result.1;

        last_runtime_id = Some(runtime.runtime_id.clone());

        accumulate_session_from_runtime(session, &runtime, true)?;
        increment_turn_with_fresh_timestamp(session);
        persist_session_checkpoint(session, "runtime");
        publish_runtime_usage_record(session, &runtime);

        if runtime.state == RuntimeState::TimedOut || runtime_failure_allows_retry(&runtime) {
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
                    let error = format!(
                        "Provider/model does not support `{content_type}` media input for this request. Use an image-capable model or a route whose model metadata includes that input modality. Original provider error: {error_text}"
                    );
                    publish_runtime_failure_message(session, &runtime.runtime_id, &error);
                    final_error = Some(error);
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
                    status = ?runtime.call_result_status(),
                    error = %error_text,
                    retry = provider_timeout_retries,
                    wait_ms = wait_duration.as_millis(),
                    "provider runtime failed transiently; waiting before retrying with full tool set"
                );
                thread::sleep(wait_duration);
                if let Some((content_type, removed)) = removed_media {
                    tail_injection::append_tail_prompt(
                        &mut current_messages,
                        tail_injection::TailPrompt::developer(provider_retry::media_fallback(
                            content_type,
                            removed,
                        )),
                    );
                }
                tail_injection::append_tail_prompt(
                    &mut current_messages,
                    tail_injection::TailPrompt::developer(provider_retry::transient_failure_retry(
                        &error_text,
                        provider_timeout_retries,
                        3,
                    )),
                );
                continue;
            }

            warn!(
                session_id = %session.session_id,
                turn = turn,
                runtime_id = %runtime.runtime_id,
                status = ?runtime.call_result_status(),
                error = %error_text,
                retries = provider_timeout_retries,
                "provider runtime failed transiently after retries; publishing visible failure"
            );
            let error = format!(
                "Provider runtime failed after 3 retries before completing the task: {error_text}"
            );
            publish_runtime_failure_message(session, &runtime.runtime_id, &error);
            final_error = Some(error);
            final_session_state = SessionState::Failed;
            break;
        }
        if runtime.state == RuntimeState::Failed {
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
            if env_flag("TURA_RUNTIME_ERRORS_FATAL") {
                return Err(error_text);
            }
            final_error = Some(error_text);
            final_session_state = SessionState::Failed;
            break;
        }

        if !tool_calls.is_empty() {
            let visible_reply_before_tool = visible_runtime_reply(&runtime);
            let visible_reply_published_before_terminal_status =
                visible_reply_before_tool.is_some();
            if let Some(content) = visible_reply_before_tool.as_deref() {
                if let Err(error) = publish_gateway_agent_message_from_runtime(
                    &session.session_id,
                    &runtime,
                    content.to_string(),
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
            let pending_compact_contexts =
                extract_compact_context_results(&mut tool_results, Some(&runtime));
            let terminal_task_status = tool_results
                .iter()
                .find_map(|result| command_run_result_terminal_task_status(&result.result));
            let terminal_status_followed_command = tool_results
                .iter()
                .any(|result| command_run_result_has_command(&result.result));

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
                    Some(&runtime.runtime_id),
                    tool_calls
                        .get(index)
                        .and_then(|tool_call| tool_call.provider_metadata.clone()),
                )?;
            }
            persist_session_checkpoint(session, "tool_results");

            if pending_compact_contexts.is_empty()
                && should_end_turn_without_task_status_backfill(
                    &tool_results,
                    terminal_task_status.as_deref(),
                    visible_reply_before_tool.as_deref(),
                )
            {
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    runtime_id = %runtime.runtime_id,
                    "single done task_status followed a long visible assistant reply; ending turn without tool-result backfill"
                );
                break;
            }

            if !pending_compact_contexts.is_empty() {
                for pending in &pending_compact_contexts {
                    compact_session_context_with_agent_message_and_capabilities(
                        session,
                        &pending.summary,
                        pending.agent_message_content.as_deref().map(|content| {
                            CompactContextAgentMessage {
                                content,
                                timestamp: pending.agent_message_timestamp,
                            }
                        }),
                        &active_agent_capabilities,
                    )?;
                }
                persist_session_checkpoint(session, "compact_context");
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    runtime_id = %runtime.runtime_id,
                    "compact_context applied after persisted tool results; continuing task with rebuilt compacted context"
                );

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
                continue;
            }

            if let Some(summary) =
                auto_compact_summary_after_new_context(session, &runtime, &tool_results)
            {
                compact_session_context_automatically_with_capabilities(
                    session,
                    &summary,
                    &active_agent_capabilities,
                )?;
                persist_session_checkpoint(session, "auto_compact_context");
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    runtime_id = %runtime.runtime_id,
                    "automatic context compaction applied after new tool context exceeded active limit; continuing task"
                );

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
                continue;
            }

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
            if should_auto_complete_non_planning_doing_after_tool_turn(
                session.goal_mode,
                supports_planning,
                terminal_task_status.as_deref(),
                visible_reply_published_before_terminal_status,
                terminal_status_followed_command,
            ) {
                if complete_active_doing_task_after_non_planning_reply(session, true) {
                    persist_session_checkpoint(session, "task_auto_completed");
                }
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    supports_planning = supports_planning,
                    supports_task_status = supports_task_status,
                    "non-planning agent returned visible final text without a command_run result that needs backfill; active task was auto-completed and loop ended"
                );
                break;
            }
            if matches!(terminal_task_status.as_deref(), Some("done" | "question")) {
                let final_response_published = if terminal_status_needs_final_response_turn(
                    terminal_task_status.as_deref(),
                    visible_reply_published_before_terminal_status,
                    terminal_status_followed_command,
                ) {
                    run_terminal_final_response_turn(
                        agents,
                        session,
                        &current_messages,
                        redis_url,
                        &original_user_task,
                    )?
                } else {
                    true
                };
                if !final_response_published {
                    warn!(
                        session_id = %session.session_id,
                        runtime_id = %runtime.runtime_id,
                        status = terminal_task_status.as_deref().unwrap_or("unknown"),
                        "terminal task_status produced no user-facing reply; suppressing internal fallback text"
                    );
                }
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    status = terminal_task_status.as_deref().unwrap_or("unknown"),
                    "terminal task_status returned and no next executable task exists; ending loop"
                );
                break;
            } else if terminal_task_status.as_deref() == Some("doing") {
                if let Some(next_task) = active_doing_task_user_message(session) {
                    record_task_focus_message_for_terminal_done(session, &next_task, false);
                    persist_session_checkpoint(session, "task_focus");
                }
            } else if let Some(next_task) = active_task_user_message(session) {
                record_task_focus_message_for_terminal_done(session, &next_task, false);
                persist_session_checkpoint(session, "task_focus");
            }
        } else {
            if let Some(summary) = auto_compact_summary_after_new_context(session, &runtime, &[]) {
                compact_session_context_automatically_with_capabilities(
                    session,
                    &summary,
                    &active_agent_capabilities,
                )?;
                persist_session_checkpoint(session, "auto_compact_context");
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    runtime_id = %runtime.runtime_id,
                    "automatic context compaction applied after new assistant context exceeded active limit; continuing task"
                );

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
                continue;
            }

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

            let has_visible_reply = visible_runtime_reply(&runtime).is_some();

            let has_active_doing_task = active_doing_task_user_message(session).is_some();
            if has_visible_reply {
                if complete_active_doing_task_after_non_planning_reply(
                    session,
                    !session.goal_mode && !supports_planning,
                ) {
                    persist_session_checkpoint(session, "task_auto_completed");
                }
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    supports_planning = supports_planning,
                    supports_task_status = supports_task_status,
                    has_active_doing_task = has_active_doing_task,
                    "turn completed without command_run but produced a user-visible reply; ending session after any required task_status backfill"
                );
                break;
            }
            if !has_active_doing_task {
                if should_retry_no_tool_task_status(
                    session,
                    supports_planning,
                    supports_task_status,
                    false,
                ) && should_continue_no_tool_task_status_retry(session, no_tool_retries)
                {
                    no_tool_retries = no_tool_retries.saturating_add(1);
                    push_no_tool_task_status_retry_message(&mut current_messages, session);
                    warn!(
                        session_id = %session.session_id,
                        turn = turn,
                        runtime_id = %runtime.runtime_id,
                        no_tool_retries = no_tool_retries,
                        "goal-mode turn returned no tool calls and no task_status marker; retrying until task_status settles the goal"
                    );
                    continue;
                }
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    "turn completed without command_run and no task_status doing marker; ending session"
                );
                break;
            }

            if !session.goal_mode && !supports_planning {
                if complete_active_doing_task_after_non_planning_reply(session, false) {
                    persist_session_checkpoint(session, "task_auto_completed");
                }
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    supports_planning = supports_planning,
                    supports_task_status = supports_task_status,
                    "turn completed without command_run while task_status is still active; non-planning agent ended and active task was settled when a visible reply existed"
                );
                break;
            }

            if !should_retry_no_tool_task_status(
                session,
                supports_planning,
                supports_task_status,
                true,
            ) {
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    goal_mode = session.goal_mode,
                    supports_planning = supports_planning,
                    supports_task_status = supports_task_status,
                    "turn completed without command_run while task_status is still active; retry mode is not enabled or task_status is unavailable"
                );
                break;
            }

            if should_continue_no_tool_task_status_retry(session, no_tool_retries) {
                no_tool_retries = no_tool_retries.saturating_add(1);
                push_no_tool_task_status_retry_message(&mut current_messages, session);
                if let Some(next_task) = active_doing_task_user_message(session) {
                    record_task_focus_message(session, &next_task);
                    persist_session_checkpoint(session, "task_focus");
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

    session.transition(final_session_state, Utc::now())?;
    persist_session_checkpoint(
        session,
        if final_session_state == SessionState::Failed {
            "failed"
        } else {
            "completed"
        },
    );
    let git_event = if final_session_state == SessionState::Failed {
        "failed"
    } else {
        "completed"
    };
    commit_terminal_session_checkpoint(session, git_event);

    for agent in agents.iter_mut() {
        agent.state = if final_session_state == SessionState::Failed {
            AgentState::Failed
        } else {
            AgentState::Completed
        };
        agent.updated_at = Utc::now();
    }

    let final_runtime = create_dummy_runtime(last_runtime_id.unwrap_or_default(), session)?;

    Ok(ManasResult {
        agents: agents.to_vec(),
        session: session.clone(),
        final_runtime,
        final_error,
    })
}

fn commit_terminal_session_checkpoint(session: &SessionManagement, git_event: &str) -> bool {
    if crate::router_command_run::command_run_sandbox_enabled() {
        info!(
            session_id = %session.session_id,
            event = git_event,
            "skipping workspace session checkpoint commit because command_run sandbox is enabled"
        );
        return false;
    }

    match crate::workspace_git::commit_session_checkpoint(session, git_event) {
        Ok(Some(commit)) => info!(
            session_id = %session.session_id,
            commit = %commit,
            event = git_event,
            "committed workspace session checkpoint"
        ),
        Ok(None) => info!(
            session_id = %session.session_id,
            event = git_event,
            "workspace session checkpoint commit completed without a resolved hash"
        ),
        Err(error) => warn!(
            session_id = %session.session_id,
            event = git_event,
            error = %error,
            "failed to commit workspace session checkpoint"
        ),
    }
    true
}

fn manas_max_turns() -> u64 {
    std::env::var("TURA_MANAS_MAX_TURNS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MANAS_MAX_TURNS)
}

fn increment_turn_with_fresh_timestamp(session: &mut SessionManagement) {
    session.increment_turn(Utc::now());
}

fn should_retry_no_tool_task_status(
    session: &SessionManagement,
    supports_planning: bool,
    supports_task_status: bool,
    has_active_doing_task: bool,
) -> bool {
    if !supports_task_status {
        return false;
    }
    if session.goal_mode {
        return true;
    }
    supports_planning && has_active_doing_task
}

fn should_continue_no_tool_task_status_retry(
    session: &SessionManagement,
    no_tool_retries: u64,
) -> bool {
    if session.goal_mode {
        return true;
    }
    no_tool_retries < u64::from(no_tool_retry_limit())
}

fn complete_active_doing_task_after_non_planning_reply(
    session: &mut SessionManagement,
    has_visible_reply: bool,
) -> bool {
    if !has_visible_reply {
        return false;
    }
    let Some(task) = session
        .task_plan
        .detailed_tasks
        .iter_mut()
        .find(|task| task.status == PlanStatus::Doing)
    else {
        return false;
    };
    task.status = PlanStatus::Done;
    session.session_last_update_at = Utc::now();
    true
}

fn should_auto_complete_non_planning_doing_after_tool_turn(
    _goal_mode: bool,
    _supports_planning: bool,
    _terminal_task_status: Option<&str>,
    _visible_reply_already_published: bool,
    _terminal_status_followed_command: bool,
) -> bool {
    // A `doing` task_status is only a progress update. It must be replayed to the
    // next model turn so newly activated manuals and task context can be used.
    false
}

fn should_end_turn_without_task_status_backfill(
    tool_results: &[crate::tool_router::execute_tool::ToolExecutionResult],
    terminal_task_status: Option<&str>,
    visible_reply: Option<&str>,
) -> bool {
    let Some(status @ ("done" | "question")) = terminal_task_status else {
        return false;
    };

    visible_reply.is_some_and(|reply| reply.len() > DONE_TASK_STATUS_LONG_REPLY_BACKFILL_CUTOFF)
        && command_run_has_only_terminal_task_status_result(tool_results, status)
}

fn command_run_has_only_terminal_task_status_result(
    tool_results: &[crate::tool_router::execute_tool::ToolExecutionResult],
    status: &str,
) -> bool {
    let [tool_result] = tool_results else {
        return false;
    };
    tool_result.tool_name == crate::manas::COMMAND_RUN_TOOL
        && command_run_result_is_single_task_status(&tool_result.result, status)
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

fn append_active_runtime_prompt_manual_context(
    session: &mut SessionManagement,
    current_messages: &mut Vec<serde_json::Value>,
) -> Result<(), String> {
    runtime_prompt_manual::append_missing_runtime_prompt_manuals(session, Some(current_messages))
        .map(|_| ())
}

fn run_terminal_final_response_turn(
    agents: &[AgentManagement],
    session: &mut SessionManagement,
    current_messages: &[serde_json::Value],
    redis_url: &str,
    original_user_task: &str,
) -> Result<bool, String> {
    let (runtime, _tool_calls) = execute_turn(
        agents,
        session,
        current_messages,
        original_user_task,
        Some(terminal_final_response::TERMINAL_FINAL_RESPONSE),
        redis_url,
        false,
        true,
        true,
    )?;
    let visible_text = visible_runtime_reply(&runtime);
    let has_visible_text = visible_text
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    let visible_text = visible_text.filter(|text| !text.trim().is_empty());
    accumulate_session_from_runtime(session, &runtime, true)?;
    session.increment_turn(Utc::now());
    persist_session_checkpoint(session, "terminal_final_response");
    if let Some(content) = visible_text {
        if let Err(error) = publish_gateway_agent_message_from_runtime(
            &session.session_id,
            &runtime,
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
    publish_runtime_usage_record(session, &runtime);
    Ok(has_visible_text)
}

fn visible_runtime_reply(runtime: &RuntimeManagement) -> Option<String> {
    user_visible_runtime_text(&runtime.text)
        .or_else(|| {
            runtime
                .output
                .as_ref()
                .and_then(user_visible_runtime_output_text)
        })
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn auto_compact_summary_after_new_context(
    session: &SessionManagement,
    runtime: &RuntimeManagement,
    tool_results: &[crate::tool_router::execute_tool::ToolExecutionResult],
) -> Option<String> {
    let limit = session.context_tokens.limit;
    if limit == 0 {
        return None;
    }
    let added_tokens = estimated_new_context_tokens(runtime, tool_results);
    if added_tokens == 0 {
        return None;
    }
    let input_tokens = session.context_tokens.input;
    let projected = input_tokens.saturating_add(added_tokens);
    if projected <= limit {
        return None;
    }
    Some(format!(
        "Automatic context checkpoint: provider input was about {input_tokens} tokens, newly persisted context is estimated at about {added_tokens} tokens by bytes/4, and the projected total {projected} exceeds the active context limit {limit}. Continue the same task from the retained timeline above; preserve completed commands and validation results, and do not rerun work unless the current task requires it."
    ))
}

fn estimated_new_context_tokens(
    runtime: &RuntimeManagement,
    tool_results: &[crate::tool_router::execute_tool::ToolExecutionResult],
) -> u64 {
    let visible_bytes = visible_runtime_reply(runtime)
        .map(|text| text.len() as u64)
        .unwrap_or(0);
    let tool_bytes = tool_results
        .iter()
        .map(|result| {
            let mut result = result.clone();
            result.result = sanitize_tool_callback_output(&result.result);
            serde_json::to_string(&result)
                .map(|text| text.len() as u64)
                .unwrap_or(0)
        })
        .sum::<u64>();
    estimated_tokens_from_bytes_u64(visible_bytes.saturating_add(tool_bytes))
}

fn terminal_status_needs_final_response_turn(
    terminal_task_status: Option<&str>,
    visible_reply_already_published: bool,
    terminal_status_followed_command: bool,
) -> bool {
    match terminal_task_status {
        Some("done") => true,
        Some("question") => !visible_reply_already_published || terminal_status_followed_command,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        auto_compact_summary_after_new_context, commit_terminal_session_checkpoint,
        complete_active_doing_task_after_non_planning_reply, increment_turn_with_fresh_timestamp,
        manas_max_turns, messages_with_initial_context_prefix,
        should_auto_complete_non_planning_doing_after_tool_turn,
        should_continue_no_tool_task_status_retry, should_end_turn_without_task_status_backfill,
        should_retry_no_tool_task_status, terminal_status_needs_final_response_turn,
        DEFAULT_MANAS_MAX_TURNS,
    };
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{
        RuntimeManagement, RuntimeProviderConfig, UsageReport,
    };
    use crate::state_machine::session_management::{
        PlanStatus, SessionInput, SessionManagement, TaskStep,
    };
    use crate::tool_router::execute_tool::ToolExecutionResult;
    use crate::turn_loop::no_tool_policy::no_tool_retry_limit;
    use chrono::{Duration, Utc};
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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
    fn manas_max_turns_uses_positive_env_override_and_ignores_invalid_values() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let previous = std::env::var_os("TURA_MANAS_MAX_TURNS");

        std::env::set_var("TURA_MANAS_MAX_TURNS", "3");
        assert_eq!(manas_max_turns(), 3);

        std::env::set_var("TURA_MANAS_MAX_TURNS", "0");
        assert_eq!(manas_max_turns(), DEFAULT_MANAS_MAX_TURNS);

        std::env::set_var("TURA_MANAS_MAX_TURNS", "not-a-number");
        assert_eq!(manas_max_turns(), DEFAULT_MANAS_MAX_TURNS);

        if let Some(previous) = previous {
            std::env::set_var("TURA_MANAS_MAX_TURNS", previous);
        } else {
            std::env::remove_var("TURA_MANAS_MAX_TURNS");
        }
    }

    #[test]
    fn completed_turn_timestamp_does_not_regress_after_runtime_log_update() {
        let started_at = Utc::now() - Duration::minutes(5);
        let mut session = SessionManagement::new(
            "session-timestamp".to_string(),
            "Timestamp".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "work".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "work".to_string(),
            started_at,
        );
        let runtime_log_at = Utc::now();
        session.push_log("runtime output", runtime_log_at);

        increment_turn_with_fresh_timestamp(&mut session);

        assert_eq!(session.session_current_turn, 1);
        assert!(
            session.session_last_update_at >= runtime_log_at,
            "turn completion must not restore the stale session start timestamp"
        );
    }

    #[test]
    fn non_planning_visible_reply_auto_completes_active_doing_task() {
        let mut session = test_session("session-auto-done");
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "active".to_string(),
            step: 1,
            task_summary: "Answer directly".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });

        assert!(complete_active_doing_task_after_non_planning_reply(
            &mut session,
            true,
        ));

        assert_eq!(session.task_plan.detailed_tasks[0].status, PlanStatus::Done);
    }

    #[test]
    fn non_planning_without_visible_reply_keeps_active_doing_task_open() {
        let mut session = test_session("session-no-visible-reply");
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "active".to_string(),
            step: 1,
            task_summary: "Wait for real output".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });

        assert!(!complete_active_doing_task_after_non_planning_reply(
            &mut session,
            false,
        ));

        assert_eq!(
            session.task_plan.detailed_tasks[0].status,
            PlanStatus::Doing
        );
    }

    #[test]
    fn non_planning_tool_turn_auto_completes_only_visible_status_only_doing() {
        assert!(!should_auto_complete_non_planning_doing_after_tool_turn(
            false,
            false,
            Some("doing"),
            true,
            false,
        ));
        assert!(!should_auto_complete_non_planning_doing_after_tool_turn(
            false,
            true,
            Some("doing"),
            true,
            false,
        ));
        assert!(!should_auto_complete_non_planning_doing_after_tool_turn(
            false,
            false,
            Some("doing"),
            false,
            false,
        ));
        assert!(!should_auto_complete_non_planning_doing_after_tool_turn(
            false,
            false,
            Some("doing"),
            true,
            true,
        ));
        assert!(!should_auto_complete_non_planning_doing_after_tool_turn(
            false,
            false,
            Some("done"),
            true,
            false,
        ));
        assert!(!should_auto_complete_non_planning_doing_after_tool_turn(
            true,
            false,
            Some("doing"),
            true,
            false,
        ));
    }

    #[test]
    fn no_tool_retry_uses_goal_mode_instead_of_planning_capability() {
        let mut session = test_session("session-goal-retry");

        assert!(!should_retry_no_tool_task_status(
            &session, true, true, false
        ));
        assert!(should_retry_no_tool_task_status(&session, true, true, true));
        assert!(!should_retry_no_tool_task_status(
            &session, false, true, true
        ));

        session.goal_mode = true;
        assert!(should_retry_no_tool_task_status(
            &session, false, true, false
        ));
        assert!(!should_retry_no_tool_task_status(
            &session, true, false, true
        ));
    }

    #[test]
    fn goal_mode_no_tool_retry_ignores_retry_limit() {
        let mut session = test_session("session-goal-retry-budget");
        session.goal_mode = true;

        assert!(should_continue_no_tool_task_status_retry(&session, 0));
        assert!(should_continue_no_tool_task_status_retry(&session, 20));
        assert!(should_continue_no_tool_task_status_retry(&session, 10_000));

        session.goal_mode = false;
        assert!(should_continue_no_tool_task_status_retry(&session, 0));
        assert!(!should_continue_no_tool_task_status_retry(
            &session,
            u64::from(no_tool_retry_limit())
        ));
    }

    #[test]
    fn auto_compact_summary_triggers_when_new_context_estimate_exceeds_limit() {
        let mut session = test_session("session-auto-compact");
        session.context_tokens.input = 950;
        session.context_tokens.limit = 1_000;
        let runtime = test_runtime_with_usage(&session, 950);
        let tool_results = vec![ToolExecutionResult {
            tool_name: "command_run".to_string(),
            arguments: json!({"commands":[{"command_type":"shell_command","command_line":"probe"}]}),
            result: json!({"results":[{"step":1,"command_type":"shell_command","success":true,"output":"X".repeat(300)}]}),
            success: true,
            error: None,
        }];

        let summary = auto_compact_summary_after_new_context(&session, &runtime, &tool_results)
            .expect("new context should force automatic compaction");

        assert!(summary.contains("Automatic context checkpoint"));
        assert!(summary.contains("bytes/4"));
        assert!(summary.contains("active context limit 1000"));
    }

    #[test]
    fn auto_compact_summary_does_not_trigger_when_projection_fits_limit() {
        let mut session = test_session("session-auto-compact-fits");
        session.context_tokens.input = 100;
        session.context_tokens.limit = 1_000;
        let runtime = test_runtime_with_usage(&session, 100);

        assert!(auto_compact_summary_after_new_context(&session, &runtime, &[]).is_none());
    }

    #[test]
    fn long_visible_terminal_task_status_can_end_without_backfill_only_when_single_status_call() {
        let status_result = ToolExecutionResult {
            tool_name: "command_run".to_string(),
            arguments: json!({"commands":[{"command_type":"task_status"}]}),
            result: json!({"results":[{
                "command_type":"task_status",
                "success":true,
                "output":{"task_status":{"status":"done"}}
            }]}),
            success: true,
            error: None,
        };
        let question_status_result = ToolExecutionResult {
            tool_name: "command_run".to_string(),
            arguments: json!({"commands":[{"command_type":"task_status"}]}),
            result: json!({"results":[{
                "command_type":"task_status",
                "success":true,
                "output":{"task_status":{"status":"question"}}
            }]}),
            success: true,
            error: None,
        };
        let doing_status_result = ToolExecutionResult {
            tool_name: "command_run".to_string(),
            arguments: json!({"commands":[{"command_type":"task_status"}]}),
            result: json!({"results":[{
                "command_type":"task_status",
                "success":true,
                "output":{"task_status":{"status":"doing"}}
            }]}),
            success: true,
            error: None,
        };
        let long_reply = "x".repeat(1_001);

        assert!(should_end_turn_without_task_status_backfill(
            std::slice::from_ref(&status_result),
            Some("done"),
            Some(&long_reply),
        ));
        assert!(!should_end_turn_without_task_status_backfill(
            std::slice::from_ref(&status_result),
            Some("done"),
            Some(&"x".repeat(1_000)),
        ));
        assert!(should_end_turn_without_task_status_backfill(
            std::slice::from_ref(&question_status_result),
            Some("question"),
            Some(&long_reply),
        ));
        assert!(!should_end_turn_without_task_status_backfill(
            std::slice::from_ref(&doing_status_result),
            Some("doing"),
            Some(&long_reply),
        ));

        let mut with_command = status_result.clone();
        with_command.result = json!({"results":[
            {"command_type":"shell_command","success":true,"output":"ok"},
            {"command_type":"task_status","success":true,"output":{"task_status":{"status":"done"}}}
        ]});
        assert!(!should_end_turn_without_task_status_backfill(
            &[with_command],
            Some("done"),
            Some(&long_reply),
        ));

        let multibyte_reply_over_byte_cutoff = "完".repeat(400);
        assert!(should_end_turn_without_task_status_backfill(
            &[status_result],
            Some("done"),
            Some(&multibyte_reply_over_byte_cutoff),
        ));
    }

    #[test]
    fn terminal_done_skips_final_response_turn_when_reply_is_already_visible() {
        assert!(terminal_status_needs_final_response_turn(
            Some("done"),
            true,
            false,
        ));
        assert!(terminal_status_needs_final_response_turn(
            Some("done"),
            false,
            false,
        ));
        assert!(terminal_status_needs_final_response_turn(
            Some("done"),
            true,
            true,
        ));
        assert!(!terminal_status_needs_final_response_turn(
            Some("question"),
            true,
            false,
        ));
        assert!(terminal_status_needs_final_response_turn(
            Some("question"),
            false,
            false,
        ));
        assert!(terminal_status_needs_final_response_turn(
            Some("question"),
            true,
            true,
        ));
        assert!(!terminal_status_needs_final_response_turn(
            Some("doing"),
            false,
            true,
        ));
        assert!(!terminal_status_needs_final_response_turn(None, true, true));
    }

    #[test]
    fn terminal_checkpoint_commit_skips_when_command_run_sandbox_is_enabled() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let _sandbox = EnvGuard::set("TURA_COMMAND_RUN_SANDBOX", "enabled");
        let temp = tempfile::tempdir().expect("temp workspace");
        std::fs::write(temp.path().join("src.txt"), "sandboxed change").expect("fixture file");
        let mut session = test_session("session-sandbox-checkpoint");
        session.session_directory = temp.path().to_path_buf();

        assert!(!commit_terminal_session_checkpoint(&session, "completed"));
        assert!(
            !temp.path().join(".git").exists(),
            "sandboxed runtime completion must not initialize git or create checkpoint commits"
        );
    }

    fn test_session(id: &str) -> SessionManagement {
        let now = Utc::now();
        SessionManagement::new(
            id.to_string(),
            "Test".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "work".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "work".to_string(),
            now,
        )
    }

    fn test_runtime_with_usage(
        session: &SessionManagement,
        input_tokens: u64,
    ) -> RuntimeManagement {
        let mut runtime = RuntimeManagement::new(
            format!("runtime-{}", session.session_id),
            session.session_id.clone(),
            "agent-test".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "provider".to_string(),
                    default_model_tier: None,
                    current_model: None,
                    stream: false,
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
            Utc::now(),
        );
        runtime.usage = Some(UsageReport {
            input_tokens,
            output_tokens: 0,
            total_tokens: input_tokens,
            cached_input_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            attachment_input_tokens: 0,
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost: 0.0,
            currency: "USD".to_string(),
            pricing_source: "test".to_string(),
            latency_ms: 0,
            time_to_first_token_ms: 0,
            token_per_second: 0.0,
        });
        runtime
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
