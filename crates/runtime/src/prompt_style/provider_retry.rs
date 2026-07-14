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

#[cfg(test)]
mod tests {
    use super::{media_fallback, transient_failure_retry};

    #[test]
    fn provider_retry_prompts_keep_retry_context() {
        let retry = transient_failure_retry("timeout", 2, 3);

        assert!(retry.contains("retry 2 of 3"));
        assert!(retry.contains("not task completion"));
        assert!(retry.contains("normal command_run tool"));
    }

    #[test]
    fn media_fallback_prompt_names_removed_content() {
        let prompt = media_fallback("image/png", 4);

        assert!(prompt.contains("image/png"));
        assert!(prompt.contains("4 item(s)"));
    }
}
