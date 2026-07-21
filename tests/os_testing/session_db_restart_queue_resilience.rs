//! Required root-package business E2E for session_db restart and file-queue
//! resilience.
//!
//! The flow is intentionally local-only: it uses the real session_db socket
//! service, the durable file queue, the embedded SQLite index DB, and workspace
//! `.tura/session_log.sqlite3` stores without public network access, API keys,
//! or third-party services.

use anyhow::{anyhow, bail, Context, Result};
use session_log_contract::{
    DeleteSessionRequest, GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest,
    SessionLogCommand, SessionLogResponse,
};
use std::{
    sync::{Arc, Barrier},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[path = "helpers/session_db_restart_queue_resilience.rs"]
mod helpers;
use helpers::*;
#[test]
fn session_db_restarts_drain_offline_queue_quarantine_bad_items_and_keep_checkpoint_idempotency(
) -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let env = TestEnv::new("session-db-restart-queue")?;
    let workspace = env.workspace("primary")?;
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let keep_id = env.session_id("keep");
    let delete_id = env.session_id("delete");
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;

    for command in create_session_commands(
        &keep_id,
        &workspace_key,
        timestamp,
        SessionState::Running,
        &["keep-m1", "keep-m2"],
        &[("keep-todo-1", PlanStatus::Doing)],
    ) {
        enqueue(command)?;
    }
    for command in create_session_commands(
        &delete_id,
        &workspace_key,
        timestamp + 10,
        SessionState::Created,
        &["delete-m1"],
        &[],
    ) {
        enqueue(command)?;
    }
    let first_checkpoint = checkpoint(&keep_id, 1, CheckpointType::CommandFinished);
    enqueue(SessionLogCommand::ApplyCommandCheckpoint(Box::new(
        first_checkpoint.clone(),
    )))?;
    enqueue(SessionLogCommand::ApplyCommandCheckpoint(Box::new(
        first_checkpoint,
    )))?;
    let corrupt_path = write_corrupt_pending_queue_item(&env.home, "bad-json-before-start")?;

    let mut service = SessionDbService::start()?;
    wait_until(Duration::from_secs(10), || {
        session_visible(&keep_id)
            && session_visible(&delete_id)
            && failed_queue_items(&env.home) >= 1
            && pending_queue_items(&env.home) == 0
    })
    .context("initial offline session queue did not drain")?;
    assert!(
        !corrupt_path.exists(),
        "corrupt pending file should be moved out of pending after the drain loop sees it"
    );
    assert_failed_queue_contains_error(&env.home, "bad-json-before-start")?;
    assert_pending_queue_empty(&env.home)?;
    assert_session_snapshot(
        &keep_id,
        &workspace_key,
        "running",
        "busy",
        2,
        Some("doing"),
    )?;
    assert_session_snapshot(&delete_id, &workspace_key, "created", "idle", 1, None)?;
    assert_records(&keep_id, &["keep-m1", "keep-m2"])?;
    assert_checkpoint_rows(&env.home, &keep_id, 1)?;
    assert_workspace_db_exists(&workspace)?;
    service.shutdown()?;

    enqueue(SessionLogCommand::DeleteSession(DeleteSessionRequest {
        session_id: delete_id.clone(),
    }))?;
    enqueue(execute_session_command(
        &keep_id,
        SessionCommand::SubmitUserInput,
    ))?;
    enqueue(execute_session_command(
        &keep_id,
        SessionCommand::RuntimeStarted {
            runtime_id: "runtime-keep-restart".to_string(),
        },
    ))?;
    enqueue(execute_session_command(
        &keep_id,
        SessionCommand::ApplyTaskStatus {
            task_plan: task_plan(&[
                ("keep-todo-1", PlanStatus::Done),
                ("keep-todo-2", PlanStatus::Done),
            ]),
        },
    ))?;
    enqueue(execute_session_command(
        &keep_id,
        SessionCommand::RuntimeCompleted {
            runtime_id: "runtime-keep-restart".to_string(),
        },
    ))?;
    enqueue(persist_session_delta_command(
        &keep_id,
        &workspace_key,
        timestamp + 40,
        1,
        2,
        &["keep-m3"],
        &[
            ("keep-todo-1", PlanStatus::Done),
            ("keep-todo-2", PlanStatus::Done),
        ],
    ))?;
    enqueue(SessionLogCommand::ApplyCommandCheckpoint(Box::new(
        checkpoint(&keep_id, 2, CheckpointType::TurnFinished),
    )))?;

    let mut restarted = SessionDbService::start()?;
    wait_until(Duration::from_secs(10), || {
        session_missing(&delete_id)
            && session_message_count(&keep_id).is_some_and(|count| count == 3)
            && checkpoint_row_count(&env.home, &keep_id) == 2
    })
    .context("restarted offline session queue did not drain")?;
    assert_session_snapshot(
        &keep_id,
        &workspace_key,
        "completed",
        "idle",
        3,
        Some("done"),
    )?;
    assert_session_missing(&delete_id)?;
    assert_records(&keep_id, &["keep-m1", "keep-m2", "keep-m3"])?;
    assert_checkpoint_rows(&env.home, &keep_id, 2)?;
    assert_pending_queue_empty(&env.home)?;
    restarted.shutdown()?;

    Ok(())
}

#[test]
fn session_db_restart_marks_running_and_paused_sessions_interrupted_without_losing_history(
) -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let env = TestEnv::new("session-db-restart-interrupted")?;
    let workspace = env.workspace("recovery")?;
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let running_id = env.session_id("running");
    let paused_id = env.session_id("paused");
    let completed_id = env.session_id("completed");

    let mut service = SessionDbService::start()?;
    for command in create_session_commands(
        &running_id,
        &workspace_key,
        100,
        SessionState::Running,
        &["running-m1", "running-m2"],
        &[("running-todo", PlanStatus::Doing)],
    ) {
        assert_ok(session_log_contract::client::call_service(&command)?)?;
    }
    for command in create_session_commands(
        &paused_id,
        &workspace_key,
        110,
        SessionState::Paused,
        &["paused-m1"],
        &[("paused-todo", PlanStatus::WaitingUser)],
    ) {
        assert_ok(session_log_contract::client::call_service(&command)?)?;
    }
    for command in create_session_commands(
        &completed_id,
        &workspace_key,
        120,
        SessionState::Completed,
        &["completed-m1"],
        &[("completed-todo", PlanStatus::Done)],
    ) {
        assert_ok(session_log_contract::client::call_service(&command)?)?;
    }
    service.shutdown()?;

    let mut restarted = SessionDbService::start()?;
    wait_until(Duration::from_secs(10), || {
        session_state_status(&running_id) == Some(("interrupted".to_string(), "error".to_string()))
            && session_state_status(&paused_id)
                == Some(("interrupted".to_string(), "error".to_string()))
    })?;

    assert_session_snapshot(
        &running_id,
        &workspace_key,
        "interrupted",
        "error",
        2,
        Some("waiting_user"),
    )?;
    assert_session_snapshot(
        &paused_id,
        &workspace_key,
        "interrupted",
        "error",
        1,
        Some("waiting_user"),
    )?;
    assert_session_snapshot(
        &completed_id,
        &workspace_key,
        "completed",
        "idle",
        1,
        Some("done"),
    )?;
    assert_records(&running_id, &["running-m1", "running-m2"])?;
    assert_records(&paused_id, &["paused-m1"])?;
    assert_records(&completed_id, &["completed-m1"])?;
    assert_index_state_matches_workspace_state(
        &env.home,
        &[&running_id, &paused_id, &completed_id],
    )?;
    restarted.shutdown()?;

    Ok(())
}

#[test]
fn session_db_handles_concurrent_short_lived_clients_after_restart_with_workspace_pagination(
) -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let env = TestEnv::new("session-db-concurrent-after-restart")?;
    let workspace_a = env.workspace("workspace-a")?;
    let workspace_b = env.workspace("workspace-b")?;
    let workspace_a_key = session_log::path::normalize_workspace(&workspace_a.to_string_lossy());
    let workspace_b_key = session_log::path::normalize_workspace(&workspace_b.to_string_lossy());

    let mut service = SessionDbService::start()?;
    service.shutdown()?;

    let mut restarted = SessionDbService::start()?;
    let barrier = Arc::new(Barrier::new(12));
    let mut handles = Vec::new();
    for index in 0..12 {
        let barrier = Arc::clone(&barrier);
        let workspace = if index % 3 == 0 {
            workspace_b_key.clone()
        } else {
            workspace_a_key.clone()
        };
        let session_id = env.session_id(&format!("concurrent-{index}"));
        handles.push(thread::spawn(move || -> Result<(String, String)> {
            barrier.wait();
            let messages = [
                format!("m-{index}-0"),
                format!("m-{index}-1"),
                format!("m-{index}-2"),
            ];
            let message_refs = messages.iter().map(String::as_str).collect::<Vec<_>>();
            for command in create_session_commands(
                &session_id,
                &workspace,
                1_000 + index as i64,
                SessionState::Completed,
                &message_refs,
                &[("batch-task", PlanStatus::Done)],
            ) {
                assert_ok(session_log_contract::client::call_service(&command)?)?;
            }
            Ok((session_id, workspace))
        }));
    }

    let mut workspace_a_sessions = Vec::new();
    let mut workspace_b_sessions = Vec::new();
    for handle in handles {
        let (session_id, workspace) = handle
            .join()
            .map_err(|_| anyhow!("concurrent session writer thread panicked"))??;
        if workspace == workspace_a_key {
            workspace_a_sessions.push(session_id);
        } else {
            workspace_b_sessions.push(session_id);
        }
    }
    workspace_a_sessions.sort();
    workspace_b_sessions.sort();

    assert_eq!(workspace_a_sessions.len(), 8);
    assert_eq!(workspace_b_sessions.len(), 4);
    assert_workspace_page(&workspace_a_key, 0, 3, 8, 3)?;
    assert_workspace_page(&workspace_a_key, 1, 3, 8, 3)?;
    assert_workspace_page(&workspace_a_key, 2, 3, 8, 2)?;
    assert_workspace_page(&workspace_b_key, 0, 10, 4, 4)?;
    assert_workspace_page(&workspace_b_key, 99, 2, 4, 2)?;

    for session_id in workspace_a_sessions
        .iter()
        .chain(workspace_b_sessions.iter())
    {
        let records = record_ids(session_id)?;
        assert_eq!(records.len(), 3);
        assert!(
            records.iter().all(|record| record.starts_with("m-")),
            "concurrent records should keep their message ids: {records:?}"
        );
    }
    assert_workspace_summaries(&[(workspace_a_key, 8), (workspace_b_key, 4)])?;
    restarted.shutdown()?;

    Ok(())
}

#[test]
fn session_db_drains_file_queue_under_concurrent_socket_reads_and_writes() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let env = TestEnv::new("session-db-queue-concurrent-rw")?;
    let workspace_a = env.workspace("queue-a")?;
    let workspace_b = env.workspace("queue-b")?;
    let workspace_a_key = session_log::path::normalize_workspace(&workspace_a.to_string_lossy());
    let workspace_b_key = session_log::path::normalize_workspace(&workspace_b.to_string_lossy());
    let mut expected_sessions = Vec::new();

    for index in 0..72 {
        let workspace = if index % 2 == 0 {
            workspace_a_key.clone()
        } else {
            workspace_b_key.clone()
        };
        let session_id = env.session_id(&format!("queued-before-start-{index}"));
        let messages = [
            format!("queued-before-start-{index}-0"),
            format!("queued-before-start-{index}-1"),
        ];
        let message_refs = messages.iter().map(String::as_str).collect::<Vec<_>>();
        for command in create_session_payload_commands(
            &session_id,
            &workspace,
            2_000 + index as i64,
            &message_refs,
            &[("queued-task", PlanStatus::Done)],
        ) {
            enqueue(command)?;
        }
        expected_sessions.push((session_id, workspace, messages.len()));
    }

    let mut service = SessionDbService::start()?;
    let barrier = Arc::new(Barrier::new(24));
    let sample_session_id = expected_sessions
        .first()
        .map(|(session_id, _, _)| session_id.clone())
        .ok_or_else(|| anyhow!("seeded queue sessions missing"))?;
    let mut handles = Vec::new();

    for writer in 0..12 {
        let barrier = Arc::clone(&barrier);
        let workspace = if writer % 2 == 0 {
            workspace_a_key.clone()
        } else {
            workspace_b_key.clone()
        };
        handles.push(thread::spawn(
            move || -> Result<Vec<(String, String, usize)>> {
                barrier.wait();
                let session_id = format!("socket-writer-{writer}-{}", std::process::id());
                let messages = [
                    format!("socket-writer-{writer}-0"),
                    format!("socket-writer-{writer}-1"),
                    format!("socket-writer-{writer}-2"),
                ];
                let message_refs = messages.iter().map(String::as_str).collect::<Vec<_>>();
                for command in create_session_payload_commands(
                    &session_id,
                    &workspace,
                    3_000 + writer as i64,
                    &message_refs,
                    &[("socket-task", PlanStatus::Done)],
                ) {
                    assert_ok(session_log_contract::client::call_service(&command)?)?;
                }
                let _ = record_ids(&session_id)?;
                Ok(vec![(session_id, workspace, messages.len())])
            },
        ));
    }

    for queue_writer in 0..6 {
        let barrier = Arc::clone(&barrier);
        let workspace_a = workspace_a_key.clone();
        let workspace_b = workspace_b_key.clone();
        handles.push(thread::spawn(
            move || -> Result<Vec<(String, String, usize)>> {
                barrier.wait();
                let mut sessions = Vec::new();
                for item in 0..6 {
                    let workspace = if (queue_writer + item) % 2 == 0 {
                        workspace_a.clone()
                    } else {
                        workspace_b.clone()
                    };
                    let session_id = format!(
                        "queued-while-running-{queue_writer}-{item}-{}",
                        std::process::id()
                    );
                    let messages = [
                        format!("queued-while-running-{queue_writer}-{item}-0"),
                        format!("queued-while-running-{queue_writer}-{item}-1"),
                    ];
                    let message_refs = messages.iter().map(String::as_str).collect::<Vec<_>>();
                    for command in create_session_payload_commands(
                        &session_id,
                        &workspace,
                        4_000 + queue_writer as i64 * 10 + item as i64,
                        &message_refs,
                        &[("live-queue-task", PlanStatus::Done)],
                    ) {
                        enqueue(command)?;
                    }
                    sessions.push((session_id, workspace, messages.len()));
                }
                Ok(sessions)
            },
        ));
    }

    for _reader in 0..6 {
        let barrier = Arc::clone(&barrier);
        let workspace_a = workspace_a_key.clone();
        let workspace_b = workspace_b_key.clone();
        let sample_session_id = sample_session_id.clone();
        handles.push(thread::spawn(
            move || -> Result<Vec<(String, String, usize)>> {
                barrier.wait();
                for round in 0..20 {
                    match session_log_contract::client::call_service(
                        &SessionLogCommand::ListWorkspaces,
                    )? {
                        SessionLogResponse::Workspaces { .. } => {}
                        other => bail!(
                            "unexpected workspace response during concurrent reads: {other:?}"
                        ),
                    }
                    for workspace in [&workspace_a, &workspace_b] {
                        match session_log_contract::client::call_service(
                            &SessionLogCommand::ListSessions(ListSessionsRequest {
                                workspace: workspace.clone(),
                                page: round % 4,
                                page_size: 7,
                            }),
                        )? {
                            SessionLogResponse::Sessions { .. } => {}
                            other => bail!(
                                "unexpected sessions response during concurrent reads: {other:?}"
                            ),
                        }
                    }
                    let _ = session_log_contract::client::call_service(
                        &SessionLogCommand::GetSession(GetSessionRequest {
                            session_id: sample_session_id.clone(),
                        }),
                    )?;
                    match session_log_contract::client::call_service(
                        &SessionLogCommand::ListSessionRecords(ListSessionRecordsRequest {
                            session_id: sample_session_id.clone(),
                            page: 0,
                            page_size: 10,
                        }),
                    )? {
                        SessionLogResponse::Records { .. } => {}
                        other => {
                            bail!("unexpected records response during concurrent reads: {other:?}")
                        }
                    }
                }
                Ok(Vec::new())
            },
        ));
    }

    for handle in handles {
        expected_sessions.extend(
            handle
                .join()
                .map_err(|_| anyhow!("concurrent session_db worker thread panicked"))??,
        );
    }

    let expected_a = expected_sessions
        .iter()
        .filter(|(_, workspace, _)| workspace == &workspace_a_key)
        .count() as u64;
    let expected_b = expected_sessions
        .iter()
        .filter(|(_, workspace, _)| workspace == &workspace_b_key)
        .count() as u64;

    wait_until(Duration::from_secs(60), || {
        pending_queue_items(&env.home) == 0
            && workspace_session_total(&workspace_a_key) == Some(expected_a)
            && workspace_session_total(&workspace_b_key) == Some(expected_b)
    })
    .with_context(|| {
        format!(
            "concurrent queue drain did not converge: pending={}, workspace_a={:?}/{expected_a}, workspace_b={:?}/{expected_b}",
            pending_queue_items(&env.home),
            workspace_session_total(&workspace_a_key),
            workspace_session_total(&workspace_b_key),
        )
    })?;
    assert_pending_queue_empty(&env.home)?;
    assert_workspace_page(&workspace_a_key, 0, 500, expected_a, expected_a as usize)?;
    assert_workspace_page(&workspace_b_key, 0, 500, expected_b, expected_b as usize)?;
    assert_workspace_summaries(&[(workspace_a_key, expected_a), (workspace_b_key, expected_b)])?;
    for (session_id, _workspace, expected_records) in &expected_sessions {
        assert!(
            session_visible(session_id),
            "session {session_id} should be visible after queue drain"
        );
        let records = record_ids(session_id)?;
        assert_eq!(
            records.len(),
            *expected_records,
            "session {session_id} should keep all records after concurrent writes"
        );
    }
    service.shutdown()?;
    Ok(())
}
