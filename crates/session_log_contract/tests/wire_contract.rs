use lifecycle::{SessionAggregate, SessionQuery};
use serde_json::json;
use session_log_contract::{
    GetSessionRequest, ServiceEndpoint, SessionFeedEvent, SessionLogCommand, SessionLogResponse,
    SessionMetadataPatch, SessionSnapshot, UpdateSessionRequest, UpdateSessionTodosRequest,
};

#[test]
fn session_database_command_and_endpoint_shapes_are_stable() {
    assert_eq!(
        serde_json::to_value(SessionLogCommand::GetSession(GetSessionRequest {
            session_id: "session-1".to_string(),
        }))
        .expect("get-session command"),
        json!({ "command": "get_session", "session_id": "session-1" })
    );
    assert_eq!(
        serde_json::to_value(ServiceEndpoint {
            addr: "127.0.0.1:40123".to_string(),
            version: "0.1.0".to_string(),
        })
        .expect("endpoint"),
        json!({ "addr": "127.0.0.1:40123", "version": "0.1.0" })
    );
}

#[test]
fn session_feed_subscription_command_shape_is_stable() {
    assert_eq!(
        serde_json::to_value(SessionLogCommand::SubscribeSessionFeed)
            .expect("subscribe-session-feed command"),
        json!({ "command": "subscribe_session_feed" })
    );
}

#[test]
fn update_session_command_keeps_the_flat_strict_shape() {
    let command = SessionLogCommand::UpdateSession(UpdateSessionRequest {
        command_id: "update-1".to_string(),
        session_id: "session-1".to_string(),
        metadata: SessionMetadataPatch {
            name: Some("Renamed".to_string()),
            model: Some("model-1".to_string()),
            auto_session_name: Some(false),
            ..SessionMetadataPatch::default()
        },
        task_plan_patch: None,
    });
    assert_eq!(
        serde_json::to_value(command).expect("update-session command"),
        json!({
            "command": "update_session",
            "command_id": "update-1",
            "session_id": "session-1",
            "metadata": {
                "name": "Renamed",
                "model": "model-1",
                "agent": null,
                "clear_agent": false,
                "session_type": null,
                "kill_processes_on_start": null,
                "validator_enabled": null,
                "force_planning": null,
                "disable_permission_restrictions": null,
                "use_last_tool_call_response": null,
                "auto_session_name": false
            },
            "task_plan_patch": null
        })
    );
}

#[test]
fn update_session_todos_command_keeps_the_flat_strict_shape() {
    let command = SessionLogCommand::UpdateSessionTodos(UpdateSessionTodosRequest {
        command_id: "todos-1".to_string(),
        session_id: "session-1".to_string(),
        todos: vec![json!({"id": "todo-1", "status": "in_progress"})],
        updated_at: 42,
    });
    assert_eq!(
        serde_json::to_value(command).expect("update-session-todos command"),
        json!({
            "command": "update_session_todos",
            "command_id": "todos-1",
            "session_id": "session-1",
            "todos": [{"id": "todo-1", "status": "in_progress"}],
            "updated_at": 42
        })
    );
}

#[test]
fn update_session_todos_response_keeps_the_flat_shape() {
    let response = SessionLogResponse::SessionTodosUpdated {
        todos: vec![json!({"id": "todo-1", "status": "in_progress"})],
        cursor: 3,
    };
    assert_eq!(
        serde_json::to_value(response).expect("update-session-todos response"),
        json!({
            "kind": "session_todos_updated",
            "todos": [{"id": "todo-1", "status": "in_progress"}],
            "cursor": 3
        })
    );
}

#[test]
fn session_snapshot_feed_event_shapes_are_stable() {
    let projection = SessionAggregate::new("session-1".to_string()).query(SessionQuery::Lifecycle);
    let snapshot = SessionSnapshot {
        session_id: "session-1".to_string(),
        workspace: "C:/workspace".to_string(),
        name: Some("Session".to_string()),
        parent_id: None,
        created_at: 1,
        updated_at: 2,
        last_user_message_at: None,
        state: Some("created".to_string()),
        status: Some("idle".to_string()),
        message_count: 0,
        task_management: json!({}),
        lifecycle_projection: projection,
        management: json!({}),
        session: json!({}),
        todos: Vec::new(),
    };
    for (event_name, event) in [
        (
            "session_snapshot_created",
            SessionFeedEvent::SessionSnapshotCreated {
                snapshot: Box::new(snapshot.clone()),
            },
        ),
        (
            "session_snapshot_updated",
            SessionFeedEvent::SessionSnapshotUpdated {
                snapshot: Box::new(snapshot.clone()),
            },
        ),
    ] {
        let value = serde_json::to_value(event).expect("snapshot feed event");
        assert_eq!(value["event"], event_name);
        assert_eq!(value["snapshot"]["session_id"], "session-1");
        assert_eq!(value["snapshot"]["workspace"], "C:/workspace");
        assert_eq!(value["snapshot"]["name"], "Session");
        assert_eq!(
            value["snapshot"]["lifecycle_projection"]["state"],
            "created"
        );
        assert!(value["snapshot"].get("management").is_some());
        assert!(value["snapshot"].get("session").is_some());
    }

    assert_eq!(
        serde_json::to_value(SessionFeedEvent::SessionDeleted {}).expect("deleted feed event"),
        json!({ "event": "session_deleted" })
    );
    assert!(serde_json::from_value::<SessionFeedEvent>(json!({
        "event": "session_deleted",
        "snapshot": snapshot
    }))
    .is_err());
}
