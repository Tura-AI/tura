use crate::runtime::tool::{CommandRouter, ToolCall, ToolContext, ToolPayload};
use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "handler_parse.rs"]
mod handler_parse;
use handler_parse::{
    command_values, parse_arguments_value, parse_command_item, string_field, u64_field,
};

const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 15_000;
const APPLY_PATCH_FAILURE_CANCEL_REASON: &str =
    "apply_patch failed; command_run stopped before later commands";
const COMMAND_RUN_SANDBOX_ENV: &str = "TURA_COMMAND_RUN_SANDBOX";

#[derive(Clone, Debug)]
struct CommandRunArgs {
    commands: Vec<CommandItem>,
    workdir: Option<String>,
    timeout_ms: Option<u64>,
    allowed_commands: Option<BTreeSet<String>>,
    sandbox: bool,
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
    execute_async_value_with_allowed(arguments, session_dir, None).await
}

pub async fn execute_async_value_with_lock_scope(
    arguments: Value,
    session_dir: std::path::PathBuf,
    lock_scope: Option<String>,
) -> Value {
    execute_async_value_with_allowed_and_lock_scope(arguments, session_dir, None, lock_scope).await
}

pub async fn execute_async_value_with_allowed(
    arguments: Value,
    session_dir: std::path::PathBuf,
    allowed_commands: Option<BTreeSet<String>>,
) -> Value {
    execute_async_value_with_allowed_and_lock_scope(arguments, session_dir, allowed_commands, None)
        .await
}

pub async fn execute_async_value_with_allowed_and_lock_scope(
    arguments: Value,
    session_dir: std::path::PathBuf,
    allowed_commands: Option<BTreeSet<String>>,
    lock_scope: Option<String>,
) -> Value {
    execute_async_value_with_allowed_lock_scope_and_sandbox(
        arguments,
        session_dir,
        allowed_commands,
        lock_scope,
        command_run_sandbox_enabled(),
    )
    .await
}

pub async fn execute_async_value_with_allowed_lock_scope_and_sandbox(
    arguments: Value,
    session_dir: std::path::PathBuf,
    allowed_commands: Option<BTreeSet<String>>,
    lock_scope: Option<String>,
    sandbox: bool,
) -> Value {
    let mut args = match parse_args(&arguments) {
        Ok(args) => args,
        Err(message) => return error_payload(message),
    };
    args.allowed_commands = allowed_commands;
    args.sandbox = sandbox;
    execute_async_args_with_lock_scope(args, session_dir, lock_scope).await
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

pub fn normalize_command_value_for_execution(
    command: Value,
    index: usize,
) -> Result<Value, String> {
    let item = parse_single_streamed_command(command.clone(), index).map_err(|(_, error)| error)?;
    let Some(command_type) = command_type_for_execution(&item) else {
        return Ok(command);
    };
    let mut object = command.as_object().cloned().unwrap_or_default();
    object.insert("step".to_string(), json!(item.effective_step()));
    object.insert("command_type".to_string(), Value::String(command_type));
    if !item.command_line.trim().is_empty() {
        object.insert(
            "command_line".to_string(),
            Value::String(item.command_line.clone()),
        );
    }
    if let Some(workdir) = item.workdir {
        object.insert("workdir".to_string(), Value::String(workdir));
    }
    if let Some(timeout_ms) = item.timeout_ms {
        object.insert("timeout_ms".to_string(), json!(timeout_ms));
    }
    Ok(Value::Object(object))
}

pub struct StreamingCommandRunExecutor {
    router: Arc<CommandRouter>,
    ctx: ToolContext,
    allowed_commands: Option<BTreeSet<String>>,
    sandbox: bool,
    active_step: Option<u64>,
    active_step_repaired: bool,
    next_index: usize,
    macro_command_batch: FuturesUnordered<tokio::task::JoinHandle<CommandRunItemResult>>,
    results: Vec<CommandRunItemResult>,
    halted: bool,
    halt_reason: Option<String>,
}

impl StreamingCommandRunExecutor {
    pub fn new(session_dir: std::path::PathBuf) -> Self {
        Self::new_with_allowed(session_dir, None)
    }

    pub fn new_with_allowed(
        session_dir: std::path::PathBuf,
        allowed_commands: Option<BTreeSet<String>>,
    ) -> Self {
        Self::new_with_allowed_and_lock_scope(session_dir, allowed_commands, None)
    }

    pub fn new_with_allowed_and_lock_scope(
        session_dir: std::path::PathBuf,
        allowed_commands: Option<BTreeSet<String>>,
        lock_scope: Option<String>,
    ) -> Self {
        Self {
            router: Arc::new(CommandRouter::new()),
            ctx: ToolContext::new_with_lock_scope(session_dir, lock_scope),
            allowed_commands,
            sandbox: command_run_sandbox_enabled(),
            active_step: None,
            active_step_repaired: false,
            next_index: 0,
            macro_command_batch: FuturesUnordered::new(),
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

        let requested_step = command.effective_step();
        let step = self.normalize_next_step(requested_step);
        command.step = Some(step);
        if self.active_step.is_some_and(|current| step != current) {
            self.flush_macro_command_batch().await;
        }
        self.active_step = Some(step);

        let macro_command_safe = command
            .is_macro_command_safe(&self.router, &self.ctx.child())
            .await;
        if macro_command_safe {
            let router = Arc::clone(&self.router);
            let ctx = self.ctx.child();
            let allowed_commands = self.allowed_commands.clone();
            let sandbox = self.sandbox;
            self.macro_command_batch.push(tokio::spawn(async move {
                run_command_run_item(
                    &router,
                    command,
                    ctx,
                    false,
                    allowed_commands.as_ref(),
                    sandbox,
                )
                .await
            }));
            self.flush_macro_command_batch().await;
            return self.drain_finished_results();
        }

        self.flush_macro_command_batch().await;
        let result = run_command_run_item(
            &self.router,
            command,
            self.ctx.child(),
            true,
            self.allowed_commands.as_ref(),
            self.sandbox,
        )
        .await;
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
        self.flush_macro_command_batch().await;
        self.drain_finished_results()
    }

    pub fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn event_context(&self) -> ToolContext {
        self.ctx.child()
    }

    pub fn halt_reason(&self) -> Option<&str> {
        self.halt_reason.as_deref()
    }

    fn normalize_next_step(&mut self, requested_step: u64) -> u64 {
        let step = match self.active_step {
            Some(previous) if requested_step < previous => previous + 1,
            Some(previous) if requested_step == previous && self.active_step_repaired => {
                previous + 1
            }
            _ => requested_step,
        };
        self.active_step_repaired = step != requested_step;
        step
    }

    async fn flush_macro_command_batch(&mut self) {
        while let Some(result) = self.macro_command_batch.next().await {
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

fn command_type_for_execution(command: &CommandItem) -> Option<String> {
    let canonical = crate::commands::canonical_command(&command.command);
    if canonical == "task_status" || canonical == "planning" {
        return Some(canonical);
    }
    CommandRouter::new().resolve_command_tool_name(&command.command)
}

async fn execute_async_args(args: CommandRunArgs, session_dir: std::path::PathBuf) -> Value {
    execute_async_args_with_lock_scope(args, session_dir, None).await
}

async fn execute_async_args_with_lock_scope(
    args: CommandRunArgs,
    session_dir: std::path::PathBuf,
    lock_scope: Option<String>,
) -> Value {
    let ctx = ToolContext::new_with_lock_scope(session_dir, lock_scope);
    let output = execute_async(args, ctx).await;
    serde_json::to_value(output).unwrap_or_else(|err| error_payload(err.to_string()))
}

async fn execute_async(args: CommandRunArgs, ctx: ToolContext) -> CommandRunOutput {
    let mut by_step: BTreeMap<u64, Vec<CommandItem>> = BTreeMap::new();
    let CommandRunArgs {
        mut commands,
        allowed_commands,
        sandbox,
        ..
    } = args;
    normalize_command_steps(&mut commands);
    for command in commands {
        by_step
            .entry(command.effective_step())
            .or_default()
            .push(command);
    }

    let router = CommandRouter::new();
    let mut results = Vec::new();
    let mut cancelled = false;
    let mut cancel_reason = None;
    for commands in by_step.into_values() {
        let step_output = run_command_run_step(
            &router,
            commands,
            ctx.child(),
            allowed_commands.as_ref(),
            sandbox,
        )
        .await;
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

fn normalize_command_steps(commands: &mut [CommandItem]) {
    let mut previous_step: Option<u64> = None;
    let mut previous_repaired = false;
    for command in commands {
        let requested_step = command.effective_step();
        let step = match previous_step {
            Some(previous) if requested_step < previous => previous + 1,
            Some(previous) if requested_step == previous && previous_repaired => previous + 1,
            _ => requested_step,
        };
        command.step = Some(step);
        previous_step = Some(step);
        previous_repaired = step != requested_step;
    }
}

async fn run_command_run_step(
    router: &CommandRouter,
    commands: Vec<CommandItem>,
    ctx: ToolContext,
    allowed_commands: Option<&BTreeSet<String>>,
    sandbox: bool,
) -> CommandRunStepOutput {
    let mut results = Vec::new();
    let mut macro_command_batch = Vec::new();

    for command in commands {
        let force_exclusive = !command.is_macro_command_safe(router, &ctx).await;
        if !force_exclusive {
            macro_command_batch.push(command);
            continue;
        }

        results.extend(
            run_macro_command_batch(
                router,
                std::mem::take(&mut macro_command_batch),
                ctx.child(),
                allowed_commands,
                sandbox,
            )
            .await,
        );
        let result = run_command_run_item(
            router,
            command,
            ctx.child(),
            true,
            allowed_commands,
            sandbox,
        )
        .await;
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

    results.extend(
        run_macro_command_batch(router, macro_command_batch, ctx, allowed_commands, sandbox).await,
    );
    CommandRunStepOutput {
        results,
        cancelled: false,
        cancel_reason: None,
    }
}

fn is_failed_apply_patch_result(result: &CommandRunItemResult) -> bool {
    result.command_type == "apply_patch" && !result.success
}

async fn run_macro_command_batch(
    router: &CommandRouter,
    commands: Vec<CommandItem>,
    ctx: ToolContext,
    allowed_commands: Option<&BTreeSet<String>>,
    sandbox: bool,
) -> Vec<CommandRunItemResult> {
    if commands.is_empty() {
        return Vec::new();
    }

    let mut in_flight = FuturesUnordered::new();
    for command in commands {
        in_flight.push(run_command_run_item(
            router,
            command,
            ctx.child(),
            false,
            allowed_commands,
            sandbox,
        ));
    }
    let mut results = Vec::new();
    while let Some(result) = in_flight.next().await {
        results.push(result);
    }
    results.sort_by_key(|result| (result.step, result.index));
    results
}

async fn run_command_run_item(
    router: &CommandRouter,
    command: CommandItem,
    ctx: ToolContext,
    force_exclusive: bool,
    allowed_commands: Option<&BTreeSet<String>>,
    sandbox: bool,
) -> CommandRunItemResult {
    if !command_allowed(&command.command, allowed_commands) {
        return CommandRunItemResult::failed(
            command.index,
            command.effective_step(),
            command.command,
            "unsupported command_run command".to_string(),
        );
    }
    let canonical_command = crate::commands::canonical_command(&command.command);
    if canonical_command == "task_status" {
        return command_run_task_status_result(command);
    }
    if canonical_command == "planning"
        && allowed_commands.is_some_and(|commands| commands.contains("planning"))
    {
        let response = crate::commands::planning::execute(&command.command_line, &ctx.session_dir);
        return CommandRunItemResult {
            index: command.index,
            step: command.effective_step(),
            command_type: "planning".to_string(),
            success: response.success,
            output: Some(response.output),
            error: (!response.success).then_some(response.stderr),
        };
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
    if sandbox {
        if let Err(message) = validate_command_sandbox(&command_name, &call, &ctx.session_dir) {
            return CommandRunItemResult::blocked(
                command.index,
                command.effective_step(),
                command_name,
                message,
            );
        }
    }
    match router.dispatch(call, ctx, force_exclusive).await {
        Ok(result) => CommandRunItemResult {
            index: command.index,
            step: command.effective_step(),
            command_type: command_name.clone(),
            success: result.result.success_for_logging(),
            output: Some(result.result.code_mode_result()),
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

fn command_allowed(command: &str, allowed_commands: Option<&BTreeSet<String>>) -> bool {
    let Some(allowed_commands) = allowed_commands else {
        return true;
    };
    allowed_commands.contains(&crate::commands::canonical_command(command))
}

fn command_run_sandbox_enabled() -> bool {
    std::env::var(COMMAND_RUN_SANDBOX_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "enabled"
            )
        })
        .unwrap_or(false)
}

fn validate_command_sandbox(
    command_name: &str,
    call: &ToolCall,
    session_dir: &Path,
) -> Result<(), String> {
    match command_name {
        "apply_patch" => match &call.payload {
            ToolPayload::Freeform { input } => {
                crate::commands::apply_patch::validate_paths_within_session_dir(input, session_dir)
            }
            ToolPayload::Function { .. } => Ok(()),
        },
        "shell_command" | "bash" | "zsh" => {
            if let Some(workdir) = shell_workdir_from_payload(&call.payload) {
                validate_workdir_within_session_dir(session_dir, &workdir)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn shell_workdir_from_payload(payload: &ToolPayload) -> Option<String> {
    let ToolPayload::Function { arguments } = payload else {
        return None;
    };
    arguments
        .get("workdir")
        .or_else(|| arguments.get("cwd"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn validate_workdir_within_session_dir(session_dir: &Path, raw: &str) -> Result<(), String> {
    let root = session_dir.canonicalize().map_err(|err| {
        format!(
            "failed to resolve command_run sandbox workspace {}: {err}",
            session_dir.display()
        )
    })?;
    let raw_path = PathBuf::from(raw.trim());
    let path = if raw_path.is_absolute() {
        raw_path
    } else {
        root.join(raw_path)
    };
    let path = normalize_path_lexically(&path.canonicalize().unwrap_or(path));
    if path_is_within_root(&path, &root) {
        return Ok(());
    }
    Err(format!(
        "command_run sandbox blocked workdir outside workspace: {}",
        PathBuf::from(raw.trim()).display()
    ))
}

fn path_is_within_root(path: &Path, root: &Path) -> bool {
    if path.strip_prefix(root).is_ok() {
        return true;
    }
    let path = comparable_path_string(path);
    let root = comparable_path_string(root);
    path == root || path.starts_with(&(root + "/"))
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn comparable_path_string(path: &Path) -> String {
    let mut text = path.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_prefix("//?/") {
        text = stripped.to_string();
    }
    #[cfg(windows)]
    {
        text = text.to_ascii_lowercase();
    }
    text.trim_end_matches('/').to_string()
}

fn command_run_task_status_result(command: CommandItem) -> CommandRunItemResult {
    match crate::commands::task_status::normalize_output(
        command.inline_arguments.as_ref(),
        &command.command_line,
    ) {
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

fn build_tool_call(command_name: &str, command: &CommandItem) -> Result<ToolCall, String> {
    let router = CommandRouter::new();
    let payload = match command_name {
        "apply_patch" => ToolPayload::Freeform {
            input: extract_apply_patch_body(&command.command_line)
                .unwrap_or_else(|| command.command_line.clone()),
        },
        "generate_media" => ToolPayload::Function {
            arguments: normalize_json_or_cli_command_arguments(command, "generate_media")?,
        },
        "planning" => ToolPayload::Function {
            arguments: normalize_planning_arguments(command)?,
        },
        "read_media" => ToolPayload::Function {
            arguments: normalize_json_or_cli_command_arguments(command, "read_media")?,
        },
        "web_discover" => ToolPayload::Function {
            arguments: normalize_json_or_cli_command_arguments(command, "web_discover")?,
        },
        _ if router.handler(command_name).is_none()
            && router.resolve_command_tool_name(command_name).as_deref() == Some(command_name) =>
        {
            ToolPayload::Function {
                arguments: normalize_json_or_cli_command_arguments(command, command_name)?,
            }
        }
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
        allowed_commands: None,
        sandbox: command_run_sandbox_enabled(),
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
        if canonical_command == "compact_context" {
            return Err(
                "standalone compact_context command has been removed; use task_status compact_context"
                    .to_string(),
            );
        }
        if !matches!(
            canonical_command.as_str(),
            "shell_command" | "bash" | "zsh" | "apply_patch" | "planning" | "task_status"
        ) {
            if CommandRouter::new()
                .resolve_command_tool_name(&canonical_command)
                .is_some()
            {
                command.command = canonical_command;
                continue;
            }
            if looks_like_removed_structured_tool_call(&command.command, &command.command_line) {
                continue;
            }
            if command.command_line.is_empty()
                && (looks_like_shell_command_text(&command.command)
                    || looks_like_shell_request_payload(&command.command))
            {
                command.command_line = command.command.clone();
                command.command = crate::commands::active_shell_command_name().to_string();
            } else if !command.command_line.is_empty() {
                command.command = crate::commands::active_shell_command_name().to_string();
            }
        } else if command.command_line.is_empty() && looks_like_shell_command_text(&command.command)
        {
            command.command_line = command.command.clone();
            command.command = crate::commands::active_shell_command_name().to_string();
        }
    }
    validate_task_status_compact_context_position(&args.commands)?;
    Ok(args)
}

fn normalize_json_or_cli_command_arguments(
    command: &CommandItem,
    command_name: &str,
) -> Result<Value, String> {
    let trimmed = command.command_line.trim();
    let arguments = if trimmed.is_empty() {
        if let Some(arguments) = &command.inline_arguments {
            Ok(arguments.clone())
        } else {
            Ok(json!({ "cli": command.command_line }))
        }
    } else if trimmed.starts_with('{') || trimmed.starts_with('[') {
        normalize_json_command_arguments(command, command_name)
    } else {
        Ok(json!({ "cli": command.command_line }))
    }?;
    Ok(with_command_timeout(arguments, command))
}

fn with_command_timeout(mut arguments: Value, command: &CommandItem) -> Value {
    if let Some(object) = arguments.as_object_mut() {
        let has_timeout = object.contains_key("timeout_ms")
            || object.contains_key("timeoutMs")
            || object.contains_key("timeout_secs")
            || object.contains_key("timeoutSecs");
        if !has_timeout {
            object.insert(
                "timeout_ms".to_string(),
                json!(command.effective_timeout_ms()),
            );
        }
    }
    arguments
}

fn validate_task_status_compact_context_position(commands: &[CommandItem]) -> Result<(), String> {
    let Some((compact_index, compact)) = commands.iter().enumerate().find(|(_, command)| {
        crate::commands::canonical_command(&command.command) == "task_status"
            && command_has_compact_context(command)
    }) else {
        return Ok(());
    };
    if commands[compact_index + 1..]
        .iter()
        .any(command_has_compact_context)
    {
        return Err("only one task_status compact_context command is allowed".to_string());
    }
    if commands.get(compact_index + 1).is_some() {
        return Err(
            "task_status compact_context must be the final command in the highest step of command_run"
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
            "task_status compact_context must be the final command in the highest step of command_run"
                .to_string(),
        );
    }
    Ok(())
}

fn command_has_compact_context(command: &CommandItem) -> bool {
    command
        .inline_arguments
        .as_ref()
        .is_some_and(value_has_compact_context)
        || command_line_has_compact_context(&command.command_line)
}

fn value_has_compact_context(value: &Value) -> bool {
    value
        .get("compact_context")
        .and_then(Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
}

fn command_line_has_compact_context(command_line: &str) -> bool {
    let trimmed = command_line.trim();
    if trimmed.is_empty() || !trimmed.starts_with('{') {
        return false;
    }
    serde_json::from_str::<Value>(trimmed)
        .ok()
        .is_some_and(|value| value_has_compact_context(&value))
        || extract_jsonish_string_field(trimmed, "compact_context")
            .is_some_and(|value| !value.trim().is_empty())
}

fn normalize_planning_arguments(command: &CommandItem) -> Result<Value, String> {
    let trimmed = command.command_line.trim();
    if trimmed.is_empty() {
        return Err("planning command_line must be a JSON array".to_string());
    }
    let value: Value = serde_json::from_str(trimmed)
        .map_err(|err| format!("invalid planning command_line JSON: {err}"))?;
    Ok(value)
}

fn escape_control_chars_in_json_strings(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in input.chars() {
        if in_string {
            if escaped {
                output.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => {
                    output.push(ch);
                    escaped = true;
                }
                '"' => {
                    output.push(ch);
                    in_string = false;
                }
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                ch if ch.is_control() => {
                    output.push_str(&format!("\\u{:04x}", ch as u32));
                }
                _ => output.push(ch),
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
        }
        output.push(ch);
    }
    output
}

fn extract_jsonish_string_field(input: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let key_start = input.find(&key)?;
    let after_key = &input[key_start + key.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let value_start = after_colon.strip_prefix('"')?;
    let close = find_jsonish_string_close(value_start)?;
    let raw = &value_start[..close];
    decode_jsonish_string(raw)
}

fn find_jsonish_string_close(input: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' if looks_like_field_close(&input[index + ch.len_utf8()..]) => return Some(index),
            _ => {}
        }
    }
    None
}

fn looks_like_field_close(suffix: &str) -> bool {
    let suffix = suffix.trim_start();
    suffix.starts_with('}') || suffix.starts_with(',')
}

fn decode_jsonish_string(raw: &str) -> Option<String> {
    let escaped = escape_control_chars_in_json_strings(&format!("\"{raw}\""));
    serde_json::from_str::<String>(&escaped).ok().or_else(|| {
        Some(
            raw.replace("\\\"", "\"")
                .replace("\\n", "\n")
                .replace("\\r", "\r")
                .replace("\\t", "\t"),
        )
    })
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

fn extract_apply_patch_body(text: &str) -> Option<String> {
    let end_marker = "*** End Patch";
    if let Some(begin) = text.find("*** Begin Patch") {
        let end = text[begin..].find(end_marker)? + begin + end_marker.len();
        return Some(text[begin..end].trim().to_string());
    }
    normalize_apply_patch_body_without_begin(text)
}

fn normalize_apply_patch_body_without_begin(text: &str) -> Option<String> {
    let body = strip_apply_patch_command_line(text.trim());
    if !starts_with_patch_hunk(body) {
        return None;
    }
    let end_marker = "*** End Patch";
    let body = if let Some(end) = body.find(end_marker) {
        body[..end + end_marker.len()].trim().to_string()
    } else {
        format!("{}\n{end_marker}", body.trim_end())
    };
    Some(format!("*** Begin Patch\n{body}"))
}

fn strip_apply_patch_command_line(text: &str) -> &str {
    for prefix in [
        "apply_patch <<'PATCH'",
        "apply_patch <<\"PATCH\"",
        "apply_patch",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return rest.trim_start_matches(['\r', '\n', ' ', '\t']);
        }
    }
    text
}

fn starts_with_patch_hunk(text: &str) -> bool {
    matches!(
        text.trim_start(),
        body if body.starts_with("*** Add File: ")
            || body.starts_with("*** Delete File: ")
            || body.starts_with("*** Update File: ")
    )
}

impl CommandItem {
    fn effective_step(&self) -> u64 {
        self.step.unwrap_or((self.index + 1) as u64).max(1)
    }

    fn effective_timeout_ms(&self) -> u64 {
        self.timeout_ms
            .unwrap_or_else(|| default_timeout_ms_for_command(&self.command))
            .max(1)
    }

    async fn is_macro_command_safe(&self, router: &CommandRouter, ctx: &ToolContext) -> bool {
        let Some(tool_name) = router.resolve_command_tool_name(&self.command) else {
            return false;
        };
        if tool_name == "apply_patch" {
            return false;
        }
        let Ok(call) = build_tool_call(&tool_name, self) else {
            return false;
        };
        if !router.tool_supports_macro_command(&call) {
            return false;
        }
        !router.command_is_mutating(&call, ctx).await
    }
}

fn default_timeout_ms_for_command(command: &str) -> u64 {
    CommandRouter::new()
        .default_timeout_ms_for_command(command)
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS)
}

impl CommandRunItemResult {
    fn blocked(index: usize, step: u64, command: String, error: String) -> Self {
        Self {
            index,
            step,
            command_type: command,
            success: false,
            output: Some(crate::shell_executor::json_like_output(
                126,
                String::new(),
                error.clone(),
                json!({
                    "error_type": "SandboxViolation",
                    "message": error,
                }),
                Vec::new(),
            )),
            error: None,
        }
    }

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
    let trimmed = command.trim_start();
    let text = trimmed.to_ascii_lowercase();
    let first = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(['"', '\'']);
    const KNOWN_SHELL_COMMANDS: &[&str] = &[
        "cat",
        "cargo",
        "cd",
        "cmd",
        "cmd.exe",
        "copy",
        "cp",
        "del",
        "dir",
        "echo",
        "get-childitem",
        "get-content",
        "git",
        "ls",
        "measure-object",
        "mkdir",
        "mv",
        "node",
        "npm",
        "npx",
        "pnpm",
        "powershell",
        "powershell.exe",
        "pwsh",
        "pwsh.exe",
        "py",
        "python",
        "rg",
        "rm",
        "robocopy",
        "select-object",
        "set-content",
        "tsc",
        "tsx",
        "type",
        "where-object",
        "write-output",
        "xcopy",
        "yarn",
    ];
    text.starts_with("powershell ")
        || text.starts_with("powershell.exe ")
        || text.starts_with("pwsh ")
        || text.starts_with("pwsh.exe ")
        || text.starts_with('"')
            && (text.contains("powershell.exe\"") || text.contains("pwsh.exe\""))
        || trimmed.starts_with('$')
        || trimmed.starts_with("./")
        || trimmed.starts_with(".\\")
        || KNOWN_SHELL_COMMANDS.contains(&first)
}

fn looks_like_shell_request_payload(command: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(command.trim()) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    [
        "command",
        "command_line",
        "commandLine",
        "command_code",
        "commandCode",
        "script",
        "code",
    ]
    .iter()
    .any(|name| object.get(*name).and_then(Value::as_str).is_some())
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
#[path = "tests.rs"]
mod tests;
