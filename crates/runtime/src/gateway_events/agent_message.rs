use crate::prompt_style::{runtime_fallback, tool_progress};
use crate::state_machine::session_management::SessionManagement;
use std::io::Write;
use tracing::warn;

use crate::manas::constants::gateway_callbacks_disabled;
use crate::manas::final_response::summarize_tool_results_for_user;
use crate::manas::tool_catalog::env_flag;
use crate::state_machine::runtime_management::{
    RuntimeManagement, RuntimeSessionSyncStatus, UsageReport,
};
use crate::state_machine::session_management::ContextTokenStats;

pub(crate) fn publish_runtime_failure_message(
    session: &SessionManagement,
    runtime_id: &str,
    error: &str,
) {
    let reply_message = summarize_tool_results_for_user(session).map_or_else(
        || runtime_fallback::no_tool_results_runtime_failed(error),
        |summary| runtime_fallback::tool_results_then_runtime_failed(&summary, error),
    );
    emit_cli_agent_message(&reply_message);

    if let Err(publish_error) = publish_gateway_agent_message(
        &session.session_id,
        runtime_id,
        reply_message,
        tool_progress::runtime_failed_after_tool_execution(error),
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
/// Runtime callbacks, live overlays, and persisted session snapshots all derive
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
pub(crate) async fn publish_streamed_agent_text(
    session_id: &str,
    runtime: &RuntimeManagement,
    delta: &str,
) {
    if gateway_callbacks_disabled() || delta.is_empty() {
        return;
    }
    let target_session_id = gateway_callback_session_id(session_id);
    let payload = serde_json::json!({
        "delta": delta,
        "runtime_id": &runtime.runtime_id,
        "context_tokens": runtime.context_tokens,
        "usage": runtime.usage.clone(),
    });
    if !publish_gateway_callback_ipc("session.agent_stream", &target_session_id, payload) {
        warn!(
            session_id = %session_id,
            runtime_id = %runtime.runtime_id,
            "dropping streamed agent text delta because gateway callback IPC is not enabled"
        );
    }
}

pub(crate) fn publish_gateway_agent_message(
    session_id: &str,
    runtime_id: &str,
    reply_message: String,
    new_learning: String,
) -> Result<(), String> {
    publish_gateway_agent_message_with_sync(
        session_id,
        runtime_id,
        reply_message,
        new_learning,
        None,
        None,
        None,
        None,
        None,
    )
}

pub(crate) fn publish_gateway_agent_message_from_runtime(
    session_id: &str,
    runtime: &RuntimeManagement,
    reply_message: String,
    new_learning: String,
) -> Result<(), String> {
    let (created_at, updated_at) = runtime.assistant_message_timestamps();
    publish_gateway_agent_message_with_sync(
        session_id,
        &runtime.runtime_id,
        reply_message,
        new_learning,
        Some(runtime.session_sync_status()),
        Some(runtime.context_tokens),
        runtime.usage.clone(),
        Some(created_at),
        Some(updated_at),
    )
}

fn publish_gateway_agent_message_with_sync(
    session_id: &str,
    runtime_id: &str,
    reply_message: String,
    new_learning: String,
    runtime_status: Option<RuntimeSessionSyncStatus>,
    context_tokens: Option<ContextTokenStats>,
    usage: Option<UsageReport>,
    created_at: Option<i64>,
    updated_at: Option<i64>,
) -> Result<(), String> {
    if gateway_callbacks_disabled() {
        return Ok(());
    }

    let target_session_id = gateway_callback_session_id(session_id);
    let payload = serde_json::json!({
        "reply_message": reply_message,
        "new_learning": new_learning,
        "media": [],
        "runtime_id": runtime_id,
        "runtime_status": runtime_status,
        "context_tokens": context_tokens,
        "usage": usage,
        "created_at": created_at,
        "updated_at": updated_at,
    });
    if publish_gateway_callback_ipc("session.agent_message", &target_session_id, payload.clone()) {
        return Ok(());
    }
    Err("gateway callback IPC transport is not enabled".to_string())
}

pub(crate) fn post_gateway_callback_detached(
    method: &'static str,
    payload: serde_json::Value,
    session_id: String,
    runtime_id: String,
    context: &'static str,
) {
    if publish_gateway_callback_ipc(method, &session_id, payload) {
        return;
    }
    warn!(
        session_id = %session_id,
        runtime_id = %runtime_id,
        context = context,
        method,
        "dropping gateway callback because IPC transport is not enabled"
    );
}

pub(crate) fn publish_gateway_callback_ipc(
    method: &str,
    session_id: &str,
    body: serde_json::Value,
) -> bool {
    if !gateway_callback_ipc_enabled() {
        return false;
    }
    let frame = serde_json::json!({
        "kind": "gateway.callback",
        "method": method,
        "payload": {
            "session_id": session_id,
            "body": body,
        },
    });
    let encoded = match serde_json::to_string(&frame) {
        Ok(encoded) => encoded,
        Err(error) => {
            warn!(method, session_id, error = %error, "failed to encode gateway callback IPC frame");
            return true;
        }
    };
    let mut stdout = std::io::stdout().lock();
    if let Err(error) = stdout.write_all(encoded.as_bytes()) {
        warn!(method, session_id, error = %error, "failed to write gateway callback IPC frame");
        return true;
    }
    if let Err(error) = stdout.write_all(b"\n").and_then(|_| stdout.flush()) {
        warn!(method, session_id, error = %error, "failed to flush gateway callback IPC frame");
    }
    true
}

pub(crate) fn gateway_callback_ipc_enabled() -> bool {
    std::env::var("TURA_GATEWAY_CALLBACK_TRANSPORT")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "ipc" | "router-ipc" | "stdout"
            )
        })
}

pub(super) fn gateway_callback_session_id(session_id: &str) -> String {
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
        .or_else(|_| std::env::var("TURA_EXECUTE_TOOLS_DEPTH"))
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
    fn gateway_callback_ipc_enabled_accepts_known_transports() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();

        assert!(!gateway_callback_ipc_enabled());

        std::env::set_var("TURA_GATEWAY_CALLBACK_TRANSPORT", "ipc");
        assert!(gateway_callback_ipc_enabled());

        std::env::set_var("TURA_GATEWAY_CALLBACK_TRANSPORT", "router-ipc");
        assert!(gateway_callback_ipc_enabled());

        std::env::set_var("TURA_GATEWAY_CALLBACK_TRANSPORT", "http");
        assert!(!gateway_callback_ipc_enabled());
    }

    #[test]
    fn callback_session_id_uses_parent_only_for_planning_child_depth() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();

        std::env::set_var("TURA_PARENT_SESSION_ID", " parent-session ");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "child-session"
        );

        std::env::set_var("TURA_PLANNING_DEPTH", "1");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "parent-session"
        );

        std::env::set_var("TURA_PARENT_SESSION_ID", "   ");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "child-session"
        );

        std::env::remove_var("TURA_PLANNING_DEPTH");
        std::env::set_var("TURA_EXECUTE_TOOLS_DEPTH", "2");
        std::env::set_var("TURA_PARENT_SESSION_ID", "execute-parent");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "execute-parent"
        );

        std::env::set_var("TURA_EXECUTE_TOOLS_DEPTH", "not-a-number");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "child-session"
        );
    }

    #[test]
    fn publish_gateway_agent_message_returns_ok_when_callbacks_are_disabled() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();
        std::env::set_var("TURA_GATEWAY_CALLBACKS", "off");

        let result = publish_gateway_agent_message(
            "session-1",
            "runtime-1",
            "reply".to_string(),
            "learning".to_string(),
        );

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn publish_gateway_agent_message_requires_ipc_transport_when_callbacks_enabled() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();

        let error = publish_gateway_agent_message(
            "session-1",
            "runtime-42",
            "visible reply".to_string(),
            "new learning".to_string(),
        )
        .expect_err("gateway callbacks require router IPC");
        assert!(error.contains("IPC transport is not enabled"));
    }

    fn clear_gateway_env() {
        for key in [
            "TURA_PARENT_SESSION_ID",
            "TURA_PLANNING_DEPTH",
            "TURA_EXECUTE_TOOLS_DEPTH",
            "TURA_GATEWAY_CALLBACKS",
            "TURA_GATEWAY_CALLBACK_TRANSPORT",
            "TURA_CLI_LIVE_JSONL",
        ] {
            std::env::remove_var(key);
        }
    }
}
