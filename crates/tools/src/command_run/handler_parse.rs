use serde_json::Value;

use super::CommandItem;

pub(super) fn parse_arguments_value(arguments: &Value) -> Result<Value, String> {
    let value = match arguments {
        Value::String(text) => parse_jsonish_value(text)
            .map_err(|err| format!("failed to parse command_run arguments: {err}"))?,
        other => other.clone(),
    };
    Ok(value.get("requests").cloned().unwrap_or(value))
}

pub(super) fn command_values(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.clone(),
        Value::Object(_) | Value::String(_) => vec![value.clone()],
        _ => Vec::new(),
    }
}

pub(super) fn parse_command_item(value: &Value) -> Result<CommandItem, String> {
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
    .or_else(|| {
        // Some models name the command in `command_type` and put the argument
        // payload in `command` (e.g. task_status carrying a `{status,...}` JSON
        // blob). Recover that payload as the command_line so it is not dropped.
        if object.contains_key("command_type") || object.contains_key("commandType") {
            string_field(object, &["command", "cmd"])
                .filter(|payload| payload.trim() != command.trim())
        } else {
            None
        }
    })
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

pub(super) fn string_field(
    object: &serde_json::Map<String, Value>,
    names: &[&str],
) -> Option<String> {
    names.iter().find_map(|name| {
        object.get(*name).and_then(|value| match value {
            Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
            Value::Object(_) | Value::Array(_) => Some(value.to_string()),
            _ => None,
        })
    })
}

pub(super) fn u64_field(object: &serde_json::Map<String, Value>, names: &[&str]) -> Option<u64> {
    names.iter().find_map(|name| {
        object.get(*name).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
        })
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
