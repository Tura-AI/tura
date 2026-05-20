# Command Run Codex Two Way E2E

This folder is a standalone copy of the command-run benchmark focused on the two Codex implementations only.

## Contents

- `command_run_codex_two_way_e2e.mjs` is the shared configurable E2E runner.
- `command_run_original_e2e.mjs` runs the original benchmark shape from the GitHub baseline: no fixed context block, no enterprise expansion, and fixture scale `1`.
- `command_run_context_compact_e2e.mjs` runs the compact-context benchmark with a fixed long starting context and no enterprise expansion.
- `command_run_long_task_e2e.mjs` runs the expanded long-task benchmark with enterprise backend/frontend acceptance tests, hard enterprise scenario-matrix tests, mega control-plane/control-tower generated tests, active generated support/integration/policy/view/shared code, and no fixed starting context block.
- `command_run_compact_stress_e2e.mjs` is a compatibility wrapper for the compact-context benchmark.
- `command_run_compact_context_e2e.mjs` directly probes the Tura `compact_context` command and verifies that long contexts can be replaced by a compact handoff without resetting the session state machine.
- `command_run_read_media_e2e.mjs` verifies `read_media` command execution and context size behavior for images, PDFs, and videos.
- `command_run_media_recall_e2e.mjs` verifies that media observations remain recallable across turns without retaining raw base64 in context.
- `command_run_frontend_playwright_e2e.mjs` compares current, codex-main, and Tura on a Playwright-heavy frontend repair task with live per-agent logs and hidden validation.
- `records/summaries/` contains the existing `command-run-agent-three-way-*.json` summary reports.
- `records/runs/` contains the existing per-run `codex-current/` and `codex-main/` execution logs, including stdout JSONL, stderr, and last-message files when present.

## Run

From this folder:

```powershell
$env:COMMAND_RUN_AGENT_CODEX_MODEL='gpt-5.5'
$env:COMMAND_RUN_AGENT_REASONING_EFFORT='low'
$env:COMMAND_RUN_AGENT_CODEX_SERVICE_TIER='priority'
$env:COMMAND_RUN_AGENT_TIMEOUT_MS='300000'
node .\command_run_codex_two_way_e2e.mjs
```

Useful optional overrides:

```powershell
$env:COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT='C:\Users\liuliu\Documents\Codex'
$env:COMMAND_RUN_AGENT_CODEX_MAIN_ROOT='C:\Users\liuliu\Documents\codex-main'
$env:COMMAND_RUN_AGENT_CODEX_MAIN_FALLBACK_ROOT='C:\Users\liuliu\codex-main'
```

Long-context and frontend-specific runners also accept runner-local flags such
as `--context-full`, `--timeout-ms`, `--run-id`, `--smoke-only`, and agent
selection values documented in the script headers. New runs should keep
per-agent stdout/stderr/status files under the generated target run directory
so unfinished or timed-out sessions can still be analyzed.

New runs write to:

```text
target/command-run-codex-two-way/<run_id>/
target/codex-logs/command-run-codex-two-way-<run_id>.json
```
