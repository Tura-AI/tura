use router_contract::{IpcNotification, IpcRequest, IpcResponse, RouterEndpoint, RunAgentRequest};
use serde_json::json;

#[test]
fn router_ipc_request_response_and_notification_shapes_are_stable() {
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
    assert_eq!(
        serde_json::to_value(IpcNotification::new(
            "request-1",
            "gateway.callback",
            "session.agent_stream",
            json!({ "session_id": "session-1" })
        ))
        .expect("notification"),
        json!({
            "request_id": "request-1",
            "kind": "gateway.callback",
            "method": "session.agent_stream",
            "payload": { "session_id": "session-1" }
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
fn run_agent_defaults_match_the_existing_request_contract() {
    let request: RunAgentRequest = serde_json::from_value(json!({
        "session_id": "session-1",
        "prompt": "hello"
    }))
    .expect("run-agent request");
    assert_eq!(request.session_id.as_deref(), Some("session-1"));
    assert_eq!(request.prompt.as_deref(), Some("hello"));
    assert!(!request.return_log);
    assert!(request.worker_env.is_empty());
}
