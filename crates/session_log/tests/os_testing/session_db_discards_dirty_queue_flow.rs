//! Business E2E: dirty session_db queue data must never prevent the owner from
//! starting. Malformed durable queue rows are deleted, malformed file queue
//! items are quarantined, and clean writes still work through the live service.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use session_log::{
    file_queue, GetSessionRequest, SessionLogCommand, SessionLogResponse, SessionLogStore,
    UpsertSessionRequest,
};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn session_db_start_quarantines_dirty_file_queue_items_and_accepts_clean_writes() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let repo = repo_root()?;
    ensure_session_db_binary(&repo)?;

    let root = temp_root("session-db-dirty-queue")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let _env = EnvGuard::set(&[
        ("TURA_HOME", Some(home.as_path())),
        ("TURA_PROJECT_ROOT", Some(repo.as_path())),
        ("TURA_DB_ROOT", None),
        ("SESSION_LOG_DB_ROOT", None),
    ]);

    let store = SessionLogStore::open_default()?;
    insert_dirty_sqlite_queue_rows(&home, &workspace_key)?;
    let dirty_file = write_dirty_file_queue_item(&home)?;

    let mut service = SessionDbGuard::start(&repo, &home, &workspace)?;
    wait_until(
        Duration::from_secs(30),
        session_log::ipc::service_is_running,
    )
    .context("session_db did not publish a reachable socket")?;
    call_service_with_retry(&SessionLogCommand::ListWorkspaces, Duration::from_secs(30))
        .context("session_db did not become ready for data-path reads")?;

    assert_eq!(pending_sqlite_queue_count(&home)?, 0);
    wait_until(Duration::from_secs(10), || !dirty_file.exists())
        .context("dirty file queue item stayed pending")?;
    assert_failed_file_queue_contains(&home, &dirty_file)?;

    assert_ok(
        call_service_with_retry(
            &SessionLogCommand::UpsertSession(upsert_request("clean-direct", &workspace_key, 10)),
            Duration::from_secs(30),
        )?,
        "direct clean write",
    )?;
    file_queue::enqueue_command(&SessionLogCommand::UpsertSession(upsert_request(
        "clean-file-queue",
        &workspace_key,
        20,
    )))?;
    wait_until(Duration::from_secs(10), || {
        session_visible("clean-file-queue").unwrap_or(false)
    })
    .context("clean file queue write was not drained")?;

    assert!(session_visible("clean-direct")?);
    assert!(store
        .get_session(GetSessionRequest {
            session_id: "clean-direct".to_string(),
        })?
        .is_some());

    let _ = call_service_with_retry(&SessionLogCommand::Shutdown, Duration::from_secs(10));
    service.wait(Duration::from_secs(10))?;
    Ok(())
}

fn insert_dirty_sqlite_queue_rows(home: &Path, workspace: &str) -> Result<()> {
    let index_db = home.join("db").join("session_log").join("index.sqlite3");
    let conn = rusqlite::Connection::open(&index_db)
        .with_context(|| format!("open index db {}", index_db.display()))?;
    let bad_state = serde_json::to_string(&upsert_request("dirty-state", workspace, 1))?
        .replace("\"state\":\"created\"", "\"state\":\"Created\"");
    conn.execute(
        "INSERT INTO session_write_queue(
            idempotency_key, session_id, event_type, payload_json, status, retry_count, created_at
        ) VALUES
            ('dirty-json', 'dirty-json', 'upsert_session', '{not-json', 'pending', 0, 1),
            ('dirty-state', 'dirty-state', 'upsert_session', ?1, 'pending', 0, 2),
            ('dirty-event', 'dirty-event', 'unknown_event', '{}', 'pending', 0, 3)",
        rusqlite::params![bad_state],
    )?;
    Ok(())
}

fn write_dirty_file_queue_item(home: &Path) -> Result<PathBuf> {
    let pending = home
        .join("db")
        .join("session_log")
        .join("message_queue")
        .join("pending");
    std::fs::create_dir_all(&pending)?;
    let path = pending.join("00000000000000000001-1-00000000000000000001.json");
    std::fs::write(&path, "{not-json")?;
    Ok(path)
}

fn pending_sqlite_queue_count(home: &Path) -> Result<i64> {
    let index_db = home.join("db").join("session_log").join("index.sqlite3");
    let conn = rusqlite::Connection::open(&index_db)?;
    conn.query_row(
        "SELECT COUNT(*) FROM session_write_queue WHERE status = 'pending'",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn assert_failed_file_queue_contains(home: &Path, dirty_file: &Path) -> Result<()> {
    let failed = home
        .join("db")
        .join("session_log")
        .join("message_queue")
        .join("failed");
    let file_name = dirty_file
        .file_name()
        .ok_or_else(|| anyhow!("dirty file missing name"))?;
    let failed_json = failed.join(file_name);
    let failed_error = failed_json.with_extension("error.txt");
    assert!(
        failed_json.exists(),
        "dirty file queue item should be retained in failed"
    );
    assert!(
        failed_error.exists(),
        "dirty file queue item should have an error sidecar"
    );
    Ok(())
}

fn upsert_request(session_id: &str, workspace: &str, tick: i64) -> UpsertSessionRequest {
    UpsertSessionRequest {
        session: json!({
            "id": session_id,
            "name": format!("Dirty Queue {session_id}"),
            "directory": workspace,
            "created_at": tick,
            "updated_at": tick,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": format!("Dirty Queue {session_id}"),
                "state": "created"
            }
        }),
        parent_id: None,
        messages: vec![json!({
            "id": format!("message-{session_id}"),
            "role": "assistant",
            "created_at": tick,
            "updated_at": tick,
            "content": format!("content for {session_id}")
        })],
        todos: Vec::new(),
    }
}

fn session_visible(session_id: &str) -> Result<bool> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => Ok(session.is_some()),
        SessionLogResponse::Error { error } => bail!("get session returned error: {error}"),
        other => bail!("unexpected get session response: {other:?}"),
    }
}

fn assert_ok(response: SessionLogResponse, context: &str) -> Result<()> {
    match response {
        SessionLogResponse::Ok => Ok(()),
        SessionLogResponse::Error { error } => bail!("{context} returned error: {error}"),
        other => bail!("{context} returned unexpected response: {other:?}"),
    }
}

fn call_service_with_retry(
    command: &SessionLogCommand,
    timeout: Duration,
) -> Result<SessionLogResponse> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match session_log::ipc::call_service(command) {
            Ok(response) => return Ok(response),
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("session_db service call did not run")))
}

struct SessionDbGuard {
    child: Option<Child>,
}

impl SessionDbGuard {
    fn start(repo: &Path, home: &Path, workspace: &Path) -> Result<Self> {
        let stdout = process_log_file(home, "session-db-dirty-queue.stdout.log")?;
        let stderr = process_log_file(home, "session-db-dirty-queue.stderr.log")?;
        let child = Command::new(session_db_bin(repo))
            .current_dir(workspace)
            .env("TURA_HOME", home)
            .env("TURA_PROJECT_ROOT", repo)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .context("spawn tura_session_db")?;
        Ok(Self { child: Some(child) })
    }

    fn wait(&mut self, timeout: Duration) -> Result<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        let started = Instant::now();
        while started.elapsed() < timeout {
            if child.try_wait()?.is_some() {
                self.child.take();
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        bail!("session_db did not exit within {}ms", timeout.as_millis())
    }
}

impl Drop for SessionDbGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    bail!("timed out after {}ms", timeout.as_millis())
}

fn ensure_session_db_binary(repo: &Path) -> Result<()> {
    if session_db_bin(repo).exists() {
        return Ok(());
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(repo)
        .args(["build", "-p", "session_log", "--bin", "tura_session_db"])
        .status()
        .context("build session_log::tura_session_db")?;
    if !status.success() {
        bail!("cargo build -p session_log --bin tura_session_db failed with {status}");
    }
    Ok(())
}

fn session_db_bin(repo: &Path) -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_tura_session_db")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            target_dir(repo)
                .join("debug")
                .join(exe_name("tura_session_db"))
        })
}

fn target_dir(repo: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target"))
}

fn exe_name(binary: &str) -> String {
    if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

fn process_log_file(home: &Path, name: &str) -> Result<std::fs::File> {
    let dir = home.join("logs");
    std::fs::create_dir_all(&dir)?;
    std::fs::File::create(dir.join(name)).map_err(Into::into)
}

fn repo_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("session_log crate is not under workspace/crates"))
}

fn temp_root(prefix: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        unique_nonce()?
    ));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn unique_nonce() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before UNIX_EPOCH")?
        .as_nanos())
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<OsString>)>,
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
