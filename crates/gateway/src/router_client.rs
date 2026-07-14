//! Typed client for the persistent gateway-owned router process.
//!
//! This client is for execution supervision only. Session DB data reads/writes
//! must use `SessionDbClient`, never router calls.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RouterClient;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnqueueTurnRequest {
    pub turn_id: String,
    pub session_id: String,
    pub payload: Value,
}

impl RouterClient {
    pub fn global() -> Self {
        Self
    }

    pub fn health_check(&self) -> Result<Value> {
        crate::router_process::global_router_process()?.call("health_check", json!({}))
    }

    pub fn enqueue_turn(&self, request: EnqueueTurnRequest) -> Result<Value> {
        let payload = enqueue_turn_payload(request)?;
        crate::router_process::global_router_process()?
            .call("execution.enqueue_turn", payload)
            .map_err(|error| anyhow!("router execution enqueue failed: {error}"))
    }

    pub fn cancel_turn(&self, session_id: &str, active_turn_id: Option<&str>) -> Result<Value> {
        crate::router_process::global_router_process()?.call(
            "execution.cancel_turn",
            cancel_turn_payload(session_id, active_turn_id),
        )
    }

    pub fn kill_session_workers(&self, session_id: &str) -> Result<Value> {
        crate::router_process::global_router_process()?.call(
            "execution.kill_session_workers",
            kill_session_workers_payload(session_id),
        )
    }

    pub fn probe_sessions(&self, session_ids: &[String]) -> Result<Value> {
        crate::router_process::global_router_process()?.call_existing_with_timeout(
            "execution.probe_sessions",
            json!({ "session_ids": session_ids }),
            Duration::from_secs(5),
        )
    }

    pub fn append_user_command(
        &self,
        session_id: &str,
        root_session_id: &str,
        command: &str,
    ) -> Result<Value> {
        crate::router_process::global_router_process()?.call(
            "session.append_user_command",
            json!({
                "session_id": session_id,
                "root_session_id": root_session_id,
                "command": command,
            }),
        )
    }

    pub fn clear_user_commands(&self, session_id: &str, root_session_id: &str) -> Result<Value> {
        crate::router_process::global_router_process()?.call(
            "session.clear_user_commands",
            json!({
                "session_id": session_id,
                "root_session_id": root_session_id,
            }),
        )
    }

    pub fn shutdown(&self) -> Result<Value> {
        crate::router_process::global_router_process()?.shutdown()
    }
}

fn enqueue_turn_payload(request: EnqueueTurnRequest) -> Result<Value> {
    serde_json::to_value(request).map_err(Into::into)
}

fn cancel_turn_payload(session_id: &str, active_turn_id: Option<&str>) -> Value {
    json!({ "session_id": session_id, "active_turn_id": active_turn_id })
}

fn kill_session_workers_payload(session_id: &str) -> Value {
    json!({ "session_id": session_id })
}

#[cfg(test)]
mod tests {
    use super::{
        cancel_turn_payload, enqueue_turn_payload, kill_session_workers_payload, EnqueueTurnRequest,
    };
    use serde_json::json;

    #[test]
    fn enqueue_turn_payload_preserves_turn_session_and_nested_payload() {
        let payload = enqueue_turn_payload(EnqueueTurnRequest {
            turn_id: "turn-1".to_string(),
            session_id: "session-1".to_string(),
            payload: json!({
                "prompt": "hello",
                "worker_env": { "TURA_REASONING_EFFORT": "low" }
            }),
        })
        .expect("enqueue request should serialize");

        assert_eq!(payload["turn_id"], "turn-1");
        assert_eq!(payload["session_id"], "session-1");
        assert_eq!(payload["payload"]["prompt"], "hello");
        assert_eq!(
            payload["payload"]["worker_env"]["TURA_REASONING_EFFORT"],
            "low"
        );
    }

    #[test]
    fn cancel_turn_payload_serializes_optional_active_turn_id() {
        assert_eq!(
            cancel_turn_payload("session-1", Some("turn-1")),
            json!({ "session_id": "session-1", "active_turn_id": "turn-1" })
        );
        assert_eq!(
            cancel_turn_payload("session-1", None),
            json!({ "session_id": "session-1", "active_turn_id": null })
        );
    }

    #[test]
    fn kill_session_workers_payload_targets_session_runtime() {
        assert_eq!(
            kill_session_workers_payload("session-1"),
            json!({ "session_id": "session-1" })
        );
    }
}
