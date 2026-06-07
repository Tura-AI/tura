# session_log Architecture

`crates/session_log` owns durable session history for Tura. It stores session
snapshots, task-management state, todos, parent links, and replayable
message/event records.

Gateway and runtime must go through `runtime::session_log_client` or the
session-log CLI bridge instead of writing workspace-local session JSON.

## Layout

```text
crates/session_log/
  Cargo.toml
  README.md
  ARCHITECTURE.md
  src/
    cli.rs
    lib.rs
    local_postgres.rs
    path.rs
    protocol.rs
    store.rs
  tests/
    store_test.rs
```

## Storage

The default local store is embedded PostgreSQL under `db/session_log/`.

Environment overrides:

```text
session_log_DATABASE_URL
DATABASE_URL
session_log_POSTGRES_PORT
```

The database name is `session_log`. The default local port is `55432`.

## Tables

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

`sessions.management_json` is the runtime `SessionManagement` payload used for
runtime resume. `sessions.session_json` is the gateway `SessionInfo` snapshot
used for gateway hydration. `todos_json` keeps UI todo projections with the
session snapshot.

## Commands

The protocol is `SessionLogCommand` in `src/protocol.rs`.

```powershell
'{"command":"list_workspaces"}' | target\debug\gateway.exe session-log
'{"command":"list_sessions","workspace":"C:/repo","page":0,"page_size":50}' | target\debug\gateway.exe session-log
'{"command":"get_session","session_id":"session-id"}' | target\debug\gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\gateway.exe session-log
```

`list_sessions` returns snapshots for a workspace. `get_session` returns one
snapshot by id. `list_session_records` returns ordered records; page `0`
means the last page for records.

## HTTP Projection

Gateway projects read APIs:

```text
GET /session-log/workspaces
GET /session-log/sessions?workspace=<workspace>&page=0&page_size=50
GET /session-log/{sessionID}/records?page=0&page_size=100
```

Gateway currently uses `get_session` through the CLI/client bridge for full
single-session debugging and runtime resume.

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
