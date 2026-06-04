pub fn step_summary(step_summary: &str) -> String {
    format!("Step summary: {step_summary}")
}

pub fn calling_tool(tool_name: &str) -> String {
    format!("Calling tool `{tool_name}`.")
}

pub fn runtime_failed_after_tool_execution(error: &str) -> String {
    format!("Runtime failed after tool execution: {error}")
}
