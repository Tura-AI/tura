# Context Management

Context management answers a practical question: how does a long session keep
working without carrying its entire attic into every provider call? Tura stores
raw session events, builds provider messages from them, caches compact
tool-result context, and replaces crowded history with structured compaction
records when needed. The active objective gets the desk; old tool output does
not get every chair.

## Practical difference from ordinary agent context

Many agent stacks treat context as a growing transcript plus a pile of reusable
instructions. When the context gets crowded, they often call another model turn
whose only job is to summarize the old chat. That turn has to read the crowded
history, produce a summary, and then a later turn has to read that summary before
work can continue. In practice, compaction becomes extra latency, extra input
tokens, extra output tokens, and another point where the model can flatten the
actual work into "we ran some tests". Helpful, in the same way fog is a map.

The bigger issue is execution continuity. A separate summary turn usually cannot
preserve the concrete provider tool-call shape from prior turns. Function-call
ids, command batches, command outputs, media previews, task-status updates, and
validation evidence get converted into prose. If the next model needs exact
state, it has to spend more turns re-reading files, re-running commands, or
asking the user what happened. The old tool call cannot be continued; it has to
be reconstructed.

Tura does not manage context as "paste everything and hope", and it does not
treat compaction as a literary summary agent. It records the raw session log, but
rebuilds provider messages from structured records. Tool results can keep compact
context views, active Runtime Prompt manuals are tracked as records, and crowded
history is replaced by a `context_compaction` checkpoint with the goal, evidence,
workspace snapshot, environment context, recent tool context, and next action.

Example: a long refactor already searched the repo, patched three files, and ran
tests with a 3,000-line failure log.

| Context problem | Ordinary agent behavior | Tura behavior |
| --- | --- | --- |
| Compaction cost | Spend a separate summarizer agent turn reading old context and writing a summary before useful work resumes. | Convert `task_status.compact_context` from the current `command_run` turn into a `context_compaction` record; no standalone summarizer turn is required for manual checkpoints. |
| Tool-call continuity | Collapse tool calls into prose, losing call ids, command batches, exact command lines, and structured success/error data. | Store `tool_result` records with compact `context_cache` and provider-shaped `context_messages` when possible. |
| Rebuilding after reset | Re-run searches/tests or ask the user to recover facts that were in prior tool output. | Replay compact command evidence, immediate pre-checkpoint tool tail, workspace snapshot, and environment context. |
| Prompt manuals | Paste all generally useful instructions or rely on a low-priority skill trigger. | Reinsert only active Runtime Prompt manuals selected by `task_status.task_type`. |
| Task memory | Depend on the model remembering the old chat. Quaint. | Persist `current_objective`, `task_type`, task state, session capabilities, and compaction records in session state. |
| Media evidence | Summarize images/audio/doc inspection as text or drop binary previews. | Rebuild `read_media` context with compact text plus image/file input items when provider support allows it. |
| Reset after crowding | Summarize loosely, often losing files, commands, and validation state. | Create a structured checkpoint with explicit retained/omitted history metadata. |

This matters for token cost and correctness. If a normal agent carries a 20k
base prompt, 15k of skills/manuals, and 30k of old tool output into every call,
each follow-up starts around 65k input tokens before the new user request. If it
then uses a separate summarizer turn, that turn pays to read the same bloated
context and to emit summary tokens, and the next work turn pays again to read the
summary. The visible cost is extra input tokens, extra output tokens, and another
round-trip before the real task continues. Tura's goal is to keep durable facts
as structured state and replay only the pieces needed for the next step.
Compaction is not a shorter chat transcript; it is a runtime checkpoint.

The actual compact-context prompt is intentionally operational, not literary. It
requires the assistant to call `command_run`, put any required final checks
first, then finish the highest step with `task_status.compact_context`. The turn
loop extracts that handoff from the command result, strips it out of the stored
command payload to avoid replay noise, and writes a `context_compaction` record.
The handoff must preserve the goal, completed work, incomplete work,
deliverables, relevant files, validation state, and next steps. That is the
difference between "summarize our chat" and "make the next model able to
continue the job without wasting three turns rediscovering its own footprints".

## Execution continuity details

The practical details are where the design earns its keep:

- `extract_compact_context_results` reads successful `task_status` results inside
  `command_run`, captures the handoff text, captures the assistant's visible
  message when available, and removes the raw `compact_context` text from the
  stored command result.
- `compact_session_context_with_options` writes one structured
  `context_compaction` record, resets provider token usage, resets session
  capabilities to the baseline command set, records the retention boundary, and
  re-appends active Runtime Prompt manuals.
- The compaction record stores a workspace snapshot and environment context, so
  the next turn does not need to infer the working directory, shell, date,
  language, or visible repository shape from stale chat.
- Compaction keeps a timestamped timeline of relevant user, assistant,
  user-agent, tool, and inherited compact-context entries, then trims lower-value
  entries to fit the compact budget.
- Immediate pre-checkpoint tool results are replayed as compact context messages,
  so a final verification command run just before checkpointing can still be
  visible after the old transcript is removed.
- `tool_result_context_cache` stores compact output/error views with a stable
  cache id. For `command_run`, context rebuilding recreates the provider-shaped
  `function_call` / `function_call_output` pair so the provider receives a legal
  transcript with no orphan tool outputs.
- Command-run context keeps step, command type, command line, success, compact
  output, and compact error. Reporting fields such as command ids, UI event ids,
  receipts, timestamps, and provider-only metadata are stripped unless they are
  needed for task-status continuity.
- `read_media` output is treated specially: bulky preview payloads are stripped
  from text, but visual/file input items can be rebuilt so inspected media can
  remain visible to the provider.

The result is that a later turn can continue from the prior operational state
instead of spending new turns rebuilding it from vague prose. The raw audit trail
still exists in the session log; the provider context gets the compact execution
view that helps it move.

The implementation lives mostly under
[`crates/runtime/src/context`](../../crates/runtime/src/context), with session
state fields defined in
[`crates/runtime/src/state_machine/session_management.rs`](../../crates/runtime/src/state_machine/session_management.rs).

## Design goals

Context management is built around five invariants:

1. The session log is the durable source of runtime memory.
2. Provider prompts are rebuilt from structured records, not from a flat chat
   transcript pasted forward forever.
3. Tool results are stored with compact context views so later turns can replay
   useful evidence without replaying all raw output.
4. Compaction preserves active goals, recent evidence, workspace state, and
   operation manuals.
5. Prompt bloat should be handled by explicit checkpoints, not by hoping the
   model remembers what matters.

## Main session fields

The relevant `SessionManagement` fields are:

| Field | Purpose |
| --- | --- |
| `session_log` | Ordered JSON-string records for messages, tool results, prompt manuals, compaction records, and task-plan events. |
| `session_log_retention` | Tracks how much history was omitted after the latest compaction. |
| `session_started_at` | Separates current-run records from previous-task history. |
| `input.user_input` | Current user turn text, used to reconstruct the active user message. |
| `current_objective` | Active task scope used by planning/reflection prompts. |
| `task_type` | Active runtime prompt manual ids. |
| `session_capabilities` | Command-run capabilities already injected into the current context. |
| `context_tokens` | Latest provider-reported input count and active context limit. Default limit is 260,000 tokens. |
| `runtime_usage` | Latest usage/cost payload; reset after compaction. |

`SessionManagement.push_log` is the common persistence boundary: most runtime
events become serialized JSON entries in `session_log`.

## Building provider context

The provider context path starts at `build_context` in
[`crates/runtime/src/context/build.rs`](../../crates/runtime/src/context/build.rs).

At a high level:

1. `build_messages_from_session_with_options` converts `session_log` into
   provider messages.
2. Current runtime reasoning and assistant text are added when the session log is
   empty or when reasoning should be preserved in `ContextState`.
3. Runtime tool-call summaries are copied into `ContextState.tool_results`.
4. The last tool-call response may be attached to `ContextState` when
   `use_last_tool_call_response` is enabled.
5. Additional messages from prompt assembly are appended.
6. The final `ContextOutput` returns the possibly updated session, provider
   messages, and inspectable context state.

`ContextState` is not the provider prompt itself. It is the runtime's structured
view of what it assembled: messages, tool result summaries, last tool response,
and reasoning history.

## Session-log record types

Common records in `session_log` include:

| Record shape | Created by | Context use |
| --- | --- | --- |
| `{ "role": "user" | "assistant" | ... }` | `accumulate_message` and prompt-style insertion paths | Replayed as provider messages. |
| `{ "type": "tool_result", ... }` | `accumulate_tool_result_with_provider_metadata` | Replayed through cached compact context messages. |
| `{ "type": "context_compaction", ... }` | compaction path | Replaces older history with compact handoff plus workspace/environment context. |
| `{ "type": "runtime_prompt_manual", ... }` | runtime prompt manual injection | Replays active operation manual text. |
| `{ "type": "runtime_prompt_command_run_capabilities", ... }` | runtime prompt capability injection | Replays command format extensions and records loaded capabilities. |
| `{ "type": "task_focus", ... }` | task-progress helpers | Records active planned task focus. |
| `{ "type": "task_topology_applied", ... }` | planning/task-status update path | Audits replacement of active task topology. |

The builder accepts normal provider roles (`user`, `assistant`, `system`,
`developer`) and Tura's `user-agent` context role. Unsupported or malformed
records are ignored rather than breaking the whole prompt rebuild.

## Tool-result context caching

Tool results can be huge. A command run may contain long stdout, image payloads,
media metadata, provider tool call ids, event ids, and reporting fields that are
useful for storage but wasteful in model context.

When a tool result is accumulated, runtime stores three related views:

| Field | Purpose |
| --- | --- |
| `output` | Sanitized raw output for session history and diagnostics. |
| `context_cache` | Stable compact summary with a cache id, compact output, and compact error. |
| `context_messages` | Provider-message representation used when rebuilding context. |

The code strips reporting-only fields such as command ids, timestamps, summaries,
receipt metadata, and UI event ids. It preserves task-status fields inside
`task_status` results so state updates remain auditable.

For `command_run`, cached context is represented as a provider function-call
pair when provider metadata includes a call id. Without that metadata, it falls
back to a user message containing compact output. Media tool results are handled
specially so inspected media can remain visible without keeping raw payload
noise in every record.

When a previous `command_run` is replayed into provider context, runtime keeps
the legal provider transcript shape:

```json
{
  "type": "function_call",
  "call_id": "call_...",
  "name": "command_run",
  "arguments": "{\"commands\":[{\"step\":1,\"command_type\":\"shell_command\",\"command_line\":\"rg TODO\"}]}"
}
{
  "type": "function_call_output",
  "call_id": "call_...",
  "output": "{\"results\":[{\"success\":true,\"output\":{\"exit_code\":0,\"stdout\":\"...\",\"stderr\":\"\"}}]}"
}
```

Cached `context_messages` with orphan outputs, empty call arguments, or command
identity fields duplicated in `function_call_output.output` are treated as stale
and rebuilt instead of reused. `read_media` uses the same paired replay shape,
but its `function_call_output.output` can be provider media content items rather
than a JSON string so images or files can be passed through the media channel.

Relevant code:

- [`crates/runtime/src/context/tool_results.rs`](../../crates/runtime/src/context/tool_results.rs)
- [`crates/runtime/src/context/media.rs`](../../crates/runtime/src/context/media.rs)
- [`crates/runtime/src/tool_callback_sanitizer.rs`](../../crates/runtime/src/tool_callback_sanitizer.rs)

## Output budgets

Context management uses byte/character budgets before provider token accounting
is available:

| Constant | Value | Meaning |
| --- | --- | --- |
| `CONTEXT_OUTPUT_MAX_CHARS` | `10_000` | General compact output limit. |
| `COMMAND_RUN_RESULT_OUTPUT_MAX_CHARS` | `10_000` | Command-run result output limit. |
| `COMPACT_CONTEXT_ESTIMATED_TOKEN_BYTES` | `4` | Estimated token size for compaction budgeting. |
| fallback compaction budget | `25_500` estimated tokens | Used when no active token limit is known. |

When real provider usage is available, `SessionManagement.context_tokens.limit`
sets the active limit. Compact context targets about one tenth of that limit.
With the default 260,000-token limit, a compact checkpoint targets roughly
26,000 estimated tokens before truncation.

## Manual compaction flow

Manual compaction starts with a `task_status` command containing
`compact_context`:

```json
{
  "status": "doing",
  "compact_context": "Continue by running cargo test -p runtime. The new docs are in docs/core..."
}
```

Then the turn loop:

1. scans `command_run` results for successful `task_status.compact_context`;
2. stores a `PendingCompactContext` with the summary and the visible assistant
   text from the current turn;
3. strips the bulky `compact_context` field from command arguments and results;
4. applies normal task-state changes separately;
5. calls `compact_session_context_with_agent_message_and_capabilities` later in
   the turn.

The extraction path is in
[`crates/runtime/src/turn_loop/tool_step.rs`](../../crates/runtime/src/turn_loop/tool_step.rs).

## Compaction record creation

`compact_session_context_with_options` writes the actual checkpoint. It builds a
new `context_compaction` record containing:

- compacted handoff content;
- current workspace snapshot;
- current environment context;
- timestamp;
- retention boundary metadata in `session_log_retention`.

The compacted content contains:

1. a timestamped context history selected from the old session log;
2. the agent's explicit handoff summary;
3. goal-mode last user command, if goal mode is active;
4. an omission line if older entries were dropped to meet the budget.

After writing the record, compaction resets `context_tokens.input`, clears
`runtime_usage`, resets session capabilities to the baseline command set, records
the compaction point, and reinserts active runtime prompt manuals.

Relevant code:

- [`crates/runtime/src/context/compaction.rs`](../../crates/runtime/src/context/compaction.rs)
- [`crates/runtime/src/context/workspace.rs`](../../crates/runtime/src/context/workspace.rs)
- [`crates/runtime/src/context/text_truncate.rs`](../../crates/runtime/src/context/text_truncate.rs)

## What compaction keeps

The compaction timeline is intentionally selective.

It keeps:

- recent inherited compact summaries, capped at two;
- current-run user messages;
- current-run assistant-visible text;
- current-run `user-agent` context blocks;
- current-run tool evidence;
- last relevant previous-task assistant message;
- explicit agent handoff summary;
- goal-mode fallback or last goal command;
- immediately pre-checkpoint tool results as context messages.

It skips or reduces:

- prompt-style scaffolding records;
- older compact summaries beyond the inherited-summary limit;
- old tool output that is no longer near the checkpoint boundary;
- context-reporting fields that do not help the model continue.

This is why compaction is not just "summarize the chat". It reconstructs a
working state with timestamps, workspace evidence, and active runtime manuals.

## Rebuilding from a compaction record

When `build_messages_from_session_with_options` encounters a
`context_compaction` record, it calls `context_compaction_messages`.

That function emits provider messages in this order:

1. developer message with the workspace snapshot;
2. developer message with environment context;
3. user message with compact handoff content;
4. compact provider messages for immediate pre-checkpoint tool results;
5. reflection/planning objective context when reflection mode is enabled.

After compaction, runtime prompt manuals are re-appended as normal session-log
records, so the next turn sees the same active operation manuals without having
to preserve the entire pre-compaction transcript.

## Automatic compaction and context pressure

The same compaction function can be called automatically when context approaches
the active limit. Automatic compaction uses the same record format and retention
logic; the difference is simply who supplied the handoff text. Manual handoffs
are better because the agent can preserve task-specific facts, decisions, and
next steps explicitly. Automatic compaction is the safety net.

The user-facing prompt reminds the agent to use `compact_context` when a
meaningful phase is complete, when previous context is mostly irrelevant, or
when the active context reaches the hard cap.

## Interaction with runtime prompts

Runtime prompt manuals and context management are tightly connected:

- active manuals are session-log records;
- compaction stops older records from bloating the prompt;
- after compaction, active manuals are appended again;
- command-run capabilities are reset and then re-added from active manuals;
- `task_type` survives because it is session state, not transcript text.

This preserves the operating mode after history is trimmed. A visual task remains
a visual task after compaction; a debug task remains a debug task. Otherwise the
model would wake up with amnesia and a wrench. Bad combination.

## Operational guidance

Use `compact_context` when:

- the task is entering a new phase;
- the session has accumulated large tool outputs;
- the next turn needs only a concise handoff plus key files and commands;
- active context is near the configured limit;
- a long-running task should continue after a provider retry or context reset.

Do not use it when:

- the current transcript is still short and directly relevant;
- the handoff would merely repeat visible conversation;
- the task is already complete;
- required evidence has not been read or captured yet.

A good handoff names the current goal, active task types/manuals, user
constraints, completed work, incomplete work, file paths, validation results,
known blockers, and the exact next action.
