use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use code_tools_suite::state_machine::session_management::SessionInput;
use serde_json::{json, Value};

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if wants_help(&args) {
        print_help();
        return Ok(0);
    }
    let config = CliConfig::parse(args)?;
    if let Some(model) = config.model.as_deref() {
        std::env::set_var("TURA_SESSION_MODEL_OVERRIDE", normalize_model(model));
    }
    if let Some(reasoning) = config.reasoning_effort.as_deref() {
        std::env::set_var("TURA_SESSION_REASONING_EFFORT", reasoning);
    }
    if config.priority {
        std::env::set_var("TURA_SESSION_ACCELERATION_ENABLED", "1");
    }
    if config.multiple_tasks_mode {
        std::env::set_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS", "1");
    } else {
        std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS");
    }
    if let Some(max_tokens) = config.max_tokens {
        std::env::set_var("TURA_SESSION_MAX_TOKENS", max_tokens.to_string());
    }
    std::env::set_var("TURA_PROJECT_ROOT", project_root_from_exe());
    std::env::set_var("TURA_DISABLE_GATEWAY_CALLBACKS", "1");
    std::env::set_var("TURA_DISABLE_ROUTER_AUTOSTART", "1");
    std::env::set_var("TURA_FAIL_ON_RUNTIME_ERROR", "1");
    if config.json {
        std::env::set_var("TURA_CLI_LIVE_JSONL", "1");
    } else {
        std::env::remove_var("TURA_CLI_LIVE_JSONL");
    }
    let prompt = config.prompt()?;
    let session_id = config
        .session_id
        .clone()
        .unwrap_or_else(|| format!("cli-{}", uuid::Uuid::new_v4()));
    if config.json {
        emit_jsonl(&json!({"type": "thread.started", "thread_id": session_id}))?;
        emit_jsonl(&json!({"type": "turn.started"}))?;
        io::stdout()
            .flush()
            .map_err(|err| format!("failed to flush stdout: {err}"))?;
    }
    let result = code_tools_suite::mano::process_from_gateway_session_in_directory(
        session_id.clone(),
        SessionInput {
            user_input: prompt,
            file_input: Vec::new(),
            agent: config.agent.clone(),
            runtime_context: None,
        },
        config.cwd.clone(),
    )?;

    if let Some(path) = config.last_message_path.as_ref() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create last-message directory: {err}"))?;
        }
        fs::write(path, final_message_text(&result.session.session_log))
            .map_err(|err| format!("failed to write last message: {err}"))?;
    }

    if config.json {
        write_jsonl(&result.session.session_log, &session_id, &config.cwd, false)?;
    } else {
        println!("{}", final_message_text(&result.session.session_log));
    }

    Ok(0)
}

fn wants_help(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("help") | Some("--help") | Some("-h")
    ) || args.first().is_some_and(|arg| arg == "exec")
        && matches!(
            args.get(1).map(String::as_str),
            Some("help") | Some("--help") | Some("-h")
        )
}

fn print_help() {
    println!(
        "\
Tura Rust CLI

Usage:
  tura exec [OPTIONS] [PROMPT...]
  tura [OPTIONS] [PROMPT...]

Options:
  -C, --cwd PATH                  workspace directory for the session
  -m, --model MODEL               model override; bare names become openai/MODEL
  -p, --priority                  enable priority model routing for this model
  -a, --agent-id ID               agent id loaded from agents/ or built-ins
      --session-id ID             reuse a deterministic session id
      --json                      emit JSONL events instead of final text only
      --output-last-message PATH  write the final assistant message to PATH
      --model-reasoning-effort LEVEL
                                  reasoning effort override
      --force-multiple-tasks      enable the multiple_tasks command surface
      --multiple-tasks-mode       alias for --force-multiple-tasks
      --enable-multiple-tasks     alias for --force-multiple-tasks
  -c, --config KEY=VALUE          runtime override:
                                  model_reasoning_effort, max_tokens,
                                  model_max_tokens,
                                  force_multiple_tasks=true
      --skip-git-repo-check       accepted for compatibility
      --dangerously-bypass-approvals-and-sandbox
                                  accepted for compatibility
  -h, --help                      show this help

If PROMPT is omitted, tura reads it from stdin.

Examples:
  tura exec -C . -m openai/gpt-5 \"Inspect the workspace\"
  tura exec -C . -m openai/gpt-5 -p --model-reasoning-effort high \"Fix tests\"
  echo \"Summarize the architecture\" | tura exec --json
"
    );
}

#[derive(Debug)]
struct CliConfig {
    cwd: PathBuf,
    json: bool,
    model: Option<String>,
    reasoning_effort: Option<String>,
    priority: bool,
    multiple_tasks_mode: bool,
    max_tokens: Option<u64>,
    agent: Option<String>,
    session_id: Option<String>,
    last_message_path: Option<PathBuf>,
    prompt_parts: Vec<String>,
}

impl CliConfig {
    fn parse(mut args: Vec<String>) -> Result<Self, String> {
        if args.first().is_some_and(|arg| arg == "exec") {
            args.remove(0);
        }

        let mut config = Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            json: false,
            model: None,
            reasoning_effort: None,
            priority: false,
            multiple_tasks_mode: false,
            max_tokens: None,
            agent: None,
            session_id: None,
            last_message_path: None,
            prompt_parts: Vec::new(),
        };

        let mut index = 0;
        while index < args.len() {
            let arg = args[index].as_str();
            if let Some(value) = arg.strip_prefix("--model=") {
                config.model = Some(value.to_string());
                index += 1;
                continue;
            }
            if let Some(value) = arg
                .strip_prefix("--agent-id=")
                .or_else(|| arg.strip_prefix("--agent="))
            {
                config.agent = Some(value.to_string());
                index += 1;
                continue;
            }
            if let Some(value) = arg.strip_prefix("--model-reasoning-effort=") {
                config.reasoning_effort = Some(value.to_string());
                index += 1;
                continue;
            }
            match arg {
                "--skip-git-repo-check" | "--dangerously-bypass-approvals-and-sandbox" => {
                    index += 1;
                }
                "--json" => {
                    config.json = true;
                    index += 1;
                }
                "--multiple-tasks-mode" | "--enable-multiple-tasks" | "--force-multiple-tasks" => {
                    config.multiple_tasks_mode = true;
                    index += 1;
                }
                "--no-force-multiple-tasks" => {
                    config.multiple_tasks_mode = false;
                    index += 1;
                }
                "-p" | "--priority" => {
                    config.priority = true;
                    index += 1;
                }
                "-C" | "--cwd" => {
                    let value = take_value(&args, index)?;
                    config.cwd = PathBuf::from(value);
                    index += 2;
                }
                "-m" | "--model" => {
                    config.model = Some(take_value(&args, index)?);
                    index += 2;
                }
                "--model-reasoning-effort" | "--reasoning-effort" => {
                    config.reasoning_effort = Some(take_value(&args, index)?);
                    index += 2;
                }
                "--output-last-message" => {
                    config.last_message_path = Some(PathBuf::from(take_value(&args, index)?));
                    index += 2;
                }
                "-a" | "--agent" | "--agent-id" | "--agent-name" => {
                    config.agent = Some(take_value(&args, index)?);
                    index += 2;
                }
                "--session-id" => {
                    config.session_id = Some(take_value(&args, index)?);
                    index += 2;
                }
                "-c" | "--config" => {
                    apply_config_arg(&mut config, &take_value(&args, index)?);
                    index += 2;
                }
                value if value.starts_with('-') => {
                    return Err(format!("unsupported tura option: {value}"));
                }
                _ => {
                    config.prompt_parts.extend(args[index..].iter().cloned());
                    break;
                }
            }
        }

        Ok(config)
    }

    fn prompt(&self) -> Result<String, String> {
        let prompt = self.prompt_parts.join(" ").trim().to_string();
        if !prompt.is_empty() {
            return Ok(prompt);
        }
        let mut stdin = String::new();
        io::stdin()
            .read_to_string(&mut stdin)
            .map_err(|err| format!("failed to read prompt from stdin: {err}"))?;
        let stdin = stdin.trim().to_string();
        if stdin.is_empty() {
            return Err("prompt cannot be empty".to_string());
        }
        Ok(stdin)
    }
}

fn take_value(args: &[String], index: usize) -> Result<String, String> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {}", args[index]))
}

fn apply_config_arg(config: &mut CliConfig, value: &str) {
    let Some((key, raw_value)) = value.split_once('=') else {
        return;
    };
    let value = raw_value.trim().trim_matches('"');
    match key.trim() {
        "model_reasoning_effort" | "reasoning_effort" | "model_variant" => {
            config.reasoning_effort = Some(value.to_string())
        }
        "model_acceleration_enabled" if is_truthy(value) => config.priority = true,
        "max_tokens" | "model_max_tokens" => {
            if let Ok(max_tokens) = value.parse::<u64>() {
                config.max_tokens = Some(max_tokens);
            }
        }
        "service_tier" if value.eq_ignore_ascii_case("priority") => config.priority = true,
        "force_multiple_tasks" | "multiple_tasks_mode" if is_truthy(value) => {
            config.multiple_tasks_mode = true
        }
        _ => {}
    }
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "enabled" | "priority"
    )
}

fn normalize_model(model: &str) -> String {
    if model.contains('/') {
        model.to_string()
    } else {
        format!("openai/{model}")
    }
}

fn project_root_from_exe() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .and_then(|debug| debug.parent())
                .map(|target| target.to_path_buf())
        })
        .and_then(|target| target.parent().map(|root| root.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .display()
        .to_string()
}

fn write_jsonl(
    session_log: &[String],
    session_id: &str,
    cwd: &Path,
    emit_thread_start: bool,
) -> Result<(), String> {
    if emit_thread_start {
        emit_jsonl(&json!({"type": "thread.started", "thread_id": session_id}))?;
        emit_jsonl(&json!({"type": "turn.started"}))?;
    }

    let mut item_index = 0usize;
    for value in session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
    {
        if value.get("role").and_then(Value::as_str) == Some("assistant") {
            let text = value
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let text = clean_agent_message(text);
            if !text.trim().is_empty() {
                emit_jsonl(&json!({
                    "type": "item.completed",
                    "item": {
                        "id": format!("item_{item_index}"),
                        "type": "agent_message",
                        "text": text
                    }
                }))?;
                item_index += 1;
            }
        } else if value.get("type").and_then(Value::as_str) == Some("tool_result") {
            let tool_name = value
                .get("tool_name")
                .and_then(Value::as_str)
                .unwrap_or("tool");
            if tool_name == "command_run" {
                if item_index == 0 {
                    let summary = value
                        .get("input")
                        .and_then(|input| input.get("step_summary"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|summary| !summary.is_empty())
                        .unwrap_or(
                            "I’ll inspect the requested file first, then apply the patch and run verification.",
                        );
                    emit_jsonl(&json!({
                        "type": "item.completed",
                        "item": {
                            "id": format!("item_{item_index}"),
                            "type": "agent_message",
                            "text": summary
                        }
                    }))?;
                    item_index += 1;
                }
                if !cli_live_jsonl_enabled() {
                    emit_command_run_events(&value, &mut item_index, cwd)?;
                }
            }
        }
    }

    let usage = aggregate_runtime_usage(session_log);
    emit_jsonl(&json!({
        "type": "turn.completed",
        "usage": usage
    }))?;
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush stdout: {err}"))
}

fn cli_live_jsonl_enabled() -> bool {
    std::env::var("TURA_CLI_LIVE_JSONL")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

fn aggregate_runtime_usage(session_log: &[String]) -> Value {
    let mut input_tokens = 0u64;
    let mut cached_input_tokens = 0u64;
    let mut cache_write_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut reasoning_tokens = 0u64;
    let mut total_tokens = 0u64;
    let mut latency_ms = 0u64;

    for usage in session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .filter(|value| value.get("type").and_then(Value::as_str) == Some("runtime_usage"))
        .filter_map(|value| value.get("usage").cloned())
    {
        input_tokens = input_tokens.saturating_add(json_u64(&usage, "input_tokens"));
        cached_input_tokens =
            cached_input_tokens.saturating_add(json_u64(&usage, "cached_input_tokens"));
        cache_write_tokens =
            cache_write_tokens.saturating_add(json_u64(&usage, "cache_write_tokens"));
        output_tokens = output_tokens.saturating_add(json_u64(&usage, "output_tokens"));
        reasoning_tokens = reasoning_tokens.saturating_add(json_u64(&usage, "reasoning_tokens"));
        total_tokens = total_tokens.saturating_add(json_u64(&usage, "total_tokens"));
        latency_ms = latency_ms.saturating_add(json_u64(&usage, "latency_ms"));
    }

    if total_tokens == 0 {
        total_tokens = input_tokens
            .saturating_add(output_tokens)
            .saturating_add(reasoning_tokens);
    }

    json!({
        "input_tokens": input_tokens,
        "cached_input_tokens": cached_input_tokens,
        "cache_write_tokens": cache_write_tokens,
        "output_tokens": output_tokens,
        "reasoning_output_tokens": reasoning_tokens,
        "reasoning_tokens": reasoning_tokens,
        "total_tokens": total_tokens,
        "latency_ms": latency_ms,
    })
}

fn json_u64(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn emit_command_run_events(
    value: &Value,
    item_index: &mut usize,
    cwd: &Path,
) -> Result<(), String> {
    for result in flatten_command_results(
        value.get("output").unwrap_or(&Value::Null),
        value.get("input").unwrap_or(&Value::Null),
    ) {
        let command_type = result
            .get("command_type")
            .or_else(|| result.get("command"))
            .and_then(Value::as_str);
        if command_type == Some("apply_patch") {
            emit_file_change_event(&result, item_index, cwd)?;
            continue;
        }
        let command = display_command(&result);
        emit_jsonl(&json!({
            "type": "item.started",
            "item": {
                "id": format!("item_{}", *item_index),
                "type": "command_execution",
                "command": command,
                "aggregated_output": "",
                "exit_code": null,
                "status": "in_progress"
            }
        }))?;
        emit_jsonl(&json!({
            "type": "item.completed",
            "item": {
                "id": format!("item_{}", *item_index),
                "type": "command_execution",
                "command": command,
                "aggregated_output": command_output(&result),
                "exit_code": result.get("exit_code").and_then(Value::as_i64),
                "status": if result.get("success").and_then(Value::as_bool).unwrap_or(false) { "completed" } else { "failed" }
            }
        }))?;
        *item_index += 1;
    }
    Ok(())
}

fn emit_file_change_event(
    result: &Value,
    item_index: &mut usize,
    cwd: &Path,
) -> Result<(), String> {
    let changes = file_changes(result, cwd);
    emit_jsonl(&json!({
        "type": "item.started",
        "item": {
            "id": format!("item_{}", *item_index),
            "type": "file_change",
            "changes": changes,
            "status": "in_progress"
        }
    }))?;
    emit_jsonl(&json!({
        "type": "item.completed",
        "item": {
            "id": format!("item_{}", *item_index),
            "type": "file_change",
            "changes": changes,
            "status": if result.get("success").and_then(Value::as_bool).unwrap_or(false) { "completed" } else { "failed" }
        }
    }))?;
    *item_index += 1;
    Ok(())
}

fn file_changes(result: &Value, cwd: &Path) -> Vec<Value> {
    let mut changes = Vec::new();
    for change in result
        .get("response")
        .and_then(|value| value.get("changes"))
        .or_else(|| result.get("changes"))
        .or_else(|| result.get("output").and_then(|value| value.get("changes")))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(raw_path) = change.get("path").and_then(Value::as_str) else {
            continue;
        };
        let kind = change
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("update");
        let path = PathBuf::from(raw_path);
        let display_path = if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        };
        changes.push(json!({
            "path": display_path.to_string_lossy().to_string(),
            "kind": kind
        }));
    }
    if changes.is_empty() {
        changes.push(json!({
            "path": cwd.to_string_lossy().to_string(),
            "kind": "update"
        }));
    }
    changes
}

fn flatten_command_results(output: &Value, input: &Value) -> Vec<Value> {
    let mut values = Vec::new();
    let input_commands = input
        .get("commands")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(runs) = output.get("results").and_then(Value::as_array) {
        for (index, run) in runs.iter().enumerate() {
            let mut run = run.clone();
            if let (Some(object), Some(input_command)) =
                (run.as_object_mut(), input_commands.get(index))
            {
                if let Some(command_type) = input_command
                    .get("command_type")
                    .or_else(|| input_command.get("command"))
                    .cloned()
                {
                    object
                        .entry("command_type".to_string())
                        .or_insert(command_type);
                }
                if let Some(command_line) = input_command.get("command_line").cloned() {
                    object
                        .entry("command_line".to_string())
                        .or_insert(command_line);
                }
            }
            if let Some(nested) = run.get("results").and_then(Value::as_array) {
                values.extend(nested.iter().cloned());
            } else {
                values.push(run);
            }
        }
    }
    if values.is_empty() {
        values.push(output.clone());
    }
    values
}

fn display_command(result: &Value) -> String {
    let command_type = result
        .get("command_type")
        .or_else(|| result.get("command"))
        .and_then(Value::as_str);
    let command = result
        .get("display_command")
        .or_else(|| result.get("command_line"))
        .or_else(|| result.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("command_run")
        .to_string();
    if command_type == Some("shell_command") {
        return display_shell_command(&command);
    }
    command
}

fn display_shell_command(command: &str) -> String {
    let escaped = command.replace('\'', "''");
    if cfg!(windows) {
        format!("{} -Command '{escaped}'", quoted_powershell_path())
    } else {
        format!("/bin/bash -lc '{escaped}'")
    }
}

fn quoted_powershell_path() -> String {
    let preferred = PathBuf::from(r"C:\Program Files\PowerShell\7\pwsh.exe");
    if preferred.exists() {
        return format!("\"{}\"", preferred.to_string_lossy());
    }
    "\"pwsh.exe\"".to_string()
}

fn command_output(result: &Value) -> String {
    if let Some(text) = result.get("stdout").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(text) = result.get("output").and_then(Value::as_str) {
        return shell_display_output(text).to_string();
    }
    if let Some(value) = result.get("output") {
        return serde_json::to_string(value).unwrap_or_default();
    }
    String::new()
}

fn shell_display_output(text: &str) -> &str {
    let Some(after_output) = text.split_once("\nOutput:\n").map(|(_, output)| output) else {
        return text;
    };
    if text.starts_with("Exit code: ") && text.contains("\nWall time: ") {
        return after_output;
    }
    text
}

fn final_message_text(session_log: &[String]) -> String {
    for value in session_log
        .iter()
        .rev()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
    {
        if value.get("role").and_then(Value::as_str) == Some("assistant") {
            if let Some(text) = value.get("content").and_then(Value::as_str) {
                let text = clean_agent_message(text);
                if !text.trim().is_empty() {
                    return text;
                }
            }
        }
    }
    "Tura session completed.".to_string()
}

fn clean_agent_message(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || looks_like_tool_payload(trimmed) {
        return String::new();
    }
    if let Some(index) = trimmed.find("{\"commands\"") {
        let (prefix, suffix) = trimmed.split_at(index);
        if looks_like_tool_payload(suffix) {
            return prefix.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn looks_like_tool_payload(text: &str) -> bool {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('{') {
        return false;
    }
    trimmed.contains("\"commands\"")
        || trimmed.contains("\"step_summary\"")
        || trimmed.contains("\"tool_calls\"")
        || trimmed.contains("\"reply_message\"")
}

fn emit_jsonl(value: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(value).map_err(|err| format!("failed to encode jsonl: {err}"))?
    );
    Ok(())
}
