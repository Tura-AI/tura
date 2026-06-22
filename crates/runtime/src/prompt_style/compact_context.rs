pub fn compact_context_required(limit_tokens: u64) -> String {
    let limit = format_token_count(limit_tokens);
    format!(
        "Context checkpoint required.\n\nThe visible context is above about {limit} tokens and is now crowded. Continue the original task, but this assistant turn must call command_run. Put any required final work first, then finish the highest step with a task_status update carrying compact_context. Do not use a standalone compact_context command and do not answer only in prose before checkpointing.\n\nExample shape:\n{{\"commands\":[{{\"command_type\":\"shell_command\",\"command_line\":\"<any required final check>\",\"step\":1}},{{\"command_type\":\"task_status\",\"command_line\":\"{{\\\"task_group\\\":\\\"storefront frontend\\\",\\\"compact_context\\\":\\\"<handoff summary for the next turn>\\\"}}\",\"step\":2}}]}}\n\nThe task_status compact_context value becomes the new handoff context. Preserve the task goal, completed work, incomplete work, deliverables, relevant files, validation state, and next steps. The user will receive the current task conversation and any previous summary; include only details not already covered there. Keep it concise."
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
        assert!(prompt.contains("this assistant turn must call command_run"));
        assert!(prompt.contains("task_status update carrying compact_context"));
        assert!(prompt.contains("Do not use a standalone compact_context command"));
        assert!(prompt.contains("\"command_type\":\"task_status\""));
        assert!(prompt.contains("compact_context"));
        assert!(prompt.contains("<handoff summary for the next turn>"));
    }
}
