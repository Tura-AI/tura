//! Router-owned execution supervision.
//!
//! This module owns runtime worker lifecycle decisions. Gateway may enqueue or
//! cancel turns, but must not spawn runtime workers directly.

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, sync::Arc};

use crate::{dispatch_run_agent, AppState, RunAgentRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueTurnRequest {
    pub turn_id: String,
    pub session_id: String,
    pub payload: Value,
}

#[derive(Clone)]
pub struct ExecutionService {
    active_sessions: Arc<Mutex<HashSet<String>>>,
}

impl ExecutionService {
    pub fn new() -> Self {
        Self {
            active_sessions: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn enqueue_turn(&self, state: &AppState, input: Value) -> Result<Value> {
        let request: EnqueueTurnRequest = serde_json::from_value(input)?;
        if debug_runtime_enabled() {
            eprintln!(
                "router debug: enqueue_turn start session_id={} turn_id={}",
                request.session_id, request.turn_id
            );
        }
        {
            let mut active = self.active_sessions.lock();
            if !active.insert(request.session_id.clone()) {
                return Err(anyhow!(
                    "session {} already has an active turn",
                    request.session_id
                ));
            }
        }

        let run_request = payload_to_run_agent_request(&request)?;
        if debug_runtime_enabled() {
            eprintln!(
                "router debug: enqueue_turn dispatch session_id={}",
                request.session_id
            );
        }
        let (status, body) = dispatch_run_agent(state, run_request).await;
        self.active_sessions.lock().remove(&request.session_id);
        if debug_runtime_enabled() {
            eprintln!(
                "router debug: enqueue_turn finished session_id={} status={} body={}",
                request.session_id, status, body
            );
        }
        if status >= 400 {
            return Err(anyhow!(
                "{}",
                body.pointer("/result/error")
                    .or_else(|| body.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("runtime worker failed")
            ));
        }
        Ok(json!({
            "status": "finished",
            "turn_id": request.turn_id,
            "session_id": request.session_id,
            "result": body
        }))
    }

    pub async fn cancel_turn(&self, state: &AppState, input: Value) -> Value {
        let session_id = input
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let removed = self.active_sessions.lock().remove(&session_id);
        let stopped_worker = state
            .manager
            .stop_worker_by_key(&format!("runtime_worker:{session_id}"))
            .await;
        json!({
            "status": if removed || stopped_worker { "cancelling" } else { "idle" },
            "session_id": session_id,
            "stopped_worker": stopped_worker
        })
    }
}

fn payload_to_run_agent_request(request: &EnqueueTurnRequest) -> Result<RunAgentRequest> {
    let mut value = request.payload.clone();
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "session_id".to_string(),
            Value::String(request.session_id.clone()),
        );
    }
    Ok(serde_json::from_value(value)?)
}

fn debug_runtime_enabled() -> bool {
    std::env::var("TURA_DEBUG_RUNTIME")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}
