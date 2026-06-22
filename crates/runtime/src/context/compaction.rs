use chrono::{DateTime, TimeZone, Utc};

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

pub(crate) fn compact_session_context_automatically(
    session: &mut SessionManagement,
    compact_text: &str,
) -> Result<(), String> {
    compact_session_context_with_options(
        session,
        compact_text,
        None,
        CompactContextOptions {
            include_current_tool_results: true,
        },
    )
}

#[derive(Debug, Clone, Copy, Default)]
struct CompactContextOptions {
    include_current_tool_results: bool,
}

fn compact_session_context_with_options(
    session: &mut SessionManagement,
    compact_text: &str,
    agent_message: Option<CompactContextAgentMessage<'_>>,
    options: CompactContextOptions,
) -> Result<(), String> {
    let now = Utc::now();
    let content = compact_rebuild_content(session, compact_text, agent_message, options);
    let compact_text = truncate_text_to_char_budget(content.trim(), COMPACT_CONTEXT_MAX_CHARS);
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
    compact_text: &str,
    agent_message: Option<CompactContextAgentMessage<'_>>,
    options: CompactContextOptions,
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

    compact_content_with_trimmed_timeline(entries, compact_text)
}

const COMPACT_CONTEXT_TARGET_TOKENS: usize = 12_000;
const ESTIMATED_TOKEN_BYTES: usize = 3;

fn compact_content_with_trimmed_timeline(
    mut entries: Vec<TimelineEntry>,
    compact_text: &str,
) -> String {
    let mut omitted = 0usize;
    let mut out = render_compact_content(&entries, omitted, compact_text);
    let max_bytes = COMPACT_CONTEXT_TARGET_TOKENS * ESTIMATED_TOKEN_BYTES;
    while estimated_tokens_from_bytes(out.len()) > COMPACT_CONTEXT_TARGET_TOKENS
        && !entries.is_empty()
    {
        entries.remove(0);
        omitted += 1;
        out = render_compact_content(&entries, omitted, compact_text);
    }
    if out.len() > max_bytes {
        out = truncate_text_to_char_budget(&out, max_bytes);
    }
    out
}

fn render_compact_content(entries: &[TimelineEntry], omitted: usize, compact_text: &str) -> String {
    let mut out =
        String::from("Context rebuild timeline before this checkpoint (timestamps are UTC):\n");
    if omitted > 0 {
        out.push_str(&format!(
            "- [older timeline entries omitted to keep this checkpoint under about {COMPACT_CONTEXT_TARGET_TOKENS} estimated tokens: {omitted}]\n"
        ));
    }
    if entries.is_empty() {
        out.push_str("- none\n");
    } else {
        for entry in entries {
            out.push_str(&format!(
                "- [{}] {}/{}: {}\n",
                entry.timestamp.to_rfc3339(),
                entry.scope,
                entry.role,
                compact_timeline_text(&entry.content)
            ));
        }
    }
    out.push_str("\nCompact context handoff:\n");
    out.push_str(compact_text.trim());
    out
}

fn estimated_tokens_from_bytes(bytes: usize) -> usize {
    bytes.div_ceil(ESTIMATED_TOKEN_BYTES)
}

fn compaction_timeline_entries(
    session: &SessionManagement,
    options: CompactContextOptions,
) -> Vec<TimelineEntry> {
    let values = session
        .session_log
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            serde_json::from_str::<serde_json::Value>(entry)
                .ok()
                .map(|value| (index, value))
        })
        .collect::<Vec<_>>();
    let retained_start = values
        .iter()
        .rposition(|(_, value)| {
            value.get("type").and_then(serde_json::Value::as_str) == Some("context_compaction")
        })
        .unwrap_or(0);
    let mut entries = Vec::new();
    for (index, value) in values {
        let Some(timestamp) = log_timestamp(&value) else {
            continue;
        };
        let in_current_run = timestamp >= session.session_started_at;
        let in_retained_context = index >= retained_start;
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
        if in_retained_context {
            if let Some((role, content)) = retained_context_timeline_item(&value) {
                entries.push(TimelineEntry {
                    timestamp,
                    scope: "retained_context",
                    role,
                    content,
                    index,
                });
            }
        }
    }
    entries
}

fn current_run_timeline_item(
    value: &serde_json::Value,
    options: CompactContextOptions,
) -> Option<(&'static str, String)> {
    role_timeline_item(value, true).or_else(|| {
        options
            .include_current_tool_results
            .then(|| tool_result_timeline_item(value))
            .flatten()
    })
}

fn retained_context_timeline_item(value: &serde_json::Value) -> Option<(&'static str, String)> {
    if value.get("type").and_then(serde_json::Value::as_str) == Some("context_compaction") {
        return value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .map(|content| ("agent_summary", content.to_string()));
    }
    role_timeline_item(value, false).map(|(role, content)| {
        let role = if role == "agent" {
            "agent_summary"
        } else {
            role
        };
        (role, content)
    })
}

fn tool_result_timeline_item(value: &serde_json::Value) -> Option<(&'static str, String)> {
    if value.get("type").and_then(serde_json::Value::as_str) != Some("tool_result") {
        return None;
    }
    let tool_name = value
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool");
    let success = value
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let mut content = format!("{tool_name} success={success}");
    if let Some(error) = value.get("error").and_then(serde_json::Value::as_str) {
        if !error.trim().is_empty() {
            content.push_str(" error=");
            content.push_str(error.trim());
        }
    }
    if let Some(output) = value
        .get("context_cache")
        .and_then(|cache| cache.get("output"))
        .or_else(|| value.get("context_message"))
        .or_else(|| value.get("output"))
    {
        content.push_str(" output=");
        content.push_str(&content_text(output));
    }
    Some(("tool", content))
}

fn role_timeline_item(
    value: &serde_json::Value,
    include_user_agent: bool,
) -> Option<(&'static str, String)> {
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
