# Task status

`task_status` is Tura's structured task-state command. It updates the active
work area, task state, runtime prompt `task_type`, and compact-context handoff.

Read the full reference in [docs/core/task-status.md](../../docs/core/task-status.md).

## Why it matters

- It selects [Runtime prompt](runtime-prompt.md) manuals through `task_type`.
- It records whether work is `doing`, blocked as `question`, or complete as `done`.
- It preserves handoff state for [Context management](context-management.md).
- It is a state update, not a replacement for a user-visible answer.

## Related

- [Command run](command-run.md)
- [Dynamic prompt injection](dynamic-prompt-injection.md)
- [Sessions](../start/sessions.md)
