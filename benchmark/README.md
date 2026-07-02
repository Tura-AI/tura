# Benchmarks

This top-level directory contains manual benchmark, comparison, and scoring suites.
They can launch real agents, clone or rebuild external fixtures, run browser
evaluators, consume provider quota, and write large artifacts.

Benchmarks are not part of GitHub CI or default `cargo test --workspace`.
Correctness, release, live, performance, and OS tests stay under `tests/` or the
owning crate/app. Benchmarks are deliberately separate from `tests/`.

## Agent CLI Configuration

The five local benchmark agents are mapped in `config/agents.json`:

- `pi`
- `codex`
- `claudecode`
- `opencode`
- `tura`

Each profile declares aliases, the default executable name, editable argument
templates, model/reasoning environment variables, and any agent-specific env.
The resolver in `src/agents.ts` turns those profiles into the common
`AgentLaunchConfig` consumed by `src/preparer.ts`.

Executable overrides are environment based:

- `COMMAND_RUN_AGENT_PI_EXE`
- `COMMAND_RUN_AGENT_CODEX_EXE`
- `COMMAND_RUN_AGENT_CLAUDE_EXE`
- `COMMAND_RUN_AGENT_OPENCODE_EXE`
- `COMMAND_RUN_AGENT_TURA_EXE`

Model overrides follow the same pattern, for example
`COMMAND_RUN_AGENT_CODEX_MODEL` and `COMMAND_RUN_AGENT_TURA_MODEL`.

## Layout

Benchmark tasks are grouped by task type. Every task has its own directory and a
`benchmark.task.json` declaration.

```text
benchmark/build/<task>/benchmark.task.json
benchmark/debug/<task>/benchmark.task.json
benchmark/refactoring/<task>/benchmark.task.json
```

Current task groups:

- `build/`: new build and artifact-generation benchmarks.
- `debug/`: bug-fix and repair benchmarks.
- `refactoring/`: rebuild, port, and compatibility benchmarks.

Shared code lives under `benchmark/src/`; MJS compatibility helpers live under
`benchmark/lib/`.

## Task Declaration Contract

Each task declaration uses `tura.benchmark.task-declaration.v1` and declares:

- `id`, `type`, `title`, `directory`, and `summary`
- the common output contracts: CLI metadata, agent round, task report, harness report
- one or more `variants`, each pointing to a task-local runner
- `legacyScripts`, preserving the old script provenance
- `duplicatePolicy`, used when old scripts were merged into variants

Merged duplicates:

- `build/apply-patch-contract`: single-block and marker-ablation are variants of
  one apply-patch contract task.
- `refactoring/prompt-gallery-tanstack-rebuild`: the old frontend wrapper is now
  a `frontend` variant with `COMMAND_RUN_MAKEUP_TANSTACK_VERSION=frontend`.
- `refactoring/source-port-python`: default, defined-workflow, and composite are
  variants of one source-port task.

## TypeScript Abstraction Layer

`benchmark/src/` defines the common benchmark contract:

- `contracts.ts`: shared schemas and TypeScript interfaces.
- `declaration.ts`: discovery and validation for `benchmark.task.json` files.
- `parser.ts`: normalizes benchmark instructions into CLI commands and converts
  agent round callbacks into `tura.benchmark.agent-round.v1` JSON. It flattens
  Tura `command_run` batches, ordinary tools, and parallel tool calls into one
  `toolCalls[]` shape with command names and full command lines.
- `preparer.ts`: builds the task workspace, captures the initial repository
  snapshot, records CLI metadata, and creates the agent launch request.
- `monitor.ts`: records each agent round, aggregates token/provider timings,
  saves git diff, and writes `tura.benchmark.task-report.v1`.
- `harness.ts`: runs scoring harnesses and writes
  `tura.benchmark.harness-report.v1`.

Legacy `.mjs` runners continue to write their historical summary files, and
`benchmark/lib/business_paths.mjs` attaches the new contract artifacts under the
run directory's `contracts/` folder.

## Entry Points

- `benchmark/build/apply-patch-contract/`
- `benchmark/build/game-prompt-difficulty/`
- `benchmark/build/ogas-pdf-cost/`
- `benchmark/build/tui-streaming-memory/`
- `benchmark/debug/react-ops-board-playwright-repair/`
- `benchmark/debug/retail-ops-defect-repair/`
- `benchmark/debug/swebench-verified-issue-patch/`
- `benchmark/refactoring/programbench-cli-cleanroom-rebuild/`
- `benchmark/refactoring/prompt-gallery-tanstack-rebuild/`
- `benchmark/refactoring/react-ops-board-programbench-rebuild/`
- `benchmark/refactoring/source-port-python/`
