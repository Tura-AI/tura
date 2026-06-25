use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub(crate) fn write_last_message(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create last-message directory: {err}"))?;
    }
    fs::write(path, text).map_err(|err| format!("failed to write last message: {err}"))
}

pub(crate) fn write_jsonl(
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
                            "I'll inspect the requested file first, then apply the patch and run verification.",
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
    let output = output.get("streamed_command_run_result").unwrap_or(output);
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

pub(crate) fn final_message_text(session_log: &[String]) -> String {
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

pub(crate) fn emit_jsonl(value: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(value).map_err(|err| format!("failed to encode jsonl: {err}"))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_runtime_usage, clean_agent_message, command_output, display_command,
        file_changes, final_message_text, flatten_command_results, shell_display_output,
    };
    use serde_json::{json, Value};
    use std::path::PathBuf;

    #[test]
    fn aggregate_runtime_usage_sums_known_fields_and_derives_total_when_missing() {
        let log = vec![
            json!({
                "type": "runtime_usage",
                "usage": {
                    "input_tokens": 10,
                    "cached_input_tokens": 3,
                    "cache_write_tokens": 2,
                    "output_tokens": 5,
                    "reasoning_tokens": 7,
                    "latency_ms": 100
                }
            })
            .to_string(),
            json!({
                "type": "runtime_usage",
                "usage": {
                    "input_tokens": 1,
                    "output_tokens": 2,
                    "reasoning_tokens": 3,
                    "total_tokens": 99,
                    "latency_ms": 4
                }
            })
            .to_string(),
            "not json".to_string(),
        ];

        let usage = aggregate_runtime_usage(&log);

        assert_eq!(usage["input_tokens"], 11);
        assert_eq!(usage["cached_input_tokens"], 3);
        assert_eq!(usage["cache_write_tokens"], 2);
        assert_eq!(usage["output_tokens"], 7);
        assert_eq!(usage["reasoning_tokens"], 10);
        assert_eq!(usage["reasoning_output_tokens"], 10);
        assert_eq!(usage["total_tokens"], 99);
        assert_eq!(usage["latency_ms"], 104);

        let derived = aggregate_runtime_usage(&[json!({
            "type": "runtime_usage",
            "usage": {"input_tokens": 2, "output_tokens": 3, "reasoning_tokens": 4}
        })
        .to_string()]);
        assert_eq!(derived["total_tokens"], 9);
    }

    #[test]
    fn clean_agent_message_removes_raw_tool_payloads_and_keeps_visible_prefix() {
        assert_eq!(clean_agent_message("  hello user  "), "hello user");
        assert_eq!(
            clean_agent_message(r#"{"commands":[{"command":"pwd"}]}"#),
            ""
        );
        assert_eq!(
            clean_agent_message(r#"Done. {"commands":[{"command":"pwd"}]}"#),
            "Done."
        );
        assert_eq!(clean_agent_message(r#"{"reply_message":"hidden"}"#), "");
        assert_eq!(clean_agent_message(""), "");
    }

    #[test]
    fn final_message_text_uses_last_nonempty_assistant_after_cleaning() {
        let log = vec![
            json!({"role": "assistant", "content": "first"}).to_string(),
            json!({"role": "assistant", "content": "{\"commands\":[]}"}).to_string(),
            json!({"role": "assistant", "content": " final answer "}).to_string(),
        ];

        assert_eq!(final_message_text(&log), "final answer");
        assert_eq!(
            final_message_text(&["bad json".to_string()]),
            "Tura session completed."
        );
    }

    #[test]
    fn flatten_command_results_merges_input_command_metadata_and_batch_children() {
        let output = json!({
            "results": [
                {
                    "results": [
                        {"success": true, "stdout": "nested"}
                    ]
                },
                {
                    "success": false,
                    "output": "plain"
                }
            ]
        });
        let input = json!({
            "commands": [
                {"command_type": "shell_command", "command_line": "echo nested"},
                {"command": "apply_patch", "command_line": "*** Begin Patch"}
            ]
        });

        let flattened = flatten_command_results(&output, &input);

        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0]["stdout"], "nested");
        assert_eq!(flattened[1]["command_type"], "apply_patch");
        assert_eq!(flattened[1]["command_line"], "*** Begin Patch");
        assert_eq!(
            flatten_command_results(&json!({"ok": true}), &Value::Null),
            vec![json!({"ok": true})]
        );

        let streamed = flatten_command_results(
            &json!({
                "streamed_command_run_result": {
                    "results": [
                        {"command_type": "shell_command", "output": "ok"}
                    ]
                }
            }),
            &Value::Null,
        );
        assert_eq!(
            streamed,
            vec![json!({"command_type": "shell_command", "output": "ok"})]
        );
    }

    #[test]
    fn file_changes_prefers_explicit_changes_and_falls_back_to_workspace() {
        let cwd = PathBuf::from("C:/workspace");
        let changes = file_changes(
            &json!({
                "response": {
                    "changes": [
                        {"path": "src/lib.rs", "kind": "update"},
                        {"path": "C:/abs/file.rs", "kind": "create"},
                        {"kind": "missing-path"}
                    ]
                }
            }),
            &cwd,
        );

        assert_eq!(changes.len(), 2);
        assert!(changes[0]["path"]
            .as_str()
            .unwrap_or_default()
            .replace('\\', "/")
            .ends_with("C:/workspace/src/lib.rs"));
        assert_eq!(changes[0]["kind"], "update");
        assert_eq!(changes[1]["kind"], "create");

        let fallback = file_changes(&json!({}), &cwd);
        assert_eq!(
            fallback,
            vec![json!({"path": cwd.to_string_lossy().to_string(), "kind": "update"})]
        );
    }

    #[test]
    fn display_and_output_helpers_render_shell_and_structured_outputs() {
        let shell = json!({
            "command_type": "shell_command",
            "command_line": "echo 'hello'",
            "output": "Exit code: 0\nWall time: 0.1 seconds\nOutput:\nhello\n"
        });
        let non_shell = json!({
            "command_type": "read_media",
            "display_command": "read_media photo.png",
            "output": {"summary": "ok"}
        });

        let display = display_command(&shell);
        assert!(display.contains("echo ''hello''") || display.contains("echo 'hello'"));
        assert_eq!(display_command(&non_shell), "read_media photo.png");
        assert_eq!(command_output(&shell), "hello\n");
        assert_eq!(command_output(&json!({"stdout": "direct"})), "direct");
        assert_eq!(command_output(&non_shell), "{\"summary\":\"ok\"}");
        assert_eq!(
            shell_display_output("Exit code: 0\nWall time: 0.1 seconds\nOutput:\nbody"),
            "body"
        );
        assert_eq!(shell_display_output("plain output"), "plain output");
    }
}
