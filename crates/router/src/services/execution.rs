//! Router-owned execution supervision.
//!
//! This module owns runtime worker lifecycle decisions. Gateway may enqueue or
//! cancel turns, but must not spawn runtime workers directly.

use anyhow::{anyhow, Result};
use lifecycle::RuntimeId;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::services::runtime_workers::{MAX_ACTIVE_RUNTIME_WORKERS, MAX_QUEUED_RUNTIME_TURNS};
use crate::{dispatch_run_agent_with_runtime_slot, AppState};
use router_contract::{CancelRuntimeRequest, EnqueueTurnRequest, ProbeSessionsRequest};
use runtime_contract::RunAgentRequest;
use session_log_contract::{
    ActivateRuntimeLeaseRequest, RegisterRuntimeRequest, RuntimeLeaseOutcome,
    RuntimeRegistrationOutcome, SessionLogCommand, SessionLogResponse,
};

#[derive(Clone)]
pub struct ExecutionService {
    sessions: Arc<Mutex<HashMap<String, RuntimeLease>>>,
    runtime_slots: Arc<Semaphore>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeLease {
    runtime_id: RuntimeId,
    lease_id: String,
    slot_acquired: bool,
}

impl ExecutionService {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            runtime_slots: Arc::new(Semaphore::new(MAX_ACTIVE_RUNTIME_WORKERS)),
        }
    }

    #[allow(dead_code)]
    pub async fn enqueue_turn(&self, state: &AppState, input: Value) -> Result<Value> {
        self.enqueue_turn_request(state, input, "").await
    }

    pub async fn enqueue_turn_request(
        &self,
        state: &AppState,
        input: Value,
        request_id: &str,
    ) -> Result<Value> {
        let request: EnqueueTurnRequest = serde_json::from_value(input)?;
        let lease_id = format!("lease-{}", uuid::Uuid::new_v4());
        let run_request = payload_to_run_agent_request(&request, &lease_id)?;
        if debug_runtime_enabled() {
            eprintln!(
                "router debug: enqueue_turn start session_id={} runtime_id={}",
                request.session_id, request.runtime_id
            );
        }
        {
            let mut sessions = self.sessions.lock();
            if let Some(active) = sessions.get(&request.session_id) {
                return Ok(json!({
                    "ok": false,
                    "code": "session_active_turn",
                    "session_id": request.session_id,
                    "runtime_id": active.runtime_id,
                    "error": format!("session {} already has an active turn", request.session_id),
                }));
            }
            let queued = sessions
                .values()
                .filter(|lease| !lease.slot_acquired)
                .count();
            if queued >= MAX_QUEUED_RUNTIME_TURNS {
                return Err(anyhow!(
                    "runtime turn queue is full ({queued}/{MAX_QUEUED_RUNTIME_TURNS})"
                ));
            }
            sessions.insert(
                request.session_id.clone(),
                RuntimeLease {
                    runtime_id: request.runtime_id.clone(),
                    lease_id: lease_id.clone(),
                    slot_acquired: false,
                },
            );
        }
        let active_guard = ActiveSessionGuard::new(
            Arc::clone(&self.sessions),
            &request.session_id,
            &request.runtime_id,
        );
        let _permit = self.acquire_runtime_slot(&request.session_id).await?;
        if !self.mark_slot_acquired(&request.session_id, &request.runtime_id) {
            return Err(anyhow!(
                "session {} was cancelled before runtime dispatch",
                request.session_id
            ));
        }
        state.session_db.start()?;
        register_and_activate_runtime(&request.session_id, &request.runtime_id, &lease_id)?;
        if debug_runtime_enabled() {
            eprintln!(
                "router debug: enqueue_turn dispatch session_id={}",
                request.session_id
            );
        }
        let (status, body) =
            dispatch_run_agent_with_runtime_slot(state, run_request, request_id.to_string()).await;
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
            "runtime_id": request.runtime_id,
            "session_id": request.session_id,
            "result": body
        }))
    }

    pub async fn cancel_turn(&self, state: &AppState, input: Value) -> Value {
        let request = match serde_json::from_value::<CancelRuntimeRequest>(input) {
            Ok(request)
                if !request.session_id.trim().is_empty()
                    && !request.runtime_id.trim().is_empty() =>
            {
                request
            }
            Ok(_) => {
                return json!({
                    "status": "error",
                    "error": "session_id and runtime_id must be non-empty",
                    "stopped_worker": false,
                });
            }
            Err(error) => {
                return json!({
                    "status": "error",
                    "error": format!("invalid cancel runtime request: {error}"),
                    "stopped_worker": false,
                });
            }
        };
        let session_id = request.session_id;
        let runtime_id = request.runtime_id;
        let removed = {
            let mut sessions = self.sessions.lock();
            if sessions
                .get(&session_id)
                .is_some_and(|lease| lease.runtime_id == runtime_id)
            {
                sessions.remove(&session_id).is_some()
            } else {
                false
            }
        };
        let stopped_worker = if removed {
            state
                .manager
                .stop_worker_by_key(&format!("runtime_worker:{session_id}"))
                .await
        } else {
            false
        };
        json!({
            "status": if removed || stopped_worker { "cancelling" } else { "idle" },
            "session_id": session_id,
            "runtime_id": runtime_id,
            "stopped_worker": stopped_worker
        })
    }

    pub async fn kill_session_workers(&self, state: &AppState, input: Value) -> Value {
        let session_id = input
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let Some(session_id) = session_id else {
            let stopped = state
                .manager
                .stop_workers_with_prefix("runtime_worker:")
                .await;
            let active_turns_removed = self.sessions.lock().drain().count();
            return json!({
                "status": "stopped",
                "stopped": stopped,
                "stopped_worker": stopped > 0,
                "active_turns_removed": active_turns_removed
            });
        };

        let active_turn_removed = self.sessions.lock().remove(&session_id).is_some();
        let stopped_worker = state
            .manager
            .stop_worker_by_key(&format!("runtime_worker:{session_id}"))
            .await;
        json!({
            "status": "stopped",
            "session_id": session_id,
            "stopped": usize::from(stopped_worker),
            "stopped_worker": stopped_worker,
            "active_turn_removed": active_turn_removed
        })
    }

    pub async fn probe_sessions(&self, state: &AppState, input: Value) -> Result<Value> {
        let request: ProbeSessionsRequest = serde_json::from_value(input)?;
        let states = self.sessions.lock().clone();
        let mut sessions = Vec::new();
        for session_id in request
            .session_ids
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            let lease = states.get(&session_id);
            let queued_turn = lease.is_some_and(|lease| !lease.slot_acquired);
            let running_turn = lease.is_some_and(|lease| lease.slot_acquired);
            let active_turn = lease.is_some();
            let worker_alive = state
                .manager
                .worker_alive_by_key(&format!("runtime_worker:{session_id}"))
                .await;
            let status = if queued_turn {
                "queued"
            } else if running_turn || worker_alive {
                "running"
            } else {
                "inactive"
            };
            sessions.push(json!({
                "session_id": session_id,
                "runtime_id": lease.map(|lease| lease.runtime_id.clone()),
                "active_turn": active_turn,
                "queued_turn": queued_turn,
                "running_turn": running_turn,
                "worker_alive": worker_alive,
                "status": status
            }));
        }
        Ok(json!({ "sessions": sessions }))
    }

    pub fn active_session_count(&self) -> usize {
        self.sessions.lock().len()
    }

    #[cfg(test)]
    pub(crate) fn set_session_lease_for_test(&self, session_id: &str, slot_acquired: bool) {
        self.sessions.lock().insert(
            session_id.to_string(),
            RuntimeLease {
                runtime_id: format!("runtime-{session_id}"),
                lease_id: format!("lease-{session_id}"),
                slot_acquired,
            },
        );
    }

    async fn acquire_runtime_slot(&self, session_id: &str) -> Result<OwnedSemaphorePermit> {
        if debug_runtime_enabled() {
            eprintln!(
                "router debug: enqueue_turn waiting for runtime slot session_id={session_id}"
            );
        }
        let permit = Arc::clone(&self.runtime_slots)
            .acquire_owned()
            .await
            .map_err(|_| anyhow!("runtime worker queue is closed"))?;
        if debug_runtime_enabled() {
            eprintln!("router debug: enqueue_turn acquired runtime slot session_id={session_id}");
        }
        Ok(permit)
    }

    fn mark_slot_acquired(&self, session_id: &str, runtime_id: &str) -> bool {
        let mut sessions = self.sessions.lock();
        let Some(lease) = sessions.get_mut(session_id) else {
            return false;
        };
        if lease.runtime_id != runtime_id {
            return false;
        }
        lease.slot_acquired = true;
        true
    }
}

struct ActiveSessionGuard {
    sessions: Arc<Mutex<HashMap<String, RuntimeLease>>>,
    session_id: String,
    runtime_id: RuntimeId,
    active: std::sync::atomic::AtomicBool,
}

impl ActiveSessionGuard {
    fn new(
        sessions: Arc<Mutex<HashMap<String, RuntimeLease>>>,
        session_id: &str,
        runtime_id: &str,
    ) -> Self {
        Self {
            sessions,
            session_id: session_id.to_string(),
            runtime_id: runtime_id.to_string(),
            active: std::sync::atomic::AtomicBool::new(true),
        }
    }

    fn finish(&self) {
        self.active
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.remove_matching_lease();
    }

    fn remove_matching_lease(&self) {
        let mut sessions = self.sessions.lock();
        if sessions
            .get(&self.session_id)
            .is_some_and(|lease| lease.runtime_id == self.runtime_id)
        {
            sessions.remove(&self.session_id);
        }
    }
}

impl Drop for ActiveSessionGuard {
    fn drop(&mut self) {
        if self.active.load(std::sync::atomic::Ordering::SeqCst) {
            self.remove_matching_lease();
        }
    }
}

fn payload_to_run_agent_request(
    request: &EnqueueTurnRequest,
    lease_id: &str,
) -> Result<RunAgentRequest> {
    let mut value = request.payload.clone();
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "session_id".to_string(),
            Value::String(request.session_id.clone()),
        );
        object.insert(
            "runtime_id".to_string(),
            Value::String(request.runtime_id.clone()),
        );
        object.insert("lease_id".to_string(), Value::String(lease_id.to_string()));
    }
    serde_json::from_value(value).map_err(|error| {
        anyhow!(
            "invalid run-agent payload for runtime {} session {}: {error}",
            request.runtime_id,
            request.session_id
        )
    })
}

fn register_and_activate_runtime(session_id: &str, runtime_id: &str, lease_id: &str) -> Result<()> {
    let response = session_log_contract::client::call_service(
        &SessionLogCommand::RegisterRuntime(RegisterRuntimeRequest {
            runtime_id: runtime_id.to_string(),
            session_id: session_id.to_string(),
            fallback_from_id: None,
        }),
    )?;
    match response {
        SessionLogResponse::RuntimeRegistered {
            result:
                RuntimeRegistrationOutcome::Registered { .. }
                | RuntimeRegistrationOutcome::AlreadyRegistered { .. },
        } => {}
        SessionLogResponse::RuntimeRegistered { result } => {
            return Err(anyhow!(
                "session_db rejected runtime {runtime_id} registration for session {session_id}: {result:?}"
            ));
        }
        SessionLogResponse::Error { error } => {
            return Err(anyhow!(
                "session_db failed runtime {runtime_id} registration for session {session_id}: {error}"
            ));
        }
        other => {
            return Err(anyhow!(
                "unexpected session_db registration response for runtime {runtime_id}: {other:?}"
            ));
        }
    }

    let response = session_log_contract::client::call_service(
        &SessionLogCommand::ActivateRuntimeLease(ActivateRuntimeLeaseRequest {
            runtime_id: runtime_id.to_string(),
            lease_id: lease_id.to_string(),
        }),
    )?;
    match response {
        SessionLogResponse::RuntimeLeaseActivated {
            result: RuntimeLeaseOutcome::Activated | RuntimeLeaseOutcome::AlreadyActive,
        } => Ok(()),
        SessionLogResponse::RuntimeLeaseActivated { result } => Err(anyhow!(
            "session_db rejected lease {lease_id} for runtime {runtime_id}: {result:?}"
        )),
        SessionLogResponse::Error { error } => Err(anyhow!(
            "session_db failed to activate lease for runtime {runtime_id}: {error}"
        )),
        other => Err(anyhow!(
            "unexpected session_db lease response for runtime {runtime_id}: {other:?}"
        )),
    }
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
    use crate::{
        build_state,
        services::{manager::ServiceManager, runtime_workers::MAX_ACTIVE_RUNTIME_WORKERS},
    };
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn payload_to_run_agent_request_injects_authoritative_session_id() {
        let request = EnqueueTurnRequest {
            runtime_id: "runtime-1".to_string(),
            session_id: "session-authoritative".to_string(),
            payload: json!({
                "session_id": "stale-session",
                "prompt": "hello",
                "model": "openai/gpt-test",
                "worker_env": { "TURA_REASONING_EFFORT": "low" }
            }),
        };

        let run = payload_to_run_agent_request(&request, "lease-test")
            .expect("valid enqueue payload should become run-agent request");

        assert_eq!(run.session_id.as_deref(), Some("session-authoritative"));
        assert_eq!(run.runtime_id, "runtime-1");
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
            runtime_id: "runtime-invalid".to_string(),
            session_id: "session-invalid".to_string(),
            payload: json!({
                "worker_env": "not-an-object"
            }),
        };

        let error = payload_to_run_agent_request(&request, "lease-test")
            .expect_err("invalid worker_env shape should be rejected");

        assert!(
            error.to_string().contains("invalid run-agent payload")
                && error.to_string().contains("runtime-invalid")
                && error.to_string().contains("session-invalid"),
            "invalid payload error should include runtime and session context: {error}"
        );
    }

    #[tokio::test]
    async fn cancel_idle_turn_reports_idle_without_worker_stop() {
        let state = build_state();
        let response = ExecutionService::new()
            .cancel_turn(
                &state,
                json!({
                    "session_id": "idle-session",
                    "runtime_id": "runtime-idle-session"
                }),
            )
            .await;

        assert_eq!(response["status"], "idle");
        assert_eq!(response["session_id"], "idle-session");
        assert_eq!(response["stopped_worker"], false);
    }

    #[tokio::test]
    async fn cancel_active_turn_clears_active_session_without_worker() {
        let state = build_state();
        let service = ExecutionService::new();
        service.set_session_lease_for_test("active-session", true);

        let response = service
            .cancel_turn(
                &state,
                json!({
                    "session_id": "active-session",
                    "runtime_id": "runtime-active-session"
                }),
            )
            .await;

        assert_eq!(response["status"], "cancelling");
        assert_eq!(response["session_id"], "active-session");
        assert_eq!(response["stopped_worker"], false);
        assert!(!service.sessions.lock().contains_key("active-session"));
    }

    #[tokio::test]
    async fn stale_runtime_cancel_does_not_remove_the_current_lease() {
        let state = build_state();
        let service = ExecutionService::new();
        service.set_session_lease_for_test("active-session", true);

        let response = service
            .cancel_turn(
                &state,
                json!({
                    "session_id": "active-session",
                    "runtime_id": "runtime-stale"
                }),
            )
            .await;

        assert_eq!(response["status"], "idle");
        assert!(service.sessions.lock().contains_key("active-session"));
    }

    #[test]
    fn execution_service_starts_with_no_active_runtime_workers() {
        let manager = ServiceManager::new();

        assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    }

    #[tokio::test]
    async fn probe_sessions_reports_active_and_inactive_sessions() {
        let state = build_state();
        let service = ExecutionService::new();
        service.set_session_lease_for_test("active-session", true);

        let response = service
            .probe_sessions(
                &state,
                json!({ "session_ids": ["active-session", "inactive-session"] }),
            )
            .await
            .expect("probe sessions");

        let sessions = response["sessions"]
            .as_array()
            .expect("sessions array should be present");
        assert_eq!(sessions[0]["session_id"], "active-session");
        assert_eq!(sessions[0]["status"], "running");
        assert_eq!(sessions[0]["active_turn"], true);
        assert_eq!(sessions[0]["running_turn"], true);
        assert_eq!(sessions[1]["session_id"], "inactive-session");
        assert_eq!(sessions[1]["status"], "inactive");
    }

    #[tokio::test]
    async fn probe_sessions_reports_queued_turns_as_active_without_worker() {
        let state = build_state();
        let service = ExecutionService::new();
        service.set_session_lease_for_test("queued-session", false);

        let response = service
            .probe_sessions(&state, json!({ "session_ids": ["queued-session"] }))
            .await
            .expect("probe sessions");

        let sessions = response["sessions"]
            .as_array()
            .expect("sessions array should be present");
        assert_eq!(sessions[0]["session_id"], "queued-session");
        assert_eq!(sessions[0]["runtime_id"], "runtime-queued-session");
        assert_eq!(sessions[0]["status"], "queued");
        assert_eq!(sessions[0]["active_turn"], true);
        assert_eq!(sessions[0]["queued_turn"], true);
        assert_eq!(sessions[0]["worker_alive"], false);
    }

    #[tokio::test]
    async fn enqueue_turn_reports_active_session_as_structured_payload_without_dispatch() {
        let state = build_state();
        let service = ExecutionService::new();
        service.set_session_lease_for_test("active-session", true);

        let response = service
            .enqueue_turn(
                &state,
                json!({
                    "runtime_id": "active-runtime-2",
                    "session_id": "active-session",
                    "payload": {
                        "prompt": "append instead of failing"
                    }
                }),
            )
            .await
            .expect("active-session rejection is a gateway-handled payload");

        assert_eq!(response["ok"], false);
        assert_eq!(response["code"], "session_active_turn");
        assert_eq!(response["session_id"], "active-session");
        assert_eq!(response["runtime_id"], "runtime-active-session");
        assert!(service.sessions.lock().contains_key("active-session"));
        assert_eq!(
            state.manager.count_workers_with_prefix("runtime_worker:"),
            0
        );
    }

    #[tokio::test]
    async fn acquire_runtime_slot_queues_above_runtime_worker_limit_instead_of_rejecting() {
        let service = Arc::new(ExecutionService::new());
        let mut permits = Vec::new();
        for index in 0..MAX_ACTIVE_RUNTIME_WORKERS {
            permits.push(
                service
                    .acquire_runtime_slot(&format!("running-{index}"))
                    .await
                    .expect("initial runtime slots should be available"),
            );
        }

        service.set_session_lease_for_test("queued-session", false);
        let queued_service = Arc::clone(&service);
        let queued = tokio::spawn(async move {
            queued_service
                .acquire_runtime_slot("queued-session")
                .await
                .expect("queued turn should acquire the released runtime slot")
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !queued.is_finished(),
            "turn above the runtime worker limit should wait in the queue, not fail immediately"
        );

        drop(permits.pop());
        let permit = tokio::time::timeout(std::time::Duration::from_secs(1), queued)
            .await
            .expect("queued turn should resume after a runtime slot is released")
            .expect("queued task should not panic");
        drop(permit);
    }
}
