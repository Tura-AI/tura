pub fn compact_context_required(limit_tokens: u64) -> String {
    let limit = format_token_count(limit_tokens);
    format!(
        "Context checkpoint required.\n\nThe visible context is above about {limit} tokens and is now crowded. Continue the original task, but in this assistant turn you must call command_run, and your command_run batch must include compact_context as the final command in the highest step after any required work in that same batch. Do not answer only in prose before compacting.\n\nExample shape:\n{{\"commands\":[{{\"command_type\":\"shell_command\",\"command_line\":\"<any required final check>\",\"step\":1}},{{\"command_type\":\"compact_context\",\"command_line\":\"{{\\\"summary\\\":\\\"<handoff summary for the next turn>\\\"}}\",\"step\":2}}]}}\n\nThe compact_context summary becomes the new handoff context and must preserve the task goal, completed work, incomplete work, deliverables, relevant files, validation state, and next steps. Keep it under about 15,000 English words."
    )
}

fn format_token_count(tokens: u64) -> String {
    let text = tokens.to_string();
    let mut out = String::new();
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::compact_context_required;

    #[test]
    fn compact_context_required_formats_dynamic_limit_and_current_turn_instruction() {
        let prompt = compact_context_required(76_800);

        assert!(prompt.contains("above about 76,800 tokens"));
        assert!(!prompt.contains("above about 250,000 tokens"));
        assert!(prompt.contains("in this assistant turn you must call command_run"));
        assert!(prompt.contains("\"command_type\":\"compact_context\""));
        assert!(prompt.contains("<handoff summary for the next turn>"));
    }
}
