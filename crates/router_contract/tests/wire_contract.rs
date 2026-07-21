use router_contract::{
    CancelRuntimeRequest, EnqueueTurnRequest, IpcRequest, IpcResponse, ListCommandsRequest,
    PatchToolRequest, ProbeSessionsRequest, RouterEndpoint,
};
use serde_json::json;

#[test]
fn router_ipc_request_and_response_shapes_are_stable() {
    assert_eq!(
        serde_json::to_value(IpcRequest::health_check("health-1", 20_000)).expect("health request"),
        json!({
            "request_id": "health-1",
            "kind": "health_check",
            "method": "health_check",
            "payload": {},
            "deadline_ms": 20_000
        })
    );
    assert_eq!(
        serde_json::to_value(IpcResponse::ok("request-1", json!({ "status": "ok" })))
            .expect("response"),
        json!({
            "request_id": "request-1",
            "ok": true,
            "payload": { "status": "ok" },
            "error": null
        })
    );
}

#[test]
fn router_endpoint_shape_is_stable() {
    let endpoint = RouterEndpoint {
        addr: "127.0.0.1:7788".to_string(),
        version: "0.1.0+debug".to_string(),
        pid: Some(42),
        process_start_time: Some(84),
    };
    assert_eq!(
        serde_json::to_value(endpoint).expect("router endpoint"),
        json!({
            "addr": "127.0.0.1:7788",
            "version": "0.1.0+debug",
            "pid": 42,
            "process_start_time": 84
        })
    );
}

#[test]
fn runtime_routing_requests_reject_extra_fields() {
    assert!(serde_json::from_value::<EnqueueTurnRequest>(json!({
        "runtime_id": "runtime-1",
        "session_id": "session-1",
        "payload": {},
        "turn_id": "legacy"
    }))
    .is_err());
    assert!(serde_json::from_value::<CancelRuntimeRequest>(json!({
        "session_id": "session-1",
        "runtime_id": "runtime-1",
        "extra": true
    }))
    .is_err());
    assert!(serde_json::from_value::<ProbeSessionsRequest>(json!({
        "session_ids": ["session-1"],
        "extra": true
    }))
    .is_err());
}

#[test]
fn registry_requests_reject_extra_fields() {
    assert!(serde_json::from_value::<ListCommandsRequest>(json!({
        "directory": null,
        "legacy": true
    }))
    .is_err());
    assert!(serde_json::from_value::<PatchToolRequest>(json!({
        "repo_root": "C:/repo",
        "tool_id": "read_media",
        "patch": { "enabled": true, "legacy": true }
    }))
    .is_err());
}
