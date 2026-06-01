use crate::commands::CommandResponse;
use crate::runtime::tool::ToolContext;
use serde_json::Value;
use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;

use super::process::{
    configure_process_group, configure_tokio_process_group, kill_child_process_tree,
};
use super::response::failed_async_response;

pub(super) fn run_command_with_timeout(mut command: Command, timeout_secs: u64) -> CommandResponse {
    let started = Instant::now();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut command);
    match command.spawn() {
        Ok(mut child) => {
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
                        let (stdout, stderr) = drain_blocking_stream_tasks(
                            stdout_task,
                            stderr_task,
                            Duration::from_secs(2),
                        );
                        return command_response_from_status(started, status, stdout, stderr);
                    }
                    Ok(None) => {
                        if started.elapsed() >= Duration::from_secs(timeout_secs) {
                            kill_child_process_tree(child.id());
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
    let mut output = Vec::new();
    let _ = stream.read_to_end(&mut output);
    String::from_utf8_lossy(&output).to_string()
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
            let task = task.take().expect("task was checked as present");
            task.join().unwrap_or_default()
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
    let mut text = format!(
        "Exit code: {exit_code}\nWall time: {:.1} seconds\nOutput:\n{}",
        wall, stdout
    );
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
    configure_tokio_process_group(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => return failed_async_response(&err.to_string(), 1),
    };
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let call_id = ctx.current_call_id().unwrap_or("command_run").to_string();
    let stdout_task = stdout.map(|reader| {
        tokio::spawn(read_stream_with_deltas(
            reader,
            ctx.clone(),
            call_id.clone(),
            "stdout",
        ))
    });
    let stderr_task = stderr.map(|reader| {
        tokio::spawn(read_stream_with_deltas(
            reader,
            ctx.clone(),
            call_id.clone(),
            "stderr",
        ))
    });
    let mut wait_task = tokio::spawn(async move { child.wait().await });
    let mut expiration = None;
    let status = if ctx.cancellation.is_cancelled() {
        expiration = Some("tool task aborted".to_string());
        if let Some(pid) = pid {
            kill_child_process_tree(pid);
        }
        None
    } else {
        tokio::select! {
            output = &mut wait_task => output.ok().and_then(Result::ok),
            _ = tokio::time::sleep(Duration::from_secs(timeout_secs)) => {
                expiration = Some(format!("Timed out after {timeout_secs} seconds"));
                if let Some(pid) = pid {
                    kill_child_process_tree(pid);
                }
                None
            }
            _ = ctx.cancellation.cancelled() => {
                expiration = Some("tool task aborted".to_string());
                if let Some(pid) = pid {
                    kill_child_process_tree(pid);
                }
                None
            }
        }
    };
    if status.is_none() {
        wait_task.abort();
    }
    let (stdout, stderr) = drain_stream_tasks(stdout_task, stderr_task).await;

    let wall = started.elapsed().as_secs_f32();
    match status {
        Some(status) => {
            let exit_code = status.code().unwrap_or(1);
            let mut text = format!(
                "Exit code: {exit_code}\nWall time: {:.1} seconds\nOutput:\n{}",
                wall, stdout
            );
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
            CommandResponse {
                success: false,
                exit_code: -1,
                stdout: String::new(),
                stderr: message.clone(),
                output: Value::String(message),
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
) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                output.extend_from_slice(&buffer[..n]);
                ctx.record_event(crate::runtime::tool::ToolRuntimeEvent::OutputDelta {
                    call_id: call_id.clone(),
                    stream: stream.to_string(),
                    text: String::from_utf8_lossy(&buffer[..n]).to_string(),
                });
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&output).to_string()
}

async fn drain_stream_tasks(
    mut stdout_task: Option<tokio::task::JoinHandle<String>>,
    mut stderr_task: Option<tokio::task::JoinHandle<String>>,
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
            (String::new(), String::new())
        }
    }
}
