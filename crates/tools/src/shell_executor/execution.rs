use crate::commands::CommandResponse;
use crate::runtime::tool::ToolContext;
use serde_json::Value;
use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;

use super::process::{
    attach_shell_process_scope, configure_process_scope, configure_tokio_process_scope,
    retain_shell_process_scope,
};
use super::response::failed_async_response;

const EXEC_OUTPUT_MAX_BYTES: usize = 1024 * 1024;
const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 512;

pub(super) fn run_command_with_timeout(mut command: Command, timeout_secs: u64) -> CommandResponse {
    let started = Instant::now();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_scope(&mut command);
    match command.spawn() {
        Ok(mut child) => {
            let mut scope = attach_shell_process_scope(child.id());
            let stdout_task = child
                .stdout
                .take()
                .map(|stream| thread::spawn(move || read_blocking_stream(stream)));
            let stderr_task = child
                .stderr
                .take()
                .map(|stream| thread::spawn(move || read_blocking_stream(stream)));
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if let Some(scope) = scope.take() {
                            retain_shell_process_scope(scope);
                        }
                        let (stdout, stderr) = drain_blocking_stream_tasks(
                            stdout_task,
                            stderr_task,
                            Duration::from_secs(2),
                        );
                        return command_response_from_status(started, status, stdout, stderr);
                    }
                    Ok(None) => {
                        if started.elapsed() >= Duration::from_secs(timeout_secs) {
                            if let Some(scope) = &scope {
                                scope.terminate();
                            }
                            let _ = child.kill();
                            let _ = child.wait();
                            let (stdout, stderr) = drain_blocking_stream_tasks(
                                stdout_task,
                                stderr_task,
                                Duration::from_secs(2),
                            );
                            let mut message = format!("Timed out after {timeout_secs} seconds");
                            if !stderr.is_empty() {
                                message.push_str("\nStderr tail:\n");
                                message.push_str(&tail_chars(&stderr, 4000));
                            }
                            return CommandResponse {
                                success: false,
                                exit_code: -1,
                                stdout,
                                stderr,
                                output: Value::String(message),
                                changes: Vec::new(),
                            };
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(err) => {
                        return CommandResponse {
                            success: false,
                            exit_code: 1,
                            stdout: String::new(),
                            stderr: err.to_string(),
                            output: Value::String(err.to_string()),
                            changes: Vec::new(),
                        };
                    }
                }
            }
        }
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.to_string(),
            output: Value::String(err.to_string()),
            changes: Vec::new(),
        },
    }
}

fn read_blocking_stream<R: Read>(mut stream: R) -> String {
    let mut output = CappedOutput::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                output.push(&buffer[..n]);
            }
            Err(_) => break,
        }
    }
    output.finish()
}

fn drain_blocking_stream_tasks(
    mut stdout_task: Option<JoinHandle<String>>,
    mut stderr_task: Option<JoinHandle<String>>,
    timeout: Duration,
) -> (String, String) {
    fn finished(task: &Option<JoinHandle<String>>) -> bool {
        task.as_ref().is_none_or(JoinHandle::is_finished)
    }

    let started = Instant::now();
    while !(finished(&stdout_task) && finished(&stderr_task)) && started.elapsed() < timeout {
        thread::sleep(Duration::from_millis(10));
    }

    fn take_if_finished(task: &mut Option<JoinHandle<String>>) -> String {
        if task.as_ref().is_some_and(JoinHandle::is_finished) {
            match task.take() {
                Some(task) => task.join().unwrap_or_default(),
                None => String::new(),
            }
        } else {
            String::new()
        }
    }

    let stdout = take_if_finished(&mut stdout_task);
    let stderr = take_if_finished(&mut stderr_task);
    (stdout, stderr)
}

fn command_response_from_status(
    started: Instant,
    status: ExitStatus,
    stdout: String,
    stderr: String,
) -> CommandResponse {
    let wall = started.elapsed().as_secs_f32();
    let exit_code = status.code().unwrap_or(1);
    let mut text =
        format!("Exit code: {exit_code}\nWall time: {wall:.1} seconds\nOutput:\n{stdout}");
    if !stderr.is_empty() {
        text.push_str("\nStderr:\n");
        text.push_str(&stderr);
    }
    CommandResponse {
        success: status.success(),
        exit_code,
        stdout,
        stderr,
        output: Value::String(text),
        changes: Vec::new(),
    }
}

fn tail_chars(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let start = chars.len().saturating_sub(max_chars);
    chars[start..].iter().collect()
}

pub(super) async fn run_tokio_command_with_timeout(
    mut command: tokio::process::Command,
    timeout_secs: u64,
    ctx: &ToolContext,
) -> CommandResponse {
    let started = Instant::now();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_tokio_process_scope(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => return failed_async_response(&err.to_string(), 1),
    };
    let pid = child.id();
    let mut scope = pid.and_then(attach_shell_process_scope);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let call_id = ctx.current_call_id().unwrap_or("command_run").to_string();
    let stdout_capture = stdout.as_ref().map(|_| SharedOutput::new());
    let stderr_capture = stderr.as_ref().map(|_| SharedOutput::new());
    let stdout_task = stdout.map(|reader| {
        let capture = stdout_capture
            .as_ref()
            .expect("stdout capture exists when stdout reader exists")
            .clone();
        tokio::spawn(read_stream_with_deltas(
            reader,
            ctx.clone(),
            call_id.clone(),
            "stdout",
            capture,
        ))
    });
    let stderr_task = stderr.map(|reader| {
        let capture = stderr_capture
            .as_ref()
            .expect("stderr capture exists when stderr reader exists")
            .clone();
        tokio::spawn(read_stream_with_deltas(
            reader,
            ctx.clone(),
            call_id.clone(),
            "stderr",
            capture,
        ))
    });
    let mut wait_task = tokio::spawn(async move { child.wait().await });
    let mut expiration = None;
    let status = if ctx.cancellation.is_cancelled() {
        expiration = Some("tool task aborted".to_string());
        if let Some(scope) = &scope {
            scope.terminate();
        }
        None
    } else {
        tokio::select! {
            output = &mut wait_task => output.ok().and_then(Result::ok),
            _ = tokio::time::sleep(Duration::from_secs(timeout_secs)) => {
                expiration = Some(format!("Timed out after {timeout_secs} seconds"));
                if let Some(scope) = &scope {
                    scope.terminate();
                }
                None
            }
            _ = ctx.cancellation.cancelled() => {
                expiration = Some("tool task aborted".to_string());
                if let Some(scope) = &scope {
                    scope.terminate();
                }
                None
            }
        }
    };
    if status.is_some() {
        if let Some(scope) = scope.take() {
            retain_shell_process_scope(scope);
        }
    }
    if status.is_none() {
        wait_task.abort();
    }
    let (stdout, stderr) =
        drain_stream_tasks(stdout_task, stdout_capture, stderr_task, stderr_capture).await;

    let wall = started.elapsed().as_secs_f32();
    match status {
        Some(status) => {
            let exit_code = status.code().unwrap_or(1);
            let mut text =
                format!("Exit code: {exit_code}\nWall time: {wall:.1} seconds\nOutput:\n{stdout}");
            if !stderr.is_empty() {
                text.push_str("\nStderr:\n");
                text.push_str(&stderr);
            }
            CommandResponse {
                success: status.success(),
                exit_code,
                stdout,
                stderr,
                output: Value::String(text),
                changes: Vec::new(),
            }
        }
        None => {
            let message = expiration.unwrap_or_else(|| "tool task aborted".to_string());
            let stderr = if stderr.is_empty() {
                message.clone()
            } else {
                format!("{stderr}\n{message}")
            };
            let mut text = format!("{message}\nWall time: {wall:.1} seconds\nOutput:\n{stdout}");
            if !stderr.is_empty() {
                text.push_str("\nStderr:\n");
                text.push_str(&stderr);
            }
            CommandResponse {
                success: false,
                exit_code: -1,
                stdout,
                stderr,
                output: Value::String(text),
                changes: Vec::new(),
            }
        }
    }
}

async fn read_stream_with_deltas<R>(
    mut reader: R,
    ctx: ToolContext,
    call_id: String,
    stream: &'static str,
    output: SharedOutput,
) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 8192];
    let mut emitted_deltas = 0_usize;
    let mut truncation_delta_emitted = false;
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                let (accepted, truncated) = output.push(&buffer[..n]);
                if emitted_deltas < MAX_EXEC_OUTPUT_DELTAS_PER_CALL && accepted > 0 {
                    emitted_deltas += 1;
                    ctx.record_event(crate::runtime::tool::ToolRuntimeEvent::OutputDelta {
                        call_id: call_id.clone(),
                        stream: stream.to_string(),
                        text: String::from_utf8_lossy(&buffer[..accepted]).to_string(),
                    });
                } else if truncated && !truncation_delta_emitted {
                    truncation_delta_emitted = true;
                    ctx.record_event(crate::runtime::tool::ToolRuntimeEvent::OutputDelta {
                        call_id: call_id.clone(),
                        stream: stream.to_string(),
                        text: format!(
                            "\n[{stream} output truncated after {EXEC_OUTPUT_MAX_BYTES} bytes]\n"
                        ),
                    });
                }
            }
            Err(_) => break,
        }
    }
    output.snapshot()
}

#[derive(Clone)]
struct SharedOutput {
    inner: Arc<Mutex<CappedOutput>>,
}

impl SharedOutput {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CappedOutput::new())),
        }
    }

    fn push(&self, chunk: &[u8]) -> (usize, bool) {
        let Ok(mut output) = self.inner.lock() else {
            return (0, false);
        };
        let accepted = output.push(chunk);
        (accepted, output.truncated())
    }

    fn snapshot(&self) -> String {
        let Ok(output) = self.inner.lock() else {
            return String::new();
        };
        output.to_text()
    }
}

struct CappedOutput {
    bytes: Vec<u8>,
    total_bytes: usize,
    truncated: bool,
}

impl CappedOutput {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            total_bytes: 0,
            truncated: false,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> usize {
        self.total_bytes = self.total_bytes.saturating_add(chunk.len());
        let remaining = EXEC_OUTPUT_MAX_BYTES.saturating_sub(self.bytes.len());
        let accepted = remaining.min(chunk.len());
        if accepted > 0 {
            self.bytes.extend_from_slice(&chunk[..accepted]);
        }
        if accepted < chunk.len() {
            self.truncated = true;
        }
        accepted
    }

    fn truncated(&self) -> bool {
        self.truncated
    }

    fn finish(self) -> String {
        self.to_text()
    }

    fn to_text(&self) -> String {
        let mut text = String::from_utf8_lossy(&self.bytes).to_string();
        if self.truncated {
            text.push_str(&format!(
                "\n[output truncated after {} bytes; {} bytes were read]\n",
                EXEC_OUTPUT_MAX_BYTES, self.total_bytes
            ));
        }
        text
    }
}

async fn drain_stream_tasks(
    mut stdout_task: Option<tokio::task::JoinHandle<String>>,
    stdout_capture: Option<SharedOutput>,
    mut stderr_task: Option<tokio::task::JoinHandle<String>>,
    stderr_capture: Option<SharedOutput>,
) -> (String, String) {
    async fn wait_task(task: &mut Option<tokio::task::JoinHandle<String>>) -> String {
        match task.as_mut() {
            Some(task) => task.await.unwrap_or_default(),
            None => String::new(),
        }
    }

    match tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(wait_task(&mut stdout_task), wait_task(&mut stderr_task))
    })
    .await
    {
        Ok(outputs) => outputs,
        Err(_) => {
            if let Some(task) = stdout_task {
                task.abort();
            }
            if let Some(task) = stderr_task {
                task.abort();
            }
            (
                stdout_capture.map_or_else(String::new, |capture| capture.snapshot()),
                stderr_capture.map_or_else(String::new, |capture| capture.snapshot()),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        read_stream_with_deltas, run_command_with_timeout, run_tokio_command_with_timeout,
        tail_chars, SharedOutput,
    };
    use crate::runtime::tool::{ToolContext, ToolRuntimeEvent};
    use std::path::PathBuf;
    use std::process::Command;
    use tokio::io::AsyncWriteExt;

    fn success_command() -> Command {
        if cfg!(windows) {
            let mut command = Command::new("powershell");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Write-Output shell-ok",
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "printf shell-ok"]);
            command
        }
    }

    fn failing_command() -> Command {
        if cfg!(windows) {
            let mut command = Command::new("powershell");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Write-Error shell-bad; exit 7",
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "printf shell-bad >&2; exit 7"]);
            command
        }
    }

    fn slow_command() -> Command {
        if cfg!(windows) {
            let mut command = Command::new("powershell");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 5",
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 5"]);
            command
        }
    }

    fn tokio_slow_command() -> tokio::process::Command {
        let command = slow_command();
        let mut tokio_command = tokio::process::Command::new(command.get_program());
        tokio_command.args(command.get_args());
        tokio_command
    }

    fn tokio_output_then_sleep_command() -> tokio::process::Command {
        if cfg!(windows) {
            let mut command = tokio::process::Command::new("powershell");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Write-Output retained-output; Start-Sleep -Seconds 5",
            ]);
            command
        } else {
            let mut command = tokio::process::Command::new("sh");
            command.args(["-c", "printf retained-output; sleep 5"]);
            command
        }
    }

    #[test]
    fn tail_chars_preserves_unicode_boundaries() {
        assert_eq!(tail_chars("alpha", 10), "alpha");
        assert_eq!(tail_chars("aébc", 3), "ébc");
        assert_eq!(tail_chars("abcdef", 0), "");
    }

    #[test]
    fn run_command_with_timeout_captures_success_output_and_exit_code() {
        let response = run_command_with_timeout(success_command(), 10);

        assert!(response.success, "{}", response.stderr);
        assert_eq!(response.exit_code, 0);
        assert!(response.stdout.contains("shell-ok"), "{response:?}");
        assert!(response
            .output
            .as_str()
            .unwrap_or_default()
            .contains("Exit code: 0"));
    }

    #[test]
    fn run_command_with_timeout_captures_failure_stderr() {
        let response = run_command_with_timeout(failing_command(), 10);

        assert!(!response.success);
        assert_eq!(response.exit_code, 7);
        assert!(response.stderr.contains("shell-bad"), "{response:?}");
        assert!(response
            .output
            .as_str()
            .unwrap_or_default()
            .contains("Stderr:"));
    }

    #[test]
    fn run_command_with_timeout_reports_spawn_error() {
        let response = run_command_with_timeout(Command::new("__tura_missing_shell_binary__"), 1);

        assert!(!response.success);
        assert_eq!(response.exit_code, 1);
        assert!(!response.stderr.is_empty());
        assert_eq!(
            response.output.as_str().unwrap_or_default(),
            response.stderr.as_str()
        );
    }

    #[test]
    fn run_command_with_timeout_kills_slow_command() {
        let response = run_command_with_timeout(slow_command(), 1);

        assert!(!response.success);
        assert_eq!(response.exit_code, -1);
        assert!(response
            .output
            .as_str()
            .unwrap_or_default()
            .contains("Timed out after 1 seconds"));
    }

    #[tokio::test]
    async fn read_stream_with_deltas_returns_output_and_records_each_chunk() {
        let (mut writer, reader) = tokio::io::duplex(64);
        let context = ToolContext::new(PathBuf::from("workspace"));
        let read_context = context.clone();
        let task = tokio::spawn(async move {
            read_stream_with_deltas(
                reader,
                read_context,
                "call-1".to_string(),
                "stdout",
                SharedOutput::new(),
            )
            .await
        });

        writer.write_all(b"alpha").await.expect("write first chunk");
        writer
            .write_all(b"bravo")
            .await
            .expect("write second chunk");
        drop(writer);

        assert_eq!(task.await.expect("reader task"), "alphabravo");
        let events = context.events();
        assert!(!events.is_empty());
        let mut combined = String::new();
        for event in events {
            let ToolRuntimeEvent::OutputDelta {
                call_id,
                stream,
                text,
            } = event
            else {
                panic!("unexpected event: {event:?}");
            };
            assert_eq!(call_id, "call-1");
            assert_eq!(stream, "stdout");
            combined.push_str(&text);
        }
        assert_eq!(combined, "alphabravo");
    }

    #[tokio::test]
    async fn run_tokio_command_with_timeout_honors_pre_cancelled_context() {
        let context = ToolContext::new(PathBuf::from("workspace"));
        context.cancellation.cancel();

        let response = run_tokio_command_with_timeout(tokio_slow_command(), 10, &context).await;

        assert!(!response.success);
        assert_eq!(response.exit_code, -1);
        assert_eq!(response.stderr, "tool task aborted");
        assert!(context.events().is_empty());
    }

    #[tokio::test]
    async fn run_tokio_command_with_timeout_retains_stdout_on_timeout() {
        let context = ToolContext::new(PathBuf::from("workspace"));

        let response =
            run_tokio_command_with_timeout(tokio_output_then_sleep_command(), 1, &context).await;

        assert!(!response.success);
        assert_eq!(response.exit_code, -1);
        assert!(
            response.stdout.contains("retained-output"),
            "stdout should keep bytes read before timeout: {response:?}"
        );
        assert!(response.stderr.contains("Timed out after 1 seconds"));
        assert!(response
            .output
            .as_str()
            .unwrap_or_default()
            .contains("retained-output"));
    }
}
