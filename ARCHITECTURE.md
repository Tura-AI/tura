# Tura Architecture

This is the whole-project architecture document for the current `tura`
directory. The target design is CLI-driven: runtime, gateway, provider, tools,
router, and memory behavior are implemented as crates and command modules, not
as independent long-running services.

Project root is the repository root. All paths in docs and config should be
relative to the project root.

## Repository Layout

```text
.
  apps/
    ui/
    ui-desktop/
    telegram/

  config/

  crates/
    agents/
    gateway/
    memory/
    provider/
    router/
    runtime/
    tools/
    utils/

  db/

  scripts/
    install.ps1
    install.sh
    start.ps1
    start.sh
    installers/
    packages/
    persistent/

  storage/
  tests/
```

## Crate Names And Runnable Packages

Directory names describe architecture ownership. Cargo package names should
follow the existing Tura names so build scripts, logs, and developer commands
stay compatible.

```text
crates/gateway     -> package gateway
crates/runtime     -> package code-tools-suite, library code_tools_suite
crates/agents      -> package tura-agents, library tura_agents
crates/provider    -> package tura-llm-rust, library tura_llm_rust
crates/tools       -> package code-tools
crates/router      -> package tura_router, default binary tura_router
crates/memory      -> package alaya_memory unless renamed deliberately
```

Do not derive package names from directory names. Always check the local
`Cargo.toml` package name before writing build, check, install, or start
commands.

## Architectural Boundaries

### `crates/gateway`

Gateway is the middleware between the frontend and backend crates. It provides
the UI-facing API surface, translates UI requests into runtime/router/provider
calls, persists UI-facing session data, and streams backend events back to the
frontend.

Gateway owns frontend-facing API routes, payload validation, session/thread/turn
APIs, UI persistence, event streaming, permission request forwarding, provider
config projection, process/PTY adapters, workspace config, and runtime client
calls.

Gateway does not own agent loops, provider calls, tool execution, shell
sandboxing, file locks, command registration, or CLI forwarding rules.

### `crates/runtime`

Runtime is the agent orchestration crate. It replaces the old Mano directory
while preserving the useful MANO/MANAS split as internal modules.

Runtime owns session creation/resume, agent activation, state machines, prompt
assembly, tool catalog selection, one provider turn at a time, tool-call
normalization, tool execution orchestration through `crates/tools`, gateway
event publishing, final-response forcing, and session completion.

Runtime does not own provider auth, shell execution details, file locks, router
command registration, CLI forwarding, or memory/vector internals.

### `crates/agents`

Agents are configured under `crates/agents`.

Agents own identity, default prompts, provider defaults, command selections,
planning/multiple-task defaults, and generated/static agent interfaces.
Runtime loads agent config from this crate instead of hard-coding agent
defaults.

Current agent-owned files live under:

```text
crates/agents/src/<agent_name>/
  agent_config.json
  prompt.md
```

Agent-specific prompt text must stay in `prompt.md`; runtime prompt fragments
and command prompts are injected separately by their owning crates.

### `crates/provider`

Provider owns model access and model-account control: route lookup, model
aliases, auth/token resolution, OAuth/login state, provider settings,
pause/resume controls, retry/backoff, streaming and non-streaming calls,
response normalization, tool-call normalization, token usage, cost records,
monitoring, and logs.

Runtime decides what to do with provider output. Provider only performs and
normalizes model calls.

### `crates/tools`

Tools owns the model-visible tool layer and command execution.

Tools owns:

- The compact `command_run` visible tool.
- Command handlers under `crates/tools/commands`.
- Command prompts, schemas, handlers, and policies.
- Runtime validation.
- Permission checks.
- Sandbox policy.
- File locks.
- Audit records.
- Output truncation and display-ready normalization.
- `shell_command`, `apply_patch`, `read_media`, and future commands.
- mode-gated commands such as `compact_context` and `multiple_tasks`.

`command_run` remains the compact model-visible request shape. It accepts
command items, asks router-owned metadata how to route those command names, and
then executes the selected `crates/tools/commands/<command>` handler.

### `crates/router`

Router owns CLI forwarding, command registration metadata, and managed local
service/process lifecycle. It no longer uses ports as the service boundary.

Router owns:

- Command registry.
- Command alias mapping.
- CLI forwarding rules.
- Runtime/tool command routing metadata.
- Managed service/process startup and shutdown.
- Managed service status monitoring.
- Health checks that do not depend on port allocation.
- Restart and cleanup policy for router-managed processes.
- Permission forwarding for routed command actions.

Router does not own command implementation logic, shell execution, file locks,
provider calls, or port allocation. It resolves a CLI command request to the
crate or command handler that should execute it, and it owns lifecycle for any
managed process needed to serve that request.

### `crates/memory`

Memory behavior is a crate-level implementation boundary, not an independent
service boundary.

Memory owns long-lived memory store behavior, vector or registry-backed recall
when enabled, memory health/persistence, and memory-specific tests/examples.
Runtime and tools call memory only through explicit memory-backed commands or
clients.

### `scripts`

Scripts owns setup, startup, install manifests, package environments, and
persistent reusable CLI workflows.

Scripts owns one-click install/start scripts, toolchain verification, frontend
dependency install, Rust dependency fetch/build helpers, Python package
environment manifests, shell/app/module installer manifests, persistent script
manifests, and reusable script entrypoints used by router/tools commands.

## End-To-End Flow

```text
apps/ui
  -> crates/gateway API
  -> gateway session manager
  -> gateway translates request and loads UI/session config
  -> crates/runtime starts or resumes session
  -> crates/agents supplies active agent config
  -> crates/runtime builds prompt/context/tool catalog
  -> crates/provider calls selected model
  -> crates/runtime normalizes text/tool calls
  -> crates/tools receives command_run requests
  -> crates/router resolves CLI forwarding and starts managed services when needed
  -> crates/tools/commands executes the selected command handler
  -> crates/memory handles memory-backed requests when needed
  -> crates/runtime stores compact tool results and usage
  -> crates/gateway streams events and replayable state
  -> apps/ui renders rollout, tool state, usage, and final response
```

## Prompt System

Prompt text has three owners:

- Agent prompts: `crates/agents/<agent>/prompts/`.
- Runtime prompt fragments: `crates/runtime/src/prompt_style/`.
- The `command_run` visible tool description:
  `crates/tools/src/command_run/schema.json`, augmented at runtime by
  `crates/runtime/src/manas/tool_catalog.rs`.
- Command prompts: `crates/tools/src/commands/<command>/prompt.md`.

Rules:

- Fixed runtime prompt text belongs in Rust constants under `prompt_style/`.
- Dynamic runtime values are inserted by named builder sections.
- Tool-specific instructions belong near the command.
- Agent prompts describe behavior and priorities, not every tool schema detail.
- `command_run` should remain last in the provider tool list for cache
  stability.
- Prompt-cache identity should not include dynamic command-run runtime limits.

## Agent Config

Recommended `crates/agents` layout:

```text
crates/agents/
  src/
    lib.rs
    registry.rs
    loader.rs
    interface.rs

  coding_agent/
    src/
      agent.rs
      config.rs
      prompts.rs
      tools.rs
    prompts/
      system.md
      developer.md
      task.md
    config/
      agent.toml
      provider.toml
      commands.toml
    interface/
      Icoding_agent.json
```

Agent config should define agent id/version, provider route/model defaults,
reasoning effort, service tier, tool choice, stream defaults, enabled command
ids, planning defaults, and final response policy.

Default coding-agent behavior:

- Inspect before editing.
- Prefer compact `command_run` batches for search, reads, shell, tests, and CLI
  calls.
- Put independent read-only work in the same step.
- Put dependent work, edits, and tests in later steps.
- Use `apply_patch` for source edits when available.
- Preserve user changes.
- Run focused checks.
- End with a concise final response.

## Tools And Commands

Recommended `crates/tools` layout:

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
        handler.rs
        schema.json
        prompt.md
        policy.toml
      apply_patch/
        mod.rs
        handler.rs
        schema.json
        prompt.md
        policy.toml
      bash/
        mod.rs
        schema.json
        prompt.md
        policy.toml

    modes/
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

Command files:

- `handler.rs`: argument normalization and high-level command handling.
- `schema.json`: validation and UI/handler matching.
- `prompt.md`: compact model-facing usage guidance.
- `policy.toml`: read/write/network/background/permission policy.

Schemas are for validation and handlers. Compact prompts are what should enter
model context.

## Command Run

`command_run` is the default compact visible tool. It wraps command items but no
longer owns the command registry. All registered command names and aliases live
in `crates/router`.

Provider-facing shape:

```json
{
  "step_summary": "Inspect files and run focused checks.",
  "commands": [
    {
      "command_type": "shell_command",
      "step": 1,
      "command_line": "rg \"pattern\" crates/runtime",
      "timeout_secs": 30,
      "env_keys": []
    }
  ]
}
```

Execution rules:

- `step` is optional in the provider-facing schema to match codex-current; the
  handler treats a missing step as the command's original 1-based order.
- Every command item executes with a positive integer `step` after
  normalization.
- Same-step read-only commands may run concurrently.
- Different steps run in ascending order.
- Mutating commands need compatible file locks.
- Partial results may be emitted after each step group.
- Outputs should be structured and display-ready when possible.

Built-in command families:

- `shell_command`
- `apply_patch`
- `read_media`
- `compact_context`
- `multiple_tasks`

This version exposes console shell commands (`shell_command`, `powershell:*`,
`bash:*`, `shell:*`), `apply_patch`, read-only local media inspection, and
mode-gated context/task lifecycle commands through `command_run`.

`command_type` is the canonical provider-facing command field. Legacy
`command` payloads may be accepted for compatibility at the handler boundary,
but prompt and schema text should use `command_type`.

### Compact Context

`compact_context` is a command-run command used for long coding sessions. It is
injected for coding agents and should be placed in the last step of a batch
when used. The command asks the model for a structured handoff summary covering
current progress, user requirements, relevant files/docs, completed and
remaining work, validation status, and concrete next steps.

After the command completes, runtime:

- removes prior tool-call history from retained context;
- converts the compact summary into the next user-context item;
- preserves the active session and task state machine;
- reinjects the workspace snapshot and recent-file snapshot just like a fresh
  session;
- keeps non-compact commands from the same batch and their outputs in order;
- does not retain raw compact command scaffolding as extra prompt noise.

If estimated context passes the high-water mark, runtime injects a short
continuation prompt asking the agent to compact before continuing. The prompt is
only a trigger; the token savings come from the context-management reset above.

### Media Reading

`read_media` is a read-only command for local images, PDFs, and video metadata
or sampled frames. It returns compact textual observations to the model. Binary
payloads and raw base64 are not kept in retained context; later turns recall the
media through the summarized tool output.

## File Locks

File locks are owned by `crates/tools/runtime/file_locks`.

Rules:

- Lock keys are canonical workspace-relative paths.
- Reads acquire shared locks.
- Writes acquire exclusive locks.
- `apply_patch`, `write_file`, `delete_file`, and similar commands declare
  affected paths before execution.
- Unknown mutating shell commands acquire a workspace-wide exclusive lock.
- Locks are acquired in sorted path order.
- Locks are released on success, error, timeout, and cancellation.
- Background commands hold startup locks only unless their manifest declares a
  long-lived write lease.

## Router CLI Forwarding

Recommended `crates/router` layout:

```text
crates/router/
  src/
    main.rs
    lib.rs

    registry/
      command_registry.rs
      aliases.rs
      health.rs

    lifecycle/
      manager.rs
      managed_process.rs
      cleanup.rs
      restart.rs

    monitor/
      status.rs
      health_check.rs
      heartbeat.rs

    routes/
      forward_cli.rs
      resolve_command.rs
      status.rs

    clients/
      runtime_client.rs
      tools_client.rs
      memory_client.rs

    security/
      permission_forwarder.rs
      sandbox_profile.rs
      network_policy.rs

    events/
      runtime_events.rs
      command_events.rs
```

Command registration records should include command id, aliases, owning crate
path, handler or binary target, CLI argument schema, default timeout, permission
scope, startup mode, health check, restart policy, and stdio strategy. They
should not require port allocation.

Router owns CLI forwarding metadata and lifecycle. The owning crate owns
behavior.

## Memory Crate

`crates/memory` is the memory implementation boundary.

Recommended layout:

```text
crates/memory/
  src/
    lib.rs
    memory/
    registry/
    session/
    vector_store.rs
    embedding.rs
  tests/
  examples/
```

Memory should expose stable request/response types and health checks. Runtime
and tools should call it through explicit clients or commands.

## Adding A Command

1. Create `crates/tools/commands/<name>/`.
2. Add `handler.rs`, `schema.json`, `prompt.md`, `policy.toml`, and tests.
3. Register command aliases, CLI forwarding metadata, and lifecycle metadata in
   `crates/router`.
4. Add router forwarding and lifecycle tests.
5. Enable it in the target agent config under `crates/agents`.
6. If it needs memory, add an explicit memory client/command path.
7. Run focused tools and runtime checks.

## Adding CLI Routing

1. Implement behavior in the owning crate or `crates/tools/commands`.
2. Add command and lifecycle metadata in `crates/router`.
3. Add timeout, health check, restart, and permission/sandbox policy.
4. Add agent command selection in `crates/agents`.
5. Update scripts only when startup/build/install detection changes.

## Workspace Members

Workspace members should follow the current crate layout:

```toml
members = [
  "crates/agents",
  "crates/gateway",
  "crates/memory",
  "crates/provider",
  "crates/router",
  "crates/runtime",
  "crates/tools",
  "crates/utils"
]
```

Package names for those members still follow the Tura package-name table above.

## Runnable Build And Start Paths

Install scripts should keep the Tura-style runnable path:

```text
scripts/install.ps1
scripts/install.sh
scripts/start.ps1
scripts/start.sh
```

Core Rust build targets should use package names:

```text
cargo build -p tura_router
cargo build -p gateway
cargo build -p code-tools-suite
cargo build -p code-tools
```

The installer/package manifest tree also includes Playwright support for
frontend debugging workflows:

```text
scripts/installers/playwright.toml
scripts/packages/playwright_node/manifest.toml
```

These manifests make Node Playwright and Chromium available to command-run
sessions without putting generated browser artifacts into source control.

The normal local path is CLI-driven. Router may start managed local services as
needed, but those services are not addressed through fixed ports:

```text
cargo run -p tura_router -- forward <command> [args...]
```

Direct package checks should use the same package names as the build targets.

## Focused Build Rules

- `crates/gateway/**`: `cargo fmt -p gateway`, `cargo check -p gateway`.
- `crates/runtime/**`: `cargo fmt -p code-tools-suite`,
  `cargo check -p code-tools-suite`.
- `crates/agents/**`: `cargo fmt -p tura-agents`,
  `cargo check -p tura-agents`, plus affected agent interface checks.
- `crates/provider/**`: `cargo fmt -p tura-llm-rust`,
  `cargo check -p tura-llm-rust`.
- `crates/tools/**`: `cargo fmt -p code-tools`, `cargo check -p code-tools`.
- `crates/router/**`: `cargo fmt -p tura_router`,
  `cargo check -p tura_router`.
- `crates/memory/**`: `cargo fmt -p alaya_memory`,
  `cargo check -p alaya_memory` unless the memory package has been deliberately
  renamed.
- `apps/ui/**`: UI typecheck and focused frontend tests.
- `scripts/**`: manifest validation and install dry run when possible.

## Documentation Ownership

- `ARCHITECTURE.md`: whole-project architecture and flow.
- `crates/gateway/ARCHITECTURE.md`: gateway API/session/event design.
- `crates/runtime/ARCHITECTURE.md`: runtime, state machines, prompt flow, and
  turn flow.
- `crates/agents/ARCHITECTURE.md`: agent config and prompt rules.
- `crates/provider/ARCHITECTURE.md`: provider auth, settings, routing, usage,
  and monitoring.
- `crates/tools/ARCHITECTURE.md`: command-run, commands, policies, file locks,
  and output rules.
- `crates/router/ARCHITECTURE.md`: CLI forwarding, command registration,
  lifecycle management, status monitoring, routing metadata, and permission
  forwarding.
- `crates/memory/ARCHITECTURE.md`: memory and recall behavior as a crate
  boundary.
- `scripts/ARCHITECTURE.md`: install/start/package/persistent-script rules.
