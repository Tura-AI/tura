//! task_status command: a marker command inside `command_run` that updates the
//! task-management state (done / question). It does no workspace work and is not
//! routed through the normal tool dispatch; `command_run` calls
//! [`normalize_output`] directly. The model-facing prompt and schema live in
//! `prompt.md` / `schema.json`, mirroring every other command.

use serde_json::{json, Value};

pub const PROMPT: &str = include_str!("prompt.md");
pub const SCHEMA: &str = include_str!("schema.json");

/// Normalize a task_status command into its result output
/// `{ "task_status": { "status", "task_summary" } }`.
///
/// `inline_arguments` is the structured argument object (if the model sent the
/// command as a function call), `command_line` is the freeform text form.
pub fn normalize_output(
    inline_arguments: Option<&Value>,
    command_line: &str,
) -> Result<Value, String> {
    let mut value = inline_arguments
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let trimmed = command_line.trim();
    if !trimmed.is_empty() {
        value = if trimmed.starts_with('{') {
            serde_json::from_str(trimmed)
                .map_err(|err| format!("invalid task_status command_line JSON: {err}"))?
        } else {
            parse_status_text(trimmed)
        };
    }
    let Some(object) = value.as_object() else {
        return Err("task_status expects an object".to_string());
    };
    let status = string_field(object, &["status", "task_status"]).map(|status| {
        status
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .to_string()
    });
    if let Some(status) = status.as_deref() {
        if !matches!(status, "question" | "done") {
            return Err("task_status status must be question or done".to_string());
        }
    }
    let task_summary = string_field(object, &["task_summary"]);
    Ok(json!({
        "task_status": {
            "status": status,
            "task_summary": task_summary,
        }
    }))
}

fn parse_status_text(text: &str) -> Value {
    let status = text
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '=' | ',' | ';'))
        .find_map(|part| {
            let part = part.trim().to_ascii_lowercase().replace('-', "_");
            matches!(part.as_str(), "question" | "done").then_some(part)
        });
    json!({ "status": status })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_status_normalizes() {
        let out = normalize_output(None, "{\"status\":\"done\",\"task_summary\":\"Fix bug\"}")
            .expect("ok");
        assert_eq!(out.pointer("/task_status/status").unwrap(), "done");
        assert_eq!(out.pointer("/task_status/task_summary").unwrap(), "Fix bug");
    }

    #[test]
    fn question_text_form_normalizes() {
        let out = normalize_output(None, "question").expect("ok");
        assert_eq!(out.pointer("/task_status/status").unwrap(), "question");
    }

    #[test]
    fn invalid_status_rejected() {
        let err = normalize_output(None, "{\"status\":\"doing\"}").unwrap_err();
        assert_eq!(err, "task_status status must be question or done");
    }
}
