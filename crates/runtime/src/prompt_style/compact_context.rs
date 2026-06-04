pub const COMPACT_CONTEXT_REQUIRED: &str = r#"Context checkpoint required.

The visible context is above about 220,000 tokens and is now crowded. Continue the original task, but the next command_run output must include compact_context as the final command in the highest step after any required work in that batch.

The compact_context summary becomes the new handoff context and must preserve the task goal, completed work, incomplete work, deliverables, relevant files, validation state, and next steps. Keep it under about 15,000 English words."#;
