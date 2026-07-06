# Tura

Tura is a terminal-native developer tool for turning intent into verified code
changes with disciplined motion, audit trails, and repo-aware control.

It is built for long-horizon repository work: inspect the workspace, reason from
the desired outcome back to the next necessary step, make narrow changes, run the
checks, preserve context, and attach evidence before calling the work done.

```bash
npm install tura-ai
tura
```

```text
dev@linux:~/workspace$ tura
▏build a website for yourself
thinking
I'll inspect the workspace and assemble the smallest page first.

◇ Commands
├─ □ #1 shell_command running $ rg --files .
└─ □ #1 shell_command pending $ rg -n "TODO" .
```

## Why Tura exists

Most coding agents look strong on short prompts and then leak discipline on real
engineering work: too much tool chatter, too much irrelevant context, weak
session continuity, and a suspicious habit of declaring victory before
verification. Charming. Expensive, too.

Tura's answer is structural:

- a compact macro command surface instead of scattered tool schemas;
- runtime-selected operation manuals instead of one giant prompt blob;
- durable session records instead of chat-only memory;
- backward reasoning from outcome to root cause;
- test-driven repair flows that reproduce before patching;
- CLI, TUI, GUI, gateway, router, runtime, provider, tool, and session DB pieces
  that share one backend pipeline.

The landing copy in [`i18n.js`](i18n.js) frames the system around four operating
ideas: **Macro CLI**, **Reasoning**, **Prompt**, and **TDD**. The source tree is
organized around the same ideas, not around brochure confetti.

## Core features

| Feature | What it means |
| --- | --- |
| Macro CLI | `command_run` batches ordered reads, edits, validation, media/web commands, and task-state updates through one compact tool surface. |
| Backward reasoning | Tura works from the verified end state back to the cause, reproduction, smallest safe edit, and evidence. |
| Runtime context | Runtime prompt manuals are selected by `task_type`, persisted as session records, and reinserted after compaction. |
| Test-driven repair | Debug work starts with reproduction and ends with a check, not vibes wearing a lab coat. |
| Durable sessions | Session DB stores workspace sessions, messages, task state, todos, compact handoffs, and command evidence. |
| Provider routing | Provider configuration, model tiers, auth metadata, routes, fallback, and logs are first-class. |
| Multiple fronts | Use the direct CLI, terminal UI, local gateway, web GUI, or Tauri desktop client. |
| Customizable runtime | Add providers, personas, agents, runtime prompts, and commands from release or source layouts. |

## Benchmark direction

Tura is built from evaluation data, not just claims. The benchmark copy in
[`i18n.js`](i18n.js) focuses on long-horizon repository tasks, diversified debug
workflows, high-resolution challenge tests, real repo refactoring, token
discipline, and scored outcomes.

The benchmark system is intentionally separate from ordinary CI because it can
launch real agents, consume provider quota, clone or rebuild fixtures, collect
artifacts, normalize token and command usage, and score outcomes.

Read more in [Benchmark](docs/development/benchmark.md).

## Quick start

### NPM release

```bash
npm install tura-ai
tura
tura exec "Inspect this workspace and summarize the risky parts"
```

On Windows:

```powershell
npm install -g tura-ai
tura
tura exec "Inspect this workspace and summarize the risky parts"
```

The npm entrypoint resolves the platform release binary, sets the runtime root,
and forwards arguments to the real Tura executable.

### Source checkout

```powershell
git clone https://github.com/Tura-AI/tura.git
cd tura
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
tura exec "Inspect this workspace"
```

```bash
git clone https://github.com/Tura-AI/tura.git
cd tura
./scripts/install.sh
./scripts/build-release.sh
./scripts/register-cli.sh
tura exec "Inspect this workspace"
```

For OS-specific PATH and executor requirements, see
[How to start](docs/start/how-to-start.md).

## Ways to run Tura

| Entry | Use it for |
| --- | --- |
| `tura` | Interactive terminal UI. |
| `tura "prompt"` | Open the TUI with an initial prompt. |
| `tura exec "prompt"` | Direct Rust CLI prompt runner. |
| `tura run "prompt"` | Gateway-backed prompt with streaming/history. |
| `tura bash`, `tura zsh`, `tura shel` | Prompt with a selected command-run shell surface. |
| `tura_gateway` | Local HTTP/SSE gateway and optional web GUI serving. |
| `tura_gui` | Desktop GUI workspace client. |

See [CLI parameters](docs/start/cli-parameters.md) for the full command surface.

## What makes it different

| Problem in ordinary agent stacks | Tura mechanism | Docs |
| --- | --- | --- |
| Repeated tool schemas and noisy tool traffic | One compact `command_run` macro surface with ordered steps | [Command Run](docs/core/command-run.md) |
| Every task receives every instruction | Runtime prompt manuals selected by `task_type` | [Runtime Prompt](docs/core/runtime-prompt.md) |
| Long tasks lose state | `task_status`, session records, compact handoffs | [Task Status](docs/core/task-status.md), [Context Management](docs/core/context-management.md) |
| CLI, TUI, and GUI split history | Shared gateway/router/session DB pipeline | [Sessions](docs/start/sessions.md), [Session DB](docs/architecture/session-db.md) |
| Model/provider routing becomes ad hoc | Provider catalog, routes, auth metadata, latency policy | [Providers](docs/start/providers.md) |
| Success is asserted, not verified | Business, OS, live, release, performance, and benchmark checks | [Testing](docs/development/testing.md), [Benchmark](docs/development/benchmark.md) |

## Documentation

The organized documentation lives in [docs/SUMMARY.md](docs/SUMMARY.md).

### Start

- [Overview](docs/start/overview.md)
- [Install](docs/start/install.md)
- [How to start](docs/start/how-to-start.md)
- [CLI parameters](docs/start/cli-parameters.md)
- [Settings](docs/start/settings.md)
- [Providers](docs/start/providers.md)
- [Sessions](docs/start/sessions.md)
- [Navigation](docs/start/navigation.md)

### Core

- [Task status](docs/core/task-status.md)
- [Context management](docs/core/context-management.md)
- [Runtime prompt](docs/core/runtime-prompt.md)
- [Command run](docs/core/command-run.md)
- [Commands](docs/core/commands.md)
- [Agents](docs/core/agents.md)
- [Personas](docs/core/personas.md)
- [Rich text](docs/core/rich-text.md)
- [Dynamic prompt injection](docs/core/dynamic-prompt-injection.md)

### Architecture

- [Session DB](docs/architecture/session-db.md)
- [Gateway](docs/architecture/gateway.md)
- [Router](docs/architecture/router.md)
- [Runtime](docs/architecture/runtime.md)
- [Tool](docs/architecture/tool.md)
- [Terminal user interface](docs/architecture/terminal-user-interface.md)
- [Graphic user interface](docs/architecture/graphic-user-interface.md)

### Customization

- [Custom providers](docs/customization/custom-providers.md)
- [Custom personas](docs/customization/custom-personas.md)
- [Custom agents](docs/customization/custom-agents.md)
- [Custom runtime prompt](docs/customization/custom-runtime-prompt.md)
- [Custom commands](docs/customization/custom-commands.md)

### Development

- [Scripts](docs/development/scripts.md)
- [Testing](docs/development/testing.md)
- [Environment](docs/development/environment.md)
- [Architecture](docs/development/architecture.md)
- [Benchmark](docs/development/benchmark.md)

## Architecture at a glance

| Area | Owner |
| --- | --- |
| Runtime turn loop, prompt assembly, manuals, compaction, provider streaming | `crates/runtime` |
| Tool contracts, `command_run`, patches, shell execution, locks, output shaping | `crates/tools` |
| Durable SQLite-backed sessions, messages, task state, todos, checkpoints | `crates/session_log` |
| Per-home daemon, runtime worker dispatch, registry and IPC routing | `crates/router` |
| HTTP/SSE API for TUI and GUI clients | `crates/gateway` |
| Provider config, model routing, auth, logs, usage extraction | `crates/provider` |
| Terminal client and CLI command layer | `apps/tui` |
| GUI workspace client and Tauri shell | `apps/gui`, `apps/tauri` |

## Development checks

```powershell
.\scripts\check-backend-quality.ps1
npm --prefix apps\tui test
bun run --cwd apps\gui test
```

Use focused subsystem checks while editing, then broader checks before packaging
or release work. Live provider tests are intentionally separate from deterministic
business tests because bills are not a testing strategy.

## License

Tura is licensed under AGPL-3.0-or-later. See [LICENSE](LICENSE).
