# Tura

Tura is a CLI-driven, multi-crate Rust system for running AI coding sessions.
It combines a gateway API, a runtime/agent orchestration layer, model provider
integration, command execution tools, router-managed local processes, and
supporting application surfaces.

The current repository is intentionally organized around crate ownership rather
than long-running service directories. Runtime, gateway, provider, router,
tools, agents, and utilities are workspace members. Local build output,
provider logs, session artifacts, storage, and secrets are kept out of source
control.

## Repository Status

This repository currently contains the backend workspace and architecture
documentation for Tura. The root `ARCHITECTURE.md` describes the target design
and crate boundaries. This README expands that into an operational map for
developers: where code lives, how requests move through the system, which crate
owns each responsibility, and what should not cross module boundaries.

Tracked source is intentionally limited to code, configuration, docs, and test
drivers. The following local artifacts are ignored:

- `.env` and `.env.*`
- `target/`
- `storage/`
- `sessions/`
- provider call logs under `crates/provider/log/`
- generated command-run records under `tests/command-run-codex-two-way/records`
- local Qdrant binary `db/qdrant/qdrant.exe`

## Top-Level Layout

```text
.
  apps/
    cli/
    telegram/
    ui/
    ui-desktop/

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
    qdrant/

  scripts/
    installers/
    packages/
    persistent/

  storage/
  target/
  tests/
```

## Workspace Members

The Rust workspace is defined in the root `Cargo.toml`.

```text
crates/agents    package tura-agents       library tura_agents
crates/gateway   package gateway           binary tura
crates/provider  package tura-llm-rust     library tura_llm_rust
crates/router    package tura_router       binary tura_router
crates/runtime   package code-tools-suite  library code_tools_suite
crates/tools     package code-tools        library code_tools
crates/utils     package utils             library utils
```

Package names do not always match directory names. Prefer package names from
the local `Cargo.toml` when running `cargo check`, `cargo fmt`, or tests.

## System Flow

At a high level, a user request enters through an app or CLI, is shaped by the
gateway, executed by the runtime, resolved through an agent configuration,
answered by a model provider, and may call tools through the command layer.

```text
apps or CLI
  -> crates/gateway
  -> crates/runtime mano layer
  -> crates/agents selected agent config
  -> crates/runtime manas loop
  -> crates/provider model call
  -> crates/runtime tool-call normalization
  -> crates/tools command_run
  -> crates/router command/lifecycle resolution when needed
  -> crates/tools command handler
  -> crates/runtime compact results and final response
  -> crates/gateway replayable events and UI state
```

The product direction is CLI-driven. Ports may exist for compatibility or UI
development, but the architecture avoids treating every subsystem as a separate
network service.

## Crate Responsibilities

### `crates/gateway`

Gateway is the frontend and CLI-facing middleware. It exposes API handlers,
session state, replayable events, provider projections, file/project helpers,
PTY/process adapters, and runtime client calls.

Current important modules:

- `src/api/`: HTTP-compatible API surfaces such as session, provider, file,
  project, MCP, PTY, and miscellaneous endpoints.
- `src/session/`: session manager, store, startup cleanup, process snapshots,
  Docker snapshots, and session lifecycle helpers.
- `src/runtime.rs` and `src/simple_runtime.rs`: gateway runtime adapters.
- `src/web/`: static/web serving support.
- `src/mock/`: mock stores used by tests.
- `src/bin/tura.rs`: CLI entrypoint that runs a prompt through
  `code_tools_suite::mano`.

Gateway owns request/response shaping for apps and users. It does not own the
agent loop, prompt assembly, shell execution, provider request details, file
locks, or command registry.

Useful checks:

```powershell
cargo fmt -p gateway
cargo check -p gateway
```

### `crates/runtime`

Runtime is the renamed Mano/MANAS orchestration crate. It owns sessions,
agents, state machines, prompt/context assembly, provider turns, tool-call
normalization, compact tool results, gateway event publishing, and final
response behavior.

Current important modules:

- `src/mano/`: user/session entry layer. It creates or resumes sessions,
  activates agents, builds initial messages, and starts MANAS processing.
- `src/manas/`: agent runtime loop. It builds provider turns, filters tools,
  executes returned tool calls, injects follow-up context, and forces a final
  answer when needed.
- `src/session/`: session creation and activation helpers.
- `src/state_machine/`: session, agent, and runtime state transitions.
- `src/agent_router/`: agent loading and activation.
- `src/runtime/`: provider runtime construction, provider calls, and streaming
  receive helpers.
- `src/context/`: retained message context and workspace/runtime fragments.
- `src/prompt_style/`: fixed runtime prompt fragments as Rust modules.
- `src/tool_router/`: compatibility bridge for executing local tools.

The internal names `mano` and `manas` remain because they describe two layers:
the user/session orchestration entrypoint and the active agent execution loop.
Large behavior should live in focused helper modules rather than in `mod.rs`
files.

Useful checks:

```powershell
cargo fmt -p code-tools-suite
cargo check -p code-tools-suite
```

### `crates/agents`

Agents owns model-facing agent definitions. Runtime loads agent configuration
from this crate rather than hard-coding prompt text, provider defaults, or
capabilities in the runtime loop.

Current important files:

- `src/coding_agent.rs`
- `src/coding_agent/agent_config.json`
- `src/coding_agent/prompt.md`
- `src/coding_agent_fast/agent_config.json`
- `src/coding_agent_fast/prompt.md`

Each agent currently owns an `agent_config.json` and a `prompt.md`. The coding
agents share the same capability surface and differ by prompt behavior. The
active capability in this version is `command_run`.

### `crates/provider`

Provider owns model access, configuration, authentication, routing, response
normalization, streaming normalization, usage/cost records, and provider call
logs.

Current implementation keeps compatibility with legacy files:

- `src/tura_conf.rs`
- `src/tura_llm_conf.rs`
- `src/tura_llm.rs`
- `src/llm/_openai_provider.rs`
- `src/llm/_google_provider.rs`
- `src/llm/_bedrock_provider.rs`
- `src/llm/_llm_log.rs`
- `config/tura_llm_config.json`

Architecture docs also define the target subdomains:

- `auth/`
- `config/`
- `models/`
- `routing/`
- `providers/`
- `request/`
- `response/`
- `streaming/`
- `usage/`
- `logging/`
- `monitoring/`
- `control/`
- `state/`
- `storage/`

Runtime asks Provider for one model call and decides what to do with the
result. Provider should not execute tools, compact context, manage user
sessions, or own gateway event streaming.

Useful checks:

```powershell
cargo fmt -p tura-llm-rust
cargo check -p tura-llm-rust
```

### `crates/tools`

Tools owns the model-visible command layer, command handlers, validation,
policies, sandbox decisions, file locks, audit/output normalization, and
command execution.

Current important modules:

- `src/command_run/`: compact visible tool surface and handler entrypoint.
- `src/commands/shell_command/`: shell command execution.
- `src/commands/bash/`: Bash execution surface.
- `src/commands/apply_patch/`: patch application command.
- `src/commands/read_media/`: read-only local image/PDF/video inspection.
- `src/runtime/file_locks/`: shared and exclusive workspace locks.
- `src/modes/code/`: code-mode prompt and policy.

`command_run` accepts a list of command items. Each item has a command name,
command line or parameters, timeout, and optional step. The provider-facing
field is named `command_type` so models do not confuse the command environment
with the shell text in `command_line`. Missing steps are normalized to the
command's original order. Same-step read-only commands may run concurrently;
mutating commands acquire compatible locks.

Only a compact tool surface should be shown to the model by default. Command
schemas validate inputs, while command prompts provide concise model-facing
guidance.

Useful checks:

```powershell
cargo fmt -p code-tools
cargo check -p code-tools
```

### `crates/router`

Router owns command registration metadata, aliases, CLI forwarding, and managed
local process lifecycle. It also contains compatibility routes used by older
gateway/frontend flows.

Current important modules:

- `src/main.rs`: Axum router with health, service bootstrap, `/run_tool`,
  `/run_agent`, `/run_service`, service call, and LSP proxy endpoints.
- `src/services/`: service manager, managed process support, worker process
  support, Rust service helpers, and service models.
- `src/utils/`: CLI, port, and process helpers.

Target architecture expands this into explicit registry, lifecycle, monitor,
route, client, security, and event modules. Router should resolve command
requests and manage local processes; it should not implement command behavior,
shell execution, file locks, provider calls, memory/vector internals, or prompt
assembly.

Useful checks:

```powershell
cargo fmt -p tura_router
cargo check -p tura_router
```

### `crates/memory`

Memory is the implementation boundary for long-lived memory and recall. It is
documented as a crate-level boundary rather than a standalone service
directory.

Responsibilities include:

- long-lived memory store behavior
- vector or registry-backed recall when enabled
- memory health and persistence
- memory-specific tests and examples

Runtime and tools should call memory only through explicit clients or
memory-backed commands. Router may start or monitor a memory-backed process,
but memory behavior remains owned by this crate.

### `crates/utils`

Utils contains shared helper code for media processing, Markdown management,
and streaming text processing.

Current important modules:

- `src/media_processor.rs`
- `src/md_manager.rs`
- `src/stream_text_processor.rs`

Utilities should remain generic. Domain behavior that belongs to gateway,
runtime, provider, router, or tools should stay in the owning crate.

## Apps

The `apps/` directory is reserved for user-facing surfaces:

- `apps/cli/`
- `apps/telegram/`
- `apps/ui/`
- `apps/ui-desktop/`

In this snapshot, the pushed repository primarily contains the backend
workspace and architecture docs. UI package source may be populated separately
as the frontend surface evolves. Gateway is the integration boundary consumed
by UI and desktop apps.

## Scripts

Scripts own setup, startup, package environments, installer manifests, and
persistent reusable workflows.

Current tracked scripts:

- `scripts/start.ps1`
- `scripts/test-command-run-robustness.ps1`
- `scripts/ARCHITECTURE.md`

Target script structure:

```text
scripts/
  installers/
  packages/
  persistent/
```

Scripts must not hard-code one run's workspace paths or output locations.
Persistent scripts should read task-specific values from stdin JSON or
`TURA_COMMAND_PARAMS`.

## Configuration And Secrets

Local secrets live in `.env`, which is ignored. Do not commit API keys, OAuth
tokens, GitHub tokens, provider tokens, cloud credentials, or generated session
payloads.

Provider configuration can come from:

- `.env`
- `TURA_ENV_PATH`
- `TURALLM_CONFIG`
- `crates/provider/config/tura_llm_config.json`
- project-root-aware path configuration

Rules:

- environment variables override file config
- explicit runtime/session overrides take precedence over defaults
- missing provider config should return typed errors rather than panic
- logs and usage records must not expose raw secrets

## Prompt Ownership

Prompt text has separate owners:

- Agent prompts: `crates/agents/src/<agent>/prompt.md`
- Runtime prompt fragments: `crates/runtime/src/prompt_style/`
- Tool prompts: `crates/tools/src/command_run/prompt.md` and
  `crates/tools/src/commands/<command>/prompt.md`

Fixed runtime prompt text should be Rust constants under `prompt_style/`.
Dynamic runtime values should be injected by named builder sections. Tool
instructions belong near the command implementation. Agent prompts describe
behavior and priorities rather than duplicating every command schema detail.

## Command Execution Model

The default model-visible tool is `command_run`.

Conceptual request:

```json
{
  "step_summary": "Inspect files and run focused checks.",
  "commands": [
    {
      "command_type": "shell_command",
      "step": 1,
      "command_line": "rg \"pattern\" crates/runtime",
      "timeout_secs": 30
    }
  ]
}
```

Execution rules:

- command names are canonicalized before execution
- `command_type` is accepted as the canonical provider-facing field; legacy
  `command` payloads are normalized at the handler boundary
- missing `step` values are normalized
- same-step read-only commands may run concurrently
- later steps wait for earlier steps
- mutating commands acquire file locks
- unknown mutating shell commands acquire a workspace-wide exclusive lock
- results should be structured and display-ready where possible

Additional command-run commands in this version:

- `compact_context`: creates a concise handoff summary, clears retained tool
  call history after the current batch, reinjects the workspace snapshot and
  recent-file snapshot, and continues the same session/state machine.
- `read_media`: reads local images, PDFs, and video metadata/frames and returns
  compact model-facing descriptions without retaining raw base64 in the next
  context.
- `multiple_tasks`: optional planning-mode command, injected only when the CLI
  enables multiple-task mode. It is not part of the default coding-agent tool
  surface.

`compact_context` is intentionally architectural, not just prompt text. It is
what keeps long Tura sessions from repeatedly sending old tool-call history
after a stage is complete or the context approaches the configured limit.

## Runtime State Model

Runtime state is split into three validated state machines:

- session state in `state_machine/session_management.rs`
- agent state in `state_machine/agent_management.rs`
- runtime-call state in `state_machine/runtime_management.rs`

Use transition methods instead of assigning lifecycle states directly. Direct
state assignment should be limited to narrow initialization and test setup
paths that already follow local patterns.

## Local Development

The project is a Rust workspace. On Windows PowerShell, useful commands are:

```powershell
cargo check
cargo test
cargo fmt
```

Focused checks by touched surface:

```powershell
cargo check -p gateway
cargo check -p code-tools-suite
cargo check -p code-tools
cargo check -p tura_router
cargo check -p tura-llm-rust
```

The CLI binary is provided by the gateway crate:

```powershell
cargo run -p gateway --bin tura -- exec "Inspect the workspace"
```

The router binary is:

```powershell
cargo run -p tura_router
```

The scripts directory may provide higher-level startup commands when local UI
or router flows need coordinated startup:

```powershell
.\scripts\start.ps1
```

## Testing

Tests are split by crate and by end-to-end command-run scripts.

Crate-native tests:

```powershell
cargo test -p code-tools-suite
cargo test -p code-tools
cargo test -p tura_router
cargo test -p tura-llm-rust
```

Command-run E2E drivers:

```text
tests/command-run-codex-two-way/
  command_run_codex_two_way_e2e.mjs
  command_run_single_round_e2e.mjs
  command_run_context_compact_e2e.mjs
  command_run_long_task_e2e.mjs
  command_run_compact_context_e2e.mjs
  command_run_read_media_e2e.mjs
  command_run_media_recall_e2e.mjs
  command_run_frontend_playwright_e2e.mjs
  ROBUSTNESS_TEST_MATRIX.md
```

Generated E2E records and target outputs are ignored and should not be
committed.

## Development Rules

- Keep changes scoped to the crate that owns the behavior.
- Prefer existing local patterns over new abstractions.
- Do not add prompt text to orchestration loops when it belongs in
  `prompt_style`, agent prompts, or tool prompts.
- Do not add command registry data to `command_run`; router owns command
  metadata and aliases.
- Do not move provider authentication or routing decisions into runtime.
- Do not let gateway own agent loops, prompt assembly, or command execution.
- Use structured JSON or typed Rust structs for stable contracts.
- Keep generated logs, screenshots, target output, caches, local databases,
  `.env`, and session artifacts out of git.

## Architecture Roadmap

The current codebase already reflects the core boundaries, but several areas
are still in transition:

- Provider has compatibility files and target subdomain directories. Continue
  moving auth, routing, request, response, streaming, usage, and logging logic
  into focused modules without breaking legacy config paths.
- Router currently exposes compatibility HTTP routes and managed process
  helpers. Continue separating registry, lifecycle, monitor, security, and
  event responsibilities.
- Runtime keeps Mano/MANAS naming internally. Preserve that split while keeping
  public entrypoints thin and moving detailed behavior into owner modules.
- Tools currently exposes command-run plus shell/bash/apply_patch/read_media commands.
  It also contains mode-gated `compact_context` and `multiple_tasks` commands.
  New commands should be added under `crates/tools/src/commands/<name>` and
  registered through router metadata or the command-run capability gate.
- Apps directories are present as product surfaces. Gateway remains the stable
  boundary for UI and desktop integration.

## Related Documentation

- `ARCHITECTURE.md`: whole-project architecture and target boundaries.
- `crates/gateway/ARCHITECTURE.md`: gateway API and session boundary.
- `crates/runtime/ARCHITECTURE.md`: Mano/MANAS runtime architecture.
- `crates/agents/ARCHITECTURE.md`: agent config and prompt ownership.
- `crates/provider/ARCHITECTURE.md`: provider routing/auth/model boundary.
- `crates/tools/ARCHITECTURE.md`: command-run, policies, and file locks.
- `crates/router/ARCHITECTURE.md`: router command registry and lifecycle.
- `crates/memory/ARCHITECTURE.md`: memory boundary.
- `scripts/ARCHITECTURE.md`: startup, installer, package, and persistent
  script design.
