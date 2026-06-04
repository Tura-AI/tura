# session_log

`crates/session_log` is the durable session/task/message history store for
Tura. Gateway and runtime use it instead of writing workspace-local
`.tura/sessions/*.json` files.

## Storage

By default, `session_log` starts an embedded PostgreSQL database under:

```text
db/session_log/
```

Environment overrides:

```text
session_log_DATABASE_URL=postgres://...
DATABASE_URL=postgres://...
session_log_POSTGRES_PORT=55432
```

## Query

Use the gateway bridge when the gateway binary is available:

```powershell
'{"command":"list_workspaces"}' | target\debug\gateway.exe session-log
'{"command":"list_sessions","workspace":"C:/repo","page":0,"page_size":50}' | target\debug\gateway.exe session-log
'{"command":"get_session","session_id":"session-id"}' | target\debug\gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\gateway.exe session-log
```

The router bridge accepts the same payloads:

```powershell
'{"command":"get_session","session_id":"session-id"}' | target\debug\tura_router.exe session-log
```

Gateway HTTP exposes:

```text
GET /session-log/workspaces
GET /session-log/sessions?workspace=C%3A%2Frepo&page=0&page_size=50
GET /session-log/{sessionID}/records?page=0&page_size=100
```

## Data Shape

`sessions` stores one row per session with workspace, parent id, timestamps,
status, message count, `task_management_json`, `management_json`,
`session_json`, and `todos_json`.

`session_records` stores ordered message/event records for a session.

Use `get_session` for the full persisted session snapshot and todos. Use
`list_session_records` for replay/history records.

## Provider Logs

Provider call logs are not stored here. They are JSON files written by
`crates/provider` under `log/provider/YYYY-MM-DD/*.json`, or under `LOG_PATH`
when that environment variable is set.

## Checks

```powershell
cargo fmt -p session_log
cargo check -p session_log
cargo test -p session_log
```
