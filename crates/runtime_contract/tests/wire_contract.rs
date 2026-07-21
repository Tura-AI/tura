use runtime_contract::{
    CallContext, RunAgentRequest, RuntimeWorkerResponse, WorkerEnvelope, WORKER_KIND_CALL,
    WORKER_KIND_HEALTH_CHECK,
};
use serde_json::json;

#[test]
fn worker_envelopes_preserve_the_existing_wire_shape() {
    assert_eq!(
        serde_json::to_value(WorkerEnvelope::health_check()).expect("health envelope"),
        json!({ "kind": WORKER_KIND_HEALTH_CHECK, "payload": {} })
    );
    let call = WorkerEnvelope::call(CallContext {
        request_id: "request-1".to_string(),
        method: "POST".to_string(),
        path: "/runtime_worker/session-1".to_string(),
        input: json!({ "session_id": "session-1", "prompt": "hello" }),
    });
    assert_eq!(call.kind, WORKER_KIND_CALL);
    assert_eq!(call.payload["input"]["request_id"], "request-1");
    assert_eq!(call.payload["input"]["input"]["prompt"], "hello");
}

#[test]
fn run_agent_request_is_strict_and_defaults_optional_worker_inputs() {
    let request: RunAgentRequest = serde_json::from_value(json!({
        "runtime_id": "runtime-1",
        "lease_id": "lease-1",
        "session_id": "session-1",
        "prompt": "hello"
    }))
    .expect("run-agent request");
    assert_eq!(request.runtime_id, "runtime-1");
    assert_eq!(request.lease_id, "lease-1");
    assert_eq!(request.session_id.as_deref(), Some("session-1"));
    assert_eq!(request.prompt.as_deref(), Some("hello"));
    assert!(!request.no_op_manual);
    assert!(!request.return_log);
    assert!(request.worker_env.is_empty());
    assert!(serde_json::from_value::<RunAgentRequest>(json!({
        "runtime_id": "runtime-1",
        "turn_id": "legacy"
    }))
    .is_err());
}
#[test]
fn runtime_worker_response_rejects_unknown_fields_and_uses_typed_state() {
    let response: RuntimeWorkerResponse = serde_json::from_value(json!({
        "ok": true,
        "session_id": "session-1",
        "session_state": "completed",
        "message_count": 3,
        "turn_started_at_ms": 42,
        "final_text": "done",
        "session_log": []
    }))
    .expect("runtime response");
    assert_eq!(
        response.session_state,
        Some(lifecycle::SessionState::Completed)
    );
    assert!(serde_json::from_value::<RuntimeWorkerResponse>(json!({
        "ok": true,
        "legacy_status": "done"
    }))
    .is_err());
}
