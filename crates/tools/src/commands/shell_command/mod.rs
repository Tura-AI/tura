pub const COMMAND_NAME: &str = "shell_command";
pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

use super::{apply_patch, CommandResponse};
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub struct ShellCommandHandler;

#[async_trait::async_trait]
impl ToolHandler for ShellCommandHandler {
    fn tool_name(&self) -> &'static str {
        "shell_command"
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    async fn is_mutating(&self, call: &ToolCall, ctx: &ToolContext) -> bool {
        !looks_read_only_with_root(&payload_command_line(&call.payload), &ctx.session_dir)
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let response = execute_async_with_shell(
            &payload_command_line(&call.payload),
            &ctx.session_dir,
            120,
            "shell_command",
            &ctx,
        )
        .await;
        let success = response.success;
        Ok(FunctionToolOutput::from_value(
            response.output,
            Some(success),
        ))
    }
}

#[derive(Clone, Debug)]
struct ShellRequest {
    command: String,
    cwd: PathBuf,
    timeout_secs: u64,
}

pub fn execute(command_line: &str, session_dir: &Path, timeout_secs: u64) -> CommandResponse {
    execute_with_shell(command_line, session_dir, timeout_secs, "shell_command")
}

pub(super) fn execute_with_shell(
    command_line: &str,
    session_dir: &Path,
    timeout_secs: u64,
    shell_kind: &str,
) -> CommandResponse {
    let request = parse_shell_request(command_line, session_dir, timeout_secs);
    if let Some(patch_text) = embedded_apply_patch_text(&request.command) {
        return apply_patch::execute(&patch_text, session_dir);
    }
    let use_bash =
        shell_kind == "bash" || (cfg!(windows) && looks_posix_shell_script(&request.command));
    let mut command = if use_bash {
        let bash = bash_executable();
        let mut command = Command::new(bash);
        command
            .arg("-lc")
            .arg(normalize_bash_command(&request.command));
        command
    } else if cfg!(windows) {
        let mut command = Command::new(powershell_executable());
        command
            .arg("-NoProfile")
            .arg("-Command")
            .arg(&request.command);
        command
    } else {
        let mut command = Command::new("/bin/bash");
        command.arg("-lc").arg(&request.command);
        command
    };
    command.current_dir(&request.cwd);

    run_command_with_timeout(command, request.timeout_secs)
}

pub(super) async fn execute_async_with_shell(
    command_line: &str,
    session_dir: &Path,
    timeout_secs: u64,
    shell_kind: &str,
    ctx: &ToolContext,
) -> CommandResponse {
    let request = parse_shell_request(command_line, session_dir, timeout_secs);
    if let Some(patch_text) = embedded_apply_patch_text(&request.command) {
        return apply_patch::execute(&patch_text, session_dir);
    }
    if ctx.cancellation.is_cancelled() {
        return failed_async_response("tool task aborted", -1);
    }
    let use_bash =
        shell_kind == "bash" || (cfg!(windows) && looks_posix_shell_script(&request.command));
    let mut command = if use_bash {
        let bash = bash_executable();
        let mut command = tokio::process::Command::new(bash);
        command
            .arg("-lc")
            .arg(normalize_bash_command(&request.command));
        command
    } else if cfg!(windows) {
        let mut command = tokio::process::Command::new(powershell_executable());
        command
            .arg("-NoProfile")
            .arg("-Command")
            .arg(prefix_powershell_script_with_utf8(&request.command));
        command
    } else {
        let mut command = tokio::process::Command::new("/bin/bash");
        command.arg("-lc").arg(&request.command);
        command
    };
    command.current_dir(&request.cwd);
    run_tokio_command_with_timeout(command, request.timeout_secs, ctx).await
}

fn run_command_with_timeout(mut command: Command, timeout_secs: u64) -> CommandResponse {
    let started = Instant::now();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut command);
    match command.spawn() {
        Ok(mut child) => loop {
            match child.try_wait() {
                Ok(Some(_status)) => match child.wait_with_output() {
                    Ok(output) => {
                        let wall = started.elapsed().as_secs_f32();
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let exit_code = output.status.code().unwrap_or(1);
                        let mut text = format!(
                            "Exit code: {exit_code}\nWall time: {:.1} seconds\nOutput:\n{}",
                            wall, stdout
                        );
                        if !stderr.is_empty() {
                            text.push_str("\nStderr:\n");
                            text.push_str(&stderr);
                        }
                        return CommandResponse {
                            success: output.status.success(),
                            exit_code,
                            stdout,
                            stderr,
                            output: Value::String(text),
                            changes: Vec::new(),
                        };
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
                },
                Ok(None) => {
                    if started.elapsed() >= Duration::from_secs(timeout_secs) {
                        kill_child_process_tree(child.id());
                        let _ = child.kill();
                        let _ = child.wait();
                        return CommandResponse {
                            success: false,
                            exit_code: -1,
                            stdout: String::new(),
                            stderr: format!("Timed out after {timeout_secs} seconds"),
                            output: Value::String(format!(
                                "Timed out after {timeout_secs} seconds"
                            )),
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
        },
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

async fn run_tokio_command_with_timeout(
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

fn failed_async_response(message: &str, exit_code: i32) -> CommandResponse {
    CommandResponse {
        success: false,
        exit_code,
        stdout: String::new(),
        stderr: message.to_string(),
        output: Value::String(message.to_string()),
        changes: Vec::new(),
    }
}

fn configure_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
    let _ = command;
}

fn configure_tokio_process_group(command: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
    let _ = command;
}

fn kill_child_process_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let group = format!("-{}", pid);
        let _ = Command::new("kill")
            .args(["-TERM", &group])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        thread::sleep(Duration::from_millis(100));
        let _ = Command::new("kill")
            .args(["-KILL", &group])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

pub fn looks_read_only(command_line: &str) -> bool {
    looks_read_only_with_root(command_line, Path::new("."))
}

fn looks_read_only_with_root(command_line: &str, root: &Path) -> bool {
    let request = parse_shell_request(command_line, root, 120);
    let command = request.command.trim_start();
    let lower = command.to_ascii_lowercase();
    let tokens = lower
        .split_whitespace()
        .map(|token| token.trim_matches(&['"', '\''][..]))
        .collect::<Vec<_>>();
    let first_token = tokens.first().copied().unwrap_or_default();

    if first_token == "git" {
        return git_command_line_is_read_only(&tokens) && !contains_shell_write_operator(&lower);
    }

    matches!(
        first_token,
        "rg" | "grep"
            | "find"
            | "fd"
            | "ls"
            | "dir"
            | "pwd"
            | "cat"
            | "type"
            | "get-content"
            | "select-string"
            | "get-childitem"
            | "get-location"
            | "test-path"
            | "where-object"
    ) && !contains_shell_write_operator(&lower)
}

fn payload_command_line(payload: &ToolPayload) -> String {
    match payload {
        ToolPayload::Function { arguments } => {
            if arguments.is_object() {
                serde_json::to_string(arguments).unwrap_or_default()
            } else {
                arguments.as_str().unwrap_or_default().to_string()
            }
        }
        ToolPayload::Freeform { input } => input.clone(),
    }
}

fn prefix_powershell_script_with_utf8(script: &str) -> String {
    if script.contains("[Console]::OutputEncoding") {
        script.to_string()
    } else {
        format!(
            "[Console]::InputEncoding=[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new(); $OutputEncoding=[Console]::OutputEncoding; {script}"
        )
    }
}

pub fn display_command(command_line: &str, session_dir: &Path, timeout_secs: u64) -> String {
    parse_shell_request(command_line, session_dir, timeout_secs).command
}

fn bash_executable() -> PathBuf {
    if !cfg!(windows) {
        return PathBuf::from("/bin/bash");
    }
    [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
        "bash",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|path| path == Path::new("bash") || path.exists())
    .unwrap_or_else(|| PathBuf::from("bash"))
}

fn powershell_executable() -> PathBuf {
    if !cfg!(windows) {
        return PathBuf::from("pwsh");
    }
    [
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        "pwsh",
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        "powershell",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|path| path == Path::new("pwsh") || path == Path::new("powershell") || path.exists())
    .unwrap_or_else(|| PathBuf::from("powershell"))
}

fn normalize_bash_command(command: &str) -> String {
    if !cfg!(windows) {
        return command.to_string();
    }

    let mut normalized = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(index) = rest.find("/mnt/") {
        normalized.push_str(&rest[..index]);
        rest = &rest[index + "/mnt/".len()..];
        let Some(drive) = rest.chars().next() else {
            normalized.push_str("/mnt/");
            break;
        };
        let drive_len = drive.len_utf8();
        let after_drive = &rest[drive_len..];
        if drive.is_ascii_alphabetic() && after_drive.starts_with('/') {
            normalized.push(drive.to_ascii_uppercase());
            normalized.push(':');
            rest = after_drive;
        } else {
            normalized.push_str("/mnt/");
        }
    }
    normalized.push_str(rest);
    normalized
}

fn looks_posix_shell_script(command: &str) -> bool {
    let text = command.trim_start();
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("powershell ")
        || lower.starts_with("powershell.exe ")
        || lower.starts_with("pwsh ")
        || lower.starts_with("pwsh.exe ")
        || (lower.starts_with('"')
            && (lower.contains("powershell.exe\"") || lower.contains("pwsh.exe\"")))
    {
        return false;
    }

    lower.starts_with("python - <<")
        || lower.contains(" python - <<")
        || lower.starts_with("python3 - <<")
        || lower.contains(" python3 - <<")
        || lower.starts_with("pythonpath=")
        || lower.contains(" pythonpath=")
        || lower.contains("; do ")
        || lower.contains("; done")
        || (lower.starts_with("for ") && lower.contains(" in ") && lower.contains(" do "))
        || lower.contains(" && sed ")
        || lower.contains(" && cat ")
        || lower.contains("$(basename ")
        || lower.contains("#!/usr/bin/env bash")
        || lower.contains("#!/bin/bash")
}

fn parse_shell_request(
    command_line: &str,
    session_dir: &Path,
    default_timeout_secs: u64,
) -> ShellRequest {
    let text = command_line.trim();
    if text.starts_with('{') || text.starts_with('"') || text.starts_with('\'') {
        if let Some(value) = parse_shell_request_json(text) {
            if let Some(command) = value
                .get("command")
                .or_else(|| value.get("cmd"))
                .and_then(Value::as_str)
            {
                let timeout_secs = value
                    .get("timeout_secs")
                    .and_then(Value::as_u64)
                    .or_else(|| {
                        value
                            .get("timeout_ms")
                            .and_then(Value::as_u64)
                            .map(|ms| ms.div_ceil(1000).max(1))
                    })
                    .unwrap_or(default_timeout_secs);
                let cwd = value
                    .get("workdir")
                    .or_else(|| value.get("cwd"))
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .map(|path| {
                        if path.is_absolute() {
                            path
                        } else {
                            session_dir.join(path)
                        }
                    })
                    .unwrap_or_else(|| session_dir.to_path_buf());
                return ShellRequest {
                    command: normalize_shell_command_text(command),
                    cwd,
                    timeout_secs,
                };
            }
        }
    }
    ShellRequest {
        command: normalize_shell_command_text(command_line),
        cwd: session_dir.to_path_buf(),
        timeout_secs: default_timeout_secs,
    }
}

fn normalize_shell_command_text(command: &str) -> String {
    let trimmed = command.trim_start();
    for prefix in ["command:", "cmd:", "shell:", "bash:"] {
        if trimmed
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            return trimmed[prefix.len()..].trim_start().to_string();
        }
    }
    let normalized_lines = command
        .lines()
        .map(|line| {
            let line_trimmed = line.trim_start();
            for prefix in ["command:", "cmd:", "shell:", "bash:"] {
                if line_trimmed
                    .get(..prefix.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
                {
                    let leading_len = line.len().saturating_sub(line_trimmed.len());
                    return format!(
                        "{}{}",
                        &line[..leading_len],
                        line_trimmed[prefix.len()..].trim_start()
                    );
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    if command.ends_with('\n') {
        format!("{normalized_lines}\n")
    } else {
        normalized_lines
    }
}

fn embedded_apply_patch_text(command: &str) -> Option<String> {
    let begin = command.find("*** Begin Patch")?;
    let after_begin = &command[begin..];
    let end_relative = after_begin.find("*** End Patch")?;
    let end = begin + end_relative + "*** End Patch".len();
    let patch = &command[begin..end];
    if command[..begin].contains("cat ")
        || command[..begin].contains("Get-Content")
        || command[..begin].contains("grep ")
        || command[..begin].contains("rg ")
    {
        return None;
    }
    Some(patch.trim().to_string())
}

fn parse_shell_request_json(text: &str) -> Option<Value> {
    fn parse_candidate(candidate: &str, depth: usize) -> Option<Value> {
        if depth > 3 {
            return None;
        }
        match serde_json::from_str::<Value>(candidate).ok()? {
            Value::String(inner) => parse_candidate(inner.trim(), depth + 1),
            value => Some(value),
        }
    }

    let trimmed = text.trim();
    parse_candidate(trimmed, 0)
        .or_else(|| parse_candidate(&format!("\"{trimmed}\""), 0))
        .or_else(|| parse_loose_shell_request_object(trimmed))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
                .and_then(|inner| parse_candidate(inner.trim(), 0))
        })
        .or_else(|| {
            if trimmed.contains("\\\"") {
                parse_candidate(&trimmed.replace("\\\"", "\""), 0)
            } else {
                None
            }
        })
}

fn parse_loose_shell_request_object(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }

    let command = loose_json_string_field(trimmed, "command")
        .or_else(|| loose_json_string_field(trimmed, "cmd"))?;
    let mut object = serde_json::Map::new();
    object.insert("command".to_string(), Value::String(command));
    if let Some(workdir) = loose_json_string_field(trimmed, "workdir") {
        object.insert("workdir".to_string(), Value::String(workdir));
    }
    if let Some(timeout_ms) = loose_json_number_field(trimmed, "timeout_ms") {
        object.insert("timeout_ms".to_string(), Value::Number(timeout_ms.into()));
    }
    if let Some(timeout_secs) = loose_json_number_field(trimmed, "timeout_secs") {
        object.insert(
            "timeout_secs".to_string(),
            Value::Number(timeout_secs.into()),
        );
    }
    Some(Value::Object(object))
}

fn loose_json_string_field(text: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\":\"");
    let start = text.find(&marker)? + marker.len();
    let raw = loose_json_string_field_raw(&text[start..])?;
    decode_loose_json_string(raw)
}

fn loose_json_string_field_raw(rest: &str) -> Option<&str> {
    let bytes = rest.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let mut slash_count = 0;
            let mut cursor = index;
            while cursor > 0 && bytes[cursor - 1] == b'\\' {
                slash_count += 1;
                cursor -= 1;
            }
            if slash_count % 2 == 0 {
                let after = &rest[index + 1..];
                if after.trim_start().starts_with(',')
                    || after.trim_start().starts_with('}')
                    || after.trim_start().is_empty()
                {
                    return Some(&rest[..index]);
                }
            }
        }
        index += 1;
    }
    None
}

fn loose_json_number_field(text: &str, field: &str) -> Option<u64> {
    let marker = format!("\"{field}\":");
    let start = text.find(&marker)? + marker.len();
    let rest = text[start..].trim_start();
    let digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty())
        .then(|| digits.parse::<u64>().ok())
        .flatten()
}

fn decode_loose_json_string(raw: &str) -> Option<String> {
    let mut decoded = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        let Some(next) = chars.next() else {
            decoded.push('\\');
            break;
        };
        match next {
            '"' => decoded.push('"'),
            '\\' => decoded.push('\\'),
            '/' => decoded.push('/'),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            'b' => decoded.push('\u{0008}'),
            'f' => decoded.push('\u{000c}'),
            'u' => {
                let digits = chars.by_ref().take(4).collect::<String>();
                if let Ok(code) = u16::from_str_radix(&digits, 16) {
                    if let Some(value) = char::from_u32(code as u32) {
                        decoded.push(value);
                    }
                }
            }
            other => {
                decoded.push('\\');
                decoded.push(other);
            }
        }
    }
    Some(decoded)
}

fn git_command_line_is_read_only(tokens: &[&str]) -> bool {
    let mut index = 1;
    while index < tokens.len() {
        let token = tokens[index];
        match token {
            "-c" | "-C" | "--git-dir" | "--work-tree" => {
                index += 2;
            }
            token if token.starts_with('-') => {
                index += 1;
            }
            "status" | "diff" | "show" | "log" | "ls-files" | "grep" | "rev-parse" | "describe"
            | "blame" => {
                return true;
            }
            "branch" => {
                return tokens
                    .iter()
                    .skip(index + 1)
                    .any(|token| matches!(*token, "--show-current" | "--list" | "-a" | "-r"));
            }
            _ => return false,
        }
    }

    false
}

fn contains_shell_write_operator(command: &str) -> bool {
    command.contains(" >")
        || command.contains(">>")
        || command.contains("set-content")
        || command.contains("out-file")
        || command.contains("new-item")
        || command.contains("remove-item")
        || command.contains("move-item")
        || command.contains("copy-item")
        || command.contains("tee-object")
        || command.contains("apply_patch")
        || command.contains("cargo test")
        || command.contains("cargo build")
        || command.contains("cargo check")
}

#[cfg(test)]
mod tests {
    use super::{
        embedded_apply_patch_text, execute_with_shell, looks_posix_shell_script,
        normalize_bash_command, parse_shell_request,
    };
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn parses_json_shell_request_with_escaped_quotes() {
        let request = parse_shell_request(
            r#"{\"command\":\"Write-Output ok\",\"workdir\":\"subdir\",\"timeout_ms\":1500}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "Write-Output ok");
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 2);
    }

    #[test]
    fn strips_current_style_shell_text_prefixes() {
        let request = parse_shell_request(
            r#"{"command":"command:rg -n symbol src","workdir":"subdir","timeout_ms":1500}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "rg -n symbol src");
        assert!(request.cwd.ends_with("subdir"));
    }

    #[test]
    fn strips_current_style_shell_text_prefixes_inside_multiline_scripts() {
        let request = parse_shell_request(
            "echo before\ncommand:for i in 1 2; do echo $i; done\n",
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(
            request.command,
            "echo before\nfor i in 1 2; do echo $i; done\n"
        );
    }

    #[test]
    fn extracts_apply_patch_embedded_in_shell_wrapper() {
        let patch = embedded_apply_patch_text(
            "@'\n*** Begin Patch\n*** Update File: src/app.txt\n@@\n-old\n+new\n*** End Patch\n'@ | apply_patch",
        )
        .expect("patch should be extracted");

        assert_eq!(
            patch,
            "*** Begin Patch\n*** Update File: src/app.txt\n@@\n-old\n+new\n*** End Patch"
        );
    }

    #[test]
    fn does_not_extract_patch_from_read_only_text_output() {
        assert!(
            embedded_apply_patch_text("cat <<'EOF'\n*** Begin Patch\n*** End Patch\nEOF").is_none()
        );
    }

    #[test]
    fn parses_escaped_json_shell_request_with_inner_command_quotes() {
        let request = parse_shell_request(
            r#"{\"command\":\"rg -n \\\"def close_month|score_policy\\\" src/retail_core\",\"workdir\":\"subdir\",\"timeout_ms\":120000}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(
            request.command,
            r#"rg -n "def close_month|score_policy" src/retail_core"#
        );
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 120);
    }

    #[test]
    fn parses_json_shell_request_wrapped_as_json_string() {
        let request = parse_shell_request(
            r#""{\"command\":\"Write-Output ok\",\"workdir\":\"subdir\",\"timeout_ms\":1500}""#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "Write-Output ok");
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 2);
    }

    #[test]
    fn parses_escaped_json_request_with_here_string_command() {
        let request = parse_shell_request(
            r#"{\"command\":\"@'\\nprint(1)\\n'@ | python -\",\"workdir\":\"subdir\",\"timeout_ms\":10000}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert!(request.command.starts_with("@'"));
        assert!(request.command.contains("python -"));
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 10);
    }

    #[test]
    fn parses_loose_json_request_with_raw_multiline_command() {
        let request = parse_shell_request(
            "{\"command\":\"@'\nprint(\\\"ok\\\")\n'@ | python -\",\"workdir\":\"subdir\",\"timeout_ms\":10000}",
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "@'\nprint(\"ok\")\n'@ | python -");
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 10);
    }

    #[test]
    fn parses_loose_json_request_with_regex_backslashes() {
        let request = parse_shell_request(
            r#"{"command":"rg -n \"toFixed\(1\)|count \+ 2\" frontend/src/views","workdir":"subdir","timeout_ms":10000}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(
            request.command,
            r#"rg -n "toFixed\(1\)|count \+ 2" frontend/src/views"#
        );
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 10);
    }

    #[test]
    fn accepts_codex_command_run_cmd_alias() {
        let request = parse_shell_request(
            r#"{\"cmd\":\"Write-Output ok\",\"workdir\":\"subdir\",\"timeout_ms\":1500}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "Write-Output ok");
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 2);
    }

    #[test]
    fn raw_shell_text_stays_raw() {
        let request = parse_shell_request("rg -n needle src", Path::new("C:/workspace"), 120);

        assert_eq!(request.command, "rg -n needle src");
        assert_eq!(request.timeout_secs, 120);
    }

    #[test]
    fn windows_bash_command_normalizes_wsl_mount_paths() {
        let command = "cd /mnt/c/Users/example/project && python - <<'PY'\nprint('ok')\nPY";

        let normalized = normalize_bash_command(command);

        if cfg!(windows) {
            assert!(normalized.starts_with("cd C:/Users/example/project"));
        } else {
            assert_eq!(normalized, command);
        }
    }

    #[test]
    fn detects_posix_shell_scripts_sent_to_shell_command() {
        assert!(looks_posix_shell_script(
            "for f in src/*.py; do sed -n '1,20p' \"$f\"; done"
        ));
        assert!(looks_posix_shell_script(
            "PYTHONPATH=src python - <<'PY'\nprint('ok')\nPY"
        ));
        assert!(!looks_posix_shell_script(
            "Get-Content -Raw src/app.txt; Write-Output ok"
        ));
        assert!(!looks_posix_shell_script(
            "$env:PYTHONPATH='src'; python -c \"print('ok')\""
        ));
        assert!(!looks_posix_shell_script(
            "\"C:\\Program Files\\PowerShell\\7\\pwsh.exe\" -Command 'for f in *.py; do echo $f; done'"
        ));
    }

    #[test]
    fn timeout_kills_descendants_that_hold_output_pipes() {
        let started = Instant::now();
        let response = execute_with_shell(
            r#"{"command":"sh -c 'sleep 10 & wait'","timeout_ms":1000}"#,
            Path::new("."),
            120,
            "bash",
        );

        assert!(!response.success);
        assert_eq!(response.exit_code, -1);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timeout should not wait for orphaned descendants"
        );
    }
}
