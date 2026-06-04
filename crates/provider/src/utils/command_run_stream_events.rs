use serde_json::Value;

use crate::tura_llm::{normalize_response_content, ProviderStreamEvent, ProviderStreamEventSink};

pub fn emit_command_run_stream_events_from_content(
    content: &Value,
    stream_events: Option<&ProviderStreamEventSink>,
) {
    let Some(sink) = stream_events else {
        return;
    };
    for (tool_call_id, commands) in command_run_commands_from_content(content) {
        for (command_index, command) in commands.into_iter().enumerate() {
            sink(ProviderStreamEvent::CommandRunCommandReady {
                tool_call_id: tool_call_id.clone(),
                command_index,
                command,
            });
        }
    }
}

fn command_run_commands_from_content(content: &Value) -> Vec<(String, Vec<Value>)> {
    let normalized;
    let tool_calls = if let Some(tool_calls) = content.get("tool_calls").and_then(Value::as_array) {
        tool_calls
    } else {
        normalized = normalize_response_content(content);
        let Some(tool_calls) = normalized.get("tool_calls").and_then(Value::as_array) else {
            return Vec::new();
        };
        tool_calls
    };
    if tool_calls.is_empty() {
        return Vec::new();
    }
    tool_calls
        .iter()
        .enumerate()
        .filter_map(|(index, call)| {
            let function = call.get("function")?;
            let name = function.get("name").and_then(Value::as_str)?;
            if name != "command_run" {
                return None;
            }
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("call_command_run_{index}"));
            let commands = command_run_commands_from_arguments(function.get("arguments")?)?;
            Some((id, commands))
        })
        .collect()
}

fn command_run_commands_from_arguments(arguments: &Value) -> Option<Vec<Value>> {
    if let Some(commands) = arguments.get("commands").and_then(Value::as_array) {
        return Some(commands.clone());
    }
    if let Some(text) = arguments.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            if let Some(commands) = parsed.get("commands").and_then(Value::as_array) {
                return Some(commands.clone());
            }
        }
    }
    None
}
