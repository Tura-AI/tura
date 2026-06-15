# Tests

Test scripts are split by runtime cost and blast radius.

## Required Workspace OS Tests

Root integration tests owned by the `tura_workspace` package are mandatory
backend checks. Process, daemon, socket-owner, shutdown, and cross-OS policy
tests live directly under `tests/os_testing/` and run serially through the OS
test runner. `business`, `os_testing`, `performance`, `live`, `release`, and
`benchmark` are peer directories, and typed directories are scanned one level
deep.

`tests/os_testing/process_state_management_e2e.rs` starts the real debug `tura_gateway`,
`tura_router serve-socket`, and `tura_session_db` binaries under isolated
`TURA_HOME` directories.

It covers the process/state cases that must never regress:

- stale `router.addr` and `service.addr` files are probed, removed, and replaced;
- a gateway can restart the router/session_db pair for the same home after a
  graceful router shutdown;
- a second gateway for the same `TURA_HOME` is rejected by the ownership lock;
- a gateway fails cleanly when its requested port is owned by a foreign process,
  without starting backend daemons or leaking its owner lock;
- gateway status restarts a crashed detached router and the restarted router
  adopts the still-running orphan session_db;
- router health restarts a crashed session_db and publishes a fresh endpoint;
- an already-running orphan `tura_session_db` is adopted by router and stopped
  by router shutdown;
- an already-running orphan router is adopted by gateway and stopped by gateway
  cleanup;
- router-owned `command_run` subprocesses stop when the runtime/router socket
  that requested them disconnects;
- a GUI/TUI-style gateway stdin EOF exits gateway without directly killing
  router; router then self-shuts down after its gateway lease expires and the
  idle grace elapses;
- router and session_db endpoint files are removed and unreachable after
  cleanup.

`tests/os_testing/process_lifecycle_policy_matrix.rs` is the cross-OS lifecycle
contract. It simulates Windows, Linux, macOS, and fallback OS families for the
gateway front, router daemon, session_db owner, runtime worker, and command_run
roles. It pins which roles are reusable owners, which roles must clean whole
process trees, and where macOS requires explicit router-owned cleanup because
it has no Linux-style parent-death signal.

Run it directly with:

```powershell
cargo test -p tura_workspace --features os-tests --test process_state_management_e2e -- --nocapture
```

```bash
cargo test -p tura_workspace --features os-tests --test process_state_management_e2e -- --nocapture
```

`tests/os_testing/session_db_workspace_flow_e2e.rs` starts a real in-process session_db
socket owner and exercises the IPC path with concurrent short-lived clients
across two workspaces. It verifies workspace summaries, session pagination,
message record preservation, checkpoint ACK idempotency in the global index DB,
workspace `.tura` storage, delete isolation, and graceful endpoint cleanup.

`crates/session_log/tests/os_testing/router_adopts_live_session_db_flow.rs`
starts a real router/session_db pair, kills the router, verifies the still-live
session_db continues to drain queued writes and serve direct socket writes, then
starts a new router for the same home and verifies it adopts the existing
session_db endpoint.

Run it directly with:

```powershell
cargo test -p tura_workspace --features os-tests --test session_db_workspace_flow_e2e -- --nocapture
```

```bash
cargo test -p tura_workspace --features os-tests --test session_db_workspace_flow_e2e -- --nocapture
```

The GitHub CI crate matrix includes `tura_workspace`, so root package tests are
part of the required crate checks rather than an optional performance or live
suite.

The workspace root also declares backend `default-members`, but CI does not use
a single workspace cargo test as its main backend check. `business`,
`os_testing`, `performance`, `live`, `release`, and `benchmark` are peer test
types at the workspace root; crate-owned typed tests use the backend package
directories. None of these types owns or nests the others. Backend quality
checks enforce this layout through `tests/business`, `tests/os_testing`,
`tests/performance`, `tests/live`, `tests/release`, and `tests/benchmark`.
Do not run OS testing coverage with a single parallel workspace cargo command:
process-owning tests share global env, local sockets, owner locks, and
child-process cleanup, so the backend OS runner serializes every typed target
with `--test-threads=1`.
Do not create an empty typed directory; add a typed directory only when files in
that type exist. Except for special hand-authored harness entrypoints, runners
and docs should refer to the test type plus directory scan, not hardcoded
individual script paths.
Any Rust test under a crate-owned typed directory must be declared as a
`[[test]]` target with the matching feature gate. Do not create `tests/e2e`
directories.

## Crate-Owned Contract Scripts

Focused command-run CLI flows now live with the gateway crate under
`crates/gateway/tests/command-run/`. Cross-crate command contracts that
aggregate Cargo tests live under `crates/tools/tests/contracts/`. The TUI
gateway CLI fixture lives under the app-owned `apps/tui/tests/e2e/` suite.

Multi-agent dispatch (router-CLI subprocess + concurrent + 2-level recursive
sub-sessions) is covered by `crates/runtime/tests/child_dispatch_test.rs`
against an in-package mock router binary (`mock_router_for_test`). It
verifies the runtime never opens a URL/HTTP channel to the router — all
internal runtime ↔ router traffic is CLI stdin/stdout JSON.

## Business Tests

`tests/business/` contains required local business validation scripts. They may
use local fixtures, local HTTP servers, files, and in-process stores, but
process, daemon, socket-owner, shutdown, and OS policy coverage belongs in
`tests/os_testing/`. Business tests must not require third-party services,
provider tokens, API keys, paid providers, or public live systems.

Crate-owned Rust business tests live under backend package
`tests/business/` directories. The runner scans `crates/`, `commands/`,
`agents/`, and `personas/`. They cover business workflows and local link flows
without third-party services, provider tokens, API keys, paid providers, or
public live systems.

The typed directories are peers, not nested suites:

```text
<backend-package>/tests/business/           required local business/link flows
<backend-package>/tests/os_testing/         process, daemon, owner, and OS policy flows
<backend-package>/tests/performance/        performance, stress, load, and soak tests
<backend-package>/tests/live/               third-party or live-network tests
<backend-package>/tests/benchmark/          scoring and comparison tests
```

Business and OS testing targets may use `helpers/` plus target-owned module
directories beside the top-level `.rs` entrypoint. Performance, live, release,
and benchmark crate-owned typed directories stay flat. Runners select tests by
type and a one-level directory scan, so do not add one-off script-path
references when a typed directory scan can discover the case. Do not keep empty
typed directories.

Tests and production logic must not special-case prompts or model text with
fixed exact-response wording. If a flow needs a machine-readable assertion, use
a structured fixture, command result, protocol field, file artifact, or
parser-owned output contract instead of matching user or model prose.

```powershell
.\scripts\run-backend-business-tests.ps1 -List
.\scripts\run-backend-business-tests.ps1 -Crate tools
```

```bash
sh scripts/run-backend-business-tests.sh --list
sh scripts/run-backend-business-tests.sh --crate tools
```

Business-test outputs default to
`~/Documents/tura_workspace/target/{test_name}/{run_id}/summary.json` for
manual backend business scripts.
Override the artifact root with `TURA_BUSINESS_TARGET_ROOT` or
`COMMAND_RUN_BUSINESS_TARGET_ROOT`.

See `tests/business/README.md` for command examples and output schema.

## OS Testing

`tests/os_testing/` contains process/OS-sensitive local validation: backend
owners, router/session_db adoption, worker lifecycle, command-run process tree
cleanup, task scheduler service state, and cross-OS lifecycle policy. These
targets are gated by `os-tests` and are excluded from default workspace cargo
runs.

Run OS testing after business coverage:

```powershell
.\scripts\run-backend-os-tests.ps1 -List
.\scripts\run-backend-os-tests.ps1
```

```bash
sh scripts/run-backend-os-tests.sh --list
sh scripts/run-backend-os-tests.sh
```

The backend business runners deliberately ignore root `.mjs` files and never
run app-owned TUI/GUI/browser scripts. TUI scripts live under
`apps/tui/tests/e2e/` and GUI scripts live under `apps/gui/e2e/`; run those
suites explicitly from their app package scripts.

## Live Tests

`tests/live/` is the workspace peer for tests that require public network
access, third-party systems, provider tokens, API keys, paid providers, or other
external state. Crate-owned live tests use the same peer layout under
`<backend-package>/tests/live/`.

Live tests are opt-in and must not be nested under `business`, `performance`, or
`benchmark`. When a live runner is needed, it should scan by the `live` test
type and directory instead of naming individual scripts, unless the case is a
special external harness that cannot be discovered generically.

Backend live tests are selected by direct Rust scans and backend-owned root
live scripts. App-owned TUI/GUI live scripts are not part of backend live
runners; run them through the app package commands.

```powershell
.\scripts\run-backend-live-tests.ps1 -List
.\scripts\run-backend-live-tests.ps1 -Crate provider -TimeoutSeconds 300
```

```bash
sh scripts/run-backend-live-tests.sh --list
sh scripts/run-backend-live-tests.sh --crate provider --timeout-seconds 300
```

## Release Tests

`tests/release/` contains tests that validate built release binaries themselves.
These are separated from business and live scans so ordinary test runs do not
start or clean up release daemons.

Release tests are selected by direct `tests/release/*.mjs` scans:

```powershell
.\scripts\run-backend-release-tests.ps1 -List
.\scripts\run-backend-release-tests.ps1 -TimeoutSeconds 600
```

```bash
sh scripts/run-backend-release-tests.sh --list
sh scripts/run-backend-release-tests.sh --timeout-seconds 600
```

Release-entry scripts validate the built command surfaces with real model
execution and write summaries under:

```text
target/business/{profile}/{surface}/{case}/{run_id}/summary.json
```

See `tests/release/README.md` for release-entry script commands.

## Backend Performance Tests

Rust compatibility, in-process concurrency, stress, and stability tests live
under backend package `tests/performance/` directories. Process/lifecycle,
router, and session_db IPC stress belongs in `tests/os_testing/` instead so it
runs serially. The performance runner scans `crates/`, `commands/`, `agents/`,
and `personas/`; these tests are excluded from default `cargo test`.

```powershell
.\scripts\run-backend-performance-tests.ps1 -List
.\scripts\run-backend-performance-tests.ps1 -Crate session_log
.\scripts\run-backend-performance-tests.ps1 -Crate gateway -TimeoutSeconds 240
```

```bash
sh scripts/run-backend-performance-tests.sh --list
sh scripts/run-backend-performance-tests.sh --crate session_log
sh scripts/run-backend-performance-tests.sh --crate gateway --timeout-seconds 240
```

## Benchmarks

`tests/benchmark/` contains manual comparison, scoring, and long-running repair
suites. GitHub CI must not execute scripts from this directory or read them as
test fixtures. Crate-owned tests should live under the owning crate, for example
`crates/*/tests/`.

See `tests/benchmark/README.md` for the benchmark entry list and contract.

## Inspecting Logs In Tests

Tests, business scripts, and benchmark scripts should query Tura session history through
`session_log`, not by reading `.tura/sessions/*.json`.

```powershell
'{"command":"get_session","session_id":"session-id"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\tura_gateway.exe session-log
```

Provider-call diagnostics are separate files under
`log/provider/YYYY-MM-DD/*.json` by default, or under `LOG_PATH` when that
environment variable is set. Use provider logs for model request/response
debugging and session_log for session/task/message assertions.
