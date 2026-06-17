//! Router-owned execution supervision.
//!
//! This module owns runtime worker lifecycle decisions. Gateway may enqueue or
//! cancel turns, but must not spawn runtime workers directly.

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, sync::Arc};

use crate::{dispatch_run_agent, ipc, AppState, RunAgentRequest};

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

    #[allow(dead_code)]
    pub async fn enqueue_turn(&self, state: &AppState, input: Value) -> Result<Value> {
        self.enqueue_turn_with_notifications(state, input, "", None)
            .await
    }

    pub async fn enqueue_turn_with_notifications(
        &self,
        state: &AppState,
        input: Value,
        request_id: &str,
        notifications: Option<ipc::IpcNotificationSender>,
    ) -> Result<Value> {
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
        let active_guard =
            ActiveSessionGuard::new(Arc::clone(&self.active_sessions), &request.session_id);

        let run_request = payload_to_run_agent_request(&request)?;
        if debug_runtime_enabled() {
            eprintln!(
                "router debug: enqueue_turn dispatch session_id={}",
                request.session_id
            );
        }
        let (status, body) =
            dispatch_run_agent(state, run_request, request_id.to_string(), notifications).await;
        active_guard.finish();
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

    pub fn active_session_count(&self) -> usize {
        self.active_sessions.lock().len()
    }
}

struct ActiveSessionGuard {
    sessions: Arc<Mutex<HashSet<String>>>,
    session_id: String,
    active: std::sync::atomic::AtomicBool,
}

impl ActiveSessionGuard {
    fn new(sessions: Arc<Mutex<HashSet<String>>>, session_id: &str) -> Self {
        Self {
            sessions,
            session_id: session_id.to_string(),
            active: std::sync::atomic::AtomicBool::new(true),
        }
    }

    fn finish(&self) {
        self.active
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.sessions.lock().remove(&self.session_id);
    }
}

impl Drop for ActiveSessionGuard {
    fn drop(&mut self) {
        if self.active.load(std::sync::atomic::Ordering::SeqCst) {
            self.sessions.lock().remove(&self.session_id);
        }
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
    serde_json::from_value(value).map_err(|error| {
        anyhow!(
            "invalid run-agent payload for turn {} session {}: {error}",
            request.turn_id,
            request.session_id
        )
    })
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

#[cfg(test)]
mod tests {
    use super::{payload_to_run_agent_request, EnqueueTurnRequest, ExecutionService};
    use crate::{build_state, services::manager::ServiceManager};
    use serde_json::json;

    #[test]
    fn payload_to_run_agent_request_injects_authoritative_session_id() {
        let request = EnqueueTurnRequest {
            turn_id: "turn-1".to_string(),
            session_id: "session-authoritative".to_string(),
            payload: json!({
                "session_id": "stale-session",
                "prompt": "hello",
                "model": "openai/gpt-test",
                "worker_env": { "TURA_REASONING_EFFORT": "low" }
            }),
        };

        let run = payload_to_run_agent_request(&request)
            .expect("valid enqueue payload should become run-agent request");

        assert_eq!(run.session_id.as_deref(), Some("session-authoritative"));
        assert_eq!(run.prompt.as_deref(), Some("hello"));
        assert_eq!(run.model.as_deref(), Some("openai/gpt-test"));
        assert_eq!(
            run.worker_env
                .get("TURA_REASONING_EFFORT")
                .map(String::as_str),
            Some("low")
        );
    }

    #[test]
    fn payload_to_run_agent_request_reports_invalid_payload_shape() {
        let request = EnqueueTurnRequest {
            turn_id: "turn-invalid".to_string(),
            session_id: "session-invalid".to_string(),
            payload: json!({
                "worker_env": "not-an-object"
            }),
        };

        let error = payload_to_run_agent_request(&request)
            .expect_err("invalid worker_env shape should be rejected");

        assert!(
            error.to_string().contains("invalid run-agent payload")
                && error.to_string().contains("turn-invalid")
                && error.to_string().contains("session-invalid"),
            "invalid payload error should include turn and session context: {error}"
        );
    }

    #[tokio::test]
    async fn cancel_idle_turn_reports_idle_without_worker_stop() {
        let state = build_state();
        let response = ExecutionService::new()
            .cancel_turn(&state, json!({ "session_id": "idle-session" }))
            .await;

        assert_eq!(response["status"], "idle");
        assert_eq!(response["session_id"], "idle-session");
        assert_eq!(response["stopped_worker"], false);
    }

    #[tokio::test]
    async fn cancel_active_turn_clears_active_session_without_worker() {
        let state = build_state();
        let service = ExecutionService::new();
        service
            .active_sessions
            .lock()
            .insert("active-session".to_string());

        let response = service
            .cancel_turn(&state, json!({ "session_id": "active-session" }))
            .await;

        assert_eq!(response["status"], "cancelling");
        assert_eq!(response["session_id"], "active-session");
        assert_eq!(response["stopped_worker"], false);
        assert!(!service.active_sessions.lock().contains("active-session"));
    }

    #[test]
    fn execution_service_starts_with_no_active_runtime_workers() {
        let manager = ServiceManager::new();

        assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    }
}
