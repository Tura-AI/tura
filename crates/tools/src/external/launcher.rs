use super::client::{metadata_for, repo_root};
use super::protocol::{ExternalCommandEnvelope, ExternalCommandResponse};
use crate::runtime::tool::ToolError;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const DEFAULT_EXTERNAL_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

pub async fn invoke(
    command_id: &str,
    kind: &str,
    payload: Value,
) -> Result<ExternalCommandResponse, ToolError> {
    invoke_with_timeout(command_id, kind, payload, DEFAULT_EXTERNAL_COMMAND_TIMEOUT).await
}

pub async fn invoke_with_timeout(
    command_id: &str,
    kind: &str,
    payload: Value,
    timeout: Duration,
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

    let command = if let Some(binary_path) = metadata.binary_path {
        let mut command = Command::new(binary_path);
        command.arg("--protocol");
        command
    } else {
        let package = match command_id {
            "image_generate" => "image_generate",
            "read_media" => "read_media",
            "web_discover" => "web_discover",
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

    let output = run_command_with_timeout(command_id, command, input, timeout).await?;
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
    execute_with_timeout(
        command_id,
        arguments,
        session_dir,
        call_id,
        DEFAULT_EXTERNAL_COMMAND_TIMEOUT,
    )
    .await
}

pub async fn execute_with_timeout(
    command_id: &str,
    arguments: Value,
    session_dir: &std::path::Path,
    call_id: &str,
    timeout: Duration,
) -> Result<Value, ToolError> {
    let response = invoke_with_timeout(
        command_id,
        "execute",
        json!({
            "arguments": arguments,
            "session_dir": session_dir.display().to_string(),
            "call_id": call_id,
        }),
        timeout,
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

async fn run_command_with_timeout(
    command_id: &str,
    mut command: Command,
    input: Vec<u8>,
    timeout: Duration,
) -> Result<std::process::Output, ToolError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
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

    match tokio::time::timeout(
        timeout.max(Duration::from_millis(1)),
        child.wait_with_output(),
    )
    .await
    {
        Ok(output) => output
            .map_err(|err| ToolError::Fatal(format!("failed to wait for {command_id}: {err}"))),
        Err(_) => Err(ToolError::RespondToModel(format!(
            "{command_id} timed out after {}",
            format_timeout(timeout)
        ))),
    }
}

fn format_timeout(timeout: Duration) -> String {
    let millis = timeout.as_millis().max(1);
    if millis.is_multiple_of(1000) {
        format!("{} seconds", millis / 1000)
    } else {
        format!("{millis} ms")
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

#[cfg(test)]
mod tests {
    use super::{execute, invoke, run_command_with_timeout, tail};
    use crate::runtime::tool::ToolError;
    use serde_json::json;
    use std::path::Path;
    use std::time::Duration;
    use tokio::process::Command;

    #[test]
    fn tail_keeps_short_text_and_takes_char_suffix() {
        assert_eq!(tail("short", 20), "short");
        assert_eq!(tail("abcdef", 3), "def");
        assert_eq!(tail("a😀b😀c", 3), "b😀c");
        assert_eq!(tail("abcdef", 0), "");
    }

    #[tokio::test]
    async fn invoke_rejects_unsupported_external_command_before_spawning() {
        let err = invoke("not_a_command", "execute", json!({}))
            .await
            .expect_err("unsupported command should fail locally");

        match err {
            ToolError::RespondToModel(message) => {
                assert!(message.contains("unsupported external command"));
                assert!(message.contains("not_a_command"));
            }
            ToolError::Fatal(message) => panic!("unexpected fatal error: {message}"),
        }
    }

    #[tokio::test]
    async fn execute_propagates_unsupported_command_error() {
        let err = execute(
            "not_a_command",
            json!({"q":"anything"}),
            std::path::Path::new("."),
            "call-1",
        )
        .await
        .expect_err("unsupported command should fail locally");

        match err {
            ToolError::RespondToModel(message) => {
                assert!(message.contains("unsupported external command"));
            }
            ToolError::Fatal(message) => panic!("unexpected fatal error: {message}"),
        }
    }

    #[tokio::test]
    async fn launcher_timeout_kills_external_child_before_late_write() {
        let done_path = std::env::temp_dir().join(format!(
            "tura-external-timeout-{}-{}.txt",
            std::process::id(),
            unique_nanos()
        ));
        let _ = std::fs::remove_file(&done_path);

        let err = run_command_with_timeout(
            "read_media",
            delayed_write_command(&done_path),
            Vec::new(),
            Duration::from_millis(50),
        )
        .await
        .expect_err("slow external process should time out");

        match err {
            ToolError::RespondToModel(message) => {
                assert!(message.contains("read_media timed out after 50 ms"));
            }
            ToolError::Fatal(message) => panic!("unexpected fatal error: {message}"),
        }

        tokio::time::sleep(Duration::from_millis(700)).await;
        assert!(
            !done_path.exists(),
            "timed out external child should not keep running long enough to write {}",
            done_path.display()
        );
    }

    #[cfg(windows)]
    fn delayed_write_command(done_path: &Path) -> Command {
        let path = done_path.to_string_lossy().replace('\'', "''");
        let mut command = Command::new("powershell");
        command.arg("-NoProfile").arg("-Command").arg(format!(
            "Start-Sleep -Milliseconds 500; Set-Content -LiteralPath '{path}' -Value done"
        ));
        command
    }

    #[cfg(not(windows))]
    fn delayed_write_command(done_path: &Path) -> Command {
        let path = done_path.to_string_lossy().replace('\'', "'\\''");
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(format!("sleep 0.5; printf done > '{path}'"));
        command
    }

    fn unique_nanos() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after UNIX_EPOCH")
            .as_nanos()
    }
}
