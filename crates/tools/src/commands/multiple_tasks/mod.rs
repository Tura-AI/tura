pub const COMMAND_NAME: &str = "multiple_tasks";
pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

use super::CommandResponse;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MultipleTask {
    nonce_id: String,
    #[serde(default)]
    step: Option<u64>,
    #[serde(default)]
    task_summary: String,
    #[serde(default)]
    delivery: String,
}

pub struct MultipleTasksHandler;

#[async_trait::async_trait]
impl ToolHandler for MultipleTasksHandler {
    fn tool_name(&self) -> &'static str {
        COMMAND_NAME
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        true
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let input = match call.payload {
            ToolPayload::Function { arguments } => arguments,
            ToolPayload::Freeform { input } => {
                parse_multiple_tasks_value(&input).map_err(ToolError::RespondToModel)?
            }
        };
        let output = normalize_multiple_tasks_output(input, &ctx.session_dir)
            .map_err(ToolError::RespondToModel)?;
        Ok(FunctionToolOutput::from_value(output, Some(true)))
    }
}

pub fn execute(command_line: &str, session_dir: &Path) -> CommandResponse {
    match parse_multiple_tasks_value(command_line)
        .and_then(|value| normalize_multiple_tasks_output(value, session_dir))
    {
        Ok(output) => CommandResponse {
            success: true,
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            output,
            changes: Vec::new(),
        },
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.clone(),
            output: Value::String(err),
            changes: Vec::new(),
        },
    }
}

pub fn execute_value(value: Value, session_dir: &Path) -> CommandResponse {
    match normalize_multiple_tasks_output(value, session_dir) {
        Ok(output) => CommandResponse {
            success: true,
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            output,
            changes: Vec::new(),
        },
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.clone(),
            output: Value::String(err),
            changes: Vec::new(),
        },
    }
}

fn parse_multiple_tasks_value(text: &str) -> Result<Value, String> {
    serde_json::from_str(text.trim()).map_err(|err| format!("invalid multiple_tasks JSON: {err}"))
}

fn normalize_multiple_tasks_output(value: Value, _session_dir: &Path) -> Result<Value, String> {
    let tasks_value = normalize_provider_task_value(value)?;
    let tasks: Vec<MultipleTask> = serde_json::from_value(tasks_value).map_err(|err| {
        format!(
            "multiple_tasks expects an array of objects with required nonce_id and optional step, task_summary, and delivery: {err}"
        )
    })?;
    if tasks.len() < 2 {
        return Err(
            "multiple_tasks requires at least two independent tasks; skip it for single-goal work"
                .to_string(),
        );
    }
    let steps = tasks
        .into_iter()
        .enumerate()
        .map(|(index, task)| {
            json!({
                "index": index + 1,
                "nonce_id": task.nonce_id.trim(),
                "step": task.step.unwrap_or(index as u64),
                "task_summary": task.task_summary.trim(),
                "delivery": task.delivery.trim()
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "steps": steps }))
}

fn normalize_provider_task_value(value: Value) -> Result<Value, String> {
    if value.is_array() {
        return Ok(value);
    }
    let Some(object) = value.as_object() else {
        return Ok(value);
    };
    for key in ["tasks", "requests"] {
        if let Some(value) = object.get(key) {
            return normalize_provider_task_value(value.clone());
        }
    }
    for key in ["command_line", "commandLine", "input", "args", "items"] {
        if let Some(value) = object.get(key) {
            return normalize_provider_task_value(parse_provider_task_field(value)?);
        }
    }
    if object.contains_key("nonce_id")
        || object.contains_key("task_summary")
        || object.contains_key("delivery")
    {
        return Ok(Value::Array(vec![Value::Object(object.clone())]));
    }
    Ok(Value::Object(object.clone()))
}

fn parse_provider_task_field(value: &Value) -> Result<Value, String> {
    match value {
        Value::String(text) => parse_task_text(text),
        Value::Array(items) if items.len() == 1 && items[0].is_string() => {
            parse_task_text(items[0].as_str().unwrap_or_default())
        }
        other => Ok(other.clone()),
    }
}

fn parse_task_text(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return normalize_provider_task_value(value);
    }
    parse_powershell_task_object(trimmed)
        .map(|object| Value::Array(vec![object]))
        .ok_or_else(|| format!("invalid multiple_tasks JSON: expected task array, got {trimmed}"))
}

fn parse_powershell_task_object(text: &str) -> Option<Value> {
    let inner = text.trim().strip_prefix("@{")?.strip_suffix('}')?.trim();
    let mut object = serde_json::Map::new();
    for part in inner.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        object.insert(key.to_string(), Value::String(value.to_string()));
    }
    (!object.is_empty()).then_some(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::{execute, execute_value};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn multiple_tasks_accepts_task_array() {
        let output = execute(
            r#"[{"nonce_id":"inspect","task_summary":"Inspect code","delivery":"Find files and criteria."},{"nonce_id":"patch","task_summary":"Patch code","delivery":"Edit files and run tests."}]"#,
            Path::new("."),
        );

        assert!(output.success, "{}", output.stderr);
        assert_eq!(output.output["steps"][0]["task_summary"], "Inspect code");
        assert_eq!(
            output.output["steps"][1]["delivery"],
            "Edit files and run tests."
        );
    }

    #[test]
    fn multiple_tasks_rejects_single_task() {
        let output = execute(
            &json!([
                {"nonce_id":"only","task_summary":"Only step","delivery":"One thing."}
            ])
            .to_string(),
            Path::new("."),
        );

        assert!(!output.success);
        assert!(output.stderr.contains("at least two independent tasks"));
    }

    #[test]
    fn multiple_tasks_accepts_command_line_json_wrapper() {
        let output = execute_value(
            json!({
                "command_line": "[{\"nonce_id\":\"one\",\"task_summary\":\"One\",\"delivery\":\"First.\"},{\"nonce_id\":\"two\",\"task_summary\":\"Two\",\"delivery\":\"Second.\"}]"
            }),
            Path::new("."),
        );

        assert!(output.success, "{}", output.stderr);
        assert_eq!(output.output["steps"][1]["task_summary"], "Two");
    }

    #[test]
    fn multiple_tasks_accepts_gemini_items_powershell_wrapper() {
        let output = execute_value(
            json!({
                "items": ["@{nonce_id=first; delivery=First deliverable.; task_summary=First task}"]
            }),
            Path::new("."),
        );

        assert!(!output.success);
        assert!(output.stderr.contains("at least two independent tasks"));
        assert!(output.stderr.contains("single-goal work"));
    }
}
