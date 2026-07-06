# Tura

Tura is a local AI coding system for long, tool-heavy engineering work. It is
built around a Rust runtime, a single compact `command_run` tool surface,
runtime operation manuals, durable task state, local session history, and
first-class CLI/TUI/GUI clients.

It is not trying to be another chat box that sprays code into a repo and calls
that progress. Tura is aimed at the parts of agentic coding people complain
about most: loops that forget the goal, prompt bloat, scattered tool calls,
"AI slop" patches, weak verification, and assistants that have no maintenance
sense.

Rust builds use the pinned toolchain in `rust-toolchain.toml`. The repository is
licensed under AGPL-3.0-or-later; see `LICENSE`.

## Try It

Full setup, build, launcher, CI, and release commands live in the dedicated
[install and start guide](docs/getting-started.md). The shortest local path is:

```powershell
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
tura exec "Inspect this workspace and summarize what makes it unusual"
```

```bash
./scripts/install.sh
./scripts/build-release.sh
scripts/register-cli.sh
tura exec "Inspect this workspace and summarize what makes it unusual"
```

## Why Tura Exists

Most coding agents expose too much raw machinery to the model, keep enormous
prompt surfaces alive forever, and rely on the model to remember how to be a
good engineer. Tura moves the boring but important parts into runtime structure.

The shape is:

- A compact `command_run` surface for shell, patch, media, web, and task-state
  operations, with internal scheduling for multi-step work.
- Runtime prompt manuals selected by task type, so a frontend task gets
  frontend guidance and a debug task gets debug discipline without pasting every
  manual into every prompt.
- Dynamic context and session management across CLI/TUI/GUI. CLI runs do not
  have to start from empty memory, and a desktop workspace does not become a
  graveyard of hundreds of disconnected new sessions.
- Structured task state for active work, open questions, compact handoffs, and
  completion, so long tasks can pause, compress, and resume with the useful
  context still attached.
- Repo-aware guardrails for file locks, command safety, schema normalization,
  shell process handling, media reading, web discovery, command output shaping,
  business tests, and benchmarks.

The result is a coding assistant that is designed to be cheaper in tokens,
faster in command-heavy loops, harder to derail, and more useful on work that
needs taste, verification, and maintenance judgement.

## Feature Map

| Problem people complain about | Tura answer | Details |
| --- | --- | --- |
| Tool-call spam and repeated schema overhead | `command_run` exposes one compact tool, then schedules many internal commands by step | [Command Run](docs/command-run.md) |
| Prompt bloat from every skill, every time | Runtime prompt manuals are selected by `task_type` and persisted as session records | [Runtime Prompts vs Skills](docs/runtime-prompts-vs-skills.md) |
| Long tasks that loop, forget, or declare victory early | Structured task state, explicit completion rules, retry prompts, and compact handoffs | [Long Task Loop](docs/long-task-loop.md) |
| CLI starts from nothing, desktop piles up too many fresh sessions | Dynamic context and workspace session management reuse the useful history, task state, and handoffs across fronts | [Operational overview](docs/overview.md), [Long Task Loop](docs/long-task-loop.md) |
| "AI slop" code that only satisfies the visible prompt | Repo rules, business tests, typed runners, command safety, and verification pressure | [Rules](docs/rules.md), [Tests](tests/README.md) |
| Agents that cannot inspect media, web pages, or reusable assets without bloating context | `read_media`, `web_discover`, and `generate_media` return compact artifacts, summaries, and downloaded asset folders | [Tools Crate](crates/tools/ARCHITECTURE.md) |
| Benchmarks nobody can reproduce | Benchmark harnesses collect usage, command counts, wall time, provider time, and artifacts | [Benchmarks](benchmark/README.md) |

## Command Run Is The Bet

`command_run` is the main trick. Instead of asking the provider to juggle a
large menu of tools, Tura shows one compact schema and lets the model submit a
batch:

```json
{
  "commands": [
    { "command_type": "shell_command", "command_line": "rg -n \"TODO\" crates", "step": 1 },
    { "command_type": "shell_command", "command_line": "rg --files crates/runtime/src", "step": 1 },
    { "command_type": "apply_patch", "command_line": "*** Begin Patch\n...\n*** End Patch", "step": 2 },
    { "command_type": "shell_command", "command_line": "cargo test -p runtime --lib", "step": 3 },
    { "command_type": "task_status", "command_line": "{\"status\":\"done\"}", "step": 4 }
  ]
}
```

That matters for token economics. A normal multi-tool loop pays for repeated
tool schemas, separate tool calls, repeated command narration, and callback
history. Tura keeps the provider-facing surface small, groups independent reads
in one step, streams command progress, and normalizes results before they become
future context. In command-heavy workflows this is the path to 70%+ token
reduction compared with direct multi-tool chatter; the exact number depends on
the provider, task, and benchmark mix.

Start with [docs/command-run.md](docs/command-run.md), then inspect the source:
[schema](crates/tools/src/command_run/schema.json),
[handler](crates/tools/src/command_run/handler.rs),
[tool catalog injection](crates/runtime/src/manas/tool_catalog.rs), and
[streamed command handling](crates/runtime/src/provider_flow/streamed_command_run.rs).

## Runtime Prompts Are Not Skills

Skills are external capability packs: instructions, tools, assets, or connector
knowledge that the agent may load because the environment exposes them.
Runtime prompt manuals are Tura's internal operating manuals. They are selected
by task type (`debug`, `frontend`, `visual`, `refactoring`, `new_build`, and
others), persisted into session history, and can extend the active
`command_run` command set.

This lets Tura keep the base agent small while still giving a frontend task
frontend taste, a refactor task source-port discipline, and a visual task media
tools. See [Runtime Prompts vs Skills](docs/runtime-prompts-vs-skills.md).

## Long Tasks Need State, Not Vibes

Tura treats long work as a state-machine problem:

- `task_status` records whether work is doing, blocked by a question, or done.
- Provider retries explicitly say that transient failures are not completion.
- Compact context is a structured handoff, not a fuzzy summary.
- Runtime manuals can be reinserted after compaction so the next turn keeps the
  same operating discipline.
- Final replies are separated from internal status updates, so user-visible
  communication does not disappear into tool output.

The practical goal is a two-minute-scale compression window: when context gets
crowded, the agent should spend a short, bounded turn creating a useful handoff
instead of spending the next hour slowly degrading. See
[Long Task Loop](docs/long-task-loop.md).

## Benchmarks And Tests

The repo has ordinary tests, business flows, OS/process tests, release-entry
tests, live provider checks, performance pressure tests, and benchmark harnesses.
The benchmark scripts under [benchmark](benchmark/README.md) measure
the things that matter for agent work: token usage, command executions, wall
time, provider time, artifacts, browser checks, and task score.

Useful entry points:

- [Command-run pressure test](crates/tools/tests/performance/command_run_pressure_test.rs)
- [Command-run business flow](crates/tools/tests/business/command_run_current_flow.rs)
- [Source-port benchmark harness](benchmark/refactoring/source-port-python/runner.mjs)
- [Defined-workflow source-port harness](benchmark/refactoring/source-port-python/defined-workflow.runner.mjs)
- [PDF cost comparison benchmark](benchmark/build/ogas-pdf-cost/runner.mjs)

## Project Map

- [Install and start](docs/getting-started.md)
- [Operational overview](docs/overview.md)
- [Architecture boundaries](ARCHITECTURE.md)
- [Command Run](docs/command-run.md)
- [Runtime Prompts vs Skills](docs/runtime-prompts-vs-skills.md)
- [Long Task Loop](docs/long-task-loop.md)
- [Tools crate](crates/tools/ARCHITECTURE.md)
- [Benchmark guide](benchmark/README.md)
- [Test guide](tests/README.md)
- [Business test guide](tests/business/README.md)
- [TUI guide](apps/tui/README.md)
- [GUI guide](apps/gui/README.md)
