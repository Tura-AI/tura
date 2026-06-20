# session_log Architecture

`crates/session_log` owns durable session history for Tura. It stores session
snapshots, task-management state, todos, parent links, and replayable
message/event records.

Gateway and runtime must go through `runtime::session_log_client`,
`gateway::session_db_client`, or the session-log CLI bridge instead of writing
workspace-local session JSON.

## Layout

```text
crates/session_log/
  Cargo.toml
  README.md
  ARCHITECTURE.md
  src/
    cli.rs
    file_queue.rs
    ipc.rs
    lib.rs
    path.rs
    protocol.rs
    service.rs
    session_state.rs
    store.rs
  tests/
    os_testing/
      router_adopts_live_session_db_flow.rs
      process_management_test.rs
      process_lifecycle_e2e.rs
    performance/
      store_concurrency_test.rs
    store_test.rs
```

## Storage

The store is embedded SQLite and is owned by the `tura_session_db` service.
There is no listener port or external database process.

```text
<tura_path::home_db_dir()>/index.sqlite3
<workspace>/.tura/session_log.sqlite3
```

The per-home index database stores workspace/session lookup rows and the
durable command/write queue. The workspace database stores full session
snapshots and replay records. dev and release builds use the same workspace
database for a project because the log follows the workspace, while each
`TURA_HOME` keeps separate sockets, locks, and index state.

`SESSION_LOG_DB_ROOT` and `TURA_DB_ROOT` are still honored for isolated tests
and local diagnostics. They affect the per-home index only; workspace logs are
written under the workspace `.tura` directory.

### Version Handshake

`ipc.rs` publishes a JSON endpoint record (`service.addr`) carrying the
service's `tura_path::instance_version()`. `call_service` refuses to talk to a
service whose version does not match this build (`ensure_version_compatible`),
implementing the codex-style handshake so a dev client never drives a release
service (or vice versa). Endpoint files without a published version are accepted
only long enough to probe or replace the service record.

### Single Store Owner

Only `tura_session_db` serves the database. Gateway, runtime, router, and CLI
fronts send commands to the socket; if the service is down, async writes are
queued through the file queue and reads fail fast instead of opening the store
inside the front process. This keeps process ownership predictable and avoids
multi-writer startup races.

The service also holds an exclusive per-home owner lock under
`<TURA_HOME>/.tura/locks/session-db-<build_kind>.lock`. A second
`tura_session_db` for the same home must fail before it can replace the
published endpoint.

Clients treat the published `service.addr` as a hint, not as proof that the
owner still exists. `service_is_running` uses a short loopback probe and removes
an unreachable address file; async runtime writes then enqueue locally instead
of paying the full service connection timeout on every checkpoint. The file
queue also moves orphaned `message_queue/processing/*.json` files back to
`pending` at the start of each drain, which recovers writes left behind by a
killed session-db process.

The SQLite `session_write_queue` replays pending write commands on service
startup. It accepts `upsert_session`, `apply_command_checkpoint`,
`delete_session`, and `delete_workspace`; checkpoint replay is idempotent by the
checkpoint idempotency key.

Mandatory crate tests cover the service owner rule directly:
`tests/os_testing/process_lifecycle_e2e.rs` starts a real `tura_session_db`, verifies that a
second owner is rejected, checks bad-input recovery and idempotent upsert, then
performs graceful shutdown and asserts endpoint/lock cleanup.
`tests/os_testing/router_adopts_live_session_db_flow.rs` kills a real router while
leaving session_db alive, verifies queued and direct writes continue, and then
starts a new router that adopts the existing session_db endpoint. Higher-level
workspace process tests live in root `tests/os_testing/process_state_management_e2e.rs`.

## Tables

Index database:

```text
sessions
  session_id primary key
  workspace
  workspace_db_path
  name
  parent_id
  created_at
  updated_at
  state
  status
  message_count

session_write_queue
  id primary key
  idempotency_key
  session_id
  event_type
  payload_json
  status
  retry_count
  timestamps and last_error
```

Workspace database:

```text
sessions
  session_id primary key
  workspace
  name
  parent_id
  created_at
  updated_at
  state
  status
  message_count
  task_management_json
  management_json
  session_json
  todos_json

session_records
  id
  session_id
  message_id
  role
  created_at
  updated_at
  record_json
```

`management_json` is the runtime `SessionManagement` payload used for resume.
`session_json` is the gateway `SessionInfo` snapshot used for hydration.
`todos_json` keeps UI todo projections with the session snapshot.

The workspace `sessions` row is the authoritative snapshot for reads and
resume. The index row locates that workspace database and supports listing, but
`get_session` and `list_sessions` hydrate lifecycle fields and management JSON
from the workspace row so stale index state cannot resurrect an old FSM value.

`session_records` is append/update oriented. Records are uniquely identified by
`session_id + message_id`; an upsert updates an existing record with the same
message id and inserts new records, but it does not delete the whole session's
record history before writing. This keeps earlier replay records available if a
later process crash only flushes a partial turn.

`state` is the only authoritative session lifecycle value. It is the canonical
`session_log::SessionState` enum serialized in snake_case:
`created`, `running`, `paused`, `completed`, `failed`, `cancelled`, and
`interrupted`. `status` is only a derived UI projection:
`idle`, `busy`, or `error`. Store writes must derive `status` from `state`;
callers must not provide a second lifecycle vocabulary.

Startup recovery marks `running` and `paused` sessions as `interrupted` through
the shared state transition rules and updates both `management_json` and
`session_json`. Invalid internal state strings are rejected instead of being
silently coerced or dropped.

If a workspace database is missing, reads remove stale index snapshots and
return only sessions that still have an authoritative workspace log.

## Commands

The protocol is `SessionLogCommand` in `src/protocol.rs`.

```powershell
'{"command":"list_workspaces"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_sessions","workspace":"C:/repo","page":0,"page_size":50}' | target\debug\tura_gateway.exe session-log
'{"command":"get_session","session_id":"session-id"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\tura_gateway.exe session-log
'{"command":"delete_session","session_id":"session-id"}' | target\debug\tura_gateway.exe session-log
'{"command":"delete_workspace","workspace":"C:/repo"}' | target\debug\tura_gateway.exe session-log
```

`list_sessions` returns snapshots for a workspace. `get_session` returns one
snapshot by id. `list_session_records` returns ordered records; page `0` means
the last page for records.

## HTTP Projection

Gateway projects read APIs:

```text
GET /session-log/workspaces
GET /session-log/sessions?workspace=<workspace>&page=0&page_size=50
GET /session-log/{sessionID}/records?page=0&page_size=100
```

Gateway uses the session-db client bridge for full single-session debugging and
runtime resume.

## Provider Logs Boundary

Provider call logs are not session_log rows. They are file diagnostics written
by `crates/provider` under:

```text
log/provider/YYYY-MM-DD/HHMMSS_mmm_<call_id>.json
```

`LOG_PATH` overrides the provider log root. Keep provider request/response
diagnostics in provider logs and session/task/message history in session_log.

## Checks

```powershell
cargo fmt -p session_log
cargo check -p session_log
cargo test -p session_log
```
