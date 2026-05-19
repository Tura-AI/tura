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
      prompt.md
      policy.toml

    runtime/
      mod.rs
      validator/
      permission/
      sandbox/
      audit/
      context/
      error/
      file_locks/

    commands/
      mod.rs
      shell_command/
        mod.rs
        schema.json
        prompt.md
        policy.toml
        handler.rs  # optional when split out of command_run orchestration
      apply_patch/
        mod.rs
        schema.json
        prompt.md
        policy.toml
        handler.rs  # optional when split out of command_run orchestration
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

    utils/
      path.rs
      diff.rs
      json.rs
      output.rs
      redaction.rs
      process.rs
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

- `handler.rs`: argument normalization and high-level handling.
- `schema.json`: validation and UI/handler matching.
- `prompt.md`: compact usage guidance.
- `policy.toml`: read/write/network/background/permission policy.
- Tests.

Schemas are not automatically dumped into prompt context. Runtime should inject
compact prompts for active commands.

## Registry Boundary

Router owns registered command names, aliases, CLI forwarding metadata, and
managed service/process lifecycle. Tools owns executable command handlers.

Examples:

- `powershell` -> router alias -> `shell_command`
- `bash` -> router alias -> `shell_command`
- `shell_command` -> router command id -> tools handler
- `apply_patch` -> router command id -> tools handler
Only `shell_command`, `bash`, and `apply_patch` are enabled for command-run
coding-agent sessions in this version.

`command_run/` must not contain a command registry. New command registration
belongs in `crates/router`.

## Step Scheduling

`command_run` receives an array of command items.

Rules:

- `step` is optional in the provider-facing schema to match codex-current.
  The handler normalizes missing steps to the command's original 1-based
  position.
- Every command executes with a positive step after normalization.
- Same-step read-only commands may run concurrently.
- Later steps wait for earlier steps.
- Mutating commands acquire file locks.
- Partial results may be emitted after each step group.

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
4. Enable the command in `crates/agents`.
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
