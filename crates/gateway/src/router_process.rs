//! Gateway-owned router process supervisor.
//!
//! Ownership boundary: gateway may start, health-check, restart, and stop the
//! single persistent router child. Gateway must not spawn runtime workers or
//! any process tree below the router.

use anyhow::{anyhow, Context, Result};
use once_cell::sync::OnceCell;
use parking_lot::Mutex as ParkingMutex;
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

const ROUTER_HEALTH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, serde::Serialize)]
pub struct RouterProcessStatus {
    pub status: String,
    pub pid: Option<u32>,
    pub restart_count: u64,
    pub error: Option<String>,
}

pub struct RouterProcess {
    root: PathBuf,
    router_bin: PathBuf,
    child: ParkingMutex<Option<Child>>,
    stdin: ParkingMutex<Option<ChildStdin>>,
    stdout: ParkingMutex<Option<std::io::BufReader<ChildStdout>>>,
    request_seq: AtomicU64,
    restart_count: AtomicU64,
    last_error: ParkingMutex<Option<String>>,
}

static ROUTER_PROCESS: OnceCell<Arc<RouterProcess>> = OnceCell::new();

pub fn global_router_process() -> Arc<RouterProcess> {
    Arc::clone(ROUTER_PROCESS.get_or_init(|| {
        Arc::new(RouterProcess::new().unwrap_or_else(|error| {
            panic!("failed to initialize router process supervisor: {error:#}")
        }))
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
            .ok_or_else(|| anyhow!("router binary not found in current exe/bin/target"))?;
        Ok(Self {
            root,
            router_bin,
            child: ParkingMutex::new(None),
            stdin: ParkingMutex::new(None),
            stdout: ParkingMutex::new(None),
            request_seq: AtomicU64::new(1),
            restart_count: AtomicU64::new(0),
            last_error: ParkingMutex::new(None),
        })
    }

    pub fn ensure_started(&self) -> Result<()> {
        if self.is_child_alive() {
            return Ok(());
        }
        self.clear_child();
        self.spawn_router()?;
        self.restart_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn restart(&self) -> Result<()> {
        self.clear_child();
        self.spawn_router()?;
        self.restart_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn status(&self) -> RouterProcessStatus {
        let pid = self.child.lock().as_ref().map(Child::id);
        let alive = self.is_child_alive();
        RouterProcessStatus {
            status: if alive { "running" } else { "stopped" }.to_string(),
            pid,
            restart_count: self.restart_count.load(Ordering::SeqCst),
            error: self.last_error.lock().clone(),
        }
    }

    pub fn call(&self, method: &str, payload: serde_json::Value) -> Result<serde_json::Value> {
        self.ensure_started()?;
        let request_id = format!(
            "gateway-{}",
            self.request_seq.fetch_add(1, Ordering::SeqCst)
        );
        let request = json!({
            "request_id": request_id,
            "kind": if method == "health_check" { "health_check" } else { "call" },
            "method": method,
            "payload": payload,
            "deadline_ms": ROUTER_HEALTH_TIMEOUT.as_millis() as u64,
        });
        let line = serde_json::to_string(&request)?;
        {
            let mut stdin = self.stdin.lock();
            let stdin = stdin
                .as_mut()
                .ok_or_else(|| anyhow!("router stdin is unavailable"))?;
            use std::io::Write;
            stdin.write_all(line.as_bytes())?;
            stdin.write_all(b"\n")?;
            stdin.flush()?;
        }
        let mut response_line = String::new();
        {
            let mut stdout = self.stdout.lock();
            let stdout = stdout
                .as_mut()
                .ok_or_else(|| anyhow!("router stdout is unavailable"))?;
            use std::io::BufRead;
            stdout.read_line(&mut response_line)?;
        }
        if response_line.trim().is_empty() {
            self.clear_child();
            return Err(anyhow!("router returned an empty response"));
        }
        let response: serde_json::Value = serde_json::from_str(response_line.trim())?;
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

    fn spawn_router(&self) -> Result<()> {
        let mut command = Command::new(&self.router_bin);
        command
            .arg("serve")
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_child_window(&mut command);
        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to spawn persistent router {}",
                self.router_bin.display()
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("router stdin missing"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("router stdout missing"))?;
        *self.stdin.lock() = Some(stdin);
        *self.stdout.lock() = Some(std::io::BufReader::new(stdout));
        *self.child.lock() = Some(child);
        if let Err(error) = self.call("health_check", json!({})) {
            *self.last_error.lock() = Some(error.to_string());
            self.clear_child();
            return Err(error);
        }
        *self.last_error.lock() = None;
        Ok(())
    }

    fn is_child_alive(&self) -> bool {
        let mut child = self.child.lock();
        match child.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }

    fn clear_child(&self) {
        self.stdin.lock().take();
        self.stdout.lock().take();
        if let Some(mut child) = self.child.lock().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
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
    candidates.push(root.join("bin").join(executable));
    candidates.push(root.join("target").join("release").join(executable));
    candidates.push(root.join("target").join("debug").join(executable));
    candidates
}

fn hide_child_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}
