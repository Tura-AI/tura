//! Gateway/router/session_db lifecycle E2E coverage.
//!
//! This is a local stability test: it starts the real gateway binary, verifies
//! that it owns one router/session_db pair, proves a second gateway cannot take
//! the same home, then performs a router shutdown and checks endpoint cleanup.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[test]
fn gateway_router_session_db_conflict_and_shutdown_e2e() -> Result<()> {
    let repo = repo_root();
    ensure_backend_binary(&repo, "router", "tura_router")?;
    ensure_backend_binary(&repo, "session_log", "tura_session_db")?;

    let root = temp_root("gateway-lifecycle-e2e")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let port = free_port()?;
    let gateway_url = format!("http://127.0.0.1:{port}");
    let mut gateway = GatewayGuard::start(&repo, &home, &workspace, port)?;
    wait_for_http_ok(port, "/global/health", Duration::from_secs(30))?;
    wait_for_endpoint(&router_addr_path(&home), Duration::from_secs(30))?;
    wait_for_endpoint(&service_addr_path(&home), Duration::from_secs(30))?;

    let service_status = http_json(port, "/service/status")?;
    assert_eq!(
        service_status["router"]["status"], "running",
        "router should report running after gateway startup: {service_status}"
    );

    let conflict = spawn_conflicting_gateway(&repo, &home, &workspace, port)?;
    assert!(
        !conflict.status.success(),
        "second gateway with same TURA_HOME should fail, stdout={}, stderr={}",
        conflict.stdout,
        conflict.stderr
    );
    assert!(
        conflict
            .stderr
            .contains("gateway ownership lock refused startup"),
        "conflict stderr should explain ownership lock refusal, got: {}",
        conflict.stderr
    );

    let shutdown = shutdown_router(&home)?;
    assert!(
        shutdown["ok"].as_bool().unwrap_or(false),
        "shutdown failed: {shutdown}"
    );
    assert_eq!(shutdown["payload"]["status"], "shutting_down");
    wait_for_missing(&router_addr_path(&home), Duration::from_secs(10))?;
    wait_for_missing(&service_addr_path(&home), Duration::from_secs(10))?;

    assert!(
        http_get(port, "/global/health", Duration::from_secs(2))?.starts_with("HTTP/1.1 200"),
        "gateway should stay alive until its owner process is explicitly stopped"
    );

    gateway.stop()?;
    assert!(
        !router_endpoint_reachable(&home),
        "router endpoint must be unreachable after graceful shutdown"
    );
    assert!(
        !session_db_endpoint_reachable(&home),
        "session_db endpoint must be unreachable after graceful shutdown"
    );
    assert!(
        !router_addr_path(&home).exists() && !service_addr_path(&home).exists(),
        "shutdown must remove endpoint files under {gateway_url}"
    );
    Ok(())
}

struct GatewayGuard {
    child: Option<Child>,
    home: PathBuf,
}

impl GatewayGuard {
    fn start(repo: &Path, home: &Path, workspace: &Path, port: u16) -> Result<Self> {
        let child = Command::new(gateway_bin())
            .current_dir(workspace)
            .envs(gateway_env(repo, home, workspace, port))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawn tura_gateway")?;
        Ok(Self {
            child: Some(child),
            home: home.to_path_buf(),
        })
    }

    fn stop(&mut self) -> Result<()> {
        let _ = shutdown_router(&self.home);
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill().context("kill gateway")?;
            }
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for GatewayGuard {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct CommandOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn spawn_conflicting_gateway(
    repo: &Path,
    home: &Path,
    workspace: &Path,
    existing_port: u16,
) -> Result<CommandOutput> {
    let conflict_port = loop {
        let candidate = free_port()?;
        if candidate != existing_port {
            break candidate;
        }
    };
    let mut child = Command::new(gateway_bin())
        .current_dir(workspace)
        .envs(gateway_env(repo, home, workspace, conflict_port))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn conflicting tura_gateway")?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(10) {
            child.kill().context("kill hung conflicting gateway")?;
            bail!("conflicting gateway did not exit within 10s");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_string(&mut stdout);
    }
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

fn gateway_env(repo: &Path, home: &Path, workspace: &Path, port: u16) -> Vec<(String, String)> {
    vec![
        ("PORT".to_string(), port.to_string()),
        ("TURA_GATEWAY_PORT".to_string(), port.to_string()),
        (
            "TURA_GATEWAY_URL".to_string(),
            format!("http://127.0.0.1:{port}"),
        ),
        ("TURA_HOME".to_string(), home.display().to_string()),
        ("TURA_PROJECT_ROOT".to_string(), repo.display().to_string()),
        ("TURA_CWD".to_string(), workspace.display().to_string()),
        (
            "TURA_PROVIDER_CONFIG".to_string(),
            repo.join("crates")
                .join("provider")
                .join("config")
                .join("provider_config.json")
                .display()
                .to_string(),
        ),
    ]
}

fn shutdown_router(home: &Path) -> Result<serde_json::Value> {
    let endpoint = read_endpoint(&router_addr_path(home))?;
    let addr = endpoint
        .get("addr")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("router endpoint missing addr: {endpoint}"))?;
    call_jsonl(
        addr,
        &json!({
            "request_id": "process-lifecycle-shutdown",
            "kind": "call",
            "method": "execution.shutdown",
            "payload": {}
        }),
    )
}

fn call_jsonl(addr: &str, payload: &serde_json::Value) -> Result<serde_json::Value> {
    let socket: SocketAddr = addr
        .parse()
        .with_context(|| format!("invalid router address {addr}"))?;
    let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))
        .with_context(|| format!("connect router at {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(serde_json::to_string(payload)?.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.trim().is_empty() {
        bail!("router closed without shutdown response");
    }
    serde_json::from_str(line.trim()).context("parse router shutdown response")
}

fn router_endpoint_reachable(home: &Path) -> bool {
    endpoint_reachable(&router_addr_path(home))
}

fn session_db_endpoint_reachable(home: &Path) -> bool {
    endpoint_reachable(&service_addr_path(home))
}

fn endpoint_reachable(path: &Path) -> bool {
    let Ok(endpoint) = read_endpoint(path) else {
        return false;
    };
    let Some(addr) = endpoint.get("addr").and_then(serde_json::Value::as_str) else {
        return false;
    };
    let Ok(socket) = addr.parse::<SocketAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&socket, Duration::from_millis(200)).is_ok()
}

fn read_endpoint(path: &Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read endpoint {}", path.display()))?;
    serde_json::from_str(raw.trim()).with_context(|| format!("parse endpoint {}", path.display()))
}

fn wait_for_endpoint(path: &Path, timeout: Duration) -> Result<()> {
    wait_until(timeout, || path.exists())
        .with_context(|| format!("wait for endpoint {}", path.display()))
}

fn wait_for_missing(path: &Path, timeout: Duration) -> Result<()> {
    wait_until(timeout, || !path.exists())
        .with_context(|| format!("wait for endpoint cleanup {}", path.display()))
}

fn wait_for_http_ok(port: u16, path: &str, timeout: Duration) -> Result<()> {
    wait_until(timeout, || {
        http_get(port, path, Duration::from_secs(1))
            .map(|response| response.starts_with("HTTP/1.1 200"))
            .unwrap_or(false)
    })
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

fn http_json(port: u16, path: &str) -> Result<serde_json::Value> {
    let response = http_get(port, path, Duration::from_secs(5))?;
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .ok_or_else(|| anyhow!("HTTP response missing body: {response}"))?;
    serde_json::from_str(body.trim()).with_context(|| format!("parse HTTP body for {path}"))
}

fn http_get(port: u16, path: &str, timeout: Duration) -> Result<String> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .with_context(|| format!("connect gateway on port {port}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes())?;
    stream.flush()?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn free_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn router_addr_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("router.addr")
}

fn service_addr_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("service.addr")
}

fn temp_root(prefix: &str) -> Result<PathBuf> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn gateway_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tura_gateway"))
}

fn ensure_backend_binary(repo: &Path, package: &str, bin: &str) -> Result<()> {
    let executable = if cfg!(windows) {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    };
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target"));
    let candidate = target_dir.join("debug").join(&executable);
    if candidate.exists() {
        return Ok(());
    }
    let status = Command::new("cargo")
        .current_dir(repo)
        .args(["build", "-p", package, "--bin", bin])
        .status()
        .with_context(|| format!("build {package}::{bin}"))?;
    if !status.success() {
        bail!("cargo build -p {package} --bin {bin} failed with {status}");
    }
    if candidate.exists() {
        Ok(())
    } else {
        bail!(
            "expected backend binary not found after build: {}",
            candidate.display()
        )
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("gateway crate should live under crates/gateway")
        .to_path_buf()
}
