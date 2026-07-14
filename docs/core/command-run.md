# Command Run

`command_run` is Tura's model-visible execution surface for terminal-native
work. The provider sees one compact tool instead of negotiating every local
action in a separate round trip. Tura routes each batch item to internal commands
such as `shell_command`, `bash`, `zsh`, `apply_patch`, `web_discover`,
`read_media`, `generate_media`, `task_status`, and optional `planning`.

The GUI i18n text exposes the product shape directly: command blocks are shown
with labels such as `runCommands` / `runningCommands`, and individual command
items are rendered as `commandTypeShell`, `commandTypePatch`,
`commandTypeReadMedia`, `commandTypeWebDiscover`, or
`commandTypeCompactContext`. In other words, the user sees one command run, not
a pile of disconnected tool calls.

## Practical difference from ordinary agent tooling

Ordinary tool calling makes every file read, patch, test, and status update
compete for another provider-visible turn. Tura's difference is not a nicer JSON
shape; it is fewer LLM round trips around the same local work.

Example: fix a small documentation bug safely.

| Work item | Ordinary agent loop | Tura with `command_run` |
| --- | --- | --- |
| Read files and references | Turn 1: the model calls search/read tools, then waits for results. | Step 1: run `rg --files`, targeted `rg -n`, and `Get-Content` together. |
| Decide the edit | Turn 2: the model sees search output and asks for more reads or prepares a patch. | Same provider turn when the needed reads were already known and batched. |
| Apply the patch | Turn 3: the model calls an edit tool. | Step 2: `apply_patch` runs after step 1. |
| Validate | Turn 4: the model calls tests, build, or lint after the patch result returns. | Step 3: known validation runs after the patch. |
| Persist task state | Often another tool call or prose-only bookkeeping. | The same batch can include `task_status` when state actually changes. |

In that common case, ordinary tooling needs about four provider-visible LLM
turns. `command_run` can execute the same bounded local sequence in one provider
turn when the dependencies are already known. If the active prompt is 40k input
tokens, four ordinary turns replay roughly 160k input tokens before counting
tool schemas and result payloads; one `command_run` turn replays that base
context once. The exact multiplier depends on the task, but the waste is easy to
spot: repeated context, repeated tool-choice latency, and repeated model
planning for work the runtime can schedule deterministically.

The second difference is scheduling. Ordinary agents usually serialize tool
calls because the model has to wait after each call. Tura's `step` groups let
the runtime run independent reads together and only serialize real dependencies.
That is why the command schema says same-step commands must have no output
dependency on each other. It is not decoration. It is the difference between
using the machine and politely asking the machine four times.

The actual model-facing prompt reinforces this behavior. The checked-in
`command_run` schema tells the model to "complete all currently needed steps in
one batch", to prefer five or more commands during real task execution, to put
independent reads/searches/lists in the same step, and not to invent probes that
depend on unknown earlier output. Command-specific prompts then add the sharp
edges: `apply_patch` must be a raw patch body, shell commands need bounded
service readiness checks, and `task_status.compact_context` belongs after the
work it summarizes.

Source of truth:

- [provider schema](../../crates/tools/src/command_run/schema.json)
- [argument parser](../../crates/tools/src/command_run/handler_parse.rs)
- [executor and scheduler](../../crates/tools/src/command_run/handler.rs)
- [agent command injection](../../crates/runtime/src/manas/tool_catalog.rs)
- [streamed execution](../../crates/runtime/src/provider_flow/command_run_streaming.rs)
- [streamed records](../../crates/runtime/src/provider_flow/streamed_command_run.rs)

## Why Tura Uses One Tool

`command_run` keeps the provider interface small and stable:

1. The model emits one `command_run` call.
2. The call contains a `commands` array.
3. Each item names a `command_type`, a `command_line` payload, and a `step`.
4. Tura executes safe same-step commands together, serializes mutating work, and
   waits for later steps only after earlier steps complete.
5. Tura returns one normalized result envelope.

This reduces token overhead and latency on command-heavy development tasks. It
does not make every task cheaper by magic. The gain appears when several local
actions are already known and can be batched in one provider-visible call.

## Model-Visible Schema

The checked-in schema is intentionally small:

```json
{
  "name": "command_run",
  "description": "Run tools as a pure batch+step command runner.",
  "input_schema": {
    "type": "object",
    "required": ["commands"],
    "additionalProperties": false,
    "properties": {
      "commands": {
        "type": "array",
        "minItems": 5,
        "maxItems": 20,
        "items": {
          "type": "object",
          "required": ["command_type", "command_line"],
          "additionalProperties": false,
          "properties": {
            "command_type": { "type": "string" },
            "command_line": { "type": "string" },
            "step": { "type": "integer", "minimum": 1 }
          }
        }
      }
    }
  }
}
```

At runtime, Tura tightens that schema for the active agent. It injects the
allowed `command_type` enum, selects the active shell surface, and appends
command-specific format instructions to the description. The default command set
is `apply_patch`, the active shell, `web_discover`, and `task_status`. Manuals
and agent capabilities can add commands such as `read_media`, `generate_media`,
and `planning`.

The schema above is the shape the model emits for the current tool call. It is
not the shape used when an old `command_run` is replayed into later provider
context. Replay keeps the old provider transcript shape: a completed
`function_call` immediately followed by its matching `function_call_output`.
The call arguments contain the original `command_run` input after runtime-only
reporting fields are stripped:

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

This is intentionally the same legal provider shape produced during the original
tool call. Tests enforce that replayed `function_call_output` items are paired
with a preceding `command_run` `function_call`; orphan outputs are invalid. The
output projection omits `step`, `command_type`, and `command_line` because the
paired `function_call.arguments` already contains the command input.

## Command Item Fields

| Field | Required | Meaning |
| --- | --- | --- |
| `command_type` | Yes | Internal command name. Common values are `shell_command`, `bash`, `zsh`, `apply_patch`, `web_discover`, `read_media`, `generate_media`, and `task_status`. |
| `command_line` | Yes | String payload for the target command. Shell commands may be plain text or escaped JSON. `apply_patch` uses the raw patch body. External commands such as `web_discover` and `read_media` can use compact CLI-style text. |
| `step` | Required in the provider schema after runtime injection | Dependency group. Same-step commands must not depend on each other's output. Higher steps wait for lower steps. Low-level execution can infer steps from command order when omitted. |

The parser also accepts compatibility shapes: top-level `requests`, legacy
top-level `steps`, aliases such as `command`, `cmd`, `tool`, `name`,
`tool_name`, `commandLine`, `command_code`, `input`, `args`, `code`, `script`,
and `payload`, plus top-level or per-item `workdir` and `timeout_ms`. New docs
and examples should use `command_type`, `command_line`, and `step`.

## Step Semantics

`step` is a dependency group, not a serial command number.

Use the same step for independent reads:

```json
{
  "commands": [
    {
      "command_type": "shell_command",
      "step": 1,
      "command_line": "rg --files"
    },
    {
      "command_type": "shell_command",
      "step": 1,
      "command_line": "rg -n \"command_run\" crates docs"
    }
  ]
}
```

Use a later step when a command depends on earlier output or completed edits:

```json
{
  "commands": [
    {
      "command_type": "apply_patch",
      "step": 1,
      "command_line": "patch body that updates README.md"
    },
    {
      "command_type": "shell_command",
      "step": 2,
      "command_line": "cargo test -q"
    }
  ]
}
```

The executor groups commands by normalized step. Same-step read-only macro
commands can run concurrently. Commands that are not safe macro commands run on
the exclusive path. If a model emits backwards step numbers, the runtime repairs
them to preserve deterministic ordering instead of running a later dependency
too early.

## Scheduling And Locks

The scheduler works from command access, not model intent.

- Read-only macro commands in the same step may execute together.
- `apply_patch` is always treated as exclusive.
- Mutating shell commands are serialized through a workspace-level write lock.
- Read-only shell commands can share a step when `shell_executor` recognizes
  them as read-only.
- External commands such as `web_discover`, `read_media`, and `generate_media`
  provide access metadata through the external command launcher.
- A failed `apply_patch` cancels later commands with the reason
  `apply_patch failed; command_run stopped before later commands`.

This is why a batch can safely contain discovery, focused edits, and validation
without turning the model into a shell scheduler. Models are bad at that job; the
runtime is less impressionable.

## Command Payload Formats

### Shell Commands

Use the active shell command name exposed in the schema. On Windows the default
is `shell_command`; on macOS it defaults to `zsh`; on Linux it defaults to
`bash`. On Windows, `shell_command` resolves PowerShell through PATH or the
standard system install paths before falling back to `cmd.exe`; it does not
trust an unresolved bare `pwsh`/`powershell` name. `TURA_COMMAND_RUN_SHELL` can
force `shell_command`, `bash`, or `zsh`.

Plain text is accepted:

```json
{
  "command_type": "shell_command",
  "step": 1,
  "command_line": "rg -n \"TODO\" crates docs"
}
```

Escaped JSON is also accepted when the shell command needs `workdir` or timeout
metadata inside the payload:

```json
{
  "command_type": "shell_command",
  "step": 1,
  "command_line": "{\"command\":\"cargo test -q\",\"workdir\":\"crates/tools\",\"timeout_ms\":120000}"
}
```

Top-level and per-command `workdir` and `timeout_ms` are normalized into the
shell payload when missing.

### `apply_patch`

`apply_patch` is a raw freeform patch command. Keep each patch focused on one
file or one coherent code block. Batch multiple `apply_patch` commands instead
of hiding unrelated edits in one large patch.

The patch body must start with `*** Begin Patch`. In this Markdown document the
full sentinel sequence is described in prose instead of embedded as a literal
patch block, because nested patch sentinels are easy to mangle while editing the
documentation itself. Tiny trapdoor. Very funny.

### External CLI-Style Commands

Commands such as `web_discover`, `read_media`, and `generate_media` accept
compact CLI-style strings. The command runner wraps those strings as the
external command's `cli` argument unless the payload is already JSON.

```json
{
  "command_type": "web_discover",
  "step": 1,
  "command_line": "website \"OpenAPI docs\" --max-results 3"
}
```

### `task_status`

`task_status` is handled directly by the command runner. It updates internal
task state and does not replace a normal user-visible assistant response.

```json
{
  "command_type": "task_status",
  "step": 1,
  "command_line": "{\"status\":\"doing\",\"task_group\":\"developer docs\",\"task_type\":[\"editorial\"]}"
}
```

`task_status.compact_context` has stricter placement rules: only one compact
context command is allowed, it must be the final command, and it must be in the
highest step.

## Streaming Execution

When provider streaming is active, Tura does not have to wait for the final
assistant message before starting work. As command items become available, the
runtime normalizes them, attaches command identity fields, checkpoints start and
finish events, and publishes live updates for UI and CLI surfaces.

Streaming still respects step order:

- commands in the active step can start when ready;
- later steps wait until the active step has no running or pending commands;
- results are ordered back into the provider command order;
- halted command batches mark the streamed command run as cancelled or errored.

The visible live fields include command status, command identity, command index,
step, command type, sanitized result output, and timestamps.

## Output Shape

Every command returns a normalized item in `results`:

```json
{
  "results": [
    {
      "step": 1,
      "command_type": "shell_command",
      "command_line": "rg --files",
      "success": true,
      "output": {
        "exit_code": 0,
        "stdout": "...",
        "stderr": ""
      }
    }
  ]
}
```

When the batch is stopped after a failed patch, the top-level output includes
`cancelled: true` and `cancel_reason`. Sandbox blocks are returned as failed
command items with shell-like output, exit code `126`, and `SandboxViolation`
metadata.

## Sandbox And Safety

`TURA_COMMAND_RUN_SANDBOX=1` enables workspace-bound checks for commands that can
write or change working directories:

- `apply_patch` paths must remain inside the session workspace;
- shell `workdir` / `cwd` must remain inside the session workspace;
- blocked commands return a model-visible failure instead of silently doing
  nothing.

Without sandbox mode, the lower-level command behavior applies. That is useful
for trusted local development, but it is not a substitute for reading the command
before running it. Sharp tools remain sharp.

## Recommended Usage Pattern

- Put independent reads in the same step.
- Put edits in a later step after the facts they depend on are known.
- Put validation after edits in a later step.
- Use bounded timeouts for long-running checks.
- Keep `apply_patch` focused and reviewable.
- Do not create probes in the same batch that depend on output from earlier
  commands whose output is not known yet.
- Use `task_status` only for internal task state transitions.
- Use `read_media` after generating or preparing media artifacts before showing
  them to the user.

## Useful Implementation Tests

The current behavior is covered by tests around command shape compatibility,
patch handling, locking, sandbox checks, streaming, and pressure scheduling:

- [command shape compatibility](../../crates/tools/tests/business/command_run_current/command_shapes.rs)
- [apply_patch, streaming, and locks](../../crates/tools/tests/business/command_run_current/apply_patch_streaming_locks.rs)
- [shell process hooks](../../crates/tools/tests/os_testing/command_run_shell_process_hooks.rs)
- [command-run pressure test](../../crates/tools/tests/performance/command_run_pressure_test.rs)
