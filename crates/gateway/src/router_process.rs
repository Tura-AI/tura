//! Gateway client for the per-home detached router daemon.
//!
//! Gateway is a front door, not the backend owner. It probes the endpoint
//! published by `tura_router serve-socket`, starts that daemon detached when no
//! compatible daemon is reachable, and sends one request per socket connection.

use anyhow::{anyhow, Context, Result};
use once_cell::sync::OnceCell;
use parking_lot::Mutex as ParkingMutex;
use serde_json::json;
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

const ROUTER_HEALTH_TIMEOUT: Duration = Duration::from_secs(10);
const ROUTER_EXECUTION_TIMEOUT: Duration = Duration::from_secs(900);
const ROUTER_PROBE_CONNECT_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, serde::Serialize)]
pub struct RouterProcessStatus {
    pub status: String,
    pub pid: Option<u32>,
    pub restart_count: u64,
    pub error: Option<String>,
}

pub struct RouterProcess {
    router_bin: PathBuf,
    addr: ParkingMutex<Option<String>>,
    request_seq: AtomicU64,
    restart_count: AtomicU64,
    last_error: ParkingMutex<Option<String>>,
}

static ROUTER_PROCESS: OnceCell<Arc<RouterProcess>> = OnceCell::new();

pub fn global_router_process() -> Arc<RouterProcess> {
    Arc::clone(ROUTER_PROCESS.get_or_init(|| {
        Arc::new(
            RouterProcess::new().unwrap_or_else(|error| {
                panic!("failed to initialize router daemon client: {error:#}")
            }),
        )
    }))
}

pub fn start_global_router_process() -> Result<Arc<RouterProcess>> {
    let process = global_router_process();
    process.ensure_started()?;
    Ok(process)
}

impl RouterProcess {
    pub fn new() -> Result<Self> {
        let root = repo_root()
            .ok_or_else(|| anyhow!("failed to locate project root for router process"))?;
        let router_bin = router_executable_candidates(&root)
            .into_iter()
            .find(|path| path.exists())
            .ok_or_else(|| anyhow!("router binary not found in current exe/target"))?;
        Ok(Self {
            router_bin,
            addr: ParkingMutex::new(None),
            request_seq: AtomicU64::new(1),
            restart_count: AtomicU64::new(0),
            last_error: ParkingMutex::new(None),
        })
    }

    pub fn ensure_started(&self) -> Result<()> {
        if let Some(addr) = reachable_router_addr()? {
            *self.addr.lock() = Some(addr);
            *self.last_error.lock() = None;
            return Ok(());
        }

        self.spawn_router_daemon()?;
        self.restart_count.fetch_add(1, Ordering::SeqCst);

        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(90) {
            if let Some(addr) = reachable_router_addr()? {
                *self.addr.lock() = Some(addr);
                *self.last_error.lock() = None;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }

        let error = "router daemon did not become reachable".to_string();
        *self.last_error.lock() = Some(error.clone());
        Err(anyhow!(error))
    }

    pub fn restart(&self) -> Result<()> {
        *self.addr.lock() = None;
        self.spawn_router_daemon()?;
        self.restart_count.fetch_add(1, Ordering::SeqCst);
        self.ensure_started()
    }

    pub fn status(&self) -> RouterProcessStatus {
        match reachable_router_addr() {
            Ok(Some(addr)) => {
                *self.addr.lock() = Some(addr);
                RouterProcessStatus {
                    status: "running".to_string(),
                    pid: None,
                    restart_count: self.restart_count.load(Ordering::SeqCst),
                    error: self.last_error.lock().clone(),
                }
            }
            Ok(None) => RouterProcessStatus {
                status: "stopped".to_string(),
                pid: None,
                restart_count: self.restart_count.load(Ordering::SeqCst),
                error: self.last_error.lock().clone(),
            },
            Err(error) => RouterProcessStatus {
                status: "unhealthy".to_string(),
                pid: None,
                restart_count: self.restart_count.load(Ordering::SeqCst),
                error: Some(error.to_string()),
            },
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

    fn call_once(&self, method: &str, payload: serde_json::Value) -> Result<serde_json::Value> {
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
        call_router_addr(&addr, &request, read_timeout_for(method))
    }

    fn spawn_router_daemon(&self) -> Result<()> {
        let mut command = Command::new(&self.router_bin);
        command
            .arg("serve-socket")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide_child_window_and_detach(&mut command);
        command.spawn().with_context(|| {
            format!(
                "failed to spawn detached router daemon {}",
                self.router_bin.display()
            )
        })?;
        Ok(())
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

    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.trim().is_empty() {
        return Err(anyhow!("router daemon closed without a response"));
    }
    serde_json::from_str(line.trim()).context("failed to decode router daemon response")
}

fn read_timeout_for(method: &str) -> Duration {
    if method == "health_check" {
        ROUTER_HEALTH_TIMEOUT + Duration::from_secs(1)
    } else {
        ROUTER_EXECUTION_TIMEOUT
    }
}

fn router_addr_path() -> PathBuf {
    session_log::path::default_db_dir().join("router.addr")
}

fn reachable_router_addr() -> Result<Option<String>> {
    let path = router_addr_path();
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    let endpoint: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(error).with_context(|| format!("invalid {}", path.display()));
        }
    };
    let version = endpoint
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !version.is_empty() && version != tura_path::instance_version() {
        let _ = std::fs::remove_file(&path);
        return Ok(None);
    }
    let Some(addr) = endpoint
        .get("addr")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
    else {
        let _ = std::fs::remove_file(&path);
        return Ok(None);
    };
    let socket: SocketAddr = match addr.parse() {
        Ok(socket) => socket,
        Err(_) => {
            let _ = std::fs::remove_file(&path);
            return Ok(None);
        }
    };
    if TcpStream::connect_timeout(&socket, ROUTER_PROBE_CONNECT_TIMEOUT).is_ok() {
        Ok(Some(addr))
    } else {
        let _ = std::fs::remove_file(&path);
        Ok(None)
    }
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
        candidates.push(current_exe.with_file_name(executable));
    }
    candidates.push(root.join("target").join("release").join(executable));
    candidates.push(root.join("target").join("debug").join(executable));
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
    use std::net::TcpListener;
    use std::time::Instant;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    #[test]
    fn router_probe_removes_unreachable_addr_file_quickly() -> anyhow::Result<()> {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let home = std::env::temp_dir().join(format!(
            "tura-router-stale-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir_all(&home)?;
        let _env = EnvGuard::set_home(&home);

        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        drop(listener);

        let path = router_addr_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &path,
            serde_json::to_string(&json!({
                "addr": addr.to_string(),
                "version": tura_path::instance_version(),
            }))?,
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
}
