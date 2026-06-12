# Tura Operational Overview

This document is the short operational map for the current repository. The
source of truth for exact crate boundaries remains the root
`ARCHITECTURE.md`; crate-local details live in each crate's `ARCHITECTURE.md`.

## Runtime Shape

Tura has several fronts and one local backend pipeline:

```text
CLI (`tura exec`)
TUI (`tura` / `tura run`)
GUI (`apps/gui` hosted by Tauri or the dev server)
  -> gateway HTTP/SSE or CLI adapter
  -> detached router daemon
  -> per-home session_db SQLite owner
  -> runtime worker
  -> provider
  -> tools / command_run
```

The backend is scoped by `TURA_HOME`. All sockets, locks, and the private
session_db index derive from that home through `tura_path`. Workspace session
history follows the project and is stored under `<workspace>/.tura`.

## Storage

`crates/session_log` owns durable session/task/message/todo storage. It uses
embedded SQLite with a single owner process:

- per-home index and write queue:
  `<TURA_HOME>/db/session_log/index.sqlite3`
- workspace session log:
  `<workspace>/.tura/session_log.sqlite3`
- IPC endpoint:
  `<TURA_HOME>/.tura/sockets/session-db.sock` or the Windows equivalent

Gateway and runtime clients talk to the owner through `session_log::ipc`. Async
writes can fall back to the durable file queue and are replayed when the owner
starts. Tests should query through session_log APIs instead of reading database
files directly.

## Process Ownership

`tura_router serve-socket` is the long-running daemon for one `TURA_HOME`.
It starts or adopts `tura_session_db`, supervises runtime workers, and exposes
request-id-multiplexed socket IPC. Fronts probe the published endpoint before
starting a new daemon.

Runtime workers are short-lived per-session workers. The router owns worker
spawn, liveness, and cancellation; the runtime owns agent loops, provider calls,
tool routing, checkpoints, and final response shaping.

## Command Entries

Build scripts place binaries directly in Cargo's standard output directories:

```text
target/debug/
target/release/
```

The registered release command directory is `target/release`. The user-facing
entries are:

- `tura exec "prompt"`: Rust one-shot CLI front
- `tura run "prompt"`: TUI gateway client command
- `tura`: interactive TUI
- `tura_gateway`, `tura_router`, `tura_runtime`, `tura_session_db`: services

## Provider Logs

Provider diagnostics are separate from session history. Provider request and
response logs are written under:

```text
log/provider/YYYY-MM-DD/*.json
```

or under `LOG_PATH` when set. Use these files for provider payload and usage
diagnostics; use session_log for session, task, message, checkpoint, and replay
assertions.

## Test Layout

Required local tests:

```powershell
.\xtask\scripts\check-backend-quality.ps1
npm --prefix apps\tui test
bun run --cwd apps\gui test
```

Crate-owned Rust tests are directory classified:

```text
crates/*/tests/business/   required local business and link tests
crates/*/tests/performance/ opt-in performance, stress, load, and soak checks
crates/*/tests/live/       opt-in third-party, public-network, key, or token checks
crates/*/tests/benchmark/  opt-in scoring and comparison checks
tura/tests/business/       required workspace E2E business flows
tura/tests/performance/    opt-in workspace performance/stability checks
tura/tests/live/           opt-in workspace live checks
tura/tests/benchmark/      opt-in scoring, comparison, and long-running benchmarks
```

The four typed directories are peers; do not nest `live`, `performance`, or
`benchmark` under `business`. Create typed directories only when that type has
files. Keep tests directly under the typed directory; do not create category
subdirectories under `business`, `performance`, `live`, or `benchmark` because
runners scan typed directories one level deep. Runners should discover cases by
type and directory scan instead of hardcoding individual script paths. Do not
use fixed response wording as product logic or test oracles; assert structured
command results, protocol fields, parser contracts, files, or stored records.

Business tests may use local processes, local sockets, controlled fixtures, and
workspace files. They must not require third-party services, provider tokens,
API keys, paid providers, or public live systems. Run local business suites
with:

```powershell
.\scripts\run-backend-business-tests.ps1 -Crate tools -TimeoutSeconds 240
```

Live tests may require provider credentials, public network access, model
quota, or third-party systems. Run crate-owned live suites explicitly with:

```powershell
.\scripts\run-backend-live-tests.ps1 -Crate provider -TimeoutSeconds 300
```

Release-entry live scripts validate the built command surfaces and write
summaries under `target/business/{profile}/{surface}/{case}/{run_id}`.

## Build And Acceptance

Debug build:

```powershell
.\scripts\build-debug.ps1
```

Release build and registration:

```powershell
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
```

Snake acceptance is surface-specific:

```powershell
npm --prefix apps\tui run test:business:debug:snake
bun run --cwd apps\gui e2e:business:debug:snake
npm --prefix apps\tui run test:live:release:snake
bun run --cwd apps\gui e2e:live:release:snake
```
