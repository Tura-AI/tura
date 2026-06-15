# session_log

`crates/session_log` is the durable session/task/message history store for
Tura. Gateway and runtime use it instead of writing workspace-local
`.tura/sessions/*.json` files.

## Storage

`session_log` uses embedded SQLite through the `tura_session_db` service.

```text
<instance-db>/index.sqlite3
<workspace>/.tura/session_log.sqlite3
```

The index database stores workspace/session lookup rows and the durable write
queue. The workspace database stores the full session snapshot and replayable
records. dev and release builds share the same workspace log for a project
because it lives in the workspace `.tura` directory; each `TURA_HOME` still has
its own sockets, locks, and index database.

Pending SQLite write-queue items are replayed on service startup for session
upserts, command checkpoints, and delete operations. Command checkpoint replay
is idempotent by the checkpoint idempotency key.

Runtime and gateway fronts do not open SQLite directly. They probe
`service.addr` with a short timeout; an unreachable endpoint file is removed so
later writes fall back to the file queue immediately instead of blocking on a
stale socket. The file queue recovers orphaned `message_queue/processing/*.json`
items on drain, so writes left behind by a killed owner process are replayed
rather than stranded.

Test and tool environments can isolate the index with `SESSION_LOG_DB_ROOT` or
`TURA_DB_ROOT`. Workspace logs are always placed under the requested workspace
directory.

## Query

Use the gateway bridge when the gateway binary is available:

```powershell
'{"command":"list_workspaces"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_sessions","workspace":"C:/repo","page":0,"page_size":50}' | target\debug\tura_gateway.exe session-log
'{"command":"get_session","session_id":"session-id"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\tura_gateway.exe session-log
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

The index `sessions` table stores lookup metadata and the path of the
workspace database. Reads hydrate the session snapshot from the workspace
database; the index must not be treated as the authoritative lifecycle source.

The workspace `sessions` table stores the full persisted session snapshot:
workspace, parent id, timestamps, status, message count,
`task_management_json`, `management_json`, `session_json`, and `todos_json`.

The workspace `session_records` table stores ordered message/event records for
a session. Records are keyed by `session_id + message_id`; upserts update the
same record idempotently and keep earlier records that are absent from a later
partial write. Use `get_session` for the full snapshot and todos. Use
`list_session_records` for replay/history records.

If a workspace `.tura/session_log.sqlite3` database disappears, the service
treats that workspace database as authoritative and removes stale index
snapshots during reads.

## Provider Logs

Provider call logs are not stored here. They are JSON files written by
`crates/provider` under `log/provider/YYYY-MM-DD/*.json`, or under `LOG_PATH`
when that environment variable is set.

## Checks

```powershell
cargo fmt -p session_log
cargo check -p session_log
cargo test -p session_log
.\scripts\run-backend-os-tests.ps1 -Crate session_log
.\scripts\run-backend-performance-tests.ps1 -Crate session_log
```

The OS runner covers process/service-owner lifecycle tests; the performance
runner covers non-process stress tests.
