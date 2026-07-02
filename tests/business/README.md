# Business Tests

This directory contains root backend business tests and shared business-test
helpers. Backend business runners only execute Rust tests from this directory;
they do not execute `.mjs` TUI, GUI, browser, OS testing, or app scripts.

Root Rust tests in this directory are part of the backend business runner.
Shared `.mjs` helper files are not executable app tests and must not be wired
into CI or crate tests as one-off fixtures.

App-owned scripts belong under their app package, such as
`apps/tui/tests/e2e/business/` or `apps/gui/e2e/business/`, and must be run
through those app scripts explicitly.

The workspace test types are peers: `tests/business`, `tests/os_testing`,
`tests/performance`, `tests/live`, and `benchmark`. Crate-owned Rust
tests follow the same peer layout under each package's `tests/` directory.
Business tests may use local HTTP fixtures, files, and in-process stores, but
process, daemon, service-owner, shutdown, and cross-OS policy coverage belongs
under `tests/os_testing`. Business tests must not require third-party services,
provider tokens, API keys, paid providers, or public live systems. Required
non-network integration tests that should run with
plain `cargo test` live directly under each package's `tests/` directory; do
not create `tests/e2e` directories. Do not keep empty
`business`, `os_testing`, `performance`, `live`, or `benchmark` directories.
Business and OS testing targets may use `helpers/` plus target-owned module
directories beside the top-level `.rs` entrypoint; other crate-owned typed
directories stay flat. Runners should select cases by test type and a one-level
directory scan rather than by hardcoded one-off script paths whenever the
directory layout can express the suite.

Do not write production logic or tests that pass by matching arbitrary
exact-response prompt wording. Avoid assertions that only prove a prompt or
provider description contains a particular sentence. Business assertions must
be based on structured outputs, command exit/result shape, schema enums,
protocol fields, files, or explicit parser contracts.

```powershell
.\xtask\scripts\run-backend-business-tests.ps1 -List
.\xtask\scripts\run-backend-business-tests.ps1 -Crate tools
```

```bash
sh xtask/scripts/run-backend-business-tests.sh --list
sh xtask/scripts/run-backend-business-tests.sh --crate tools
```

The runner completes the discovered business-test set and reports all failed
`package::target` entries together. It does not stop at the first failed target,
but it still stops timed-out process trees before moving on.

Manual backend business script outputs default to:

```text
~/Documents/tura_workspace/target/{test_name}/{run_id}/summary.json
```

Override the artifact root with `TURA_BUSINESS_TARGET_ROOT` or
`COMMAND_RUN_BUSINESS_TARGET_ROOT`.

Long-running comparison and scoring suites belong under `benchmark/`, not
this directory.

Root process tests have moved to `tests/os_testing/` so business coverage can
run in parallel without owning backend daemon state.

## Process And Session Read Coverage

The backend OS testing suite covers these process startup and session read cases:

| Area | Covered cases | OS coverage |
| --- | --- | --- |
| Gateway/router startup | stale `router.addr`, stale `service.addr`, graceful router shutdown followed by gateway restart, same-home gateway contention, foreign port conflict, router crash recovery, orphan router adoption and shutdown | Real host in `tests/os_testing/process_state_management_e2e.rs`; Windows/Linux/macOS/fallback policy rows in `tests/os_testing/process_lifecycle_policy_matrix.rs` |
| Router/session_db lifecycle | crashed session_db restart, unresponsive published session_db endpoint replacement, orphan session_db adoption, router crash leaving session_db alive for the next router, adopted session_db shutdown | Real host in process/session_log OS testing E2E; cross-OS owner/adoption policy rows in the matrix |
| Runtime workers and command runs | router-owned worker reuse/replacement/cleanup, stop-by-key isolation, command_run socket-disconnect cleanup, workspace scan avoidance for session abort/startup cleanup, process-tree strategy contract | Real host for behavior; Windows job object, Linux/macOS process group, and fallback direct-child policy rows in the matrix |
| Session DB reads | workspaces, sessions, get-session, records, pagination, concurrent clients, checkpoint idempotency, deletes, workspace isolation, down-service read errors, queued-write drain, dirty queue quarantine, stale workspace DB/index pruning | Real host through the session_db socket path |
| Gateway session reads | session-log HTTP API, workspace header decoding, in-memory session hydration from session_db, stale cached router endpoint recovery without losing prompt history | Real host through gateway business tests |

Partially covered by design: cross-OS process behavior is policy-matrix covered
on every development machine and fully behavior-tested on the current host.
Actual process primitive behavior on non-current OS families must run on those
OS runners to provide host-native confirmation.
