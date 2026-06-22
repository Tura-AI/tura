pub(super) const CONTEXT_OUTPUT_MAX_CHARS: usize = 10_000;
pub(super) const COMMAND_RUN_RESULT_OUTPUT_MAX_CHARS: usize = 10_000;
pub(super) const COMPACT_CONTEXT_MAX_CHARS: usize = 60_000;

pub(super) fn truncate_text_to_char_budget(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("\n\n[context checkpoint truncated to fit the compact_context summary limit]");
    out
}

pub(super) fn context_output_byte_budget() -> usize {
    CONTEXT_OUTPUT_MAX_CHARS
}

pub(super) fn formatted_truncate_text(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }
    let total_lines = content.lines().count();
    let truncated = truncate_middle_with_char_budget(content, max_chars);
    format!("Total output lines: {total_lines}\n\n{truncated}")
}

pub(super) fn truncate_middle_with_char_budget(content: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return format!("...{} characters truncated...", content.len());
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
    format!(
        "{}...{} characters truncated...{}",
        &content[..head_end],
        removed,
        &content[tail_start..]
    )
}
