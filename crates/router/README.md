# Tura Router

Router owns agent registration metadata, CLI forwarding, runtime-worker
dispatch, and worker lifecycle. It does not own `command_run` implementation
logic or command alias canonicalization; both live in `crates/tools`
(`commands::canonical_command`) and execute inside the runtime worker.

This version keeps `command_run` as the only coding-agent visible tool. Internal
command ids such as `shell_command`, `bash`, and `apply_patch` are resolved and
dispatched by `crates/tools/src/commands`, not by the router.

## Layering

- **Gateway** forwards HTTP, persists UI/session state through `session_log`,
  owns provider OAuth credential lifecycle, and launches the router. It runs no
  agent loop and holds no in-process runtime.
- **Router** owns the agent registry, CLI forwarding, and the lifecycle of
  runtime workers. `POST /run_agent` resolves an agent spec, builds the worker
  environment contract, and dispatches a runtime worker subprocess (the gateway
  binary re-invoked with `TURA_ROLE=runtime_worker`). Command alias resolution
  and handler dispatch are owned by `crates/tools`, not the router.
- **Runtime** (`crates/runtime`, package `runtime`) activates
  `AgentManagement`, assembles agent prompts/tools, and runs the MANAS loop. It
  is a library executed inside a runtime worker, never spawned directly by the
  gateway.
- **Provider** OAuth is the sole source of truth for credentials. Workers
  receive provider context through the worker environment contract and must not
  fabricate or bypass missing credentials.

Spawning is single-direction: gateway → router → runtime worker. The only
exception is **multi-agent dispatch from inside a runtime worker**: a worker
may invoke `tura_router run-agent` as a subprocess (CLI stdin/stdout JSON, not
HTTP) to spawn child sub-sessions for concurrent or recursive agent flows.
Internal runtime ↔ router communication is always CLI; URL/HTTP is reserved
for the external gateway boundary.

## Subcommands

| Form | Used by | Channel |
|---|---|---|
| `tura_router` (no subcommand) | Boot — Axum HTTP server on `TURA_ROUTER_PORT` | HTTP |
| `tura_router run-agent` | Runtime worker spawning a child sub-session | stdin/stdout JSON |

Both modes share the same `dispatch_run_agent` core.

## Session Log Bridge

Router also exposes the `session-log` CLI bridge used by runtime and gateway
helpers. It forwards JSON commands to `crates/session_log`; it does not own the
database schema.

Examples:

```powershell
'{"command":"list_workspaces"}' | target\debug\tura_router.exe session-log
'{"command":"list_sessions","workspace":"C:/repo","page":0,"page_size":50}' | target\debug\tura_router.exe session-log
'{"command":"get_session","session_id":"session-id"}' | target\debug\tura_router.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\tura_router.exe session-log
```

Gateway exposes the same bridge as `target\debug\gateway.exe session-log` and
adds HTTP routes under `/session-log/*`.
