use crate::prompt_style::{runtime_fallback, tool_progress};
use lifecycle::SessionManagement;
use std::io::Write;
use tracing::warn;

use crate::manas::final_response::summarize_tool_results_for_user;
use crate::manas::tool_catalog::env_flag;
use crate::runtime_event_writer::RuntimeFeedPublisher;
use lifecycle::ContextTokenStats;
use lifecycle::RuntimeProjection;
use lifecycle::{RuntimeAggregate, UsageReport};
use session_log_contract::SessionFeedEvent;

pub(crate) fn publish_runtime_failure_message(
    session: &SessionManagement,
    runtime_id: &str,
    error: &str,
    publisher: Option<&RuntimeFeedPublisher>,
) {
    let reply_message = summarize_tool_results_for_user(session).map_or_else(
        || runtime_fallback::no_tool_results_runtime_failed(error),
        |summary| runtime_fallback::tool_results_then_runtime_failed(&summary, error),
    );
    emit_cli_agent_message(&reply_message);

    if let Err(publish_error) = publish_agent_message(
        &session.session_id,
        runtime_id,
        reply_message,
        tool_progress::runtime_failed_after_tool_execution(error),
        publisher,
    ) {
        warn!(
            session_id = %session.session_id,
            runtime_id = %runtime_id,
            error = %publish_error,
            "failed to publish visible runtime failure"
        );
    }
}

fn emit_cli_agent_message(reply_message: &str) {
    if !env_flag("TURA_CLI_LIVE_JSONL") {
        return;
    }
    let event = serde_json::json!({
        "type": "item.completed",
        "item": {
            "id": "item_runtime_failure",
            "type": "agent_message",
            "text": reply_message,
        }
    });
    println!("{event}");
    let _ = std::io::stdout().flush();
}

/// Canonical frontend message id for a runtime-owned assistant turn.
///
/// Runtime feed events, live overlays, and persisted session snapshots all derive
/// the same id from `runtime_id` so one provider call has one assistant message.
pub(crate) fn runtime_message_id(runtime_id: &str) -> String {
    format!("{runtime_id}.message")
}

/// Canonical text part id paired with [`runtime_message_id`].
pub(crate) fn runtime_text_part_id(runtime_id: &str) -> String {
    format!("{runtime_id}.message")
}

/// Canonical tool part id for a runtime-owned assistant turn.
pub(crate) fn runtime_tool_part_id(runtime_id: &str, tool_name: &str) -> String {
    format!("{runtime_id}.tool.{tool_name}")
}

/// Publish one incremental assistant text delta to the gateway, which re-emits it
/// as a `message.part.delta` so the frontend renders tokens as they arrive.
pub(crate) fn publish_streamed_agent_text(
    runtime: &RuntimeAggregate,
    delta: &str,
    publisher: Option<&RuntimeFeedPublisher>,
) -> Result<(), String> {
    if delta.is_empty() {
        return Ok(());
    }
    let (created_at, _) = runtime.assistant_message_timestamps();
    publish_feed_event(
        publisher,
        SessionFeedEvent::AssistantTextDelta {
            message_id: runtime_message_id(&runtime.runtime_id),
            part_id: runtime_text_part_id(&runtime.runtime_id),
            delta: delta.to_string(),
            created_at,
            updated_at: chrono::Utc::now().timestamp_millis(),
        },
    )
}

pub(crate) fn publish_agent_message(
    session_id: &str,
    runtime_id: &str,
    reply_message: String,
    new_learning: String,
    publisher: Option<&RuntimeFeedPublisher>,
) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    publish_agent_message_event(AgentMessageEvent {
        session_id,
        runtime_id,
        reply_message,
        new_learning,
        runtime_status: None,
        context_tokens: None,
        usage: None,
        created_at: now,
        updated_at: now,
        publisher,
    })
}

pub(crate) fn publish_agent_message_from_runtime(
    session_id: &str,
    runtime: &RuntimeAggregate,
    reply_message: String,
    new_learning: String,
    publisher: Option<&RuntimeFeedPublisher>,
) -> Result<(), String> {
    let (created_at, updated_at) = runtime.assistant_message_timestamps();
    publish_agent_message_event(AgentMessageEvent {
        session_id,
        runtime_id: &runtime.runtime_id,
        reply_message,
        new_learning,
        runtime_status: Some(runtime.lifecycle_projection()),
        context_tokens: Some(runtime.context_tokens),
        usage: runtime.usage.clone(),
        created_at,
        updated_at,
        publisher,
    })
}

struct AgentMessageEvent<'a> {
    session_id: &'a str,
    runtime_id: &'a str,
    reply_message: String,
    new_learning: String,
    runtime_status: Option<RuntimeProjection>,
    context_tokens: Option<ContextTokenStats>,
    usage: Option<UsageReport>,
    created_at: i64,
    updated_at: i64,
    publisher: Option<&'a RuntimeFeedPublisher>,
}

fn publish_agent_message_event(message: AgentMessageEvent<'_>) -> Result<(), String> {
    let _ = message.session_id;
    publish_feed_event(
        message.publisher,
        SessionFeedEvent::AgentMessage {
            message_id: runtime_message_id(message.runtime_id),
            part_id: runtime_text_part_id(message.runtime_id),
            reply_message: message.reply_message,
            new_learning: message.new_learning,
            runtime_status: message.runtime_status,
            context_tokens: message.context_tokens,
            usage: message.usage,
            created_at: message.created_at,
            updated_at: message.updated_at,
        },
    )
}

pub(crate) fn publish_feed_event(
    publisher: Option<&RuntimeFeedPublisher>,
    event: SessionFeedEvent,
) -> Result<(), String> {
    publisher.map_or(Ok(()), |publisher| publisher.publish(event))
}

pub(crate) fn frontend_session_id(session_id: &str) -> String {
    if planning_child_depth_from_env() > 0 {
        if let Ok(parent_session_id) = std::env::var("TURA_PARENT_SESSION_ID") {
            let parent_session_id = parent_session_id.trim();
            if !parent_session_id.is_empty() {
                return parent_session_id.to_string();
            }
        }
    }

    session_id.to_string()
}

fn planning_child_depth_from_env() -> usize {
    std::env::var("TURA_PLANNING_DEPTH")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn stream_message_ids_are_stable_and_runtime_scoped() {
        assert_eq!(runtime_message_id("runtime-123"), "runtime-123.message");
        assert_eq!(runtime_text_part_id("runtime-123"), "runtime-123.message");
        assert_ne!(
            runtime_message_id("runtime-123"),
            runtime_message_id("runtime-456")
        );
    }

    #[test]
    fn frontend_session_id_uses_parent_only_for_planning_child_depth() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();

        std::env::set_var("TURA_PARENT_SESSION_ID", " parent-session ");
        assert_eq!(frontend_session_id("child-session"), "child-session");

        std::env::set_var("TURA_PLANNING_DEPTH", "1");
        assert_eq!(frontend_session_id("child-session"), "parent-session");

        std::env::set_var("TURA_PARENT_SESSION_ID", "   ");
        assert_eq!(frontend_session_id("child-session"), "child-session");

        std::env::set_var("TURA_PLANNING_DEPTH", "2");
        std::env::set_var("TURA_PARENT_SESSION_ID", "execute-parent");
        assert_eq!(frontend_session_id("child-session"), "execute-parent");

        std::env::set_var("TURA_PLANNING_DEPTH", "not-a-number");
        assert_eq!(frontend_session_id("child-session"), "child-session");
    }

    #[test]
    fn publish_agent_message_without_a_feed_is_a_noop() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();

        let result = publish_agent_message(
            "session-1",
            "runtime-1",
            "reply".to_string(),
            "learning".to_string(),
            None,
        );

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn publish_agent_message_without_a_feed_is_repeatable() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();

        let result = publish_agent_message(
            "session-1",
            "runtime-42",
            "visible reply".to_string(),
            "new learning".to_string(),
            None,
        );
        assert_eq!(result, Ok(()));
    }

    fn clear_gateway_env() {
        for key in [
            "TURA_PARENT_SESSION_ID",
            "TURA_PLANNING_DEPTH",
            "TURA_CLI_LIVE_JSONL",
        ] {
            std::env::remove_var(key);
        }
    }
}
