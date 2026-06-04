use serde_json::{json, Map, Value};

use super::{json_prefix, xml_parameters};

pub fn normalize_command_run_tool_input(name: &str, input: Value) -> Value {
    if name != "command_run" {
        return input;
    }
    normalize_command_run_input(input)
}

fn normalize_command_run_input(input: Value) -> Value {
    let mut object = match input {
        Value::Object(object) => object,
        Value::String(text) => {
            return json!({ "commands": command_run_commands_from_text(&text) });
        }
        other => return json!({ "commands": [other] }),
    };

    if let Some(commands) = object.remove("commands") {
        let mut normalized: Vec<Value> = match commands {
            Value::Array(items) => items.into_iter().map(normalize_command_value).collect(),
            Value::String(text) => {
                if let Some(value) = command_json_fragment(&text) {
                    command_run_commands_from_json_value(value)
                } else if commands_text_contains_complete_xml_command(&text) {
                    command_run_commands_from_text(&text)
                        .into_iter()
                        .map(normalize_command_value)
                        .collect()
                } else if contains_command_shape(&object) {
                    if !object.contains_key("command_line") {
                        if let Some(command_line) =
                            partial_xml_parameter_value(&text, "command_line")
                                .or_else(|| partial_xml_parameter_value(&text, "cmd"))
                        {
                            object.insert("command_line".to_string(), Value::String(command_line));
                        }
                    }
                    return json!({ "commands": [normalize_command_value(Value::Object(object))] });
                } else {
                    command_run_commands_from_text(&text)
                        .into_iter()
                        .map(normalize_command_value)
                        .collect()
                }
            }
            Value::Object(_) => vec![normalize_command_value(commands)],
            other => vec![other],
        };
        for command in &mut normalized {
            inherit_command_fields(command, &object);
        }
        object.insert("commands".to_string(), Value::Array(normalized));
        return Value::Object(object);
    }

    if contains_command_shape(&object) {
        return json!({ "commands": [normalize_command_value(Value::Object(object))] });
    }

    Value::Object(object)
}

fn partial_xml_parameter_value(text: &str, wanted_name: &str) -> Option<String> {
    let needle = format!("name=\"{wanted_name}\"");
    let start = text.find(&needle)?;
    let after_name = &text[start + needle.len()..];
    let value_start = after_name.find('>')? + 1;
    let value = after_name[value_start..].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn commands_text_contains_complete_xml_command(text: &str) -> bool {
    let params = xml_parameters(text.trim());
    params
        .get("command_line")
        .or_else(|| params.get("cmd"))
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
}

fn command_run_commands_from_json_value(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.into_iter().map(normalize_command_value).collect(),
        Value::Object(object) => {
            if let Some(commands) = object.get("commands").and_then(Value::as_array) {
                commands
                    .iter()
                    .cloned()
                    .map(normalize_command_value)
                    .collect()
            } else {
                vec![normalize_command_value(Value::Object(object))]
            }
        }
        other => vec![other],
    }
}

fn inherit_command_fields(command: &mut Value, parent: &Map<String, Value>) {
    let Some(command_object) = command.as_object_mut() else {
        return;
    };
    for key in ["command_type", "command", "step"] {
        if let Some(value) = parent.get(key).cloned() {
            command_object.entry(key.to_string()).or_insert(value);
        }
    }
    if let Some(Value::String(command_type)) = command_object.get("command_type").cloned() {
        command_object
            .entry("command".to_string())
            .or_insert(Value::String(command_type));
    }
    if let Some(Value::String(command)) = command_object.get("command").cloned() {
        command_object
            .entry("command_type".to_string())
            .or_insert(Value::String(command));
    }
}

fn contains_command_shape(object: &Map<String, Value>) -> bool {
    object.contains_key("command_type")
        || object.contains_key("command")
        || object.contains_key("command_line")
        || object.contains_key("cmd")
}

fn command_run_commands_from_text(text: &str) -> Vec<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return match value {
            Value::Array(items) => items.into_iter().map(normalize_command_value).collect(),
            Value::Object(object) => {
                if let Some(commands) = object.get("commands").and_then(Value::as_array) {
                    commands
                        .iter()
                        .cloned()
                        .map(normalize_command_value)
                        .collect()
                } else {
                    vec![normalize_command_value(Value::Object(object))]
                }
            }
            other => vec![other],
        };
    }

    let xml_params = xml_parameters(trimmed);
    if !xml_params.is_empty() {
        return vec![normalize_command_value(Value::Object(xml_params))];
    }
    if let Some(value) = command_json_fragment(trimmed) {
        return match value {
            Value::Array(items) => items.into_iter().map(normalize_command_value).collect(),
            Value::Object(object) => {
                if let Some(commands) = object.get("commands").and_then(Value::as_array) {
                    commands
                        .iter()
                        .cloned()
                        .map(normalize_command_value)
                        .collect()
                } else {
                    vec![normalize_command_value(Value::Object(object))]
                }
            }
            other => vec![other],
        };
    }

    vec![json!({ "command_type": "shell_command", "command_line": trimmed })]
}

fn normalize_command_value(value: Value) -> Value {
    let mut object = match value {
        Value::Object(object) => object,
        Value::String(text) => {
            let commands = command_run_commands_from_text(&text);
            return commands.into_iter().next().unwrap_or_else(
                || json!({ "command_type": "shell_command", "command_line": text }),
            );
        }
        other => return other,
    };

    for key in ["command_line", "cmd", "command", "args", "path", "content"] {
        if let Some(Value::String(text)) = object.get(key).cloned() {
            let params = xml_parameters(&text);
            if !params.is_empty() {
                object.remove(key);
                for (param_key, param_value) in params {
                    object.entry(param_key).or_insert(param_value);
                }
                continue;
            }
            if let Some(Value::Object(fragment)) = command_json_fragment(&text) {
                object.remove(key);
                for (fragment_key, fragment_value) in fragment {
                    object.entry(fragment_key).or_insert(fragment_value);
                }
            }
        }
    }

    if let Some(Value::String(command_type)) = object.get("command_type").cloned() {
        object
            .entry("command".to_string())
            .or_insert(Value::String(command_type));
    }
    if let Some(Value::String(command)) = object.get("command").cloned() {
        object
            .entry("command_type".to_string())
            .or_insert(Value::String(command));
    }
    Value::Object(object)
}

fn command_json_fragment(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    let start = trimmed.find(['{', '['])?;
    let json_text = json_prefix(&trimmed[start..])?;
    let value = serde_json::from_str::<Value>(json_text).ok()?;
    match &value {
        Value::Array(items) => items
            .iter()
            .any(|item| item.as_object().is_some_and(contains_command_shape))
            .then_some(value),
        Value::Object(object) => {
            (contains_command_shape(object) || object.contains_key("commands")).then_some(value)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_command_run_tool_input;
    use serde_json::json;

    #[test]
    fn command_run_input_xml_parameters_become_command_array() {
        let input = json!({
            "command_type": "shell_command",
            "commands": "<parameter name=\"command_line\">cat package.json</parameter><parameter name=\"step\">1</parameter>"
        });

        let normalized = normalize_command_run_tool_input("command_run", input);
        let command = &normalized["commands"][0];

        assert_eq!(command["command_type"], "shell_command");
        assert_eq!(command["command"], "shell_command");
        assert_eq!(command["command_line"], "cat package.json");
        assert_eq!(command["step"], 1);
    }

    #[test]
    fn command_run_input_recovers_json_command_after_partial_wrapper() {
        let input = json!({
            "commands": [{
                "command_type": "shell_command",
                "command_line": "command_line\">{\"cmd\":\"cat package.json\"}"
            }]
        });

        let normalized = normalize_command_run_tool_input("command_run", input);
        let command = &normalized["commands"][0];

        assert_eq!(command["command_type"], "shell_command");
        assert_eq!(command["cmd"], "cat package.json");
        assert_ne!(
            command["command_line"],
            "command_line\">{\"cmd\":\"cat package.json\"}"
        );
    }

    #[test]
    fn command_run_input_prefers_top_level_command_over_partial_commands_xml() {
        let input = json!({
            "command_type": "shell_command",
            "command_line": "Get-Content probe.txt",
            "commands": "\n<parameter name=\"command_type\">apply_patch",
            "step": 2
        });

        let normalized = normalize_command_run_tool_input("command_run", input);
        let command = &normalized["commands"][0];

        assert_eq!(
            normalized["commands"].as_array().expect("commands").len(),
            1
        );
        assert_eq!(command["command_type"], "shell_command");
        assert_eq!(command["command_line"], "Get-Content probe.txt");
        assert_eq!(command["step"], 2);
    }

    #[test]
    fn command_run_input_recovers_unclosed_xml_command_line_when_no_parent_line() {
        let input = json!({
            "command_type": "apply_patch",
            "commands": "\n<parameter name=\"command_line\">*** Begin Patch\n*** Add File: probe.txt\n+ok\n*** End Patch\n",
            "step": 1
        });

        let normalized = normalize_command_run_tool_input("command_run", input);
        let command = &normalized["commands"][0];

        assert_eq!(command["command_type"], "apply_patch");
        assert!(command["command_line"]
            .as_str()
            .expect("command line")
            .starts_with("*** Begin Patch"));
    }
}
