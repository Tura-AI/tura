pub(super) const CONTEXT_OUTPUT_MAX_TOKENS: usize = 2_500;
pub(super) const COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS: usize = 2_500;
pub(super) const APPROX_CHARS_PER_TOKEN: usize = 4;

pub(super) fn truncate_text_to_token_budget(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
    if text.len() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("\n\n[context checkpoint truncated to about 20,000 tokens]");
    out
}

pub(super) fn context_output_byte_budget() -> usize {
    CONTEXT_OUTPUT_MAX_TOKENS * APPROX_CHARS_PER_TOKEN
}

pub(super) fn formatted_truncate_text(content: &str, max_tokens: usize) -> String {
    if content.len() <= max_tokens * APPROX_CHARS_PER_TOKEN {
        return content.to_string();
    }
    let total_lines = content.lines().count();
    let truncated = truncate_middle_with_token_budget(content, max_tokens);
    format!("Total output lines: {total_lines}\n\n{truncated}")
}

pub(super) fn truncate_middle_with_token_budget(content: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
    if max_chars == 0 {
        return format!(
            "...{} tokens truncated...",
            approx_token_count(content.len())
        );
    }
    let keep_each_side = max_chars / 2;
    let mut head_end = 0usize;
    for (count, (index, ch)) in content.char_indices().enumerate() {
        if count >= keep_each_side {
            break;
        }
        head_end = index + ch.len_utf8();
    }
    let mut tail_start = content.len();
    for (count, (index, _)) in content.char_indices().rev().enumerate() {
        if count >= keep_each_side {
            break;
        }
        tail_start = index;
    }
    if head_end >= tail_start {
        return content.to_string();
    }
    let removed = tail_start.saturating_sub(head_end);
    let removed_tokens = approx_token_count(removed);
    format!(
        "{}...{} tokens truncated...{}",
        &content[..head_end],
        removed_tokens,
        &content[tail_start..]
    )
}

fn approx_token_count(byte_count: usize) -> usize {
    byte_count.div_ceil(APPROX_CHARS_PER_TOKEN)
}
