use super::*;
use crate::contracts::{ShellRequest, ShellResponse};

pub async fn session_shell(
    Path(session_id): Path<String>,
    Json(payload): Json<ShellRequest>,
) -> Json<ShellResponse> {
    let directory = session_store()
        .get_session(&session_id)
        .and_then(|session| session.directory)
        .unwrap_or_else(|| ".".to_string());
    let output = run_session_shell_command(&directory, &payload.input)
        .unwrap_or_else(|error| format!("failed to run shell command: {error}"));
    Json(ShellResponse { output })
}

pub(super) fn truncate_summary_text(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in value.chars().take(max_chars) {
        output.push(ch);
    }
    if value.chars().count() > max_chars {
        output.push_str("...");
    }
    output.replace('\n', " ")
}

pub(super) fn run_session_shell_command(directory: &str, input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let mut command = if cfg!(windows) {
        let mut command = ProcessCommand::new("powershell");
        command.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            trimmed,
        ]);
        command
    } else {
        let mut command = ProcessCommand::new("sh");
        command.args(["-lc", trimmed]);
        command
    };
    command.current_dir(directory);
    let output = command.output().map_err(|error| {
        format!("failed to spawn session shell command in {directory}: {error}")
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut combined = String::new();
    if !stdout.trim().is_empty() {
        combined.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(stderr.trim_end());
    }
    if !output.status.success() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&format!("exit status: {}", output.status));
    }
    Ok(combined)
}
