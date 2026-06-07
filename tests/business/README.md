# Business Benchmark Tests

This directory contains long-running business benchmarks that launch real CLI
agents, prepare isolated workspaces, and score the result through harnesses.
They can consume provider quota and write large artifacts.

## Workspace And Output Layout

Business tests use the same default user workspace as GUI and TUI:

```text
~/Documents/tura workspace
```

On Windows this resolves through `USERPROFILE`; on Unix-like systems it resolves
through `HOME`. Override it with `TURA_BUSINESS_TARGET_ROOT` or
`COMMAND_RUN_BUSINESS_TARGET_ROOT` when CI needs a different artifact root.

Every entry script writes run output under:

```text
<user workspace>/target/{test_name}/{run_id}/
```

Each run writes `summary.json` at the run root. Shared path behavior lives in
`tests/business/lib/business_paths.mjs`.

## Summary Contract

Every `summary.json` is wrapped with:

```json
{
  "schema": "tura.business-test.summary.v1",
  "test_name": "frontend-playwright-lite",
  "run_id": "frontend-playwright-...",
  "user_workspace": ".../Documents/tura workspace",
  "target_root": ".../Documents/tura workspace/target",
  "run_root": ".../target/frontend-playwright-lite/...",
  "summary_path": ".../summary.json",
  "ok": true,
  "standard_metrics": {
    "duration_ms": 120000,
    "timeout_ms": 900000,
    "token_usage": {},
    "time_windows": {},
    "harness": {},
    "scores": {}
  }
}
```

Task-specific fields remain in place, including token usage, elapsed time,
event counts, harness reports, validation scores, screenshots, patches, and
comparison rows. Readers should key off the schema fields above and then inspect
`standard_metrics` for common rollups. Task-specific sections such as `results`,
`harness`, `comparison`, `aggregate_usage`, `observations.aggregate_llm`, or
`validation` preserve the full detail.

## Agent CLI Standard

Benchmarks compare agents by launching their command-line interfaces, not by
calling provider APIs directly. The categorized entry scripts support local
Tura, Codex/current Codex, Claude Code, and Pi agent ids where that agent can
reason over the task through its CLI. Environment variables select agents and
binaries:

- `COMMAND_RUN_AGENT_AGENTS`: comma-separated agent ids.
- `COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT`: local Codex checkout for current CLI.
- `COMMAND_RUN_AGENT_CODEX_MAIN_ROOT`: local codex-main checkout.
- `COMMAND_RUN_AGENT_TURA_MODEL`: Tura model id, usually `openai/...`.
- `COMMAND_RUN_AGENT_CODEX_MODEL`: Codex CLI model id.
- `COMMAND_RUN_AGENT_CLAUDE_MODEL`: Claude Code model id.
- `COMMAND_RUN_AGENT_PI_EXE`: optional Pi CLI executable path. Scripts that
  accept Pi aliases use `pi --mode json "prompt"` and collect its JSONL event
  stream.
- `COMMAND_RUN_AGENT_TIMEOUT_MS`: per-agent timeout.

New agent integrations, including PI agent variants, should follow the same
pattern: prepare an identical workspace, launch the agent through its CLI,
capture stdout/stderr/status/token usage if exposed, and score only through the
same harness used for the other agents.

Reference CLI modes:

- Claude Code supports print/headless execution with stream JSON output and
  resume flags: https://code.claude.com/docs/en/cli-usage
- Pi supports `pi --mode json "prompt"` JSONL event stream output:
  https://pi.dev/docs/latest/json

## Current Entry Scripts

### `bug-fix/`

- `retail_ops_defect_repair_agent_comparison.mjs`
  - `test_name`: `bug-fix-agent-benchmark`
  - Creates a retail operations codebase with seeded defects and verifies the
    repaired repository through generated Python and JS tests.
- `swebench_verified_issue_patch_harness.mjs`
  - `test_name`: `bug-fix-swebench`
  - Runs selected SWE-bench verified issues, collects patches, and can invoke
    the SWE-bench harness.

### `daily-ops/`

- `local_service_background_lifecycle_harness.mjs`
  - `test_name`: `daily-ops-background-services`
  - Verifies command-run background process handling by asking agents to start,
    probe, report, and clean up two local HTTP services.
- `enterprise_retail_ops_repair_expansion.mjs`
  - `test_name`: `daily-ops-enterprise-task`
  - Wraps the bug-fix benchmark with enterprise expansion and larger fixtures.
- `long_context_retail_repair_stress.mjs`
  - `test_name`: `daily-ops-context-compaction`
  - Wraps the bug-fix benchmark with a large simulated prior context window to
    stress context compaction and continuation behavior.

### `frontend-playwright/`

- `react_ops_board_programbench_rebuild_full.mjs`
  - `test_name`: `frontend-playwright-full`
  - Repairs a React operations board and includes the heavier ProgramBench-mini
    side task.
- `react_ops_board_playwright_repair_lite.mjs`
  - `test_name`: `frontend-playwright-lite`
  - Repairs the same style of React operations board with a lighter hidden
    Playwright evaluator.

### `media-internet/`

- `image_recall_two_turn_media_harness.mjs`
  - `test_name`: `media-recall`
  - Checks image inspection and second-turn visual recall. Codex uses image
    attachment, Tura uses `read_media`, and Claude/Pi are invoked through their
    CLIs with the image path and scored by the same recall checks.
- `official_media_docs_research_harness.mjs`
  - `test_name`: `media-official-research`
  - Uses web/media tools to gather official media and API docs, then verifies
    files, tool use, and token usage. External CLI agents run against the same
    workspace and are scored through the same artifact/media/doc scan.

### `project-rebuild-refactor/`

- `prompt_gallery_tanstack_frontend_rebuild.mjs`
  - `test_name`: `project-rebuild-makeup-tanstack-frontend`
  - Rebuilds `makeup.html` as a TanStack Start frontend-focused prompt gallery
    with the complete original frontend experience: brand/navigation, filters,
    search/sort, media gallery, responsive layout, interactive states, and a
    runnable Playwright browser smoke/e2e script installed by default.
- `prompt_gallery_tanstack_rebuild.mjs`
  - `test_name`: `project-rebuild-makeup-tanstack-fullstack`
  - Rebuilds `makeup.html` as a full-stack TanStack Start prompt marketplace
    while preserving the same complete frontend experience as the frontend
    version. Adds backend routes/server functions, local database seed/query
    logic, database-side business calculations, storefront/cart/analytics flows,
    and database/API/browser tests. It also requires Playwright to be installed
    by default with a runnable browser test.
- `programbench_cli_cleanroom_rebuild.mjs`
  - `test_name`: `project-rebuild-programbench`
  - Rebuilds a ProgramBench cleanroom CLI with inventory, compile, submission,
    and optional evaluation artifacts.
- `rust_cli_python_port_suite.mjs`
  - `test_name`: `project-rebuild-source-port`
  - Ports selected Rust CLI tools to Python and compares behavior against the
    official binary through an isolated harness.

`makeup.html` is a fixture, not an entry script.

### `tooling/`

- `claude_apply_patch_tool_probe.mjs`
  - `test_name`: `tooling-claude-apply-patch-probe`
  - Probes whether a Claude-backed Tura command-run flow can edit via
    `apply_patch` across two turns and records provider/tool-use evidence.

### `tui/`

- `real_gateway_tui_session_flow.mjs`
  - `test_name`: `tui-real-gateway`
  - Exercises the TUI against a real gateway, including config/session/provider
    commands, the web terminal, and an optional live provider round trip.
- `tui_web_terminal_snake_game_flow.mjs`
  - `test_name`: `tui-snake-playwright`
  - Drives TUI/web-terminal Playwright flows to build and inspect a multi-phase
    Snake game conversation in one session.
- `arcade_app_two_phase_cli_comparison.mjs`
  - `test_name`: `tui-command-run-arcade-two-step`
  - Compares multiple Tura CLI model/agent configs on a two-step arcade app
    build, or Claude/Pi external CLIs when selected, then scores the generated
    app with Playwright.

## Duplicate Policy

Entry scripts are grouped by category and are hash-unique; do not add thin
duplicate wrappers unless they set a distinct `test_name` and write to their
own run directory.
