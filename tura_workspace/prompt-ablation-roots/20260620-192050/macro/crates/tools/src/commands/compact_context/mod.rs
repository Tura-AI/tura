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
    fn tool_name(&self) -> &str {
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

#[cfg(test)]
mod tests {
    use super::{
        access, execute, parse_summary, summary_from_value, truncate_summary,
        CompactContextHandler, MAX_SUMMARY_CHARS,
    };
    use crate::runtime::tool::{ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn execute_accepts_freeform_and_structured_summary_fields() {
        let freeform = execute("  keep this summary  ", Path::new("."));
        assert!(freeform.success);
        assert_eq!(freeform.exit_code, 0);
        assert_eq!(freeform.stdout, "keep this summary");
        assert_eq!(freeform.output["compact_context"], "keep this summary");

        let from_summary = execute(r#"{"summary":"from summary"}"#, Path::new("."));
        assert_eq!(from_summary.output["compact_context"], "from summary");

        let from_content = execute(r#"{"content":"from content"}"#, Path::new("."));
        assert_eq!(from_content.output["compact_context"], "from content");

        let from_text = execute(r#"{"text":"from text"}"#, Path::new("."));
        assert_eq!(from_text.output["compact_context"], "from text");
    }

    #[test]
    fn execute_rejects_empty_summary_with_clear_error() {
        for input in ["", "   ", r#"{"summary":"   "}"#, r#"{"missing":"field"}"#] {
            let response = execute(input, Path::new("."));
            assert!(!response.success, "{input}");
            assert_eq!(response.exit_code, 1);
            assert!(response.stderr.contains("must not be empty"));
        }
    }

    #[test]
    fn parse_summary_falls_back_to_raw_text_for_invalid_json() {
        assert_eq!(parse_summary("{not json}"), "{not json}");
        assert_eq!(summary_from_value(&json!({"summary":" a "})), "a");
        assert_eq!(summary_from_value(&json!({"content":" b "})), "b");
        assert_eq!(summary_from_value(&json!({"text":" c "})), "c");
        assert_eq!(summary_from_value(&json!({"summary": 3})), "");
    }

    #[test]
    fn truncate_summary_preserves_short_text_and_marks_long_text() {
        assert_eq!(truncate_summary("short"), "short");

        let long = "x".repeat(MAX_SUMMARY_CHARS + 200);
        let truncated = truncate_summary(&long);
        assert!(truncated.starts_with(&"x".repeat(100)));
        assert!(truncated.contains("truncated to about 20,000 tokens"));
        assert!(truncated.len() < long.len());
    }

    #[test]
    fn compact_context_access_is_non_file_specific() {
        let access = access("anything", Path::new("."));
        assert!(access.read_paths.is_empty());
        assert!(access.write_paths.is_empty());
        assert!(!access.workspace_write);
        assert!(access.is_read_only());
    }

    #[tokio::test]
    async fn handler_accepts_freeform_and_function_payloads() {
        let handler = CompactContextHandler;
        let ctx = ToolContext::new(std::env::temp_dir());

        let freeform = handler
            .handle(
                ToolCall {
                    tool_name: "compact_context".into(),
                    call_id: "call-freeform".into(),
                    payload: ToolPayload::Freeform {
                        input: "freeform summary".into(),
                    },
                },
                ctx.clone(),
            )
            .await
            .expect("freeform summary should succeed");
        assert_eq!(freeform.body["compact_context"], "freeform summary");
        assert_eq!(freeform.success, Some(true));

        let function = handler
            .handle(
                ToolCall {
                    tool_name: "compact_context".into(),
                    call_id: "call-function".into(),
                    payload: ToolPayload::Function {
                        arguments: json!({"summary":"function summary"}),
                    },
                },
                ctx,
            )
            .await
            .expect("function summary should succeed");
        assert_eq!(function.body["compact_context"], "function summary");
    }

    #[tokio::test]
    async fn handler_rejects_empty_summary() {
        let handler = CompactContextHandler;
        let err = handler
            .handle(
                ToolCall {
                    tool_name: "compact_context".into(),
                    call_id: "call-empty".into(),
                    payload: ToolPayload::Function {
                        arguments: json!({"summary":"   "}),
                    },
                },
                ToolContext::new(std::env::temp_dir()),
            )
            .await
            .expect_err("empty summary should fail");

        match err {
            ToolError::RespondToModel(message) => assert!(message.contains("must not be empty")),
            ToolError::Fatal(message) => panic!("unexpected fatal error: {message}"),
        }
    }
}
