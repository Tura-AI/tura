use std::io::Write;

pub(crate) fn emit_cli_live_command_run_results(
    results: &[serde_json::Value],
    item_index: &mut usize,
) {
    if env_flag("TURA_CLI_LIVE_JSONL") {
        for event in cli_live_command_run_events(results, item_index) {
            println!("{event}");
        }
        let _ = std::io::stdout().flush();
    } else if env_flag("TURA_CLI_PROGRESS") {
        for result in results {
            emit_cli_progress_command_run_completed(result);
        }
    }
}

pub(crate) fn emit_cli_live_command_run_started(
    command: &serde_json::Value,
    provider_tool_call_id: &str,
    command_index: usize,
) {
    let command_type = command
        .get("command_type")
        .or_else(|| command.get("command"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("command");
    if env_flag("TURA_CLI_PROGRESS") && !env_flag("TURA_CLI_LIVE_JSONL") {
        emit_cli_progress_command_run_started(command_type, command);
        return;
    }
    if !env_flag("TURA_CLI_LIVE_JSONL") {
        return;
    }
    let item_type = if command_type == "apply_patch" {
        "file_change"
    } else {
        "command_execution"
    };
    println!(
        "{}",
        serde_json::json!({
            "type": "item.started",
            "item": {
                "id": format!("item_streamed_command_{provider_tool_call_id}_{command_index}"),
                "type": item_type,
                "command": command_type,
                "command_line": command.get("command_line").cloned().unwrap_or(serde_json::Value::Null),
                "step": command.get("step").cloned().unwrap_or(serde_json::Value::Null),
                "provider_tool_call_id": provider_tool_call_id,
                "command_index": command_index,
                "status": "running",
            }
        })
    );
    let _ = std::io::stdout().flush();
}

fn emit_cli_progress_command_run_started(command_type: &str, command: &serde_json::Value) {
    let mut detail = command
        .get("step")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            command
                .get("command_line")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(truncate_progress_text)
        });
    if detail.is_none() && command_type != "command" {
        detail = Some(command_type.to_string());
    }
    match detail {
        Some(detail) => eprintln!("tool: {command_type} started - {detail}"),
        None => eprintln!("tool: {command_type} started"),
    }
}

fn emit_cli_progress_command_run_completed(result: &serde_json::Value) {
    let command_type = result
        .get("command_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("command");
    let status = if result
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        "completed"
    } else {
        "failed"
    };
    let detail = cli_live_command_aggregated_output(command_type, result)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(truncate_progress_text);
    match detail {
        Some(detail) => eprintln!("tool: {command_type} {status} - {detail}"),
        None => eprintln!("tool: {command_type} {status}"),
    }
}

fn truncate_progress_text(value: &str) -> String {
    const MAX_CHARS: usize = 160;
    let value = value.trim();
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

pub(crate) fn cli_live_command_run_events(
    results: &[serde_json::Value],
    item_index: &mut usize,
) -> Vec<serde_json::Value> {
    let mut events = Vec::new();
    for result in results {
        let command_type = result
            .get("command_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("command");
        let success = result
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let status = if success { "completed" } else { "failed" };
        let aggregated_output = cli_live_command_aggregated_output(command_type, result);
        let item_type = if command_type == "apply_patch" {
            "file_change"
        } else {
            "command_execution"
        };
        events.push(serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": format!("item_live_command_{}", *item_index),
                "type": item_type,
                "command": command_type,
                "aggregated_output": aggregated_output,
                "status": status,
            }
        }));
        *item_index += 1;
    }
    events
}

fn cli_live_command_aggregated_output(command_type: &str, result: &serde_json::Value) -> String {
    result
        .get("output")
        .map(|output| {
            let output = if command_type == "read_media" {
                redacted_read_media_output(output)
            } else {
                output.clone()
            };
            output.as_str().map(ToString::to_string).unwrap_or_else(|| {
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| output.to_string())
            })
        })
        .or_else(|| {
            result
                .get("error")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_default()
}

fn redacted_read_media_output(output: &serde_json::Value) -> serde_json::Value {
    let mut redacted = output.clone();
    redact_media_payload_data(&mut redacted);
    redacted
}

fn redact_media_payload_data(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            let preview_count = object
                .get("visual_preview_count")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if object.contains_key("visual_previews") {
                object.insert(
                    "visual_previews".to_string(),
                    serde_json::json!({
                        "redacted_from_cli_log": true,
                        "count": preview_count,
                        "reason": "media payload is sent through the provider media channel"
                    }),
                );
            }
            let audio_count = object
                .get("audio_preview_count")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if object.contains_key("audio_previews") {
                object.insert(
                    "audio_previews".to_string(),
                    serde_json::json!({
                        "redacted_from_cli_log": true,
                        "count": audio_count,
                        "reason": "media payload is sent through the provider media channel"
                    }),
                );
            }
            let file_count = object
                .get("file_attachment_count")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if object.contains_key("file_attachments") {
                object.insert(
                    "file_attachments".to_string(),
                    serde_json::json!({
                        "redacted_from_cli_log": true,
                        "count": file_count,
                        "reason": "file payload is sent through the provider file channel"
                    }),
                );
            }
            if let Some(serde_json::Value::String(url)) = object.get_mut("url") {
                if is_base64_data_url(url) {
                    *url = "[redacted media data URL]".to_string();
                }
            }
            if let Some(serde_json::Value::String(data)) = object.get_mut("data_base64") {
                if !data.is_empty() {
                    *data = "[redacted base64 file payload]".to_string();
                }
            }
            for child in object.values_mut() {
                redact_media_payload_data(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_media_payload_data(item);
            }
        }
        _ => {}
    }
}

fn is_base64_data_url(value: &str) -> bool {
    value.starts_with("data:") && value.contains(";base64,")
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::cli_live_command_run_events;

    #[test]
    fn cli_live_command_run_events_emit_per_completed_command() {
        let mut item_index = 0;
        let events = cli_live_command_run_events(
            &[serde_json::json!({
                "command_type": "apply_patch",
                "success": false,
                "output": {
                    "error_type": "ContextMismatch",
                    "message": "patch context not found"
                }
            })],
            &mut item_index,
        );

        assert_eq!(item_index, 1);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "item.completed");
        assert_eq!(events[0]["item"]["type"], "file_change");
        assert_eq!(events[0]["item"]["status"], "failed");
        assert!(events[0]["item"]["aggregated_output"]
            .as_str()
            .is_some_and(|text| text.contains("ContextMismatch")));
    }

    #[test]
    fn cli_live_command_run_events_redact_read_media_payloads() {
        let mut item_index = 0;
        let events = cli_live_command_run_events(
            &[serde_json::json!({
                "command_type": "read_media",
                "success": true,
                "output": {
                    "summary_markdown": "- reference/desktop.png: image, 1 visual preview",
                    "visual_preview_count": 1,
                    "visual_previews": [{
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/jpeg;base64,AAA"
                        }
                    }],
                    "media_results": [{
                        "path": "reference/desktop.png",
                        "visual_preview_count": 1,
                        "visual_previews": [{
                            "type": "image_url",
                            "image_url": {
                                "url": "data:image/jpeg;base64,BBB"
                            }
                        }],
                        "file_attachment_count": 1,
                        "file_attachments": [{
                            "data_base64": "QUJD"
                        }]
                    }]
                }
            })],
            &mut item_index,
        );

        let output = events[0]["item"]["aggregated_output"]
            .as_str()
            .expect("aggregated output is text");
        assert!(output.contains("reference/desktop.png"));
        assert!(output.contains("redacted_from_cli_log"));
        assert!(!output.contains("data:image/jpeg;base64"));
        assert!(!output.contains("QUJD"));
    }
}
