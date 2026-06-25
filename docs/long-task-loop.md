# Long Task Loop

Long tasks fail in boring ways. The agent forgets the original goal, mistakes a
provider retry for progress, keeps re-reading the same files, marks work done
after a shallow patch, or produces polished-looking code with no maintenance
sense. Tura treats those failures as runtime design problems.

## The Complaints

| Common failure | What it looks like |
| --- | --- |
| Infinite or sleepy loops | The agent keeps searching, restating, or retrying without changing the plan. |
| Fake self-reflection | The agent writes "I should verify this" but does not actually run the check. |
| Reverse logic drift | It optimizes for satisfying the prompt text instead of the underlying product behavior. |
| AI slop | Code compiles by accident, duplicates logic, ignores style, or leaves dead paths behind. |
| Context rot | Old tool output dominates the next turn and the actual user goal gets weaker. |
| Premature completion | The assistant says done before tests, media inspection, or user-visible deliverables are complete. |

## Tura's Runtime Answer

Tura puts structure around the loop:

- `task_status` records whether the current task is `doing`, `question`, or
  `done`.
- Completion prompts require a user-visible answer before terminal status.
- Provider retry prompts explicitly say transient provider failure is not task
  completion.
- `command_run` encourages batching investigation, edits, verification, and
  status in one ordered batch.
- Runtime prompt manuals add task-mode discipline only when relevant.
- Compact context handoffs replace stale transcript mass with a short record of
  what still matters.
- Business tests and typed runners make "it kind of worked once" less valuable
  than verified behavior.

The goal is not theatrical chain-of-thought. The goal is observable work:
inspect, change, verify, summarize, and keep the next turn oriented.

## Two-Minute Compression Window

When context is crowded, many agents degrade slowly: more repetition, less
attention, weaker edits. Tura's compact context path is designed to turn that
into a short maintenance action.

The runtime can require a checkpoint when context approaches the active limit.
The agent then uses `task_status.compact_context` inside `command_run` to write a
handoff summary. Runtime extracts that summary, clears retained tool-call
history, regenerates workspace snapshots, and continues the same task state.

In practice, the target shape is a two-minute-scale compression window: spend a
bounded turn preserving the goal, decisions, files, validation state, and next
steps, then continue with cleaner context.

Source entry points:

- [compact context prompt](../crates/runtime/src/prompt_style/compact_context.rs)
- [task_status prompt and schema](../crates/runtime/src/prompt_style/task_status.rs)
- [compact extraction](../crates/runtime/src/turn_loop/tool_step.rs)
- [runtime prompt manual reinsertion](../crates/runtime/src/prompt_style/runtime_prompt_manual.rs)

## Taste And Maintenance

Tura's docs and tests push the agent away from prompt-matching and toward
maintainable behavior:

- Prefer structured schema/protocol assertions over matching arbitrary prompt
  prose.
- Use existing codebase patterns before inventing abstractions.
- Verify with focused tests after edits.
- Treat dead code, dead branches, duplicate variables, and format drift as real
  product quality issues.
- Keep user-visible answers separate from internal task-state calls.

That is the difference between an AI slop generator and an assistant you would
let touch a real codebase twice.
