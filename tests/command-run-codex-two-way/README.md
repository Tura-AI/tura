# Command Run Codex Two Way E2E

This folder is a standalone copy of the command-run benchmark focused on the two Codex implementations only.

## Contents

- `command_run_codex_two_way_e2e.mjs` runs the E2E against local `tura` and `codex-current` by default.
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

New runs write to:

```text
target/command-run-codex-two-way/<run_id>/
target/codex-logs/command-run-codex-two-way-<run_id>.json
```
