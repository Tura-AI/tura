# Runtime prompt

Runtime prompts are Tura-owned operation manuals selected by `task_type`. They
load task-specific discipline and optional command capabilities only when the
current task needs them.

The full reference is [docs/core/runtime-prompt.md](../../docs/core/runtime-prompt.md).
Customization instructions are in [Custom runtime prompt](../customization/custom-runtime-prompt.md).

## Mechanism

1. `task_status.task_type` selects one or more manual ids.
2. Runtime normalizes ids and expands parent manuals.
3. Manual text is appended as session prompt records.
4. Manual capabilities extend the allowed `command_run` command set.
5. Compaction re-adds active manuals when needed.

## Related

- [Task status](task-status.md)
- [Command run](command-run.md)
- [Dynamic prompt injection](dynamic-prompt-injection.md)
