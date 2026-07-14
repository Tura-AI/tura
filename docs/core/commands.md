# Commands

Commands are Tura's local execution units: the concrete operations that
`command_run` can route, schedule, lock, execute, normalize, and record. Some run
ordinary CLI programs. Others are Tura-native operations or external command
packages with their own schemas. Giving them one execution model keeps the
runtime consistent without pretending every operation is merely a shell line in
a nicer coat.

The important boundary is:

| Layer | What it is | Example |
| --- | --- | --- |
| Tool | Provider-visible interface the model can call. | `command_run` |
| Command | Tura-owned executable operation selected inside `command_run`. | `shell_command`, `apply_patch`, `web_discover`, `task_status` |
| CLI command | A local program or shell expression run by a shell command. | `rg --files`, `cargo test -q`, `npm test` |

So `rg --files` is a CLI command. `shell_command` is a Tura command that can run
that CLI command. `command_run` is the model-visible tool that carries the
`shell_command` item. Three layers. One job. Fewer sharp edges.

## Why commands exist

Most provider tool systems expose many individual tools directly to the model:
one tool for shell execution, one for patches, one for web search, one for media
inspection, one for task state, and so on. That works, but it has costs:

- every tool schema consumes prompt space;
- every small action often needs another model-visible turn;
- the model has to reason about ordering, dependencies, and parallelism itself;
- tool outputs arrive as scattered records instead of one auditable execution
  batch;
- safety and locking policy is duplicated across tools or left implicit.

Tura keeps the provider surface narrow. The provider sees one main tool,
`command_run`. Inside that call, the model sends a list of command items. The
runtime dispatches each item to the matching local command implementation.

That split lets Tura keep provider integration simple while still supporting a
rich local command system.

## Commands vs CLI commands

A CLI command is something a shell can run:

```powershell
rg -n "command_run" crates docs
cargo test -q -p code-tools
npm --prefix apps/tui test
```

In Tura, those lines are payloads for a shell command type:

```json
{
  "command_type": "shell_command",
  "step": 1,
  "command_line": "rg -n \"command_run\" crates docs"
}
```

The Tura command is `shell_command`. The CLI command is the string inside
`command_line`.

That distinction matters because not every Tura command is shell-backed:

| Tura command | Shell-backed? | What it does |
| --- | --- | --- |
| `shell_command`, `bash`, `zsh` | Yes | Runs a local shell command with timeout, working-directory handling, process control, and output shaping. |
| `apply_patch` | No | Applies a structured patch through Tura's patch parser and workspace mutation rules. |
| `task_status` | No | Updates session task state, task type, status, or compact-context handoff. |
| `web_discover` | External package | Searches or fetches websites and media through the external command launcher. |
| `read_media` | External package | Inspects images, PDFs, audio, video, and documents for model-usable evidence. |
| `generate_media` | External package | Generates image or speech assets when the active task has that capability. |
| `planning` | Native optional command | Updates structured task planning when enabled. |

If Tura treated every operation as a raw shell line, `task_status` would become a
fake CLI, `apply_patch` would become brittle text surgery, and media commands
would need ad hoc wrappers. That is the kind of cleverness that later asks for a
maintenance budget.

## Internal commands vs external command packages

Tura commands come in two execution families.

| Family | Where it lives | How it runs | Examples |
| --- | --- | --- | --- |
| Internal command | `crates/tools/src/commands/<id>` | In-process Rust handler or command-run special case. | `shell_command`, `bash`, `zsh`, `apply_patch`, `task_status`, `planning` |
| External command package | `commands/<id>` with `command.toml` | Separate command binary launched through Tura's external JSON protocol. | `web_discover`, `read_media`, `generate_media` |

Internal commands are compiled with the tools crate and have Rust handlers behind
`CommandRouter`. They can directly implement command-specific parsing, access
rules, file-lock behavior, and output normalization. `apply_patch` is a good
example: it is not a shell wrapper; it parses patch grammar, validates workspace
paths, applies changes, and can halt later commands when a patch fails.
`task_status` is even more internal: `command_run` normalizes it directly because
it updates session state rather than spawning any process.

External command packages are registered by manifests. A package manifest says
whether the command is core or external, whether it runs `one_shot` or
`persistent`, which binary owns it, whether it supports macro command batching,
whether it is mutating, and what timeout limits apply. For example,
`commands/read_media/command.toml` registers `read_media` as a non-core
`one_shot` command with binary `tura-command-read-media`.

At runtime, external commands use the protocol in
`crates/tools/src/external/protocol.rs`:

```json
{
  "kind": "execute",
  "payload": {
    "arguments": "media/downloads --max-files 10",
    "session_dir": "C:/workspace",
    "call_id": "command_run:1:0"
  }
}
```

The external binary returns:

```json
{
  "ok": true,
  "success": true,
  "output": {},
  "stderr": "",
  "exit_code": 0
}
```

In code, that response shape is parsed as `ExternalCommandResponse` before it is
folded back into the normalized `command_run` result.

That protocol boundary is the difference between an external command package and
a CLI command. `read_media` may run as a process, but it is still a Tura command
with a manifest, protocol envelope, timeout policy, access metadata, and
normalized output. `rg --files` has none of that until it is wrapped inside the
internal `shell_command` command.

Do not confuse these with router slash/template commands from
`crates/router/src/registry/command.rs`. Those are user-facing prompt templates
loaded from directories such as `.tura/commands` or `.opencode/commands`. They
expand text for a user command. They are not `command_run` execution units unless
that expanded text later asks the model to call `command_run`.

## Commands vs tools

A tool is part of the provider-facing API. A command is part of Tura's local
runtime API.

The model does not normally receive separate provider tools named `apply_patch`,
`web_discover`, or `read_media`. Instead, runtime injects a single `command_run`
schema with an allowed `command_type` enum for the current agent, platform, and
active runtime manuals.

For example, a normal coding task may expose:

```text
apply_patch, shell_command, web_discover, task_status
```

A visual or editorial task can add:

```text
read_media, generate_media
```

The provider still sees one tool. The allowed command list changes inside that
tool.

This keeps capability management local:

- agents decide which base commands are available;
- runtime prompt manuals can add task-specific commands;
- the active OS selects the shell command surface;
- command schemas and prompts stay near their implementations;
- unsupported commands fail before they become accidental execution paths.

## How `command_run` uses commands

A `command_run` call contains a `commands` array. Each item has a
`command_type`, a `command_line`, and usually a `step`.

```json
{
  "commands": [
    {
      "command_type": "shell_command",
      "step": 1,
      "command_line": "rg --files docs/core"
    },
    {
      "command_type": "shell_command",
      "step": 1,
      "command_line": "rg -n \"task_status\" docs/core crates/runtime crates/tools"
    },
    {
      "command_type": "apply_patch",
      "step": 2,
      "command_line": "structured patch body"
    },
    {
      "command_type": "shell_command",
      "step": 3,
      "command_line": "cargo test -q -p code-tools"
    }
  ]
}
```

`step` is a dependency group. The two reads in step 1 can run together because
neither depends on the other's output. The patch waits until step 2. The test
waits until step 3, after the edit exists.

The model describes the dependency shape. The runtime handles the unglamorous
parts: grouping, locks, cancellation, output normalization, and audit records.

## Command execution lifecycle

The command path is deliberately boring and explicit:

1. The provider emits one `command_run` tool call.
2. Runtime normalizes the tool arguments and computes the allowed command set
   from the active agent plus `SessionManagement.session_capabilities` injected
   by active Runtime Prompt manuals.
3. Router-owned `CommandRunService` executes the batch so shell/tool child
   processes are owned outside the runtime worker. If a runtime worker is
   aborted, process cleanup is less likely to become confetti.
4. `command_run` parses the payload. It accepts `commands` or `steps`, recognizes
   aliases such as `command_type`, `command`, `tool_name`, and pulls payload from
   `command_line`, `input`, `args`, `payload`, or inline `arguments`.
5. Command names are canonicalized. `shell`, `bash`, `zsh`, and common misspells
   resolve to the active shell command surface; `web_search` resolves to
   `web_discover`; `view_media` resolves to `read_media`; and so on.
6. Command steps are normalized into non-decreasing dependency groups. Commands
   in the same effective step may run together only when the command handler or
   manifest says macro execution is safe.
7. `CommandRouter` resolves the command. Internal handlers win first;
   otherwise the router looks for an external manifest under the command
   registry directories.
8. Access policy is calculated. Read-only commands can share a read gate.
   Mutating commands, forced-exclusive commands, and commands with workspace
   write access take the write gate and file lock path.
9. The command is executed:
   - shell commands go through the shell executor with timeout, workdir, process
     control, stdout/stderr capture, and output shaping;
   - `apply_patch` uses the patch parser and workspace path checks;
   - `task_status` normalizes state updates directly;
   - external packages are invoked through the JSON protocol with a bounded
     timeout and parsed protocol response.
10. Each item becomes a normalized command result containing `step`,
    `command_type`, `success`, optional `output`, and optional `error`.
11. A failed `apply_patch` cancels later commands with the explicit cancel reason
    `apply_patch failed; command_run stopped before later commands`.
12. Runtime applies command side effects that belong to session state. Successful
    `task_status` results can update task group, status, task type, and compact
    context; planning output can replace active task topology.
13. Runtime publishes tool progress, records the normalized result, and stores a
    compact context view so later turns can replay useful evidence without
    replaying every raw byte.

That lifecycle is why commands are not just "tools with another name". The model
chooses a batch shape; the runtime owns authorization, scheduling, locking,
process boundaries, state updates, and evidence retention.

## Why the command mechanism is useful

The command mechanism has practical advantages over exposing many provider tools
or forcing everything through a shell.

### Smaller provider surface

The provider receives one stable tool schema for `command_run` instead of a pile
of unrelated tool schemas. That saves context and reduces schema drift across
providers. Command-specific instructions can still be injected into the
`command_run` description when the active agent or runtime manual allows them.

### Fewer model-visible turns

A single call can contain discovery, edits, validation, and task-state updates
when those actions are already known. That cuts latency and avoids the repetitive
pattern where the model reads one file, waits, reads another file, waits, then
finally edits. Thrilling only if your hobby is watching round trips age.

### Runtime-owned ordering and concurrency

Commands in the same step can run together when they are independent and safe.
Later steps wait for earlier steps. Mutating commands are serialized through the
workspace lock path. The model does not have to become a scheduler.

### Command-specific safety

Different commands need different safety rules. `apply_patch` should validate
patch structure and stop later steps if the patch fails. Shell commands need
timeouts and process cleanup. Media and web commands need external launcher
policy. `task_status` should update session state, not spawn a process.

Putting these behind command implementations keeps policy close to behavior.

### Better audit records

`command_run` returns one normalized result envelope containing each command's
type, step, success state, output, and error. Runtime can store compact command
evidence in the session log without replaying a noisy pile of unrelated tool
messages on every later turn.

When that evidence is replayed into provider context, runtime keeps the provider
tool transcript legal by replaying the `command_run` `function_call` and its
matching `function_call_output` with the same `call_id`. Orphan
`function_call_output` records are invalid. The output projection omits `step`,
`command_type`, and `command_line`; those fields live in the paired
`function_call.arguments`.

### Task-specific capabilities

Commands can be added only when the task needs them. A plain backend debugging
turn does not need image generation. A visual task may need `read_media` and
`generate_media`. Runtime prompt manuals can add those capabilities without
changing the base provider tool contract.

## Example: shell plus task state

This batch starts a documentation task and inspects relevant files in one call:

```json
{
  "commands": [
    {
      "command_type": "task_status",
      "step": 1,
      "command_line": "{\"task_group\":\"core docs\",\"task_type\":[\"editorial\"],\"status\":\"doing\"}"
    },
    {
      "command_type": "shell_command",
      "step": 1,
      "command_line": "rg --files docs/core"
    },
    {
      "command_type": "shell_command",
      "step": 1,
      "command_line": "Get-Content docs/core/command-run.md -TotalCount 160"
    }
  ]
}
```

The task-state update is not a CLI command. The file reads are CLI commands run
through `shell_command`. They share one `command_run` call because none depends
on another.

## Example: external command package

External command packages use the same command slot:

```json
{
  "commands": [
    {
      "command_type": "web_discover",
      "step": 1,
      "command_line": "website \"OpenAPI docs\" --max-results 3"
    },
    {
      "command_type": "read_media",
      "step": 2,
      "command_line": "media/downloads --max-files 10 --max-side 512"
    }
  ]
}
```

Those commands are not normal shell lines. `command_run` invokes them through the
external launcher and still returns their results in the same normalized shape.

## Implementation map

Useful source paths:

- [`crates/tools/src/commands`](../../crates/tools/src/commands) contains native
  command implementations and command metadata.
- [`crates/tools/src/command_run`](../../crates/tools/src/command_run) parses,
  schedules, executes, and normalizes `command_run` batches.
- [`crates/tools/src/external`](../../crates/tools/src/external) launches
  external command packages such as web and media commands.
- [`crates/runtime/src/manas/tool_catalog.rs`](../../crates/runtime/src/manas/tool_catalog.rs)
  injects the provider-visible `command_run` schema and restricts allowed
  commands for the active agent and task.
- [`crates/runtime/src/runtime_prompt`](../../crates/runtime/src/runtime_prompt)
  defines runtime manuals that can add command-run capabilities.

See also [Command Run](command-run.md), [Runtime Prompt](runtime-prompt.md), and
[Task Status](task-status.md).
