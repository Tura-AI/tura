use crate::commands::CommandResponse;
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use serde_json::json;
use std::path::Path;

#[path = "src/access.rs"]
mod access;
#[path = "src/args.rs"]
mod args;
#[path = "src/download.rs"]
mod download;
#[path = "src/files.rs"]
mod files;
#[path = "src/filter.rs"]
mod filter;
#[path = "src/html.rs"]
mod html;
#[path = "src/media.rs"]
mod media;
#[path = "src/output.rs"]
mod output;
#[path = "src/policy.rs"]
mod policy;
#[path = "src/runner.rs"]
mod runner;
#[path = "src/search.rs"]
mod search;
#[path = "src/types.rs"]
mod types;
#[path = "src/util.rs"]
mod util;
#[path = "src/website.rs"]
mod website;

pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

pub fn execute(command_line: &str, session_dir: &Path, _timeout_secs: u64) -> CommandResponse {
    match runner::run_web_discover(args::parse_args_text(command_line), session_dir) {
        Ok(output) => CommandResponse {
            success: true,
            exit_code: 0,
            stdout: output::summary_text(&output),
            stderr: String::new(),
            output,
            changes: Vec::new(),
        },
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.clone(),
            output: json!({ "error": err }),
            changes: Vec::new(),
        },
    }
}

pub fn access(command_line: &str, session_dir: &Path) -> Access {
    access::access(command_line, session_dir)
}

pub struct WebDiscoverHandler;

#[async_trait::async_trait]
impl ToolHandler for WebDiscoverHandler {
    fn tool_name(&self) -> &'static str {
        "web_discover"
    }

    fn supports_macro_command(&self) -> bool {
        true
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        false
    }

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> Access {
        match &call.payload {
            ToolPayload::Function { arguments } => {
                access::access_for_value(arguments.clone(), &ctx.session_dir)
            }
            ToolPayload::Freeform { input } => access(input, &ctx.session_dir),
        }
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let args = match call.payload {
            ToolPayload::Function { arguments } => args::parse_args_value(arguments),
            ToolPayload::Freeform { input } => args::parse_args_text(&input),
        }
        .map_err(ToolError::RespondToModel)?;
        let output = runner::run_web_discover(Ok(args), &ctx.session_dir)
            .map_err(ToolError::RespondToModel)?;
        Ok(FunctionToolOutput::from_value(output, Some(true)))
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
