pub fn final_runtime_failed(error: &str) -> String {
    format!("Model call failed; unable to produce a new summary turn.\n\nError: {error}")
}

pub fn tool_chain_summary_header() -> &'static str {
    "I finished the tool-call chain; here is what the tools returned:"
}

pub fn missing_final_answer() -> &'static str {
    "My runtime turn ended, but the model did not return a final reply. The current session context is preserved — send another message and I will continue."
}

pub fn no_tool_results_runtime_failed(error: &str) -> String {
    format!("Model call failed; no tool results to summarize yet.\n\nError: {error}")
}

pub fn tool_results_then_runtime_failed(summary: &str, error: &str) -> String {
    format!(
        "{summary}\n\nA later model call failed, so I am showing the tool results completed so far first.\n\nError: {error}"
    )
}

pub fn glob_match_summary(preview: &str, total_count: usize) -> String {
    let suffix = if total_count > 5 {
        format!(" and {total_count} matches in total")
    } else {
        String::new()
    };
    format!("Found {preview}{suffix}.")
}
