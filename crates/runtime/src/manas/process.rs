use super::constants::{COMMAND_RUN_TOOL, TASK_DELIVERED_TOOL};
use super::final_response::user_visible_runtime_text;
use super::gateway_events::{publish_runtime_failure_message, publish_runtime_usage_record};
use super::prompt_messages::{messages_for_turn, push_task_continuity_message};
use super::runtime_turn::execute_turn;
use super::tool_catalog::{multiple_tasks_child_depth, multiple_tasks_env_enabled};
use super::tool_execution::execute_tool_calls;
use chrono::Utc;
use std::{
    collections::HashSet,
    io::Write,
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};
use tracing::{info, warn};

use crate::context::{accumulate_tool_result_with_feedback, build_context, ContextInput};
use crate::manas::ManasOverrides;
use crate::mano::persist_gateway_session;
use crate::state_machine::agent_management::{AgentManagement, AgentState};
use crate::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeId, RuntimeManagement,
};
use crate::state_machine::session_management::TaskStatus;
use crate::state_machine::session_management::{SessionManagement, SessionState};

#[cfg(test)]
use super::constants::MULTIPLE_TASKS_TOOL;
#[cfg(test)]
use super::tool_arguments::{normalize_tool_arguments, normalize_tool_arguments_for_tool};
#[cfg(test)]
use super::tool_catalog::{
    filter_tools_for_turn, load_agent_prompt_messages, remove_tool,
    require_multiple_tasks_tool_for_multiple_tasks_mode,
};

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
    let mut command_run_turns = 0_u64;
    let mut last_task_delivered = false;
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

        if runtime.call_result_status == RuntimeCallResultStatus::TimedOut {
            if let Some(wait_duration) = provider_timeout_retry_wait(provider_timeout_retries) {
                provider_timeout_retries = provider_timeout_retries.saturating_add(1);
                warn!(
                    session_id = %session.session_id,
                    turn = turn,
                    runtime_id = %runtime.runtime_id,
                    retry = provider_timeout_retries,
                    wait_ms = wait_duration.as_millis(),
                    "provider runtime timed out; waiting before retrying with full tool set"
                );
                thread::sleep(wait_duration);
                current_messages.push(serde_json::json!({
                    "role": "system",
                    "content": format!("Provider timeout while waiting for the model response. This is transient provider failure retry {} of 3, not task completion. Retry the current task with the normal command_run tool unless the requested edits and validation are actually complete.", provider_timeout_retries)
                }));
                continue;
            }

            warn!(
                session_id = %session.session_id,
                turn = turn,
                runtime_id = %runtime.runtime_id,
                retries = provider_timeout_retries,
                "provider runtime timed out after retries; publishing visible failure"
            );
            publish_runtime_failure_message(
                session,
                &runtime.runtime_id,
                "Provider runtime timed out after 3 retries before completing the task.",
            );
            break;
        }

        if !tool_calls.is_empty() {
            provider_timeout_retries = 0;
            no_tool_retries = 0;
            if tool_calls
                .iter()
                .any(|tool_call| tool_call.tool_name == COMMAND_RUN_TOOL)
            {
                command_run_turns = command_run_turns.saturating_add(1);
            }
            let tool_results = execute_tool_calls(&tool_calls, session, &runtime, redis_url)?;
            last_task_delivered = tool_results
                .iter()
                .filter(|result| result.tool_name == TASK_DELIVERED_TOOL)
                .any(|result| task_delivered_marked_true(&result.arguments));

            for tool_result in tool_results.iter() {
                accumulate_tool_result_with_feedback(
                    session,
                    &tool_result.tool_name,
                    tool_result.arguments.clone(),
                    tool_result.result.clone(),
                    tool_result.success,
                    tool_result.error.clone(),
                    None,
                    None,
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
            if let Some(next_task) = active_task_user_message(session) {
                current_messages.push(next_task);
            } else if planned_tasks_all_completed(session) && last_task_delivered {
                current_messages.push(serde_json::json!({
                    "role": "system",
                    "content": "All planned tasks are marked delivered. Provide the final user-facing answer now."
                }));
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
            push_task_continuity_message(&mut current_messages, session, &original_user_task);
            if multiple_tasks_child_depth() > 0 {
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    "multiple_tasks child turn completed without tool calls, ending child session without synthesized user receipt"
                );
                break;
            }

            let final_text = user_visible_runtime_text(&runtime.text)
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty());
            if command_run_turns > 0
                && (!multiple_tasks_env_enabled()
                    || (last_task_delivered && !planned_tasks_incomplete(session)))
                && final_text
                    .as_deref()
                    .is_some_and(text_looks_like_final_answer)
            {
                info!(
                    session_id = %session.session_id,
                    turn = turn,
                    "assistant final text completed after command_run"
                );
                break;
            }

            if no_tool_retries < no_tool_retry_limit() {
                no_tool_retries = no_tool_retries.saturating_add(1);
                let prior_text = final_text.unwrap_or_else(|| {
                    "The previous model turn returned no tool call.".to_string()
                });
                let multiple_tasks_delivery_missing = multiple_tasks_env_enabled()
                    && command_run_turns > 0
                    && planned_tasks_incomplete(session)
                    && !last_task_delivered;
                let completion_guard = if multiple_tasks_delivery_missing {
                    "The active planned task is not marked delivered. If it is complete and verified, call task_delivered with task_delivered true now. Do not call command_run only to rerun verification or status. Use command_run only if work remains."
                } else if planned_tasks_incomplete(session) {
                    "The planned task list is not complete. Continue the active planned task with command_run."
                } else {
                    "Continue the original task now by calling command_run to inspect, edit, test, or write required files."
                };
                let retry_reason = if multiple_tasks_delivery_missing {
                    "The previous text response did not update the multiple_tasks state."
                } else {
                    "The previous non-final model turn did not call a required tool, so no workspace action happened."
                };
                current_messages.push(serde_json::json!({
                    "role": "system",
                    "content": format!(
                        "{retry_reason} {completion_guard}\n\nPrevious text-only response:\n{}",
                        prior_text
                    ),
                }));
                if let Some(next_task) = active_task_user_message(session) {
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

    session.transition(SessionState::Completed, now)?;
    persist_session_checkpoint(session, "completed");

    for agent in agents.iter_mut() {
        agent.state = AgentState::Completed;
        agent.updated_at = Utc::now();
    }

    let final_runtime = create_dummy_runtime(last_runtime_id.unwrap_or_default(), session);

    Ok(ManasResult {
        agents: agents.to_vec(),
        session: session.clone(),
        final_runtime,
    })
}

fn task_delivered_marked_true(result: &serde_json::Value) -> bool {
    result
        .get("task_delivered")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

fn active_task_user_message(session: &SessionManagement) -> Option<serde_json::Value> {
    let (index, task) = session
        .task_plan
        .detailed_tasks
        .iter()
        .enumerate()
        .find(|(_, task)| task.status == TaskStatus::InProgress)?;
    Some(serde_json::json!({
        "role": "user",
        "content": format!(
            "Continue planned task {}.\nTask summary: {}\nDeliverble: {}\n\nUse command_run only for remaining workspace work. If this task is complete and verified, call task_delivered with task_delivered true; do not rerun verification only to finish.",
            index + 1,
            task.task_summary,
            task.step_deliverable_description
        )
    }))
}

fn planned_tasks_incomplete(session: &SessionManagement) -> bool {
    session
        .task_plan
        .detailed_tasks
        .iter()
        .any(|task| task.status != TaskStatus::Completed)
}

fn planned_tasks_all_completed(session: &SessionManagement) -> bool {
    !session.task_plan.detailed_tasks.is_empty() && !planned_tasks_incomplete(session)
}

fn text_looks_like_final_answer(text: &str) -> bool {
    let normalized = text.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let continuation_markers = [
        "let me ",
        "i need to ",
        "i'll ",
        "i will ",
        "now i ",
        "next ",
        "then i'll ",
        "then i will ",
        "going to ",
        "need to ",
    ];
    !continuation_markers
        .iter()
        .any(|marker| normalized.contains(marker))
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
            role != Some("user")
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

fn apply_validator_reliability_feedback(runtime: &RuntimeManagement) {
    for record in &runtime.tool_call {
        let Some(success) = record.validator_reported_success else {
            continue;
        };
        if record.tool_called_name != COMMAND_RUN_TOOL {
            continue;
        }
        let Some(commands) = record
            .tool_called_input
            .get("commands")
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for command in commands {
            let Some(label) = command.get("command").and_then(|value| value.as_str()) else {
                continue;
            };
            if !label.trim_start().starts_with("py:") {
                continue;
            }
            let tool_name = registry_tool_name_for_command_label(label);
            let note = if success {
                None
            } else {
                Some(format!(
                    "validator reported failure for runtime {} command {}",
                    runtime.runtime_id, label
                ))
            };
            let _ = call_alaya_registry_reliability(
                "command-run-auto",
                &tool_name,
                success,
                note.as_deref(),
            );
        }
    }
}

fn call_alaya_registry_reliability(
    service_id: &str,
    tool_name: &str,
    success: bool,
    note: Option<&str>,
) -> Result<(), String> {
    let root = project_root_for_alaya().ok_or_else(|| "project root not found".to_string())?;
    let exe = alaya_executable_for_feedback(&root)
        .ok_or_else(|| "alaya executable not found".to_string())?;
    let mut command = std::process::Command::new(exe);
    command
        .args([
            "registry",
            "update-reliability",
            "--service-id",
            service_id,
            "--tool-name",
            tool_name,
            "--success",
            if success { "true" } else { "false" },
        ])
        .env("TURA_PROJECT_ROOT", &root);
    if let Some(note) = note {
        command.args(["--note", note]);
    }
    let status = command.status().map_err(|err| err.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "alaya update-reliability failed".to_string())
}

fn registry_tool_name_for_command_label(command: &str) -> String {
    let mut out = command
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "command_run_tool".to_string()
    } else {
        out
    }
}

fn alaya_executable_for_feedback(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let exe_name = if cfg!(windows) {
        "alaya_memory_server.exe"
    } else {
        "alaya_memory_server"
    };
    [
        root.join("target")
            .join("alaya-service-target")
            .join("debug")
            .join(exe_name),
        root.join("target").join("debug").join(exe_name),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn project_root_for_alaya() -> Option<std::path::PathBuf> {
    if let Ok(root) = std::env::var("TURA_PROJECT_ROOT") {
        let path = std::path::PathBuf::from(root);
        if path
            .join("services")
            .join("alaya")
            .join("Cargo.toml")
            .exists()
        {
            return Some(path);
        }
    }
    for start in [std::env::current_dir().ok(), std::env::current_exe().ok()]
        .into_iter()
        .flatten()
    {
        for candidate in start.ancestors() {
            if candidate
                .join("services")
                .join("alaya")
                .join("Cargo.toml")
                .exists()
            {
                return Some(candidate.to_path_buf());
            }
        }
    }
    None
}

fn create_dummy_runtime(runtime_id: RuntimeId, session: &SessionManagement) -> RuntimeManagement {
    let now = Utc::now();
    let provider_name = crate::agent_router::coding_agent_provider_name();

    let runtime_provider_config = crate::state_machine::runtime_management::RuntimeProviderConfig {
        base: crate::state_machine::agent_management::ProviderConfig {
            tura_llm_name: provider_name.clone(),
            stream: false,
            temperature: 0.5,
            max_tokens: 0,
            tool_choice: crate::state_machine::agent_management::ToolChoice::Auto,
            time_out_ms: 120_000,
        },
        thinking: false,
        provider_name: provider_name.clone(),
        model_name: String::new(),
        provider_url_name: String::new(),
        provider_router_name: provider_name,
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
        filter_tools_for_turn, load_agent_prompt_messages, normalize_tool_arguments,
        normalize_tool_arguments_for_tool, provider_timeout_retry_wait, remove_tool,
        require_multiple_tasks_tool_for_multiple_tasks_mode, user_visible_runtime_text,
        COMMAND_RUN_TOOL, MULTIPLE_TASKS_TOOL,
    };
    use crate::state_machine::agent_management::{
        AgentCapabilityItem, AgentManagement, AgentPromptItem, ProviderConfig, ToolChoice,
        ValidatorConfig,
    };
    use serde_json::json;
    use std::collections::HashSet;

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
    fn multiple_tasks_mode_still_exposes_only_command_run() {
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
                "function": { "name": MULTIPLE_TASKS_TOOL, "parameters": { "type": "object" } }
            }),
        ];

        let filtered = filter_tools_for_turn(tools, false, false, false, true)
            .expect("multiple_tasks filtering should succeed");
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
        assert!(!names.contains(MULTIPLE_TASKS_TOOL));
    }

    #[test]
    fn multiple_tasks_mode_requires_multiple_tasks_command() {
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

        let error = require_multiple_tasks_tool_for_multiple_tasks_mode(tools)
            .expect_err("multiple_tasks should be required");

        assert!(error.contains("multiple-tasks mode requested but multiple_tasks is unavailable"));
    }

    #[test]
    fn default_non_final_turn_keeps_development_tools_without_multiple_tasks() {
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
                "function": { "name": MULTIPLE_TASKS_TOOL, "parameters": { "type": "object" } }
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
        assert!(!names.contains(MULTIPLE_TASKS_TOOL));
    }

    #[test]
    fn multiple_tasks_child_turn_keeps_development_tools_and_removes_multiple_tasks_command() {
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
                "function": { "name": MULTIPLE_TASKS_TOOL, "parameters": { "type": "object" } }
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
        assert!(!names.contains(MULTIPLE_TASKS_TOOL));
    }

    #[test]
    fn remove_tool_filters_matching_provider_schema() {
        let tools = vec![
            json!({
                "type": "function",
                "function": { "name": MULTIPLE_TASKS_TOOL, "parameters": { "type": "object" } }
            }),
            json!({
                "type": "function",
                "function": { "name": COMMAND_RUN_TOOL, "parameters": { "type": "object" } }
            }),
        ];

        let filtered = remove_tool(tools, MULTIPLE_TASKS_TOOL);
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

        let active_tool_names = HashSet::from([COMMAND_RUN_TOOL.to_string()]);
        let messages = load_agent_prompt_messages(&agent, &active_tool_names)
            .expect("prompt loading should succeed");
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
