use chrono::Utc;

use crate::manas::prompt_messages::planning_objective_block;
use crate::prompt_style::task_status;
use crate::state_machine::session_management::SessionManagement;

use super::char_budget::{truncate_text_to_char_budget, COMPACT_CONTEXT_MAX_CHARS};
use super::text_truncate::environment_context_message;
use super::{ContextualUserFragment, WorkspaceSnapshot, USER_AGENT_CONTEXT_ROLE};

pub fn compact_session_context(
    session: &mut SessionManagement,
    compact_text: &str,
) -> Result<(), String> {
    let now = Utc::now();
    let compact_text = truncate_text_to_char_budget(compact_text.trim(), COMPACT_CONTEXT_MAX_CHARS);
    let workspace_snapshot = WorkspaceSnapshot::from_cwd(&session.session_directory)
        .map(|snapshot| snapshot.render())
        .unwrap_or_else(|| "<WORKSPACE_SNAPSHOT>\n\n</WORKSPACE_SNAPSHOT>".to_string());
    let environment_context = environment_context_message(&session.session_directory);
    let compact_record = serde_json::json!({
            "type": "context_compaction",
            "content": compact_text,
            "workspace_snapshot": workspace_snapshot,
            "environment_context": environment_context,
            "timestamp": now.to_rfc3339(),
    });
    session.context_tokens.input = 0;
    session.runtime_usage = serde_json::Value::Null;
    session.push_log(compact_record.to_string(), now);
    Ok(())
}

pub(super) fn context_compaction_messages(
    value: &serde_json::Value,
    session: &SessionManagement,
) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    if let Some(content) = value.get("content").and_then(serde_json::Value::as_str) {
        messages.push(serde_json::json!({
            "role": "user",
            "content": content,
        }));
    }
    if session.planning_enabled {
        let objective = planning_objective_block(session);
        if !objective.trim().is_empty() {
            messages.push(serde_json::json!({
                "role": "user",
                "content": task_status::planning_objective_context(&objective),
            }));
        }
    }
    if let Some(snapshot) = value
        .get("workspace_snapshot")
        .and_then(serde_json::Value::as_str)
    {
        messages.push(serde_json::json!({
            "role": USER_AGENT_CONTEXT_ROLE,
            "content": snapshot,
        }));
    }
    if let Some(environment) = value
        .get("environment_context")
        .and_then(serde_json::Value::as_str)
    {
        messages.push(serde_json::json!({
            "role": USER_AGENT_CONTEXT_ROLE,
            "content": environment,
        }));
    }
    messages
}
