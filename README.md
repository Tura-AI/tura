# Tura

Tura is a terminal-native AI coding system for long-horizon engineering work: repository inspection, code changes, verification, session continuity, provider routing, and UI-assisted operation.

The short version: Tura tries to make agent work less like a chat box throwing patches at a wall and more like a local engineering runtime with state, tools, manuals, and evidence. Still glamorous, in the way a torque wrench is glamorous.

## What Tura is

Tura combines a Rust backend pipeline, a compact `command_run` tool surface, runtime prompt manuals, durable session history, provider/model routing, and CLI/TUI/GUI clients. The landing-page copy in `i18n.js` describes it as an open-source coding agent built around macro CLI execution, reasoning discipline, prompt/runtime control, and test-driven development. The repository code backs that up with real components:

- `crates/runtime` owns the agent turn loop, prompt assembly, runtime manuals, provider streaming, command callbacks, context compaction, and final response shaping.
- `crates/tools` owns `command_run`, concrete commands, shell execution, patches, task status, locks, cancellation, and tool output shaping.
- `crates/session_log` owns durable SQLite-backed session, message, task, todo, checkpoint, and workspace history.
- `crates/router` owns the per-home daemon, session_db startup/adoption, runtime worker dispatch, registry operations, and router-owned command execution.
- `crates/gateway` exposes HTTP/SSE APIs used by the TUI and GUI.
- `crates/provider` owns provider configuration, auth metadata, model routing, response extraction, streaming, logging, and usage data.
- `apps/tui` is the terminal client and CLI command layer.
- `apps/gui` and `apps/tauri` provide the graphical workspace client.

## Install and start

```powershell
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
tura exec "Inspect this workspace and summarize the risky parts"
```

```bash
./scripts/install.sh
./scripts/build-release.sh
./scripts/register-cli.sh
tura exec "Inspect this workspace and summarize the risky parts"
```

If installed from npm, the package entry is `npm/tura.mjs`; it locates the platform release binary, sets `TURA_PROJECT_ROOT`, optionally sets `TURA_PROVIDER_CONFIG`, and forwards arguments to the real binary.

## Why it is built this way

Most coding agents pay too much token overhead for scattered tools, forget long tasks, and declare success before verification. Tura's answer is structural rather than motivational posters:

| Problem | Tura mechanism | Main docs |
| --- | --- | --- |
| Repeated tool schemas and command chatter | One compact `command_run` macro surface with ordered steps | [Command Run](doc/core/command-run.md) |
| Prompt bloat from every manual every turn | Runtime prompt manuals selected by `task_type` | [Runtime Prompt](doc/core/runtime-prompt.md) |
| Long tasks losing state | `task_status`, session records, compact context handoffs | [Task Status](doc/core/task-status.md), [Context Management](doc/core/context-management.md) |
| CLI, TUI, and GUI splitting history | Shared gateway/router/session_db pipeline | [Sessions](doc/start/sessions.md), [Session DB](doc/architecture/session-db.md) |
| Provider and model routing chaos | Provider catalog, routes, auth metadata, latency policy | [Providers](doc/start/providers.md) |
| Weak verification | Business tests, OS tests, live tests, benchmarks, command evidence | [Testing](doc/development/testing.md), [Benchmark](doc/development/benchmark.md) |

## Documentation map

The organized documentation set lives in [`doc/`](doc/SUMMARY.md). Older Markdown files under `docs/` and crate-local README/ARCHITECTURE files are preserved.

### Start

- [Overview](doc/start/overview.md) - Tura is a terminal-native coding agent system built for long-horizon engineering work, verified edits, durable context, and a compact command surface.
- [Install](doc/start/install.md) - Installation prepares Rust, Node or Bun client dependencies, command packages, release binaries, and the registered CLI path.
- [How to Start](doc/start/how-to-start.md) - Start from the smallest front that fits the job: CLI for direct work, TUI for terminal conversation, GUI for workspace session management.
- [CLI Parameters](doc/start/cli-parameters.md) - CLI parameters choose workspace, model, agent, output mode, streaming behavior, command-run shell, session reuse, and low-level gateway access.
- [Settings](doc/start/settings.md) - Settings are split between provider configuration, workspace session configuration, UI preferences, and environment overrides.
- [Sessions](doc/start/sessions.md) - Sessions are durable workspace-scoped conversations with messages, task state, todos, compact handoffs, and replayable command records.
- [Providers](doc/start/providers.md) - Providers are catalog entries, auth methods, model routes, latency policies, and runtime model tiers used by the provider crate and clients.

### Core

- [Task Status](doc/core/task-status.md) - task_status is an internal state update command for doing, question, done, task_group, task_type, and compact_context; it is not a substitute for user-visible replies.
- [Context Management](doc/core/context-management.md) - Context management keeps long tasks oriented by storing session records, compacting crowded transcripts, and reinserting active runtime manuals.
- [Runtime Prompt](doc/core/runtime-prompt.md) - Runtime prompts are Tura-owned operating manuals selected by task_type; they differ from external skills because they shape discipline, tools, and completion rules.
- [Command Run](doc/core/command-run.md) - command_run is the compact macro tool surface for batching shell commands, patches, media/web commands, and task-state updates into ordered steps.
- [Commands](doc/core/commands.md) - Commands are local tool implementations exposed through command_run or router registry entries, with schemas, policies, prompts, timeouts, and output shaping.
- [Agents](doc/core/agents.md) - Agents define prompt identity, provider defaults, capabilities, reporting behavior, validation behavior, aliases, and whether operation manuals are active.
- [Personas](doc/core/personas.md) - Personas control communication style, visible identity, optional media expressions, and prompt fragments without changing the agent's engineering capabilities.

### Architecture

- [Session DB](doc/architecture/session-db.md) - Session DB is the single SQLite owner per TURA_HOME and the durable store for workspace sessions, records, task state, todos, and queued writes.
- [Gateway](doc/architecture/gateway.md) - Gateway is the HTTP/SSE front used by TUI and GUI for health, config, providers, sessions, prompt streaming, files, projects, and product routes.
- [Router](doc/architecture/router.md) - Router is the per-home daemon that owns session_db startup, runtime worker dispatch, command_run execution, registry operations, and IPC routing.
- [Runtime](doc/architecture/runtime.md) - Runtime owns the agent turn loop: session bootstrap, prompt assembly, provider streaming, tool callbacks, runtime manuals, checkpoints, and final response shaping.
- [Tool](doc/architecture/tool.md) - The tool crate defines ToolCall, ToolPayload, ToolContext, cancellation, file locks, command routing, and concrete command implementations.
- [Terminal User Interface](doc/architecture/terminal-user-interface.md) - The TypeScript TUI is a thin terminal client that talks to gateway APIs for sessions, prompts, providers, agents, personas, config, and streaming events.
- [Graphic User Interface](doc/architecture/graphic-user-interface.md) - The GUI is a Solid/Vite application hosted by Tauri or gateway static serving; it manages workspace navigation, sessions, settings, files, plans, and provider auth.

### Customization

- [Custom Providers](doc/customization/custom-providers.md) - Custom providers extend provider_config.json, auth registry behavior, model tiers, route fallback, latency policy, and client-visible catalog metadata.
- [Custom Personas](doc/customization/custom-personas.md) - Custom personas add prompt fragments, communication style, display metadata, and optional media expression manifests under personas/src.
- [Custom Agents](doc/customization/custom-agents.md) - Custom agents define model defaults, capabilities, prompts, validator behavior, aliases, operation manual policy, and reporting style.
- [Custom Runtime Prompt](doc/customization/custom-runtime-prompt.md) - Runtime prompt customization changes the task_type catalog, manual dependency graph, capability injection, and manual append rules in runtime_prompt_manual.rs.
- [Custom Commands](doc/customization/custom-commands.md) - Custom commands add new tool handlers, schemas, policies, prompts, router metadata, tests, and agent capability exposure.

### Development

- [Scripts](doc/development/scripts.md) - Scripts install dependencies, build debug/release binaries, register CLI paths, run CI, create release packages, and verify NPM platform artifacts.
- [Testing](doc/development/testing.md) - Testing is split into business, OS, live, performance, release, app, and benchmark-style checks, with business tests kept local and deterministic.
- [Environment](doc/development/environment.md) - Environment variables select home directories, release binaries, provider config, provider keys, logs, benchmark agents, and command-run shell behavior.
- [Development Architecture](doc/development/architecture.md) - Development architecture explains crate ownership, binary topology, package boundaries, process roles, file locks, and documentation ownership.
- [Benchmark](doc/development/benchmark.md) - Benchmarks are manual long-horizon comparison suites that launch real agents, collect artifacts, normalize token and command usage, and score outcomes.

## Main executable paths

| Entry | Purpose | Source owner |
| --- | --- | --- |
| `tura exec` | Rust one-shot coding prompt | `runRustCliExec` in `apps/tui/src/cli.ts`, `tura_exec::main` in `crates/gateway/src/bin/tura_exec.rs` |
| `tura` / `tura run` | Interactive or non-interactive terminal client | `runCli` in `apps/tui/src/cli.ts`, `runPrompt` in `apps/tui/src/commands/run.ts` |
| `tura_gateway` | HTTP/SSE API and static GUI serving | `build_router` and `run_server` in `crates/gateway/src/web/server.rs` |
| `tura_router` | Per-home router daemon and registry CLI | `run_router_command` in `crates/router/src/cli.rs` |
| `tura_runtime` | Per-session runtime worker | `main` in `crates/runtime/src/bin/tura_runtime.rs` |
| `tura_session_db` | SQLite session-log owner | `run_socket_service` in `crates/session_log/src/service.rs` |

## Development checks

```powershell
.\scripts\check-backend-quality.ps1
npm --prefix apps\tui test
bun run --cwd apps\gui test
```

Use focused crate checks while editing a single subsystem, then run broader checks before packaging or release work. Live provider tests are intentionally separate from deterministic business tests because bills are not a testing strategy.

## License

Tura is licensed under AGPL-3.0-or-later. See `LICENSE`.
