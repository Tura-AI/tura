use chrono::{DateTime, TimeZone, Utc};

use crate::manas::prompt_messages::planning_objective_block;
use crate::prompt_style::{runtime_prompt_manual, task_status};
use lifecycle::{SessionLogEntry, SessionManagement};

use super::char_budget::{
    compact_context_byte_budget, estimated_tokens_from_bytes, truncate_text_to_char_budget,
    COMPACT_CONTEXT_FALLBACK_MAX_ESTIMATED_TOKENS,
};
use super::text_truncate::environment_context_message;
use super::tool_results::{
    immutable_tool_result_context_messages, strip_context_reporting_fields,
    tool_result_context_cache,
};
use super::{ContextualUserFragment, WorkspaceSnapshot};

const MAX_INHERITED_COMPACT_SUMMARIES: usize = 2;
const INHERITED_COMPACT_CONTEXT_MARKER: &str = "[inherited_compact_context]";

pub fn compact_session_context(
    session: &mut SessionManagement,
    compact_text: &str,
) -> Result<(), String> {
    compact_session_context_with_options(
        session,
        compact_text,
        None,
        CompactContextOptions::default(),
    )
}

pub(crate) struct CompactContextAgentMessage<'a> {
    pub content: &'a str,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
pub(crate) fn compact_session_context_with_agent_message(
    session: &mut SessionManagement,
    compact_text: &str,
    agent_message: Option<CompactContextAgentMessage<'_>>,
) -> Result<(), String> {
    compact_session_context_with_options(
        session,
        compact_text,
        agent_message,
        CompactContextOptions::default(),
    )
}

pub(crate) fn compact_session_context_with_agent_message_and_capabilities(
    session: &mut SessionManagement,
    compact_text: &str,
    agent_message: Option<CompactContextAgentMessage<'_>>,
    baseline_capabilities: &[String],
) -> Result<(), String> {
    compact_session_context_with_options(
        session,
        compact_text,
        agent_message,
        CompactContextOptions {
            baseline_capabilities: baseline_capabilities.to_vec(),
        },
    )
}

#[cfg(test)]
pub(crate) fn compact_session_context_automatically(
    session: &mut SessionManagement,
    compact_text: &str,
) -> Result<(), String> {
    compact_session_context_with_options(
        session,
        compact_text,
        None,
        CompactContextOptions::default(),
    )
}

pub(crate) fn compact_session_context_automatically_with_capabilities(
    session: &mut SessionManagement,
    compact_text: &str,
    baseline_capabilities: &[String],
) -> Result<(), String> {
    compact_session_context_with_options(
        session,
        compact_text,
        None,
        CompactContextOptions {
            baseline_capabilities: baseline_capabilities.to_vec(),
        },
    )
}

#[derive(Debug, Clone, Default)]
struct CompactContextOptions {
    baseline_capabilities: Vec<String>,
}

fn compact_session_context_with_options(
    session: &mut SessionManagement,
    compact_text: &str,
    agent_message: Option<CompactContextAgentMessage<'_>>,
    options: CompactContextOptions,
) -> Result<(), String> {
    let now = Utc::now();
    let agent_handoff = compact_agent_handoff_text(compact_text);
    let goal_text = compact_goal_text(session, agent_handoff.is_empty());
    let max_estimated_tokens = compact_context_max_estimated_tokens(session);
    let content = compact_rebuild_content(
        session,
        &agent_handoff,
        &goal_text,
        agent_message,
        &options,
        max_estimated_tokens,
    );
    let compact_text = truncate_text_to_char_budget(
        content.trim(),
        compact_context_byte_budget(max_estimated_tokens),
    );
    let retained_from_index = compact_retained_log_start(&session.session_log);
    let compact_entry_index = session.session_log.len();
    let workspace_snapshot = WorkspaceSnapshot::from_cwd(&session.session_directory)
        .map(|snapshot| snapshot.render())
        .unwrap_or_else(|| "<WORKSPACE_SNAPSHOT>\n\n</WORKSPACE_SNAPSHOT>".to_string());
    let environment_context = environment_context_message(&session.session_directory);
    let compact_record = serde_json::json!({
            "type": "context_compaction",
            "category": "compact_context",
            "content": compact_text,
            "workspace_snapshot": workspace_snapshot,
            "environment_context": environment_context,
            "timestamp": now.to_rfc3339(),
    });
    session.context_tokens.input = 0;
    session.runtime_usage = serde_json::Value::Null;
    session.reset_session_capabilities_at(
        options
            .baseline_capabilities
            .iter()
            .map(std::string::String::as_str),
        now,
    );
    session.push_log(compact_record.to_string(), now);
    session.record_context_compaction_point(retained_from_index, compact_entry_index, now);
    runtime_prompt_manual::append_runtime_prompt_manuals_after_compact(session)?;
    Ok(())
}

fn compact_retained_log_start(session_log: &[SessionLogEntry]) -> usize {
    let mut index = session_log.len();
    while index > 0 {
        let Some(value) = session_log.get(index - 1).map(SessionLogEntry::value) else {
            break;
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some("tool_result") {
            break;
        }
        index -= 1;
    }
    index
}

const GOAL_MODE_CONTINUE_PROMPT_STYLE: &str = "[goal_mode_prompt_style]\nContinue working on the previous goal-mode task. The session is still in goal mode, and no explicit compact handoff or recorded goal command was available in the session state. Recover the prior task from the timestamped context above, continue the same objective, and do not treat the task as complete until task_status marks it done or question.";

fn compact_agent_handoff_text(compact_text: &str) -> String {
    compact_text.trim().to_string()
}

fn compact_goal_text(session: &SessionManagement, agent_handoff_empty: bool) -> String {
    if !session.goal_mode {
        return String::new();
    }
    let goal_text = session.last_goal_user_input.trim();
    match (agent_handoff_empty, goal_text.is_empty()) {
        (_, false) => goal_text.to_string(),
        (true, true) => GOAL_MODE_CONTINUE_PROMPT_STYLE.to_string(),
        (false, true) => String::new(),
    }
}

fn compact_context_max_estimated_tokens(session: &SessionManagement) -> usize {
    let active_limit = session.context_tokens.limit;
    if active_limit > 0 {
        return (active_limit / 10).max(1) as usize;
    }
    COMPACT_CONTEXT_FALLBACK_MAX_ESTIMATED_TOKENS
}

#[derive(Debug, Clone)]
struct TimelineEntry {
    timestamp: DateTime<Utc>,
    scope: &'static str,
    role: &'static str,
    content: String,
    index: usize,
}

fn compact_rebuild_content(
    session: &SessionManagement,
    agent_handoff: &str,
    goal_text: &str,
    agent_message: Option<CompactContextAgentMessage<'_>>,
    options: &CompactContextOptions,
    max_estimated_tokens: usize,
) -> String {
    let mut entries = compaction_timeline_entries(session, options);
    ensure_current_user_input_entry(session, &mut entries);
    if let Some(agent_message) = agent_message {
        let content = agent_message.content.trim();
        if !content.is_empty() {
            entries.push(TimelineEntry {
                timestamp: agent_message.timestamp,
                scope: "current_run",
                role: "agent",
                content: content.to_string(),
                index: usize::MAX,
            });
        }
    }
    entries.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.index.cmp(&right.index))
    });

    compact_content_with_trimmed_timeline(entries, agent_handoff, goal_text, max_estimated_tokens)
}

fn compact_content_with_trimmed_timeline(
    mut entries: Vec<TimelineEntry>,
    agent_handoff: &str,
    goal_text: &str,
    max_estimated_tokens: usize,
) -> String {
    let mut omitted = 0usize;
    let mut out = render_compact_content(
        &entries,
        omitted,
        agent_handoff,
        goal_text,
        max_estimated_tokens,
    );
    let max_bytes = compact_context_byte_budget(max_estimated_tokens);
    while estimated_tokens_from_bytes(out.len()) > max_estimated_tokens && !entries.is_empty() {
        let Some(remove_index) = removable_timeline_entry_index(&entries) else {
            break;
        };
        entries.remove(remove_index);
        omitted += 1;
        out = render_compact_content(
            &entries,
            omitted,
            agent_handoff,
            goal_text,
            max_estimated_tokens,
        );
    }
    if out.len() > max_bytes {
        out = truncate_text_to_char_budget(&out, max_bytes);
    }
    out
}

fn removable_timeline_entry_index(entries: &[TimelineEntry]) -> Option<usize> {
    entries
        .iter()
        .position(|entry| entry.scope == "other_task" && entry.role != "user")
        .or_else(|| entries.iter().position(|entry| entry.scope == "other_task"))
        .or_else(|| {
            let protected_user_agents = entries
                .iter()
                .filter(|entry| {
                    entry.scope == "current_run" && (entry.role == "user" || entry.role == "agent")
                })
                .count();
            (protected_user_agents > 1).then(|| {
                entries
                    .iter()
                    .position(|entry| {
                        entry.scope == "current_run"
                            && (entry.role == "user" || entry.role == "agent")
                    })
                    .unwrap_or(0)
            })
        })
        .or_else(|| {
            entries
                .iter()
                .position(|entry| entry.scope == "current_run" && entry.role == "user_context")
        })
        .or_else(|| {
            entries
                .iter()
                .position(|entry| entry.scope == "current_run" && entry.role == "tool")
        })
        .or_else(|| {
            entries.iter().position(|entry| {
                !(entry.scope == "compact_context"
                    || (entry.scope == "current_run"
                        && (entry.role == "user" || entry.role == "agent")))
            })
        })
}

fn render_compact_content(
    entries: &[TimelineEntry],
    omitted: usize,
    agent_handoff: &str,
    goal_text: &str,
    max_estimated_tokens: usize,
) -> String {
    let mut out = String::from("Context rebuild before this checkpoint (timestamps are UTC):\n");
    if omitted > 0 {
        out.push_str(&format!(
            "- [older timeline entries omitted to keep this checkpoint under about {max_estimated_tokens} estimated tokens using byte/4 estimation: {omitted}]\n"
        ));
    }
    out.push_str("\nTimestamped context history:\n");
    render_timeline_group(&mut out, entries.iter());
    out.push_str("\nAgent compact handoff:\n");
    let agent_handoff = agent_handoff.trim();
    if agent_handoff.is_empty() {
        out.push_str("- none\n");
    } else {
        out.push_str(agent_handoff);
        out.push('\n');
    }
    out.push_str("\nGoal-mode last user command from session state:\n");
    let goal_text = goal_text.trim();
    if goal_text.is_empty() {
        out.push_str("- none\n");
    } else {
        out.push_str(goal_text);
        out.push('\n');
    }
    out
}

fn render_timeline_group<'a>(out: &mut String, entries: impl Iterator<Item = &'a TimelineEntry>) {
    let mut saw_any = false;
    for entry in entries {
        saw_any = true;
        out.push_str(&format!(
            "- [{}] {}/{}: {}\n",
            entry.timestamp.to_rfc3339(),
            entry.scope,
            entry.role,
            compact_timeline_text(&entry.content)
        ));
    }
    if !saw_any {
        out.push_str("- none\n");
    }
}

fn compaction_timeline_entries(
    session: &SessionManagement,
    options: &CompactContextOptions,
) -> Vec<TimelineEntry> {
    let values = session
        .session_log
        .iter()
        .enumerate()
        .map(|(index, entry)| (index, entry.value().clone()))
        .collect::<Vec<_>>();
    let compact_count = values
        .iter()
        .filter(|(_, value)| {
            value.get("type").and_then(serde_json::Value::as_str) == Some("context_compaction")
        })
        .count();
    let compact_skip_count = compact_count.saturating_sub(MAX_INHERITED_COMPACT_SUMMARIES);
    let mut compact_seen = 0usize;
    let mut entries = Vec::new();
    let mut last_other_task_agent: Option<TimelineEntry> = None;
    for (index, value) in values {
        let Some(timestamp) = log_timestamp(&value) else {
            continue;
        };
        if value.get("type").and_then(serde_json::Value::as_str) == Some("prompt_style") {
            continue;
        }
        if value.get("type").and_then(serde_json::Value::as_str) == Some("context_compaction") {
            compact_seen += 1;
            if compact_seen <= compact_skip_count {
                continue;
            }
            if let Some(content) = value.get("content").and_then(serde_json::Value::as_str) {
                entries.push(TimelineEntry {
                    timestamp,
                    scope: "compact_context",
                    role: "user_instruction",
                    content: inherited_compact_summary_text(content),
                    index,
                });
            }
            continue;
        }
        let in_current_run = timestamp >= session.session_started_at;
        if in_current_run {
            if let Some((role, content)) = current_run_timeline_item(&value, options) {
                entries.push(TimelineEntry {
                    timestamp,
                    scope: "current_run",
                    role,
                    content,
                    index,
                });
            }
            continue;
        }
        if let Some((role, content)) = other_task_timeline_item(&value) {
            let entry = TimelineEntry {
                timestamp,
                scope: "other_task",
                role,
                content,
                index,
            };
            if role == "last_agent" {
                last_other_task_agent = Some(entry);
            } else {
                entries.push(entry);
            }
        }
    }
    if let Some(entry) = last_other_task_agent {
        entries.push(entry);
    }
    entries
}

fn current_run_timeline_item(
    value: &serde_json::Value,
    options: &CompactContextOptions,
) -> Option<(&'static str, String)> {
    let _ = options;
    role_timeline_item(value, true)
}

fn other_task_timeline_item(value: &serde_json::Value) -> Option<(&'static str, String)> {
    if value.get("type").and_then(serde_json::Value::as_str) == Some("user") {
        return value
            .get("content")
            .map(content_text)
            .map(|content| ("user", content));
    }
    let role = value.get("role").and_then(serde_json::Value::as_str)?;
    let content = value.get("content").map(content_text)?;
    match role {
        "user" => Some(("user", content)),
        super::USER_AGENT_CONTEXT_ROLE => Some(("user", content)),
        "assistant" => Some(("last_agent", content)),
        _ => None,
    }
}

fn role_timeline_item(
    value: &serde_json::Value,
    include_user_agent: bool,
) -> Option<(&'static str, String)> {
    if value.get("type").and_then(serde_json::Value::as_str) == Some("user") {
        return value
            .get("content")
            .map(content_text)
            .map(|content| ("user", content));
    }
    let role = value.get("role").and_then(serde_json::Value::as_str)?;
    let content = value.get("content").map(content_text)?;
    match role {
        "user" => Some(("user", content)),
        "assistant" => Some(("agent", content)),
        super::USER_AGENT_CONTEXT_ROLE if include_user_agent => Some(("user_context", content)),
        super::USER_AGENT_CONTEXT_ROLE => Some(("user", content)),
        _ => None,
    }
}

fn content_text(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn compact_timeline_text(value: &str) -> String {
    let one_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_text_to_char_budget(&one_line, 1_000)
}

fn ensure_current_user_input_entry(session: &SessionManagement, entries: &mut Vec<TimelineEntry>) {
    let input = session.input.user_input.trim();
    if input.is_empty() {
        return;
    }
    if entries.iter().any(|entry| {
        entry.scope == "current_run" && entry.role == "user" && entry.content.trim() == input
    }) {
        return;
    }
    entries.push(TimelineEntry {
        timestamp: session.session_started_at,
        scope: "current_run",
        role: "user",
        content: input.to_string(),
        index: usize::MAX,
    });
}

fn log_timestamp(value: &serde_json::Value) -> Option<DateTime<Utc>> {
    value
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .or_else(|| {
            value
                .get("created_at")
                .and_then(serde_json::Value::as_i64)
                .and_then(|millis| Utc.timestamp_millis_opt(millis).single())
        })
}

pub(super) fn context_compaction_messages(
    value: &serde_json::Value,
    session: &SessionManagement,
    raw_history_messages: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    if let Some(snapshot) = value
        .get("workspace_snapshot")
        .and_then(serde_json::Value::as_str)
    {
        messages.push(serde_json::json!({
            "role": "developer",
            "content": snapshot,
        }));
    }
    if let Some(environment) = value
        .get("environment_context")
        .and_then(serde_json::Value::as_str)
    {
        messages.push(serde_json::json!({
            "role": "developer",
            "content": environment,
        }));
    }
    if let Some(content) = value.get("content").and_then(serde_json::Value::as_str) {
        messages.push(serde_json::json!({
            "role": "user",
            "content": content,
        }));
    }
    messages.extend(recent_pre_compact_tool_context_messages(
        raw_history_messages,
    ));
    if session.reflection_enabled {
        let objective = planning_objective_block(session);
        if !objective.trim().is_empty() {
            messages.push(serde_json::json!({
                "role": "user",
                "content": task_status::planning_objective_context(&objective),
            }));
        }
    }
    messages
}

fn recent_pre_compact_tool_context_messages(
    raw_history_messages: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let mut recent_tool_results = Vec::new();
    for value in raw_history_messages.iter().rev() {
        if value.get("type").and_then(serde_json::Value::as_str) != Some("tool_result") {
            break;
        }
        recent_tool_results.push(value);
    }
    recent_tool_results.reverse();

    recent_tool_results
        .into_iter()
        .flat_map(compact_tool_result_context_messages)
        .collect()
}

fn compact_tool_result_context_messages(value: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(messages) = value
        .get("context_messages")
        .and_then(serde_json::Value::as_array)
    {
        return messages
            .iter()
            .cloned()
            .map(strip_context_reporting_fields)
            .collect();
    }

    let mut value = value.clone();
    if value.get("context_cache").is_none() {
        value["context_cache"] = tool_result_context_cache(&value);
    }
    immutable_tool_result_context_messages(&value)
}

fn inherited_compact_summary_text(content: &str) -> String {
    let handoff = compact_section(
        content,
        "Agent compact handoff:",
        Some("Goal-mode last user command from session state:"),
    );
    let goal = compact_section(
        content,
        "Goal-mode last user command from session state:",
        None,
    );
    if handoff.is_none() && goal.is_none() {
        return content.trim().to_string();
    }
    let handoff = handoff.unwrap_or_else(|| "- none".to_string());
    let goal = goal.unwrap_or_else(|| "- none".to_string());
    format!(
        "{INHERITED_COMPACT_CONTEXT_MARKER}\nAgent compact handoff:\n{}\n\nGoal-mode last user command from session state:\n{}",
        handoff.trim(),
        goal.trim()
    )
}

fn compact_section(content: &str, start_marker: &str, end_marker: Option<&str>) -> Option<String> {
    let start = content.rfind(start_marker)? + start_marker.len();
    let tail = &content[start..];
    let end = end_marker
        .and_then(|marker| tail.find(marker))
        .unwrap_or(tail.len());
    Some(tail[..end].trim().to_string())
}
