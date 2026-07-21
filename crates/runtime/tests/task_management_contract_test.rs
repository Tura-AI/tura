use std::path::PathBuf;

use chrono::Utc;
use lifecycle::{SessionInput, SessionManagement, StartCondition, TaskStep};

#[test]
fn task_management_json_exposes_start_condition_for_gateway_and_gui() {
    let now = Utc::now();
    let mut session = SessionManagement::new(
        "session-contract".to_string(),
        "Contract".to_string(),
        PathBuf::from("C:/workspace"),
        false,
        "coding".to_string(),
        SessionInput {
            user_input: "ship contract".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        "ship contract".to_string(),
        now,
    );
    session.task_plan.plan_summary = "Contract plan".to_string();
    session.task_plan.detailed_tasks.push(TaskStep {
        task_id: "idle-task".to_string(),
        step: 0,
        task_summary: "Queued work".to_string(),
        start_condition: StartCondition::SessionIdle,
        ..TaskStep::default()
    });

    let value = session.task_management_json();

    assert_eq!(value["task_summary"], "Queued work");
    assert_eq!(value["start_condition"], "session_idle");
    assert!(value.get("status").is_none());
}
