use super::connection::{init_workspace_db, with_connection};
use super::helpers::{
    bounded_page, management_task_management, millis_at, millis_to_rfc3339, parse_json_field,
    remove_sqlite_files, session_state_from_management, session_state_text, string_at,
    transition_management_to_interrupted,
};
use super::payload::{load_workspace_session_payload, mark_workspace_session_interrupted};
use lifecycle::SessionState;
use rusqlite::params;
use serde_json::{json, Value};
use std::path::Path;

fn insert_workspace_session(
    db_path: &Path,
    session_id: &str,
    state: SessionState,
    updated_at: i64,
) {
    let state_text = session_state_text(state).expect("state text");
    let management = json!({
        "session_id": session_id,
        "session_name": "Test session",
        "state": state_text,
        "session_last_update_at": "2026-01-01T00:00:00.000Z",
        "task_plan": {
            "plan_summary": "Plan",
            "detailed_tasks": [{"id": "task-1"}]
        }
    });
    let session = json!({
        "id": session_id,
        "directory": "C:/workspace",
        "updated_at": updated_at,
        "status": state.ui_status(),
        "management": management
    });
    with_connection(db_path, init_workspace_db, |conn| {
        conn.execute(
            "INSERT INTO sessions(
                    session_id, workspace, name, parent_id, created_at, updated_at,
                    state, status, message_count, task_management_json, management_json,
                    session_json, todos_json
                ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                session_id,
                "C:/workspace",
                "Test session",
                10_i64,
                updated_at,
                state_text,
                state.ui_status(),
                0_i64,
                serde_json::to_string(
                    &management_task_management(&management).expect("task management")
                )
                .expect("task json"),
                serde_json::to_string(&management).expect("management json"),
                serde_json::to_string(&session).expect("session json"),
                "[]",
            ],
        )?;
        Ok(())
    })
    .expect("insert workspace session");
}

#[test]
fn state_text_and_management_state_use_canonical_snake_case_only() {
    assert_eq!(
        session_state_text(SessionState::Interrupted).expect("state text"),
        "interrupted"
    );
    assert_eq!(
        session_state_from_management(&json!({"state":"running"}), "s1").expect("running state"),
        SessionState::Running
    );

    let missing = session_state_from_management(&json!({}), "s1")
        .expect_err("missing state should fail")
        .to_string();
    assert!(missing.contains("session management state missing for session s1"));

    let invalid = session_state_from_management(&json!({"state":"Running"}), "s1")
        .expect_err("PascalCase is not an internal state")
        .to_string();
    assert!(invalid.contains("invalid canonical session state for session s1"));
}

#[test]
fn transition_to_interrupted_updates_only_recoverable_states() {
    let now_ms = 1_789_123_456_789_i64;
    let mut running = json!({"state":"running"});
    assert!(
        transition_management_to_interrupted(&mut running, "running-session", now_ms)
            .expect("running should transition")
    );
    assert_eq!(running["state"], "interrupted");
    assert_eq!(
        running["session_last_update_at"],
        millis_to_rfc3339(now_ms).expect("timestamp")
    );

    let mut paused = json!({"state":"paused"});
    assert!(
        transition_management_to_interrupted(&mut paused, "paused-session", now_ms)
            .expect("paused should transition")
    );
    assert_eq!(paused["state"], "interrupted");

    for terminal in ["created", "completed", "failed", "cancelled", "interrupted"] {
        let mut management = json!({"state": terminal});
        assert!(
            !transition_management_to_interrupted(&mut management, "terminal-session", now_ms)
                .expect("non-running state should not transition"),
            "{terminal}"
        );
        assert_eq!(management["state"], terminal);
    }
}

#[test]
fn mark_workspace_session_interrupted_updates_workspace_payload_atomically() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("workspace.sqlite3");
    insert_workspace_session(&db_path, "s-running", SessionState::Running, 100);

    let management = mark_workspace_session_interrupted(&db_path, "s-running", 200)
        .expect("mark interrupted")
        .expect("running session should be updated");

    assert_eq!(management["state"], "interrupted");
    let payload = load_workspace_session_payload(&db_path, "s-running")
        .expect("load payload")
        .expect("payload exists");
    assert_eq!(payload.state.as_deref(), Some("interrupted"));
    assert_eq!(payload.status.as_deref(), Some("error"));
    assert_eq!(payload.updated_at, 200);
    assert_eq!(payload.management["state"], "interrupted");
    assert_eq!(payload.session["status"], "error");
    assert_eq!(payload.session["updated_at"], 200);
}

#[test]
fn mark_workspace_session_interrupted_skips_missing_nonexistent_and_terminal_sessions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("workspace.sqlite3");
    insert_workspace_session(&db_path, "s-done", SessionState::Completed, 100);

    assert!(mark_workspace_session_interrupted(&db_path, "missing", 200)
        .expect("missing row should be harmless")
        .is_none());
    assert!(mark_workspace_session_interrupted(&db_path, "s-done", 200)
        .expect("terminal state should be harmless")
        .is_none());
    assert!(
        mark_workspace_session_interrupted(&dir.path().join("absent.sqlite3"), "s1", 200)
            .expect("missing DB should be harmless")
            .is_none()
    );

    let payload = load_workspace_session_payload(&db_path, "s-done")
        .expect("load payload")
        .expect("payload exists");
    assert_eq!(payload.state.as_deref(), Some("completed"));
    assert_eq!(payload.updated_at, 100);
}

#[test]
fn load_workspace_payload_reports_json_corruption_with_session_context() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("workspace.sqlite3");
    insert_workspace_session(&db_path, "s-bad-json", SessionState::Running, 100);
    with_connection(&db_path, init_workspace_db, |conn| {
        conn.execute(
            "UPDATE sessions SET management_json = '{bad json' WHERE session_id = ?1",
            params!["s-bad-json"],
        )?;
        Ok(())
    })
    .expect("corrupt json");

    let error = match load_workspace_session_payload(&db_path, "s-bad-json") {
        Ok(_) => panic!("corrupt JSON should fail"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("failed to parse management_json for session s-bad-json"));
}

#[test]
fn helper_extractors_cover_nested_strings_millis_and_task_management() {
    let value = json!({
        "a": { "b": "text" },
        "time": "2026-06-12T01:02:03.004Z",
        "task_plan": {
            "plan_summary": "Keep it tidy",
            "detailed_tasks": [{"title": "one"}]
        }
    });

    assert_eq!(string_at(&value, &["a", "b"]).as_deref(), Some("text"));
    assert_eq!(string_at(&value, &["a", "missing"]), None);
    assert_eq!(millis_at(&value, &["time"]), Some(1_781_226_123_004));
    assert_eq!(
        millis_at(&json!({"time":"not a timestamp"}), &["time"]),
        None
    );

    let task = management_task_management(&value).expect("task management");
    assert_eq!(task["plan_summary"], "Keep it tidy");
    assert_eq!(task["tasks"][0]["title"], "one");
    assert!(management_task_management(&json!({})).is_none());
}

#[test]
fn pagination_bounds_match_session_and_record_listing_contracts() {
    assert_eq!(bounded_page(0, 25, 0, false), 0);
    assert_eq!(bounded_page(99, 10, 95, false), 9);
    assert_eq!(bounded_page(0, 10, 95, false), 0);
    assert_eq!(bounded_page(0, 10, 95, true), 9);
    assert_eq!(bounded_page(2, 10, 95, true), 2);
}

#[test]
fn parse_json_field_and_timestamp_errors_are_contextual() {
    let parsed: Value =
        parse_json_field(r#"{"ok":true}"#, "payload", Some("s1")).expect("valid json");
    assert_eq!(parsed["ok"], true);

    let error = parse_json_field::<Value>("{bad", "payload", Some("s1"))
        .expect_err("bad json")
        .to_string();
    assert!(error.contains("failed to parse payload for session s1"));

    let error = millis_to_rfc3339(i64::MAX)
        .expect_err("timestamp overflow should fail")
        .to_string();
    assert!(error.contains("invalid session timestamp millis"));
}

#[test]
fn remove_sqlite_files_removes_db_wal_and_shm_idempotently() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("store.sqlite3");
    std::fs::write(&db_path, b"db").expect("db file");
    std::fs::write(format!("{}-wal", db_path.display()), b"wal").expect("wal file");
    std::fs::write(format!("{}-shm", db_path.display()), b"shm").expect("shm file");

    remove_sqlite_files(&db_path).expect("remove sqlite files");
    assert!(!db_path.exists());
    assert!(!Path::new(&format!("{}-wal", db_path.display())).exists());
    assert!(!Path::new(&format!("{}-shm", db_path.display())).exists());

    remove_sqlite_files(&db_path).expect("second remove is idempotent");
}
