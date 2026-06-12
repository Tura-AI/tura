//! Required root-package business E2E for session_db restart and file-queue
//! resilience.
//!
//! The flow is intentionally local-only: it uses the real session_db socket
//! service, the durable file queue, the embedded SQLite index DB, and workspace
//! `.tura/session_log.sqlite3` stores without public network access, API keys,
//! or third-party services.

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::Connection;
use serde_json::json;
use session_log::{
    file_queue, CommandCheckpoint, DeleteSessionRequest, GetSessionRequest,
    ListSessionRecordsRequest, ListSessionsRequest, SessionLogCommand, SessionLogResponse,
    UpsertSessionRequest,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Barrier, Mutex},
    thread,
    time::{Duration, Instant},
};

static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn session_db_restarts_drain_offline_queue_quarantine_bad_items_and_keep_checkpoint_idempotency(
) -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let env = TestEnv::new("session-db-restart-queue")?;
    let workspace = env.workspace("primary")?;
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let keep_id = env.session_id("keep");
    let delete_id = env.session_id("delete");

    enqueue(SessionLogCommand::UpsertSession(upsert_session(
        &keep_id,
        &workspace_key,
        10,
        "running",
        &["keep-m1", "keep-m2"],
        &[("keep-todo-1", "doing")],
    )))?;
    enqueue(SessionLogCommand::UpsertSession(upsert_session(
        &delete_id,
        &workspace_key,
        20,
        "created",
        &["delete-m1"],
        &[],
    )))?;
    let first_checkpoint = checkpoint(&keep_id, 1, "command_finished");
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
    })?;
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
    enqueue(SessionLogCommand::UpsertSession(upsert_session(
        &keep_id,
        &workspace_key,
        50,
        "completed",
        &["keep-m1", "keep-m2", "keep-m3"],
        &[("keep-todo-1", "done"), ("keep-todo-2", "done")],
    )))?;
    enqueue(SessionLogCommand::ApplyCommandCheckpoint(Box::new(
        checkpoint(&keep_id, 2, "turn_finished"),
    )))?;

    let mut restarted = SessionDbService::start()?;
    wait_until(Duration::from_secs(10), || {
        session_missing(&delete_id)
            && session_message_count(&keep_id).is_some_and(|count| count == 3)
            && checkpoint_row_count(&env.home, &keep_id) == 2
    })?;
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
    assert_ok(session_log::ipc::call_service(
        &SessionLogCommand::UpsertSession(upsert_session(
            &running_id,
            &workspace_key,
            100,
            "running",
            &["running-m1", "running-m2"],
            &[("running-todo", "doing")],
        )),
    )?)?;
    assert_ok(session_log::ipc::call_service(
        &SessionLogCommand::UpsertSession(upsert_session(
            &paused_id,
            &workspace_key,
            110,
            "paused",
            &["paused-m1"],
            &[("paused-todo", "waiting_user")],
        )),
    )?)?;
    assert_ok(session_log::ipc::call_service(
        &SessionLogCommand::UpsertSession(upsert_session(
            &completed_id,
            &workspace_key,
            120,
            "completed",
            &["completed-m1"],
            &[("completed-todo", "done")],
        )),
    )?)?;
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
        Some("doing"),
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
            let messages = vec![
                format!("m-{index}-0"),
                format!("m-{index}-1"),
                format!("m-{index}-2"),
            ];
            let message_refs = messages.iter().map(String::as_str).collect::<Vec<_>>();
            assert_ok(session_log::ipc::call_service(
                &SessionLogCommand::UpsertSession(upsert_session(
                    &session_id,
                    &workspace,
                    1_000 + index as i64,
                    "completed",
                    &message_refs,
                    &[("batch-task", "done")],
                )),
            )?)?;
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

fn enqueue(command: SessionLogCommand) -> Result<()> {
    file_queue::enqueue_command(&command)?;
    Ok(())
}

fn upsert_session(
    session_id: &str,
    workspace: &str,
    updated_at: i64,
    state: &str,
    messages: &[&str],
    todos: &[(&str, &str)],
) -> UpsertSessionRequest {
    UpsertSessionRequest {
        session: json!({
            "id": session_id,
            "name": format!("Session {session_id}"),
            "directory": workspace,
            "created_at": updated_at - 1,
            "updated_at": updated_at,
            "management": {
                "session_id": session_id,
                "session_name": format!("Session {session_id}"),
                "session_directory": workspace,
                "session_created_at": "2026-06-12T00:00:00.000Z",
                "session_last_update_at": "2026-06-12T00:00:01.000Z",
                "state": state,
                "task_plan": {
                    "plan_summary": format!("plan for {session_id}"),
                    "detailed_tasks": todos.iter().map(|(id, status)| {
                        json!({
                            "id": id,
                            "step": format!("task {id}"),
                            "status": status,
                            "deliverables": []
                        })
                    }).collect::<Vec<_>>()
                }
            }
        }),
        parent_id: None,
        messages: messages
            .iter()
            .enumerate()
            .map(|(index, message_id)| {
                json!({
                    "id": message_id,
                    "role": if index == 0 { "user" } else { "assistant" },
                    "created_at": updated_at + index as i64,
                    "updated_at": updated_at + index as i64,
                    "content": format!("content for {message_id}")
                })
            })
            .collect(),
        todos: todos
            .iter()
            .map(|(id, status)| {
                json!({
                    "id": id,
                    "content": format!("todo {id}"),
                    "status": status
                })
            })
            .collect(),
    }
}

fn checkpoint(session_id: &str, seq: i64, status: &str) -> CommandCheckpoint {
    CommandCheckpoint {
        session_id: session_id.to_string(),
        turn_id: "turn-restart-queue".to_string(),
        runtime_worker_id: Some("runtime-worker-restart-queue".to_string()),
        provider_call_id: Some("provider-restart-queue".to_string()),
        command_run_id: Some("command-run-restart-queue".to_string()),
        command_id: Some(format!("command-{seq}")),
        event_seq: Some(seq),
        command_type: Some("shell_command".to_string()),
        command_line: Some(format!("Write-Output restart-queue-{seq}")),
        status: status.to_string(),
        output_summary: Some(format!("restart queue checkpoint {seq}")),
        changes: json!({ "seq": seq, "files": [format!("file-{seq}.txt")] }),
        started_at: Some("2026-06-12T00:00:00Z".to_string()),
        finished_at: Some("2026-06-12T00:00:01Z".to_string()),
    }
}

fn assert_ok(response: SessionLogResponse) -> Result<()> {
    match response {
        SessionLogResponse::Ok => Ok(()),
        SessionLogResponse::Error { error } => bail!("session_db returned error: {error}"),
        other => bail!("session_db returned unexpected response: {other:?}"),
    }
}

fn assert_session_snapshot(
    session_id: &str,
    workspace: &str,
    state: &str,
    status: &str,
    message_count: u64,
    expected_task_status: Option<&str>,
) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => {
            let snapshot = session.ok_or_else(|| anyhow!("session {session_id} missing"))?;
            assert_eq!(snapshot.session_id, session_id);
            assert_eq!(snapshot.workspace, workspace);
            assert_eq!(snapshot.state.as_deref(), Some(state));
            assert_eq!(snapshot.status.as_deref(), Some(status));
            assert_eq!(snapshot.message_count, message_count);
            assert_eq!(snapshot.session["status"].as_str(), Some(status));
            assert_eq!(snapshot.management["state"].as_str(), Some(state));
            if let Some(expected_task_status) = expected_task_status {
                let tasks = snapshot
                    .task_management
                    .get("tasks")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| anyhow!("task management tasks missing"))?;
                assert!(
                    tasks
                        .iter()
                        .any(|task| task["status"].as_str() == Some(expected_task_status)),
                    "session {session_id} should retain task status {expected_task_status}: {tasks:?}"
                );
            }
            Ok(())
        }
        other => bail!("unexpected get session response: {other:?}"),
    }
}

fn assert_session_missing(session_id: &str) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => {
            assert!(session.is_none(), "session {session_id} should be missing");
            Ok(())
        }
        other => bail!("unexpected get missing session response: {other:?}"),
    }
}

fn assert_records(session_id: &str, expected: &[&str]) -> Result<()> {
    assert_eq!(record_ids(session_id)?, expected);
    Ok(())
}

fn record_ids(session_id: &str) -> Result<Vec<String>> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 50,
        },
    ))? {
        SessionLogResponse::Records { page, records } => {
            assert_eq!(page.total as usize, records.len());
            Ok(records
                .into_iter()
                .map(|record| record.message_id)
                .collect())
        }
        other => bail!("unexpected records response: {other:?}"),
    }
}

fn assert_workspace_page(
    workspace: &str,
    page: u64,
    page_size: u64,
    total: u64,
    expected_len: usize,
) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessions(ListSessionsRequest {
        workspace: workspace.to_string(),
        page,
        page_size,
    }))? {
        SessionLogResponse::Sessions {
            page: actual_page,
            sessions,
        } => {
            assert_eq!(actual_page.total, total);
            assert_eq!(actual_page.page_size, page_size.clamp(1, 500));
            assert_eq!(sessions.len(), expected_len);
            assert!(sessions
                .iter()
                .all(|session| session.workspace == workspace));
            Ok(())
        }
        other => bail!("unexpected list sessions response: {other:?}"),
    }
}

fn assert_workspace_summaries(expected: &[(String, u64)]) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListWorkspaces)? {
        SessionLogResponse::Workspaces { workspaces } => {
            for (workspace, count) in expected {
                let summary = workspaces
                    .iter()
                    .find(|summary| &summary.directory == workspace)
                    .ok_or_else(|| anyhow!("workspace summary missing for {workspace}"))?;
                assert_eq!(summary.session_count, *count);
                assert!(summary.last_updated_at > 0);
            }
            Ok(())
        }
        other => bail!("unexpected workspace summary response: {other:?}"),
    }
}

fn assert_checkpoint_rows(home: &Path, session_id: &str, expected: i64) -> Result<()> {
    let count = checkpoint_row_count(home, session_id);
    assert_eq!(
        count, expected,
        "session {session_id} should have {expected} durable checkpoint rows"
    );
    Ok(())
}

fn checkpoint_row_count(home: &Path, session_id: &str) -> i64 {
    let Ok(conn) = Connection::open(index_db_path(home)) else {
        return 0;
    };
    conn.query_row(
        "SELECT COUNT(*) FROM session_write_queue
         WHERE session_id = ?1
           AND turn_id = 'turn-restart-queue'
           AND runtime_worker_id = 'runtime-worker-restart-queue'
           AND command_run_id = 'command-run-restart-queue'
           AND status = 'applied'",
        [session_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

fn assert_index_state_matches_workspace_state(home: &Path, session_ids: &[&str]) -> Result<()> {
    let conn = Connection::open(index_db_path(home)).context("open index db")?;
    for session_id in session_ids {
        let (state, status, management): (String, String, String) = conn.query_row(
            "SELECT state, status, management_json FROM sessions WHERE session_id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let snapshot = get_session_snapshot(session_id)?
            .ok_or_else(|| anyhow!("snapshot missing for {session_id}"))?;
        assert_eq!(Some(state.as_str()), snapshot.state.as_deref());
        assert_eq!(Some(status.as_str()), snapshot.status.as_deref());
        let management: serde_json::Value = serde_json::from_str(&management)?;
        assert_eq!(management["state"].as_str(), snapshot.state.as_deref());
    }
    Ok(())
}

fn get_session_snapshot(session_id: &str) -> Result<Option<Box<session_log::SessionSnapshot>>> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => Ok(session),
        other => bail!("unexpected get session response: {other:?}"),
    }
}

fn session_visible(session_id: &str) -> bool {
    get_session_snapshot(session_id).ok().flatten().is_some()
}

fn session_missing(session_id: &str) -> bool {
    get_session_snapshot(session_id)
        .ok()
        .is_some_and(|session| session.is_none())
}

fn session_message_count(session_id: &str) -> Option<u64> {
    get_session_snapshot(session_id)
        .ok()
        .flatten()
        .map(|snapshot| snapshot.message_count)
}

fn session_state_status(session_id: &str) -> Option<(String, String)> {
    let snapshot = get_session_snapshot(session_id).ok().flatten()?;
    Some((snapshot.state?, snapshot.status?))
}

fn write_corrupt_pending_queue_item(home: &Path, label: &str) -> Result<PathBuf> {
    let pending = queue_dir(home, "pending");
    std::fs::create_dir_all(&pending)?;
    let path = pending.join(format!(
        "00000000000000000000-0-00000000000000000000-{label}.json"
    ));
    std::fs::write(&path, b"{ this is not valid json")?;
    Ok(path)
}

fn assert_failed_queue_contains_error(home: &Path, label: &str) -> Result<()> {
    let failed = queue_dir(home, "failed");
    let entries = std::fs::read_dir(&failed)
        .with_context(|| format!("read failed queue {}", failed.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let failed_json = entries.iter().any(|entry| {
        entry.file_name().to_string_lossy().contains(label)
            && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
    });
    let failed_error = entries.iter().any(|entry| {
        entry.file_name().to_string_lossy().contains(label)
            && entry.path().extension().and_then(|value| value.to_str()) == Some("txt")
    });
    assert!(
        failed_json,
        "failed queue should retain the corrupt json file"
    );
    assert!(failed_error, "failed queue should include an error sidecar");
    Ok(())
}

fn assert_pending_queue_empty(home: &Path) -> Result<()> {
    let pending_json = pending_queue_items(home);
    assert_eq!(pending_json, 0, "pending queue should be empty");
    Ok(())
}

fn pending_queue_items(home: &Path) -> usize {
    std::fs::read_dir(queue_dir(home, "pending"))
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .count()
}

fn failed_queue_items(home: &Path) -> usize {
    std::fs::read_dir(queue_dir(home, "failed"))
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .count()
}

fn assert_workspace_db_exists(workspace: &Path) -> Result<()> {
    let db = workspace.join(".tura").join("session_log.sqlite3");
    assert!(db.exists(), "workspace DB should exist at {}", db.display());
    Ok(())
}

fn queue_dir(home: &Path, segment: &str) -> PathBuf {
    home.join("db")
        .join("session_log")
        .join("message_queue")
        .join(segment)
}

fn index_db_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("index.sqlite3")
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    bail!("timed out after {}ms", timeout.as_millis())
}

struct SessionDbService {
    handle: Option<thread::JoinHandle<Result<()>>>,
}

impl SessionDbService {
    fn start() -> Result<Self> {
        let handle = thread::spawn(session_log::service::run_socket_service);
        wait_until(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )
        .context("session_db service did not become reachable")?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    fn shutdown(&mut self) -> Result<()> {
        if self.handle.is_none() {
            return Ok(());
        }
        assert_ok(session_log::ipc::call_service(
            &SessionLogCommand::Shutdown,
        )?)?;
        self.join(Duration::from_secs(10))
    }

    fn join(&mut self, timeout: Duration) -> Result<()> {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if self
                .handle
                .as_ref()
                .is_some_and(thread::JoinHandle::is_finished)
            {
                let handle = self
                    .handle
                    .take()
                    .ok_or_else(|| anyhow!("service handle missing"))?;
                return handle
                    .join()
                    .map_err(|_| anyhow!("session_db service thread panicked"))?;
            }
            thread::sleep(Duration::from_millis(25));
        }
        bail!(
            "session_db service did not stop within {}ms",
            timeout.as_millis()
        )
    }
}

impl Drop for SessionDbService {
    fn drop(&mut self) {
        if self.handle.is_some() {
            let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
            let _ = self.join(Duration::from_secs(5));
        }
    }
}

struct TestEnv {
    _temp: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    _env: EnvGuard,
}

impl TestEnv {
    fn new(name: &str) -> Result<Self> {
        let temp = tempfile::tempdir().context("create temp test root")?;
        let root = temp.path().join(name);
        let home = root.join("home");
        std::fs::create_dir_all(&home)?;
        let env = EnvGuard::set(&[
            ("TURA_HOME", Some(home.as_path())),
            ("TURA_DB_ROOT", None),
            ("SESSION_LOG_DB_ROOT", None),
            ("TURA_SESSION_DB_PROBE_TIMEOUT_MS", Some(Path::new("25"))),
        ]);
        Ok(Self {
            _temp: temp,
            home,
            root,
            _env: env,
        })
    }

    fn workspace(&self, name: &str) -> Result<PathBuf> {
        let workspace = self.root.join(name);
        std::fs::create_dir_all(&workspace)?;
        Ok(workspace)
    }

    fn session_id(&self, name: &str) -> String {
        format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            self.root
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("session")
        )
    }
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn set(values: &[(&'static str, Option<&Path>)]) -> Self {
        let previous = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        for (key, value) in values {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
