# Context management

Context management controls how Tura keeps useful session history without
replaying every old token forever.

The detailed reference is [docs/core/context-management.md](../../docs/core/context-management.md).

## Main ideas

- Session records preserve messages, tool evidence, task state, and prompt records.
- Compact-context handoffs summarize what must survive context pressure.
- Active runtime prompt manuals are reinserted after compaction when still active.
- Command output is stored as compact evidence instead of raw noise where possible.

## Related

- [Task status](task-status.md)
- [Runtime prompt](runtime-prompt.md)
- [Session DB](../architecture/session-db.md)
