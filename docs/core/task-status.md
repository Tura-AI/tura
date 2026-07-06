# Task Status

`task_status` is Tura's internal task-management update command. It is exposed
as a `command_run` subcommand, but it does not edit files, call a shell, or talk
to a provider. Its job is narrow: update the session's active work area, task
state, runtime prompt task types, or compact-context handoff.

It is deliberately not a user-visible response channel. The assistant still has
to answer the user normally before marking a task `done` or asking a `question`.
State is not prose. Useful boundary, that one.

## What it updates

The command accepts four fields:

| Field | Meaning | Persistence effect |
| --- | --- | --- |
| `task_group` | A short broad work area such as `runtime documentation` or `storefront frontend`. | Updates the active task summary in `SessionManagement.task_plan`. |
| `status` | One of `doing`, `question`, or `done`. | Moves the active task-plan item through its lifecycle. |
| `task_type` | Array of runtime prompt manual ids. | Replaces `SessionManagement.task_type`, normalizes inherited manual ids, and may append missing manuals/capabilities. |
| `compact_context` | A concise handoff summary. | Extracted by the turn loop and converted into a `context_compaction` session-log record. |

The command schema lives in
[`crates/tools/src/commands/task_status/schema.json`](../../crates/tools/src/commands/task_status/schema.json).
The model-facing usage rules live in
[`crates/tools/src/commands/task_status/prompt.md`](../../crates/tools/src/commands/task_status/prompt.md)
and are also injected by runtime prompt-style helpers.

## Call forms

Inside `command_run`, `task_status` can be sent as structured arguments or as a
JSON command line:

```json
{
  "commands": [
    {
      "command_type": "task_status",
      "step": 1,
      "command_line": "{\"task_group\":\"runtime documentation\",\"task_type\":[\"editorial\"],\"status\":\"doing\"}"
    }
  ]
}
```

Minimal text also works for status-only updates:

```text
done
question
doing
```

The tool normalizer converts either form to:

```json
{
  "task_status": {
    "status": "doing",
    "task_group": "runtime documentation",
    "task_type": ["editorial"]
  }
}
```

That normalization is implemented in
[`crates/tools/src/commands/task_status/mod.rs`](../../crates/tools/src/commands/task_status/mod.rs).

## Validation and normalization

The command normalizer performs the first validation pass:

- `status` is optional, but when present it must be `doing`, `question`, or
  `done`.
- `task_group` and `compact_context` are strings.
- `task_type` must be an array; duplicates are removed.
- each `task_type` value must exist in the runtime prompt identity catalog.
- freeform JSON command text has control characters escaped if needed before
  parsing.

The available `task_type` ids are discovered dynamically from runtime prompt
identity files. In a source checkout, those identities live under
[`crates/runtime/src/runtime_prompt`](../../crates/runtime/src/runtime_prompt). A
custom root can be supplied through `TURA_RUNTIME_PROMPT_ROOT`.

## Runtime application path

`task_status` output is not applied directly by the command implementation. The
runtime applies it after tool execution:

1. The model calls `command_run` with one or more commands.
2. `command_run` executes the batch and returns a result list.
3. Runtime calls `apply_tool_result_session_state_update` for `command_run`
   results.
4. That helper scans successful result items whose `command_type` is
   `task_status`.
5. The helper mutates `SessionManagement` and publishes updated task todos when
   state changed.

The runtime-side logic is in
[`crates/runtime/src/tool_flow/task_status.rs`](../../crates/runtime/src/tool_flow/task_status.rs).

## `task_group` behavior

`task_group` is intentionally broad. It should name the work area, not the
specific current step.

Good examples:

- `CLI documentation`
- `checkout frontend`
- `order settlement service`

Bad examples:

- `Add a spinner to the button`
- `Tell the user it is fixed`
- `Create a slide deck about Constantinople`

When runtime applies a new `task_group`, it ensures there is at least one active
task-plan item. If the current active task still has the previous user goal or
plan summary as its task summary, the group can replace that summary. If the
task is already specific, runtime can create or activate a separate task-plan
item instead of flattening useful detail.

## `status` behavior

`status` changes the active task-plan item, not the whole conversation record by
itself.

| Status | Runtime meaning | Assistant obligation |
| --- | --- | --- |
| `doing` | Work is active and more tool calls are expected. | Use only when additional `command_run` calls are required. |
| `question` | Work is blocked on user input, permission, credentials, or an environment condition. | First send the user-facing question/blocker, then update state. |
| `done` | The active task is complete. Runtime may advance or finish the task. | First send the final answer with files and verification, then update state. |

The prompt explicitly forbids marking `done` when required verification failed,
timed out, was skipped, or could not start. For visual/media work, media must be
inspected before `done`. The important bit: `done` means finished and verified,
not merely "I stopped typing".

## `task_type` behavior

`task_type` selects Runtime Prompt operation manuals. The array is treated as the
complete set for the active task, not as a one-off note.

When runtime applies a `task_type` update:

1. Values are normalized through the runtime prompt manual catalog.
2. Father manuals are inserted before child manuals. For example,
   `interactive_and_3d` expands to `visual`, `frontend`, then
   `interactive_and_3d`.
3. `SessionManagement.task_type` is replaced with the normalized list.
4. Operation manual injection is enabled unless the session explicitly disabled
   manuals.
5. Missing runtime prompt manual records and command-run capability records are
   appended to the session log.

This is why a single task can legitimately have multiple task types. A frontend
visual task may need `visual` and `frontend`; a slide/PDF task may need `visual`
and `editorial`.

## Startup gate

When the current session has no `task_type`, runtime injects a stricter reminder:

> Before any `apply_patch` command or write-producing shell command, define
> `task_type` based on the current context and the user's request, and include
> `task_group` in the same update.

Non-writing discovery can run in the same `command_run` batch as that update.
This lets the agent inspect the repo before deciding the task type, while still
preventing write operations under an undefined operating manual.

The startup gate is assembled in
[`crates/runtime/src/prompt_style/task_status.rs`](../../crates/runtime/src/prompt_style/task_status.rs)
and enforced defensively in the command execution path by discarding startup
`apply_patch` writes when the gate has not been satisfied.

## `compact_context` behavior

`compact_context` is the bridge from task status to context management. It is a
handoff summary, not normal task state.

When a successful `task_status` command includes `compact_context`:

1. The turn loop extracts the summary from the `command_run` result.
2. It strips the raw `compact_context` text out of the stored command/result so
   the giant handoff is not duplicated in normal tool history.
3. It captures the visible assistant text from the current runtime turn, if any.
4. It later calls the context compaction path, which writes a compact session-log
   record and reinserts active manuals.

Extraction and stripping live in
[`crates/runtime/src/turn_loop/tool_step.rs`](../../crates/runtime/src/turn_loop/tool_step.rs).

## Relationship to planning

`task_status` and `planning` are related but separate.

- `planning` can replace the active task-plan topology with structured steps.
- `task_status` updates the current task state and runtime prompt task types.
- Both can trigger session state changes through
  `apply_tool_result_session_state_update`.

If a `planning` result includes steps, runtime replaces the active task with the
incoming planned steps, renumbers them, activates the first user-action task when
needed, and records a `task_topology_applied` log entry. `task_status` then marks
the active item as `doing`, `question`, or `done` as the task progresses.

## Persistence model

The session state fields affected by `task_status` are part of
`SessionManagement`:

- `task_type`
- `session_capabilities`
- `task_plan`
- `session_log`
- `session_last_update_at`
- `op_manual_enabled`

The gateway/session database layer stores session-management JSON so TUI, GUI,
and CLI clients can all see the same task state. This is why `task_status` is
designed as a structured runtime update rather than a decorative chat message.

## Failure and non-use cases

Use `task_status` only when state actually needs to change or a checkpoint is
needed. Avoid it for ordinary conversation.

Do not use it to:

- answer the user;
- hide a missing final response;
- mark work done before verification;
- record progress prose that belongs in the assistant message;
- replace real planning or real test output;
- ask a question without first asking the user in the assistant channel.

The command is useful because it is boring and strict. Keep it that way.
