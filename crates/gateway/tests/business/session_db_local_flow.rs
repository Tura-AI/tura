use anyhow::{anyhow, Context, Result};
use gateway::session_db_client::SessionDbClient;
use serde_json::json;
use session_log::{SessionLogCommand, SessionLogStore};
use std::path::Path;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn gateway_session_db_business_flow_reads_written_session_and_records() -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);
    let service = ServiceThread::start()?;

    let client = SessionDbClient::discover()?;
    let session_id = format!("gateway-session-db-business-{}", std::process::id());
    client.upsert_session(
        session_payload(&session_id, &workspace, 1),
        None,
        vec![message_payload(&session_id, "m-1", "user", 1)],
        vec![json!({ "id": "todo-1", "content": "prove local session db flow" })],
    )?;
    client.upsert_session(
        session_payload(&session_id, &workspace, 2),
        None,
        vec![
            message_payload(&session_id, "m-1", "user", 1),
            message_payload(&session_id, "m-2", "assistant", 2),
        ],
        vec![json!({ "id": "todo-1", "content": "prove local session db flow", "status": "done" })],
    )?;

    let workspaces = client.list_workspaces()?;
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    assert!(
        workspaces
            .iter()
            .any(|summary| summary.directory == workspace_key && summary.session_count == 1),
        "gateway client should see the workspace summary written through session_db: {workspaces:?}"
    );

    let (sessions_page, sessions) = client.list_sessions(workspace_key, 0, 50)?;
    assert_eq!(sessions_page.total, 1);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, session_id);
    assert_eq!(sessions[0].message_count, 2);
    assert_eq!(sessions[0].state.as_deref(), Some("created"));
    assert_eq!(sessions[0].status.as_deref(), Some("idle"));

    let loaded = client
        .get_session(session_id.clone())?
        .ok_or_else(|| anyhow!("expected persisted session"))?;
    assert_eq!(loaded.session["id"], session_id);
    assert_eq!(loaded.todos.len(), 1);
    assert_eq!(loaded.todos[0]["status"], "done");

    let (records_page, records) = client.list_session_records(session_id, 0, 50)?;
    assert_eq!(records_page.total, 2);
    assert_eq!(
        records
            .iter()
            .map(|record| record.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["m-1", "m-2"]
    );

    let workspace_db = workspace.join(".tura").join("session_log.sqlite3");
    assert!(
        workspace_db.exists(),
        "business flow must store the workspace session log under .tura, got {}",
        workspace_db.display()
    );

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    let read_after_shutdown = client
        .list_workspaces()
        .expect_err("reads require live session_db");
    assert!(
        read_after_shutdown
            .to_string()
            .contains("session_db service is not running"),
        "read failure should explain missing session_db service: {read_after_shutdown:#}"
    );
    Ok(())
}

#[test]
fn gateway_session_db_client_queues_mutating_write_while_service_is_down_then_reads_after_drain(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);

    let client = SessionDbClient::discover()?;
    let session_id = format!("gateway-offline-queue-{}", uuid::Uuid::new_v4());
    client.upsert_session(
        session_payload(&session_id, &workspace, 1),
        None,
        vec![message_payload(&session_id, "offline-m-1", "user", 1)],
        vec![json!({ "id": "offline-todo", "content": "queued while owner is down" })],
    )?;

    let read_error = client
        .list_workspaces()
        .expect_err("read operations must not silently fall back without a live service");
    assert!(
        read_error
            .to_string()
            .contains("session_db service is not running"),
        "read error should name the missing session_db service: {read_error:#}"
    );

    let store = SessionLogStore::open_default().context("open queued store")?;
    assert_eq!(
        session_log::file_queue::drain_queue(&store, 10)?,
        1,
        "the gateway client should enqueue exactly one offline write"
    );
    drop(store);

    let service = ServiceThread::start()?;
    let loaded = client
        .get_session(session_id.clone())?
        .ok_or_else(|| anyhow!("expected queued session after drain"))?;
    assert_eq!(loaded.session_id, session_id);
    assert_eq!(loaded.message_count, 1);
    assert_eq!(loaded.todos[0]["id"], "offline-todo");

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}

#[test]
fn gateway_session_db_client_concurrent_writes_preserve_workspace_listing_and_records() -> Result<()>
{
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);
    let service = ServiceThread::start()?;

    let barrier = Arc::new(Barrier::new(6));
    let mut handles = Vec::new();
    for index in 0..6 {
        let barrier = Arc::clone(&barrier);
        let workspace = workspace.clone();
        handles.push(std::thread::spawn(move || -> Result<String> {
            let client = SessionDbClient::discover()?;
            let session_id = format!("gateway-concurrent-{index}-{}", uuid::Uuid::new_v4());
            barrier.wait();
            client.upsert_session(
                session_payload(&session_id, &workspace, index),
                None,
                vec![message_payload(
                    &session_id,
                    &format!("concurrent-m-{index}"),
                    "assistant",
                    index,
                )],
                vec![json!({ "id": format!("todo-{index}"), "status": "todo" })],
            )?;
            Ok(session_id)
        }));
    }

    let mut session_ids = Vec::new();
    for handle in handles {
        session_ids.push(
            handle
                .join()
                .map_err(|_| anyhow!("gateway session_db client thread panicked"))??,
        );
    }
    session_ids.sort();

    let client = SessionDbClient::discover()?;
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let (page, sessions) = client.list_sessions(workspace_key, 0, 50)?;
    assert_eq!(page.total, 6);
    let mut listed = sessions
        .iter()
        .map(|session| session.session_id.clone())
        .collect::<Vec<_>>();
    listed.sort();
    assert_eq!(listed, session_ids);

    for session_id in session_ids {
        let (records_page, records) = client.list_session_records(session_id.clone(), 0, 10)?;
        assert_eq!(records_page.total, 1);
        assert_eq!(records[0].session_id, session_id);
        assert!(
            records[0].message_id.starts_with("concurrent-m-"),
            "unexpected message id: {}",
            records[0].message_id
        );
    }

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}

fn session_payload(session_id: &str, workspace: &Path, sequence: i64) -> serde_json::Value {
    json!({
        "id": session_id,
        "name": "Gateway Session DB Business",
        "directory": workspace.to_string_lossy(),
        "created_at": 1,
        "updated_at": sequence,
        "status": "idle",
        "management": {
            "session_id": session_id,
            "session_name": "Gateway Session DB Business",
            "state": "created"
        }
    })
}

fn message_payload(
    session_id: &str,
    message_id: &str,
    role: &str,
    sequence: i64,
) -> serde_json::Value {
    json!({
        "id": message_id,
        "session_id": session_id,
        "role": role,
        "created_at": sequence,
        "updated_at": sequence,
        "parts": [{ "type": "text", "text": format!("{role} message {sequence}") }]
    })
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &Path) -> Self {
        let keys = [
            "TURA_HOME",
            "SESSION_LOG_DB_ROOT",
            "TURA_DB_ROOT",
            "TURA_SESSION_DB_PROBE_TIMEOUT_MS",
        ];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
        std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", "20");
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

struct ServiceThread {
    handle: Option<std::thread::JoinHandle<Result<()>>>,
}

impl ServiceThread {
    fn start() -> Result<Self> {
        let store = SessionLogStore::open_default().context("open session log store")?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_until(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
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

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(anyhow!(
        "condition was not met within {}ms",
        timeout.as_millis()
    ))
}
