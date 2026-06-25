#[path = "helpers/task_scheduler.rs"]
mod helpers;
use helpers::*;
#[tokio::test]
async fn gateway_task_scheduler_business_flow_triggers_due_tasks_without_duplicate_runs() {
    let root = tempfile::tempdir().expect("temp scheduler root");
    let now = Utc::now();
    let due = now - chrono::Duration::seconds(30);
    let future = now + chrono::Duration::hours(1);
    let scheduled = session_store().create_session(
        Some(root.path().join("scheduled").to_string_lossy().to_string()),
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
    let polling = session_store().create_session(
        Some(root.path().join("polling").to_string_lossy().to_string()),
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
    let future_session = session_store().create_session(
        Some(root.path().join("future").to_string_lossy().to_string()),
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

    session_store().update_session(
        &scheduled.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(json!({
            "task_id": "scheduled-task",
            "task_summary": "Run the scheduled local maintenance task",
            "status": "todo",
            "start_at": due.to_rfc3339()
        })),
    );
    session_store().update_session(
        &polling.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(json!({
            "task_id": "polling-task",
            "task_summary": "Poll local state and continue if needed",
            "status": "todo",
            "start_at": due.to_rfc3339(),
            "poll_interval": { "d": 0, "h": 0, "m": 5, "s": 0 }
        })),
    );
    session_store().update_session(
        &future_session.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(json!({
            "task_id": "future-task",
            "task_summary": "Future task must not run early",
            "status": "todo",
            "start_at": future.to_rfc3339()
        })),
    );

    run_due_task_scheduler_tick_for_business_test();

    assert_scheduler_triggered(
        &scheduled.id,
        "scheduled_task",
        "scheduled start time arrived",
        "Run the scheduled local maintenance task",
    );
    assert_scheduler_triggered(
        &polling.id,
        "polling_task",
        "polling interval became due",
        "Poll local state and continue if needed",
    );
    assert_eq!(
        session_store().get_messages(&future_session.id).len(),
        0,
        "future scheduled tasks must not run before start_at"
    );
    assert_eq!(
        session_store()
            .get_session(&future_session.id)
            .expect("future session")
            .status,
        ApiSessionStatus::Idle
    );

    let polling_after = session_store()
        .get_session(&polling.id)
        .expect("polling session after tick");
    let next_start = DateTime::parse_from_rfc3339(
        polling_after.task_management["start_at"]
            .as_str()
            .expect("polling start_at"),
    )
    .expect("parse polling start_at")
    .with_timezone(&Utc);
    assert!(
        next_start > now,
        "polling task should advance start_at after claim"
    );

    let scheduled_message_count = session_store().get_messages(&scheduled.id).len();
    let polling_message_count = session_store().get_messages(&polling.id).len();
    run_due_task_scheduler_tick_for_business_test();
    assert_eq!(
        session_store().get_messages(&scheduled.id).len(),
        scheduled_message_count,
        "scheduled task should not be claimed twice while doing"
    );
    assert_eq!(
        session_store().get_messages(&polling.id).len(),
        polling_message_count,
        "polling task should not be claimed again before next start_at"
    );
}

#[tokio::test]
async fn gateway_task_scheduler_business_flow_claims_session_idle_question_once() {
    let root = tempfile::tempdir().expect("temp scheduler idle root");
    let idle = session_store().create_session(
        Some(
            root.path()
                .join("idle-question")
                .to_string_lossy()
                .to_string(),
        ),
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
    let done = session_store().create_session(
        Some(root.path().join("done").to_string_lossy().to_string()),
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
    let busy = session_store().create_session(
        Some(root.path().join("busy").to_string_lossy().to_string()),
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

    let _ = update_session_task_management(
        Path(idle.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "plan_summary": "Session idle business plan",
                "tasks": [
                    {
                        "task_id": "idle-waiting-user",
                        "task_summary": "Wait for the human before continuing",
                        "status": "waiting_user",
                        "start_condition": "session_idle"
                    },
                    {
                        "task_id": "idle-question",
                        "task_summary": "Continue the idle question task",
                        "status": "question",
                        "start_condition": "session_idle"
                    }
                ]
            }),
        }),
    )
    .await;
    let _ = update_session_task_management(
        Path(done.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "task_id": "idle-done",
                "task_summary": "Completed idle work must stay quiet",
                "status": "done",
                "start_condition": "session_idle"
            }),
        }),
    )
    .await;
    let _ = update_session_task_management(
        Path(busy.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "task_id": "busy-idle",
                "task_summary": "Busy sessions must not be claimed",
                "status": "todo",
                "start_condition": "session_idle"
            }),
        }),
    )
    .await;
    session_store().update_session_status(&busy.id, StoreSessionStatus::Busy);

    run_due_task_scheduler_tick_for_business_test();

    assert_scheduler_triggered(
        &idle.id,
        "session_idle",
        "session became idle",
        "Continue the idle question task",
    );
    let idle_session = session_store()
        .get_session(&idle.id)
        .expect("idle session after scheduler tick");
    let idle_tasks = idle_session
        .task_management
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .expect("idle tasks");
    let waiting = task_by_id(idle_tasks, "idle-waiting-user");
    assert_eq!(waiting["status"], "waiting_user");
    let question = task_by_id(idle_tasks, "idle-question");
    assert_eq!(question["status"], "doing");

    assert_no_scheduler_side_effects(&done.id, ApiSessionStatus::Idle, "done");
    assert_no_scheduler_side_effects(&busy.id, ApiSessionStatus::Busy, "busy");

    let idle_message_count = session_store().get_messages(&idle.id).len();
    run_due_task_scheduler_tick_for_business_test();
    assert_eq!(
        session_store().get_messages(&idle.id).len(),
        idle_message_count,
        "session-idle question task should not be claimed twice after it is doing"
    );
}

#[tokio::test]
async fn gateway_task_scheduler_business_flow_survives_edits_between_ticks() {
    let root = tempfile::tempdir().expect("temp scheduler edit root");
    let now = Utc::now();
    let first_due = now - chrono::Duration::minutes(10);
    let second_due = now - chrono::Duration::minutes(5);
    let session = session_store().create_session(
        Some(
            root.path()
                .join("scheduler-edit")
                .to_string_lossy()
                .to_string(),
        ),
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

    let _ = update_session_task_management(
        Path(session.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "plan_summary": "Scheduler edit business plan",
                "tasks": [
                    {
                        "task_id": "first-due",
                        "task_summary": "Run the first due task",
                        "status": "todo",
                        "start_at": first_due.to_rfc3339()
                    },
                    {
                        "task_id": "manual-follow-up",
                        "task_summary": "Manual follow up must not be scheduled",
                        "status": "todo",
                        "start_condition": "user_action"
                    }
                ]
            }),
        }),
    )
    .await;

    run_due_task_scheduler_tick_for_business_test();
    assert_scheduler_triggered(
        &session.id,
        "scheduled_task",
        "scheduled start time arrived",
        "Run the first due task",
    );
    let first_message_count = session_store().get_messages(&session.id).len();

    let _ = update_session_task_management(
        Path(session.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "tasks": [
                    {
                        "task_id": "manual-follow-up",
                        "task_summary": "Manual follow up stays manual after edit",
                        "status": "todo",
                        "start_condition": "user_action"
                    },
                    {
                        "task_id": "first-due",
                        "task_summary": "Run the first due task",
                        "status": "doing",
                        "start_at": first_due.to_rfc3339()
                    },
                    {
                        "task_id": "second-due",
                        "task_summary": "Run the second due task after idle",
                        "status": "todo",
                        "start_at": second_due.to_rfc3339()
                    }
                ]
            }),
        }),
    )
    .await;

    run_due_task_scheduler_tick_for_business_test();
    assert_eq!(
        session_store().get_messages(&session.id).len(),
        first_message_count,
        "busy session should not be reclaimed while task management is edited"
    );
    let edited = session_store()
        .get_session(&session.id)
        .expect("edited scheduler session");
    assert_eq!(edited.status, ApiSessionStatus::Busy);
    let edited_tasks = edited
        .task_management
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .expect("edited tasks");
    assert_eq!(edited_tasks[0]["task_id"], "manual-follow-up");
    assert_eq!(edited_tasks[0]["step"], 1);
    assert_eq!(
        task_by_id(edited_tasks, "manual-follow-up")["start_condition"],
        "user_action"
    );
    assert_eq!(task_by_id(edited_tasks, "first-due")["status"], "doing");
    assert!(
        task_by_id(edited_tasks, "second-due")
            .get("status")
            .is_none(),
        "todo tasks serialize without an explicit status field"
    );

    let _ = update_session_task_management(
        Path(session.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "task_id": "first-due",
                "status": "done"
            }),
        }),
    )
    .await;
    session_store().update_session_status(&session.id, StoreSessionStatus::Idle);

    run_due_task_scheduler_tick_for_business_test();
    assert_eq!(
        session_store().get_messages(&session.id).len(),
        first_message_count + 1,
        "second due task should be claimed exactly once after the session returns idle"
    );
    let after_second = session_store()
        .get_session(&session.id)
        .expect("session after second scheduler tick");
    let after_tasks = after_second
        .task_management
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .expect("tasks after second tick");
    assert!(
        task_by_id(after_tasks, "manual-follow-up")
            .get("status")
            .is_none(),
        "manual todo task should stay unclaimed"
    );
    assert_eq!(task_by_id(after_tasks, "first-due")["status"], "done");
    assert_eq!(task_by_id(after_tasks, "second-due")["status"], "doing");
    assert_eq!(
        session_store().get_todos(&session.id)[0]["content"],
        "Run the second due task after idle"
    );

    let final_message_count = session_store().get_messages(&session.id).len();
    run_due_task_scheduler_tick_for_business_test();
    assert_eq!(
        session_store().get_messages(&session.id).len(),
        final_message_count,
        "doing second task must not be claimed again"
    );
}

#[tokio::test]
async fn gateway_task_scheduler_business_flow_repeats_polling_cycles_without_duplicate_claims() {
    let root = tempfile::tempdir().expect("temp scheduler polling cycle root");
    let session = session_store().create_session(
        Some(
            root.path()
                .join("scheduler-polling-cycle")
                .to_string_lossy()
                .to_string(),
        ),
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

    let summaries = [
        "Poll local workspace cycle 1",
        "Poll local workspace cycle 2",
        "Poll local workspace cycle 3",
    ];

    for (index, summary) in summaries.iter().enumerate() {
        let due = Utc::now() - chrono::Duration::seconds(10);
        session_store().update_session_status(&session.id, StoreSessionStatus::Idle);
        set_polling_task_due(&session.id, summary, due).await;

        run_due_task_scheduler_tick_for_business_test();

        let expected_messages = index + 1;
        let messages = session_store().get_messages(&session.id);
        assert_eq!(
            messages.len(),
            expected_messages,
            "polling cycle {expected_messages} should create one scheduler message"
        );
        let latest_message = messages
            .last()
            .unwrap_or_else(|| panic!("polling cycle {expected_messages} should create a message"));
        assert_eq!(latest_message.role, MessageRole::User);
        assert_eq!(
            latest_message
                .parts
                .first()
                .and_then(|part| part.metadata.as_ref())
                .and_then(|metadata| metadata.get("start_condition")),
            Some(&json!("polling_task"))
        );
        let latest_text = latest_message
            .parts
            .iter()
            .find_map(|part| part.text.as_deref().or(part.content.as_deref()))
            .unwrap_or_else(|| {
                panic!("polling cycle {expected_messages} message should have text")
            });
        assert!(
            latest_text.contains(summary),
            "polling cycle prompt should include the current task summary: {latest_text}"
        );

        let session_after_tick = session_store()
            .get_session(&session.id)
            .expect("polling cycle session after tick");
        assert_eq!(session_after_tick.status, ApiSessionStatus::Busy);
        assert_eq!(session_after_tick.task_management["status"], "doing");
        assert_eq!(session_after_tick.task_management["task_summary"], *summary);
        let next_start = DateTime::parse_from_rfc3339(
            session_after_tick.task_management["start_at"]
                .as_str()
                .expect("polling cycle start_at"),
        )
        .expect("parse polling cycle start_at")
        .with_timezone(&Utc);
        assert!(
            next_start > due,
            "polling cycle should advance start_at after claim"
        );
        let todos = session_store().get_todos(&session.id);
        assert_eq!(
            todos.len(),
            1,
            "polling cycles keep one active runtime todo"
        );
        assert_eq!(todos[0]["status"], "in_progress");
        assert_eq!(todos[0]["content"], *summary);

        run_due_task_scheduler_tick_for_business_test();
        assert_eq!(
            session_store().get_messages(&session.id).len(),
            expected_messages,
            "polling cycle {expected_messages} must not duplicate while busy"
        );
    }
}

#[tokio::test]
async fn gateway_task_management_business_flow_reorders_and_rejects_invalid_multi_task_patches() {
    let root = tempfile::tempdir().expect("temp task patch root");
    let session = session_store().create_session(
        Some(
            root.path()
                .join("task-management-patch")
                .to_string_lossy()
                .to_string(),
        ),
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

    let Json(initial) = update_session_task_management(
        Path(session.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "plan_summary": "Patch ordering business plan",
                "tasks": [
                    {
                        "task_id": "alpha",
                        "step": 10,
                        "task_summary": "Alpha manual task",
                        "status": "todo",
                        "start_condition": "user_action"
                    },
                    {
                        "task_id": "beta",
                        "step": 3,
                        "task_summary": "Beta polling task",
                        "status": "question",
                        "poll_interval": { "d": 0, "h": 0, "m": 1, "s": 30 }
                    },
                    {
                        "task_id": "gamma",
                        "step": 1,
                        "task_summary": "Gamma scheduled task",
                        "status": "todo",
                        "start_at": (Utc::now() + chrono::Duration::minutes(30)).to_rfc3339()
                    }
                ]
            }),
        }),
    )
    .await;
    let initial_tasks = task_array(&initial.task_management);
    assert_eq!(initial_tasks[0]["task_id"], "alpha");
    assert_eq!(initial_tasks[0]["step"], 1);
    assert_eq!(initial_tasks[1]["task_id"], "beta");
    assert_eq!(initial_tasks[1]["step"], 2);
    assert_eq!(initial_tasks[1]["start_condition"], "polling_task");
    assert_eq!(initial_tasks[1]["status"], "question");
    assert_eq!(initial_tasks[2]["task_id"], "gamma");
    assert_eq!(initial_tasks[2]["step"], 3);
    assert_eq!(initial_tasks[2]["start_condition"], "scheduled_task");

    let Json(reordered) = update_session_task_management(
        Path(session.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "tasks": [
                    {
                        "task_id": "gamma",
                        "task_summary": "Gamma should run first",
                        "status": "question"
                    },
                    {
                        "task_id": "alpha",
                        "task_summary": "Alpha moves second",
                        "status": "done"
                    },
                    {
                        "task_id": "beta",
                        "poll_interval": { "d": 0, "h": 0, "m": 0, "s": 0 },
                        "start_condition": "user_action"
                    }
                ]
            }),
        }),
    )
    .await;
    let reordered_tasks = task_array(&reordered.task_management);
    assert_eq!(reordered_tasks[0]["task_id"], "gamma");
    assert_eq!(reordered_tasks[0]["step"], 1);
    assert_eq!(reordered_tasks[0]["status"], "question");
    assert_eq!(reordered_tasks[0]["task_summary"], "Gamma should run first");
    assert_eq!(reordered_tasks[1]["task_id"], "alpha");
    assert_eq!(reordered_tasks[1]["step"], 2);
    assert_eq!(reordered_tasks[1]["status"], "done");
    assert_eq!(reordered_tasks[2]["task_id"], "beta");
    assert_eq!(reordered_tasks[2]["step"], 3);
    assert_eq!(reordered_tasks[2]["start_condition"], "user_action");

    let before_invalid = reordered.task_management.clone();
    let Json(after_invalid) = update_session_task_management(
        Path(session.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "status": "done",
                "task_summary": "Ambiguous patch must not overwrite multi-task state"
            }),
        }),
    )
    .await;
    assert_eq!(
        after_invalid.task_management, before_invalid,
        "multi-task sessions require task_id for single-task patches"
    );

    let Json(after_bad_status) = update_session_task_management(
        Path(session.id.clone()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "task_id": "gamma",
                "status": "not_a_real_status"
            }),
        }),
    )
    .await;
    assert_eq!(
        after_bad_status.task_management, before_invalid,
        "invalid status patches should leave the previous task plan intact"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gateway_task_scheduler_business_flow_concurrent_edits_race_scheduler_tick_once() {
    let root = tempfile::tempdir().expect("temp scheduler edit race root");
    let session = session_store().create_session(
        Some(
            root.path()
                .join("scheduler-edit-race")
                .to_string_lossy()
                .to_string(),
        ),
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
    let due = Utc::now() - chrono::Duration::minutes(1);
    let future = Utc::now() + chrono::Duration::hours(1);
    let worker_count = 12;
    let barrier = Arc::new(Barrier::new(worker_count + 1));
    let mut workers = Vec::new();

    for index in 0..8 {
        let session_id = session.id.clone();
        let barrier = Arc::clone(&barrier);
        let due = due.to_rfc3339();
        let future = future.to_rfc3339();
        workers.push(tokio::spawn(async move {
            barrier.wait().await;
            let _ = update_session_task_management(
                Path(session_id),
                Json(UpdateSessionTaskManagementRequest {
                    task_management: json!({
                        "plan_summary": format!("Scheduler edit race plan {index}"),
                        "tasks": [
                            {
                                "task_id": "race-due",
                                "task_summary": "Claim the shared due task once",
                                "status": "todo",
                                "start_at": due
                            },
                            {
                                "task_id": format!("manual-edit-{index}"),
                                "task_summary": format!("Manual edit {index} must stay manual"),
                                "status": "todo",
                                "start_condition": "user_action"
                            },
                            {
                                "task_id": format!("future-edit-{index}"),
                                "task_summary": format!("Future edit {index} must not run"),
                                "status": "todo",
                                "start_at": future
                            }
                        ]
                    }),
                }),
            )
            .await;
        }));
    }

    for _ in 0..4 {
        let barrier = Arc::clone(&barrier);
        workers.push(tokio::spawn(async move {
            barrier.wait().await;
            run_due_task_scheduler_tick_for_business_test();
        }));
    }

    barrier.wait().await;
    for worker in workers {
        worker
            .await
            .expect("scheduler race worker should not panic");
    }

    let messages = session_store().get_messages(&session.id);
    assert_eq!(
        messages.len(),
        1,
        "concurrent scheduler ticks and task edits must claim the due task once"
    );
    let message = &messages[0];
    assert_eq!(message.role, MessageRole::User);
    assert_eq!(
        message
            .parts
            .first()
            .and_then(|part| part.metadata.as_ref())
            .and_then(|metadata| metadata.get("start_condition")),
        Some(&json!("scheduled_task"))
    );
    let message_text = message
        .parts
        .iter()
        .find_map(|part| part.text.as_deref().or(part.content.as_deref()))
        .expect("scheduler race message should have text");
    assert!(
        message_text.contains("Claim the shared due task once"),
        "scheduler race prompt should include the due task summary: {message_text}"
    );

    let session_after_race = session_store()
        .get_session(&session.id)
        .expect("scheduler race session after workers");
    assert_eq!(session_after_race.status, ApiSessionStatus::Busy);
    let tasks = session_after_race
        .task_management
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .expect("scheduler race task plan should remain a task array");
    let due_task = task_by_id(tasks, "race-due");
    assert_eq!(due_task["status"], "doing");
    let manual_tasks = tasks
        .iter()
        .filter(|task| {
            task.get("start_condition") == Some(&json!("user_action"))
                && task
                    .get("task_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| id.starts_with("manual-edit-"))
        })
        .count();
    assert!(
        manual_tasks >= 1,
        "concurrent edits should keep at least one manual task"
    );
    assert!(
        tasks
            .iter()
            .filter(|task| {
                task.get("start_condition") == Some(&json!("user_action"))
                    && task
                        .get("task_id")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|id| id.starts_with("manual-edit-"))
            })
            .all(|task| task.get("status").is_none()),
        "manual user_action edits must remain unclaimed"
    );
    let future_tasks = tasks
        .iter()
        .filter(|task| {
            task.get("task_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| id.starts_with("future-edit-"))
        })
        .count();
    assert!(
        future_tasks >= 1,
        "concurrent edits should keep at least one future task"
    );
    assert_eq!(session_store().get_todos(&session.id).len(), 1);

    run_due_task_scheduler_tick_for_business_test();
    assert_eq!(
        session_store().get_messages(&session.id).len(),
        1,
        "busy session must stay idempotent after the edit race"
    );
}

#[test]
fn gateway_task_scheduler_business_flow_hydrates_persisted_due_task_after_store_restart(
) -> anyhow::Result<()> {
    let _guard = SCHEDULER_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().expect("temp scheduler recovery root");
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = SchedulerEnvGuard::new(&home);
    let service = SchedulerServiceThread::start()?;
    let writer = SessionStore::new();
    let due = Utc::now() - chrono::Duration::minutes(2);
    let session = writer.create_session(
        Some(workspace.to_string_lossy().to_string()),
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
    writer.update_session(
        &session.id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(json!({
            "task_id": "persisted-due",
            "task_summary": "Resume persisted scheduler task after restart",
            "status": "todo",
            "start_at": due.to_rfc3339()
        })),
    );
    upsert_runtime_owned_scheduler_snapshot(&writer, &session.id, &workspace)?;

    let recovered = SessionStore::new();
    recovered.hydrate_directory(Some(workspace.to_string_lossy().to_string()));
    let hydrated = recovered
        .get_session(&session.id)
        .expect("recovered store should hydrate persisted session");
    assert_eq!(hydrated.status, ApiSessionStatus::Idle);
    assert_eq!(hydrated.task_management["task_id"], "persisted-due");
    assert_eq!(
        hydrated.task_management["task_summary"],
        "Resume persisted scheduler task after restart"
    );

    run_due_task_scheduler_tick_for_store_business_test(&recovered);
    assert_scheduler_triggered_in_store(
        &recovered,
        &session.id,
        "scheduled_task",
        "scheduled start time arrived",
        "Resume persisted scheduler task after restart",
    );

    let recovered_message_count = recovered.get_messages(&session.id).len();
    run_due_task_scheduler_tick_for_store_business_test(&recovered);
    assert_eq!(
        recovered.get_messages(&session.id).len(),
        recovered_message_count,
        "hydrated due task must not be claimed twice after restart"
    );

    service.shutdown(Duration::from_secs(5))?;
    wait_for_scheduler_condition(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}
