use std::path::Path;

use super::constants::COMMAND_RUN_TOOL;

pub(super) fn normalize_tool_arguments(arguments: serde_json::Value) -> serde_json::Value {
    let mut arguments = arguments.get("requests").cloned().unwrap_or(arguments);

    if let Some(object) = arguments.as_object_mut() {
        object.remove("step_summary");
        object.remove("last_tool_call_status");
        object.remove("last_tool_call_summary");
        object.remove("summary");
        object.remove("description");
    }

    arguments
}

pub(crate) fn normalize_tool_arguments_for_tool(
    tool_name: &str,
    arguments: serde_json::Value,
    session_directory: &Path,
) -> serde_json::Value {
    if tool_name == COMMAND_RUN_TOOL {
        normalize_command_run_arguments(arguments, session_directory)
    } else {
        let arguments = normalize_tool_arguments(arguments);
        normalize_workspace_paths(arguments, session_directory)
    }
}

fn normalize_command_run_arguments(
    mut arguments: serde_json::Value,
    session_directory: &Path,
) -> serde_json::Value {
    if let Some(object) = arguments.as_object_mut() {
        if let Some(commands) = object
            .get_mut("commands")
            .and_then(|value| value.as_array_mut())
        {
            for command in commands {
                normalize_command_run_command(command, session_directory);
            }
        }
    }
    arguments
}

fn normalize_command_run_command(command: &mut serde_json::Value, session_directory: &Path) {
    let Some(object) = command.as_object_mut() else {
        return;
    };
    let tool_name = object
        .get("command_type")
        .or_else(|| object.get("command"))
        .or_else(|| object.get("commandType"))
        .and_then(|value| value.as_str())
        .map(normalize_command_run_tool_name)
        .unwrap_or_default();
    if !is_command_run_structured_tool(&tool_name) {
        return;
    }
    let Some(command_line) = object.get_mut("command_line") else {
        return;
    };
    let Some(command_line_text) = command_line.as_str() else {
        return;
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(strip_command_run_tool_prefix(
        &tool_name,
        command_line_text,
    )) else {
        return;
    };
    value = normalize_workspace_paths(value, session_directory);
    if let Ok(text) = serde_json::to_string(&value) {
        *command_line = serde_json::Value::String(text);
    }
}

fn normalize_command_run_tool_name(command: &str) -> String {
    let normalized = command
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-'], ":")
        .trim_start_matches("semantic:")
        .trim_start_matches("source:")
        .replace(':', "_");
    match normalized.as_str() {
        "apply_patch" | "applypatch" | "patch" => "apply_patch".to_string(),
        _ => normalized,
    }
}

fn is_command_run_structured_tool(tool_name: &str) -> bool {
    tool_name == "apply_patch"
}

fn strip_command_run_tool_prefix<'a>(tool_name: &str, command_line: &'a str) -> &'a str {
    let trimmed = command_line.trim_start();
    let Some((prefix, rest)) = trimmed.split_once(char::is_whitespace) else {
        return trimmed;
    };
    if normalize_command_run_tool_name(prefix) == tool_name {
        rest.trim_start()
    } else {
        trimmed
    }
}

pub(super) fn normalize_workspace_paths(
    mut arguments: serde_json::Value,
    session_directory: &Path,
) -> serde_json::Value {
    match arguments {
        serde_json::Value::Array(ref mut items) => {
            for item in items {
                if let Some(object) = item.as_object_mut() {
                    normalize_workspace_path_fields(object, session_directory);
                }
            }
        }
        serde_json::Value::Object(ref mut object) => {
            normalize_workspace_path_fields(object, session_directory);
        }
        _ => {}
    }
    arguments
}

pub(super) fn normalize_workspace_path_fields(
    object: &mut serde_json::Map<String, serde_json::Value>,
    session_directory: &Path,
) {
    for field in ["path", "directory"] {
        if let Some(value) = object.get_mut(field) {
            if let Some(path) = value.as_str() {
                *value = serde_json::Value::String(resolve_workspace_path(session_directory, path));
            }
        }
    }
}

pub(super) fn resolve_workspace_path(session_directory: &Path, raw_path: &str) -> String {
    let path = Path::new(raw_path);
    if path.is_absolute() {
        if !path_is_inside(path, session_directory) {
            return rebase_absolute_path_to_workspace(session_directory, raw_path);
        }
        return path.to_string_lossy().to_string();
    }
    session_directory.join(path).to_string_lossy().to_string()
}

fn path_is_inside(path: &Path, workspace: &Path) -> bool {
    let normalized_path = normalize_for_compare(path);
    let normalized_workspace = normalize_for_compare(workspace);
    normalized_path == normalized_workspace
        || normalized_path
            .starts_with(&(normalized_workspace.trim_end_matches('/').to_string() + "/"))
}

fn normalize_for_compare(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn rebase_absolute_path_to_workspace(session_directory: &Path, raw_path: &str) -> String {
    let normalized = raw_path.replace('\\', "/");
    if let Some(glob_index) = normalized.find(['*', '?', '[']) {
        let prefix = &normalized[..glob_index];
        let suffix_start = prefix
            .rfind('/')
            .map(|index| index + 1)
            .unwrap_or(glob_index);
        let suffix = &normalized[suffix_start..];
        return session_directory.join(suffix).to_string_lossy().to_string();
    }

    session_directory.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::{normalize_tool_arguments, normalize_tool_arguments_for_tool};
    use serde_json::json;
    const COMMAND_RUN_TOOL: &str = "command_run";
    #[test]
    fn tool_argument_normalization_removes_runtime_reporting_fields() {
        let normalized = normalize_tool_arguments(json!({
            "reply_message": "done",
            "new_learning": "state changed",
            "step_summary": "summarize"
        }));

        assert_eq!(normalized["reply_message"], "done");
        assert_eq!(normalized["new_learning"], "state changed");
        assert!(normalized.get("step_summary").is_none());
        assert!(normalized.get("last_tool_call_status").is_none());
        assert!(normalized.get("last_tool_call_summary").is_none());
    }

    #[test]
    fn tool_argument_normalization_unwraps_batch_requests() {
        let normalized = normalize_tool_arguments(json!({
            "requests": [
                { "pattern": "*.rs", "directory": "." }
            ],
            "step_summary": "list files"
        }));

        assert_eq!(normalized, json!([{ "pattern": "*.rs", "directory": "." }]));
    }

    #[test]
    fn command_run_tool_keeps_runtime_reporting_fields() {
        let arguments = json!({
            "commands": [
                { "command": "shell_command", "command_line": "pwd" },
                { "command": "shell_command", "command_line": "Write-Output 2" },
                { "command": "shell_command", "command_line": "Write-Output 3" },
                { "command": "shell_command", "command_line": "Write-Output 4" },
                { "command": "shell_command", "command_line": "Write-Output 5" }
            ],
            "step_summary": "Run pwd."
        });

        let normalized = normalize_tool_arguments_for_tool(
            COMMAND_RUN_TOOL,
            arguments.clone(),
            std::path::Path::new("C:/workspace"),
        );

        assert_eq!(normalized, arguments);
    }
}
