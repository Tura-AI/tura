use crate::commands::CommandResponse;
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use serde_json::{json, Value};
use std::path::Path;

pub const PROMPT: &str = include_str!("prompt.md");
pub const SCHEMA: &str = include_str!("schema.json");
const MAX_SUMMARY_CHARS: usize = 80_000;

pub fn execute(command_line: &str, _session_dir: &Path) -> CommandResponse {
    let summary = parse_summary(command_line);
    let summary = truncate_summary(&summary);
    CommandResponse {
        success: !summary.trim().is_empty(),
        exit_code: if summary.trim().is_empty() { 1 } else { 0 },
        stdout: summary.clone(),
        stderr: if summary.trim().is_empty() {
            "compact_context summary must not be empty".to_string()
        } else {
            String::new()
        },
        output: json!({ "compact_context": summary }),
        changes: Vec::new(),
    }
}

pub fn access(_command_line: &str, _session_dir: &Path) -> Access {
    Access::default()
}

pub struct CompactContextHandler;

#[async_trait::async_trait]
impl ToolHandler for CompactContextHandler {
    fn tool_name(&self) -> &'static str {
        "compact_context"
    }

    fn supports_macro_command(&self) -> bool {
        false
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        true
    }

    async fn access(&self, _call: &ToolCall, _ctx: &ToolContext) -> Access {
        Access::default()
    }

    async fn handle(
        &self,
        call: ToolCall,
        _ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let summary = match call.payload {
            ToolPayload::Freeform { input } => parse_summary(&input),
            ToolPayload::Function { arguments } => summary_from_value(&arguments),
        };
        let summary = truncate_summary(&summary);
        if summary.trim().is_empty() {
            return Err(ToolError::RespondToModel(
                "compact_context summary must not be empty".to_string(),
            ));
        }
        Ok(FunctionToolOutput::from_value(
            json!({ "compact_context": summary }),
            Some(true),
        ))
    }
}

fn parse_summary(command_line: &str) -> String {
    let trimmed = command_line.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            return summary_from_value(&value);
        }
    }
    trimmed.to_string()
}

fn summary_from_value(value: &Value) -> String {
    value
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| value.get("content").and_then(Value::as_str))
        .or_else(|| value.get("text").and_then(Value::as_str))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn truncate_summary(summary: &str) -> String {
    if summary.len() <= MAX_SUMMARY_CHARS {
        return summary.to_string();
    }
    let mut truncated = summary.chars().take(MAX_SUMMARY_CHARS).collect::<String>();
    truncated.push_str("\n\n[compact_context truncated to about 20,000 tokens]");
    truncated
}
