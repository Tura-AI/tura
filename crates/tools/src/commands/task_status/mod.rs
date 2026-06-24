//! task_status command: a marker command inside `command_run` that updates the
//! task-management state (doing / done / question). It does no workspace work and is not
//! routed through the normal tool dispatch; `command_run` calls
//! [`normalize_output`] directly. The model-facing prompt and schema live in
//! `prompt.md` / `schema.json`, mirroring every other command.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const PROMPT: &str = include_str!("prompt.md");
pub const SCHEMA: &str = include_str!("schema.json");

const FALLBACK_TASK_TYPE_IDS: &[&str] = &[
    "creative_and_writing",
    "data_visualization",
    "debug",
    "frontend",
    "interactive_and_3d",
    "new_build",
    "refactoring",
    "research_and_learning",
    "visual",
];

/// Normalize a task_status command into its result output
/// `{ "task_status": { ...provided fields... } }`.
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
            parse_task_status_json(trimmed)
                .map_err(|err| format!("invalid task_status command_line JSON: {err}"))?
        } else {
            parse_status_text(trimmed)
        };
    }
    let Some(object) = value.as_object() else {
        return Err("task_status expects an object".to_string());
    };
    let status = string_field(object, &["status", "task_status"])
        .map(|status| status.trim().to_ascii_lowercase().replace('-', "_"));
    if let Some(status) = status.as_deref() {
        if !matches!(status, "doing" | "question" | "done") {
            return Err("task_status status must be doing, question, or done".to_string());
        }
    }
    let task_group = string_field(object, &["task_group"]);
    let task_type = task_type_field(object, "task_type")?;
    let compact_context = string_field(object, &["compact_context"]);
    let mut task_status = serde_json::Map::new();
    if let Some(status) = status {
        task_status.insert("status".to_string(), Value::String(status));
    }
    if let Some(task_group) = task_group {
        task_status.insert("task_group".to_string(), Value::String(task_group));
    }
    if let Some(task_type) = task_type {
        task_status.insert(
            "task_type".to_string(),
            Value::Array(task_type.into_iter().map(Value::String).collect()),
        );
    }
    if let Some(compact_context) = compact_context {
        task_status.insert(
            "compact_context".to_string(),
            Value::String(compact_context),
        );
    }
    Ok(json!({ "task_status": task_status }))
}

fn parse_task_status_json(trimmed: &str) -> Result<Value, serde_json::Error> {
    match serde_json::from_str(trimmed) {
        Ok(value) => Ok(value),
        Err(err) => {
            let escaped = escape_control_chars_in_json_strings(trimmed);
            if escaped != trimmed {
                serde_json::from_str(&escaped)
            } else {
                Err(err)
            }
        }
    }
}

fn escape_control_chars_in_json_strings(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in input.chars() {
        if in_string {
            if escaped {
                output.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => {
                    output.push(ch);
                    escaped = true;
                }
                '"' => {
                    output.push(ch);
                    in_string = false;
                }
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                ch if ch.is_control() => output.push_str(&format!("\\u{:04x}", ch as u32)),
                _ => output.push(ch),
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
        }
        output.push(ch);
    }
    output
}

fn parse_status_text(text: &str) -> Value {
    let Some(status) = text
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '=' | ',' | ';'))
        .find_map(|part| {
            let part = part.trim().to_ascii_lowercase().replace('-', "_");
            matches!(part.as_str(), "doing" | "question" | "done").then_some(part)
        })
    else {
        return Value::Object(serde_json::Map::new());
    };
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

fn task_type_field(
    object: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<Vec<String>>, String> {
    let Some(value) = object.get(name) else {
        return Ok(None);
    };
    let Some(items) = value.as_array() else {
        return Err("task_status task_type must be an array of strings".to_string());
    };
    let mut out = Vec::new();
    for item in items {
        let Some(id) = item.as_str().map(str::trim).filter(|id| !id.is_empty()) else {
            return Err("task_status task_type must be an array of strings".to_string());
        };
        let valid_ids = task_type_ids();
        if !valid_ids.iter().any(|valid| valid == id) {
            return Err(format!(
                "task_status task_type must be one of: {}",
                valid_ids.join(", ")
            ));
        }
        if !out.iter().any(|existing| existing == id) {
            out.push(id.to_string());
        }
    }
    Ok(Some(out))
}

fn task_type_ids() -> Vec<String> {
    runtime_prompt_root()
        .and_then(|root| read_task_type_ids_from_dir(&root).ok())
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| {
            FALLBACK_TASK_TYPE_IDS
                .iter()
                .map(|id| (*id).to_string())
                .collect()
        })
}

fn runtime_prompt_root() -> Option<PathBuf> {
    std::env::var_os("TURA_RUNTIME_PROMPT_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            let tools_crate = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            Some(
                tools_crate
                    .parent()?
                    .join("runtime")
                    .join("src")
                    .join("runtime_prompt"),
            )
        })
}

fn read_task_type_ids_from_dir(root: &Path) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(root).map_err(|err| err.to_string())?;
    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let identity_path = path.join("prompt_identity.json");
        let identity_text = std::fs::read_to_string(&identity_path)
            .map_err(|err| format!("failed to read {}: {err}", identity_path.display()))?;
        let identity: Value = serde_json::from_str(&identity_text)
            .map_err(|err| format!("failed to parse {}: {err}", identity_path.display()))?;
        if let Some(id) = identity.get("id").and_then(Value::as_str) {
            let id = id.trim();
            if !id.is_empty() && !ids.iter().any(|existing| existing == id) {
                ids.push(id.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_status_normalizes() {
        let out =
            normalize_output(None, "{\"status\":\"done\",\"task_group\":\"Fix bug\"}").expect("ok");
        assert_eq!(
            out.pointer("/task_status/status")
                .expect("status should be present"),
            "done"
        );
        assert_eq!(
            out.pointer("/task_status/task_group")
                .expect("task group should be present"),
            "Fix bug"
        );
    }

    #[test]
    fn question_text_form_normalizes() {
        let out = normalize_output(None, "question").expect("ok");
        assert_eq!(
            out.pointer("/task_status/status")
                .expect("status should be present"),
            "question"
        );
    }

    #[test]
    fn doing_status_normalizes() {
        let out = normalize_output(None, "{\"status\":\"doing\"}").expect("ok");
        assert_eq!(
            out.pointer("/task_status/status")
                .expect("status should be present"),
            "doing"
        );
    }

    #[test]
    fn group_only_omits_status() {
        let out = normalize_output(None, "{\"task_group\":\"商城前端\"}").expect("ok");
        assert!(out.pointer("/task_status/status").is_none());
        assert_eq!(
            out.pointer("/task_status/task_group")
                .expect("task group should be present"),
            "商城前端"
        );
    }

    #[test]
    fn empty_input_omits_empty_fields() {
        let out = normalize_output(None, "{}").expect("ok");
        assert_eq!(out, json!({ "task_status": {} }));
    }

    #[test]
    fn reply_text_fields_are_ignored() {
        let out = normalize_output(
            None,
            "{\"status\":\"question\",\"reply_message\":\"Need API key.\",\"message\":\"Done.\"}",
        )
        .expect("ok");
        assert_eq!(
            out.pointer("/task_status/status")
                .expect("status should be present"),
            "question"
        );
        assert!(out.pointer("/task_status/reply_message").is_none());
        assert!(out.pointer("/task_status/message").is_none());
    }

    #[test]
    fn compact_context_is_preserved() {
        let out = normalize_output(
            None,
            "{\"task_group\":\"Continue parser\",\"compact_context\":\"Need to rerun parser fixture tests.\"}",
        )
        .expect("ok");

        assert_eq!(
            out.pointer("/task_status/task_group")
                .expect("task group should be present"),
            "Continue parser"
        );
        assert_eq!(
            out.pointer("/task_status/compact_context")
                .expect("compact context should be present"),
            "Need to rerun parser fixture tests."
        );
    }

    #[test]
    fn task_type_array_normalizes_and_deduplicates() {
        let out = normalize_output(
            None,
            "{\"task_type\":[\"debug\",\"data_visualization\",\"frontend\",\"debug\"]}",
        )
        .expect("task_type should normalize");

        assert_eq!(
            out.pointer("/task_status/task_type")
                .expect("task_type should be present"),
            &json!(["debug", "data_visualization", "frontend"])
        );
    }

    #[test]
    fn task_type_rejects_unknown_ids() {
        let err = normalize_output(None, "{\"task_type\":[\"unknown\"]}")
            .expect_err("unknown task type should be rejected");

        assert!(err.contains("task_status task_type must be one of"));
    }

    #[test]
    fn invalid_status_rejected() {
        let err = normalize_output(None, "{\"status\":\"blocked\"}")
            .expect_err("status should be rejected");
        assert_eq!(err, "task_status status must be doing, question, or done");
    }
}
