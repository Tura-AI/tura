pub const COMMAND_NAME: &str = "bash";
pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

use super::CommandResponse;
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use crate::shell_executor::{self, ShellKind};
use std::path::Path;

pub struct BashHandler;

#[async_trait::async_trait]
impl ToolHandler for BashHandler {
    fn tool_name(&self) -> &str {
        "bash"
    }

    fn supports_macro_command(&self) -> bool {
        true
    }

    async fn is_mutating(&self, call: &ToolCall, _ctx: &ToolContext) -> bool {
        !shell_executor::looks_read_only(&payload_command_line(&call.payload))
    }

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> Access {
        if self.is_mutating(call, ctx).await {
            Access {
                workspace_write: true,
                ..Access::default()
            }
        } else {
            Access::default()
        }
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let response = shell_executor::execute_async(
            &payload_command_line(&call.payload),
            &ctx.session_dir,
            120,
            ShellKind::Bash,
            &ctx,
        )
        .await;
        let success = response.success;
        Ok(FunctionToolOutput::from_value(
            shell_executor::shell_output_value(response),
            Some(success),
        ))
    }
}

pub fn execute(command_line: &str, session_dir: &Path, timeout_secs: u64) -> CommandResponse {
    shell_executor::execute(command_line, session_dir, timeout_secs, ShellKind::Bash)
}

fn payload_command_line(payload: &ToolPayload) -> String {
    match payload {
        ToolPayload::Function { arguments } => {
            if arguments.is_object() {
                serde_json::to_string(arguments).unwrap_or_default()
            } else {
                arguments.as_str().unwrap_or_default().to_string()
            }
        }
        ToolPayload::Freeform { input } => input.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{payload_command_line, BashHandler};
    use crate::runtime::tool::{ToolCall, ToolContext, ToolHandler, ToolPayload};
    use serde_json::json;

    fn call(payload: ToolPayload) -> ToolCall {
        ToolCall {
            tool_name: "bash".into(),
            call_id: "call-bash".into(),
            payload,
        }
    }

    #[test]
    fn payload_command_line_accepts_freeform_string_and_object_arguments() {
        assert_eq!(
            payload_command_line(&ToolPayload::Freeform {
                input: "cat src/lib.rs".into()
            }),
            "cat src/lib.rs"
        );
        assert_eq!(
            payload_command_line(&ToolPayload::Function {
                arguments: json!("echo ok")
            }),
            "echo ok"
        );
        assert_eq!(
            payload_command_line(&ToolPayload::Function {
                arguments: json!({"command":"echo ok","timeout_ms":1000})
            }),
            r#"{"command":"echo ok","timeout_ms":1000}"#
        );
        assert_eq!(
            payload_command_line(&ToolPayload::Function {
                arguments: json!(42)
            }),
            ""
        );
    }

    #[tokio::test]
    async fn mutating_and_access_follow_shell_read_only_detection() {
        let handler = BashHandler;
        let ctx = ToolContext::new(std::env::temp_dir());

        let read = call(ToolPayload::Freeform {
            input: "cat src/lib.rs".into(),
        });
        assert!(!handler.is_mutating(&read, &ctx).await);
        let read_access = handler.access(&read, &ctx).await;
        assert!(read_access.is_read_only());
        assert!(!read_access.workspace_write);

        let write = call(ToolPayload::Freeform {
            input: "echo ok > out.txt".into(),
        });
        assert!(handler.is_mutating(&write, &ctx).await);
        let write_access = handler.access(&write, &ctx).await;
        assert!(write_access.workspace_write);
        assert!(!write_access.is_read_only());
    }

    #[tokio::test]
    async fn handler_metadata_matches_bash_command_contract() {
        let handler = BashHandler;
        let ctx = ToolContext::new(std::env::temp_dir());
        let read = call(ToolPayload::Function {
            arguments: json!({"command":"git status --short"}),
        });

        assert_eq!(handler.tool_name(), "bash");
        assert!(handler.supports_macro_command());
        assert!(!handler.is_mutating(&read, &ctx).await);
        assert!(handler.access(&read, &ctx).await.is_read_only());
    }

    #[tokio::test]
    async fn object_payload_write_command_is_treated_as_mutating() {
        let handler = BashHandler;
        let ctx = ToolContext::new(std::env::temp_dir());
        let write = call(ToolPayload::Function {
            arguments: json!({"command":"printf ok > out.txt","timeout_ms":1000}),
        });

        assert!(handler.is_mutating(&write, &ctx).await);
        let access = handler.access(&write, &ctx).await;
        assert!(access.workspace_write);
        assert!(access.read_paths.is_empty());
        assert!(access.write_paths.is_empty());
    }
}
