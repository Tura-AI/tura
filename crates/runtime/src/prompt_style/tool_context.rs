pub fn last_tool_call_response(serialized_response: &str) -> String {
    format!(
        "last_tool_call_response:\n{serialized_response}\n\nUse this callback strictly as evidence for the next step. Do not repeat a command unless the previous output was missing, truncated, failed in an actionable way, or a neighboring range is needed."
    )
}

pub fn helpful_tool_callback_result(
    tool_name: &str,
    success: bool,
    output: &str,
    error: &str,
) -> String {
    format!(
        "Helpful tool callback result from `{}`:\nsuccess: {}\noutput: {}\nerror: {}",
        tool_name, success, output, error
    )
}

pub fn retained_tool_callback_result(
    tool_name: &str,
    success: bool,
    output: &str,
    error: &str,
) -> String {
    format!(
        "Retained tool callback result from `{}`:\nsuccess: {}\noutput: {}\nerror: {}",
        tool_name, success, output, error
    )
}

pub fn recent_tool_callback_result(
    tool_name: &str,
    success: bool,
    output: &str,
    error: &str,
) -> String {
    format!(
        "Recent tool callback result from `{}`:\nsuccess: {}\noutput: {}\nerror: {}\n\nUse this callback strictly as evidence for the next step. Do not repeat commands that already produced enough evidence; continue with edits, validation, or final response when the task is complete.",
        tool_name, success, output, error
    )
}

pub fn tool_result_evaluation(tool_name: &str, evaluations: &str) -> String {
    format!(
        "Tool result enum evaluations from `{}`:\n{}",
        tool_name, evaluations
    )
}

pub fn tool_result_status(tool_name: &str, success: bool, status: &str) -> String {
    format!(
        "Tool result status from `{}`:\nsuccess: {}\nevaluation: {}",
        tool_name, success, status
    )
}

pub fn subsequent_tool_not_helpful() -> &'static str {
    "The subsequent tool call did not mark this result as helpful."
}
