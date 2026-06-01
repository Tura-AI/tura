# Tools Crate Architecture

`crates/tools` owns the model-visible tool layer, command handlers, validation,
policy, sandbox, file locks, and tool output normalization. It does not own the
command registry or managed process lifecycle; those live in `crates/router`.

The Cargo package name should stay compatible with Tura:

```text
package = code-tools
```

## Layout

`crates/tools` is a normal Rust crate. All runnable Rust modules, prompt
assets, schemas, and policies live under `src/`; crate-root files are limited to
Cargo metadata and architecture documentation.

```text
crates/tools/
  Cargo.toml
  ARCHITECTURE.md
  src/
    lib.rs
    command_run/
      mod.rs
      handler.rs
      schema.json
      policy.toml

    runtime/
      mod.rs
      tool.rs
      file_locks/
        mod.rs
        policy.toml

    commands/
      mod.rs
      command_safety.rs
      shell_command/
        mod.rs
        src/
          execution.rs
          process.rs
          read_batch.rs
          readonly.rs
          request.rs
          response.rs
          shell.rs
        tests/
          mod.rs
        schema.json
        prompt.md
        policy.toml
      apply_patch/
        mod.rs
        schema.json
        prompt.md
        policy.toml
      read_media/
        mod.rs
        src/
          config.rs
        schema.json
        prompt.md
        policy.toml
      compact_context/
        mod.rs
        schema.json
        prompt.md
        policy.toml
      multiple_tasks/
        mod.rs
        schema.json
        prompt.md
        policy.toml
      task_status/
        mod.rs
        schema.json
        prompt.md
        policy.toml
      web_discover/
        mod.rs
        src/
          access.rs
          args.rs
          download.rs
          files.rs
          filter.rs
          html.rs
          media.rs
          output.rs
          policy.rs
          runner.rs
          search.rs
          types.rs
          util.rs
          website.rs
        tests/
          mod.rs
        schema.json
        prompt.md
        policy.toml
      bash/
        mod.rs
        schema.json
        prompt.md
        policy.toml

    modes/
      mod.rs
      code/
        mod.rs
        prompt.md
        policy.toml

  tests/
    command_run_current_flow.rs
    command_interceptor_e2e.rs
    web_discover_live_provider_check.rs
    docker/
      Dockerfile
    contracts/
      compact_context_contract.mjs
      multiple_tasks_backend_contract.mjs
```

The target command implementation layout is
`crates/tools/src/commands/<name>/`. The default visible model surface is still
loaded from `crates/tools/src/command_run/schema.json`.

## Visible Tool Model

The default coding-agent surface should expose one compact tool:

```text
command_run
```

`command_run` contains command items. The agent chooses a command name and
arguments. Tools asks `crates/router` to resolve that command name through the
router-owned CLI registry, then executes the mapped command handler.

Direct model-visible tools are allowed only for compatibility or provider routes
that require them.

## Command Contract

Each command has:

- `mod.rs` or a focused `handler.rs`: argument normalization and high-level
  handling. Larger commands should move helpers into `src/` modules, following
  `shell_command`, `read_media`, and `web_discover`.
- `schema.json`: validation and UI/handler matching.
- `prompt.md`: compact usage guidance.
- `policy.toml`: read/write/network/background/permission policy plus bounded
  command-local configuration under `[configurable]` when needed.
- Tests.

The `command_run` visible tool is the exception: its model-facing description
comes from `command_run/schema.json` and is augmented by
`crates/runtime/src/manas/tool_catalog.rs`. Schemas are not automatically dumped
into prompt context. Runtime should inject compact prompts for active commands.

### Policy Configurables

Command policies may expose a small fixed set of non-secret knobs under a
single `[configurable]` table. Each entry must use this inline-table shape:

```toml
setting_name = { default = "balanced", enum = ["compact", "balanced", "detailed"] }
```

Rules:

- Use `enum`, not `values`, so all command policy files share one contract.
- Every configurable must have a `default` string and an `enum` string list.
- Defaults must be one of the enum values, and handlers must fall back to a
  safe built-in value if the policy is malformed.
- Keep configurable sets small, normally three to five entries per command.
- Configurables describe bounded behavior choices, not arbitrary numbers,
  paths, secrets, prompts, or user-specific state.

Current examples:

- `web_discover` configures the ordered search route fallback with
  `first_route`, `second_route`, and `third_route`.
- `read_media` configures media compression, default PDF page coverage,
  default directory file count, document attachment size, and audio preview
  byte budget. Runtime logic reads these policy defaults before applying any
  explicit command arguments.

## Registry Boundary

Router owns registered command names, aliases, CLI forwarding metadata, and
managed service/process lifecycle. Tools owns executable command handlers.

Examples:

- `powershell` -> router alias -> `shell_command`
- `bash` -> router alias -> `shell_command`
- `shell_command` -> router command id -> tools handler
- `apply_patch` -> router command id -> tools handler
- `read_media` -> router command id -> tools handler
- `web_discover` -> router command id -> tools handler
- `compact_context` -> command-run lifecycle handler
- `task_status` -> internal command-run status command
- `multiple_tasks` -> optional planning/multiple-task state handler

Only `shell_command`, `bash`, `apply_patch`, read-only `read_media`,
`web_discover`, `compact_context`, and internal `task_status` are enabled for
normal command-run coding-agent sessions in this version. `multiple_tasks` is
injected only by the explicit multiple-task runtime mode.

`command_run/` must not contain a command registry. New command registration
belongs in `crates/router`.

## Step Scheduling

`command_run` receives an array of command items.

Rules:

- `command_type` is the canonical provider-facing command selector. Legacy
  `command` input can be normalized for compatibility, but prompts and schemas
  should use `command_type`.
- `step` is optional in the provider-facing schema to match codex-current.
  The handler normalizes missing steps to the command's original 1-based
  position.
- Every command executes with a positive step after normalization.
- Same-step read-only commands may run concurrently as a single
  **macro_command batch** (see naming below).
- Later steps wait for earlier steps.
- Mutating commands acquire file locks.
- Partial results may be emitted after each step group.

### Macro-command naming (unified)

The internal name for the batched concurrent-read scheduling is
**`macro_command`**, not "parallel". The unified naming covers handler
internals, the router capability flag, and the per-command trait method:

| Old (legacy) | New (unified) | Site |
|---|---|---|
| `supports_parallel_tool_calls` | `supports_macro_command` | `ToolHandler` trait method on each command |
| `tool_supports_parallel` | `tool_supports_macro_command` | `ToolRouter` capability probe |
| `is_parallel_safe_read` | `is_macro_command_safe` | `CommandItem` method in `command_run::handler` |
| `parallel_reads` / `parallel_safe` | `macro_command_batch` / `macro_command_safe` | local state in `StreamingCommandRunExecutor` and `run_command_run_step` |
| `flush_parallel_reads` | `flush_macro_command_batch` | executor method |
| `run_parallel_items` | `run_macro_command_batch` | batch runner |

The OpenAI provider-side wire field `parallel_tool_calls` is **unrelated** to
this rename — that field is an OpenAI request parameter owned by the provider
crate and keeps its upstream name.

## Context Compaction Command

`compact_context` is a lifecycle command inside `command_run`. It should always
be scheduled as the last step in a batch. The command output is a single
handoff summary, capped by prompt guidance to stay compact enough for the next
agent turn.

Runtime handles the command specially after execution:

- Retained tool-call history is cleared.
- The compact summary becomes the next user-context item.
- The session and task state machine continue; compaction does not reset work.
- Workspace snapshot and recent-file snapshot are regenerated and injected.
- Other commands in the same batch remain ordered and are not repeated.

This is the main long-context optimization path. Prompt wording only tells the
model when to call the command; the token reduction comes from runtime context
replacement.

## Media Reading Command

`read_media` is read-only and safe to run with other read-only work. It inspects
local images, PDFs, and videos, returning concise textual observations and
selected metadata. Raw binary payloads and base64 are not retained in context.
Multi-turn recall should rely on the summarized media observations, not on
re-sending the original file bytes.

The command's default text/visual budget, PDF page count, directory expansion
count, document attachment byte cap, and audio preview byte cap all come from
`read_media/policy.toml` `[configurable]` entries. Explicit CLI or JSON command
arguments can narrow or expand within handler clamps, but processing logic
should not bypass policy resolution with separate hard-coded media defaults.

## Web Discover Command

`web_discover` is the network-capable discovery and download command. It can
fetch direct pages/media, search web or image routes, and write downloaded
artifacts into declared workspace paths. Its fallback route order is controlled
by `web_discover/policy.toml` `[configurable]` entries using the same
`default` + `enum` policy shape as `read_media`.

## Command Interceptor

`commands/command_safety.rs` is a single self-contained command interceptor,
mirroring the "dangerous command detection" layer of Codex / claude-code. It is
detection-only: it blocks destructive commands before they spawn, and does not
implement sandboxing, approval UI, or a read-only allow list.

Entry point:

```text
pub fn is_dangerous_command(command: &str) -> Option<String>
```

`Some(reason)` blocks; `None` allows. `shell_command` calls it on both the sync
and async execution paths before dispatch. A blocked command returns a
model-visible failure (`success = false`, `exit_code = 126`, output
`Blocked by command interceptor: {reason}\nCommand was not executed: {command}`)
instead of executing. The env var `TURA_COMMAND_INTERCEPTOR_DISABLED=1` turns the
guardrail off entirely for trusted automation.

Detection covers POSIX/bash, PowerShell, and CMD command shapes:

- Unix: `rm -r/-f` or `rm`/`rmdir` against a system path; `shutdown`/`reboot`/
  `halt`/`poweroff`; `init`/`telinit 0|6`; `dd of=/dev/…`; `mkfs`/`wipefs`/
  `fdisk`/`parted`/`sgdisk`/`shred` on `/dev/`; recursive `chmod`/`chown`/`chgrp`
  on a system path; redirect overwriting a block device; fork bombs; download
  cradles (`curl … | sh`).
- PowerShell: `Remove-Item`/`del`/`rd` with `-Force`/`-Recurse`;
  `Invoke-Expression` of a remote download; `Format-Volume`/`Clear-Disk`/etc.
- CMD: `del /f`, `rd /s /q`, `format <drive:>`.

Anti-bypass: connector splitting (`;` `&&` `||` `|` `\n`), command substitution
(`$(…)` / backticks), wrapper stripping (`sudo`/`timeout`/`env`/`nice`/`xargs`/
…), path normalization (`/bin/rm`, `rm.exe`), and recursion through
`bash -c`/`eval`. A one-level library-exec layer also extracts command-line
strings smuggled through interpreter calls (`os.system(`, `subprocess.run(`,
`child_process.exec(`, `shell_exec(`, …) and re-scans them against the same
blacklist; whitespace-stripped marker matching defeats `os . system(` spacing,
and concat-joined literal recovery defeats `'rm'+' -rf'` splitting. Blocking only
fires when a recursed inner command hits the blacklist, so benign calls such as
`subprocess.run(['ls'])` are never false-killed.

Tests: unit tests live in `command_safety.rs`; `tests/command_interceptor_e2e.rs`
(Unix-gated) drives the real `command_run` entry point and *actually executes*
commands inside the Linux Docker harness under `tests/docker/`, asserting that
dangerous commands' destructive side effects never happen while safe commands
still run. Run with `scripts/run_interceptor_e2e_docker.{sh,ps1}`.

## File Locks

Commands report expected access before execution:

```text
read_paths = []
write_paths = []
workspace_write = false
```

Rules:

- Read paths use shared locks.
- Write paths use exclusive locks.
- Unknown mutating shell commands use a workspace-wide exclusive lock.
- Lock keys are canonical workspace-relative paths.
- Locks are acquired in sorted order.
- Locks are released on success, error, timeout, or cancellation.

## Command Add Flow

1. Add `crates/tools/src/commands/<name>/`.
2. Add handler, schema, prompt, policy, and tests.
3. Register command aliases, CLI forwarding metadata, and lifecycle metadata in
   `crates/router`.
4. Enable the command in `agents`.
5. Add focused tests.

## Router Integration

If a command needs CLI routing or a managed local service/process, it asks
`crates/router` to resolve the command name, forwarding target, and lifecycle
state. Router owns startup, shutdown, status monitoring, and restart policy, but
it does not own the command implementation.

Memory-backed behavior crosses into `crates/memory` through explicit clients or
commands.

## Checks

Use:

```text
cargo fmt -p code-tools
cargo check -p code-tools
```
