use std::path::PathBuf;

use chrono::Utc;
use lifecycle::SessionState;
use runtime::state_machine::session_management::{SessionInput, SessionManagement};

fn persisted_session_value(state: SessionState) -> serde_json::Value {
    let now = Utc::now();
    let mut session = SessionManagement::new(
        "recovery-session".to_string(),
        "Recovery".to_string(),
        PathBuf::from("C:/workspace"),
        false,
        "coding".to_string(),
        SessionInput {
            user_input: "initial task".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        "initial task".to_string(),
        now,
    );
    session.push_log("history before interruption", now);
    let mut value = serde_json::to_value(session).expect("session should serialize");
    value["state"] = serde_json::to_value(state).expect("state should serialize");
    value
}

#[test]
fn interrupted_persisted_session_resumes_without_losing_history() {
    let value = persisted_session_value(SessionState::Interrupted);
    let mut decoded: SessionManagement =
        serde_json::from_value(value).expect("interrupted should deserialize");

    decoded.prepare_for_new_user_turn(
        SessionInput {
            user_input: "resume task".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        Utc::now(),
    );

    assert_eq!(decoded.state, SessionState::Created);
    assert_eq!(decoded.input.user_input, "resume task");
    assert_eq!(decoded.session_log, vec!["history before interruption"]);
}

#[test]
fn pascal_case_persisted_session_state_is_rejected() {
    let mut value = persisted_session_value(SessionState::Running);
    value["state"] = serde_json::json!("Running");

    let error = serde_json::from_value::<SessionManagement>(value)
        .expect_err("internal persisted state must be snake_case");

    assert!(
        error.to_string().contains("unknown variant") || error.to_string().contains("expected"),
        "unexpected error: {error}"
    );
}
