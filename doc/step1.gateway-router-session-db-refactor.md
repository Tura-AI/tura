# Gateway, Router, And Session DB Refactor Requirements

Status: proposed  
Scope: step 1 of 3  
Related: [step2.runtime-refactor.md](step2.runtime-refactor.md), [step3.tools-refactor.md](step3.tools-refactor.md), root [ARCHITECTURE.md](../ARCHITECTURE.md)

## Goal

Move execution ownership out of `crates/gateway` and into one persistent router child process. Gateway remains the frontend API surface. Router becomes the execution supervisor and owns runtime/worker lifecycle. The `session_db` service is the only process that connects to local PostgreSQL; router is responsible for its lifecycle (start/health/restart), but router does not sit on the session DB data path. Gateway and runtime call `session_db` directly so neither connects to the SQL binary itself.

Two distinct paths must be kept separate:

- Lifecycle/supervision path: gateway -> router -> starts and supervises `session_db`, runtime workers, and future service workers.
- Data path: gateway and runtime call `session_db` directly over its own IPC endpoint. Router does not proxy, parse, or relay session DB reads/writes.

The new target shape is:

```text
lifecycle/supervision (process tree):
  frontend
    -> gateway HTTP API
        -> router child process, exactly one persistent instance
            -> session_db service child/managed worker
            -> runtime workers, only while sessions are busy
            -> future browser or long-lived service workers

data path (direct calls, not through router):
  gateway  -> session_db -> local PostgreSQL
  runtime  -> session_db -> local PostgreSQL
```

All child processes must be hidden. On Windows, any `Start-Process`, `Command`, or equivalent process launch must avoid visible windows.

## Current Code Reality

Current code differs from the architecture target in several important ways:

- `crates/gateway/src/api/session.rs` currently spawns `gateway.exe` with `TURA_ROLE=runtime_worker` per prompt in `forward_run_agent_to_router`.
- `crates/router` has `ServiceManager` and `WorkerProcess`, but there is no persistent router process owned by gateway today.
- `crates/gateway/src/api/service.rs` reports router as `embedded`, not as a real child process.
- `crates/session_log` owns PostgreSQL storage and local PostgreSQL startup.
- `runtime::session_log_client::SessionLogClient` currently calls `tura_router::session_log_forward` as a library function, not a router process.
- Gateway also persists and hydrates session data through `crates/gateway/src/session/store.rs`.

This refactor must close that gap.

## Required Ownership Boundaries

### Gateway Owns

- HTTP API and frontend compatibility.
- SSE/event fanout to GUI/TUI.
- Auth, config, workspace, project, file browsing, and provider credential UI flows.
- Router process lifecycle: start, health check, restart, shutdown.
- Presentation cache/projection for frontend convenience.
- User prompt entrypoint and user message creation.

Gateway must not own:

- Runtime worker spawning.
- Agent loop execution.
- Tool/command execution truth.
- Runtime busy/idle/error truth.
- Worker process trees below router.

### Router Owns

- Exactly one persistent child process under gateway.
- Execution supervisor API.
- Runtime worker lifecycle.
- Browser or long-lived service worker lifecycle, after a later worker step lands.
- Tool registry and executable resolution for non-core commands.
- Per-session execution concurrency.
- Cancellation and timeout propagation.
- Crash recovery and interrupted-state compensation.
- Session DB service lifecycle and session-log write queue management.

Router is expected to be low-risk and long-lived. It is still allowed to crash; gateway must restart it and router must recover from session DB.

### Session DB Service Owns

- The only direct connection path to local PostgreSQL.
- Durable write queue.
- Queue replay.
- Schema migrations.
- Session-log read and write commands.
- Idempotent checkpoint application.

Gateway and runtime should use a `SessionDbClient` that talks to the `session_db` service directly over its own internal CLI/IPC endpoint. "Router-managed" here means router owns the service lifecycle (start/health/restart); it does not mean calls are routed through router. Router is not on the read/write data path and does not parse or relay session DB payloads. Clients must not directly connect to PostgreSQL or start embedded PostgreSQL — that path belongs only to `session_db`.

### Runtime Worker Owns

- Execution of one active session turn.
- Emitting checkpoints to `SessionDbClient`.
- Provider call and tool result generation.

Runtime worker is not a durable state owner. It exists only while a session is busy or within a short warm-idle window. The default target is to end runtime workers when the session returns to waiting/idle.

## Router Process Contract

Gateway starts one hidden router child process. Router's request surface is execution supervision plus service lifecycle. It must support at least these requests:

```text
health_check
session_db.lifecycle.start
session_db.lifecycle.status
session_db.lifecycle.restart
execution.enqueue_turn
execution.cancel_turn
execution.get_status
execution.kill_session_workers
execution.shutdown
```

Note: there is no `session_db.call` on router. Session DB reads/writes are not router requests; clients call the `session_db` service directly (see "Session DB Service Owns"). Router only starts/supervises that service and reports its health.

The first implementation can use stdin/stdout JSON lines, matching the existing `WorkerProcess` envelope style:

```json
{ "kind": "health_check", "payload": {} }
{ "kind": "call", "payload": { "service": "execution", "method": "enqueue_turn", "input": {} } }
{ "kind": "call", "payload": { "service": "session_db", "method": "lifecycle.status", "input": {} } }
```

Session DB data calls use a separate client/endpoint, not the router envelope above:

```json
{ "kind": "call", "payload": { "service": "session_db", "method": "upsert_session", "input": {} } }
```

This envelope is sent by `SessionDbClient` straight to the `session_db` service, not to router. Future IPC can change, but the logical API and the router-off-the-data-path rule must stay stable.

## Gateway API Migration

Current gateway APIs stay frontend-compatible, but their internals change.

### Session Prompt

Current:

```text
gateway /session/{id}/prompt_async
  -> run_mano_for_prompt
  -> spawn gateway.exe TURA_ROLE=runtime_worker
```

Target:

```text
gateway /session/{id}/prompt_async
  -> create turn request through SessionDbClient
  -> router.execution.enqueue_turn(turn_id, session_id, payload)
```

Gateway should write or enqueue the user message before execution starts, then pass a `turn_id` to router. Router should reject or queue duplicate active turns for the same session.

### Abort

Current gateway-local cancellation sets are not enough.

Target:

```text
gateway /session/{id}/abort
  -> router.execution.cancel_turn(session_id, active_turn_id)
  -> router cancels runtime process tree
  -> runtime/tools cancels any runtime-owned non-core command process if active
  -> router or runtime writes cancelled/interrupted checkpoint
```

### Session Reads

Frontend still calls gateway:

```text
frontend -> gateway /session-log/*
frontend -> gateway /session/*
```

Gateway reads through `SessionDbClient`, not by directly opening SQL. Reads call the `session_db` service directly; they do not pass through router at all (neither router execution logic nor a router proxy). Router only ensures `session_db` is alive. A consequence worth keeping: if router is busy or temporarily down, in-flight reads and writes against a live `session_db` are unaffected — router downtime only blocks scheduling of new turns.

## Session DB Service Requirements

Keep `crates/session_log` as the DB library. Do not move all DB implementation into router.

Target division:

```text
crates/session_log
  src/protocol.rs
  src/store.rs
  src/local_postgres.rs
  src/queue.rs
  src/service.rs
  src/client_protocol.rs

crates/router
  src/services/session_db.rs
```

`crates/session_log` owns DB mechanics. Router owns service lifecycle and IPC exposure.

### Durable Queue

Add a durable write queue managed by session DB. The queue must not be router memory only.

Required table shape:

```text
session_write_queue
  id
  idempotency_key
  session_id
  turn_id
  runtime_worker_id
  command_run_id
  command_id
  event_seq
  event_type
  payload_json
  status
  retry_count
  created_at
  applied_at
  last_error
```

Write requests must be idempotent. Replaying a queue item must not duplicate messages or tool records.

### Checkpoint ACK

Mutating command checkpoints must be acknowledged by session DB before runtime continues. Read-only command checkpoints may be batched, but must be flushed before `command_run_finished`.

If session DB is unavailable:

- Runtime must not continue after a mutating command result.
- Router should mark execution blocked or failed after timeout.
- Gateway can remain alive in read-only/degraded mode.

## Checkpoint Model

Checkpointing must move from turn-end snapshots toward command-level durability.

Required checkpoint types:

```text
turn_started
provider_call_started
command_run_started
command_ready
command_started
command_finished
command_failed
command_run_finished
provider_call_finished
turn_finished
turn_failed
turn_interrupted
```

The minimum safe rule:

```text
Any command that changes external state must write a durable checkpoint after completion.
```

This specifically protects streamed `command_run`: if a command executed while provider output is still streaming, the result must become visible to the next agent turn even if the provider call never finishes cleanly.

## Router Recovery

On router restart:

1. Start `session_db` service.
2. Replay pending `session_write_queue` items.
3. Scan DB for running turns and running commands.
4. Clean up orphan runtime workers.
5. Mark non-reattachable running work as `interrupted`.
6. Resume accepting gateway execution requests.

First phase should not attempt runtime worker reattach. Treat running workers from a crashed router as orphaned and interrupted.

## Runtime Worker Lifecycle

Router enforces:

```text
same session: max 1 active turn
different sessions: bounded parallelism
runtime worker exists only while busy or short warm-idle TTL
idle runtime worker is terminated after TTL
```

Suggested defaults:

```text
max_active_runtime_workers = 8..16
runtime_worker_idle_ttl_secs = 0..300
max_idle_runtime_workers = 0..8
```

For this requested design, default TTL can be zero so runtime exits when session returns to waiting.

## Files To Change

### Gateway

- `crates/gateway/src/api/session.rs`
  - Remove direct runtime worker spawn path.
  - Replace `forward_run_agent_to_router` with router execution client call.
  - Keep frontend API shape.
- `crates/gateway/src/api/service.rs`
  - Report real router health.
  - Include router PID/status/restart state.
- `crates/gateway/src/session/store.rs`
  - Downgrade execution-state writes.
  - Use `SessionDbClient` for persisted reads/writes.
  - Stop being execution truth.
- `crates/gateway/src/bin/gateway.rs`
  - Start router child process on gateway boot.
  - Restart hidden router process when dead.
- New:
  - `crates/gateway/src/router_client.rs`
  - `crates/gateway/src/router_process.rs`
  - `crates/gateway/src/session_db_client.rs`

### Router

- `crates/router/src/main.rs`
  - Add persistent service loop.
  - Keep existing CLI commands as compatibility subcommands.
- `crates/router/src/services/worker_process.rs`
  - Ensure all child processes are hidden.
  - Keep persistent/one-shot worker support for runtime and router-owned services.
- `crates/router/src/services/manager.rs`
  - Extend to runtime and session_db services.
- New:
  - `crates/router/src/services/session_db.rs`
  - `crates/router/src/services/execution.rs`
  - `crates/router/src/services/runtime_workers.rs`
  - `crates/router/src/services/recovery.rs`
  - `crates/router/src/ipc.rs`

Non-core command CLIs from [step3.tools-refactor.md](step3.tools-refactor.md) are not router children in the current tools plan. Router resolves their registry entries and executable paths; runtime/tools launches those hidden processes directly during command_run.

### Session Log

- `crates/session_log/src/store.rs`
  - Add queue-backed checkpoint application.
  - Move large functions into focused modules if needed.
- `crates/session_log/src/local_postgres.rs`
  - Remains DB startup/connection library.
- New:
  - `crates/session_log/src/queue.rs`
  - `crates/session_log/src/service.rs`
  - `crates/session_log/src/checkpoint.rs`
  - `crates/session_log/src/migrations.rs`
  - `crates/session_log/src/client.rs`

## Tests

Required contract tests:

- Gateway does not spawn runtime worker directly.
- Gateway starts one router process and restarts it when dead.
- Router starts session_db service.
- Gateway reads sessions through session_db service.
- Runtime checkpoints through session_db service.
- Router restart replays pending queue.
- Running turn becomes interrupted after router restart.
- Same session concurrent turn is rejected or queued.
- Runtime worker exits after idle/waiting state.

## Non-Goals In Step 1

- Full tools externalization.
- Browser/service worker migration.
- Runtime loop internal cleanup beyond client call changes.
- Multi-router or distributed DB high availability.

## Implementation Standards To Apply During Step 1

These standards apply to existing gateway, router, and session DB code as well as new code.

### File Size And Split Rules

- No source file should exceed 1000 lines after the relevant migration is complete.
- Existing large files must be reduced when touched. Do not add new behavior to an oversized file without extracting a focused module.
- Keep old files as compatibility facades only when necessary.
- Split by responsibility, not by arbitrary line ranges.
- Add module-level comments that state ownership and forbidden responsibilities.

Recommended gateway split:

```text
crates/gateway/src/
  router_process.rs       # start/health/restart hidden router child
  router_client.rs        # execution IPC client
  session_db_client.rs    # direct session_db IPC client
  session/projection.rs   # frontend read cache/projection
  api/session.rs          # HTTP handlers only
  api/service.rs          # status endpoint only
```

Recommended router split:

```text
crates/router/src/
  main.rs
  ipc.rs
  services/execution/
    mod.rs
    protocol.rs
    queue.rs
    cancel.rs
    recovery.rs
    runtime_workers.rs
  services/session_db/
    mod.rs
    protocol.rs
    process.rs
    health.rs
  registry/
```

Recommended session DB split:

```text
crates/session_log/src/
  protocol.rs
  client.rs
  client_protocol.rs
  service.rs
  store.rs
  local_postgres.rs
  migrations.rs
  queue.rs
  checkpoint.rs
  projection.rs
  reads.rs
  errors.rs
```

### Session DB Data Path Rule

Gateway and runtime read/write session DB through `SessionDbClient` to the `session_db` service. They must not:

- Open PostgreSQL directly.
- Start embedded PostgreSQL.
- Call `SessionLogStore::open_default` from gateway/runtime business logic.
- Route session DB reads through router execution APIs.

Router owns `session_db` lifecycle. Router does not parse or relay normal session DB read/write payloads except for starting, health-checking, and restarting the service.

### IPC Request Shape

All gateway/router/session_db IPC messages should use typed structs with this logical shape:

```text
request_id
kind
method
payload
deadline_ms
```

Responses should include:

```text
request_id
ok
payload
error
```

Every child process must be hidden. Stderr/stdout must be captured to logs or structured events.

### Durable Queue Rule

The session DB write queue must be durable. It must not live only in router memory or runtime memory.

Queue records must include:

```text
idempotency_key
session_id
turn_id
runtime_worker_id
command_run_id
command_id
event_seq
event_type
payload_json
status
retry_count
last_error
```

Queue replay must run before router reports execution-ready after restart.

### Prompt Submission Ordering

Gateway prompt handling must follow:

```text
1. validate request
2. write user message / turn_requested through SessionDbClient
3. wait for session_db ACK
4. call router.execution.enqueue_turn(turn_id)
5. show queued/running/error based on router response
```

If router enqueue fails after the user message is durable, gateway should keep the message visible and surface a retryable execution error.

### Checkpoint Payload Requirements

Command-level checkpoints must preserve enough information for the next runtime turn to reconstruct executed tool results:

```text
session_id
turn_id
runtime_worker_id
provider_call_id
command_run_id
command_id
command_type
command_line or normalized arguments
status
stdout/stderr/output summary
changes
started_at
finished_at
```

This is mandatory for streamed `command_run`, especially when a command completes before the provider call finishes.
