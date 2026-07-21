#[path = "../support/typed_session.rs"]
mod typed_session;

use anyhow::{anyhow, Context, Result};
use gateway::session_db_client::SessionDbClient;
use lifecycle::TaskPlan;
use serde_json::json;
use session_log::SessionLogStore;
use session_log_contract::{MarkSessionInterruptedRequest, SessionLogCommand};
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
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    typed_session::create_via_service(
        &session_id,
        &workspace_key,
        "Gateway Session DB Business",
        1,
        TaskPlan::default(),
    )?;
    typed_session::persist_messages_via_service(
        &session_id,
        vec![
            message_payload(&session_id, "m-1", "user", 1),
            message_payload(&session_id, "m-2", "assistant", 2),
        ],
    )?;

    let workspaces = client.list_workspaces()?;
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
        !session_log_contract::client::service_is_running()
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
fn gateway_session_db_client_rejects_mutating_write_while_service_is_down() -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);

    let client = SessionDbClient::discover()?;
    let session_id = format!("gateway-offline-queue-{}", uuid::Uuid::new_v4());
    let write_error = client
        .call(SessionLogCommand::MarkSessionInterrupted(
            MarkSessionInterruptedRequest { session_id },
        ))
        .expect_err("gateway session_db client must reject non-owned mutation commands");
    assert!(
        write_error
            .to_string()
            .contains("only accepts queries and typed session commands"),
        "write rejection should explain the typed-only gateway client: {write_error:#}"
    );

    let read_error = client
        .list_workspaces()
        .expect_err("read operations must not silently fall back without a live service");
    assert!(
        read_error
            .to_string()
            .contains("session_db service is not running"),
        "read error should name the missing session_db service: {read_error:#}"
    );

    let store = SessionLogStore::open_default().context("open local store")?;
    assert_eq!(
        session_log::file_queue::drain_queue(&store, 10)?,
        0,
        "gateway read-only client must not enqueue rejected writes"
    );
    Ok(())
}

#[test]
fn gateway_session_db_business_flow_preserves_mixed_message_shapes() -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);
    let service = ServiceThread::start()?;

    let client = SessionDbClient::discover()?;
    let session_id = format!("gateway-mixed-shapes-{}", uuid::Uuid::new_v4());
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    typed_session::create_via_service(
        &session_id,
        &workspace_key,
        "Gateway Mixed Shapes",
        1,
        TaskPlan::default(),
    )?;
    typed_session::persist_messages_via_service(
        &session_id,
        vec![
            json!({
                "id": "runtime-mixed.message",
                "session_id": session_id,
                "role": "assistant",
                "parent_id": null,
                "created_at": 10,
                "updated_at": 11,
                "parts": [
                    {
                        "id": "runtime-mixed.message",
                        "type": "text",
                        "content": "final visible text",
                        "text": "final visible text",
                        "metadata": null,
                        "call_id": null,
                        "tool": null,
                        "state": null
                    },
                    {
                        "id": "runtime-usage-part",
                        "type": "tool",
                        "content": null,
                        "text": null,
                        "metadata": {"usage": {"total_tokens": 9}},
                        "call_id": "runtime-mixed",
                        "tool": "runtime",
                        "state": {"status": "completed"}
                    }
                ]
            }),
            json!({
                "id": "runtime-usage-aux",
                "session_id": session_id,
                "role": "runtime",
                "type": "runtime_usage",
                "created_at": 12,
                "updated_at": 12,
                "usage": {"total_tokens": 9}
            }),
            json!({
                "id": "diagnostic-simple-assistant",
                "session_id": session_id,
                "role": "assistant",
                "created_at": 13,
                "updated_at": 13,
                "content": "diagnostic simple shape"
            }),
        ],
    )?;

    let (_records_page, records) = client.list_session_records(session_id.clone(), 0, 50)?;
    assert_eq!(records.len(), 3);
    let visible = records
        .iter()
        .find(|record| record.message_id == "runtime-mixed.message")
        .ok_or_else(|| anyhow!("missing mixed final message record"))?;
    assert_eq!(visible.record["parts"][0]["text"], "final visible text");
    assert_eq!(
        visible.record["parts"][1]["metadata"]["usage"]["total_tokens"],
        9
    );
    assert!(
        records
            .iter()
            .any(|record| record.message_id == "diagnostic-simple-assistant"),
        "simple diagnostic shapes should be retained as records"
    );

    let loaded = client
        .get_session(session_id)?
        .ok_or_else(|| anyhow!("expected mixed session"))?;
    assert_eq!(loaded.message_count, 3);

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log_contract::client::service_is_running()
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
            let session_id = format!("gateway-concurrent-{index}-{}", uuid::Uuid::new_v4());
            barrier.wait();
            let workspace_key =
                session_log::path::normalize_workspace(&workspace.to_string_lossy());
            typed_session::create_via_service(
                &session_id,
                &workspace_key,
                "Gateway Concurrent Session",
                index,
                TaskPlan::default(),
            )?;
            typed_session::persist_messages_via_service(
                &session_id,
                vec![message_payload(
                    &session_id,
                    &format!("concurrent-m-{index}"),
                    "assistant",
                    index,
                )],
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
        !session_log_contract::client::service_is_running()
    })?;
    Ok(())
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
            "TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS",
        ];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
        std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", "1000");
        std::env::set_var("TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS", "5000");
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
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            if handle.is_finished() {
                let detail = match handle.join() {
                    Ok(Ok(())) => {
                        "session_db service exited before publishing service.addr".to_string()
                    }
                    Ok(Err(error)) => format!("session_db service exited with error: {error:#}"),
                    Err(_) => "session_db service thread panicked before publishing service.addr"
                        .to_string(),
                };
                return Err(anyhow!(detail));
            }
            if session_log_contract::client::service_is_running() {
                return Ok(Self {
                    handle: Some(handle),
                });
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(anyhow!(
            "session_db service did not become reachable within 10000ms"
        ))
    }
}

impl Drop for ServiceThread {
    fn drop(&mut self) {
        let _ = session_log_contract::client::call_service(&SessionLogCommand::Shutdown);
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
