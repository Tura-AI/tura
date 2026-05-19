use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

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
    let config = CliConfig::parse(std::env::args().skip(1).collect())?;
    if let Some(model) = config.model.as_deref() {
        std::env::set_var("TURA_SESSION_MODEL_OVERRIDE", normalize_model(model));
    }
    if let Some(reasoning) = config.reasoning_effort.as_deref() {
        std::env::set_var("TURA_SESSION_REASONING_EFFORT", reasoning);
    }
    if config.priority {
        std::env::set_var("TURA_SESSION_ACCELERATION_ENABLED", "1");
    }
    std::env::set_var("TURA_PROJECT_ROOT", project_root_from_exe());
    std::env::set_var("TURA_DISABLE_GATEWAY_CALLBACKS", "1");
    std::env::set_var("TURA_DISABLE_ROUTER_AUTOSTART", "1");
    std::env::set_var("TURA_FAIL_ON_RUNTIME_ERROR", "1");
    if config.json {
        std::env::set_var("TURA_CLI_LIVE_JSONL", "1");
    }

    let prompt = config.prompt()?;
    let session_id = format!("cli-{}", uuid::Uuid::new_v4());
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

#[derive(Debug)]
struct CliConfig {
    cwd: PathBuf,
    json: bool,
    model: Option<String>,
    reasoning_effort: Option<String>,
    priority: bool,
    agent: Option<String>,
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
            agent: None,
            last_message_path: None,
            prompt_parts: Vec::new(),
        };

        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--skip-git-repo-check" | "--dangerously-bypass-approvals-and-sandbox" => {
                    index += 1;
                }
                "--json" => {
                    config.json = true;
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
                "--output-last-message" => {
                    config.last_message_path = Some(PathBuf::from(take_value(&args, index)?));
                    index += 2;
                }
                "--agent" => {
                    config.agent = Some(take_value(&args, index)?);
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
        "model_reasoning_effort" => config.reasoning_effort = Some(value.to_string()),
        "service_tier" if value.eq_ignore_ascii_case("priority") => config.priority = true,
        _ => {}
    }
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
    cwd: &PathBuf,
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
                emit_command_run_events(&value, &mut item_index, cwd)?;
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

fn aggregate_runtime_usage(session_log: &[String]) -> Value {
    let mut input_tokens = 0u64;
    let mut cached_input_tokens = 0u64;
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
    cwd: &PathBuf,
) -> Result<(), String> {
    for result in flatten_command_results(value.get("output").unwrap_or(&Value::Null)) {
        if result.get("command").and_then(Value::as_str) == Some("apply_patch") {
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
    cwd: &PathBuf,
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

fn file_changes(result: &Value, cwd: &PathBuf) -> Vec<Value> {
    let mut changes = Vec::new();
    for change in result
        .get("response")
        .and_then(|value| value.get("changes"))
        .or_else(|| result.get("changes"))
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
        let absolute = if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        };
        changes.push(json!({
            "path": absolute.to_string_lossy().to_string(),
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

fn flatten_command_results(output: &Value) -> Vec<Value> {
    let mut values = Vec::new();
    if let Some(runs) = output.get("results").and_then(Value::as_array) {
        for run in runs {
            if let Some(nested) = run.get("results").and_then(Value::as_array) {
                values.extend(nested.iter().cloned());
            } else {
                values.push(run.clone());
            }
        }
    }
    if values.is_empty() {
        values.push(output.clone());
    }
    values
}

fn display_command(result: &Value) -> String {
    let command = result
        .get("display_command")
        .or_else(|| result.get("command_line"))
        .or_else(|| result.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("command_run")
        .to_string();
    if result.get("command").and_then(Value::as_str) == Some("shell_command") {
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
        return text.to_string();
    }
    if let Some(value) = result.get("output") {
        return serde_json::to_string(value).unwrap_or_default();
    }
    String::new()
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
