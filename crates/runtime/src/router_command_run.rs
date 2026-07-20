//! Runtime client for router-owned `command_run` execution.
//!
//! The runtime owns turn orchestration and checkpoints. The router owns the
//! child process tree created by shell/tool commands.

use router_contract::{IpcRequest, IpcResponse};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const ROUTER_ADDR_ENV: &str = "TURA_ROUTER_ADDR";
const DEFAULT_COMMAND_RUN_TIMEOUT_SECS: u64 = 900;
static REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

pub async fn execute_command_run_value(
    arguments: Value,
    session_directory: PathBuf,
    session_id: Option<&str>,
    runtime_id: Option<&str>,
    allowed_commands: Option<BTreeSet<String>>,
) -> Result<Value, String> {
    let addr = std::env::var(ROUTER_ADDR_ENV).map_err(|_| {
        format!("{ROUTER_ADDR_ENV} is not set; command_run must be owned by router")
    })?;
    let addr = addr
        .parse::<SocketAddr>()
        .map_err(|error| format!("invalid {ROUTER_ADDR_ENV}: {error}"))?;
    let payload = json!({
        "session_id": session_id,
        "runtime_id": runtime_id,
        "session_directory": session_directory,
        "arguments": arguments,
        "allowed_commands": allowed_commands,
        "sandbox": command_run_sandbox_enabled(),
    });
    let request_id = format!(
        "runtime-command-run-{}-{}",
        std::process::id(),
        REQUEST_SEQ.fetch_add(1, Ordering::SeqCst)
    );
    let request = IpcRequest::call(request_id, "execution.command_run", payload);

    let timeout = command_run_router_timeout();
    let response = tokio::time::timeout(timeout, call_router(addr, request))
        .await
        .map_err(|_| {
            format!(
                "router command_run timed out after {} seconds",
                timeout.as_secs()
            )
        })??;
    if !response.ok {
        return Err(response
            .error
            .unwrap_or_else(|| "router command_run failed".to_string()));
    }
    response
        .payload
        .get("result")
        .cloned()
        .ok_or_else(|| "router command_run response did not contain payload.result".to_string())
}

pub async fn execute_command_run_value_or_error(
    arguments: Value,
    session_directory: PathBuf,
    session_id: Option<&str>,
    runtime_id: Option<&str>,
    allowed_commands: Option<BTreeSet<String>>,
) -> Value {
    execute_command_run_value(
        arguments,
        session_directory,
        session_id,
        runtime_id,
        allowed_commands,
    )
    .await
    .unwrap_or_else(command_run_error_payload)
}

pub async fn execute_streamed_command_value_or_error(
    command: Value,
    session_directory: PathBuf,
    session_id: Option<&str>,
    runtime_id: Option<&str>,
    allowed_commands: Option<BTreeSet<String>>,
) -> Value {
    execute_command_run_value_or_error(
        json!({ "commands": [command] }),
        session_directory,
        session_id,
        runtime_id,
        allowed_commands,
    )
    .await
}

pub struct RouterCommandRunExecutor {
    session_directory: PathBuf,
    allowed_commands: Option<BTreeSet<String>>,
    ctx: code_tools::runtime::tool::ToolContext,
    session_id: String,
    runtime_id: String,
    halted: bool,
}

pub(crate) struct RouterCommandRunCommandResult {
    pub(crate) results: Vec<Value>,
    pub(crate) halted: bool,
}

impl RouterCommandRunExecutor {
    pub fn new_with_allowed(
        session_directory: PathBuf,
        allowed_commands: Option<BTreeSet<String>>,
        session_id: String,
        runtime_id: String,
    ) -> Self {
        Self {
            ctx: code_tools::runtime::tool::ToolContext::new(session_directory.clone()),
            session_directory,
            allowed_commands,
            session_id,
            runtime_id,
            halted: false,
        }
    }

    pub async fn push_command_value(&mut self, command: Value) -> Vec<Value> {
        if self.halted {
            return Vec::new();
        }
        let result = execute_command_value_results(
            command,
            self.session_directory.clone(),
            Some(&self.session_id),
            Some(&self.runtime_id),
            self.allowed_commands.clone(),
        )
        .await;
        if result.halted {
            self.halted = true;
            self.ctx.cancellation.cancel();
        }
        result.results
    }

    pub async fn finish(self) -> Vec<Value> {
        Vec::new()
    }

    pub fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn event_context(&self) -> code_tools::runtime::tool::ToolContext {
        self.ctx.child()
    }
}

pub(crate) async fn execute_command_value_results(
    command: Value,
    session_directory: PathBuf,
    session_id: Option<&str>,
    runtime_id: Option<&str>,
    allowed_commands: Option<BTreeSet<String>>,
) -> RouterCommandRunCommandResult {
    let fallback_command = command.clone();
    let output = match execute_command_run_value(
        json!({ "commands": [command] }),
        session_directory,
        session_id,
        runtime_id,
        allowed_commands,
    )
    .await
    {
        Ok(output) => output,
        Err(error) => {
            let result = command_failure_result(&fallback_command, &error);
            let halted = is_failed_apply_patch_result(&result);
            return RouterCommandRunCommandResult {
                results: vec![result],
                halted,
            };
        }
    };
    let results = command_run_results(&output).unwrap_or_else(|| {
        vec![command_failure_result(
            &fallback_command,
            "router command_run did not return results",
        )]
    });
    let halted = results.iter().any(is_failed_apply_patch_result);
    RouterCommandRunCommandResult { results, halted }
}

fn command_run_results(output: &Value) -> Option<Vec<Value>> {
    output.get("results")?.as_array().cloned()
}

pub(crate) fn command_run_sandbox_enabled() -> bool {
    std::env::var("TURA_COMMAND_RUN_SANDBOX")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "enabled"
            )
        })
        .unwrap_or(false)
}

fn is_failed_apply_patch_result(result: &Value) -> bool {
    result.get("command_type").and_then(Value::as_str) == Some("apply_patch")
        && result.get("success").and_then(Value::as_bool) == Some(false)
}

fn command_failure_result(command: &Value, error: &str) -> Value {
    let step = command
        .get("step")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let command_type = command
        .get("command")
        .and_then(Value::as_str)
        .or_else(|| command.get("command_type").and_then(Value::as_str))
        .unwrap_or("command_run");
    json!({
        "step": step,
        "command_type": command_type,
        "success": false,
        "error": error,
    })
}

pub fn command_run_error_payload(error: String) -> Value {
    json!({
        "ok": false,
        "error": error,
        "results": [{
            "step": 1,
            "command_type": "command_run",
            "success": false,
            "error": error,
        }]
    })
}

async fn call_router(addr: SocketAddr, request: IpcRequest) -> Result<IpcResponse, String> {
    let stream = TcpStream::connect(addr)
        .await
        .map_err(|error| format!("failed to connect to router at {addr}: {error}"))?;
    let (read, mut write) = stream.into_split();
    write
        .write_all(
            format!(
                "{}\n",
                serde_json::to_string(&request)
                    .map_err(|error| format!("failed to encode router request: {error}"))?
            )
            .as_bytes(),
        )
        .await
        .map_err(|error| format!("failed to write router command_run request: {error}"))?;
    write
        .flush()
        .await
        .map_err(|error| format!("failed to flush router command_run request: {error}"))?;

    let mut line = String::new();
    let mut reader = BufReader::new(read);
    reader
        .read_line(&mut line)
        .await
        .map_err(|error| format!("failed to read router command_run response: {error}"))?;
    if line.trim().is_empty() {
        return Err("router closed command_run response without a body".to_string());
    }
    serde_json::from_str(line.trim())
        .map_err(|error| format!("failed to decode router command_run response: {error}"))
}

fn command_run_router_timeout() -> Duration {
    std::env::var("TURA_ROUTER_COMMAND_RUN_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_COMMAND_RUN_TIMEOUT_SECS))
}

#[cfg(test)]
mod tests {
    use super::{command_run_error_payload, command_run_results, RouterCommandRunExecutor};
    use serde_json::json;

    #[test]
    fn command_run_error_payload_is_a_failed_command_run_result() {
        let payload = command_run_error_payload("missing router".to_string());

        assert_eq!(payload["ok"], false);
        assert_eq!(payload["results"][0]["success"], false);
        assert_eq!(payload["results"][0]["error"], "missing router");
    }

    #[tokio::test]
    async fn router_executor_halts_after_failed_apply_patch() {
        let _router_addr = EnvGuard::remove("TURA_ROUTER_ADDR");
        let workspace = tempfile::tempdir().expect("workspace");
        let mut executor = RouterCommandRunExecutor::new_with_allowed(
            workspace.path().to_path_buf(),
            None,
            "session-1".to_string(),
            "runtime-1".to_string(),
        );

        let results = executor
            .push_command_value(json!({
                "step": 1,
                "command": "apply_patch",
                "command_line": "*** Begin Patch\n*** End Patch\n"
            }))
            .await;

        assert!(results.iter().any(|result| {
            result["command_type"] == "apply_patch" && result["success"] == false
        }));
        assert!(executor.is_halted());
        assert!(executor.finish().await.is_empty());
    }

    #[test]
    fn command_run_results_extracts_result_array() {
        let output = json!({ "results": [{ "success": true }] });

        assert_eq!(
            command_run_results(&output),
            Some(vec![json!({"success": true})])
        );
        assert_eq!(command_run_results(&json!({"ok": false})), None);
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            }
        }
    }
}
