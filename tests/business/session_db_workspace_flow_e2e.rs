//! Required workspace-wide session_db state-flow E2E tests.
//!
//! This target is a required root-package business E2E, so plain workspace
//! `cargo test` covers the real session_db socket path across workspaces,
//! sessions, records, checkpoints, deletion, and graceful shutdown without
//! relying on third-party services.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use session_log::{
    CommandCheckpoint, DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest,
    ListSessionRecordsRequest, ListSessionsRequest, SessionLogCommand, SessionLogResponse,
    SessionLogStore, UpsertSessionRequest,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Barrier, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn session_db_workspace_flow_handles_concurrent_clients_checkpoint_idempotency_and_deletes(
) -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let root = temp_root("workspace-session-db-flow")?;
    let home = root.join("home");
    let workspace_a = root.join("workspace-a");
    let workspace_b = root.join("workspace-b");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace_a)?;
    std::fs::create_dir_all(&workspace_b)?;
    let _env = EnvGuard::set(&[
        ("TURA_HOME", Some(home.as_path())),
        ("TURA_DB_ROOT", None),
        ("SESSION_LOG_DB_ROOT", None),
    ]);
    let mut service = ServiceThread::start()?;

    let workspace_a_key = normalize_path(&workspace_a);
    let workspace_b_key = normalize_path(&workspace_b);
    let barrier = Arc::new(Barrier::new(8));
    let mut writers = Vec::new();
    for index in 0..8 {
        let barrier = Arc::clone(&barrier);
        let workspace = if index % 2 == 0 {
            workspace_a_key.clone()
        } else {
            workspace_b_key.clone()
        };
        writers.push(thread::spawn(move || -> Result<String> {
            barrier.wait();
            let session_id = format!("flow-session-{index}");
            let response = session_log::ipc::call_service(&SessionLogCommand::UpsertSession(
                upsert_request(&session_id, &workspace, index),
            ))?;
            assert_ok(response, "upsert session")?;
            Ok(session_id)
        }));
    }
    let mut session_ids = Vec::new();
    for writer in writers {
        session_ids.push(
            writer
                .join()
                .map_err(|_| anyhow!("session writer thread panicked"))??,
        );
    }
    session_ids.sort();

    assert_workspace_summary(&workspace_a_key, 4)?;
    assert_workspace_summary(&workspace_b_key, 4)?;
    assert_list_sessions(&workspace_a_key, 0, 2, 4, 2)?;
    assert_list_sessions(&workspace_a_key, 1, 2, 4, 2)?;

    let checkpoint_session = session_ids
        .iter()
        .find(|session_id| session_id.ends_with('0'))
        .cloned()
        .ok_or_else(|| anyhow!("missing checkpoint session"))?;
    let checkpoint = command_checkpoint(&checkpoint_session);
    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::ApplyCommandCheckpoint(Box::new(
            checkpoint.clone(),
        )))?,
        "apply checkpoint",
    )?;
    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::ApplyCommandCheckpoint(Box::new(
            checkpoint,
        )))?,
        "apply duplicate checkpoint",
    )?;
    assert_checkpoint_queue_count(&home, &checkpoint_session, 1)?;
    assert_original_record_remains(&checkpoint_session)?;

    let deleted_session = session_ids
        .iter()
        .find(|session_id| session_id.ends_with('2'))
        .cloned()
        .ok_or_else(|| anyhow!("missing session to delete"))?;
    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::DeleteSession(DeleteSessionRequest {
            session_id: deleted_session.clone(),
        }))?,
        "delete one session",
    )?;
    assert_session_missing(&deleted_session)?;
    assert_workspace_summary(&workspace_a_key, 3)?;
    assert_workspace_summary(&workspace_b_key, 4)?;

    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::DeleteWorkspace(
            DeleteWorkspaceRequest {
                workspace: workspace_b_key.clone(),
            },
        ))?,
        "delete workspace b",
    )?;
    assert_list_sessions(&workspace_b_key, 0, 50, 0, 0)?;
    assert_workspace_summary(&workspace_a_key, 3)?;

    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::Shutdown)?,
        "shutdown session_db",
    )?;
    service.join(Duration::from_secs(10))?;
    assert!(
        !session_log::ipc::service_is_running(),
        "session_db should not be reachable after graceful shutdown"
    );
    assert!(
        !service_addr_path(&home).exists(),
        "service addr file should be removed after graceful shutdown"
    );
    assert!(
        workspace_a
            .join(".tura")
            .join("session_log.sqlite3")
            .exists(),
        "workspace session log should live under the workspace .tura directory"
    );

    Ok(())
}

#[test]
fn session_db_read_path_bounds_pages_and_prunes_stale_workspace_index_rows() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let root = temp_root("workspace-session-db-read-path")?;
    let home = root.join("home");
    let workspace = root.join("workspace-read");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::set(&[
        ("TURA_HOME", Some(home.as_path())),
        ("TURA_DB_ROOT", None),
        ("SESSION_LOG_DB_ROOT", None),
    ]);
    let mut service = ServiceThread::start()?;

    let workspace_key = normalize_path(&workspace);
    let get_session_id = "read-path-get-session";
    let records_session_id = "read-path-records";
    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::UpsertSession(
            upsert_request_with_messages(
                get_session_id,
                &workspace_key,
                10,
                &["get-m1", "get-m2", "get-m3"],
            ),
        ))?,
        "upsert get-session target",
    )?;
    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::UpsertSession(
            upsert_request_with_messages(
                records_session_id,
                &workspace_key,
                20,
                &["records-m1", "records-m2", "records-m3"],
            ),
        ))?,
        "upsert records target",
    )?;

    assert_get_session_snapshot(get_session_id, &workspace_key, 3)?;
    assert_list_sessions_page(&workspace_key, 99, 0, 1, 1, 2, &[get_session_id])?;
    assert_records_page(records_session_id, 0, 1, 2, 1, 3, &["records-m3"])?;
    assert_records_page(
        records_session_id,
        99,
        999,
        0,
        500,
        3,
        &["records-m1", "records-m2", "records-m3"],
    )?;

    let workspace_db = session_log::path::workspace_session_log_db(&workspace_key);
    remove_sqlite_family(&workspace_db)?;

    assert_session_missing(get_session_id)?;
    assert_empty_records_page(records_session_id)?;
    assert_list_sessions(&workspace_key, 0, 50, 0, 0)?;
    assert_workspace_absent(&workspace_key)?;

    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::Shutdown)?,
        "shutdown session_db",
    )?;
    service.join(Duration::from_secs(10))?;
    Ok(())
}

#[test]
fn session_db_close_then_reselect_sessions_survives_mixed_reads_and_writes() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let root = temp_root("workspace-session-db-close-reselect")?;
    let home = root.join("home");
    let workspace = root.join("workspace-close-reselect");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::set(&[
        ("TURA_HOME", Some(home.as_path())),
        ("TURA_DB_ROOT", None),
        ("SESSION_LOG_DB_ROOT", None),
    ]);
    let mut service = ServiceThread::start()?;

    let workspace_key = normalize_path(&workspace);
    let session_ids = [
        "mixed-session-a",
        "mixed-session-b",
        "mixed-session-c",
        "mixed-session-d",
    ];
    for (index, session_id) in session_ids.iter().enumerate() {
        assert_ok(
            session_log::ipc::call_service(&SessionLogCommand::UpsertSession(
                upsert_request_with_messages(
                    session_id,
                    &workspace_key,
                    30 + index,
                    &[&format!("{session_id}-initial")],
                ),
            ))?,
            "seed mixed session",
        )?;
    }
    assert_list_sessions(&workspace_key, 0, 50, 4, 4)?;

    let closed_session = session_ids[0];
    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::DeleteSession(DeleteSessionRequest {
            session_id: closed_session.to_string(),
        }))?,
        "close first session",
    )?;
    assert_session_missing(closed_session)?;

    let live_sessions = &session_ids[1..];
    let mut expected_counts = live_sessions
        .iter()
        .map(|session_id| (*session_id, 1_u64))
        .collect::<std::collections::HashMap<_, _>>();
    for round in 0..4 {
        let selected = live_sessions[round % live_sessions.len()];
        assert_get_session_snapshot(selected, &workspace_key, expected_counts[selected])?;
        assert_records_include(selected, &format!("{selected}-initial"))?;

        let message_id = format!("{selected}-followup-{round}");
        assert_ok(
            session_log::ipc::call_service(&SessionLogCommand::UpsertSession(
                upsert_request_with_messages(
                    selected,
                    &workspace_key,
                    50 + round,
                    &[message_id.as_str()],
                ),
            ))?,
            "append selected session follow-up",
        )?;

        *expected_counts
            .get_mut(selected)
            .expect("selected live session count") += 1;
        assert_records_include(selected, &message_id)?;
        assert_list_sessions(&workspace_key, 0, 10, 3, 3)?;
        assert_empty_records_page(closed_session)?;
        assert_session_missing(closed_session)?;
    }

    for session_id in live_sessions {
        assert_get_session_snapshot(session_id, &workspace_key, expected_counts[session_id])?;
    }

    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::Shutdown)?,
        "shutdown session_db",
    )?;
    service.join(Duration::from_secs(10))?;
    Ok(())
}

fn upsert_request(session_id: &str, workspace: &str, index: usize) -> UpsertSessionRequest {
    let message_id = format!("message-{session_id}");
    upsert_request_with_messages(session_id, workspace, index, &[message_id.as_str()])
}

fn upsert_request_with_messages(
    session_id: &str,
    workspace: &str,
    index: usize,
    message_ids: &[&str],
) -> UpsertSessionRequest {
    UpsertSessionRequest {
        session: json!({
            "id": session_id,
            "name": format!("Flow Session {index}"),
            "directory": workspace,
            "created_at": index as i64,
            "updated_at": 100 + index as i64,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": format!("Flow Session {index}"),
                "state": "created",
                "current_turn": index
            }
        }),
        parent_id: None,
        messages: message_ids
            .iter()
            .enumerate()
            .map(|(offset, message_id)| {
                json!({
                    "id": message_id,
                    "role": if offset % 2 == 0 { "user" } else { "assistant" },
                    "created_at": index as i64 * 10 + offset as i64,
                    "updated_at": index as i64 * 10 + offset as i64,
                    "content": format!("content {index}-{offset}")
                })
            })
            .collect(),
        todos: vec![json!({
            "id": format!("todo-{session_id}"),
            "content": "persist workspace flow",
            "status": "done"
        })],
    }
}

fn command_checkpoint(session_id: &str) -> CommandCheckpoint {
    CommandCheckpoint {
        session_id: session_id.to_string(),
        turn_id: "turn-workspace-flow".to_string(),
        runtime_worker_id: Some("worker-workspace-flow".to_string()),
        provider_call_id: Some("provider-workspace-flow".to_string()),
        command_run_id: Some("run-workspace-flow".to_string()),
        command_id: Some("command-workspace-flow".to_string()),
        event_seq: Some(7),
        command_type: Some("shell_command".to_string()),
        command_line: Some("Write-Output workspace-flow".to_string()),
        status: "command_finished".to_string(),
        output_summary: Some("workspace-flow".to_string()),
        changes: json!({
            "files": ["workspace-flow.txt"],
            "summary": "created workspace flow marker"
        }),
        started_at: Some("2026-06-12T00:00:00Z".to_string()),
        finished_at: Some("2026-06-12T00:00:01Z".to_string()),
    }
}

fn assert_workspace_summary(workspace: &str, expected_sessions: u64) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListWorkspaces)? {
        SessionLogResponse::Workspaces { workspaces } => {
            let summary = workspaces
                .iter()
                .find(|item| item.directory == workspace)
                .ok_or_else(|| anyhow!("workspace summary missing for {workspace}"))?;
            assert_eq!(summary.session_count, expected_sessions);
            assert!(summary.last_updated_at >= 0);
            Ok(())
        }
        other => bail!("unexpected workspace response: {other:?}"),
    }
}

fn assert_workspace_absent(workspace: &str) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListWorkspaces)? {
        SessionLogResponse::Workspaces { workspaces } => {
            assert!(
                workspaces
                    .iter()
                    .all(|summary| summary.directory != workspace),
                "workspace {workspace} should be absent after stale rows are pruned: {workspaces:?}"
            );
            Ok(())
        }
        other => bail!("unexpected workspace response: {other:?}"),
    }
}

fn assert_list_sessions(
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
        SessionLogResponse::Sessions { page, sessions } => {
            assert_eq!(page.total, total);
            assert_eq!(sessions.len(), expected_len);
            assert!(sessions
                .iter()
                .all(|session| session.workspace == workspace && session.message_count >= 1));
            Ok(())
        }
        other => bail!("unexpected sessions response: {other:?}"),
    }
}

fn assert_list_sessions_page(
    workspace: &str,
    page: u64,
    page_size: u64,
    expected_page: u64,
    expected_page_size: u64,
    total: u64,
    expected_session_ids: &[&str],
) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessions(ListSessionsRequest {
        workspace: workspace.to_string(),
        page,
        page_size,
    }))? {
        SessionLogResponse::Sessions { page, sessions } => {
            assert_eq!(page.page, expected_page);
            assert_eq!(page.page_size, expected_page_size);
            assert_eq!(page.total, total);
            assert_eq!(
                sessions
                    .iter()
                    .map(|session| session.session_id.as_str())
                    .collect::<Vec<_>>(),
                expected_session_ids
            );
            Ok(())
        }
        other => bail!("unexpected sessions response: {other:?}"),
    }
}

fn assert_get_session_snapshot(
    session_id: &str,
    workspace: &str,
    expected_messages: u64,
) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => {
            let snapshot =
                session.ok_or_else(|| anyhow!("session {session_id} should be present"))?;
            assert_eq!(snapshot.session_id, session_id);
            assert_eq!(snapshot.workspace, workspace);
            assert_eq!(snapshot.message_count, expected_messages);
            assert_eq!(snapshot.todos.len(), 1);
            Ok(())
        }
        other => bail!("unexpected get session response: {other:?}"),
    }
}

fn assert_checkpoint_queue_count(
    home: &Path,
    session_id: &str,
    expected_checkpoints: i64,
) -> Result<()> {
    let conn = rusqlite::Connection::open(index_db_path(home))
        .with_context(|| format!("open session_db index for {session_id}"))?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM session_write_queue
         WHERE session_id = ?1
           AND turn_id = 'turn-workspace-flow'
           AND runtime_worker_id = 'worker-workspace-flow'
           AND command_run_id = 'run-workspace-flow'
           AND command_id = 'command-workspace-flow'
           AND event_seq = 7
           AND event_type = 'command_finished'
           AND status = 'applied'",
        [session_id],
        |row| row.get(0),
    )?;
    assert_eq!(
        count, expected_checkpoints,
        "duplicate checkpoint ACKs must collapse into one durable idempotency row"
    );
    Ok(())
}

fn assert_records_page(
    session_id: &str,
    page: u64,
    page_size: u64,
    expected_page: u64,
    expected_page_size: u64,
    total: u64,
    expected_record_ids: &[&str],
) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page,
            page_size,
        },
    ))? {
        SessionLogResponse::Records { page, records } => {
            assert_eq!(page.page, expected_page);
            assert_eq!(page.page_size, expected_page_size);
            assert_eq!(page.total, total);
            assert_eq!(
                records
                    .iter()
                    .map(|record| record.message_id.as_str())
                    .collect::<Vec<_>>(),
                expected_record_ids
            );
            Ok(())
        }
        other => bail!("unexpected records response: {other:?}"),
    }
}

fn assert_empty_records_page(session_id: &str) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 50,
        },
    ))? {
        SessionLogResponse::Records { page, records } => {
            assert_eq!(page.total, 0);
            assert!(records.is_empty());
            Ok(())
        }
        other => bail!("unexpected empty records response: {other:?}"),
    }
}

fn assert_records_include(session_id: &str, expected_record_id: &str) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 50,
        },
    ))? {
        SessionLogResponse::Records { records, .. } => {
            assert!(
                records
                    .iter()
                    .any(|record| record.message_id == expected_record_id),
                "session {session_id} should include record {expected_record_id}: {records:?}"
            );
            Ok(())
        }
        other => bail!("unexpected records include response: {other:?}"),
    }
}

fn assert_original_record_remains(session_id: &str) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 50,
        },
    ))? {
        SessionLogResponse::Records { page, records } => {
            assert_eq!(page.total as usize, records.len());
            assert!(
                records
                    .iter()
                    .any(|record| record.message_id == format!("message-{session_id}")),
                "original upsert record should remain beside checkpoint records"
            );
            Ok(())
        }
        other => bail!("unexpected records response: {other:?}"),
    }
}

fn assert_session_missing(session_id: &str) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => {
            assert!(session.is_none(), "session {session_id} should be deleted");
            Ok(())
        }
        other => bail!("unexpected get session response: {other:?}"),
    }
}

fn remove_sqlite_family(path: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let target = PathBuf::from(format!("{}{}", path.display(), suffix));
        if target.exists() {
            std::fs::remove_file(&target)
                .with_context(|| format!("remove stale workspace db file {}", target.display()))?;
        }
    }
    Ok(())
}

fn assert_ok(response: SessionLogResponse, context: &str) -> Result<()> {
    match response {
        SessionLogResponse::Ok => Ok(()),
        SessionLogResponse::Error { error } => bail!("{context} returned error: {error}"),
        other => bail!("{context} returned unexpected response: {other:?}"),
    }
}

struct ServiceThread {
    handle: Option<thread::JoinHandle<Result<()>>>,
}

impl ServiceThread {
    fn start() -> Result<Self> {
        let store = SessionLogStore::open_default().context("open session log store")?;
        let handle = thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_until(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    fn join(&mut self, timeout: Duration) -> Result<()> {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if self
                .handle
                .as_ref()
                .is_some_and(thread::JoinHandle::is_finished)
            {
                let handle = self.handle.take().expect("finished service handle");
                return handle
                    .join()
                    .map_err(|_| anyhow!("session_db service thread panicked"))?;
            }
            thread::sleep(Duration::from_millis(25));
        }
        bail!(
            "session_db service thread did not finish within {}ms",
            timeout.as_millis()
        )
    }
}

impl Drop for ServiceThread {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
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

fn temp_root(prefix: &str) -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!("{prefix}-{}", unique_nonce()?));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn unique_nonce() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before UNIX_EPOCH")?
        .as_nanos())
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

fn service_addr_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("service.addr")
}

fn index_db_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("index.sqlite3")
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
