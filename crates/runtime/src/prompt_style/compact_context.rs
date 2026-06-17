use crate::state_machine::session_management::DEFAULT_CONTEXT_TOKEN_LIMIT;

pub fn compact_context_required(limit_tokens: u64) -> String {
    format!(
        "Context checkpoint required.\n\nThe visible context is above about {} tokens and is now crowded. Continue the original task, but the next command_run output must include compact_context as the final command in the highest step after any required work in that batch.\n\nThe compact_context summary becomes the new handoff context and must preserve the task goal, completed work, incomplete work, deliverables, relevant files, validation state, and next steps. Keep it under about 15,000 English words.",
        format_token_count(limit_tokens)
    )
}

pub const COMPACT_CONTEXT_REQUIRED: &str = r#"Context checkpoint required.

The visible context is above about 250,000 tokens and is now crowded. Continue the original task, but the next command_run output must include compact_context as the final command in the highest step after any required work in that batch.

The compact_context summary becomes the new handoff context and must preserve the task goal, completed work, incomplete work, deliverables, relevant files, validation state, and next steps. Keep it under about 15,000 English words."#;

fn format_token_count(tokens: u64) -> String {
    if tokens == DEFAULT_CONTEXT_TOKEN_LIMIT {
        return "250,000".to_string();
    }
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
