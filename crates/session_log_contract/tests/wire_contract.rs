use lifecycle::{SessionAggregate, SessionInput, SessionManagement, SessionQuery};
use serde_json::json;
use session_log_contract::{
    GetSessionRequest, ServiceEndpoint, SessionFeedEvent, SessionLogCommand, SessionLogResponse,
    SessionMetadata, SessionMetadataPatch, SessionSnapshot, UpdateSessionRequest,
    UpdateSessionTodosRequest,
};

fn snapshot_fixture(session_id: &str, workspace: &str) -> SessionSnapshot {
    let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(1)
        .expect("snapshot timestamp");
    let projection = SessionAggregate::new(session_id.to_string()).query(SessionQuery::Lifecycle);
    let mut management = SessionManagement::new(
        session_id.to_string(),
        "Session".to_string(),
        workspace.into(),
        false,
        Vec::<String>::new(),
        SessionInput {
            user_input: String::new(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        String::new(),
        timestamp,
    );
    management.replace_lifecycle_projection(projection.clone());
    SessionSnapshot {
        session_id: session_id.to_string(),
        workspace: workspace.to_string(),
        name: Some(management.session_name.clone()),
        created_at: 1,
        updated_at: 2,
        last_user_message_at: None,
        message_count: 0,
        lifecycle_projection: projection,
        metadata: SessionMetadata {
            session_directory: workspace.to_string(),
            model: None,
            agent: None,
            session_type: "coding".to_string(),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
            model_variant: None,
            model_acceleration_enabled: false,
            disable_permission_restrictions: management.disable_permission_restrictions,
            use_last_tool_call_response: management.use_last_tool_call_response,
            auto_session_name: management.auto_session_name,
            context_tokens: management.context_tokens,
            runtime_usage: management.runtime_usage.clone(),
        },
        management,
        todos: Vec::new(),
    }
}

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
    let snapshot = snapshot_fixture("session-1", "C:/workspace");
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
        assert!(value["snapshot"].get("metadata").is_some());
        assert!(value["snapshot"].get("session").is_none());
    }

    let canonical = serde_json::to_value(&snapshot).expect("canonical snapshot");
    for legacy_field in ["parent_id", "state", "status", "task_management", "session"] {
        let mut legacy = canonical.clone();
        legacy
            .as_object_mut()
            .expect("snapshot object")
            .insert(legacy_field.to_string(), json!(null));
        assert!(
            serde_json::from_value::<SessionSnapshot>(legacy).is_err(),
            "legacy snapshot field `{legacy_field}` must be rejected"
        );
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

#[test]
fn session_snapshot_rejects_split_lifecycle_truth_and_restores_full_projection() {
    let mut projection_id_mismatch = snapshot_fixture("session-1", "C:/workspace");
    projection_id_mismatch.lifecycle_projection.session_id = "session-2".to_string();
    assert!(projection_id_mismatch.into_management().is_err());

    let mut management_id_mismatch = snapshot_fixture("session-1", "C:/workspace");
    management_id_mismatch
        .management
        .rebind_session_id("session-2".to_string());
    assert!(management_id_mismatch.into_management().is_err());

    let mut state_mismatch = snapshot_fixture("session-1", "C:/workspace");
    state_mismatch.lifecycle_projection.state = lifecycle::SessionState::Running;
    assert!(state_mismatch.into_management().is_err());

    let mut task_plan_mismatch = snapshot_fixture("session-1", "C:/workspace");
    task_plan_mismatch
        .lifecycle_projection
        .task_plan
        .plan_summary = "different plan".to_string();
    assert!(task_plan_mismatch.into_management().is_err());

    let mut metadata_mismatch = snapshot_fixture("session-1", "C:/workspace");
    metadata_mismatch.metadata.auto_session_name = false;
    assert!(metadata_mismatch.into_management().is_err());

    let mut canonical = snapshot_fixture("session-1", "C:/workspace");
    canonical.lifecycle_projection.parent_id = Some("parent".to_string());
    canonical.lifecycle_projection.pending_user_inputs = vec!["queued".to_string()];
    canonical.lifecycle_projection.runtime_ids = vec!["runtime-1".to_string()];
    canonical.lifecycle_projection.active_runtime_id = Some("runtime-1".to_string());
    let expected = canonical.lifecycle_projection.clone();
    let management = canonical
        .into_management()
        .expect("canonical snapshot should restore management");
    assert_eq!(management.lifecycle_projection(), expected);
}
