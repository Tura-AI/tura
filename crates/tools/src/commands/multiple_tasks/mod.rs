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
    task_summary: String,
    deliverble: String,
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

fn parse_multiple_tasks_value(text: &str) -> Result<Value, String> {
    serde_json::from_str(text.trim()).map_err(|err| format!("invalid multiple_tasks JSON: {err}"))
}

fn normalize_multiple_tasks_output(value: Value, _session_dir: &Path) -> Result<Value, String> {
    let tasks_value = value.get("tasks").cloned().unwrap_or(value);
    let tasks: Vec<MultipleTask> = serde_json::from_value(tasks_value).map_err(|err| {
        format!(
            "multiple_tasks expects an array of objects with task_summary and deliverble: {err}"
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
                "task_summary": task.task_summary.trim(),
                "deliverble": task.deliverble.trim()
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "steps": steps }))
}

#[cfg(test)]
mod tests {
    use super::execute;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn multiple_tasks_accepts_task_array() {
        let output = execute(
            r#"[{"task_summary":"Inspect code","deliverble":"Find files and criteria."},{"task_summary":"Patch code","deliverble":"Edit files and run tests."}]"#,
            Path::new("."),
        );

        assert!(output.success, "{}", output.stderr);
        assert_eq!(output.output["steps"][0]["task_summary"], "Inspect code");
        assert_eq!(
            output.output["steps"][1]["deliverble"],
            "Edit files and run tests."
        );
    }

    #[test]
    fn multiple_tasks_rejects_single_task() {
        let output = execute(
            &json!([
                {"task_summary":"Only step","deliverble":"One thing."}
            ])
            .to_string(),
            Path::new("."),
        );

        assert!(!output.success);
        assert!(output.stderr.contains("at least two independent tasks"));
    }
}
