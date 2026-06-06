//! Typed client for the persistent gateway-owned router process.
//!
//! This client is for execution supervision only. Session DB data reads/writes
//! must use `SessionDbClient`, never router calls.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

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
        crate::router_process::global_router_process().call("health_check", json!({}))
    }

    pub fn enqueue_turn(&self, request: EnqueueTurnRequest) -> Result<Value> {
        let payload = serde_json::to_value(request)?;
        crate::router_process::global_router_process()
            .call("execution.enqueue_turn", payload)
            .map_err(|error| anyhow!("router execution enqueue failed: {error}"))
    }

    pub fn cancel_turn(&self, session_id: &str, active_turn_id: Option<&str>) -> Result<Value> {
        crate::router_process::global_router_process().call(
            "execution.cancel_turn",
            json!({ "session_id": session_id, "active_turn_id": active_turn_id }),
        )
    }
}
