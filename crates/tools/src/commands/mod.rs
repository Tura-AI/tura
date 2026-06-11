pub mod apply_patch;
pub mod bash;
pub mod command_safety;
pub mod compact_context;
pub mod planning;
pub mod shell_command;
pub mod task_status;
pub mod zsh;

use crate::runtime::file_locks::Access;
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct CommandResponse {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub output: Value,
    pub changes: Vec<Value>,
}

pub fn execute(
    command: &str,
    command_line: &str,
    session_dir: &Path,
    timeout_secs: u64,
) -> CommandResponse {
    match canonical_command(command).as_str() {
        "apply_patch" => apply_patch::execute(command_line, session_dir),
        "bash" => bash::execute(command_line, session_dir, timeout_secs),
        "compact_context" => compact_context::execute(command_line, session_dir),
        "planning" if planning_command_enabled() => planning::execute(command_line, session_dir),
        "read_media" => execute_external("read_media", command_line, session_dir),
        "shell_command" => shell_command::execute(command_line, session_dir, timeout_secs),
        "web_discover" => execute_external("web_discover", command_line, session_dir),
        "zsh" => zsh::execute(command_line, session_dir, timeout_secs),
        other => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("unsupported command_run command: {other}"),
            output: Value::String(format!("unsupported command_run command: {other}")),
            changes: Vec::new(),
        },
    }
}

pub fn access(command: &str, command_line: &str, session_dir: &Path) -> Access {
    match canonical_command(command).as_str() {
        "apply_patch" => apply_patch::access(command_line, session_dir),
        "compact_context" => Access::default(),
        "planning" if planning_command_enabled() => Access::default(),
        "read_media" => access_external("read_media", command_line, session_dir),
        "web_discover" => access_external("web_discover", command_line, session_dir),
        "shell_command" | "bash" | "zsh" if shell_command::looks_read_only(command_line) => {
            Access::default()
        }
        "shell_command" | "bash" | "zsh" => Access {
            workspace_write: true,
            ..Access::default()
        },
        _ => Access::default(),
    }
}

pub fn display_command(
    command: &str,
    command_line: &str,
    session_dir: &Path,
    timeout_secs: u64,
) -> String {
    if canonical_command(command) == "apply_patch" {
        return "apply_patch".to_string();
    }
    if canonical_command(command) == "planning" && planning_command_enabled() {
        return "planning".to_string();
    }
    if canonical_command(command) == "compact_context" {
        return "compact_context".to_string();
    }
    if canonical_command(command) == "read_media" {
        return "read_media".to_string();
    }
    if canonical_command(command) == "web_discover" {
        return "web_discover".to_string();
    }
    shell_command::display_command(command_line, session_dir, timeout_secs)
}

pub fn result_command_name(command: &str) -> String {
    canonical_command(command)
}

pub fn canonical_command(name: &str) -> String {
    let text = name.trim().to_ascii_lowercase().replace('-', "_");
    match text.as_str() {
        "bash" | "zsh" | "shell" | "shell_command" | "shll" | "shall" => {
            active_shell_command_name().to_string()
        }
        "apply_patch" => "apply_patch".to_string(),
        "compact_context" | "compact" | "compact_message" | "context_compaction" => {
            "compact_context".to_string()
        }
        "planning" => "planning".to_string(),
        "read_media" | "view_media" | "inspect_media" => "read_media".to_string(),
        "web_discover" | "web_search" | "web_fetch" | "discover_web" | "search_web" => {
            "web_discover".to_string()
        }
        "task_status" => "task_status".to_string(),
        other => other.to_string(),
    }
}

pub fn active_shell_command_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("zsh") => "zsh",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => "shell_command",
        _ if cfg!(windows) => "shell_command",
        _ if cfg!(target_os = "macos") => "zsh",
        _ => "bash",
    }
}

fn planning_command_enabled() -> bool {
    ["TURA_FORCE_PLANNING", "TURA_FORCE_EXECUTE_TOOLS_PLANNING"]
        .iter()
        .any(|name| {
            std::env::var(name)
                .ok()
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(false)
        })
}

fn execute_external(command: &str, command_line: &str, session_dir: &Path) -> CommandResponse {
    let command = command.to_string();
    let payload = Value::String(command_line.to_string());
    let session_dir = session_dir.display().to_string();
    let run = async move {
        crate::external::launcher::invoke(
            &command,
            "execute",
            serde_json::json!({
                "arguments": payload,
                "session_dir": session_dir,
                "call_id": "command_run",
            }),
        )
        .await
    };
    let response = if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|err| err.to_string())?
                .block_on(run)
                .map_err(|err| err.to_string())
        })
        .join()
        .unwrap_or_else(|_| Err("external command thread panicked".to_string()))
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| err.to_string())
            .and_then(|runtime| runtime.block_on(run).map_err(|err| err.to_string()))
    };
    match response {
        Ok(response) => CommandResponse {
            success: response.success,
            exit_code: response.exit_code,
            stdout: if response.success {
                response.output.to_string()
            } else {
                String::new()
            },
            stderr: response.stderr,
            output: response.output,
            changes: Vec::new(),
        },
        Err(error) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: error.clone(),
            output: serde_json::json!({ "error": error }),
            changes: Vec::new(),
        },
    }
}

fn access_external(command: &str, command_line: &str, session_dir: &Path) -> Access {
    let command = command.to_string();
    let payload = Value::String(command_line.to_string());
    let session_dir = session_dir.display().to_string();
    let run = async move {
        crate::external::launcher::invoke(
            &command,
            "access",
            serde_json::json!({
                "arguments": payload,
                "session_dir": session_dir,
                "call_id": "command_run",
            }),
        )
        .await
    };
    let response = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()
        .and_then(|runtime| runtime.block_on(run).ok());
    response
        .and_then(|response| serde_json::from_value(response.output).ok())
        .unwrap_or_default()
}
