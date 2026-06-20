//! Router-owned `command_run` execution.
//!
//! Runtime workers orchestrate turns, but shell/tool child processes are owned
//! here so aborting a runtime worker does not orphan process-tree cleanup.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRunRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub runtime_id: Option<String>,
    pub session_directory: PathBuf,
    pub arguments: Value,
    #[serde(default)]
    pub allowed_commands: Option<BTreeSet<String>>,
}

#[derive(Debug, Clone)]
pub struct CommandRunService {
    active: Arc<AtomicUsize>,
}

impl CommandRunService {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn execute(&self, input: Value) -> Result<Value> {
        let _active = ActiveCommandRunGuard::new(Arc::clone(&self.active));
        let request: CommandRunRequest =
            serde_json::from_value(input).context("invalid command_run router payload")?;
        if request.session_directory.as_os_str().is_empty() {
            return Err(anyhow!("command_run session_directory is required"));
        }
        let output = code_tools::command_run::execute_async_value_with_allowed_and_lock_scope(
            request.arguments,
            request.session_directory,
            request.allowed_commands,
            request.session_id.clone(),
        )
        .await;
        Ok(json!({
            "status": "finished",
            "owner": "router",
            "session_id": request.session_id,
            "runtime_id": request.runtime_id,
            "result": output,
        }))
    }

    pub fn active_count(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }
}

struct ActiveCommandRunGuard {
    active: Arc<AtomicUsize>,
}

impl ActiveCommandRunGuard {
    fn new(active: Arc<AtomicUsize>) -> Self {
        active.fetch_add(1, Ordering::SeqCst);
        Self { active }
    }
}

impl Drop for ActiveCommandRunGuard {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Default for CommandRunService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandRunRequest, CommandRunService};
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::time::Instant;

    #[tokio::test]
    async fn command_run_service_executes_inside_requested_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");
        let command_line = json!({
            "status": "done",
            "task_detail": "router-owned"
        })
        .to_string();
        let response = CommandRunService::new()
            .execute(json!({
                "session_id": "session-1",
                "runtime_id": "runtime-1",
                "session_directory": workspace.path().display().to_string(),
                "arguments": {
                    "commands": [{
                        "command": "task_status",
                        "command_line": command_line
                    }]
                },
                "allowed_commands": ["task_status"]
            }))
            .await
            .expect("router command_run should execute");

        assert_eq!(response["owner"], "router");
        assert_eq!(response["session_id"], "session-1");
        assert_eq!(response["runtime_id"], "runtime-1");
        assert_eq!(
            response["result"]["results"][0]["command_type"],
            "task_status"
        );
        assert_eq!(response["result"]["results"][0]["success"], true);
        assert_eq!(CommandRunService::new().active_count(), 0);
    }

    #[tokio::test]
    async fn command_run_service_tracks_active_requests() {
        let workspace = tempfile::tempdir().expect("workspace");
        let service = CommandRunService::new();
        assert_eq!(service.active_count(), 0);

        let request = json!({
            "session_id": "session-active",
            "runtime_id": "runtime-active",
            "session_directory": workspace.path().display().to_string(),
            "arguments": {
                "commands": [{
                    "command": "shell_command",
                    "command_line": json!({
                        "command": delayed_read_only_command("active"),
                        "timeout_ms": 5000
                    }).to_string()
                }]
            }
        });
        let running = {
            let service = service.clone();
            tokio::spawn(async move { service.execute(request).await })
        };

        let started = Instant::now();
        while service.active_count() == 0 && started.elapsed().as_secs() < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(service.active_count(), 1);
        running
            .await
            .expect("command_run task should join")
            .expect("command_run should finish");
        assert_eq!(service.active_count(), 0);
    }

    #[test]
    fn command_run_payload_deserializes_allowed_commands_as_set() {
        let request: CommandRunRequest = serde_json::from_value(json!({
            "session_directory": ".",
            "arguments": { "commands": [] },
            "allowed_commands": ["shell_command", "shell_command", "task_status"]
        }))
        .expect("payload shape");

        assert_eq!(
            request.allowed_commands,
            Some(BTreeSet::from([
                "shell_command".to_string(),
                "task_status".to_string()
            ]))
        );
    }

    #[tokio::test]
    async fn command_run_service_handles_read_only_requests_concurrently() {
        let workspace = tempfile::tempdir().expect("workspace");
        let service = CommandRunService::new();
        let request = |label: &str| {
            json!({
                "session_id": format!("session-{label}"),
                "runtime_id": format!("runtime-{label}"),
                "session_directory": workspace.path().display().to_string(),
                "arguments": {
                    "commands": [{
                        "step": 1,
                        "command": "shell_command",
                        "command_line": json!({
                            "command": delayed_read_only_command(label),
                            "timeout_ms": 5000
                        }).to_string()
                    }]
                }
            })
        };

        let sequential_started = Instant::now();
        let seq_first = service
            .execute(request("seq-first"))
            .await
            .expect("sequential first command_run should finish");
        let seq_second = service
            .execute(request("seq-second"))
            .await
            .expect("sequential second command_run should finish");
        let sequential_elapsed = sequential_started.elapsed();
        assert_eq!(seq_first["result"]["results"][0]["success"], true);
        assert_eq!(seq_second["result"]["results"][0]["success"], true);

        let concurrent_started = Instant::now();
        let (first, second) = tokio::join!(
            service.execute(request("first")),
            service.execute(request("second"))
        );
        let concurrent_elapsed = concurrent_started.elapsed();

        let first = first.expect("first command_run should finish");
        let second = second.expect("second command_run should finish");
        assert_eq!(first["result"]["results"][0]["success"], true);
        assert_eq!(second["result"]["results"][0]["success"], true);
        assert!(
            concurrent_elapsed.as_nanos() * 10 < sequential_elapsed.as_nanos() * 9,
            "read-only command_run requests should overlap instead of serializing; sequential_elapsed={sequential_elapsed:?}; concurrent_elapsed={concurrent_elapsed:?}"
        );
    }

    fn delayed_read_only_command(label: &str) -> String {
        if cfg!(windows) {
            format!("Test-Path .; Start-Sleep -Milliseconds 1200; Write-Output {label}")
        } else {
            format!("find . -maxdepth 0; sleep 1.2; printf {label}")
        }
    }
}
