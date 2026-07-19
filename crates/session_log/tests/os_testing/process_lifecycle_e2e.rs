//! Required session_db process lifecycle E2E tests.
//!
//! These are intentionally outside `tests/benchmark`: they prove the mandatory
//! single-owner, graceful shutdown, bad-input, and idempotent write rules for
//! the session_log crate itself.

use anyhow::{anyhow, bail, Context, Result};
use session_log_contract::{
    GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest, SessionLogCommand,
    SessionLogResponse, UpsertSessionRequest,
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Arc, Barrier},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SESSION_DB_BIN: &str = env!("CARGO_BIN_EXE_tura_session_db");

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn session_db_single_owner_bad_input_idempotent_upsert_and_shutdown() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let root = temp_root("session-db-lifecycle")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::set_home(&home);

    let mut service = ServiceGuard::start(&home)?;
    wait_until(
        Duration::from_secs(30),
        session_log::ipc::service_is_running,
    )?;
    let initial_endpoint = std::fs::read_to_string(service_addr_path(&home))?;

    let conflict = spawn_conflicting_service(&home)?;
    assert!(
        !conflict.status.success(),
        "second session_db owner should fail, stdout={}, stderr={}",
        conflict.stdout,
        conflict.stderr
    );
    assert!(
        conflict
            .stderr
            .contains("another session_db owner already owns"),
        "conflict stderr should explain owner lock refusal, got: {}",
        conflict.stderr
    );
    assert_eq!(
        std::fs::read_to_string(service_addr_path(&home))?,
        initial_endpoint,
        "conflicting session_db must not replace the active endpoint"
    );

    let bad = write_raw_request(&home, b"{not-json}\n")?;
    assert!(
        bad.contains("invalid session_db request"),
        "bad input should return a structured error, got: {bad}"
    );
    assert!(
        matches!(
            session_log::ipc::call_service(&SessionLogCommand::ListWorkspaces)?,
            SessionLogResponse::Workspaces { .. }
        ),
        "service should remain usable after a bad request"
    );

    let session_id = format!("lifecycle-{}", unique_nonce()?);
    let workspace = workspace.to_string_lossy().replace('\\', "/");
    let upsert = SessionLogCommand::UpsertSession(upsert_request(&session_id, &workspace));
    assert!(matches!(
        session_log::ipc::call_service(&upsert)?,
        SessionLogResponse::Ok
    ));
    assert!(matches!(
        session_log::ipc::call_service(&upsert)?,
        SessionLogResponse::Ok
    ));

    match session_log::ipc::call_service(&SessionLogCommand::ListSessions(ListSessionsRequest {
        workspace,
        page: 0,
        page_size: 50,
    }))? {
        SessionLogResponse::Sessions { page, sessions } => {
            assert_eq!(
                page.total, 1,
                "idempotent upsert must not duplicate sessions"
            );
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].session_id, session_id);
        }
        other => bail!("unexpected list sessions response: {other:?}"),
    }

    match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.clone(),
            page: 0,
            page_size: 50,
        },
    ))? {
        SessionLogResponse::Records { page, records } => {
            assert_eq!(
                page.total, 1,
                "idempotent upsert must not duplicate records"
            );
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].message_id, format!("m-{session_id}"));
        }
        other => bail!("unexpected list records response: {other:?}"),
    }

    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.clone(),
    }))? {
        SessionLogResponse::Session { session } => {
            let session = session.ok_or_else(|| anyhow!("expected session {session_id}"))?;
            assert_eq!(session.session_id, session_id);
            assert_eq!(session.message_count, 1);
        }
        other => bail!("unexpected get response: {other:?}"),
    }

    assert!(matches!(
        session_log::ipc::call_service(&SessionLogCommand::Shutdown)?,
        SessionLogResponse::Ok
    ));
    service.wait_for_exit(Duration::from_secs(10))?;
    wait_until(Duration::from_secs(10), || {
        !service_addr_path(&home).exists()
    })?;
    assert!(!session_log::ipc::service_is_running());
    assert!(
        !lock_path(&home).exists(),
        "session_db owner lock should be released on graceful exit"
    );
    Ok(())
}

#[test]
fn session_db_restart_after_crash_marks_running_sessions_interrupted_and_keeps_history(
) -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let root = temp_root("session-db-crash-restart")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::set_home(&home);

    let mut first = ServiceGuard::start(&home)?;
    wait_until(
        Duration::from_secs(30),
        session_log::ipc::service_is_running,
    )?;
    let first_endpoint = std::fs::read_to_string(service_addr_path(&home))?;

    let session_id = format!("crash-restart-{}", unique_nonce()?);
    let workspace = workspace.to_string_lossy().replace('\\', "/");
    assert!(matches!(
        session_log::ipc::call_service(&SessionLogCommand::UpsertSession(running_upsert_request(
            &session_id,
            &workspace,
            "before crash"
        )))?,
        SessionLogResponse::Ok
    ));

    first.kill_and_wait(Duration::from_secs(10))?;
    assert!(
        !session_log::ipc::service_is_running(),
        "crashed endpoint should be detected as stale and removed"
    );

    let mut second = ServiceGuard::start(&home)?;
    wait_until(
        Duration::from_secs(30),
        session_log::ipc::service_is_running,
    )?;
    assert_ne!(
        std::fs::read_to_string(service_addr_path(&home))?,
        first_endpoint,
        "restart should publish a fresh endpoint"
    );

    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.clone(),
    }))? {
        SessionLogResponse::Session { session } => {
            let session = session.ok_or_else(|| anyhow!("expected recovered session"))?;
            assert_eq!(session.session_id, session_id);
            assert_eq!(session.state.as_deref(), Some("interrupted"));
            assert_eq!(session.status.as_deref(), Some("error"));
            assert_eq!(session.management["state"], "interrupted");
            assert_eq!(session.message_count, 1);
        }
        other => bail!("unexpected get response after restart: {other:?}"),
    }

    match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.clone(),
            page: 0,
            page_size: 50,
        },
    ))? {
        SessionLogResponse::Records { page, records } => {
            assert_eq!(page.total, 1);
            assert_eq!(records[0].message_id, format!("m-{session_id}"));
            assert_eq!(records[0].record["content"], "before crash");
        }
        other => bail!("unexpected records response after restart: {other:?}"),
    }

    assert!(matches!(
        session_log::ipc::call_service(&SessionLogCommand::Shutdown)?,
        SessionLogResponse::Ok
    ));
    second.wait_for_exit(Duration::from_secs(10))?;
    Ok(())
}

#[test]
fn session_db_accepts_concurrent_short_lived_clients_without_losing_records() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let root = temp_root("session-db-concurrent-clients")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::set_home(&home);
    let mut service = ServiceGuard::start(&home)?;
    wait_until(
        Duration::from_secs(30),
        session_log::ipc::service_is_running,
    )?;

    let workspace = workspace.to_string_lossy().replace('\\', "/");
    let barrier = Arc::new(Barrier::new(8));
    let mut handles = Vec::new();
    for index in 0..8 {
        let barrier = Arc::clone(&barrier);
        let workspace = workspace.clone();
        handles.push(std::thread::spawn(move || -> Result<String> {
            let session_id = format!("concurrent-client-{index}-{}", unique_nonce()?);
            barrier.wait();
            let response = session_log::ipc::call_service(&SessionLogCommand::UpsertSession(
                upsert_request(&session_id, &workspace),
            ))?;
            if !matches!(response, SessionLogResponse::Ok) {
                bail!("unexpected concurrent upsert response: {response:?}");
            }
            Ok(session_id)
        }));
    }
    let mut session_ids = Vec::new();
    for handle in handles {
        session_ids.push(
            handle
                .join()
                .map_err(|_| anyhow!("client thread panicked"))??,
        );
    }
    session_ids.sort();

    match session_log::ipc::call_service(&SessionLogCommand::ListSessions(ListSessionsRequest {
        workspace,
        page: 0,
        page_size: 50,
    }))? {
        SessionLogResponse::Sessions { page, sessions } => {
            assert_eq!(page.total, 8);
            let mut listed = sessions
                .into_iter()
                .map(|session| session.session_id)
                .collect::<Vec<_>>();
            listed.sort();
            assert_eq!(listed, session_ids);
        }
        other => bail!("unexpected list sessions response: {other:?}"),
    }

    for session_id in session_ids {
        match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
            ListSessionRecordsRequest {
                session_id: session_id.clone(),
                page: 0,
                page_size: 10,
            },
        ))? {
            SessionLogResponse::Records { page, records } => {
                assert_eq!(page.total, 1);
                assert_eq!(records[0].message_id, format!("m-{session_id}"));
            }
            other => bail!("unexpected records response for {session_id}: {other:?}"),
        }
    }

    assert!(matches!(
        session_log::ipc::call_service(&SessionLogCommand::Shutdown)?,
        SessionLogResponse::Ok
    ));
    service.wait_for_exit(Duration::from_secs(10))?;
    Ok(())
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn set_home(home: &Path) -> Self {
        let keys = ["TURA_HOME", "SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
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

struct ServiceGuard {
    child: Option<Child>,
}

impl ServiceGuard {
    fn start(home: &Path) -> Result<Self> {
        let child = Command::new(SESSION_DB_BIN)
            .env("TURA_HOME", home)
            .env_remove("SESSION_LOG_DB_ROOT")
            .env_remove("TURA_DB_ROOT")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawn tura_session_db")?;
        Ok(Self { child: Some(child) })
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Result<()> {
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| anyhow!("service child already taken"))?;
        let started = Instant::now();
        while started.elapsed() < timeout {
            if let Some(status) = child.try_wait()? {
                if status.success() {
                    return Ok(());
                }
                bail!("session_db exited with {status}");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        bail!("session_db did not exit within {}ms", timeout.as_millis())
    }

    fn kill_and_wait(&mut self, timeout: Duration) -> Result<()> {
        let mut child = self
            .child
            .take()
            .ok_or_else(|| anyhow!("service child already taken"))?;
        child.kill().context("kill session_db")?;
        let started = Instant::now();
        while started.elapsed() < timeout {
            if let Some(status) = child.try_wait()? {
                if status.success() {
                    bail!("killed session_db exited successfully unexpectedly");
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        bail!(
            "killed session_db did not exit within {}ms",
            timeout.as_millis()
        )
    }
}

impl Drop for ServiceGuard {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        if let Some(mut child) = self.child.take() {
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(5) {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct CommandOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn spawn_conflicting_service(home: &Path) -> Result<CommandOutput> {
    let mut child = Command::new(SESSION_DB_BIN)
        .env("TURA_HOME", home)
        .env_remove("SESSION_LOG_DB_ROOT")
        .env_remove("TURA_DB_ROOT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn conflicting tura_session_db")?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(10) {
            child
                .kill()
                .context("kill hung conflicting tura_session_db")?;
            bail!("conflicting tura_session_db did not exit within 10s");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stdout = read_pipe(child.stdout.take());
    let stderr = read_pipe(child.stderr.take());
    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

fn read_pipe(pipe: Option<impl Read>) -> String {
    let Some(mut pipe) = pipe else {
        return String::new();
    };
    let mut output = String::new();
    let _ = pipe.read_to_string(&mut output);
    output
}

fn write_raw_request(home: &Path, raw: &[u8]) -> Result<String> {
    let endpoint = read_endpoint(home)?;
    let addr = endpoint
        .get("addr")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("service endpoint missing addr: {endpoint}"))?;
    let socket: SocketAddr = addr.parse()?;
    let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(raw)?;
    stream.flush()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    Ok(line)
}

fn read_endpoint(home: &Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(service_addr_path(home))?;
    serde_json::from_str(raw.trim()).context("parse service endpoint")
}

fn upsert_request(session_id: &str, workspace: &str) -> UpsertSessionRequest {
    UpsertSessionRequest {
        session: serde_json::json!({
            "id": session_id,
            "name": "Lifecycle",
            "directory": workspace,
            "created_at": 1,
            "updated_at": 2,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": "Lifecycle",
                "state": "created"
            }
        }),
        parent_id: None,
        messages: vec![serde_json::json!({
            "id": format!("m-{session_id}"),
            "role": "assistant",
            "created_at": 1,
            "updated_at": 1
        })],
        todos: vec![],
    }
}

fn running_upsert_request(
    session_id: &str,
    workspace: &str,
    content: &str,
) -> UpsertSessionRequest {
    UpsertSessionRequest {
        session: serde_json::json!({
            "id": session_id,
            "name": "Running Before Crash",
            "directory": workspace,
            "created_at": 1,
            "updated_at": 2,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": "Running Before Crash",
                "state": "running"
            }
        }),
        parent_id: None,
        messages: vec![serde_json::json!({
            "id": format!("m-{session_id}"),
            "role": "assistant",
            "created_at": 1,
            "updated_at": 1,
            "content": content
        })],
        todos: vec![],
    }
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("timed out after {}ms", timeout.as_millis())
}

fn service_addr_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("service.addr")
}

fn lock_path(home: &Path) -> PathBuf {
    home.join(".tura")
        .join("locks")
        .join(format!("session-db-{}.lock", tura_path::build_kind()))
}

fn temp_root(prefix: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("{}-{}", prefix, unique_nonce()?));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn unique_nonce() -> Result<String> {
    Ok(format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ))
}
