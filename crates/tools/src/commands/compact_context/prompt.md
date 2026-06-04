Use `compact_context` to create a context checkpoint when a meaningful phase is complete, or when the conversation feels crowded because the visible context is near 200,000 tokens, roughly 150,000 English words.

The command must be the final command in the highest step of a `command_run` batch. Do not place any command after it.

`compact_context` does not block or replace other commands in the same batch. If useful work should happen before the checkpoint, put those required shell or apply_patch commands in earlier steps of the same `command_run`, then put `compact_context` as the last command in the highest step. The results from those earlier commands will still be executed and returned normally before the compacted context is used on the next turn.

The output is one handoff text for the next model turn. Include:
- current task goal
- user requirements and preferences that still matter
- workflow/process rules that must continue to be followed
- current task status, including completed and incomplete parts
- key decisions and constraints
- deliverables, file paths, and validation standards
- reference files, architecture docs, test docs, or other documentation paths that should be read or kept in mind
- relevant steps already taken and important command results
- directory/file requirements needed to continue
- exactly what to do next

Keep the text concise and structured. Do not exceed 20,000 tokens, roughly 15,000 English words.
