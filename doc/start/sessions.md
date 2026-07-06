# Sessions

Sessions are durable workspace-scoped conversations with messages, task state,
todos, compact-context handoffs, and replayable command records.

The detailed user-facing reference is [docs/start/sessions.md](../../docs/start/sessions.md).
The storage architecture is [Session DB](../architecture/session-db.md).

## What a session keeps

- user and assistant messages;
- selected agent, model, persona, and workspace config;
- `task_status` state and active `task_type` manuals;
- command-run checkpoints and summarized evidence;
- compact context records for long tasks.

## Related pages

- [Task status](../core/task-status.md)
- [Context management](../core/context-management.md)
- [Runtime](../architecture/runtime.md)
- [Gateway](../architecture/gateway.md)
