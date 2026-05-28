use chrono::Utc;
use gateway::{api::types::SessionStatus, SessionStore};

#[test]
fn explicit_start_condition_round_trips_and_idle_scheduler_claims_it() {
    let store = SessionStore::new();
    let now = Utc::now();
    let session = store.create_session(
        Some("C:/workspace".to_string()),
        None,
        None,
        Some("coding".to_string()),
        None,
        false,
        false,
        false,
        None,
        false,
        false,
    );

    let updated = store
        .update_session(
            &session.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "Run once the session is idle",
                "status": "todo",
                "start_condition": "session_idle"
            })),
        )
        .expect("task management should update");

    assert_eq!(updated.task_management["status"], serde_json::Value::Null);
    assert_eq!(updated.task_management["start_condition"], "session_idle");

    let claimed = store.claim_due_task_runs(now);

    let claimed_run = claimed
        .iter()
        .find(|run| run.session_id == session.id)
        .expect("newly created idle task should be claimed");
    assert_eq!(claimed_run.task_summary, "Run once the session is idle");
    assert_eq!(
        store
            .get_session(&session.id)
            .expect("session should exist")
            .status,
        SessionStatus::Busy
    );
}

#[test]
fn legacy_status_start_condition_still_round_trips() {
    let store = SessionStore::new();
    let session = store.create_session(
        Some("C:/workspace".to_string()),
        None,
        None,
        Some("coding".to_string()),
        None,
        false,
        false,
        false,
        None,
        false,
        false,
    );

    let updated = store
        .update_session(
            &session.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "Legacy queued task",
                "status": "session_idle"
            })),
        )
        .expect("legacy task management should update");

    assert!(updated.task_management.get("status").is_none());
    assert_eq!(updated.task_management["start_condition"], "session_idle");
}
