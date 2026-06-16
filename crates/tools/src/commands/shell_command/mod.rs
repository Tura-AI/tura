pub const COMMAND_NAME: &str = "shell_command";
pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

pub use crate::shell_executor::{current_shell_process_scope_strategy, ShellProcessScopeStrategy};

use super::CommandResponse;
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use crate::shell_executor::{self, ShellKind};
use std::path::Path;

pub struct ShellCommandHandler;

#[async_trait::async_trait]
impl ToolHandler for ShellCommandHandler {
    fn tool_name(&self) -> &'static str {
        "shell_command"
    }

    fn supports_macro_command(&self) -> bool {
        true
    }

    async fn is_mutating(&self, call: &ToolCall, ctx: &ToolContext) -> bool {
        !shell_executor::looks_read_only_with_root(
            &payload_command_line(&call.payload),
            &ctx.session_dir,
        )
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
            ShellKind::ShellCommand,
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
    shell_executor::execute(
        command_line,
        session_dir,
        timeout_secs,
        ShellKind::ShellCommand,
    )
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
    use super::{payload_command_line, ShellCommandHandler};
    use crate::runtime::tool::{ToolCall, ToolContext, ToolHandler, ToolPayload};
    use serde_json::json;

    fn call(payload: ToolPayload) -> ToolCall {
        ToolCall {
            tool_name: "shell_command".into(),
            call_id: "call-shell".into(),
            payload,
        }
    }

    #[test]
    fn payload_command_line_accepts_freeform_string_and_object_arguments() {
        assert_eq!(
            payload_command_line(&ToolPayload::Freeform {
                input: "Get-Content src/lib.rs".into()
            }),
            "Get-Content src/lib.rs"
        );
        assert_eq!(
            payload_command_line(&ToolPayload::Function {
                arguments: json!("Write-Output ok")
            }),
            "Write-Output ok"
        );
        assert_eq!(
            payload_command_line(&ToolPayload::Function {
                arguments: json!({"command":"Write-Output ok","timeout_ms":1000})
            }),
            r#"{"command":"Write-Output ok","timeout_ms":1000}"#
        );
        assert_eq!(
            payload_command_line(&ToolPayload::Function {
                arguments: json!(false)
            }),
            ""
        );
    }

    #[tokio::test]
    async fn mutating_and_access_follow_executor_read_only_detection() {
        let handler = ShellCommandHandler;
        let ctx = ToolContext::new(std::env::temp_dir());

        let read = call(ToolPayload::Freeform {
            input: "Get-Content src/lib.rs".into(),
        });
        assert!(!handler.is_mutating(&read, &ctx).await);
        let read_access = handler.access(&read, &ctx).await;
        assert!(read_access.is_read_only());
        assert!(!read_access.workspace_write);

        let write = call(ToolPayload::Freeform {
            input: "Set-Content out.txt ok".into(),
        });
        assert!(handler.is_mutating(&write, &ctx).await);
        let write_access = handler.access(&write, &ctx).await;
        assert!(write_access.workspace_write);
        assert!(!write_access.is_read_only());
    }

    #[tokio::test]
    async fn handler_metadata_matches_shell_command_contract() {
        let handler = ShellCommandHandler;
        let ctx = ToolContext::new(std::env::temp_dir());
        let read = call(ToolPayload::Function {
            arguments: json!({"command":"Test-Path src/lib.rs"}),
        });

        assert_eq!(handler.tool_name(), "shell_command");
        assert!(handler.supports_macro_command());
        assert!(!handler.is_mutating(&read, &ctx).await);
        assert!(handler.access(&read, &ctx).await.is_read_only());
    }
}
