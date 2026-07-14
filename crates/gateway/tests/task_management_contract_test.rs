use chrono::Utc;
use gateway::SessionStore;

#[test]
fn explicit_start_condition_round_trips_and_waits_for_user_action_when_already_idle() {
    let store = SessionStore::new();
    let now = Utc::now();
    let session = store.create_session(
        Some("C:/workspace".to_string()),
        None,
        None,
        Some("coding".to_string()),
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
            Some(serde_json::json!({
                "task_summary": "Run once the session is idle",
                "status": "todo",
                "start_condition": "session_idle"
            })),
        )
        .expect("task management should update");

    assert_eq!(updated.task_management["status"], "waiting_user");
    assert_eq!(updated.task_management["start_condition"], "session_idle");

    assert!(
        store.claim_due_task_runs(now).is_empty(),
        "newly created session_idle task should wait for user action while the session is already idle"
    );
}

#[test]
fn status_field_does_not_accept_start_condition_values() {
    let store = SessionStore::new();
    let session = store.create_session(
        Some("C:/workspace".to_string()),
        None,
        None,
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );
    let before = session.task_management.clone();

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
            Some(serde_json::json!({
                "task_summary": "Invalid queued task",
                "status": "session_idle"
            })),
        )
        .expect("invalid task management remains non-fatal");

    assert_eq!(updated.task_management, before);
}
