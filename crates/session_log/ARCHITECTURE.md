# session_log Architecture

`crates/session_log` is Tura's durable memory, not the model's improvised one. It
stores ordered lifecycle/runtime events, management deltas, context facts,
typed command checkpoints, and transactional read projections.

Gateway and runtime must go through `runtime::session_log_client`,
`gateway::session_db_client`, or the session-log CLI bridge instead of writing
workspace-local session JSON. One owner means recovery has one version of the
truth.

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
    service.rs
    store.rs
    store/
      connection.rs
      helpers.rs
      payload.rs
      read.rs
      runtime_events.rs
      session_commands.rs
      write.rs
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

The per-home index database stores only session/runtime lookup rows and typed
command checkpoints. The workspace database stores event and delta facts plus
their transactional read projections. dev and release builds use the same
workspace database for a project because the log follows the workspace, while
each `TURA_HOME` keeps separate sockets, locks, index state, and file queue.

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

The strict file queue is the only deferred-write queue. It accepts the typed
write command set, replays `pending` items, recovers orphaned `processing`
items, and quarantines malformed or rejected payloads in `failed` with an error
sidecar. There is no SQLite write queue. Checkpoint replay is idempotent by its
typed identity; management and context replay use independent sequences.

Socket writes and file-queue writes execute through the same typed command
dispatcher. A successful transaction returns its committed durable Session feed
entries to that dispatcher; the socket path and the service-owned queue drain
publish those entries through the same in-process subscription hub. Assistant
text deltas bypass SQLite and are published through that hub with cursor zero;
Gateway applies them without advancing its durable replay cursor, and the
completed `AgentMessage` replaces the transient projection. Receipt or sequence
replay returns no new committed entries, so recovering a queue file after
commit cannot notify online subscribers twice.

Mandatory crate tests cover the service owner rule directly:
`tests/os_testing/process_lifecycle_e2e.rs` starts a real `tura_session_db`, verifies that a
second owner is rejected, checks bad-input recovery and idempotent delta replay,
then performs graceful shutdown and asserts endpoint/lock cleanup.
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
  updated_at
  last_user_message_at
  state

runtime_locations
  runtime_id primary key
  session_id
  workspace_db_path

command_checkpoints
  idempotency_key
  session_id
  runtime and command identities
  checkpoint_type
  command metadata
  changes_json
  started_at / finished_at / applied_at
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
  next_context_sequence
  retained_from_sequence
  next_management_sequence

session_records
  id
  session_id
  message_id
  role
  created_at
  updated_at
  record_json

session_context_records
  session_id + sequence primary key
  record_json
  projection_json

session_events
  session_id + event_seq primary key
  event_json

session_command_receipts
  command_id primary key
  session_id
  request_json
  result_json

management_deltas
  session_id + sequence primary key
  delta_json
```

`session_events` is the canonical lifecycle history. Reads replay it into a
`SessionAggregate`; there is no aggregate JSON authority. `management_deltas`
stores ordered `SessionManagementDelta` values with a cursor independent from
the context cursor. The `sessions` row is updated in the same transaction and
is only a read projection. Its historical column names are storage details:
`management_json` contains strict typed `SessionManagement`, `session_json`
contains strict typed `SessionMetadata`, and `task_management_json` is a derived
list projection. Full reads pair those values with a `SessionProjection` replayed
from canonical lifecycle events; consumers validate their shared identity,
state, and task plan before restoring the complete parent/runtime index. The
remaining task/todo/state/status/count columns serve queries. The index row only
locates the workspace database and provides a lightweight listing projection.

`session_command_receipts` is an inbox, not lifecycle state. Create and execute
requests carry a stable command id. The owner writes the canonical request and
typed result beside the event and projections in one transaction, returns the
saved result for an identical replay, and rejects key reuse with different
content. A replay also repairs the derived index from the current workspace
projection, covering owner failure after workspace commit and before index sync.
Because the workspace transaction is authoritative, an ordinary command returns
its committed result even if the derived index sync fails; the owner logs that
repairable failure rather than inviting a new logical command id. Creation keeps
the index error visible because a newly committed session is otherwise not
addressable, and its stable `create:{session_id}` receipt makes retry safe.
Workspace deletion removes every indexed workspace database before deleting its
index rows. If file removal fails or the owner exits mid-command, the remaining
index rows preserve the paths needed for an idempotent queue replay.

`session_records` is append/update oriented. Records are uniquely identified by
`session_id + message_id`; a typed delta updates the projection for an existing
message id and inserts new records, but it never deletes records omitted from
that delta. `session_context_records` has stricter identity: replay of a
`session_id + sequence` must match both `record_json` and `projection_json`.
`retained_from_sequence` records the compaction boundary while omitted history
remains available through the UI projection.

Runtime recovery data and frontend visibility are separate contracts. Runtime
checkpoints preserve the complete provider input, including identity, prompt
style, active Operation Manuals, conversation messages, tools, and provider
options. Session context likewise preserves every raw record, but only user and
assistant message records receive a `projection_json`. Internal system records
must not enter `session_records` or the Session feed; Gateway also discards a
system-role feed projection defensively. This lets restart reconstruct the exact
provider boundary without exposing internal prompt context as chat history.

The derived `state` column serializes `lifecycle::SessionState` in snake_case:
`created`, `running`, `paused`, `completed`, `failed`, `cancelled`, or
`interrupted`. `status` is a further UI projection: `idle`, `busy`, or `error`.
Store writes derive both by replaying events; callers do not provide a second
lifecycle vocabulary.

Service startup recovery marks active sessions as `interrupted` through the
shared state transition rules, appends the lifecycle event, and updates the read
projections in the same transaction. Queries never run recovery or mutate
lifecycle state. Invalid internal state strings are rejected instead of being
silently coerced or dropped.

If a workspace database is missing, reads remove stale index snapshots and
return only sessions that still have an authoritative workspace log.

## Commands

The protocol is `SessionLogCommand` in
`crates/session_log_contract/src/protocol.rs`. Lifecycle mutations use
`CreateSession`, `ExecuteSessionCommand`, and `PersistSessionDelta`; the
diagnostic CLI below intentionally exposes only read and administrative
operations.

```powershell
'{"command":"list_workspaces"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_sessions","workspace":"C:/repo","page":0,"page_size":50}' | target\debug\tura_gateway.exe session-log
'{"command":"get_session","session_id":"session-id"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\tura_gateway.exe session-log
'{"command":"delete_session","session_id":"session-id"}' | target\debug\tura_gateway.exe session-log
'{"command":"delete_workspace","workspace":"C:/repo"}' | target\debug\tura_gateway.exe session-log
```

`list_sessions` returns full typed `SessionSnapshot` values for runtime/debug
clients. Each snapshot contains `SessionManagement`, `SessionMetadata`, and the
event-replayed `lifecycle_projection`; it does not expose storage-column JSON as
an independent protocol.
`list_session_summaries` returns lightweight list rows for GUI/sidebar use and
does not include full management or metadata. `get_session` returns one full
typed snapshot by id. `list_session_records` returns ordered records; page `0`
means the last page for records.

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
