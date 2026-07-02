# Benchmarks

This directory contains manual benchmark, comparison, and scoring suites. These
scripts can launch real agents, clone or rebuild external fixtures, run browser
evaluators, consume provider quota, and write large artifacts.

Benchmarks are not part of GitHub CI or default `cargo test --workspace`.
Crate-owned correctness tests belong under the owning crate, and release-entry
validation belongs under `tests/release/` or the app-local `e2e/business/`
directories. Rust business, OS, and performance tests belong under
`crates/*/tests/business/`, `crates/*/tests/os_testing/`, and
`crates/*/tests/performance/`.

Workspace benchmark scripts keep their historical second-level categories:

```text
tests/benchmark/bug-fix/
tests/benchmark/frontend-playwright/
tests/benchmark/lib/
tests/benchmark/media-presentation/
tests/benchmark/project-rebuild-refactor/
tests/benchmark/tui/
```

Shared benchmark helper re-exports live under `tests/benchmark/lib/`.

## TypeScript Abstraction Layer

`tests/benchmark/src/` defines a common benchmark contract for new suites and
for adapters around the historical `.mjs` harnesses:

- `parser.ts` normalizes benchmark instructions into CLI commands and converts
  agent round callbacks into `tura.benchmark.agent-round.v1` JSON files. It
  flattens Tura `command_run` batches, ordinary tools, and parallel tool calls
  into one `toolCalls[]` shape with command names and full command lines.
- `preparer.ts` builds the task workspace, captures the initial repository
  snapshot, records CLI metadata, and creates the agent launch request.
- `monitor.ts` records each agent round, aggregates token/provider timings, saves
  git diff, and writes `tura.benchmark.task-report.v1`.
- `harness.ts` runs scoring harnesses and writes `tura.benchmark.harness-report.v1`
  so every model, agent, and task type emits the same contract files.

## What They Measure

The benchmark harnesses are built to make agent claims falsifiable. Depending on
the suite, they record:

- provider input, cached-input, output, reasoning, and total tokens
- wall-clock duration and provider-call duration
- command execution counts and command success rate
- generated artifacts, source files, screenshots, PDFs, or reports
- behavior scores from local evaluators or browser checks
- whether task-state features such as `task_status` and command execution were
  actually used

This is the place to validate claims such as command-heavy `command_run` flows
using dramatically fewer tokens than direct multi-tool chatter. The number is
task and provider dependent; benchmark summaries are the source of truth for any
specific run.

## Useful Entry Points

- `commands/apply_patch_single_block_contract_harness.mjs`: command-shape and
  patch-contract benchmark.
- `media-presentation/ogas_pdf_cost_comparison.mjs`: cost and artifact
  comparison for media-heavy PDF work.
- `frontend-playwright/game_prompt_difficulty_comparison.mjs`: cost and
  artifact comparison for playable browser-game prompts across four English
  difficulty levels.
- `project-rebuild-refactor/rust_cli_python_port_suite.mjs`: source-port
  benchmark with usage and command statistics.
- `project-rebuild-refactor/rust_cli_python_port_suite_defined_workflow.mjs`:
  source-port benchmark with a stricter defined workflow.
- `frontend-playwright/`: browser-scored frontend rebuild and repair suites.
