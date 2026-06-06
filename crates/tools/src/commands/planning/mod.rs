pub const COMMAND_NAME: &str = "planning";
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
struct PlanningTask {
    #[serde(default)]
    step: Option<u64>,
    #[serde(default)]
    task_summary: String,
}

pub struct PlanningHandler;

#[async_trait::async_trait]
impl ToolHandler for PlanningHandler {
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
                parse_planning_value(&input).map_err(ToolError::RespondToModel)?
            }
        };
        let output = normalize_planning_output(input, &ctx.session_dir)
            .map_err(ToolError::RespondToModel)?;
        Ok(FunctionToolOutput::from_value(output, Some(true)))
    }
}

pub fn execute(command_line: &str, session_dir: &Path) -> CommandResponse {
    match parse_planning_value(command_line)
        .and_then(|value| normalize_planning_output(value, session_dir))
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
    match normalize_planning_output(value, session_dir) {
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

fn parse_planning_value(text: &str) -> Result<Value, String> {
    serde_json::from_str(text.trim()).map_err(|err| format!("invalid planning JSON: {err}"))
}

fn normalize_planning_output(value: Value, _session_dir: &Path) -> Result<Value, String> {
    let tasks_value = normalize_provider_task_value(value)?;
    let tasks: Vec<PlanningTask> = serde_json::from_value(tasks_value).map_err(|err| {
        format!("planning expects an array of objects with optional step and task_summary: {err}")
    })?;
    if tasks.len() < 2 {
        return Err("planning requires at least two task entries".to_string());
    }
    let mut previous_step: Option<u64> = None;
    let steps = tasks
        .into_iter()
        .enumerate()
        .map(|(index, task)| {
            let requested_step = task.step.unwrap_or((index + 1) as u64).max(1);
            let step = match previous_step {
                Some(previous) if requested_step <= previous => previous + 1,
                _ => requested_step,
            };
            previous_step = Some(step);
            json!({
                "index": index + 1,
                "step": step,
                "task_summary": task.task_summary.trim()
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
    if object.contains_key("task_summary") || object.contains_key("deliverable") {
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
        .ok_or_else(|| format!("invalid planning JSON: expected task array, got {trimmed}"))
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
    fn planning_accepts_task_array() {
        let output = execute(
            r#"[{"task_summary":"Inspect code"},{"task_summary":"Patch code"}]"#,
            Path::new("."),
        );

        assert!(output.success, "{}", output.stderr);
        assert_eq!(output.output["steps"][0]["task_summary"], "Inspect code");
        assert!(output.output["steps"][1].get("deliverable").is_none());
        assert!(output.output["steps"][0].get("task_id").is_none());
    }

    #[test]
    fn planning_rejects_single_task() {
        let output = execute(
            &json!([
                {"task_summary":"Only step"}
            ])
            .to_string(),
            Path::new("."),
        );

        assert!(!output.success);
        assert!(output.stderr.contains("at least two task entries"));
    }

    #[test]
    fn planning_accepts_command_line_json_wrapper() {
        let output = execute_value(
            json!({
                "command_line": "[{\"task_summary\":\"One\"},{\"task_summary\":\"Two\"}]"
            }),
            Path::new("."),
        );

        assert!(output.success, "{}", output.stderr);
        assert_eq!(output.output["steps"][1]["task_summary"], "Two");
    }

    #[test]
    fn planning_accepts_gemini_items_powershell_wrapper() {
        let output = execute_value(
            json!({
                "items": ["@{task_summary=First task}"]
            }),
            Path::new("."),
        );

        assert!(!output.success);
        assert!(output.stderr.contains("at least two task entries"));
    }

    #[test]
    fn planning_extends_duplicate_steps_in_input_order() {
        let output = execute_value(
            json!([
                {"step":1,"task_summary":"A"},
                {"step":2,"task_summary":"B"},
                {"step":2,"task_summary":"C"},
                {"step":3,"task_summary":"D"}
            ]),
            Path::new("."),
        );

        assert!(output.success, "{}", output.stderr);
        let steps = output.output["steps"]
            .as_array()
            .expect("steps should be array")
            .iter()
            .map(|step| {
                step["step"]
                    .as_u64()
                    .expect("step number should be present")
            })
            .collect::<Vec<_>>();
        assert_eq!(steps, vec![1, 2, 3, 4]);
    }
}
