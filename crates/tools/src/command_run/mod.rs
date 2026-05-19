pub const TOOL_NAME: &str = "command_run";

mod handler;

pub use handler::{execute, execute_async_value};

pub fn prompt() -> &'static str {
    include_str!("prompt.md")
}
