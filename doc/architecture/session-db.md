# Session DB

Session DB is the single SQLite owner for durable Tura session history. It stores
workspace sessions, messages, task state, todos, command checkpoints, and replay
records.

Primary references:

- [crates/session_log/README.md](../../crates/session_log/README.md)
- [crates/session_log/ARCHITECTURE.md](../../crates/session_log/ARCHITECTURE.md)
- [root architecture session-log notes](../../ARCHITECTURE.md#session-log)

## Role

- `tura_session_db` owns SQLite access.
- Gateway, router, and runtime reach session data through the session-log client
  and socket service.
- Workspace logs live under `<workspace>/.tura/session_log.sqlite3`.
- Per-home indexes, locks, queues, and service addresses live under Tura's home
  database directory.

## Related

- [Sessions](../start/sessions.md)
- [Context management](../core/context-management.md)
- [Router](router.md)
