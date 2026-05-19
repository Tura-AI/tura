pub const COMMAND_NAME: &str = "bash";
pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

use super::{shell_command, CommandResponse};
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use std::path::Path;

pub struct BashHandler;

#[async_trait::async_trait]
impl ToolHandler for BashHandler {
    fn tool_name(&self) -> &'static str {
        "bash"
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    async fn is_mutating(&self, call: &ToolCall, _ctx: &ToolContext) -> bool {
        !shell_command::looks_read_only(&payload_command_line(&call.payload))
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let response = shell_command::execute_async_with_shell(
            &payload_command_line(&call.payload),
            &ctx.session_dir,
            120,
            "bash",
            &ctx,
        )
        .await;
        let success = response.success;
        Ok(FunctionToolOutput::from_value(
            response.output,
            Some(success),
        ))
    }
}

pub fn execute(command_line: &str, session_dir: &Path, timeout_secs: u64) -> CommandResponse {
    shell_command::execute_with_shell(command_line, session_dir, timeout_secs, "bash")
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
