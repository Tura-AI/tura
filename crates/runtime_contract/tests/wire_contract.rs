use runtime_contract::{
    CallContext, GatewayCallbackFrame, WorkerEnvelope, GATEWAY_CALLBACK_KIND, WORKER_KIND_CALL,
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
fn gateway_callback_frame_preserves_kind_method_and_nested_payload() {
    let frame = GatewayCallbackFrame::new(
        "session.agent_stream",
        "session-1",
        json!({ "delta": "hi" }),
    );
    let encoded = serde_json::to_value(frame).expect("callback frame");
    assert_eq!(encoded["kind"], GATEWAY_CALLBACK_KIND);
    assert_eq!(encoded["method"], "session.agent_stream");
    assert_eq!(encoded["payload"]["session_id"], "session-1");
    assert_eq!(encoded["payload"]["body"]["delta"], "hi");
}
