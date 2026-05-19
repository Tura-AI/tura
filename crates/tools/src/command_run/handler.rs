use crate::runtime::tool::{ToolCall, ToolContext, ToolPayload, ToolRouter};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug)]
struct CommandRunArgs {
    commands: Vec<CommandItem>,
    workdir: Option<String>,
    timeout_ms: Option<u64>,
    timeout_secs: Option<u64>,
}

#[derive(Clone, Debug)]
struct CommandItem {
    index: usize,
    command: String,
    command_line: String,
    workdir: Option<String>,
    step: Option<u64>,
    timeout_secs: Option<u64>,
    timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
struct CommandRunItemResult {
    #[serde(skip)]
    index: usize,
    step: u64,
    command: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CommandRunOutput {
    results: Vec<CommandRunItemResult>,
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
    for commands in by_step.into_values() {
        results.extend(run_command_run_step(&router, commands, ctx.child()).await);
    }
    results.sort_by_key(|result| (result.step, result.index));
    CommandRunOutput { results }
}

async fn run_command_run_step(
    router: &ToolRouter,
    commands: Vec<CommandItem>,
    ctx: ToolContext,
) -> Vec<CommandRunItemResult> {
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
        results.push(run_command_run_item(router, command, ctx.child(), true).await);
    }

    results.extend(run_parallel_items(router, parallel_reads, ctx).await);
    results
}

async fn run_parallel_items(
    router: &ToolRouter,
    commands: Vec<CommandItem>,
    ctx: ToolContext,
) -> Vec<CommandRunItemResult> {
    if commands.is_empty() {
        return Vec::new();
    }

    let futures = commands
        .into_iter()
        .map(|command| run_command_run_item(router, command, ctx.child(), false));
    let mut results = futures::future::join_all(futures).await;
    results.sort_by_key(|result| (result.step, result.index));
    results
}

async fn run_command_run_item(
    router: &ToolRouter,
    command: CommandItem,
    ctx: ToolContext,
    force_exclusive: bool,
) -> CommandRunItemResult {
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
            command: command_name,
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

fn build_tool_call(command_name: &str, command: &CommandItem) -> Result<ToolCall, String> {
    let payload = if command_name == "apply_patch" {
        ToolPayload::Freeform {
            input: command.command_line.clone(),
        }
    } else {
        ToolPayload::Function {
            arguments: normalize_shell_command_arguments(command)?,
        }
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
            object.entry("timeout_ms".to_string()).or_insert_with(|| {
                json!(command
                    .timeout_ms
                    .or_else(|| command.timeout_secs.map(|secs| secs * 1000))
                    .unwrap_or(120_000))
            });
        }
        return Ok(value);
    }
    let mut arguments = json!({
        "command": command.command_line,
        "timeout_ms": command.timeout_ms.or_else(|| command.timeout_secs.map(|secs| secs * 1000)).unwrap_or(120_000),
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
    let top_timeout_secs = u64_field(object, &["timeout_secs", "timeoutSecs"]);
    let command_values = if let Some(commands) = object.get("commands").and_then(Value::as_array) {
        commands.clone()
    } else {
        vec![arguments.clone()]
    };
    let mut args = CommandRunArgs {
        commands: Vec::new(),
        workdir: top_workdir,
        timeout_ms: top_timeout_ms,
        timeout_secs: top_timeout_secs,
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
        if command.timeout_secs.is_none() {
            command.timeout_secs = args.timeout_secs;
        }
        if command.command_line.is_empty() {
            if let Some(patch) = extract_apply_patch_body(&command.command) {
                command.command = "apply_patch".to_string();
                command.command_line = patch;
                continue;
            }
        }
        let canonical_command = crate::commands::canonical_command(&command.command);
        if command.command_line.is_empty()
            && (looks_like_shell_command_text(&command.command)
                || !matches!(
                    canonical_command.as_str(),
                    "shell_command" | "bash" | "apply_patch"
                ))
        {
            command.command_line = command.command.clone();
            command.command = crate::commands::active_shell_command_name().to_string();
        }
    }
    Ok(args)
}

fn parse_arguments_value(arguments: &Value) -> Result<Value, String> {
    match arguments {
        Value::String(text) => serde_json::from_str(text)
            .map_err(|err| format!("failed to parse command_run arguments: {err}")),
        other => Ok(other.clone()),
    }
}

fn parse_command_item(value: &Value) -> Result<CommandItem, String> {
    if let Some(text) = value.as_str() {
        return Ok(CommandItem {
            index: 0,
            command: text.to_string(),
            command_line: String::new(),
            workdir: None,
            step: None,
            timeout_secs: None,
            timeout_ms: None,
        });
    }
    let Some(object) = value.as_object() else {
        return Err("failed to parse command_run command: expected object".to_string());
    };
    let command = string_field(object, &["command", "cmd", "tool", "name"]).ok_or_else(|| {
        "failed to parse command_run command: missing field `command`".to_string()
    })?;
    let command_line =
        string_field(object, &["command_line", "commandLine", "input", "args"]).unwrap_or_default();
    Ok(CommandItem {
        index: 0,
        command,
        command_line,
        workdir: string_field(object, &["workdir", "cwd"]),
        step: u64_field(object, &["step"]),
        timeout_secs: u64_field(object, &["timeout_secs", "timeoutSecs"]),
        timeout_ms: u64_field(object, &["timeout_ms", "timeoutMs"]),
    })
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
            command,
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

fn error_payload(message: String) -> Value {
    json!({
        "results": [
            {
                "step": 1,
                "command": "command_run",
                "success": false,
                "error": message
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::execute;
    use serde_json::json;
    use std::path::Path;

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
            output["results"][0]["command"],
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
        assert_eq!(output["results"][0]["command"], "shell_command");
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
        assert_eq!(output["results"][0]["command"], "apply_patch");
        assert_eq!(
            std::fs::read_to_string(temp_dir.join("app.txt")).expect("read fixture"),
            "new\n"
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
