//! Business E2E: a crashed router must leave the session_db owner usable, and
//! the next router for the same home must reuse that live owner instead of
//! spawning or killing a replacement.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use session_log::file_queue;
use session_log_contract::{
    GetSessionRequest, SessionLogCommand, SessionLogResponse, UpsertSessionRequest,
};
use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn router_crash_leaves_session_db_alive_and_next_router_adopts_it() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let repo = repo_root()?;
    cleanup_target_backend_processes(&repo, Duration::from_secs(10))?;
    let _cleanup = TargetBackendCleanup { repo: repo.clone() };
    ensure_backend_binaries(&repo)?;

    let root = temp_root("session-db-router-adoption")?;
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
        ("TURA_ROUTER_IDLE_SHUTDOWN_SECS", Some(Path::new("120"))),
    ]);

    let mut first_router = RouterGuard::start(&repo, &home, &workspace)?;
    let first_router_addr =
        wait_for_router_addr(&home, Duration::from_secs(30)).context("first router startup")?;
    wait_until(
        Duration::from_secs(30),
        session_log::ipc::service_is_running,
    )
    .context("session_db did not start under first router")?;
    let first_service_addr = read_endpoint_addr(&service_addr_path(&home))?;
    let first_status = router_call(&first_router_addr, "session_db.lifecycle.status", json!({}))
        .context("first router session_db status")?;
    assert_eq!(first_status["payload"]["status"], "running");
    assert!(
        first_status["payload"]["pid"].as_u64().is_some(),
        "first router should own the session_db child handle: {first_status}"
    );

    first_router.crash()?;
    wait_for_router_unreachable(&first_router_addr, Duration::from_secs(10))
        .context("killed router socket should stop accepting")?;
    assert!(
        session_log::ipc::service_is_running(),
        "session_db must remain alive after router crash"
    );

    let queued_while_router_dead = "queued-while-router-dead";
    file_queue::enqueue_command(&SessionLogCommand::UpsertSession(upsert_request(
        queued_while_router_dead,
        &workspace_key,
        10,
    )))?;
    wait_until(Duration::from_secs(10), || {
        session_visible(queued_while_router_dead).unwrap_or(false)
    })
    .context("live session_db did not drain queued write after router crash")?;

    let direct_while_router_dead = "direct-while-router-dead";
    assert_ok(
        session_log::ipc::call_service(&SessionLogCommand::UpsertSession(upsert_request(
            direct_while_router_dead,
            &workspace_key,
            20,
        )))?,
        "direct write while router is dead",
    )?;

    let mut second_router = RouterGuard::start(&repo, &home, &workspace)?;
    let second_router_addr =
        wait_for_router_addr(&home, Duration::from_secs(30)).context("second router startup")?;
    let second_service_addr = read_endpoint_addr(&service_addr_path(&home))?;
    assert_eq!(
        first_service_addr, second_service_addr,
        "new router must reuse the already-live session_db endpoint"
    );
    let second_status = router_call(
        &second_router_addr,
        "session_db.lifecycle.status",
        json!({}),
    )
    .context("second router session_db status")?;
    assert_eq!(second_status["payload"]["status"], "running");
    assert!(
        second_status["payload"]["pid"].is_null(),
        "adopted live session_db has no child handle in the new router: {second_status}"
    );
    assert!(
        session_visible(queued_while_router_dead)? && session_visible(direct_while_router_dead)?,
        "sessions written while router was down must remain visible after adoption"
    );

    let shutdown = router_call(&second_router_addr, "execution.shutdown", json!({}))
        .context("shutdown adopted router")?;
    assert_eq!(shutdown["payload"]["status"], "shutting_down");
    second_router.wait(Duration::from_secs(10))?;
    wait_until(Duration::from_secs(10), || {
        !session_log::ipc::service_is_running() && !service_addr_path(&home).exists()
    })
    .context("adopted session_db did not stop during router shutdown")?;
    wait_for_missing(&router_addr_path(&home), Duration::from_secs(10))?;

    Ok(())
}

fn upsert_request(session_id: &str, workspace: &str, tick: i64) -> UpsertSessionRequest {
    UpsertSessionRequest {
        session: json!({
            "id": session_id,
            "name": format!("Router Adoption {session_id}"),
            "directory": workspace,
            "created_at": tick,
            "updated_at": tick,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": format!("Router Adoption {session_id}"),
                "state": "created",
                "current_turn": 0
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

struct RouterGuard {
    child: Option<Child>,
}

impl RouterGuard {
    fn start(repo: &Path, home: &Path, workspace: &Path) -> Result<Self> {
        let stdout = process_log_file(home, "router-adoption.stdout.log")?;
        let stderr = process_log_file(home, "router-adoption.stderr.log")?;
        let child = Command::new(debug_bin(repo, "tura_router"))
            .arg("serve-socket")
            .current_dir(workspace)
            .env("TURA_HOME", home)
            .env("TURA_PROJECT_ROOT", repo)
            .env("TURA_ROUTER_IDLE_SHUTDOWN_SECS", "120")
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .context("spawn tura_router serve-socket")?;
        Ok(Self { child: Some(child) })
    }

    fn crash(&mut self) -> Result<()> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        child.kill().context("kill router")?;
        let status = child.wait().context("wait killed router")?;
        assert!(
            !status.success(),
            "crashed router should not report successful exit: {status}"
        );
        Ok(())
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
        bail!("router did not exit within {}ms", timeout.as_millis())
    }
}

impl Drop for RouterGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn router_call(addr: &str, method: &str, payload: Value) -> Result<Value> {
    let socket: SocketAddr = addr.parse().context("parse router addr")?;
    let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))
        .with_context(|| format!("connect router at {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let request = json!({
        "request_id": format!("session-db-adoption-{}", unique_nonce()?),
        "kind": if method == "health_check" { "health_check" } else { "call" },
        "method": method,
        "payload": payload,
    });
    stream.write_all(serde_json::to_string(&request)?.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read = reader.read_line(&mut line)?;
    if read == 0 || line.trim().is_empty() {
        bail!("router at {addr} closed without response to {method}");
    }
    let response: Value = serde_json::from_str(line.trim())?;
    if response["ok"].as_bool() != Some(true) {
        bail!("router {method} failed: {response}");
    }
    Ok(response)
}

fn wait_for_router_addr(home: &Path, timeout: Duration) -> Result<String> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match read_endpoint_addr(&router_addr_path(home))
            .and_then(|addr| router_call(&addr, "health_check", json!({})).map(|_| addr))
        {
            Ok(addr) => return Ok(addr),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("router addr did not become reachable")))
}

fn wait_for_router_unreachable(addr: &str, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if router_call(addr, "health_check", json!({})).is_err() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("router at {addr} remained reachable")
}

fn wait_for_missing(path: &Path, timeout: Duration) -> Result<()> {
    wait_until(timeout, || !path.exists())
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

struct TargetBackendCleanup {
    repo: PathBuf,
}

impl Drop for TargetBackendCleanup {
    fn drop(&mut self) {
        let _ = cleanup_target_backend_processes(&self.repo, Duration::from_secs(10));
    }
}

fn cleanup_target_backend_processes(repo: &Path, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    loop {
        let pids = target_backend_process_pids(repo)?;
        if pids.is_empty() {
            return Ok(());
        }
        for pid in pids {
            if pid == std::process::id() {
                continue;
            }
            terminate_process_quietly(pid);
        }
        if started.elapsed() >= timeout {
            bail!(
                "target backend processes remained after cleanup timeout: {:?}",
                target_backend_process_pids(repo)?
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn target_backend_process_pids(repo: &Path) -> Result<Vec<u32>> {
    let target = canonical_or_self(&target_dir(repo));
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    let mut pids = Vec::new();
    for (pid, process) in system.processes() {
        if ![
            "tura_router",
            "tura_session_db",
            "tura_gateway",
            "tura_runtime",
        ]
        .iter()
        .any(|name| process_name_matches(process.name(), name))
        {
            continue;
        }
        let Some(exe) = process.exe() else {
            continue;
        };
        if canonical_or_self(exe).starts_with(&target) {
            pids.push(pid.as_u32());
        }
    }
    pids.sort_unstable();
    Ok(pids)
}

fn terminate_process_quietly(pid: u32) {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    if let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) {
        let _ = process.kill();
    }
}

fn process_name_matches(name: &str, binary: &str) -> bool {
    let normalize = |value: &str| value.trim().trim_end_matches(".exe").to_ascii_lowercase();
    normalize(name) == normalize(binary)
}

fn canonical_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn read_endpoint_addr(path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read endpoint {}", path.display()))?;
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return value
            .get("addr")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow!("endpoint {} has no addr field", path.display()));
    }
    Ok(trimmed.to_string())
}

fn ensure_backend_binaries(repo: &Path) -> Result<()> {
    for (package, binary) in [
        ("session_log", "tura_session_db"),
        ("router", "tura_router"),
    ] {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .current_dir(repo)
            .args(["build", "-p", package, "--bin", binary])
            .status()
            .with_context(|| format!("build {package}::{binary}"))?;
        if !status.success() {
            bail!("cargo build -p {package} --bin {binary} failed with {status}");
        }
    }
    Ok(())
}

fn debug_bin(repo: &Path, binary: &str) -> PathBuf {
    target_dir(repo).join("debug").join(exe_name(binary))
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

fn router_addr_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("router.addr")
}

fn service_addr_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("service.addr")
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
