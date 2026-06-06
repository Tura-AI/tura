//! Router-owned session DB service lifecycle.
//!
//! Router starts and health-checks the service, but it must not parse or proxy
//! normal session DB read/write payloads.

use anyhow::{anyhow, Result};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct SessionDbService {
    child: Arc<Mutex<Option<Child>>>,
}

impl SessionDbService {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&self) -> Result<serde_json::Value> {
        if self.is_alive() {
            return Ok(self.status_payload("running"));
        }
        let router_bin = router_binary()
            .ok_or_else(|| anyhow!("session_db service executable tura_router not found"))?;
        let mut command = Command::new(&router_bin);
        command
            .arg("session-db-service")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_child_window(&mut command);
        let child = command.spawn()?;
        *self
            .child
            .lock()
            .map_err(|_| anyhow!("session_db lock poisoned"))? = Some(child);
        Ok(self.status_payload("running"))
    }

    pub fn restart(&self) -> Result<serde_json::Value> {
        self.stop();
        self.start()
    }

    pub fn status(&self) -> serde_json::Value {
        self.status_payload(if self.is_alive() {
            "running"
        } else {
            "stopped"
        })
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    fn is_alive(&self) -> bool {
        let Ok(mut guard) = self.child.lock() else {
            return false;
        };
        match guard.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }

    fn status_payload(&self, status: &str) -> serde_json::Value {
        let pid = self
            .child
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(Child::id));
        json!({ "status": status, "pid": pid })
    }
}

fn router_binary() -> Option<PathBuf> {
    let root = std::env::var("TURA_PROJECT_ROOT")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .as_deref()
                .and_then(find_repo_root)
        })
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .as_deref()
                .and_then(find_repo_root)
        })?;
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
    candidates.into_iter().find(|path| path.exists())
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

fn hide_child_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}
