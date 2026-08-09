pub fn media_fallback(content_type: &str, removed: usize) -> String {
    format!(
        "The provider rejected `{content_type}` media content. {removed} item(s) were omitted from the next request and replaced with text placeholders; continue using the remaining text and supported media."
    )
}

pub fn transient_failure_retry(error_text: &str, retry: u8, max_retries: u8) -> String {
    format!(
        "Provider failure while waiting for the model response: {error_text}. This is transient provider failure retry {retry} of {max_retries}, not task completion. Retry the current task with the normal command_run tool unless the requested edits and validation are actually complete."
    )
}
