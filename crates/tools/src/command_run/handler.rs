use crate::runtime::tool::{ToolCall, ToolContext, ToolPayload, ToolRouter};
use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 15_000;
const APPLY_PATCH_FAILURE_CANCEL_REASON: &str =
    "apply_patch failed; command_run stopped before later commands";

#[derive(Clone, Debug)]
struct CommandRunArgs {
    commands: Vec<CommandItem>,
    workdir: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct CommandItem {
    index: usize,
    command: String,
    command_line: String,
    inline_arguments: Option<Value>,
    workdir: Option<String>,
    step: Option<u64>,
    timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
struct CommandRunItemResult {
    #[serde(skip)]
    index: usize,
    step: u64,
    command_type: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CommandRunOutput {
    results: Vec<CommandRunItemResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cancelled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cancel_reason: Option<String>,
}

#[derive(Clone, Debug)]
struct CommandRunStepOutput {
    results: Vec<CommandRunItemResult>,
    cancelled: bool,
    cancel_reason: Option<String>,
}

pub fn execute(arguments: &Value, session_dir: &Path) -> Value {
    if tokio::runtime::Handle::try_current().is_ok() {
        let arguments = arguments.clone();
        let session_dir = session_dir.to_path_buf();
        return std::thread::spawn(move || execute(&arguments, &session_dir))
            .join()
            .unwrap_or_else(|_| error_payload("command_run thread panicked".to_string()));
    }

    let args = match parse_args(arguments) {
        Ok(args) => args,
        Err(message) => return error_payload(message),
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => return error_payload(format!("failed to create tool runtime: {err}")),
    };

    runtime.block_on(async move { execute_async_args(args, session_dir.to_path_buf()).await })
}

pub async fn execute_async_value(arguments: Value, session_dir: std::path::PathBuf) -> Value {
    let args = match parse_args(&arguments) {
        Ok(args) => args,
        Err(message) => return error_payload(message),
    };
    execute_async_args(args, session_dir).await
}

pub async fn execute_streamed_command_value(
    command: Value,
    session_dir: std::path::PathBuf,
) -> Value {
    let arguments = json!({ "commands": [command] });
    let args = match parse_args(&arguments) {
        Ok(args) => args,
        Err(message) => return error_payload(message),
    };
    execute_async_args(args, session_dir).await
}

pub struct StreamingCommandRunExecutor {
    router: Arc<ToolRouter>,
    ctx: ToolContext,
    active_step: Option<u64>,
    next_index: usize,
    parallel_reads: FuturesUnordered<tokio::task::JoinHandle<CommandRunItemResult>>,
    results: Vec<CommandRunItemResult>,
    halted: bool,
    halt_reason: Option<String>,
}

impl StreamingCommandRunExecutor {
    pub fn new(session_dir: std::path::PathBuf) -> Self {
        Self {
            router: Arc::new(ToolRouter::new()),
            ctx: ToolContext::new(session_dir),
            active_step: None,
            next_index: 0,
            parallel_reads: FuturesUnordered::new(),
            results: Vec::new(),
            halted: false,
            halt_reason: None,
        }
    }

    pub async fn push_command_value(&mut self, command: Value) -> Vec<Value> {
        if self.halted {
            return Vec::new();
        }
        let mut command = match parse_single_streamed_command(command, self.next_index) {
            Ok(command) => command,
            Err((step, message)) => {
                self.results.push(CommandRunItemResult::failed(
                    self.next_index,
                    step,
                    "command_run".to_string(),
                    message,
                ));
                self.next_index += 1;
                return self.drain_finished_results();
            }
        };
        command.index = self.next_index;
        self.next_index += 1;

        let step = command.effective_step();
        if self.active_step.is_some_and(|current| step != current) {
            self.flush_parallel_reads().await;
        }
        self.active_step = Some(step);

        let parallel_safe = command
            .is_parallel_safe_read(&self.router, &self.ctx.child())
            .await;
        if parallel_safe {
            let router = Arc::clone(&self.router);
            let ctx = self.ctx.child();
            self.parallel_reads.push(tokio::spawn(async move {
                run_command_run_item(&router, command, ctx, false).await
            }));
            return self.drain_finished_results();
        }

        self.flush_parallel_reads().await;
        let result = run_command_run_item(&self.router, command, self.ctx.child(), true).await;
        let should_halt = is_failed_apply_patch_result(&result);
        self.results.push(result);
        if should_halt {
            self.halt_after_apply_patch_failure();
        }
        self.drain_finished_results()
    }

    pub async fn finish(mut self) -> Vec<Value> {
        if self.halted {
            return self.drain_finished_results();
        }
        self.flush_parallel_reads().await;
        self.drain_finished_results()
    }

    pub fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn halt_reason(&self) -> Option<&str> {
        self.halt_reason.as_deref()
    }

    async fn flush_parallel_reads(&mut self) {
        while let Some(result) = self.parallel_reads.next().await {
            match result {
                Ok(result) => self.results.push(result),
                Err(err) => self.results.push(CommandRunItemResult::failed(
                    self.next_index,
                    self.active_step.unwrap_or(1),
                    "command_run".to_string(),
                    format!("streamed command task failed: {err}"),
                )),
            }
        }
    }

    pub fn drain_finished_results(&mut self) -> Vec<Value> {
        self.results
            .sort_by_key(|result| (result.step, result.index));
        std::mem::take(&mut self.results)
            .into_iter()
            .map(|result| {
                serde_json::to_value(result).unwrap_or_else(|err| error_payload(err.to_string()))
            })
            .collect()
    }

    fn halt_after_apply_patch_failure(&mut self) {
        self.halted = true;
        self.halt_reason = Some(APPLY_PATCH_FAILURE_CANCEL_REASON.to_string());
        self.ctx.cancellation.cancel();
    }
}

fn parse_single_streamed_command(
    command: Value,
    index: usize,
) -> Result<CommandItem, (u64, String)> {
    let step = command
        .get("step")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let arguments = json!({ "commands": [command] });
    let mut args = parse_args(&arguments).map_err(|message| (step, message))?;
    let mut item = args
        .commands
        .pop()
        .ok_or_else(|| (step, "command_run commands must not be empty".to_string()))?;
    item.index = index;
    Ok(item)
}

async fn execute_async_args(args: CommandRunArgs, session_dir: std::path::PathBuf) -> Value {
    let ctx = ToolContext::new(session_dir);
    let output = execute_async(args, ctx).await;
    serde_json::to_value(output).unwrap_or_else(|err| error_payload(err.to_string()))
}

async fn execute_async(args: CommandRunArgs, ctx: ToolContext) -> CommandRunOutput {
    let mut by_step: BTreeMap<u64, Vec<CommandItem>> = BTreeMap::new();
    for command in args.commands {
        by_step
            .entry(command.effective_step())
            .or_default()
            .push(command);
    }

    let router = ToolRouter::new();
    let mut results = Vec::new();
    let mut cancelled = false;
    let mut cancel_reason = None;
    for commands in by_step.into_values() {
        let step_output = run_command_run_step(&router, commands, ctx.child()).await;
        results.extend(step_output.results);
        if step_output.cancelled {
            cancelled = true;
            cancel_reason = step_output.cancel_reason;
            ctx.cancellation.cancel();
            break;
        }
    }
    results.sort_by_key(|result| (result.step, result.index));
    CommandRunOutput {
        results,
        cancelled: cancelled.then_some(true),
        cancel_reason,
    }
}

async fn run_command_run_step(
    router: &ToolRouter,
    commands: Vec<CommandItem>,
    ctx: ToolContext,
) -> CommandRunStepOutput {
    let mut results = Vec::new();
    let mut parallel_reads = Vec::new();

    for command in commands {
        let force_exclusive = !command.is_parallel_safe_read(router, &ctx).await;
        if !force_exclusive {
            parallel_reads.push(command);
            continue;
        }

        results.extend(
            run_parallel_items(router, std::mem::take(&mut parallel_reads), ctx.child()).await,
        );
        let result = run_command_run_item(router, command, ctx.child(), true).await;
        let should_stop = is_failed_apply_patch_result(&result);
        results.push(result);
        if should_stop {
            ctx.cancellation.cancel();
            return CommandRunStepOutput {
                results,
                cancelled: true,
                cancel_reason: Some(APPLY_PATCH_FAILURE_CANCEL_REASON.to_string()),
            };
        }
    }

    results.extend(run_parallel_items(router, parallel_reads, ctx).await);
    CommandRunStepOutput {
        results,
        cancelled: false,
        cancel_reason: None,
    }
}

fn is_failed_apply_patch_result(result: &CommandRunItemResult) -> bool {
    result.command_type == "apply_patch" && !result.success
}

async fn run_parallel_items(
    router: &ToolRouter,
    commands: Vec<CommandItem>,
    ctx: ToolContext,
) -> Vec<CommandRunItemResult> {
    if commands.is_empty() {
        return Vec::new();
    }

    let mut in_flight = FuturesUnordered::new();
    for command in commands {
        in_flight.push(run_command_run_item(router, command, ctx.child(), false));
    }
    let mut results = Vec::new();
    while let Some(result) = in_flight.next().await {
        results.push(result);
    }
    results.sort_by_key(|result| (result.step, result.index));
    results
}

async fn run_command_run_item(
    router: &ToolRouter,
    command: CommandItem,
    ctx: ToolContext,
    force_exclusive: bool,
) -> CommandRunItemResult {
    if crate::commands::canonical_command(&command.command) == "task_status" {
        return command_run_task_status_result(command);
    }
    let command_name = match router.resolve_command_tool_name(&command.command) {
        Some(name) => name.to_string(),
        None => {
            return CommandRunItemResult::failed(
                command.index,
                command.effective_step(),
                command.command,
                "unsupported command_run command".to_string(),
            );
        }
    };
    let call = match build_tool_call(&command_name, &command) {
        Ok(call) => call,
        Err(message) => {
            return CommandRunItemResult::failed(
                command.index,
                command.effective_step(),
                command_name,
                message,
            );
        }
    };
    match router.dispatch(call, ctx, force_exclusive).await {
        Ok(result) => CommandRunItemResult {
            index: command.index,
            step: command.effective_step(),
            command_type: command_name.clone(),
            success: result.result.success_for_logging(),
            output: Some(command_run_model_output(
                &command_name,
                result.result.code_mode_result(),
            )),
            error: None,
        },
        Err(err) => CommandRunItemResult::failed(
            command.index,
            command.effective_step(),
            command_name,
            err.to_string(),
        ),
    }
}

fn command_run_task_status_result(command: CommandItem) -> CommandRunItemResult {
    match normalize_task_status_output(&command) {
        Ok(output) => CommandRunItemResult {
            index: command.index,
            step: command.effective_step(),
            command_type: "task_status".to_string(),
            success: true,
            output: Some(output),
            error: None,
        },
        Err(error) => CommandRunItemResult::failed(
            command.index,
            command.effective_step(),
            "task_status".to_string(),
            error,
        ),
    }
}

fn normalize_task_status_output(command: &CommandItem) -> Result<Value, String> {
    let mut value = command
        .inline_arguments
        .clone()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let trimmed = command.command_line.trim();
    if !trimmed.is_empty() {
        value = if trimmed.starts_with('{') {
            serde_json::from_str(trimmed)
                .map_err(|err| format!("invalid task_status command_line JSON: {err}"))?
        } else {
            parse_task_status_text(trimmed)
        };
    }
    let Some(object) = value.as_object() else {
        return Err("task_status expects an object".to_string());
    };
    let status = string_field(object, &["status", "task_status"]).map(|status| {
        status
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .to_string()
    });
    if let Some(status) = status.as_deref() {
        if !matches!(status, "question" | "done") {
            return Err("task_status status must be question or done".to_string());
        }
    }
    let task_summary = string_field(object, &["task_summary"]);
    Ok(json!({
        "task_status": {
            "status": status,
            "task_summary": task_summary,
        }
    }))
}

fn parse_task_status_text(text: &str) -> Value {
    let status = text
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '=' | ',' | ';'))
        .find_map(|part| {
            let part = part.trim().to_ascii_lowercase().replace('-', "_");
            matches!(part.as_str(), "question" | "done").then_some(part)
        });
    json!({ "status": status })
}

fn command_run_model_output(command_name: &str, value: Value) -> Value {
    if !matches!(command_name, "shell_command" | "bash") {
        return value;
    }
    value
        .as_object()
        .and_then(|object| object.get("transcript"))
        .and_then(Value::as_str)
        .map(|text| Value::String(text.to_string()))
        .unwrap_or(value)
}

fn build_tool_call(command_name: &str, command: &CommandItem) -> Result<ToolCall, String> {
    let payload = match command_name {
        "apply_patch" => ToolPayload::Freeform {
            input: command.command_line.clone(),
        },
        "compact_context" => ToolPayload::Function {
            arguments: normalize_compact_context_arguments(command)?,
        },
        "multiple_tasks" => ToolPayload::Function {
            arguments: normalize_multiple_tasks_arguments(command)?,
        },
        "read_media" => ToolPayload::Function {
            arguments: normalize_json_or_cli_command_arguments(command, "read_media")?,
        },
        "web_discover" => ToolPayload::Function {
            arguments: normalize_json_or_cli_command_arguments(command, "web_discover")?,
        },
        _ => ToolPayload::Function {
            arguments: normalize_shell_command_arguments(command)?,
        },
    };
    Ok(ToolCall {
        tool_name: command_name.to_string(),
        call_id: format!("command_run:{}:{}", command.effective_step(), command.index),
        payload,
    })
}

fn normalize_shell_command_arguments(command: &CommandItem) -> Result<Value, String> {
    let trimmed = command.command_line.trim();
    if trimmed.starts_with('{') {
        let mut value: Value = serde_json::from_str(trimmed)
            .map_err(|err| format!("invalid command_line JSON: {err}"))?;
        if let Value::Object(ref mut object) = value {
            if let Some(workdir) = command.workdir.as_deref() {
                object
                    .entry("workdir".to_string())
                    .or_insert_with(|| Value::String(workdir.to_string()));
            }
            object
                .entry("timeout_ms".to_string())
                .or_insert_with(|| json!(command.timeout_ms.unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS)));
        }
        return Ok(value);
    }
    let mut arguments = json!({
        "command": command.command_line,
        "timeout_ms": command.effective_timeout_ms(),
    });
    if let (Some(workdir), Some(object)) = (command.workdir.as_deref(), arguments.as_object_mut()) {
        object.insert("workdir".to_string(), Value::String(workdir.to_string()));
    }
    Ok(arguments)
}

fn parse_args(arguments: &Value) -> Result<CommandRunArgs, String> {
    let arguments = parse_arguments_value(arguments)?;
    let Some(object) = arguments.as_object() else {
        return Err("failed to parse command_run arguments: expected object".to_string());
    };
    let top_workdir = string_field(object, &["workdir", "cwd"]);
    let top_timeout_ms = u64_field(object, &["timeout_ms", "timeoutMs"]);
    let command_values = if let Some(commands) = object.get("commands") {
        command_values(commands)
    } else if let Some(steps) = object.get("steps") {
        command_values(steps)
    } else {
        vec![arguments.clone()]
    };
    let mut args = CommandRunArgs {
        commands: Vec::new(),
        workdir: top_workdir,
        timeout_ms: top_timeout_ms,
    };
    for value in command_values {
        args.commands.push(parse_command_item(&value)?);
    }
    if args.commands.is_empty() {
        return Err("command_run commands must not be empty".to_string());
    }
    for (index, command) in args.commands.iter_mut().enumerate() {
        command.index = index;
        if command.workdir.is_none() {
            command.workdir = args.workdir.clone();
        }
        if command.timeout_ms.is_none() {
            command.timeout_ms = args.timeout_ms;
        }
        if let Some(patch) = command
            .command_line
            .is_empty()
            .then(|| extract_apply_patch_body(&command.command))
            .flatten()
            .or_else(|| {
                let trimmed = command.command_line.trim_start();
                (!trimmed.starts_with('{') && command.command_line.contains('\n'))
                    .then(|| extract_apply_patch_body(&command.command_line))
                    .flatten()
            })
        {
            command.command = "apply_patch".to_string();
            command.command_line = patch;
            continue;
        }
        let canonical_command = crate::commands::canonical_command(&command.command);
        if !matches!(
            canonical_command.as_str(),
            "shell_command"
                | "bash"
                | "apply_patch"
                | "multiple_tasks"
                | "read_media"
                | "web_discover"
                | "task_status"
                | "compact_context"
        ) {
            if looks_like_removed_structured_tool_call(&command.command, &command.command_line) {
                continue;
            }
            if command.command_line.is_empty() {
                command.command_line = command.command.clone();
            }
            command.command = crate::commands::active_shell_command_name().to_string();
        } else if command.command_line.is_empty() && looks_like_shell_command_text(&command.command)
        {
            command.command_line = command.command.clone();
            command.command = crate::commands::active_shell_command_name().to_string();
        }
    }
    validate_compact_context_position(&args.commands)?;
    Ok(args)
}

fn normalize_json_or_cli_command_arguments(
    command: &CommandItem,
    command_name: &str,
) -> Result<Value, String> {
    let trimmed = command.command_line.trim();
    if trimmed.is_empty() {
        if let Some(arguments) = &command.inline_arguments {
            return Ok(arguments.clone());
        }
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        normalize_json_command_arguments(command, command_name)
    } else {
        Ok(json!({ "cli": command.command_line }))
    }
}

fn validate_compact_context_position(commands: &[CommandItem]) -> Result<(), String> {
    let Some((compact_index, compact)) = commands.iter().enumerate().find(|(_, command)| {
        crate::commands::canonical_command(&command.command) == "compact_context"
    }) else {
        return Ok(());
    };
    if commands.get(compact_index + 1).is_some() {
        return Err(
            "compact_context must be the final command in the highest step of command_run"
                .to_string(),
        );
    }
    let max_step = commands
        .iter()
        .map(CommandItem::effective_step)
        .max()
        .unwrap_or(1);
    if compact.effective_step() != max_step {
        return Err(
            "compact_context must be the final command in the highest step of command_run"
                .to_string(),
        );
    }
    Ok(())
}

fn normalize_multiple_tasks_arguments(command: &CommandItem) -> Result<Value, String> {
    let trimmed = command.command_line.trim();
    if trimmed.is_empty() {
        return Err("multiple_tasks command_line must be a JSON array".to_string());
    }
    let value: Value = serde_json::from_str(trimmed)
        .map_err(|err| format!("invalid multiple_tasks command_line JSON: {err}"))?;
    Ok(value)
}

fn normalize_compact_context_arguments(command: &CommandItem) -> Result<Value, String> {
    let trimmed = command.command_line.trim();
    if trimmed.is_empty() {
        return Err("compact_context command_line must include checkpoint text".to_string());
    }
    if trimmed.starts_with('{') {
        let value: Value = serde_json::from_str(trimmed)
            .map_err(|err| format!("invalid compact_context command_line JSON: {err}"))?;
        return Ok(value);
    }
    Ok(json!({ "summary": trimmed }))
}

fn normalize_json_command_arguments(
    command: &CommandItem,
    command_name: &str,
) -> Result<Value, String> {
    let trimmed = command.command_line.trim();
    if trimmed.is_empty() {
        return Err(format!("{command_name} command_line must be JSON"));
    }
    serde_json::from_str(trimmed)
        .map_err(|err| format!("invalid {command_name} command_line JSON: {err}"))
}

fn parse_arguments_value(arguments: &Value) -> Result<Value, String> {
    let value = match arguments {
        Value::String(text) => parse_jsonish_value(text)
            .map_err(|err| format!("failed to parse command_run arguments: {err}"))?,
        other => other.clone(),
    };
    Ok(value.get("requests").cloned().unwrap_or(value))
}

fn command_values(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.clone(),
        Value::Object(_) | Value::String(_) => vec![value.clone()],
        _ => Vec::new(),
    }
}

fn parse_command_item(value: &Value) -> Result<CommandItem, String> {
    if let Some(text) = value.as_str() {
        return Ok(CommandItem {
            index: 0,
            command: text.to_string(),
            command_line: String::new(),
            inline_arguments: None,
            workdir: None,
            step: None,
            timeout_ms: None,
        });
    }
    let Some(object) = value.as_object() else {
        return Err("failed to parse command_run command: expected object".to_string());
    };
    let command = string_field(
        object,
        &[
            "command_type",
            "commandType",
            "command",
            "cmd",
            "tool",
            "name",
            "tool_name",
            "toolName",
            "tool_package_name",
            "toolPackageName",
        ],
    )
    .or_else(|| {
        string_field(
            object,
            &[
                "command_line",
                "commandLine",
                "command_code",
                "commandCode",
                "input",
                "args",
                "code",
                "script",
                "payload",
            ],
        )
        .map(|_| crate::commands::active_shell_command_name().to_string())
    })
    .ok_or_else(|| {
        "failed to parse command_run command: missing field `command_type`".to_string()
    })?;
    let command_line = string_field(
        object,
        &[
            "command_line",
            "commandLine",
            "command_code",
            "commandCode",
            "input",
            "args",
            "code",
            "script",
            "payload",
        ],
    )
    .unwrap_or_default();
    let inline_arguments = inline_command_arguments(object);
    Ok(CommandItem {
        index: 0,
        command,
        command_line,
        inline_arguments,
        workdir: string_field(object, &["workdir", "cwd"]),
        step: u64_field(object, &["step"]),
        timeout_ms: u64_field(object, &["timeout_ms", "timeoutMs"]),
    })
}

fn inline_command_arguments(object: &serde_json::Map<String, Value>) -> Option<Value> {
    for name in [
        "arguments",
        "argument",
        "parameters",
        "parameter",
        "params",
        "options",
        "input_json",
        "inputJson",
    ] {
        if let Some(value) = object.get(name) {
            return Some(value.clone());
        }
    }

    let mut arguments = object.clone();
    for name in [
        "command_type",
        "commandType",
        "command",
        "cmd",
        "tool",
        "name",
        "tool_name",
        "toolName",
        "tool_package_name",
        "toolPackageName",
        "command_line",
        "commandLine",
        "command_code",
        "commandCode",
        "input",
        "args",
        "code",
        "script",
        "payload",
        "workdir",
        "cwd",
        "step",
        "timeout_ms",
        "timeoutMs",
    ] {
        arguments.remove(name);
    }
    (!arguments.is_empty()).then_some(Value::Object(arguments))
}

fn string_field(object: &serde_json::Map<String, Value>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        object.get(*name).and_then(|value| match value {
            Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
            Value::Object(_) | Value::Array(_) => Some(value.to_string()),
            _ => None,
        })
    })
}

fn parse_jsonish_value(text: &str) -> Result<Value, serde_json::Error> {
    let trimmed = text.trim();
    if let Some(unfenced) = strip_json_code_fence(trimmed) {
        if let Ok(value) = serde_json::from_str(unfenced.trim()) {
            return Ok(value);
        }
    }
    serde_json::from_str(trimmed)
}

fn strip_json_code_fence(text: &str) -> Option<&str> {
    let stripped = text.strip_prefix("```")?;
    let newline = stripped.find('\n')?;
    let body = &stripped[newline + 1..];
    let end = body.rfind("```")?;
    Some(&body[..end])
}

fn u64_field(object: &serde_json::Map<String, Value>, names: &[&str]) -> Option<u64> {
    names.iter().find_map(|name| {
        object.get(*name).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
        })
    })
}

fn extract_apply_patch_body(text: &str) -> Option<String> {
    let begin = text.find("*** Begin Patch")?;
    let end_marker = "*** End Patch";
    let end = text[begin..].find(end_marker)? + begin + end_marker.len();
    Some(text[begin..end].trim().to_string())
}

impl CommandItem {
    fn effective_step(&self) -> u64 {
        self.step.unwrap_or((self.index + 1) as u64).max(1)
    }

    fn effective_timeout_ms(&self) -> u64 {
        self.timeout_ms.unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS).max(1)
    }

    async fn is_parallel_safe_read(&self, router: &ToolRouter, ctx: &ToolContext) -> bool {
        let Some(tool_name) = router.resolve_command_tool_name(&self.command) else {
            return false;
        };
        if tool_name == "apply_patch" {
            return false;
        }
        let Ok(call) = build_tool_call(tool_name, self) else {
            return false;
        };
        if !router.tool_supports_parallel(&call) {
            return false;
        }
        let Some(handler) = router.handler(tool_name) else {
            return false;
        };
        !handler.is_mutating(&call, ctx).await
    }
}

impl CommandRunItemResult {
    fn failed(index: usize, step: u64, command: String, error: String) -> Self {
        Self {
            index,
            step,
            command_type: command,
            success: false,
            output: None,
            error: Some(error),
        }
    }
}

fn looks_like_shell_command_text(command: &str) -> bool {
    let text = command.trim_start().to_ascii_lowercase();
    text.starts_with("powershell ")
        || text.starts_with("powershell.exe ")
        || text.starts_with("pwsh ")
        || text.starts_with("pwsh.exe ")
        || text.starts_with('"')
            && (text.contains("powershell.exe\"") || text.contains("pwsh.exe\""))
}

fn looks_like_removed_structured_tool_call(command: &str, command_line: &str) -> bool {
    let name = command.trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        return false;
    }
    let normalized = name.to_ascii_lowercase().replace(['-', ':'], "_");
    let removed_tool_name = matches!(
        normalized.as_str(),
        "read_file"
            | "read_line"
            | "read_block"
            | "glob"
            | "rg"
            | "write_file"
            | "delete_file"
            | "apply_diff"
            | "get_file_outline"
            | "find_definition"
            | "find_references"
    );
    removed_tool_name && command_line.trim_start().starts_with('{')
}

fn error_payload(message: String) -> Value {
    json!({
        "results": [
            {
                "step": 1,
                "command_type": "command_run",
                "success": false,
                "error": message
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::{
        execute, normalize_shell_command_arguments, parse_args, StreamingCommandRunExecutor,
    };
    use serde_json::json;
    use serde_json::Value;
    use std::path::Path;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn missing_steps_default_to_original_order_steps() {
        let output = execute(
            &json!({
                "commands": [
                    { "command": "shell_command", "command_line": "pwd" },
                    { "command": "shell_command", "command_line": "pwd" }
                ]
            }),
            Path::new("."),
        );
        let results = output["results"].as_array().expect("results");

        assert_eq!(results[0]["step"], 1);
        assert_eq!(results[1]["step"], 2);
    }

    #[test]
    fn empty_command_run_is_error() {
        let output = execute(&json!({ "commands": [] }), Path::new("."));

        assert_eq!(output["results"][0]["success"], false);
        assert_eq!(
            output["results"][0]["error"],
            "command_run commands must not be empty"
        );
    }

    #[test]
    fn current_style_output_has_only_results_top_level() {
        let output = execute(
            &json!({
                "commands": [
                    { "command": "shell_command", "command_line": "echo ok" }
                ]
            }),
            Path::new("."),
        );
        assert!(output.get("results").is_some());
        assert!(output.get("ok").is_none());
        assert!(output.get("output_policy").is_none());
        assert!(output.get("task_status").is_none());
        assert!(output["results"][0].get("display_command").is_none());
        assert!(output["results"][0].get("exit_code").is_none());
        assert!(output["results"][0].get("command").is_none());
        assert!(output["results"][0].get("command_type").is_some());
    }

    #[test]
    fn top_level_task_status_argument_is_ignored_and_not_model_visible() {
        let output = execute(
            &json!({
                "task_status": { "status": "done" },
                "commands": [
                    { "command": "shell_command", "command_line": "echo ok" }
                ]
            }),
            Path::new("."),
        );

        assert!(output.get("task_status").is_none());
    }

    #[test]
    fn multiple_tasks_command_routes_through_command_run() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::set_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS", "1");
        let output = execute(
            &json!({
                "commands": [
                    {
                        "command": "multiple_tasks",
                        "command_line": "[{\"nonce_id\":\"inspect\",\"step\":1,\"task_summary\":\"Inspect files\",\"delivery\":\"Read relevant files and identify edits.\"},{\"nonce_id\":\"apply\",\"step\":1,\"task_summary\":\"Apply changes\",\"delivery\":\"Patch files and verify behavior.\"}]"
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(output["results"][0]["command_type"], "multiple_tasks");
        assert_eq!(
            output["results"][0]["output"]["steps"][0]["task_summary"],
            "Inspect files"
        );
        assert_eq!(output["results"][0]["output"]["steps"][0]["step"], 1);
        assert_eq!(
            output["results"][0]["output"]["steps"][0]["delivery"],
            "Read relevant files and identify edits."
        );
        assert_eq!(output["results"][0]["output"]["steps"][1]["step"], 1);
        std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS");
    }

    #[test]
    fn task_status_command_inside_command_run_is_not_shell_executed() {
        let output = execute(
            &json!({
                "commands": [
                    {
                        "step": 1,
                        "command_type": "task_status",
                        "command_line": "{\"status\":\"done\",\"task_summary\":\"Patch code\"}"
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["command_type"], "task_status");
        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(
            output["results"][0]["output"],
            json!({ "task_status": { "status": "done", "task_summary": "Patch code" } })
        );
    }

    #[test]
    fn task_status_accepts_no_required_arguments() {
        let output = execute(
            &json!({
                "commands": [
                    {
                        "step": 1,
                        "command_type": "task_status",
                        "command_line": "{}"
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["command_type"], "task_status");
        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(
            output["results"][0]["output"],
            json!({ "task_status": { "status": null, "task_summary": null } })
        );
    }

    #[test]
    fn task_status_rejects_status_outside_question_or_done() {
        let output = execute(
            &json!({
                "commands": [
                    {
                        "step": 1,
                        "command_type": "task_status",
                        "command_line": "{\"status\":\"doing\"}"
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["command_type"], "task_status");
        assert_eq!(output["results"][0]["success"], false);
        assert_eq!(
            output["results"][0]["error"],
            "task_status status must be question or done"
        );
    }

    #[test]
    fn multiple_tasks_command_is_unavailable_by_default() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("TURA_FORCE_MULTIPLE_TASKS");
        std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS");
        let output = execute(
            &json!({
                "commands": [
                    {
                        "command": "multiple_tasks",
                        "command_line": "[{\"nonce_id\":\"inspect\",\"task_summary\":\"Inspect files\",\"delivery\":\"Read relevant files and identify edits.\"},{\"nonce_id\":\"apply\",\"task_summary\":\"Apply changes\",\"delivery\":\"Patch files and verify behavior.\"}]"
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], false);
        assert_eq!(
            output["results"][0]["error"],
            "unsupported command_run command"
        );
    }

    #[test]
    fn compact_context_command_routes_and_outputs_summary() {
        let output = execute(
            &json!({
                "commands": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "echo before-compact"
                    },
                    {
                        "step": 2,
                        "command_type": "compact_context",
                        "command_line": "{\"summary\":\"Goal done partly. Next read src/lib.rs.\"}"
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][1]["command_type"], "compact_context");
        assert_eq!(output["results"][1]["success"], true);
        assert_eq!(
            output["results"][1]["output"]["compact_context"],
            "Goal done partly. Next read src/lib.rs."
        );
    }

    #[test]
    fn compact_context_must_be_final_highest_step() {
        let output = execute(
            &json!({
                "commands": [
                    {
                        "step": 2,
                        "command_type": "compact_context",
                        "command_line": "summary"
                    },
                    {
                        "step": 3,
                        "command_type": "shell_command",
                        "command_line": "echo after"
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], false);
        assert_eq!(
            output["results"][0]["error"],
            "compact_context must be the final command in the highest step of command_run"
        );
    }

    #[test]
    fn shell_command_output_matches_current_code_mode_string() {
        let output = execute(
            &json!({
                "commands": [
                    { "command": "shell_command", "command_line": "echo current-backfill-ok" }
                ]
            }),
            Path::new("."),
        );

        let text = output["results"][0]["output"]
            .as_str()
            .expect("shell command_run output should be current-style text");
        assert!(text.starts_with("Exit code: 0\nWall time: "));
        assert!(text.contains("\nOutput:\n"));
        assert!(text.contains("current-backfill-ok"));
        assert!(!text.contains("\"metadata\""));
        assert!(!text.contains("\"stdout\""));
        assert!(!text.contains("\"stderr\""));
    }

    #[test]
    fn model_backfill_matches_current_shape_except_command_type_key() {
        let output = execute(
            &json!({
                "commands": [
                    { "command": "shell_command", "command_line": "echo command-type-diff-only" }
                ]
            }),
            Path::new("."),
        );
        let result = output["results"][0].as_object().expect("result object");
        let mut keys = result.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        assert_eq!(keys, vec!["command_type", "output", "step", "success"]);

        let mut current_equivalent = output.clone();
        let result = current_equivalent["results"][0]
            .as_object_mut()
            .expect("result object");
        let command_type = result.remove("command_type").expect("command_type");
        result.insert("command".to_string(), command_type);

        let expected = json!({
            "results": [
                {
                    "step": 1,
                    "command": crate::commands::active_shell_command_name(),
                    "success": true,
                    "output": current_equivalent["results"][0]["output"].clone()
                }
            ]
        });
        assert_eq!(current_equivalent, expected);
    }

    #[test]
    fn command_only_shell_text_is_mapped_to_active_shell_command() {
        let output = execute(
            &json!({
                "commands": [
                    { "command": "echo ok", "step": 1 }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(
            output["results"][0]["command_type"],
            crate::commands::active_shell_command_name()
        );
    }

    #[test]
    fn top_level_workdir_is_accepted_for_current_style_shell_items() {
        let output = execute(
            &json!({
                "workdir": ".",
                "commands": [
                    { "command": "echo ok", "step": 1 }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
    }

    #[test]
    fn unknown_command_with_shell_payload_is_mapped_to_active_shell_command() {
        let output = execute(
            &json!({
                "commands": [
                    {
                        "command": "Get-Content src/app.py",
                        "command_line": "echo mapped-ok",
                        "step": 1
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(
            output["results"][0]["command_type"],
            crate::commands::active_shell_command_name()
        );
    }

    #[test]
    fn unknown_command_without_payload_runs_command_text_as_shell() {
        let output = execute(
            &json!({
                "commands": [
                    {
                        "command": "echo raw-command-ok",
                        "command_line": "",
                        "step": 1
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(
            output["results"][0]["command_type"],
            crate::commands::active_shell_command_name()
        );
    }

    #[test]
    fn command_line_without_command_defaults_to_active_shell_command() {
        let output = execute(
            &json!({
                "commands": [
                    {
                        "command_line": "echo command-line-only-ok",
                        "step": 1
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(
            output["results"][0]["command_type"],
            crate::commands::active_shell_command_name()
        );
    }

    #[test]
    fn shell_commands_default_to_15_second_timeout() {
        let args = parse_args(&json!({
            "commands": [
                {
                    "command": "shell_command",
                    "command_line": "echo timeout-default-ok",
                    "step": 1
                }
            ]
        }))
        .expect("parse command_run args");

        let arguments = normalize_shell_command_arguments(&args.commands[0])
            .expect("normalize shell arguments");

        assert_eq!(arguments["timeout_ms"], json!(15_000));
    }

    #[test]
    fn command_line_without_command_type_accepts_workdir_and_timeout() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-command-run-default-shell-workdir-{}",
            std::process::id()
        ));
        let subdir = temp_dir.join("subdir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&subdir).expect("temp subdir");

        let output = execute(
            &json!({
                "commands": [
                    {
                        "command_line": "pwd",
                        "workdir": "subdir",
                        "timeout_ms": 5000,
                        "step": 1
                    }
                ]
            }),
            &temp_dir,
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(
            output["results"][0]["command_type"],
            crate::commands::active_shell_command_name()
        );
        assert!(output["results"][0]["output"]
            .as_str()
            .is_some_and(|text| text.replace('\\', "/").contains("/subdir")));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn legacy_steps_shape_is_accepted() {
        let output = execute(
            &json!({
                "steps": [
                    {
                        "tool_name": "shell_command",
                        "command_code": "echo legacy-steps-ok",
                        "step": 1
                    }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(
            output["results"][0]["command_type"],
            crate::commands::active_shell_command_name()
        );
    }

    #[test]
    fn command_run_arguments_accept_requests_wrapper_and_json_fence() {
        let output = execute(
            &Value::String(
                "```json\n{\"requests\":{\"commands\":[{\"command\":\"shell_command\",\"command_line\":\"echo fenced-ok\",\"step\":1}]}}\n```"
                    .to_string(),
            ),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
    }

    #[test]
    fn command_line_wrapped_apply_patch_routes_to_apply_patch() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-command-run-patch-payload-route-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        std::fs::write(temp_dir.join("app.txt"), "old\n").expect("fixture");

        let output = execute(
            &json!({
                "commands": [
                    {
                        "command": "shell_command",
                        "command_line": "apply_patch <<'PATCH'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\nPATCH",
                        "step": 1
                    }
                ]
            }),
            &temp_dir,
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(output["results"][0]["command_type"], "apply_patch");
        assert_eq!(
            std::fs::read_to_string(temp_dir.join("app.txt")).expect("read fixture"),
            "new\n"
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn aliases_cmd_and_command_line_are_accepted() {
        let output = execute(
            &json!({
                "commands": [
                    { "cmd": "shell_command", "commandLine": "echo ok", "step": 1 }
                ]
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(output["results"][0]["command_type"], "shell_command");
    }

    #[test]
    fn single_shell_object_without_commands_is_wrapped() {
        let output = execute(
            &json!({
                "command": "echo ok",
                "timeoutMs": 120000
            }),
            Path::new("."),
        );

        assert_eq!(output["results"][0]["success"], true);
    }

    #[test]
    fn command_only_here_string_patch_is_routed_to_apply_patch() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-command-run-patch-route-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        std::fs::write(temp_dir.join("app.txt"), "old\n").expect("fixture");

        let output = execute(
            &json!({
                "commands": [
                    {
                        "command": "@'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n'@",
                        "step": 1
                    }
                ]
            }),
            &temp_dir,
        );

        assert_eq!(output["results"][0]["success"], true);
        assert_eq!(output["results"][0]["command_type"], "apply_patch");
        assert_eq!(
            std::fs::read_to_string(temp_dir.join("app.txt")).expect("read fixture"),
            "new\n"
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn later_batch_commands_stop_after_apply_patch_failure() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-command-run-patch-failure-stop-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        std::fs::write(temp_dir.join("app.txt"), "actual\n").expect("fixture");

        let output = execute(
            &json!({
                "commands": [
                    {
                        "command": "apply_patch",
                        "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-missing\n+new\n*** End Patch\n",
                        "step": 1
                    },
                    {
                        "command": "shell_command",
                        "command_line": "echo after",
                        "step": 1
                    },
                    {
                        "command": "shell_command",
                        "command_line": "echo next-step",
                        "step": 2
                    }
                ]
            }),
            &temp_dir,
        );

        assert_eq!(output["cancelled"], true);
        assert!(output["cancel_reason"]
            .as_str()
            .is_some_and(|text| text.contains("apply_patch failed")));
        assert_eq!(output["results"].as_array().expect("results").len(), 1);
        assert_eq!(output["results"][0]["success"], false);
        assert_eq!(
            output["results"][0]["output"]["output"]["error_type"],
            "ContextMismatch"
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn streaming_executor_returns_apply_patch_result_without_finish() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-command-run-streaming-immediate-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        std::fs::write(temp_dir.join("app.txt"), "old\n").expect("fixture");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let mut executor = StreamingCommandRunExecutor::new(temp_dir.clone());

        let immediate = runtime.block_on(executor.push_command_value(json!({
            "command": "apply_patch",
            "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n",
            "step": 1
        })));

        assert_eq!(immediate.len(), 1);
        assert_eq!(immediate[0]["command_type"], "apply_patch");
        assert_eq!(immediate[0]["success"], true);
        assert_eq!(
            std::fs::read_to_string(temp_dir.join("app.txt")).expect("fixture"),
            "new\n"
        );
        let final_results = runtime.block_on(executor.finish());
        assert!(final_results.is_empty());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn streaming_executor_ignores_commands_after_failed_apply_patch() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-command-run-streaming-patch-stop-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        std::fs::write(temp_dir.join("app.txt"), "actual\n").expect("fixture");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let mut executor = StreamingCommandRunExecutor::new(temp_dir.clone());

        let failed = runtime.block_on(executor.push_command_value(json!({
            "command": "apply_patch",
            "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-missing\n+new\n*** End Patch\n",
            "step": 1
        })));
        let ignored = runtime.block_on(executor.push_command_value(json!({
            "command": "shell_command",
            "command_line": "echo after",
            "step": 1
        })));
        let final_results = runtime.block_on(executor.finish());

        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0]["command_type"], "apply_patch");
        assert_eq!(failed[0]["success"], false);
        assert!(ignored.is_empty());
        assert!(final_results.is_empty());
        assert_eq!(
            std::fs::read_to_string(temp_dir.join("app.txt")).expect("fixture"),
            "actual\n"
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
