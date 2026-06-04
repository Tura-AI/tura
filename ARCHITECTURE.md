# Tura Architecture

This is the whole-project architecture document for the current `tura`
directory. The target design is CLI-driven: runtime, gateway, provider, tools,
router, and memory behavior are implemented as crates and command modules, not
as independent long-running services.

Project root is the repository root. All paths in docs and config should be
relative to the project root.

## Operational Logs

### Session Log

Durable session, task-management, message, todo, and workspace session history
is stored in `crates/session_log` backed by PostgreSQL. The default local
database directory is `db/session_log/`; embedded PostgreSQL listens on
`session_log_POSTGRES_PORT` or `55432`. `session_log_DATABASE_URL` or
`DATABASE_URL` overrides the embedded database.

Gateway and runtime must not write session state directly to
`.tura/sessions/*.json`. Gateway persists `SessionInfo`, messages, todos, and
parent links through `SessionLogClient::upsert_session`. Runtime resumes
gateway sessions through `SessionLogClient::get_session`, scoped by workspace.

Developer query commands:

```powershell
'{"command":"list_workspaces"}' | target\debug\gateway.exe session-log
'{"command":"list_sessions","workspace":"C:/repo","page":0,"page_size":50}' | target\debug\gateway.exe session-log
'{"command":"get_session","session_id":"session-id"}' | target\debug\gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\gateway.exe session-log
```

HTTP query endpoints:

```text
GET /session-log/workspaces
GET /session-log/sessions?workspace=C%3A%2Frepo&page=0&page_size=50
GET /session-log/{sessionID}/records?page=0&page_size=100
```

### Provider Call Logs

Provider call logs are written only by `crates/provider` under
`log/provider/YYYY-MM-DD/HHMMSS_mmm_<call_id>.json` by default. `LOG_PATH`
overrides the provider log root. The file payload is a JSON `llm_call` record
containing provider, model, base URL, request, normalized response, metrics,
duration, success, and error/traceback fields. Do not store provider requests
inside session-log records except as normalized runtime/session events.

## Repository Layout

```text
.
  apps/
    gui/
    tui/

  agents/

  crates/
    gateway/
    memory/        # docs-only boundary placeholder; not a Cargo member yet
    provider/
    router/
    runtime/
    session_log/
    tools/

  db/

  scripts/
    install.ps1
    install.sh
    start.ps1
    start.sh
    installers/
    packages/

  target/
  tests/
```

## Crate Names And Runnable Packages

Directory names describe architecture ownership. Cargo package names should
follow the existing Tura names so build scripts, logs, and developer commands
stay compatible.

```text
crates/gateway     -> package gateway
crates/runtime     -> package runtime, library runtime
crates/session_log -> package session_log, library session_log
agents      -> package tura-agents, library tura_agents
crates/provider    -> package tura-llm-rust, library tura_llm_rust
crates/tools       -> package code-tools
crates/router      -> package tura_router, default binary tura_router
crates/memory      -> documented boundary only; no Cargo package in this tree yet
```

Do not derive package names from directory names. Always check the local
`Cargo.toml` package name before writing build, check, install, or start
commands.

## Architectural Boundaries

### `apps/gui`

The GUI is the browser/desktop client. It talks to backend behavior only through
`apps/gui/sdk/gateway`; it must not call runtime, provider, router, tools, or
shell functionality directly. The current GUI app is organized around one app
shell, page folders, feature modules, hooks, state, and style parts:

```text
apps/gui/app/src/
  app/
  components/
  conversation/
  features/
  hooks/
  mock/
  pages/
    files/
    plan/
    settings/
  state/
  styles/
    parts/
  utils/
```

Settings are intentionally limited to appearance, providers, and models. Other
settings categories must not remain as hidden frontend pages or sidebar entries.
All settings text goes through `src/i18n.ts`. Model settings are driven by the
gateway model config API and display tier options as provider/model pairs.

Frontend refactors must keep `bun --cwd apps/gui/app typecheck` and
`bun --cwd apps/gui/app unused:check` passing before merging. New page-level
code belongs in a page folder; shared state, formatting, and gateway behavior
belong in their existing shared folders instead of being embedded into a single
large component.

### `crates/gateway`

Gateway is the middleware between the frontend and backend crates. It provides
the UI-facing API surface, forwards agent turns to the router, persists UI-facing
session data, owns the provider OAuth credential lifecycle, launches the router,
and streams backend events back to the frontend.

Gateway owns frontend-facing API routes, payload validation, session/thread/turn
APIs, UI/session persistence through `session_log`, event streaming, permission
request forwarding, provider config projection, OAuth credential lifecycle,
process/PTY adapters, workspace config, router launch, and `POST /run_agent`
forwarding to the router.

Gateway does not own agent loops, an in-process runtime, provider request
formatting, tool execution, shell sandboxing, file locks, command registration,
or CLI forwarding rules. It never runs the agent loop in-process; every agent
turn is forwarded to the router, which dispatches a runtime worker.

### `crates/runtime`

Runtime is the agent orchestration crate. It replaces the old Mano directory
while preserving the useful MANO/MANAS split as internal modules. Runtime is a
library executed inside a runtime worker — the gateway binary re-invoked with
`TURA_ROLE=runtime_worker` and dispatched by the router. It is never spawned
directly by the gateway and does not bind a fixed service port.

Runtime owns session creation/resume, agent activation, state machines, prompt
assembly, tool catalog selection, one provider turn at a time, tool-call
**consumption** (not parsing — that's provider-side), tool execution
orchestration through `crates/tools`, gateway event publishing,
final-response forcing, and session completion. Runtime emits/consumes the
canonical OpenAI Responses-API content shape only.

Runtime does not own:

- provider auth or any per-provider format branch (response parsing,
  `<thought>` stripping, prompt-cache key flag, SSE usage flag, unsupported
  content-type fallback) — these live in `crates/provider`;
- shell execution details, file locks, router command registration, CLI
  forwarding, runtime-worker dispatch, or memory/vector internals.

For multi-agent dispatch, runtime spawns child sub-sessions by invoking
`tura_router run-agent` as a subprocess (stdin/stdout JSON). It never calls
the router or gateway over HTTP/URL. See `crates/runtime/src/manas/child_dispatch.rs`.

### `agents`

Agents are configured under `agents`.

Agents own identity, default prompts, provider defaults, command selections,
planning/multiple-task defaults, and static or dynamic agent configuration.
Runtime loads agent config from this crate instead of hard-coding agent
defaults.

Current agent-owned files live under:

```text
agents/src/<agent_id>/
  agent_config.json
  prompt.md
```

Agent-specific prompt text stays in `prompt.md`. Persona text and
communication style live in `personas/src/<persona_id>/prompt` for built-ins
or `personas/<persona_id>/prompt` for dynamic personas, and agents bind them
through `agent_persona` in `agent_config.json`. Runtime prompt fragments and
command prompts are injected separately by their owning crates.

### `crates/provider`

Provider owns model access and model-account control: route lookup, model
aliases, auth/token resolution, OAuth/login state, provider settings,
pause/resume controls, retry/backoff, streaming and non-streaming calls,
response normalization, tool-call normalization, token usage, cost records,
monitoring, and logs.

Runtime decides what to do with provider output. Provider only performs and
normalizes model calls.

OpenAI OAuth discovery and refresh are Provider responsibilities. OAuth mode is
selected from `OPENAI_LOGIN=oauth`, `provider_auth.openai.login=oauth`, or local
Codex auth discovery through `CODEX_HOME/auth.json` / `~/.codex/auth.json`.
Provider must propagate access token, refresh token, and account id into the
OpenAI Codex responses call path and must surface refresh failures instead of
falling back to an empty API key.

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
- mode-gated commands such as `compact_context` and `planning`.
- `task_status` as an internal command inside `command_run`, not as a separate
  top-level model-visible tool.

`command_run` remains the compact model-visible request shape. It accepts
command items, canonicalizes the command names through
`crates/tools/src/commands` (`canonical_command`), and then executes the
selected `crates/tools/commands/<command>` handler.

`command_run` executes commands in ascending `step` order. Independent
read-only commands in the same step may run concurrently. Mutating commands,
unknown commands, and commands that touch shared workspace files act as
barriers and use the existing command queue and file-lock behavior; new
schedulers or custom lock layers should not be introduced for session/task
work.

Long-running service commands must not be blocking foreground commands. The
`shell_command` and `bash` command prompts are injected into the `command_run`
description so agents see the same service rule: keep the process handle/PID,
write stdout/stderr logs, poll readiness and process exit together, fail
immediately with exit code and log tail if the service exits before readiness,
and clean up only the started process tree on timeout.

### `crates/router`

Router owns CLI forwarding, agent registration metadata, runtime-worker
dispatch, and runtime-worker lifecycle. It no longer uses ports as the service
boundary.

Router owns:

- Agent registry (agent spec resolution).
- CLI forwarding rules (`POST /run_tool`: resolve a tool binary, forward stdio).
- Runtime-worker dispatch via `POST /run_agent`: agent resolution, worker
  environment contract assembly, and worker subprocess lifecycle.
- Runtime-worker concurrency guards (planning depth and active-worker
  limits, returning `429` on breach).
- Worker status monitoring via `/services/status`.
- Health checks that do not depend on port allocation.

Router does not own command implementation logic, command alias canonicalization
(owned by `crates/tools`), agent loops, prompt assembly, provider request
formatting, provider credentials, shell execution, file locks, or port
allocation. It resolves an agent or tool-binary request to the worker that
should execute it, and it owns lifecycle for any worker needed to serve that
request. Spawning is single-direction: gateway → router → runtime worker.

### `crates/memory`

Memory behavior is documented as a crate-level implementation boundary, not an
independent service boundary. In the current tree `crates/memory` contains only
architecture documentation and is not a Cargo workspace member yet.

When implemented, memory owns long-lived memory store behavior, vector or
registry-backed recall, memory health/persistence, and memory-specific
tests/examples. Runtime and tools should call memory only through explicit
memory-backed commands or clients.

### `scripts`

Scripts owns setup, startup, install manifests, package environments, and
persistent reusable CLI workflows.

Scripts owns one-click install/start scripts, toolchain verification, frontend
dependency install, Rust dependency fetch/build helpers, Python package
environment manifests, shell/app/module installer manifests, persistent script
manifests, and reusable script entrypoints used by router/tools commands.

## End-To-End Flow

```text
apps/gui or apps/tui
  -> crates/gateway API
  -> gateway session manager
  -> gateway translates request and loads UI/session config
  -> gateway forwards POST /run_agent to crates/router
  -> router resolves agent spec and dispatches a runtime worker
     (gateway binary re-invoked with TURA_ROLE=runtime_worker)
  -> crates/runtime (in the worker) starts or resumes session
  -> agents supplies active agent config
  -> crates/runtime builds prompt/context/tool catalog
  -> crates/provider calls selected model
  -> crates/provider normalizes text/tool calls (extract_response_text,
     extract_tool_calls, strip_thought_blocks); runtime consumes ProviderToolCall
  -> crates/runtime (optional) spawns child sub-sessions via
     `tura_router run-agent` CLI subprocess for multi-agent concurrent /
     recursive dispatch (never over HTTP/URL)
  -> crates/tools receives command_run requests
  -> crates/router resolves CLI forwarding and starts managed services when needed
  -> crates/tools/commands executes the selected command handler
  -> crates/memory handles memory-backed requests when needed
  -> crates/runtime stores compact tool results and usage
  -> crates/gateway streams events and replayable state
  -> apps/gui or apps/tui renders rollout, tool state, usage, and final response
```

## Prompt System

Prompt text has three owners:

- Agent prompts: `agents/src/<agent_name>/`, loaded by
  `crates/runtime/src/manas/agent_prompts.rs`.
- Runtime prompt fragments: `crates/runtime/src/prompt_style/`.
- The `command_run` visible tool description:
  `crates/tools/src/command_run/schema.json`, augmented at runtime by
  `crates/runtime/src/manas/tool_catalog.rs`.
- Command prompts: `crates/tools/src/commands/<command>/prompt.md`.

Rules:

- Fixed runtime prompt text belongs in Rust constants under `prompt_style/`.
- Dynamic runtime values are inserted by named builder sections.
- Tool-specific instructions belong near the command.
- Command-specific prompts that affect model behavior through `command_run`
  must be carried through `crates/runtime/src/manas/tool_catalog.rs`; do not
  read prompt files and discard them.
- Agent prompts describe behavior and priorities, not every tool schema detail.
- `command_run` should remain last in the provider tool list for cache
  stability.
- Prompt-cache identity should not include dynamic command-run runtime limits.

## Agent Config

Current `agents` layout:

```text
agents/
  Cargo.toml
  ARCHITECTURE.md
  src/
    lib.rs
    coding_agent.rs
    store.rs
    thinking-planning/
      agent_config.json
      prompt.md
    thinking/
      agent_config.json
      prompt.md
    fast/
      agent_config.json
      prompt.md
    fast-text-only/
      agent_config.json
      prompt.md
```

Agent config should define agent id, provider route defaults, stream/tool
choice defaults, enabled command ids, persona bindings, planning defaults, and
validator/final-response policy. The loader scans only `agents/src/<agent_id>`;
legacy root-level `agents/<agent_id>` directories are not read.

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

Current `crates/tools` layout:

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
      planning/
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
      task_status/
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
      code/
        mod.rs
        prompt.md
        policy.toml

  tests/
    command_run_current_flow.rs
    web_discover_live_provider_check.rs
    contracts/
      compact_context_contract.mjs
      planning_backend_contract.mjs
```

Command files:

- `mod.rs` or a focused `handler.rs`: argument normalization and high-level
  command handling. Larger commands may keep helper modules under command-local
  `src/` directories.
- `schema.json`: validation and UI/handler matching.
- `prompt.md`: compact model-facing usage guidance.
- `policy.toml`: read/write/network/background/permission policy. Commands may
  add a small `[configurable]` table for bounded non-secret defaults using
  `{ default = "...", enum = ["...", "..."] }`.

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
- Same-step read-only commands may run concurrently as a **macro_command**
  batch. The opt-in is per command handler via the unified trait method
  `supports_macro_command` (formerly `supports_parallel_tool_calls`); the
  command-run router checks it via `tool_supports_macro_command`. The OpenAI
  request field `parallel_tool_calls` is a separate provider-side concept and
  keeps its upstream name.
- Different steps run in ascending order.
- Mutating commands need compatible file locks.
- Partial results may be emitted after each step group.
- Outputs should be structured and display-ready when possible.

Built-in command families:

- `shell_command`
- `apply_patch`
- `read_media`
- `web_discover`
- `task_status`
- `compact_context`
- `planning`

This version exposes console shell commands (`shell_command`, `powershell:*`,
`bash:*`, `shell:*`), `apply_patch`, read-only local media inspection,
network-backed web/media discovery, internal task status, and mode-gated
context/task lifecycle commands through `command_run`.

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

Tools policy configurables are standardized across commands: use a single
`[configurable]` table, one inline table per setting, a `default` string, and an
`enum` string list. `read_media` uses this for media compression, PDF default
page count, directory expansion count, document attachment size, and audio
preview size; `web_discover` uses it for ordered search route fallback.

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

Current `crates/router` layout:

```text
crates/router/
  Cargo.toml
  README.md
  ARCHITECTURE.md
  src/
    main.rs
    services.rs
    services/
      managed_process.rs
      manager.rs
      models.rs
      rust_service.rs
      worker_process.rs
    utils/
      cli.rs
      port.rs
      process.rs
```

Command registration records should include command id, aliases, owning crate
path, handler or binary target, CLI argument schema, default timeout, permission
scope, startup mode, health check, restart policy, and stdio strategy. They
should not require port allocation.

Router owns CLI forwarding metadata and lifecycle. The owning crate owns
behavior.

## Memory Crate

`crates/memory` is the documented memory implementation boundary. It currently
contains only `ARCHITECTURE.md`; no Cargo package has been added yet.

Current layout:

```text
crates/memory/
  ARCHITECTURE.md
```

When implemented, memory should expose stable request/response types and health
checks. Runtime and tools should call it through explicit clients or commands.

## Adding A Command

1. Create `crates/tools/src/commands/<name>/`.
2. Add Rust handler code, `schema.json`, `prompt.md`, `policy.toml`, and tests.
3. Export the module from `crates/tools/src/commands/mod.rs`.
4. Add `ToolRouter` dispatch when the command should be callable as a direct
   routed tool.
5. Add router aliases, CLI forwarding metadata, and lifecycle metadata in
   `crates/router` only when the command needs router discovery or a managed
   process.
6. Enable it in the target `agents/src/<agent_id>/agent_config.json`.
7. If it needs memory, add an explicit memory client/command path.
8. Run focused tools and runtime checks.

## Adding CLI Routing

1. Implement behavior in the owning crate or `crates/tools/commands`.
2. Add command and lifecycle metadata in `crates/router`.
3. Add timeout, health check, restart, and permission/sandbox policy.
4. Add agent command selection in `agents`.
5. Update scripts only when startup/build/install detection changes.

## Workspace Members

Workspace members should follow the current crate layout:

```toml
members = [
  "agents",
  "crates/gateway",
  "crates/provider",
  "crates/router",
  "crates/runtime",
  "crates/tools"
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
cargo build -p runtime
cargo build -p code-tools
```

The installer/package manifest tree also includes Playwright support for
frontend debugging workflows and media support:

```text
scripts/installers/media.toml
scripts/installers/playwright.toml
scripts/packages/playwright_node/manifest.toml
scripts/packages/read_media/manifest.toml
```

These manifests make Node Playwright and Chromium available to command-run
sessions, and keep media-reading entrypoints separate from source-control
generated artifacts.

The normal local path is CLI-driven. Router may start managed local services as
needed, but those services are not addressed through fixed ports:

```text
cargo run -p tura_router -- forward <command> [args...]
```

Direct package checks should use the same package names as the build targets.

## Focused Build Rules

- `crates/gateway/**`: `cargo fmt -p gateway`, `cargo check -p gateway`.
- `crates/runtime/**`: `cargo fmt -p runtime`,
  `cargo check -p runtime`.
- `agents/**`: `cargo fmt -p tura-agents`,
  `cargo check -p tura-agents`, plus affected agent interface checks.
- `crates/provider/**`: `cargo fmt -p tura-llm-rust`,
  `cargo check -p tura-llm-rust`.
- `crates/tools/**`: `cargo fmt -p code-tools`, `cargo check -p code-tools`.
- `crates/router/**`: `cargo fmt -p tura_router`,
  `cargo check -p tura_router`.
- `crates/memory/**`: documentation review only until a Cargo package is added.
- `apps/gui/**`: GUI typecheck/build and focused frontend tests.
- `apps/tui/**`: TUI build and focused CLI/TUI tests.
- `scripts/**`: manifest validation and install dry run when possible.

## Documentation Ownership

- `ARCHITECTURE.md`: whole-project architecture and flow.
- `crates/gateway/ARCHITECTURE.md`: gateway API/session/event design.
- `crates/runtime/ARCHITECTURE.md`: runtime, state machines, prompt flow, and
  turn flow.
- `agents/ARCHITECTURE.md`: agent config and prompt rules.
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
