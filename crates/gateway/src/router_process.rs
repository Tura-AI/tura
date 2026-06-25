//! Gateway client for the per-home detached router daemon.
//!
//! Gateway is a front door, not the backend owner. It probes the endpoint
//! published by `tura_router serve-socket`, starts that daemon detached when no
//! compatible daemon is reachable, and sends one request per socket connection.

use anyhow::{anyhow, Context, Result};
use once_cell::sync::OnceCell;
use parking_lot::Mutex as ParkingMutex;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const ROUTER_HEALTH_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_ROUTER_EXECUTION_TIMEOUT: Duration = Duration::from_secs(35 * 60);
const ROUTER_PROBE_CONNECT_TIMEOUT: Duration = Duration::from_millis(100);
const ROUTER_STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, serde::Serialize)]
pub struct RouterProcessStatus {
    pub status: String,
    pub pid: Option<u32>,
    pub process_start_time: Option<u64>,
    pub restart_count: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessLockRecord {
    pid: Option<u32>,
    process_start_time: Option<u64>,
    kind: Option<String>,
    build_kind: Option<String>,
    home: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouterEndpoint {
    addr: String,
    pid: Option<u32>,
    process_start_time: Option<u64>,
}

pub struct RouterProcess {
    router_bin: Option<PathBuf>,
    addr: ParkingMutex<Option<String>>,
    request_seq: AtomicU64,
    restart_count: AtomicU64,
    last_error: ParkingMutex<Option<String>>,
}

static ROUTER_PROCESS: OnceCell<Arc<RouterProcess>> = OnceCell::new();

pub fn global_router_process() -> Result<Arc<RouterProcess>> {
    global_router_process_from(&ROUTER_PROCESS, RouterProcess::new)
}

fn global_router_process_from(
    cell: &OnceCell<Arc<RouterProcess>>,
    init: impl FnOnce() -> Result<RouterProcess>,
) -> Result<Arc<RouterProcess>> {
    if let Some(process) = cell.get() {
        return Ok(Arc::clone(process));
    }
    let process = Arc::new(init()?);
    if cell.set(Arc::clone(&process)).is_ok() {
        return Ok(process);
    }
    cell.get().map(Arc::clone).ok_or_else(|| {
        anyhow!("router daemon client initialization raced without storing a process")
    })
}

pub fn start_global_router_process() -> Result<Arc<RouterProcess>> {
    let process = global_router_process()?;
    process.ensure_started()?;
    Ok(process)
}

impl RouterProcess {
    pub fn new() -> Result<Self> {
        let root = repo_root()
            .ok_or_else(|| anyhow!("failed to locate project root for router process"))?;
        let router_bin = router_executable_candidates(&root)
            .into_iter()
            .find(|path| path.exists());
        Ok(Self {
            router_bin,
            addr: ParkingMutex::new(None),
            request_seq: AtomicU64::new(1),
            restart_count: AtomicU64::new(0),
            last_error: ParkingMutex::new(None),
        })
    }

    pub fn ensure_started(&self) -> Result<()> {
        if let Some((endpoint, _health)) = healthy_router_endpoint()? {
            *self.addr.lock() = Some(endpoint.addr);
            *self.last_error.lock() = None;
            return Ok(());
        }

        for attempt in 0..2 {
            let mut child = self.spawn_router_daemon()?;
            self.restart_count.fetch_add(1, Ordering::SeqCst);

            if let Some(endpoint) = wait_for_healthy_router(ROUTER_HEALTH_TIMEOUT)? {
                *self.addr.lock() = Some(endpoint.addr);
                *self.last_error.lock() = None;
                return Ok(());
            }

            let killed_by_lock = terminate_router_from_lock()?;
            let killed_spawned_child = kill_spawned_router_child(&mut child);
            let _ = std::fs::remove_file(router_addr_path());
            *self.addr.lock() = None;

            if !killed_by_lock && !killed_spawned_child {
                let error =
                    "router daemon did not become healthy within 20 seconds and could not be killed"
                        .to_string();
                *self.last_error.lock() = Some(error.clone());
                return Err(anyhow!("failed to start router daemon: {error}"));
            }

            if attempt == 0 {
                continue;
            }
        }

        let error = "router daemon did not become healthy within 20 seconds".to_string();
        *self.last_error.lock() = Some(error.clone());
        Err(anyhow!("failed to start router daemon: {error}"))
    }

    pub fn restart(&self) -> Result<()> {
        let _ = self.shutdown();
        *self.addr.lock() = None;
        self.ensure_started()
    }

    pub fn status(&self) -> RouterProcessStatus {
        match healthy_router_endpoint() {
            Ok(Some((endpoint, _health))) => {
                *self.addr.lock() = Some(endpoint.addr);
                RouterProcessStatus {
                    status: "running".to_string(),
                    pid: endpoint.pid,
                    process_start_time: endpoint.process_start_time,
                    restart_count: self.restart_count.load(Ordering::SeqCst),
                    error: self.last_error.lock().clone(),
                }
            }
            Ok(None) => match self.ensure_started() {
                Ok(()) => match healthy_router_endpoint() {
                    Ok(Some((endpoint, _health))) => {
                        *self.addr.lock() = Some(endpoint.addr);
                        RouterProcessStatus {
                            status: "running".to_string(),
                            pid: endpoint.pid,
                            process_start_time: endpoint.process_start_time,
                            restart_count: self.restart_count.load(Ordering::SeqCst),
                            error: self.last_error.lock().clone(),
                        }
                    }
                    Ok(None) => RouterProcessStatus {
                        status: "stopped".to_string(),
                        pid: None,
                        process_start_time: None,
                        restart_count: self.restart_count.load(Ordering::SeqCst),
                        error: self.last_error.lock().clone(),
                    },
                    Err(error) => RouterProcessStatus {
                        status: "unhealthy".to_string(),
                        pid: None,
                        process_start_time: None,
                        restart_count: self.restart_count.load(Ordering::SeqCst),
                        error: Some(error.to_string()),
                    },
                },
                Err(error) => RouterProcessStatus {
                    status: "stopped".to_string(),
                    pid: None,
                    process_start_time: None,
                    restart_count: self.restart_count.load(Ordering::SeqCst),
                    error: Some(error.to_string()),
                },
            },
            Err(error) => RouterProcessStatus {
                status: "unhealthy".to_string(),
                pid: None,
                process_start_time: None,
                restart_count: self.restart_count.load(Ordering::SeqCst),
                error: Some(error.to_string()),
            },
        }
    }

    pub fn shutdown(&self) -> Result<serde_json::Value> {
        let Some(endpoint) = reachable_router_endpoint()? else {
            *self.addr.lock() = None;
            return Ok(json!({
                "status": "stopped",
                "graceful": true,
                "forced": false,
                "pid": null,
                "process_start_time": null,
            }));
        };
        *self.addr.lock() = Some(endpoint.addr.clone());

        let mut graceful = false;
        let mut shutdown_payload = serde_json::Value::Null;
        match self.call_once("execution.shutdown", json!({})) {
            Ok(response)
                if response.get("ok").and_then(serde_json::Value::as_bool) == Some(true) =>
            {
                graceful = true;
                shutdown_payload = response
                    .get("payload")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
            }
            Ok(response) => {
                *self.last_error.lock() = Some(
                    response
                        .get("error")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("router shutdown failed")
                        .to_string(),
                );
            }
            Err(error) => {
                *self.last_error.lock() = Some(error.to_string());
            }
        }

        if wait_for_router_addr_unreachable(&endpoint.addr, Duration::from_secs(10)) {
            *self.addr.lock() = None;
            let _ = std::fs::remove_file(router_addr_path());
            return Ok(json!({
                "status": "stopped",
                "graceful": graceful,
                "forced": false,
                "pid": endpoint.pid,
                "process_start_time": endpoint.process_start_time,
                "shutdown": shutdown_payload,
            }));
        }

        let forced = terminate_router_endpoint_process(&endpoint)?;
        let stopped = if forced {
            wait_for_router_addr_unreachable(&endpoint.addr, Duration::from_secs(10))
        } else {
            false
        };
        if stopped {
            *self.addr.lock() = None;
            let _ = std::fs::remove_file(router_addr_path());
            Ok(json!({
                "status": "stopped",
                "graceful": graceful,
                "forced": true,
                "pid": endpoint.pid,
                "process_start_time": endpoint.process_start_time,
                "shutdown": shutdown_payload,
            }))
        } else {
            Err(anyhow!(
                "router daemon at {} did not stop{}",
                endpoint.addr,
                if endpoint.pid.is_some() {
                    ""
                } else {
                    " and has no verified pid for forced termination"
                }
            ))
        }
    }

    pub fn call(&self, method: &str, payload: serde_json::Value) -> Result<serde_json::Value> {
        self.ensure_started()?;
        let response = match self.call_once(method, payload.clone()) {
            Ok(response) => response,
            Err(first_error) => {
                *self.last_error.lock() = Some(first_error.to_string());
                *self.addr.lock() = None;
                self.ensure_started()?;
                self.call_once(method, payload)?
            }
        };

        if response
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Ok(response
                .get("payload")
                .cloned()
                .unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow!(
                "{}",
                response
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("router call failed")
            ))
        }
    }

    pub fn call_existing_with_timeout(
        &self,
        method: &str,
        payload: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value> {
        let endpoint = reachable_router_endpoint()?
            .ok_or_else(|| anyhow!("router daemon is not reachable"))?;
        *self.addr.lock() = Some(endpoint.addr);
        let response = self.call_once_with_timeout(method, payload, timeout)?;
        if response
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Ok(response
                .get("payload")
                .cloned()
                .unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow!(
                "{}",
                response
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("router call failed")
            ))
        }
    }

    fn call_once(&self, method: &str, payload: serde_json::Value) -> Result<serde_json::Value> {
        self.call_once_with_timeout(method, payload, read_timeout_for(method))
    }

    fn call_once_with_timeout(
        &self,
        method: &str,
        payload: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value> {
        let addr = self
            .addr
            .lock()
            .clone()
            .ok_or_else(|| anyhow!("router daemon address is unavailable"))?;
        let request_id = format!(
            "gateway-{}",
            self.request_seq.fetch_add(1, Ordering::SeqCst)
        );
        let deadline_ms: Option<u64> = if method == "health_check" {
            Some(ROUTER_HEALTH_TIMEOUT.as_millis() as u64)
        } else {
            None
        };
        let request = json!({
            "request_id": request_id,
            "kind": if method == "health_check" { "health_check" } else { "call" },
            "method": method,
            "payload": payload,
            "deadline_ms": deadline_ms,
        });
        call_router_addr(&addr, &request, timeout)
    }

    fn spawn_router_daemon(&self) -> Result<std::process::Child> {
        let router_bin = self
            .router_bin
            .as_ref()
            .ok_or_else(|| anyhow!("router binary not found in current exe/target"))?;
        let mut command = Command::new(router_bin);
        let project_root = std::env::var_os("TURA_PROJECT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(tura_path::canonical_root);
        command
            .arg("serve-socket")
            .env("TURA_HOME", tura_path::instance_home())
            .env("TURA_PROJECT_ROOT", project_root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide_child_window_and_detach(&mut command);
        command.spawn().with_context(|| {
            format!(
                "failed to spawn detached router daemon {}",
                router_bin.display()
            )
        })
    }
}

fn call_router_addr(
    addr: &str,
    request: &serde_json::Value,
    timeout: Duration,
) -> Result<serde_json::Value> {
    let socket: SocketAddr = addr
        .parse()
        .with_context(|| format!("invalid router daemon address {addr:?}"))?;
    let stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))
        .with_context(|| format!("failed to connect to router daemon at {addr}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut writer = stream.try_clone()?;
    writer.write_all(format!("{request}\n").as_bytes())?;
    writer.flush()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Err(anyhow!("router daemon closed without a response"));
        }
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(line.trim()).context("failed to decode router daemon response")?;
        if handle_router_notification(&value) {
            continue;
        }
        return Ok(value);
    }
}

fn handle_router_notification(value: &Value) -> bool {
    if value.get("kind").and_then(Value::as_str) != Some("gateway.callback") {
        return false;
    }
    if let Err(error) = apply_gateway_callback_notification(value) {
        tracing::warn!(error = %error, notification = %value, "failed to apply router gateway callback");
    }
    true
}

fn apply_gateway_callback_notification(value: &Value) -> Result<()> {
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("gateway callback notification missing method"))?;
    let payload = value
        .get("payload")
        .ok_or_else(|| anyhow!("gateway callback notification missing payload"))?;
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("gateway callback notification missing session_id"))?
        .to_string();
    let body = payload.get("body").cloned().unwrap_or(Value::Null);
    match method {
        "session.agent_message" => {
            let request: crate::api::session::SendAgentMessageRequest =
                serde_json::from_value(body)
                    .context("failed to decode session.agent_message callback body")?;
            let response = crate::api::session::send_agent_message_payload(session_id, request);
            if !response.ok {
                return Err(anyhow!(
                    "{}",
                    response
                        .error
                        .unwrap_or_else(|| "session.agent_message callback failed".to_string())
                ));
            }
        }
        "session.agent_stream" => {
            let request: crate::api::session::StreamAgentTextRequest = serde_json::from_value(body)
                .context("failed to decode session.agent_stream callback body")?;
            let response = crate::api::session::stream_agent_message_payload(session_id, request);
            if response
                .get("ok")
                .and_then(Value::as_bool)
                .is_some_and(|ok| !ok)
            {
                return Err(anyhow!("session.agent_stream callback failed: {response}"));
            }
        }
        "session.todos" => {
            let todos = serde_json::from_value::<Vec<Value>>(body)
                .context("failed to decode session.todos callback body")?;
            crate::session::session_store().set_todos(&session_id, todos);
        }
        other => {
            tracing::warn!(
                method = other,
                "dropping unknown gateway callback notification"
            );
        }
    }
    Ok(())
}

fn read_timeout_for(method: &str) -> Duration {
    if method == "health_check" {
        ROUTER_HEALTH_TIMEOUT
    } else if method == "execution.probe_sessions" {
        Duration::from_secs(5)
    } else if method == "execution.shutdown" {
        Duration::from_secs(10)
    } else {
        router_execution_timeout()
    }
}

fn router_execution_timeout() -> Duration {
    std::env::var("TURA_ROUTER_EXECUTION_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_ROUTER_EXECUTION_TIMEOUT)
}

fn router_addr_path() -> PathBuf {
    session_log::path::default_db_dir().join("router.addr")
}

#[cfg(test)]
fn reachable_router_addr() -> Result<Option<String>> {
    Ok(reachable_router_endpoint()?.map(|endpoint| endpoint.addr))
}

fn reachable_router_endpoint() -> Result<Option<RouterEndpoint>> {
    let Some(endpoint) = read_router_endpoint_record()? else {
        return Ok(None);
    };
    let path = router_addr_path();
    let socket: SocketAddr = match endpoint.addr.parse() {
        Ok(socket) => socket,
        Err(_) => {
            let _ = std::fs::remove_file(&path);
            return Ok(None);
        }
    };
    if TcpStream::connect_timeout(&socket, ROUTER_PROBE_CONNECT_TIMEOUT).is_ok() {
        Ok(Some(endpoint))
    } else {
        let _ = std::fs::remove_file(&path);
        Ok(None)
    }
}

fn read_router_endpoint_record() -> Result<Option<RouterEndpoint>> {
    let path = router_addr_path();
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    let endpoint = match parse_router_endpoint(raw.trim()) {
        Ok(Some(endpoint)) => endpoint,
        Ok(None) => {
            let _ = std::fs::remove_file(&path);
            return Ok(None);
        }
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(error).with_context(|| format!("invalid {}", path.display()));
        }
    };
    Ok(Some(endpoint))
}

fn healthy_router_endpoint() -> Result<Option<(RouterEndpoint, serde_json::Value)>> {
    let Some(mut endpoint) = read_router_endpoint_record()? else {
        return Ok(None);
    };
    let request = json!({
        "request_id": "gateway-health-probe",
        "kind": "health_check",
        "method": "health_check",
        "payload": {},
        "deadline_ms": ROUTER_HEALTH_TIMEOUT.as_millis() as u64,
    });
    let response =
        match call_router_addr(&endpoint.addr, &request, read_timeout_for("health_check")) {
            Ok(response) => response,
            Err(_) => {
                let _ = terminate_router_endpoint_process(&endpoint);
                let _ = std::fs::remove_file(router_addr_path());
                return Ok(None);
            }
        };
    if response.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
        let _ = std::fs::remove_file(router_addr_path());
        return Ok(None);
    }
    if let Some(pid) = response
        .pointer("/payload/pid")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
    {
        endpoint.pid = Some(pid);
    }
    if let Some(start_time) = response
        .pointer("/payload/process_start_time")
        .and_then(serde_json::Value::as_u64)
    {
        endpoint.process_start_time = Some(start_time);
    }
    Ok(Some((endpoint, response)))
}

fn wait_for_healthy_router(timeout: Duration) -> Result<Option<RouterEndpoint>> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Some((endpoint, _health)) = healthy_router_endpoint()? {
            return Ok(Some(endpoint));
        }
        std::thread::sleep(ROUTER_STARTUP_POLL_INTERVAL);
    }
    Ok(None)
}

fn parse_router_endpoint(raw: &str) -> Result<Option<RouterEndpoint>> {
    let endpoint: serde_json::Value = serde_json::from_str(raw)?;
    let version = endpoint
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !version.is_empty() && version != tura_path::instance_version() {
        return Ok(None);
    }
    let Some(addr) = endpoint
        .get("addr")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
    else {
        return Ok(None);
    };
    let pid = endpoint
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let process_start_time = endpoint
        .get("process_start_time")
        .and_then(serde_json::Value::as_u64);
    Ok(Some(RouterEndpoint {
        addr,
        pid,
        process_start_time,
    }))
}

fn wait_for_router_addr_unreachable(addr: &str, timeout: Duration) -> bool {
    let Ok(socket) = addr.parse::<SocketAddr>() else {
        return true;
    };
    let started = Instant::now();
    while started.elapsed() < timeout {
        if TcpStream::connect_timeout(&socket, ROUTER_PROBE_CONNECT_TIMEOUT).is_err() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn terminate_router_endpoint_process(endpoint: &RouterEndpoint) -> Result<bool> {
    let Some(pid) = endpoint.pid else {
        return Ok(false);
    };
    if pid == std::process::id() {
        return Ok(false);
    }
    if !router_endpoint_process_identity_matches(endpoint) {
        return Ok(false);
    }
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) else {
        return Ok(false);
    };
    let killed = process.kill();
    if killed {
        wait_for_process_to_exit(pid, Duration::from_secs(10));
    }
    Ok(killed)
}

fn terminate_router_from_lock() -> Result<bool> {
    let Some(record) = read_process_lock_record(&router_lock_path()) else {
        return Ok(false);
    };
    if record.kind.as_deref() != Some("router")
        || record.build_kind.as_deref() != Some(tura_path::build_kind())
        || !record
            .home
            .as_deref()
            .is_some_and(|home| same_path(home, &tura_path::instance_home()))
    {
        return Ok(false);
    }
    let Some(pid) = record.pid else {
        return Ok(false);
    };
    if pid == std::process::id() {
        return Ok(false);
    }
    if let Some(expected_start_time) = record.process_start_time {
        if current_process_start_time(pid)
            .is_none_or(|start_time| start_time != expected_start_time)
        {
            return Ok(false);
        }
    } else {
        return Ok(false);
    }
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) else {
        return Ok(false);
    };
    let killed = process.kill();
    if killed {
        wait_for_process_to_exit(pid, Duration::from_secs(10));
    }
    Ok(killed)
}

fn kill_spawned_router_child(child: &mut std::process::Child) -> bool {
    match child.try_wait() {
        Ok(Some(_)) => true,
        Ok(None) => {
            let killed = child.kill().is_ok();
            let _ = child.wait();
            killed
        }
        Err(_) => false,
    }
}

fn wait_for_process_to_exit(pid: u32, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        let mut system = sysinfo::System::new_all();
        system.refresh_processes();
        if system.process(sysinfo::Pid::from_u32(pid)).is_none() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn router_endpoint_process_identity_matches(endpoint: &RouterEndpoint) -> bool {
    let (Some(pid), Some(expected_start_time)) = (endpoint.pid, endpoint.process_start_time) else {
        return false;
    };
    if !router_lock_matches_endpoint(endpoint) {
        return false;
    }
    current_process_start_time(pid).is_some_and(|start_time| start_time == expected_start_time)
}

fn router_lock_matches_endpoint(endpoint: &RouterEndpoint) -> bool {
    let Some(record) = read_process_lock_record(&router_lock_path()) else {
        return false;
    };
    record.kind.as_deref() == Some("router")
        && record.build_kind.as_deref() == Some(tura_path::build_kind())
        && record
            .home
            .as_deref()
            .is_some_and(|home| same_path(home, &tura_path::instance_home()))
        && record.pid == endpoint.pid
        && record.process_start_time == endpoint.process_start_time
}

fn router_lock_path() -> PathBuf {
    tura_path::locks_dir().join(format!("router-{}.lock", tura_path::build_kind()))
}

fn read_process_lock_record(path: &Path) -> Option<ProcessLockRecord> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut record = ProcessLockRecord {
        pid: None,
        process_start_time: None,
        kind: None,
        build_kind: None,
        home: None,
    };
    for line in raw.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "pid" => record.pid = value.trim().parse().ok(),
            "process_start_time" => record.process_start_time = value.trim().parse().ok(),
            "kind" => record.kind = Some(value.trim().to_string()),
            "build_kind" => record.build_kind = Some(value.trim().to_string()),
            "home" => record.home = Some(value.trim().to_string()),
            _ => {}
        }
    }
    Some(record)
}

fn same_path(left: &str, right: &Path) -> bool {
    let left = tura_path::normalize_path(Path::new(left));
    let right = tura_path::normalize_path(right);
    if cfg!(windows) {
        left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
    } else {
        left == right
    }
}

fn current_process_start_time(pid: u32) -> Option<u64> {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    system
        .process(sysinfo::Pid::from_u32(pid))
        .map(sysinfo::Process::start_time)
}

fn repo_root() -> Option<PathBuf> {
    std::env::var("TURA_PROJECT_ROOT")
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .as_deref()
                .and_then(find_repo_root)
        })
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .as_deref()
                .and_then(find_repo_root)
        })
}

fn find_repo_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    start
        .ancestors()
        .find(|candidate| candidate.join("crates").join("router").is_dir())
        .map(Path::to_path_buf)
}

fn router_executable_candidates(root: &Path) -> Vec<PathBuf> {
    let executable = if cfg!(windows) {
        "tura_router.exe"
    } else {
        "tura_router"
    };
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if current_exe
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            != Some("deps")
        {
            candidates.push(current_exe.with_file_name(executable));
        }
    }
    let profile = if tura_path::build_kind() == "release" {
        "release"
    } else {
        "debug"
    };
    candidates.push(root.join("target").join(profile).join(executable));
    candidates
}

fn hide_child_window_and_detach(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;

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

        fn set(key: &'static str, value: Option<&str>) -> Self {
            let previous = vec![(key, std::env::var_os(key))];
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
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

    fn temp_home(name: &str) -> anyhow::Result<PathBuf> {
        let home = std::env::temp_dir().join(format!(
            "{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir_all(&home)?;
        Ok(home)
    }

    fn write_router_endpoint(path: &Path, endpoint: serde_json::Value) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string(&endpoint)?)?;
        Ok(())
    }

    #[test]
    fn router_execution_timeout_defaults_to_long_prompt_budget_and_allows_override() {
        let _guard = crate::test_support::env_lock();
        {
            let _env = EnvGuard::set("TURA_ROUTER_EXECUTION_TIMEOUT_SECS", None);
            assert_eq!(
                read_timeout_for("execution.enqueue_turn"),
                Duration::from_secs(35 * 60)
            );
            assert_eq!(read_timeout_for("health_check"), ROUTER_HEALTH_TIMEOUT);
            assert_eq!(
                read_timeout_for("execution.shutdown"),
                Duration::from_secs(10)
            );
        }

        let _env = EnvGuard::set("TURA_ROUTER_EXECUTION_TIMEOUT_SECS", Some("42"));
        assert_eq!(
            read_timeout_for("execution.enqueue_turn"),
            Duration::from_secs(42)
        );
    }

    #[test]
    fn router_probe_removes_unreachable_addr_file_quickly() -> anyhow::Result<()> {
        let _guard = crate::test_support::env_lock();
        let home = temp_home("tura-router-stale")?;
        let _env = EnvGuard::set_home(&home);

        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        drop(listener);

        let path = router_addr_path();
        write_router_endpoint(
            &path,
            json!({
                "addr": addr.to_string(),
                "version": tura_path::instance_version(),
            }),
        )?;

        let started = Instant::now();
        assert!(reachable_router_addr()?.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "stale router probe should not wait for the full router connect timeout"
        );
        assert!(!path.exists(), "unreachable router addr should be removed");
        Ok(())
    }

    #[test]
    fn router_probe_removes_invalid_or_incompatible_addr_files() -> anyhow::Result<()> {
        let _guard = crate::test_support::env_lock();
        let home = temp_home("tura-router-invalid")?;
        let _env = EnvGuard::set_home(&home);
        let path = router_addr_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, "{not-json")?;
        let invalid_json =
            reachable_router_addr().expect_err("invalid router endpoint json should be reported");
        assert!(
            invalid_json.to_string().contains("invalid"),
            "error should describe the invalid router endpoint: {invalid_json:#}"
        );
        assert!(
            !path.exists(),
            "invalid endpoint files should be removed before the next probe"
        );

        write_router_endpoint(
            &path,
            json!({
                "addr": "127.0.0.1:1",
                "version": "older-version",
            }),
        )?;
        assert!(reachable_router_addr()?.is_none());
        assert!(
            !path.exists(),
            "version-mismatched router endpoints should be removed"
        );

        write_router_endpoint(
            &path,
            json!({
                "version": tura_path::instance_version(),
            }),
        )?;
        assert!(reachable_router_addr()?.is_none());
        assert!(
            !path.exists(),
            "router endpoints without an address should be removed"
        );
        Ok(())
    }

    #[test]
    fn router_probe_accepts_reachable_current_version_endpoint() -> anyhow::Result<()> {
        let _guard = crate::test_support::env_lock();
        let home = temp_home("tura-router-reachable")?;
        let _env = EnvGuard::set_home(&home);
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?.to_string();
        let path = router_addr_path();
        write_router_endpoint(
            &path,
            json!({
                "addr": addr,
                "version": tura_path::instance_version(),
            }),
        )?;

        assert_eq!(reachable_router_addr()?.as_deref(), Some(addr.as_str()));
        assert!(
            path.exists(),
            "reachable compatible router endpoint should remain published"
        );
        Ok(())
    }

    #[test]
    fn healthy_router_probe_rejects_socket_that_does_not_answer_health() -> anyhow::Result<()> {
        let _guard = crate::test_support::env_lock();
        let home = temp_home("tura-router-abortive-health")?;
        let _env = EnvGuard::set_home(&home);
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?.to_string();
        let server = thread::spawn(move || -> anyhow::Result<()> {
            let (stream, _) = listener.accept()?;
            drop(stream);
            Ok(())
        });
        let path = router_addr_path();
        write_router_endpoint(
            &path,
            json!({
                "addr": addr,
                "version": tura_path::instance_version(),
            }),
        )?;

        assert!(
            healthy_router_endpoint()?.is_none(),
            "gateway must not adopt a raw TCP endpoint that does not answer router health"
        );
        assert!(
            !path.exists(),
            "failed router health probes should remove the stale endpoint"
        );
        server
            .join()
            .map_err(|_| anyhow!("abortive router endpoint panicked"))??;
        Ok(())
    }

    #[test]
    fn router_probe_preserves_pid_start_time_for_status() -> anyhow::Result<()> {
        let _guard = crate::test_support::env_lock();
        let home = temp_home("tura-router-pid-status")?;
        let _env = EnvGuard::set_home(&home);
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?.to_string();
        let health_server = thread::spawn(move || -> anyhow::Result<()> {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept()?;
                stream.set_read_timeout(Some(Duration::from_secs(1)))?;
                let mut request_line = String::new();
                let _ = std::io::BufRead::read_line(
                    &mut BufReader::new(stream.try_clone()?),
                    &mut request_line,
                );
                if request_line.trim().is_empty() {
                    continue;
                }
                std::io::Write::write_all(
                    &mut stream,
                    serde_json::to_string(&json!({
                        "ok": true,
                        "payload": {
                            "pid": 4242,
                            "process_start_time": 777,
                        }
                    }))?
                    .as_bytes(),
                )?;
                std::io::Write::write_all(&mut stream, b"\n")?;
                std::io::Write::flush(&mut stream)?;
                return Ok(());
            }
            Err(anyhow!("fake router did not receive health_check"))
        });
        let path = router_addr_path();
        write_router_endpoint(
            &path,
            json!({
                "addr": addr,
                "version": tura_path::instance_version(),
                "pid": 4242,
                "process_start_time": 777,
            }),
        )?;

        let endpoint = reachable_router_endpoint()?.expect("reachable endpoint");
        assert_eq!(endpoint.pid, Some(4242));
        assert_eq!(endpoint.process_start_time, Some(777));

        let process = RouterProcess {
            router_bin: Some(PathBuf::from("unused")),
            addr: ParkingMutex::new(None),
            request_seq: AtomicU64::new(1),
            restart_count: AtomicU64::new(0),
            last_error: ParkingMutex::new(None),
        };
        let status = process.status();
        assert_eq!(status.status, "running");
        assert_eq!(status.pid, Some(4242));
        assert_eq!(status.process_start_time, Some(777));
        health_server
            .join()
            .map_err(|_| anyhow!("fake health router panicked"))??;
        Ok(())
    }

    #[test]
    fn router_endpoint_parser_handles_current_version_pid_and_rejects_incompatible() {
        let parsed = parse_router_endpoint(
            &json!({
                "addr": "127.0.0.1:12",
                "version": tura_path::instance_version(),
                "pid": 12,
                "process_start_time": 34,
            })
            .to_string(),
        )
        .expect("valid endpoint")
        .expect("compatible endpoint");

        assert_eq!(parsed.addr, "127.0.0.1:12");
        assert_eq!(parsed.pid, Some(12));
        assert_eq!(parsed.process_start_time, Some(34));

        assert!(parse_router_endpoint(
            &json!({"addr": "127.0.0.1:12", "version": "old"}).to_string()
        )
        .expect("incompatible endpoint should parse")
        .is_none());
        assert!(parse_router_endpoint(
            &json!({"version": tura_path::instance_version()}).to_string()
        )
        .expect("missing address should parse")
        .is_none());
    }

    #[test]
    fn router_pid_identity_requires_matching_start_time_before_forced_kill() {
        let _guard = crate::test_support::env_lock();
        let home = temp_home("tura-router-lock-identity").expect("temp home");
        let _env = EnvGuard::set_home(&home);
        let current_pid = std::process::id();
        let current_start = current_process_start_time(current_pid)
            .expect("current process start time should be visible");
        std::fs::create_dir_all(tura_path::locks_dir()).expect("locks dir");
        std::fs::write(
            router_lock_path(),
            format!(
                "pid={current_pid}\nprocess_start_time={current_start}\nkind=router\nbuild_kind={}\nhome={}\n",
                tura_path::build_kind(),
                tura_path::instance_home().display()
            ),
        )
        .expect("router lock");
        let matching = RouterEndpoint {
            addr: "127.0.0.1:1".to_string(),
            pid: Some(current_pid),
            process_start_time: Some(current_start),
        };
        assert!(router_endpoint_process_identity_matches(&matching));

        let mismatched = RouterEndpoint {
            process_start_time: Some(current_start.saturating_sub(1)),
            ..matching
        };
        assert!(!router_endpoint_process_identity_matches(&mismatched));

        let no_start_time = RouterEndpoint {
            addr: "127.0.0.1:1".to_string(),
            pid: Some(current_pid),
            process_start_time: None,
        };
        assert!(!router_endpoint_process_identity_matches(&no_start_time));
        assert!(!terminate_router_endpoint_process(&no_start_time)
            .expect("missing fingerprint should refuse forced termination"));
    }

    #[test]
    fn router_pid_identity_requires_matching_home_and_build_lock() {
        let _guard = crate::test_support::env_lock();
        let home = temp_home("tura-router-current-home").expect("temp home");
        let foreign_home = temp_home("tura-router-foreign-home").expect("foreign home");
        let _env = EnvGuard::set_home(&home);
        let current_pid = std::process::id();
        let current_start = current_process_start_time(current_pid)
            .expect("current process start time should be visible");
        std::fs::create_dir_all(tura_path::locks_dir()).expect("locks dir");
        let endpoint = RouterEndpoint {
            addr: "127.0.0.1:1".to_string(),
            pid: Some(current_pid),
            process_start_time: Some(current_start),
        };

        std::fs::write(
            router_lock_path(),
            format!(
                "pid={current_pid}\nprocess_start_time={current_start}\nkind=router\nbuild_kind=foreign\nhome={}\n",
                tura_path::instance_home().display()
            ),
        )
        .expect("foreign build lock");
        assert!(!router_endpoint_process_identity_matches(&endpoint));
        assert!(!terminate_router_from_lock().expect("foreign build lock should not kill"));

        std::fs::write(
            router_lock_path(),
            format!(
                "pid={current_pid}\nprocess_start_time={current_start}\nkind=router\nbuild_kind={}\nhome={}\n",
                tura_path::build_kind(),
                foreign_home.display()
            ),
        )
        .expect("foreign home lock");
        assert!(!router_endpoint_process_identity_matches(&endpoint));
        assert!(!terminate_router_from_lock().expect("foreign home lock should not kill"));
    }

    #[test]
    fn router_lock_parser_ignores_malformed_identity_and_refuses_kill() {
        let _guard = crate::test_support::env_lock();
        let home = temp_home("tura-router-malformed-lock").expect("temp home");
        let _env = EnvGuard::set_home(&home);
        std::fs::create_dir_all(tura_path::locks_dir()).expect("locks dir");
        std::fs::write(
            router_lock_path(),
            format!(
                "pid=not-a-pid\nkind=router\nbuild_kind={}\nhome={}\nignored-line\n",
                tura_path::build_kind(),
                tura_path::instance_home().display()
            ),
        )
        .expect("malformed router lock");

        let record = read_process_lock_record(&router_lock_path()).expect("lock record");
        assert_eq!(record.pid, None);
        assert_eq!(record.kind.as_deref(), Some("router"));
        assert_eq!(record.process_start_time, None);
        assert!(!terminate_router_from_lock().expect("malformed lock should not kill"));

        let current_pid = std::process::id();
        std::fs::write(
            router_lock_path(),
            format!(
                "pid={current_pid}\nkind=router\nbuild_kind={}\nhome={}\n",
                tura_path::build_kind(),
                tura_path::instance_home().display()
            ),
        )
        .expect("missing start time router lock");
        assert!(!terminate_router_from_lock().expect("missing start time should not kill"));
    }

    #[test]
    fn router_socket_call_round_trips_success_and_error_responses() -> anyhow::Result<()> {
        let success_listener = TcpListener::bind(("127.0.0.1", 0))?;
        let success_addr = success_listener.local_addr()?.to_string();
        let success_server = thread::spawn(move || -> anyhow::Result<()> {
            let (stream, _) = success_listener.accept()?;
            let mut request_line = String::new();
            std::io::BufRead::read_line(
                &mut BufReader::new(stream.try_clone()?),
                &mut request_line,
            )?;
            let request: serde_json::Value = serde_json::from_str(request_line.trim())?;
            assert_eq!(request["method"], "health_check");
            let mut writer = stream;
            std::io::Write::write_all(
                &mut writer,
                serde_json::to_string(&json!({
                    "ok": true,
                    "payload": {"healthy": true}
                }))?
                .as_bytes(),
            )?;
            std::io::Write::write_all(&mut writer, b"\n")?;
            std::io::Write::flush(&mut writer)?;
            Ok(())
        });

        let response = call_router_addr(
            &success_addr,
            &json!({
                "kind": "health_check",
                "method": "health_check",
            }),
            Duration::from_secs(2),
        )?;
        assert_eq!(response["ok"], true);
        assert_eq!(response["payload"]["healthy"], true);
        success_server
            .join()
            .map_err(|_| anyhow!("success router server panicked"))??;

        let error_listener = TcpListener::bind(("127.0.0.1", 0))?;
        let error_addr = error_listener.local_addr()?.to_string();
        let error_server = thread::spawn(move || -> anyhow::Result<()> {
            let (mut stream, _) = error_listener.accept()?;
            let mut request_line = String::new();
            std::io::BufRead::read_line(
                &mut BufReader::new(stream.try_clone()?),
                &mut request_line,
            )?;
            let request: serde_json::Value = serde_json::from_str(request_line.trim())?;
            assert_eq!(request["method"], "execution.enqueue_turn");
            std::io::Write::write_all(
                &mut stream,
                b"{\"ok\":false,\"error\":\"worker unavailable\"}\n",
            )?;
            std::io::Write::flush(&mut stream)?;
            Ok(())
        });
        let response = call_router_addr(
            &error_addr,
            &json!({
                "kind": "call",
                "method": "execution.enqueue_turn",
            }),
            Duration::from_secs(2),
        )?;
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"], "worker unavailable");
        error_server
            .join()
            .map_err(|_| anyhow!("error router server panicked"))??;
        Ok(())
    }

    #[test]
    fn router_executable_candidates_use_current_build_kind_only() {
        let root = PathBuf::from("repo-root-for-order-test");
        let candidates = router_executable_candidates(&root);
        let executable = if cfg!(windows) {
            "tura_router.exe"
        } else {
            "tura_router"
        };
        let debug = root.join("target").join("debug").join(executable);
        let release = root.join("target").join("release").join(executable);
        if tura_path::build_kind() == "release" {
            assert!(candidates.contains(&release));
            assert!(!candidates.contains(&debug));
        } else {
            assert!(candidates.contains(&debug));
            assert!(!candidates.contains(&release));
        }
    }

    #[test]
    fn global_router_process_returns_initialization_error_without_panicking() {
        let cell = OnceCell::new();
        let error = match global_router_process_from(&cell, || {
            Err(anyhow!("router binary missing for test"))
        }) {
            Ok(_) => panic!("initialization error should be returned"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("router binary missing for test"));
        assert!(cell.get().is_none());
    }
}
