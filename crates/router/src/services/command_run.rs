//! Router-owned `command_run` execution.
//!
//! Runtime workers orchestrate turns, but shell/tool child processes are owned
//! here so aborting a runtime worker does not orphan process-tree cleanup.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

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
pub struct CommandRunService;

impl CommandRunService {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, input: Value) -> Result<Value> {
        let request: CommandRunRequest =
            serde_json::from_value(input).context("invalid command_run router payload")?;
        if request.session_directory.as_os_str().is_empty() {
            return Err(anyhow!("command_run session_directory is required"));
        }
        let output = code_tools::command_run::execute_async_value_with_allowed(
            request.arguments,
            request.session_directory,
            request.allowed_commands,
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
}
