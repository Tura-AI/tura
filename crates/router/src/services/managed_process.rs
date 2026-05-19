use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use serde::Serialize;
use tokio::process::{Child, Command};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize)]
pub struct ManagedProcessStatus {
    pub name: String,
    pub pid: Option<u32>,
    pub running: bool,
    pub url: Option<String>,
}

pub struct ManagedProcess {
    name: String,
    url: Option<String>,
    child: tokio::sync::Mutex<Child>,
}

#[derive(Clone)]
pub struct ManagedProcessManager {
    processes: Arc<RwLock<HashMap<String, Arc<ManagedProcess>>>>,
}

impl ManagedProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn ensure(
        &self,
        name: &str,
        program: &str,
        args: &[String],
        cwd: &Path,
        envs: &[(&str, String)],
        url: Option<String>,
    ) -> Result<ManagedProcessStatus> {
        let existing = { self.processes.read().get(name).cloned() };
        if let Some(existing) = existing {
            if existing.is_running().await {
                return Ok(existing.status(true).await);
            }
            warn!(
                name,
                "managed process existed but is no longer running, restarting"
            );
            self.processes.write().remove(name);
        }

        let child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .envs(envs.iter().map(|(key, value)| (*key, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| anyhow!("failed to start {name}: {err}"))?;

        let process = Arc::new(ManagedProcess {
            name: name.to_string(),
            url,
            child: tokio::sync::Mutex::new(child),
        });
        let status = process.status(true).await;
        self.processes.write().insert(name.to_string(), process);
        info!(name, pid = status.pid, url = ?status.url, "managed process started");
        Ok(status)
    }

    pub async fn statuses(&self) -> Vec<ManagedProcessStatus> {
        let processes = self.processes.read().values().cloned().collect::<Vec<_>>();
        let mut statuses = Vec::with_capacity(processes.len());
        for process in processes {
            let running = process.is_running().await;
            statuses.push(process.status(running).await);
        }
        statuses
    }
}

impl ManagedProcess {
    async fn is_running(&self) -> bool {
        let mut child = self.child.lock().await;
        match child.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }

    async fn status(&self, running: bool) -> ManagedProcessStatus {
        let child = self.child.lock().await;
        ManagedProcessStatus {
            name: self.name.clone(),
            pid: child.id(),
            running,
            url: self.url.clone(),
        }
    }
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
