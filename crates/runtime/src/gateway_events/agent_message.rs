use crate::prompt_style::{runtime_fallback, tool_progress};
use crate::state_machine::session_management::SessionManagement;
use std::io::Write;
use tracing::warn;

use crate::manas::constants::gateway_callbacks_disabled;
use crate::manas::final_response::summarize_tool_results_for_user;
use crate::manas::tool_catalog::env_flag;

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

/// Stable message id for the streamed assistant text of a given provider turn.
/// Both the incremental `message.part.delta` events and the final persisted
/// message reuse this id so the full reply cleanly replaces the streamed deltas.
pub(crate) fn stream_agent_message_id(runtime_id: &str) -> String {
    format!("msg-stream-{runtime_id}")
}

/// Stable text part id paired with [`stream_agent_message_id`].
pub(crate) fn stream_agent_part_id(runtime_id: &str) -> String {
    format!("part-stream-{runtime_id}")
}

/// Publish one incremental assistant text delta to the gateway, which re-emits it
/// as a `message.part.delta` so the frontend renders tokens as they arrive.
pub(crate) async fn publish_streamed_agent_text(session_id: &str, runtime_id: &str, delta: &str) {
    if gateway_callbacks_disabled() || delta.is_empty() {
        return;
    }
    let target_session_id = gateway_callback_session_id(session_id);
    let endpoint = format!(
        "{}/session/{target_session_id}/message/agent/stream",
        gateway_callback_base_url()
    );
    let payload = serde_json::json!({
        "message_id": stream_agent_message_id(runtime_id),
        "part_id": stream_agent_part_id(runtime_id),
        "delta": delta,
        "runtime_id": runtime_id,
    });
    if let Err(error) = reqwest::Client::new()
        .post(endpoint)
        .json(&payload)
        .send()
        .await
    {
        warn!(
            session_id = %session_id,
            runtime_id = %runtime_id,
            error = %error,
            "failed to publish streamed agent text delta"
        );
    }
}

pub(crate) fn publish_gateway_agent_message(
    session_id: &str,
    runtime_id: &str,
    reply_message: String,
    new_learning: String,
) -> Result<(), String> {
    if gateway_callbacks_disabled() {
        return Ok(());
    }

    let target_session_id = gateway_callback_session_id(session_id);
    let gateway_base = gateway_callback_base_url();
    let endpoint = format!("{gateway_base}/session/{target_session_id}/message/agent");
    let payload = serde_json::json!({
        "reply_message": reply_message,
        "new_learning": new_learning,
        "media": [],
        "runtime_id": runtime_id,
        "message_id": stream_agent_message_id(runtime_id),
        "part_id": stream_agent_part_id(runtime_id),
    });

    tokio::runtime::Runtime::new()
        .map_err(|err| format!("failed to create gateway callback runtime: {err}"))?
        .block_on(async {
            let response = reqwest::Client::new()
                .post(endpoint)
                .json(&payload)
                .send()
                .await
                .map_err(|err| format!("failed to call gateway: {err}"))?;
            if response.status().is_success() {
                Ok(())
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Err(format!("gateway returned {status}: {body}"))
            }
        })
}

pub(super) fn gateway_callback_base_url() -> String {
    std::env::var("TURA_GATEWAY_URL")
        .or_else(|_| std::env::var("GATEWAY_BASE_URL"))
        .unwrap_or_else(|_| {
            let port = std::env::var("TURA_GATEWAY_PORT")
                .or_else(|_| std::env::var("PORT"))
                .unwrap_or_else(|_| "4156".to_string());
            format!("http://127.0.0.1:{port}")
        })
        .trim_end_matches('/')
        .to_string()
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
