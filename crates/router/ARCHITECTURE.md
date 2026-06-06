# Router Crate Architecture

`crates/router` owns CLI forwarding, agent registration metadata,
runtime-worker dispatch, and worker lifecycle. It does not own command
implementation, command alias canonicalization, agent loop logic, or port
allocation.

The Cargo package and default binary name should stay compatible with Tura:

```text
package = tura_router
default binary = tura_router
```

## Layout

```text
crates/router/
  Cargo.toml
  README.md
  ARCHITECTURE.md
  src/
    main.rs
    registry.rs
    registry/
      agent.rs
    services.rs
    services/
      managed_process.rs
      manager.rs
      models.rs
      rust_service.rs
      worker_process.rs
    utils/
      cli.rs
      port.rs
      process.rs
```

The current implementation is concentrated in the Axum entrypoint plus the agent
registry, service manager, and utility modules above.

## Responsibilities

Router owns:

- Agent registry (agent spec resolution metadata).
- CLI forwarding rules (`/run_tool`: resolve a tool binary and forward stdio).
- Runtime-worker dispatch (`POST /run_agent`): agent resolution, worker
  environment contract assembly, and worker subprocess lifecycle.
- Concurrency guards for runtime workers (depth and active-worker limits).
- Worker status monitoring (`/services/status`).
- Health checks that do not depend on port allocation.

Router does not own:

- Agent loops.
- Prompt assembly.
- Provider request formatting.
- Provider credentials (owned by gateway OAuth).
- Command handler logic.
- Command alias canonicalization (owned by `crates/tools`).
- Shell execution.
- File locks.
- Port allocation.

## Runtime Worker Dispatch

`POST /run_agent` is the HTTP entrypoint for running an agent turn (used by the
gateway boundary). The CLI subcommand `tura_router run-agent` is the
**internal** entrypoint used by a running runtime worker that wants to spawn a
child sub-session — it reads a `RunAgentRequest` JSON from stdin and writes the
result JSON to stdout. Both entrypoints share the same core
`dispatch_run_agent(...)` implementation, so behavior is identical.

The router:

1. Resolves the agent spec from the agent registry.
2. Resolves the gateway binary target (the runtime worker is the gateway binary
   re-invoked with `TURA_ROLE=runtime_worker`).
3. Builds the worker environment contract: `TURA_ROLE`, `TURA_GATEWAY_URL`,
   `TURA_SESSION_MODEL_OVERRIDE`, `TURA_PARENT_SESSION_ID`,
   `TURA_PLANNING_DEPTH`, plus any caller-supplied session-config env
   merged from the request `worker_env`.
4. Enforces concurrency guards before dispatch: child depth must not exceed
   `MAX_PLANNING_DEPTH`, and active runtime workers must not exceed
   `MAX_RUNTIME_WORKERS`. Either breach returns `429 Too Many Requests`.
5. Ensures the worker is live and forwards the call over the worker NDJSON
   protocol.

The worker owns its own session state and reports progress back to the gateway
through callbacks; the router does not replay or merge agent state.

## Session DB Data Path

Router starts and supervises the `session_db` service, but normal session reads
and writes are outside router IPC. Gateway and runtime helpers use their direct
session DB clients, which invoke `tura_router session-db-call`; the persistent
router process only accepts lifecycle methods such as
`session_db.lifecycle.status` and execution methods such as
`execution.enqueue_turn`.

### Internal-only CLI channel (runtime ↔ router)

When a runtime worker needs to spawn a child sub-session (concurrent or
recursive multi-agent dispatch), it invokes `tura_router run-agent` as a
**subprocess** (stdin/stdout JSON). It does **not** call the router over
HTTP/URL. This rule is enforced repo-wide:

- Internal runtime ↔ router communication is **always CLI** (subprocess +
  stdin/stdout NDJSON), never URL/HTTP.
- All runtimes are subprocesses; the router process never embeds a runtime in
  the same address space.
- The HTTP `POST /run_agent` route remains only for the external gateway
  boundary; child sub-session dispatch from a runtime never goes through it.

The runtime resolves the router binary in this order: `TURA_ROUTER_BIN` env,
`current_exe()` sibling, then repo `target/{release,debug}/tura_router`.

## Agent Registration

The agent registry (`registry/agent.rs`) resolves an agent spec from a static,
in-memory table keyed by agent name and session type. `POST /run_agent` consults
it to pick the agent that the dispatched runtime worker activates. It introduces
no database and no fixed port.

Command alias canonicalization and handler dispatch are **not** owned here. They
live in `crates/tools` (`commands::canonical_command` plus the
`crates/tools/src/commands/<command>` handlers) and run inside the runtime
worker. The router does not keep a parallel command table.

## Worker Lifecycle

Router owns the lifecycle of runtime workers through `ServiceManager`
(`services/manager.rs`) and `WorkerProcess` (`services/worker_process.rs`). A
worker is the gateway binary re-invoked with `TURA_ROLE=runtime_worker`; it
speaks a line-delimited NDJSON protocol over stdio and must not require a fixed
port.

Lifecycle records include:

- service id / worker id
- executable target
- startup args and environment contract
- readiness (`health_check`) and invocation (`call`) over NDJSON
- persistent vs. one-shot mode (one-shot falls back when persistent spawn fails)
- status surfaced through `/services/status`

## CLI Forwarding

`POST /run_tool` resolves a tool binary under `target/{release,debug}` and
forwards JSON input over stdio. The owning crate or
`crates/tools/src/commands/<command>` owns behavior; the router only resolves and
forwards.

## Checks

Use:

```text
cargo fmt -p tura_router
cargo check -p tura_router
```
