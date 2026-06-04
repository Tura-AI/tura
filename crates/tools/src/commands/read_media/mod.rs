use crate::commands::CommandResponse;
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use serde_json::json;
use std::path::Path;

pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

#[path = "src/access.rs"]
mod access_control;
#[path = "src/args.rs"]
mod args;
#[path = "src/config.rs"]
mod config;
#[path = "src/document.rs"]
mod document;
#[path = "src/media_image.rs"]
mod media_image;
#[path = "src/output.rs"]
mod output;
#[path = "src/paths.rs"]
mod paths;
#[path = "src/pdf.rs"]
mod pdf;
#[path = "src/previews.rs"]
mod previews;
#[path = "src/processing.rs"]
mod processing;
#[path = "src/runner.rs"]
mod runner;
#[path = "src/types.rs"]
mod types;
#[path = "src/video.rs"]
mod video;

use access_control::access_for_value;
use args::{parse_args_text, parse_args_value};
use output::summary_text;
use paths::workspace_relative_path;
use runner::run_read_media;

pub fn execute(command_line: &str, session_dir: &Path) -> CommandResponse {
    match run_read_media(parse_args_text(command_line), session_dir) {
        Ok(output) => CommandResponse {
            success: true,
            exit_code: 0,
            stdout: summary_text(&output),
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
    let Ok(args) = parse_args_text(command_line) else {
        return Access::default();
    };
    Access {
        read_paths: args
            .paths
            .iter()
            .filter_map(|path| workspace_relative_path(path, session_dir))
            .map(|path| path.display().to_string())
            .collect(),
        ..Access::default()
    }
}

pub struct ReadMediaHandler;

#[async_trait::async_trait]
impl ToolHandler for ReadMediaHandler {
    fn tool_name(&self) -> &'static str {
        "read_media"
    }

    fn supports_macro_command(&self) -> bool {
        true
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        false
    }

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> Access {
        match &call.payload {
            ToolPayload::Function { arguments } => access_for_value(arguments, &ctx.session_dir),
            ToolPayload::Freeform { input } => access(input, &ctx.session_dir),
        }
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let args = match call.payload {
            ToolPayload::Function { arguments } => parse_args_value(arguments),
            ToolPayload::Freeform { input } => parse_args_text(&input),
        }
        .map_err(ToolError::RespondToModel)?;
        let output =
            run_read_media(Ok(args), &ctx.session_dir).map_err(ToolError::RespondToModel)?;
        Ok(FunctionToolOutput::from_value(output, Some(true)))
    }
}
