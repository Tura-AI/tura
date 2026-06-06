use super::client::{metadata_for, repo_root};
use super::protocol::{ExternalCommandEnvelope, ExternalCommandResponse};
use crate::runtime::tool::ToolError;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub async fn invoke(
    command_id: &str,
    kind: &str,
    payload: Value,
) -> Result<ExternalCommandResponse, ToolError> {
    let metadata = metadata_for(command_id).ok_or_else(|| {
        ToolError::RespondToModel(format!("unsupported external command: {command_id}"))
    })?;
    let envelope = ExternalCommandEnvelope {
        kind: kind.to_string(),
        payload,
    };
    let input = serde_json::to_vec(&envelope).map_err(|err| {
        ToolError::Fatal(format!("failed to encode external command request: {err}"))
    })?;

    let mut command = if let Some(binary_path) = metadata.binary_path {
        let mut command = Command::new(binary_path);
        command.arg("--protocol");
        command
    } else {
        let package = match command_id {
            "read_media" => "tura-command-read-media",
            "web_discover" => "tura-command-web-discover",
            _ => unreachable!("metadata_for filtered unsupported command"),
        };
        let mut command = Command::new("cargo");
        command
            .arg("run")
            .arg("-q")
            .arg("-p")
            .arg(package)
            .arg("--")
            .arg("--protocol");
        if let Some(root) = repo_root() {
            command.current_dir(root);
        }
        command
    };

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        command.creation_flags(0x08000000);
    }

    let mut child = command
        .spawn()
        .map_err(|err| ToolError::RespondToModel(format!("failed to start {command_id}: {err}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&input).await.map_err(|err| {
            ToolError::Fatal(format!("failed to send {command_id} request: {err}"))
        })?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|err| ToolError::Fatal(format!("failed to wait for {command_id}: {err}")))?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut response: ExternalCommandResponse =
        serde_json::from_str(stdout.trim()).map_err(|err| {
            ToolError::RespondToModel(format!(
                "{command_id} returned invalid protocol JSON: {err}; stderr: {}",
                tail(&stderr, 1200)
            ))
        })?;
    if !output.status.success() && response.exit_code == 0 {
        response.exit_code = output.status.code().unwrap_or(1);
    }
    if response.stderr.is_empty() {
        response.stderr = stderr;
    }
    if !response.ok {
        response.success = false;
    }
    Ok(response)
}

pub async fn execute(
    command_id: &str,
    arguments: Value,
    session_dir: &std::path::Path,
    call_id: &str,
) -> Result<Value, ToolError> {
    let response = invoke(
        command_id,
        "execute",
        json!({
            "arguments": arguments,
            "session_dir": session_dir.display().to_string(),
            "call_id": call_id,
        }),
    )
    .await?;
    if response.ok && response.success {
        Ok(response.output)
    } else {
        let message = response
            .output
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.is_empty())
            .or_else(|| (!response.stderr.is_empty()).then(|| response.stderr.clone()))
            .unwrap_or_else(|| {
                format!("{command_id} failed with exit code {}", response.exit_code)
            });
        Err(ToolError::RespondToModel(message))
    }
}

fn tail(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        value.to_string()
    } else {
        chars[chars.len() - max_chars..].iter().collect()
    }
}
