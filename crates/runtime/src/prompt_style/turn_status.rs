pub fn turn_completed_with_tool_calls(turn: u64, tool_call_count: usize) -> String {
    format!("Turn {turn} completed with {tool_call_count} tool calls")
}
