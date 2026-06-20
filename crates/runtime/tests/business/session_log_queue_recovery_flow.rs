use anyhow::{anyhow, Context, Result};
use runtime::session_log_client::SessionLogClient;
use serde_json::json;
use session_log::SessionLogCommand;
use std::path::Path;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn runtime_session_log_business_flow_replays_queued_write_after_service_start() -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp runtime session log root")?;
    let home = temp.path().join("home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);

    let client = SessionLogClient::discover()?;
    let session_id = format!("runtime-queue-recovery-{}", std::process::id());
    assert!(
        client.list_workspaces().is_err(),
        "reads should fail before the session_db service is reachable"
    );

    client.upsert_session(
        session_payload(&session_id, &workspace, 1),
        None,
        vec![message_payload(&session_id, "m-queued", "assistant", 1)],
        vec![json!({ "id": "queued-todo", "content": "drain queued runtime write" })],
    )?;
    assert!(
        queue_pending_dir(&home).exists(),
        "runtime write should create the file-backed queue before session_db starts"
    );
    assert!(
        pending_queue_files(&home)? >= 1,
        "queued runtime write should be visible as a pending file"
    );

    let service = ServiceThread::start()?;
    wait_until(Duration::from_secs(10), || {
        client
            .get_session(session_id.clone())
            .ok()
            .flatten()
            .is_some()
    })?;

    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let (sessions_page, sessions) = client.list_sessions(workspace_key, 0, 50)?;
    assert_eq!(sessions_page.total, 1);
    assert_eq!(sessions[0].session_id, session_id);
    assert_eq!(sessions[0].message_count, 1);

    let (_records_page, records) = client.list_session_records(session_id, 0, 50)?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message_id, "m-queued");

    assert_eq!(
        pending_queue_files(&home)?,
        0,
        "session_db startup should drain queued runtime writes"
    );
    assert!(
        workspace.join(".tura").join("session_log.sqlite3").exists(),
        "drained runtime write should land in the workspace .tura session log"
    );

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}

#[test]
fn runtime_session_log_business_flow_drains_concurrent_offline_writes_without_lost_records(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp runtime concurrent queue root")?;
    let home = temp.path().join("home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);
    let probe_client = SessionLogClient::discover()?;
    assert!(
        probe_client.list_workspaces().is_err(),
        "reads must fail before the session_db service is reachable"
    );

    let barrier = Arc::new(Barrier::new(6));
    let mut handles = Vec::new();
    for index in 0..6 {
        let barrier = Arc::clone(&barrier);
        let workspace = workspace.clone();
        handles.push(std::thread::spawn(move || -> Result<String> {
            let client = SessionLogClient::discover()?;
            let session_id = format!("runtime-concurrent-queue-{index}-{}", uuid::Uuid::new_v4());
            barrier.wait();
            client.upsert_session(
                session_payload(&session_id, &workspace, index),
                None,
                vec![message_payload(
                    &session_id,
                    &format!("queued-m-{index}"),
                    "assistant",
                    index,
                )],
                vec![json!({ "id": format!("queued-todo-{index}") })],
            )?;
            Ok(session_id)
        }));
    }

    let mut session_ids = Vec::new();
    for handle in handles {
        session_ids.push(
            handle
                .join()
                .map_err(|_| anyhow!("runtime queue writer thread panicked"))??,
        );
    }
    session_ids.sort();
    assert!(
        pending_queue_files(&home)? >= 1,
        "offline runtime writers should create file-backed queue items before service start"
    );

    let service = ServiceThread::start()?;
    let client = SessionLogClient::discover()?;
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let expected_session_ids = session_ids.clone();
    wait_until(Duration::from_secs(10), || {
        all_sessions_and_records_visible(&client, &workspace_key, &expected_session_ids)
    })?;

    let (sessions_page, sessions) = client.list_sessions(workspace_key, 0, 50)?;
    assert_eq!(sessions_page.total, 6);
    let mut listed = sessions
        .iter()
        .map(|session| session.session_id.clone())
        .collect::<Vec<_>>();
    listed.sort();
    assert_eq!(listed, session_ids);
    assert_eq!(
        pending_queue_files(&home)?,
        0,
        "all concurrent offline writes should be removed from the pending queue after becoming readable"
    );

    for session_id in session_ids {
        let (records_page, records) = client.list_session_records(session_id.clone(), 0, 10)?;
        assert_eq!(records_page.total, 1);
        assert_eq!(records[0].session_id, session_id);
        assert!(records[0].message_id.starts_with("queued-m-"));
    }

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}

#[test]
fn runtime_session_log_business_flow_online_reads_are_workspace_scoped_paged_and_idempotent(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp runtime online session log root")?;
    let home = temp.path().join("home");
    let workspace_a = temp.path().join("workspace-a");
    let workspace_b = temp.path().join("workspace-b");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace_a)?;
    std::fs::create_dir_all(&workspace_b)?;
    let _env = EnvGuard::new(&home);
    let service = ServiceThread::start()?;
    let client = SessionLogClient::discover()?;

    let session_a1 = format!("runtime-online-a1-{}", uuid::Uuid::new_v4());
    let session_a2 = format!("runtime-online-a2-{}", uuid::Uuid::new_v4());
    let session_b1 = format!("runtime-online-b1-{}", uuid::Uuid::new_v4());
    client.upsert_session(
        session_payload(&session_a1, &workspace_a, 1),
        None,
        vec![
            message_payload(&session_a1, "a1-m1", "user", 1),
            message_payload(&session_a1, "a1-m2", "assistant", 2),
        ],
        vec![json!({ "id": "a1-todo", "status": "todo" })],
    )?;
    client.upsert_session(
        session_payload(&session_a2, &workspace_a, 3),
        Some(session_a1.clone()),
        vec![message_payload(&session_a2, "a2-m1", "assistant", 1)],
        Vec::new(),
    )?;
    client.upsert_session(
        session_payload(&session_b1, &workspace_b, 2),
        None,
        vec![message_payload(&session_b1, "b1-m1", "assistant", 1)],
        Vec::new(),
    )?;

    let workspace_a_key = session_log::path::normalize_workspace(&workspace_a.to_string_lossy());
    let workspace_b_key = session_log::path::normalize_workspace(&workspace_b.to_string_lossy());
    wait_until(Duration::from_secs(10), || {
        let Ok(mut workspaces) = client.list_workspaces() else {
            return false;
        };
        workspaces.sort_by(|left, right| left.directory.cmp(&right.directory));
        if workspaces.len() != 2
            || workspaces[0].directory != workspace_a_key
            || workspaces[0].session_count != 2
            || workspaces[1].directory != workspace_b_key
            || workspaces[1].session_count != 1
        {
            return false;
        }
        session_records_have_ids(&client, &session_a1, &["a1-m1", "a1-m2"])
            && session_records_have_ids(&client, &session_a2, &["a2-m1"])
            && session_records_have_ids(&client, &session_b1, &["b1-m1"])
    })?;

    let mut workspaces = client.list_workspaces()?;
    workspaces.sort_by(|left, right| left.directory.cmp(&right.directory));
    assert_eq!(workspaces.len(), 2);
    assert_eq!(workspaces[0].directory, workspace_a_key);
    assert_eq!(workspaces[0].session_count, 2);
    assert_eq!(workspaces[1].directory, workspace_b_key);
    assert_eq!(workspaces[1].session_count, 1);

    let (page_a0, sessions_a0) = client.list_sessions(workspace_a_key.clone(), 0, 1)?;
    assert_eq!(page_a0.total, 2);
    assert_eq!(page_a0.page_size, 1);
    assert_eq!(sessions_a0.len(), 1);
    assert_eq!(
        sessions_a0[0].session_id, session_a2,
        "workspace A page 0 should return newest updated session first"
    );
    assert_eq!(
        sessions_a0[0].parent_id.as_deref(),
        Some(session_a1.as_str())
    );

    let (page_a1, sessions_a1) = client.list_sessions(workspace_a_key.clone(), 1, 1)?;
    assert_eq!(page_a1.total, 2);
    assert_eq!(page_a1.page, 1);
    assert_eq!(sessions_a1.len(), 1);
    assert_eq!(sessions_a1[0].session_id, session_a1);

    let (page_b, sessions_b) = client.list_sessions(workspace_b_key, 0, 10)?;
    assert_eq!(page_b.total, 1);
    assert_eq!(sessions_b.len(), 1);
    assert_eq!(sessions_b[0].session_id, session_b1);

    let snapshot = client
        .get_session(session_a1.clone())?
        .ok_or_else(|| anyhow!("expected first workspace A session"))?;
    assert_eq!(snapshot.workspace, workspace_a_key);
    assert_eq!(snapshot.message_count, 2);
    assert_eq!(snapshot.todos.len(), 1);
    assert_eq!(snapshot.todos[0]["id"], "a1-todo");

    client.upsert_session(
        session_payload(&session_a1, &workspace_a, 4),
        None,
        vec![
            updated_message_payload(&session_a1, "a1-m2", "assistant", 2, "assistant updated"),
            message_payload(&session_a1, "a1-m3", "assistant", 3),
        ],
        vec![json!({ "id": "a1-todo", "status": "done" })],
    )?;
    wait_until(Duration::from_secs(10), || {
        let Some(updated) = client.get_session(session_a1.clone()).ok().flatten() else {
            return false;
        };
        updated.message_count == 2
            && updated
                .todos
                .first()
                .and_then(|todo| todo.get("status"))
                .and_then(serde_json::Value::as_str)
                == Some("done")
            && session_records_have_ids(&client, &session_a1, &["a1-m2", "a1-m3"])
    })?;
    let updated = client
        .get_session(session_a1.clone())?
        .ok_or_else(|| anyhow!("expected updated workspace A session"))?;
    assert_eq!(updated.message_count, 2);
    assert_eq!(updated.todos[0]["status"], "done");

    let (records_page, records) = client.list_session_records(session_a1.clone(), 0, 10)?;
    assert_eq!(records_page.total, 2);
    assert_eq!(
        records
            .iter()
            .map(|record| record.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a1-m2", "a1-m3"],
        "online upsert is a full snapshot and deletes records absent from the runtime source"
    );
    let updated_m2 = records
        .iter()
        .find(|record| record.message_id == "a1-m2")
        .ok_or_else(|| anyhow!("missing updated a1-m2 record"))?;
    assert_eq!(updated_m2.record["parts"][0]["text"], "assistant updated");

    let (tail_page, tail_records) = client.list_session_records(session_a1, 0, 2)?;
    assert_eq!(tail_page.total, 2);
    assert_eq!(tail_page.page, 0);
    assert_eq!(
        tail_records
            .iter()
            .map(|record| record.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a1-m2", "a1-m3"],
        "record page 0 returns the full bounded page after snapshot replacement"
    );

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}

fn all_sessions_and_records_visible(
    client: &SessionLogClient,
    workspace_key: &str,
    expected_session_ids: &[String],
) -> bool {
    let Ok((page, sessions)) = client.list_sessions(workspace_key.to_string(), 0, 50) else {
        return false;
    };
    if page.total != expected_session_ids.len() as u64 {
        return false;
    }
    let mut listed = sessions
        .iter()
        .map(|session| session.session_id.clone())
        .collect::<Vec<_>>();
    listed.sort();
    if listed != expected_session_ids {
        return false;
    }
    expected_session_ids.iter().all(|session_id| {
        client
            .list_session_records(session_id.clone(), 0, 10)
            .is_ok_and(|(page, records)| {
                page.total == 1
                    && records.len() == 1
                    && records[0].session_id == *session_id
                    && records[0].message_id.starts_with("queued-m-")
            })
    })
}

fn wait_for_session(
    client: &SessionLogClient,
    session_id: &str,
    mut condition: impl FnMut(&session_log::SessionSnapshot) -> bool,
) -> Result<()> {
    wait_until(Duration::from_secs(10), || {
        client
            .get_session(session_id.to_string())
            .ok()
            .flatten()
            .is_some_and(|snapshot| condition(&snapshot))
    })
}

fn session_records_have_ids(
    client: &SessionLogClient,
    session_id: &str,
    expected_ids: &[&str],
) -> bool {
    client
        .list_session_records(session_id.to_string(), 0, expected_ids.len() as u64 + 1)
        .is_ok_and(|(page, records)| {
            page.total == expected_ids.len() as u64
                && records
                    .iter()
                    .map(|record| record.message_id.as_str())
                    .collect::<Vec<_>>()
                    == expected_ids
        })
}

#[test]
fn runtime_session_log_business_flow_restart_marks_running_session_interrupted() -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp runtime restart recovery root")?;
    let home = temp.path().join("home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);

    let service = ServiceThread::start()?;
    let client = SessionLogClient::discover()?;
    let session_id = format!("runtime-running-restart-{}", uuid::Uuid::new_v4());
    client.upsert_session(
        running_session_payload(&session_id, &workspace),
        None,
        vec![message_payload(&session_id, "running-m-1", "assistant", 1)],
        vec![json!({ "id": "running-todo", "status": "in_progress" })],
    )?;
    wait_for_session(&client, &session_id, |snapshot| {
        snapshot.state.as_deref() == Some("running") && snapshot.message_count == 1
    })?;
    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;

    let restarted = ServiceThread::start()?;
    wait_for_session(&client, &session_id, |snapshot| {
        snapshot.state.as_deref() == Some("interrupted")
            && snapshot.status.as_deref() == Some("error")
            && snapshot.message_count == 1
    })?;
    let recovered = client
        .get_session(session_id.clone())?
        .ok_or_else(|| anyhow!("expected recovered runtime session"))?;
    assert_eq!(recovered.session_id, session_id);
    assert_eq!(recovered.state.as_deref(), Some("interrupted"));
    assert_eq!(recovered.status.as_deref(), Some("error"));
    assert_eq!(recovered.management["state"], "interrupted");
    assert_eq!(recovered.message_count, 1);
    let (_page, records) = client.list_session_records(session_id, 0, 10)?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message_id, "running-m-1");

    drop(restarted);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}

#[test]
fn runtime_session_log_business_flow_resumes_interrupted_session_without_losing_history(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp runtime interrupted resume root")?;
    let home = temp.path().join("home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);

    let service = ServiceThread::start()?;
    let client = SessionLogClient::discover()?;
    let session_id = format!("runtime-interrupted-resume-{}", uuid::Uuid::new_v4());
    client.upsert_session(
        running_session_payload(&session_id, &workspace),
        None,
        vec![
            message_payload(&session_id, "resume-m-1", "user", 1),
            message_payload(&session_id, "resume-m-2", "assistant", 2),
        ],
        vec![json!({ "id": "resume-todo-1", "status": "doing" })],
    )?;
    wait_for_session(&client, &session_id, |snapshot| {
        snapshot.state.as_deref() == Some("running") && snapshot.message_count == 2
    })?;
    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;

    let restarted = ServiceThread::start()?;
    wait_for_session(&client, &session_id, |snapshot| {
        snapshot.state.as_deref() == Some("interrupted")
            && snapshot.status.as_deref() == Some("error")
            && snapshot.message_count == 2
    })?;
    let interrupted = client
        .get_session(session_id.clone())?
        .ok_or_else(|| anyhow!("expected interrupted runtime session"))?;
    assert_eq!(interrupted.state.as_deref(), Some("interrupted"));
    assert_eq!(interrupted.status.as_deref(), Some("error"));
    assert_eq!(interrupted.message_count, 2);

    client.upsert_session(
        resumed_session_payload(&session_id, &workspace),
        None,
        vec![
            message_payload(&session_id, "resume-m-1", "user", 1),
            message_payload(&session_id, "resume-m-2", "assistant", 2),
            message_payload(&session_id, "resume-m-3", "user", 3),
            message_payload(&session_id, "resume-m-4", "assistant", 4),
        ],
        vec![json!({ "id": "resume-todo-1", "status": "done" })],
    )?;
    wait_for_session(&client, &session_id, |snapshot| {
        snapshot.state.as_deref() == Some("created")
            && snapshot.status.as_deref() == Some("idle")
            && snapshot.message_count == 4
    })?;

    let resumed = client
        .get_session(session_id.clone())?
        .ok_or_else(|| anyhow!("expected resumed runtime session"))?;
    assert_eq!(resumed.session_id, session_id);
    assert_eq!(resumed.state.as_deref(), Some("created"));
    assert_eq!(resumed.status.as_deref(), Some("idle"));
    assert_eq!(resumed.management["state"], "created");
    assert_eq!(resumed.message_count, 4);

    let (all_page, all_records) = client.list_session_records(session_id.clone(), 0, 10)?;
    assert_eq!(all_page.total, 4);
    assert_eq!(all_records.len(), 4);
    assert_eq!(
        all_records
            .iter()
            .map(|record| record.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["resume-m-1", "resume-m-2", "resume-m-3", "resume-m-4"]
    );

    let (tail_page, tail_records) = client.list_session_records(session_id, 0, 2)?;
    assert_eq!(tail_page.total, 4);
    assert_eq!(tail_page.page, 1);
    assert_eq!(tail_records.len(), 2);
    assert_eq!(tail_records[0].message_id, "resume-m-3");
    assert_eq!(tail_records[1].message_id, "resume-m-4");

    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let (sessions_page, sessions) = client.list_sessions(workspace_key, 0, 10)?;
    assert_eq!(sessions_page.total, 1);
    assert_eq!(sessions[0].session_id, resumed.session_id);
    assert_eq!(sessions[0].message_count, 4);

    drop(restarted);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}

fn session_payload(session_id: &str, workspace: &Path, updated_at: i64) -> serde_json::Value {
    json!({
        "id": session_id,
        "name": "Runtime Queue Recovery",
        "directory": workspace.to_string_lossy(),
        "created_at": 1,
        "updated_at": updated_at,
        "status": "idle",
        "management": {
            "session_id": session_id,
            "session_name": "Runtime Queue Recovery",
            "state": "created"
        }
    })
}

fn resumed_session_payload(session_id: &str, workspace: &Path) -> serde_json::Value {
    json!({
        "id": session_id,
        "name": "Runtime Interrupted Resume Recovery",
        "directory": workspace.to_string_lossy(),
        "created_at": 1,
        "updated_at": 5,
        "status": "idle",
        "management": {
            "session_id": session_id,
            "session_name": "Runtime Interrupted Resume Recovery",
            "state": "created"
        }
    })
}

fn running_session_payload(session_id: &str, workspace: &Path) -> serde_json::Value {
    json!({
        "id": session_id,
        "name": "Runtime Running Restart Recovery",
        "directory": workspace.to_string_lossy(),
        "created_at": 1,
        "updated_at": 2,
        "status": "idle",
        "management": {
            "session_id": session_id,
            "session_name": "Runtime Running Restart Recovery",
            "state": "running"
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
        "parts": [{ "type": "text", "text": format!("{role} {sequence}") }]
    })
}

fn updated_message_payload(
    session_id: &str,
    message_id: &str,
    role: &str,
    sequence: i64,
    text: &str,
) -> serde_json::Value {
    json!({
        "id": message_id,
        "session_id": session_id,
        "role": role,
        "created_at": sequence,
        "updated_at": sequence + 100,
        "parts": [{ "type": "text", "text": text }]
    })
}

fn queue_pending_dir(home: &Path) -> std::path::PathBuf {
    home.join("db")
        .join("session_log")
        .join("message_queue")
        .join("pending")
}

fn pending_queue_files(home: &Path) -> Result<usize> {
    let pending = queue_pending_dir(home);
    if !pending.exists() {
        return Ok(0);
    }
    Ok(std::fs::read_dir(&pending)?
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        .count())
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
        let handle = std::thread::spawn(session_log::service::run_socket_service);
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
