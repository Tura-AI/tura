use crate::prompt_style::{
    latest_planning_plan, planning_gate, task_continuity, user_new_command, PromptBuilder,
};
use crate::state_machine::session_management::SessionManagement;

use super::constants::{DISABLE_GATEWAY_CALLBACKS_ENV, PLANNING_TOOL};
use super::gateway_events::gateway_callback_base_url;
use super::tool_catalog::{
    env_flag, planning_child_depth, planning_env_enabled, planning_gate_disabled,
};

pub(super) fn messages_for_turn(
    current_messages: &[serde_json::Value],
    session: &SessionManagement,
    original_user_task: &str,
) -> Vec<serde_json::Value> {
    let mut messages = current_messages.to_vec();
    let _ = (session, original_user_task);
    if planning_env_enabled() && !planning_gate_disabled() {
        let content = PromptBuilder::new()
            .part(planning_gate::PLANNING_GATE)
            .section("planning_tool", PLANNING_TOOL)
            .render();
        messages.push(serde_json::json!({
            "role": "system",
            "content": content,
        }));
    }
    if let Some(content) = user_new_command_message(&session.session_id) {
        messages.push(serde_json::json!({
            "role": "system",
            "content": content,
        }));
    }
    messages
}

pub(super) fn user_new_command_message(session_id: &str) -> Option<String> {
    let commands = fetch_user_commands(session_id);
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

pub(super) fn fetch_user_commands(session_id: &str) -> Vec<String> {
    if session_id.trim().is_empty() || env_flag(DISABLE_GATEWAY_CALLBACKS_ENV) {
        return Vec::new();
    }
    let gateway_base = gateway_callback_base_url();
    let endpoint = format!("{gateway_base}/session/{session_id}/user-commands");
    let Ok(value) = tokio::runtime::Runtime::new()
        .map_err(|_| ())
        .and_then(|runtime| {
            runtime.block_on(async {
                let response = reqwest::Client::new()
                    .get(endpoint)
                    .send()
                    .await
                    .map_err(|_| ())?;
                if !response.status().is_success() {
                    return Err(());
                }
                response.json::<serde_json::Value>().await.map_err(|_| ())
            })
        })
    else {
        return Vec::new();
    };
    value
        .get("commands")
        .and_then(|value| value.as_array())
        .map(|commands| {
            commands
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn push_task_continuity_message(
    messages: &mut Vec<serde_json::Value>,
    session: &SessionManagement,
    original_user_task: &str,
) {
    let mut builder = PromptBuilder::new()
        .part(task_continuity::TASK_CONTINUITY)
        .section("original_user_task", original_user_task);

    if planning_env_enabled() {
        if let Some(plan) = latest_planning_plan_from_session(session) {
            builder = builder
                .part(latest_planning_plan::LATEST_PLANNING_PLAN)
                .section(
                    "latest_planning_plan",
                    serde_json::to_string_pretty(&plan).unwrap_or_else(|_| plan.to_string()),
                );
        }
    }

    messages.push(serde_json::json!({
        "role": "system",
        "content": builder.render(),
    }));
}

pub(super) fn latest_planning_plan_from_session(
    session: &SessionManagement,
) -> Option<serde_json::Value> {
    if session.task_plan.summary.is_empty() && session.task_plan.detailed_tasks.is_empty() {
        return None;
    }
    if planning_child_depth() > 0 {
        return Some(session.task_plan_detail_json());
    }
    Some(session.task_plan_summary_json())
}
