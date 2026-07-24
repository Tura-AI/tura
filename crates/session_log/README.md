# session_log

`crates/session_log` is the durable session/task/message history store for Tura.
Gateway and runtime use it instead of writing workspace-local
`.tura/sessions/*.json` files. The distinction matters after a restart, which is
when improvised persistence usually introduces itself.

## Storage

`session_log` uses embedded SQLite through the `tura_session_db` service.

```text
<instance-db>/index.sqlite3
<workspace>/.tura/session_log.sqlite3
```

The index database is deliberately small: it stores session and runtime lookup
rows plus typed `command_checkpoints`. The workspace database stores ordered
session/runtime events, management deltas, context facts, and read projections.
dev and release builds share the same workspace log for a project because it
lives in the workspace `.tura` directory; each `TURA_HOME` still has its own
sockets, locks, index database, and file queue.

There is no SQLite write queue. The strict file queue is the only deferred-write
path. It replays typed commands from `message_queue/pending`, recovers orphaned
`processing` items, and quarantines malformed or rejected commands in `failed`
with an error sidecar. Command checkpoint replay is idempotent by the typed
checkpoint identity; management and context replay use independent sequences.
The socket and queue paths share one typed command dispatcher and one online
Session feed hub. Newly committed stable entries are durable and replayable.
Incremental assistant text deltas use cursor zero and are broadcast live
without a SQLite write; the completed assistant message is durable. Idempotent
command or sequence replay is silent.

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

The workspace `session_events` table is the lifecycle truth and is replayed to
derive the current aggregate. `management_deltas` records ordered typed changes
under an independent management cursor. `session_context_records` preserves
both each raw context fact and its `projection_json`; replay of an existing
sequence must match both identities.

`session_command_receipts` is the typed transport inbox for session creation
and lifecycle commands. A command id may be replayed only with byte-equivalent
canonical request JSON; successful result JSON is stored in the same workspace
transaction as its event and projections. Receipts provide idempotency, not a
second lifecycle authority.
After an ordinary command commits, a derived-index sync failure is logged rather
than returned as a false business failure. Creation still reports that failure;
replaying its stable receipt repairs the missing index without recreating state.

The workspace `sessions` columns (`state`, status, counts, and JSON snapshots)
are transactional read projections, not competing sources of lifecycle truth.
`session_records` is the UI/history projection keyed by
`session_id + message_id`. Use `get_session` for the current read projection,
`read_context_slice` for bounded runtime context and both cursors, and
`list_session_records` for replay/history records.

Runtime checkpoints retain the complete provider input needed for exact
recovery: identity, prompt style, active Operation Manuals, messages, tools, and
provider options. Raw system context remains available through
`read_context_slice`, but it has no frontend message projection and never enters
the durable Session feed. Only user and assistant messages appear in UI/history
records.

If a workspace `.tura/session_log.sqlite3` database disappears, the service
treats that workspace database as authoritative and removes stale index
snapshots during reads.
Workspace deletion does the inverse deliberately: it removes workspace database
files before their index rows, so a failed or interrupted deletion retains the
paths required for safe replay.

## Provider Logs

Provider call logs are not stored here. They are JSON files written by
`crates/provider` under `log/provider/YYYY-MM-DD/*.json`, or under `LOG_PATH`
when that environment variable is set.

## Checks

```powershell
cargo fmt -p session_log
cargo check -p session_log
cargo test -p session_log
.\xtask\scripts\run-backend-os-tests.ps1 -Crate session_log
.\xtask\scripts\run-backend-performance-tests.ps1 -Crate session_log
```

The OS runner covers process/service-owner lifecycle tests. Run the performance
runner only in a controlled, repeatable environment with fixed hardware and an
explicit baseline; GitHub-hosted runners intentionally exclude it.
