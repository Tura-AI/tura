# Command Run Agent Business Benchmarks

This folder contains long-running command-run business benchmark drivers. These
scripts spawn real agents, compare Tura/Codex variants, and may call live
providers. Focused command-run CLI E2E scripts live under
`crates/gateway/tests/e2e/command-run/`; command contract aggregators live under
`crates/tools/tests/contracts/`.

## Contents

- `command_run_agent_benchmark.mjs` is the shared configurable business benchmark runner.
- `command_run_context_compaction_business_test.mjs` runs the compact-context benchmark with a fixed long starting context and no enterprise expansion.
- `command_run_enterprise_task_business_test.mjs` runs the expanded enterprise task benchmark with backend/frontend acceptance tests, hard scenario-matrix tests, generated support code, and no fixed starting context block.
- `command_run_media_recall_business_test.mjs` verifies that media observations remain recallable across turns without retaining raw base64 in context.
- `command_run_official_media_research_business_test.mjs` runs a live web/media research task with `tura-fast-shll` by default and reports operations, session logs, token usage, and the required `SAME_STYLE` verdict.
- `command_run_frontend_playwright_business_test.mjs` compares current, codex-main, and Tura on a Playwright-heavy frontend repair task with live per-agent logs and hidden validation.
- `command_run_frontend_playwright_lite_business_test.mjs` runs a smaller Playwright-heavy frontend benchmark.
- `command_run_background_services_business_test.mjs` checks that Tura agents start multiple persistent local services in the background, probe readiness, and clean them up across `shell_command` and `bash` command surfaces.
- `command_run_tui_snake_playwright_business_test.mjs` runs the TUI snake Playwright benchmark.
- `../tui_real_gateway_business_test.mjs` runs the TUI functional business flow against the real gateway binary rather than a mock gateway.
- `agent_swebench_business_test.mjs` compares Tura and Codex agents on explicit SWE-bench Verified issue ids using issue statements only, with priority `gpt-5.5` low-reasoning defaults and per-agent patch/log capture.
- `agent_programbench_business_test.mjs` compares Tura and Codex agents on one ProgramBench cleanroom reconstruction task. It pulls the `task_cleanroom` Docker image, copies `/workspace`, tells the agent not to use the internet, requires a full source reconstruction with matching behavior, packages `submission.tar.gz`, and can optionally run `programbench eval`.
- Historical generated records from the previous layout now live under
  `target/command-run-codex-two-way-records/`.

## Session And Provider Logs

Business benchmark scripts must not read `.tura/sessions/*.json`. Tura session
history is stored in `session_log` and can be queried through the gateway
bridge:

```powershell
'{"command":"list_sessions","workspace":"C:/repo","page":0,"page_size":50}' | target\debug\gateway.exe session-log
'{"command":"get_session","session_id":"session-id"}' | target\debug\gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\gateway.exe session-log
```

Provider call logs are model-call diagnostics only. They live under
`log/provider/YYYY-MM-DD/*.json` unless `LOG_PATH` is set. Use them for raw
provider request/response, usage, latency, and error inspection; use
`session_log` for per-session task/message/turn history.

## Run

From the repository root:

```powershell
$env:COMMAND_RUN_AGENT_CODEX_MODEL='gpt-5.5'
$env:COMMAND_RUN_AGENT_REASONING_EFFORT='low'
$env:COMMAND_RUN_AGENT_CODEX_SERVICE_TIER='priority'
$env:COMMAND_RUN_AGENT_TIMEOUT_MS='300000'
node .\tests\business\command-run-agent-benchmarks\command_run_agent_benchmark.mjs
```

Run the background-service command-run probe:

```powershell
$env:COMMAND_RUN_BACKGROUND_SERVICES_AGENTS='tura-fast-shll,tura-shll,tura-fast-bash,tura-bash'
$env:COMMAND_RUN_BACKGROUND_SERVICES_TIMEOUT_MS='240000'
node .\tests\business\command-run-agent-benchmarks\command_run_background_services_business_test.mjs
```

This probe creates a tiny Node HTTP service fixture per agent. Each agent must
start two services in the background, wait for both `/ready` endpoints, run
probes, stop both services, and write `service-results.json`. The external
runner also checks that no fixture service is left running.

Useful optional overrides:

```powershell
$env:COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT='C:\Users\liuliu\Documents\Codex'
$env:COMMAND_RUN_AGENT_CODEX_MAIN_ROOT='C:\Users\liuliu\Documents\codex-main'
$env:COMMAND_RUN_AGENT_CODEX_MAIN_FALLBACK_ROOT='C:\Users\liuliu\codex-main'
```

Run the xarray SWE-bench comparison:

```powershell
$env:COMMAND_RUN_AGENT_CODEX_MODEL='gpt-5.5'
$env:COMMAND_RUN_AGENT_REASONING_EFFORT='low'
$env:COMMAND_RUN_AGENT_SERVICE_TIER='priority'
$env:COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS='pydata__xarray-4075'
node .\tests\business\command-run-agent-benchmarks\agent_swebench_business_test.mjs
```

Run a ProgramBench Tura planning-agent solve attempt:

```powershell
$env:COMMAND_RUN_AGENT_PROGRAMBENCH_ROOT='C:\Users\liuliu\Documents\programbench'
$env:COMMAND_RUN_AGENT_PROGRAMBENCH_INSTANCE_ID='agourlay__zip-password-finder.704700d'
$env:COMMAND_RUN_AGENT_AGENTS='tura-planning-shll'
$env:COMMAND_RUN_AGENT_CODEX_MODEL='gpt-5.5'
$env:COMMAND_RUN_AGENT_TURA_MODEL='openai/gpt-5.5'
$env:COMMAND_RUN_AGENT_REASONING_EFFORT='low'
$env:COMMAND_RUN_AGENT_SERVICE_TIER='priority'
$env:COMMAND_RUN_AGENT_TIMEOUT_MS='240000'
node .\tests\business\command-run-agent-benchmarks\agent_programbench_business_test.mjs
```

To explicitly override the `planning` command surface for Tura, use the
single `COMMAND_RUN_AGENT_TURA_PLANNING` setting with `auto`, `on`, or
`off`. `auto` is the default and follows the selected agent config. The summary
records both the configured agent capabilities and the effective planning
mode, but the agent prompt still requires solving the cleanroom reconstruction
task rather than stopping after dispatch/callback plumbing:

```powershell
$env:COMMAND_RUN_AGENT_AGENTS='tura-planning-shll'
$env:COMMAND_RUN_AGENT_TURA_PLANNING='on'
node .\tests\business\command-run-agent-benchmarks\agent_programbench_business_test.mjs
```

For a 10-minute Tura-vs-Codex comparison:

```powershell
$env:COMMAND_RUN_AGENT_AGENTS='tura-planning-shll,codex-main'
$env:COMMAND_RUN_AGENT_TIMEOUT_MS='600000'
$env:COMMAND_RUN_AGENT_CODEX_MAIN_ROOT='C:\Users\liuliu\Documents\codex-main'
node .\tests\business\command-run-agent-benchmarks\agent_programbench_business_test.mjs
```

ProgramBench requires Docker for real cleanroom inference. If Docker is not
available and you only need to test runner plumbing, set
`COMMAND_RUN_AGENT_PROGRAMBENCH_ALLOW_LOCAL_FIXTURE=1`; summaries will record
that this is not a valid ProgramBench cleanroom run.

`COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS` is required. Pass a comma-separated
list or JSON array of issue ids. The runner infers the repo list from those ids,
checks out each issue's `base_commit`, gives the agent only that issue
statement, and writes one prediction per issue. If
`COMMAND_RUN_AGENT_SWEBENCH_REPOS` is also set, issue ids outside those repos
are discarded and recorded in `dropped_instance_ids`.

For isolation, each selected issue and agent run gets its own cloned workspace
under `target/agent-swebench-test/<run_id>/<issue>/<agent_run>/workspace`.
Duplicate agent names are allowed; later duplicates get a numeric suffix such as
`tura-fast-shll-2`. Workspace preparation is intentionally throttled before the
agents start, because Windows/git can fail under parallel clone/init/add load.
Override this with `COMMAND_RUN_AGENT_WORKSPACE_PREP_CONCURRENCY`; the default is
`1` on Windows and up to `4` elsewhere. After preparation, agents for the same
issue still run concurrently. Tura is built once before the concurrent agent
phase instead of once per Tura agent.

To run multiple issues, pass multiple ids:

```powershell
$env:COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS='pydata__xarray-4075,pallets__flask-5014'
node .\tests\business\command-run-agent-benchmarks\agent_swebench_business_test.mjs
```

Single-issue prompt shape:

```text
Fix the bug or issue described below. Treat the report, docs, stack traces, and suggested causes as clues, not proof of the root cause or correct fix. First identify the underlying contract and the smallest stable boundary where the behavior should be guaranteed; be especially suspicious of lazy evaluation, deferred execution, caches, cloning, shared mutable state, partial or repeated execution, and compile/render/serialization steps that may trigger failures later than the reported call site. After any transformation that changes the externally visible shape or meaning of data, aggressively revalidate dependent references, aliases, indexes, caches, and invariants against the final exposed shape instead of reusing assumptions from an earlier internal shape. Work backward from the failure to the earliest invariant boundary, and make regression tests exercise derived/transformed paths before and after evaluation so stale references, cached state, and shape mismatches cannot hide. Make the minimal necessary production change, avoid unrelated refactors or new abstractions, and do not mask the failure at the call site when the invariant belongs deeper in the system. Do not search the internet.

<problem_statement>
```

Harness evaluation is optional. To run agents and then immediately evaluate
their prediction bundles for the same issue ids:

```powershell
$env:COMMAND_RUN_AGENT_RUN_HARNESS='1'
$env:COMMAND_RUN_AGENT_HARNESS_MAX_WORKERS='8'
node .\tests\business\command-run-agent-benchmarks\agent_swebench_business_test.mjs
```

To evaluate an existing run without rerunning agents, reuse its run id and set
`COMMAND_RUN_AGENT_HARNESS_ONLY=1`. This mode reads the existing `summary.json`
and `predictions\<agent>\all_preds.jsonl` files:

```powershell
$env:COMMAND_RUN_AGENT_RUN_ID='agent-swebench-test-issues-4agents'
$env:COMMAND_RUN_AGENT_HARNESS_ONLY='1'
$env:COMMAND_RUN_AGENT_HARNESS_MAX_WORKERS='8'
node .\tests\business\command-run-agent-benchmarks\agent_swebench_business_test.mjs
```

`COMMAND_RUN_AGENT_HARNESS_MAX_WORKERS` controls SWE-bench `--max_workers`; it
defaults to the local CPU count capped at 8.
`COMMAND_RUN_AGENT_HARNESS_CACHE_LEVEL` controls SWE-bench `--cache_level` and
defaults to `instance`, so repeated agent comparisons for the same issue reuse
the pulled/built instance image. The upstream harness default is `env`, which
removes instance images after each run and can make four-agent comparisons pull
the same issue image four times.
`COMMAND_RUN_AGENT_HARNESS_CLEAN` controls SWE-bench `--clean` and defaults to
`false`, so cached instance images are retained across future runs. Docker will
keep those images until they are explicitly removed with Docker cleanup commands
or Docker Desktop storage pruning.
On Windows the harness defaults to `COMMAND_RUN_AGENT_HARNESS_BACKEND=docker-linux`:
the runner builds or reuses `tura-swebench-harness:latest`, mounts the local
SWE-bench checkout and run directory into that Linux container, and executes the
official SWE-bench harness there. The effective harness worker count is capped
at the number of unique selected issue ids, so all predictions for the same
issue reuse the same cached SWE-bench instance image instead of starting
parallel workers for that issue.

To query/validate selected issue ids without running agents, use prep-only:

```powershell
$env:COMMAND_RUN_AGENT_PREP_ONLY='1'
$env:COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS='pydata__xarray-4075,pallets__flask-5014'
node .\tests\business\command-run-agent-benchmarks\agent_swebench_business_test.mjs
```

## SWE-bench Verified Repo Issue Table

Generated from `C:\Users\liuliu\Documents\benchmark\verified_repo_difficulty_stats.json`.
Total: 500 issues across 12 repos.
Use these ids with `COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS`.
`resolve_rate_pct` is the historical public-submission completion rate from the local difficulty stats file; `resolved/public_submission_attempts` is shown for context.

### astropy/astropy (22)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `astropy__astropy-12907` | 70.9% | 78 / 110 | 15 min - 1 hour |
| `astropy__astropy-13033` | 1.9% | 2 / 108 | 15 min - 1 hour |
| `astropy__astropy-13236` | 9.0% | 10 / 111 | 15 min - 1 hour |
| `astropy__astropy-13398` | 0.0% | 0 / 107 | 1-4 hours |
| `astropy__astropy-13453` | 54.5% | 60 / 110 | 15 min - 1 hour |
| `astropy__astropy-13579` | 78.0% | 85 / 109 | 1-4 hours |
| `astropy__astropy-13977` | 0.0% | 0 / 110 | 15 min - 1 hour |
| `astropy__astropy-14096` | 72.1% | 80 / 111 | 15 min - 1 hour |
| `astropy__astropy-14182` | 5.5% | 6 / 109 | 15 min - 1 hour |
| `astropy__astropy-14309` | 96.4% | 107 / 111 | <15 min fix |
| `astropy__astropy-14365` | 9.9% | 11 / 111 | 15 min - 1 hour |
| `astropy__astropy-14369` | 16.2% | 18 / 111 | 1-4 hours |
| `astropy__astropy-14508` | 60.9% | 67 / 110 | 15 min - 1 hour |
| `astropy__astropy-14539` | 78.4% | 87 / 111 | 15 min - 1 hour |
| `astropy__astropy-14598` | 6.5% | 7 / 107 | 15 min - 1 hour |
| `astropy__astropy-14995` | 90.0% | 99 / 110 | <15 min fix |
| `astropy__astropy-7166` | 86.1% | 93 / 108 | <15 min fix |
| `astropy__astropy-7336` | 94.5% | 104 / 110 | <15 min fix |
| `astropy__astropy-7606` | 8.4% | 9 / 107 | 15 min - 1 hour |
| `astropy__astropy-7671` | 91.7% | 99 / 108 | 15 min - 1 hour |
| `astropy__astropy-8707` | 3.8% | 4 / 106 | 15 min - 1 hour |
| `astropy__astropy-8872` | 3.7% | 4 / 108 | 15 min - 1 hour |

### django/django (231)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `django__django-10097` | 46.1% | 41 / 89 | <15 min fix |
| `django__django-10554` | 0.0% | 0 / 110 | 1-4 hours |
| `django__django-10880` | 78.4% | 87 / 111 | <15 min fix |
| `django__django-10914` | 90.0% | 99 / 110 | <15 min fix |
| `django__django-10973` | 83.8% | 93 / 111 | 15 min - 1 hour |
| `django__django-10999` | 0.0% | 0 / 109 | <15 min fix |
| `django__django-11066` | 100.0% | 109 / 109 | <15 min fix |
| `django__django-11087` | 0.0% | 0 / 106 | 15 min - 1 hour |
| `django__django-11095` | 93.7% | 104 / 111 | 15 min - 1 hour |
| `django__django-11099` | 99.1% | 107 / 108 | <15 min fix |
| `django__django-11119` | 97.3% | 108 / 111 | <15 min fix |
| `django__django-11133` | 96.4% | 107 / 111 | <15 min fix |
| `django__django-11138` | 11.1% | 12 / 108 | 1-4 hours |
| `django__django-11141` | 16.8% | 18 / 107 | 15 min - 1 hour |
| `django__django-11149` | 29.4% | 32 / 109 | 15 min - 1 hour |
| `django__django-11163` | 98.2% | 108 / 110 | <15 min fix |
| `django__django-11179` | 91.9% | 102 / 111 | <15 min fix |
| `django__django-11206` | 34.2% | 38 / 111 | 15 min - 1 hour |
| `django__django-11211` | 72.7% | 80 / 110 | 15 min - 1 hour |
| `django__django-11239` | 36.0% | 40 / 111 | <15 min fix |
| `django__django-11265` | 33.6% | 37 / 110 | 15 min - 1 hour |
| `django__django-11276` | 81.5% | 88 / 108 | 15 min - 1 hour |
| `django__django-11292` | 86.4% | 95 / 110 | 15 min - 1 hour |
| `django__django-11299` | 50.0% | 54 / 108 | <15 min fix |
| `django__django-11333` | 51.9% | 56 / 108 | 15 min - 1 hour |
| `django__django-11400` | 0.0% | 0 / 108 | 1-4 hours |
| `django__django-11433` | 13.8% | 15 / 109 | <15 min fix |
| `django__django-11451` | 80.2% | 89 / 111 | <15 min fix |
| `django__django-11477` | 1.9% | 2 / 107 | 15 min - 1 hour |
| `django__django-11490` | 39.6% | 44 / 111 | <15 min fix |
| `django__django-11532` | 43.6% | 48 / 110 | 15 min - 1 hour |
| `django__django-11551` | 95.5% | 106 / 111 | 15 min - 1 hour |
| `django__django-11555` | 42.2% | 46 / 109 | <15 min fix |
| `django__django-11603` | 96.4% | 106 / 110 | <15 min fix |
| `django__django-11728` | 9.6% | 10 / 104 | 15 min - 1 hour |
| `django__django-11734` | 2.8% | 3 / 107 | 15 min - 1 hour |
| `django__django-11740` | 66.0% | 68 / 103 | 15 min - 1 hour |
| `django__django-11749` | 76.4% | 84 / 110 | 15 min - 1 hour |
| `django__django-11790` | 22.7% | 25 / 110 | 15 min - 1 hour |
| `django__django-11815` | 69.4% | 77 / 111 | 15 min - 1 hour |
| `django__django-11820` | 0.9% | 1 / 110 | <15 min fix |
| `django__django-11848` | 31.8% | 35 / 110 | 15 min - 1 hour |
| `django__django-11880` | 99.1% | 110 / 111 | <15 min fix |
| `django__django-11885` | 6.7% | 7 / 105 | 1-4 hours |
| `django__django-11951` | 85.6% | 95 / 111 | <15 min fix |
| `django__django-11964` | 32.1% | 35 / 109 | <15 min fix |
| `django__django-11999` | 86.5% | 96 / 111 | 15 min - 1 hour |
| `django__django-12039` | 55.0% | 61 / 111 | 15 min - 1 hour |
| `django__django-12050` | 91.9% | 102 / 111 | 15 min - 1 hour |
| `django__django-12125` | 47.2% | 51 / 108 | <15 min fix |
| `django__django-12143` | 98.2% | 108 / 110 | 15 min - 1 hour |
| `django__django-12155` | 100.0% | 109 / 109 | 15 min - 1 hour |
| `django__django-12193` | 57.8% | 63 / 109 | <15 min fix |
| `django__django-12209` | 91.7% | 100 / 109 | <15 min fix |
| `django__django-12262` | 69.4% | 75 / 108 | 15 min - 1 hour |
| `django__django-12273` | 16.2% | 17 / 105 | 15 min - 1 hour |
| `django__django-12276` | 89.0% | 97 / 109 | <15 min fix |
| `django__django-12304` | 67.6% | 75 / 111 | <15 min fix |
| `django__django-12308` | 17.3% | 19 / 110 | <15 min fix |
| `django__django-12325` | 13.2% | 14 / 106 | 1-4 hours |
| `django__django-12406` | 0.0% | 0 / 107 | 15 min - 1 hour |
| `django__django-12419` | 95.5% | 105 / 110 | <15 min fix |
| `django__django-12663` | 46.8% | 51 / 109 | 15 min - 1 hour |
| `django__django-12708` | 70.9% | 78 / 110 | 1-4 hours |
| `django__django-12713` | 89.0% | 97 / 109 | 15 min - 1 hour |
| `django__django-12741` | 75.2% | 82 / 109 | <15 min fix |
| `django__django-12754` | 46.8% | 51 / 109 | 15 min - 1 hour |
| `django__django-12774` | 50.9% | 56 / 110 | 15 min - 1 hour |
| `django__django-12858` | 71.6% | 78 / 109 | 15 min - 1 hour |
| `django__django-12965` | 37.6% | 41 / 109 | 15 min - 1 hour |
| `django__django-13012` | 71.8% | 79 / 110 | 15 min - 1 hour |
| `django__django-13023` | 48.6% | 54 / 111 | <15 min fix |
| `django__django-13028` | 81.8% | 90 / 110 | 15 min - 1 hour |
| `django__django-13033` | 60.0% | 66 / 110 | 15 min - 1 hour |
| `django__django-13089` | 96.4% | 107 / 111 | <15 min fix |
| `django__django-13109` | 98.2% | 109 / 111 | <15 min fix |
| `django__django-13112` | 44.7% | 46 / 103 | <15 min fix |
| `django__django-13121` | 49.5% | 54 / 109 | 15 min - 1 hour |
| `django__django-13128` | 43.6% | 48 / 110 | 1-4 hours |
| `django__django-13158` | 58.7% | 64 / 109 | 15 min - 1 hour |
| `django__django-13195` | 1.9% | 2 / 107 | 15 min - 1 hour |
| `django__django-13212` | 0.0% | 0 / 110 | 1-4 hours |
| `django__django-13279` | 67.6% | 75 / 111 | 15 min - 1 hour |
| `django__django-13297` | 47.7% | 52 / 109 | <15 min fix |
| `django__django-13315` | 64.5% | 71 / 110 | 15 min - 1 hour |
| `django__django-13343` | 76.4% | 84 / 110 | 15 min - 1 hour |
| `django__django-13344` | 3.7% | 4 / 108 | 1-4 hours |
| `django__django-13346` | 47.2% | 50 / 106 | 15 min - 1 hour |
| `django__django-13363` | 94.6% | 105 / 111 | <15 min fix |
| `django__django-13401` | 70.9% | 78 / 110 | 15 min - 1 hour |
| `django__django-13406` | 52.3% | 57 / 109 | <15 min fix |
| `django__django-13410` | 97.2% | 105 / 108 | <15 min fix |
| `django__django-13417` | 86.5% | 96 / 111 | <15 min fix |
| `django__django-13449` | 60.4% | 67 / 111 | 1-4 hours |
| `django__django-13512` | 32.7% | 36 / 110 | <15 min fix |
| `django__django-13513` | 0.0% | 0 / 94 | 15 min - 1 hour |
| `django__django-13516` | 90.9% | 100 / 110 | 15 min - 1 hour |
| `django__django-13551` | 44.5% | 49 / 110 | <15 min fix |
| `django__django-13568` | 41.4% | 46 / 111 | 15 min - 1 hour |
| `django__django-13569` | 75.7% | 84 / 111 | 15 min - 1 hour |
| `django__django-13590` | 76.4% | 84 / 110 | 15 min - 1 hour |
| `django__django-13658` | 100.0% | 110 / 110 | 15 min - 1 hour |
| `django__django-13670` | 92.7% | 102 / 110 | 15 min - 1 hour |
| `django__django-13741` | 98.2% | 107 / 109 | <15 min fix |
| `django__django-13786` | 91.0% | 101 / 111 | 15 min - 1 hour |
| `django__django-13794` | 6.4% | 7 / 110 | <15 min fix |
| `django__django-13807` | 32.7% | 35 / 107 | <15 min fix |
| `django__django-13809` | 81.7% | 89 / 109 | 15 min - 1 hour |
| `django__django-13810` | 77.5% | 86 / 111 | 15 min - 1 hour |
| `django__django-13820` | 94.6% | 105 / 111 | 15 min - 1 hour |
| `django__django-13821` | 94.6% | 105 / 111 | <15 min fix |
| `django__django-13837` | 80.0% | 88 / 110 | 1-4 hours |
| `django__django-13925` | 47.2% | 51 / 108 | 15 min - 1 hour |
| `django__django-13933` | 90.7% | 98 / 108 | <15 min fix |
| `django__django-13964` | 59.6% | 65 / 109 | 15 min - 1 hour |
| `django__django-14007` | 68.5% | 74 / 108 | 1-4 hours |
| `django__django-14011` | 0.0% | 0 / 95 | 1-4 hours |
| `django__django-14017` | 65.8% | 73 / 111 | 15 min - 1 hour |
| `django__django-14034` | 0.0% | 0 / 108 | 15 min - 1 hour |
| `django__django-14053` | 79.8% | 87 / 109 | 15 min - 1 hour |
| `django__django-14089` | 99.1% | 109 / 110 | <15 min fix |
| `django__django-14122` | 61.8% | 68 / 110 | 15 min - 1 hour |
| `django__django-14140` | 25.0% | 27 / 108 | 15 min - 1 hour |
| `django__django-14155` | 0.0% | 0 / 109 | 15 min - 1 hour |
| `django__django-14170` | 2.8% | 3 / 108 | 15 min - 1 hour |
| `django__django-14238` | 87.3% | 96 / 110 | 15 min - 1 hour |
| `django__django-14311` | 49.1% | 54 / 110 | 15 min - 1 hour |
| `django__django-14315` | 2.8% | 3 / 109 | <15 min fix |
| `django__django-14349` | 91.6% | 98 / 107 | 15 min - 1 hour |
| `django__django-14351` | 40.2% | 43 / 107 | 15 min - 1 hour |
| `django__django-14373` | 100.0% | 111 / 111 | <15 min fix |
| `django__django-14376` | 21.1% | 23 / 109 | <15 min fix |
| `django__django-14404` | 31.1% | 32 / 103 | <15 min fix |
| `django__django-14434` | 78.0% | 85 / 109 | 15 min - 1 hour |
| `django__django-14493` | 83.8% | 93 / 111 | <15 min fix |
| `django__django-14500` | 72.7% | 80 / 110 | 15 min - 1 hour |
| `django__django-14534` | 9.3% | 10 / 108 | <15 min fix |
| `django__django-14539` | 80.9% | 89 / 110 | <15 min fix |
| `django__django-14559` | 75.9% | 82 / 108 | 15 min - 1 hour |
| `django__django-14580` | 54.5% | 60 / 110 | <15 min fix |
| `django__django-14608` | 74.8% | 83 / 111 | <15 min fix |
| `django__django-14631` | 37.4% | 40 / 107 | 1-4 hours |
| `django__django-14672` | 95.4% | 103 / 108 | 15 min - 1 hour |
| `django__django-14725` | 5.5% | 6 / 109 | 15 min - 1 hour |
| `django__django-14752` | 96.4% | 107 / 111 | <15 min fix |
| `django__django-14765` | 93.7% | 104 / 111 | <15 min fix |
| `django__django-14771` | 52.7% | 58 / 110 | 15 min - 1 hour |
| `django__django-14787` | 83.6% | 92 / 110 | <15 min fix |
| `django__django-14792` | 0.9% | 1 / 109 | <15 min fix |
| `django__django-14855` | 98.2% | 109 / 111 | <15 min fix |
| `django__django-14915` | 99.1% | 107 / 108 | <15 min fix |
| `django__django-14999` | 66.4% | 73 / 110 | <15 min fix |
| `django__django-15022` | 59.6% | 65 / 109 | 15 min - 1 hour |
| `django__django-15037` | 54.1% | 59 / 109 | 15 min - 1 hour |
| `django__django-15098` | 1.8% | 2 / 109 | 15 min - 1 hour |
| `django__django-15103` | 76.6% | 85 / 111 | 15 min - 1 hour |
| `django__django-15104` | 99.1% | 110 / 111 | <15 min fix |
| `django__django-15127` | 24.5% | 27 / 110 | <15 min fix |
| `django__django-15128` | 52.3% | 57 / 109 | 1-4 hours |
| `django__django-15161` | 52.8% | 57 / 108 | 15 min - 1 hour |
| `django__django-15252` | 0.0% | 0 / 107 | 15 min - 1 hour |
| `django__django-15268` | 47.7% | 51 / 107 | 1-4 hours |
| `django__django-15277` | 98.2% | 109 / 111 | <15 min fix |
| `django__django-15278` | 85.3% | 93 / 109 | 15 min - 1 hour |
| `django__django-15280` | 8.5% | 9 / 106 | 15 min - 1 hour |
| `django__django-15315` | 91.9% | 102 / 111 | <15 min fix |
| `django__django-15368` | 98.2% | 107 / 109 | <15 min fix |
| `django__django-15375` | 51.0% | 50 / 98 | 15 min - 1 hour |
| `django__django-15380` | 80.2% | 89 / 111 | <15 min fix |
| `django__django-15382` | 68.5% | 76 / 111 | 15 min - 1 hour |
| `django__django-15467` | 99.1% | 110 / 111 | <15 min fix |
| `django__django-15499` | 92.6% | 100 / 108 | <15 min fix |
| `django__django-15503` | 15.9% | 17 / 107 | 1-4 hours |
| `django__django-15525` | 62.6% | 67 / 107 | 15 min - 1 hour |
| `django__django-15554` | 17.8% | 19 / 107 | 15 min - 1 hour |
| `django__django-15561` | 84.4% | 92 / 109 | 15 min - 1 hour |
| `django__django-15563` | 13.9% | 15 / 108 | 15 min - 1 hour |
| `django__django-15569` | 95.5% | 106 / 111 | <15 min fix |
| `django__django-15572` | 80.2% | 89 / 111 | <15 min fix |
| `django__django-15629` | 0.0% | 0 / 106 | 1-4 hours |
| `django__django-15695` | 9.3% | 10 / 107 | 15 min - 1 hour |
| `django__django-15731` | 91.8% | 101 / 110 | 15 min - 1 hour |
| `django__django-15732` | 15.0% | 16 / 107 | 15 min - 1 hour |
| `django__django-15741` | 91.0% | 101 / 111 | <15 min fix |
| `django__django-15814` | 76.6% | 85 / 111 | 15 min - 1 hour |
| `django__django-15851` | 95.4% | 104 / 109 | <15 min fix |
| `django__django-15863` | 82.9% | 92 / 111 | <15 min fix |
| `django__django-15916` | 29.4% | 32 / 109 | 15 min - 1 hour |
| `django__django-15930` | 62.4% | 68 / 109 | <15 min fix |
| `django__django-15957` | 5.6% | 6 / 108 | 1-4 hours |
| `django__django-15973` | 4.9% | 5 / 102 | 15 min - 1 hour |
| `django__django-15987` | 69.4% | 77 / 111 | <15 min fix |
| `django__django-16032` | 60.7% | 65 / 107 | 15 min - 1 hour |
| `django__django-16082` | 71.8% | 79 / 110 | 15 min - 1 hour |
| `django__django-16100` | 74.5% | 82 / 110 | <15 min fix |
| `django__django-16116` | 85.0% | 91 / 107 | <15 min fix |
| `django__django-16136` | 79.8% | 87 / 109 | 15 min - 1 hour |
| `django__django-16139` | 96.4% | 107 / 111 | <15 min fix |
| `django__django-16145` | 92.7% | 102 / 110 | <15 min fix |
| `django__django-16255` | 98.2% | 109 / 111 | <15 min fix |
| `django__django-16256` | 3.8% | 4 / 106 | 15 min - 1 hour |
| `django__django-16263` | 0.0% | 0 / 104 | 1-4 hours |
| `django__django-16315` | 40.7% | 44 / 108 | 15 min - 1 hour |
| `django__django-16333` | 96.4% | 107 / 111 | <15 min fix |
| `django__django-16429` | 97.3% | 107 / 110 | <15 min fix |
| `django__django-16454` | 28.0% | 30 / 107 | 15 min - 1 hour |
| `django__django-16485` | 91.9% | 102 / 111 | 15 min - 1 hour |
| `django__django-16493` | 91.0% | 101 / 111 | 15 min - 1 hour |
| `django__django-16502` | 0.0% | 0 / 108 | 15 min - 1 hour |
| `django__django-16527` | 98.2% | 109 / 111 | 15 min - 1 hour |
| `django__django-16560` | 32.7% | 35 / 107 | 1-4 hours |
| `django__django-16569` | 97.3% | 108 / 111 | <15 min fix |
| `django__django-16595` | 95.5% | 105 / 110 | <15 min fix |
| `django__django-16612` | 88.3% | 98 / 111 | 15 min - 1 hour |
| `django__django-16631` | 0.0% | 0 / 109 | 1-4 hours |
| `django__django-16642` | 62.4% | 68 / 109 | <15 min fix |
| `django__django-16661` | 55.9% | 62 / 111 | 15 min - 1 hour |
| `django__django-16662` | 89.8% | 97 / 108 | 15 min - 1 hour |
| `django__django-16667` | 0.0% | 0 / 108 | 15 min - 1 hour |
| `django__django-16801` | 99.1% | 107 / 108 | <15 min fix |
| `django__django-16819` | 84.4% | 92 / 109 | 15 min - 1 hour |
| `django__django-16877` | 52.3% | 57 / 109 | 15 min - 1 hour |
| `django__django-16899` | 81.7% | 89 / 109 | <15 min fix |
| `django__django-16901` | 78.7% | 85 / 108 | 15 min - 1 hour |
| `django__django-16938` | 23.6% | 25 / 106 | 15 min - 1 hour |
| `django__django-16950` | 16.8% | 18 / 107 | 15 min - 1 hour |
| `django__django-17029` | 96.4% | 106 / 110 | <15 min fix |
| `django__django-17084` | 38.7% | 41 / 106 | 15 min - 1 hour |
| `django__django-17087` | 73.6% | 81 / 110 | <15 min fix |
| `django__django-7530` | 92.9% | 91 / 98 | 15 min - 1 hour |
| `django__django-9296` | 96.4% | 107 / 111 | <15 min fix |

### matplotlib/matplotlib (34)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `matplotlib__matplotlib-13989` | 85.8% | 91 / 106 | <15 min fix |
| `matplotlib__matplotlib-14623` | 48.6% | 51 / 105 | 15 min - 1 hour |
| `matplotlib__matplotlib-20488` | 28.4% | 29 / 102 | 15 min - 1 hour |
| `matplotlib__matplotlib-20676` | 17.5% | 18 / 103 | <15 min fix |
| `matplotlib__matplotlib-20826` | 33.0% | 35 / 106 | 15 min - 1 hour |
| `matplotlib__matplotlib-20859` | 78.3% | 83 / 106 | <15 min fix |
| `matplotlib__matplotlib-21568` | 0.0% | 0 / 104 | 15 min - 1 hour |
| `matplotlib__matplotlib-22719` | 96.3% | 104 / 108 | <15 min fix |
| `matplotlib__matplotlib-22865` | 41.5% | 44 / 106 | 15 min - 1 hour |
| `matplotlib__matplotlib-22871` | 37.6% | 41 / 109 | 15 min - 1 hour |
| `matplotlib__matplotlib-23299` | 4.8% | 5 / 104 | 15 min - 1 hour |
| `matplotlib__matplotlib-23314` | 79.8% | 87 / 109 | 15 min - 1 hour |
| `matplotlib__matplotlib-23412` | 85.8% | 91 / 106 | 15 min - 1 hour |
| `matplotlib__matplotlib-23476` | 1.9% | 2 / 107 | <15 min fix |
| `matplotlib__matplotlib-24026` | 80.9% | 89 / 110 | 15 min - 1 hour |
| `matplotlib__matplotlib-24149` | 83.2% | 89 / 107 | <15 min fix |
| `matplotlib__matplotlib-24177` | 9.5% | 10 / 105 | <15 min fix |
| `matplotlib__matplotlib-24570` | 73.3% | 77 / 105 | <15 min fix |
| `matplotlib__matplotlib-24627` | 63.3% | 69 / 109 | 15 min - 1 hour |
| `matplotlib__matplotlib-24637` | 63.6% | 68 / 107 | 15 min - 1 hour |
| `matplotlib__matplotlib-24870` | 2.8% | 3 / 107 | 15 min - 1 hour |
| `matplotlib__matplotlib-24970` | 87.2% | 95 / 109 | 15 min - 1 hour |
| `matplotlib__matplotlib-25122` | 89.9% | 98 / 109 | <15 min fix |
| `matplotlib__matplotlib-25287` | 89.8% | 97 / 108 | <15 min fix |
| `matplotlib__matplotlib-25311` | 51.4% | 56 / 109 | <15 min fix |
| `matplotlib__matplotlib-25332` | 55.6% | 60 / 108 | <15 min fix |
| `matplotlib__matplotlib-25479` | 0.0% | 0 / 106 | <15 min fix |
| `matplotlib__matplotlib-25775` | 50.9% | 55 / 108 | 15 min - 1 hour |
| `matplotlib__matplotlib-25960` | 8.5% | 9 / 106 | 15 min - 1 hour |
| `matplotlib__matplotlib-26113` | 93.5% | 101 / 108 | <15 min fix |
| `matplotlib__matplotlib-26208` | 2.0% | 2 / 102 | <15 min fix |
| `matplotlib__matplotlib-26291` | 54.2% | 58 / 107 | 15 min - 1 hour |
| `matplotlib__matplotlib-26342` | 85.7% | 90 / 105 | 15 min - 1 hour |
| `matplotlib__matplotlib-26466` | 0.0% | 0 / 108 | 15 min - 1 hour |

### mwaskom/seaborn (2)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `mwaskom__seaborn-3069` | 35.8% | 39 / 109 | 15 min - 1 hour |
| `mwaskom__seaborn-3187` | 5.5% | 6 / 109 | 15 min - 1 hour |

### pallets/flask (1)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `pallets__flask-5014` | 96.4% | 106 / 110 | <15 min fix |

### psf/requests (8)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `psf__requests-1142` | 66.3% | 57 / 86 | <15 min fix |
| `psf__requests-1724` | 64.9% | 72 / 111 | <15 min fix |
| `psf__requests-1766` | 76.6% | 85 / 111 | <15 min fix |
| `psf__requests-1921` | 58.7% | 64 / 109 | <15 min fix |
| `psf__requests-2317` | 65.8% | 73 / 111 | <15 min fix |
| `psf__requests-2931` | 34.9% | 37 / 106 | 15 min - 1 hour |
| `psf__requests-5414` | 58.9% | 66 / 112 | <15 min fix |
| `psf__requests-6028` | 5.5% | 6 / 110 | 15 min - 1 hour |

### pydata/xarray (22)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `pydata__xarray-2905` | 63.6% | 70 / 110 | 15 min - 1 hour |
| `pydata__xarray-3095` | 60.6% | 66 / 109 | 15 min - 1 hour |
| `pydata__xarray-3151` | 83.6% | 92 / 110 | 15 min - 1 hour |
| `pydata__xarray-3305` | 80.0% | 88 / 110 | 15 min - 1 hour |
| `pydata__xarray-3677` | 92.8% | 103 / 111 | 15 min - 1 hour |
| `pydata__xarray-3993` | 42.6% | 46 / 108 | 1-4 hours |
| `pydata__xarray-4075` | 97.3% | 108 / 111 | <15 min fix |
| `pydata__xarray-4094` | 52.3% | 56 / 107 | <15 min fix |
| `pydata__xarray-4356` | 90.1% | 100 / 111 | <15 min fix |
| `pydata__xarray-4629` | 100.0% | 111 / 111 | <15 min fix |
| `pydata__xarray-4687` | 27.8% | 30 / 108 | 15 min - 1 hour |
| `pydata__xarray-4695` | 45.5% | 50 / 110 | 15 min - 1 hour |
| `pydata__xarray-4966` | 89.1% | 98 / 110 | 15 min - 1 hour |
| `pydata__xarray-6461` | 85.0% | 91 / 107 | <15 min fix |
| `pydata__xarray-6599` | 35.2% | 37 / 105 | 15 min - 1 hour |
| `pydata__xarray-6721` | 52.9% | 55 / 104 | 15 min - 1 hour |
| `pydata__xarray-6744` | 53.2% | 59 / 111 | 15 min - 1 hour |
| `pydata__xarray-6938` | 10.2% | 11 / 108 | 15 min - 1 hour |
| `pydata__xarray-6992` | 0.0% | 0 / 108 | >4 hours |
| `pydata__xarray-7229` | 0.0% | 0 / 106 | 15 min - 1 hour |
| `pydata__xarray-7233` | 89.2% | 99 / 111 | 15 min - 1 hour |
| `pydata__xarray-7393` | 48.1% | 51 / 106 | 15 min - 1 hour |

### pylint-dev/pylint (10)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `pylint-dev__pylint-4551` | 0.0% | 0 / 108 | 1-4 hours |
| `pylint-dev__pylint-4604` | 0.0% | 0 / 107 | 15 min - 1 hour |
| `pylint-dev__pylint-4661` | 0.0% | 0 / 107 | 15 min - 1 hour |
| `pylint-dev__pylint-4970` | 18.3% | 20 / 109 | <15 min fix |
| `pylint-dev__pylint-6386` | 25.9% | 28 / 108 | 15 min - 1 hour |
| `pylint-dev__pylint-6528` | 48.1% | 51 / 106 | 15 min - 1 hour |
| `pylint-dev__pylint-6903` | 100.0% | 111 / 111 | <15 min fix |
| `pylint-dev__pylint-7080` | 16.0% | 17 / 106 | 15 min - 1 hour |
| `pylint-dev__pylint-7277` | 80.9% | 89 / 110 | <15 min fix |
| `pylint-dev__pylint-8898` | 8.4% | 9 / 107 | 1-4 hours |

### pytest-dev/pytest (19)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `pytest-dev__pytest-10051` | 31.8% | 35 / 110 | 15 min - 1 hour |
| `pytest-dev__pytest-10081` | 76.4% | 84 / 110 | <15 min fix |
| `pytest-dev__pytest-10356` | 1.0% | 1 / 105 | 1-4 hours |
| `pytest-dev__pytest-5262` | 74.5% | 82 / 110 | <15 min fix |
| `pytest-dev__pytest-5631` | 77.3% | 85 / 110 | 15 min - 1 hour |
| `pytest-dev__pytest-5787` | 27.4% | 29 / 106 | 1-4 hours |
| `pytest-dev__pytest-5809` | 100.0% | 111 / 111 | <15 min fix |
| `pytest-dev__pytest-5840` | 0.9% | 1 / 108 | 15 min - 1 hour |
| `pytest-dev__pytest-6197` | 23.1% | 25 / 108 | 1-4 hours |
| `pytest-dev__pytest-6202` | 98.2% | 109 / 111 | <15 min fix |
| `pytest-dev__pytest-7205` | 67.6% | 75 / 111 | <15 min fix |
| `pytest-dev__pytest-7236` | 66.4% | 73 / 110 | 15 min - 1 hour |
| `pytest-dev__pytest-7324` | 33.9% | 37 / 109 | 15 min - 1 hour |
| `pytest-dev__pytest-7432` | 75.7% | 84 / 111 | <15 min fix |
| `pytest-dev__pytest-7490` | 54.6% | 59 / 108 | 15 min - 1 hour |
| `pytest-dev__pytest-7521` | 72.5% | 79 / 109 | <15 min fix |
| `pytest-dev__pytest-7571` | 85.3% | 93 / 109 | 15 min - 1 hour |
| `pytest-dev__pytest-7982` | 93.5% | 101 / 108 | <15 min fix |
| `pytest-dev__pytest-8399` | 97.3% | 107 / 110 | 15 min - 1 hour |

### scikit-learn/scikit-learn (32)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `scikit-learn__scikit-learn-10297` | 96.4% | 107 / 111 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-10844` | 100.0% | 111 / 111 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-10908` | 58.2% | 64 / 110 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-11310` | 88.2% | 97 / 110 | <15 min fix |
| `scikit-learn__scikit-learn-11578` | 97.3% | 107 / 110 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-12585` | 98.2% | 109 / 111 | <15 min fix |
| `scikit-learn__scikit-learn-12682` | 55.5% | 61 / 110 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-12973` | 73.0% | 81 / 111 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-13124` | 53.2% | 59 / 111 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-13135` | 86.5% | 96 / 111 | <15 min fix |
| `scikit-learn__scikit-learn-13142` | 82.0% | 91 / 111 | <15 min fix |
| `scikit-learn__scikit-learn-13328` | 93.7% | 104 / 111 | <15 min fix |
| `scikit-learn__scikit-learn-13439` | 96.3% | 104 / 108 | <15 min fix |
| `scikit-learn__scikit-learn-13496` | 88.3% | 98 / 111 | <15 min fix |
| `scikit-learn__scikit-learn-13779` | 91.8% | 101 / 110 | <15 min fix |
| `scikit-learn__scikit-learn-14053` | 86.5% | 96 / 111 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-14087` | 49.1% | 53 / 108 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-14141` | 96.4% | 107 / 111 | <15 min fix |
| `scikit-learn__scikit-learn-14496` | 99.1% | 109 / 110 | <15 min fix |
| `scikit-learn__scikit-learn-14629` | 14.5% | 16 / 110 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-14710` | 91.8% | 89 / 97 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-14894` | 99.1% | 110 / 111 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-14983` | 40.4% | 44 / 109 | <15 min fix |
| `scikit-learn__scikit-learn-15100` | 96.4% | 107 / 111 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-25102` | 36.9% | 38 / 103 | 1-4 hours |
| `scikit-learn__scikit-learn-25232` | 81.0% | 81 / 100 | <15 min fix |
| `scikit-learn__scikit-learn-25747` | 13.6% | 14 / 103 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-25931` | 80.0% | 84 / 105 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-25973` | 76.0% | 79 / 104 | <15 min fix |
| `scikit-learn__scikit-learn-26194` | 2.9% | 3 / 104 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-26323` | 90.6% | 96 / 106 | 15 min - 1 hour |
| `scikit-learn__scikit-learn-9288` | 81.7% | 89 / 109 | 15 min - 1 hour |

### sphinx-doc/sphinx (44)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `sphinx-doc__sphinx-10323` | 45.4% | 49 / 108 | <15 min fix |
| `sphinx-doc__sphinx-10435` | 1.9% | 2 / 105 | <15 min fix |
| `sphinx-doc__sphinx-10449` | 54.6% | 59 / 108 | <15 min fix |
| `sphinx-doc__sphinx-10466` | 79.3% | 88 / 111 | 15 min - 1 hour |
| `sphinx-doc__sphinx-10614` | 1.9% | 2 / 108 | 15 min - 1 hour |
| `sphinx-doc__sphinx-10673` | 58.7% | 64 / 109 | 15 min - 1 hour |
| `sphinx-doc__sphinx-11445` | 44.4% | 48 / 108 | 15 min - 1 hour |
| `sphinx-doc__sphinx-11510` | 0.0% | 0 / 104 | 1-4 hours |
| `sphinx-doc__sphinx-7440` | 76.9% | 80 / 104 | <15 min fix |
| `sphinx-doc__sphinx-7454` | 45.5% | 50 / 110 | <15 min fix |
| `sphinx-doc__sphinx-7462` | 0.0% | 0 / 109 | <15 min fix |
| `sphinx-doc__sphinx-7590` | 1.0% | 1 / 102 | >4 hours |
| `sphinx-doc__sphinx-7748` | 0.0% | 0 / 105 | 15 min - 1 hour |
| `sphinx-doc__sphinx-7757` | 54.5% | 60 / 110 | 15 min - 1 hour |
| `sphinx-doc__sphinx-7889` | 73.4% | 80 / 109 | <15 min fix |
| `sphinx-doc__sphinx-7910` | 63.6% | 68 / 107 | <15 min fix |
| `sphinx-doc__sphinx-7985` | 3.8% | 4 / 104 | 15 min - 1 hour |
| `sphinx-doc__sphinx-8035` | 60.9% | 67 / 110 | 15 min - 1 hour |
| `sphinx-doc__sphinx-8056` | 15.7% | 17 / 108 | 15 min - 1 hour |
| `sphinx-doc__sphinx-8120` | 72.0% | 77 / 107 | 15 min - 1 hour |
| `sphinx-doc__sphinx-8265` | 12.3% | 13 / 106 | 15 min - 1 hour |
| `sphinx-doc__sphinx-8269` | 75.9% | 82 / 108 | <15 min fix |
| `sphinx-doc__sphinx-8459` | 55.6% | 60 / 108 | <15 min fix |
| `sphinx-doc__sphinx-8475` | 81.6% | 84 / 103 | <15 min fix |
| `sphinx-doc__sphinx-8548` | 11.1% | 12 / 108 | 1-4 hours |
| `sphinx-doc__sphinx-8551` | 39.3% | 42 / 107 | 15 min - 1 hour |
| `sphinx-doc__sphinx-8593` | 48.1% | 52 / 108 | 15 min - 1 hour |
| `sphinx-doc__sphinx-8595` | 80.9% | 89 / 110 | <15 min fix |
| `sphinx-doc__sphinx-8621` | 39.8% | 43 / 108 | <15 min fix |
| `sphinx-doc__sphinx-8638` | 10.3% | 11 / 107 | 15 min - 1 hour |
| `sphinx-doc__sphinx-8721` | 91.7% | 100 / 109 | <15 min fix |
| `sphinx-doc__sphinx-9229` | 0.9% | 1 / 106 | 1-4 hours |
| `sphinx-doc__sphinx-9230` | 36.4% | 39 / 107 | <15 min fix |
| `sphinx-doc__sphinx-9258` | 33.7% | 35 / 104 | <15 min fix |
| `sphinx-doc__sphinx-9281` | 66.7% | 74 / 111 | <15 min fix |
| `sphinx-doc__sphinx-9320` | 90.9% | 100 / 110 | <15 min fix |
| `sphinx-doc__sphinx-9367` | 91.7% | 100 / 109 | <15 min fix |
| `sphinx-doc__sphinx-9461` | 1.0% | 1 / 104 | 1-4 hours |
| `sphinx-doc__sphinx-9591` | 33.9% | 37 / 109 | <15 min fix |
| `sphinx-doc__sphinx-9602` | 6.5% | 7 / 108 | 15 min - 1 hour |
| `sphinx-doc__sphinx-9658` | 34.9% | 38 / 109 | 15 min - 1 hour |
| `sphinx-doc__sphinx-9673` | 58.7% | 64 / 109 | 15 min - 1 hour |
| `sphinx-doc__sphinx-9698` | 88.1% | 96 / 109 | <15 min fix |
| `sphinx-doc__sphinx-9711` | 78.4% | 87 / 111 | <15 min fix |

### sympy/sympy (75)

| issue_id | resolve_rate_pct | resolved / attempts | difficulty |
|---|---:|---:|---|
| `sympy__sympy-11618` | 84.5% | 93 / 110 | 15 min - 1 hour |
| `sympy__sympy-12096` | 93.7% | 104 / 111 | <15 min fix |
| `sympy__sympy-12419` | 57.8% | 63 / 109 | 15 min - 1 hour |
| `sympy__sympy-12481` | 73.6% | 81 / 110 | <15 min fix |
| `sympy__sympy-12489` | 13.2% | 14 / 106 | 1-4 hours |
| `sympy__sympy-13031` | 28.0% | 30 / 107 | 15 min - 1 hour |
| `sympy__sympy-13091` | 18.5% | 20 / 108 | 15 min - 1 hour |
| `sympy__sympy-13372` | 88.1% | 96 / 109 | <15 min fix |
| `sympy__sympy-13480` | 97.3% | 108 / 111 | <15 min fix |
| `sympy__sympy-13551` | 39.1% | 43 / 110 | 15 min - 1 hour |
| `sympy__sympy-13615` | 38.2% | 42 / 110 | 15 min - 1 hour |
| `sympy__sympy-13647` | 89.0% | 97 / 109 | 15 min - 1 hour |
| `sympy__sympy-13757` | 51.4% | 55 / 107 | 15 min - 1 hour |
| `sympy__sympy-13798` | 5.7% | 6 / 106 | 15 min - 1 hour |
| `sympy__sympy-13852` | 0.0% | 0 / 109 | 1-4 hours |
| `sympy__sympy-13877` | 26.9% | 29 / 108 | 15 min - 1 hour |
| `sympy__sympy-13878` | 56.6% | 60 / 106 | >4 hours |
| `sympy__sympy-13974` | 16.5% | 18 / 109 | 15 min - 1 hour |
| `sympy__sympy-14248` | 6.6% | 7 / 106 | 1-4 hours |
| `sympy__sympy-14531` | 65.1% | 71 / 109 | 15 min - 1 hour |
| `sympy__sympy-14711` | 86.2% | 94 / 109 | <15 min fix |
| `sympy__sympy-14976` | 53.3% | 57 / 107 | 15 min - 1 hour |
| `sympy__sympy-15017` | 45.0% | 49 / 109 | <15 min fix |
| `sympy__sympy-15345` | 46.4% | 51 / 110 | <15 min fix |
| `sympy__sympy-15349` | 78.0% | 85 / 109 | 15 min - 1 hour |
| `sympy__sympy-15599` | 21.3% | 23 / 108 | 15 min - 1 hour |
| `sympy__sympy-15809` | 71.3% | 77 / 108 | <15 min fix |
| `sympy__sympy-15875` | 51.4% | 55 / 107 | <15 min fix |
| `sympy__sympy-15976` | 20.0% | 22 / 110 | 15 min - 1 hour |
| `sympy__sympy-16450` | 87.9% | 94 / 107 | <15 min fix |
| `sympy__sympy-16597` | 0.0% | 0 / 109 | 1-4 hours |
| `sympy__sympy-16766` | 96.3% | 105 / 109 | <15 min fix |
| `sympy__sympy-16792` | 57.8% | 63 / 109 | 15 min - 1 hour |
| `sympy__sympy-16886` | 97.2% | 105 / 108 | <15 min fix |
| `sympy__sympy-17139` | 69.1% | 76 / 110 | <15 min fix |
| `sympy__sympy-17318` | 5.6% | 6 / 108 | 15 min - 1 hour |
| `sympy__sympy-17630` | 1.9% | 2 / 108 | 1-4 hours |
| `sympy__sympy-17655` | 73.6% | 81 / 110 | <15 min fix |
| `sympy__sympy-18189` | 78.2% | 86 / 110 | <15 min fix |
| `sympy__sympy-18199` | 0.9% | 1 / 106 | 1-4 hours |
| `sympy__sympy-18211` | 48.1% | 52 / 108 | 15 min - 1 hour |
| `sympy__sympy-18698` | 28.2% | 29 / 103 | 15 min - 1 hour |
| `sympy__sympy-18763` | 43.4% | 46 / 106 | <15 min fix |
| `sympy__sympy-19040` | 20.6% | 21 / 102 | 15 min - 1 hour |
| `sympy__sympy-19346` | 84.5% | 93 / 110 | 15 min - 1 hour |
| `sympy__sympy-19495` | 56.5% | 61 / 108 | <15 min fix |
| `sympy__sympy-19637` | 92.8% | 103 / 111 | <15 min fix |
| `sympy__sympy-19783` | 52.3% | 56 / 107 | 15 min - 1 hour |
| `sympy__sympy-19954` | 83.6% | 92 / 110 | <15 min fix |
| `sympy__sympy-20154` | 85.6% | 95 / 111 | 15 min - 1 hour |
| `sympy__sympy-20428` | 0.0% | 0 / 105 | 15 min - 1 hour |
| `sympy__sympy-20438` | 0.0% | 0 / 108 | 15 min - 1 hour |
| `sympy__sympy-20590` | 52.8% | 56 / 106 | 15 min - 1 hour |
| `sympy__sympy-20801` | 69.7% | 76 / 109 | 15 min - 1 hour |
| `sympy__sympy-20916` | 5.6% | 6 / 107 | <15 min fix |
| `sympy__sympy-21379` | 39.8% | 41 / 103 | 15 min - 1 hour |
| `sympy__sympy-21596` | 0.9% | 1 / 109 | 15 min - 1 hour |
| `sympy__sympy-21612` | 4.6% | 5 / 109 | 15 min - 1 hour |
| `sympy__sympy-21847` | 90.1% | 100 / 111 | <15 min fix |
| `sympy__sympy-21930` | 0.0% | 0 / 107 | 15 min - 1 hour |
| `sympy__sympy-22080` | 1.9% | 2 / 103 | 15 min - 1 hour |
| `sympy__sympy-22456` | 78.9% | 86 / 109 | 15 min - 1 hour |
| `sympy__sympy-22714` | 85.5% | 94 / 110 | <15 min fix |
| `sympy__sympy-22914` | 94.5% | 104 / 110 | 15 min - 1 hour |
| `sympy__sympy-23262` | 75.0% | 81 / 108 | 15 min - 1 hour |
| `sympy__sympy-23413` | 17.4% | 19 / 109 | 15 min - 1 hour |
| `sympy__sympy-23534` | 82.6% | 90 / 109 | <15 min fix |
| `sympy__sympy-23824` | 89.8% | 97 / 108 | 15 min - 1 hour |
| `sympy__sympy-23950` | 70.4% | 76 / 108 | 15 min - 1 hour |
| `sympy__sympy-24066` | 75.7% | 84 / 111 | 15 min - 1 hour |
| `sympy__sympy-24213` | 94.6% | 105 / 111 | 15 min - 1 hour |
| `sympy__sympy-24443` | 91.7% | 100 / 109 | 15 min - 1 hour |
| `sympy__sympy-24539` | 94.5% | 104 / 110 | <15 min fix |
| `sympy__sympy-24562` | 44.0% | 48 / 109 | <15 min fix |
| `sympy__sympy-24661` | 80.0% | 88 / 110 | 15 min - 1 hour |


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
