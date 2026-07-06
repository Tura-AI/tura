# Dynamic prompt injection

Dynamic prompt injection is Tura's runtime-owned prompt assembly layer. It is not
the security-bug meaning of prompt injection; user text does not become system
policy. Runtime state chooses which prompt fragments are active.

The full reference is [docs/core/prompt-style.md](../../docs/core/prompt-style.md).

## Sources

- active agent and prompt resources;
- active persona and communication style;
- session records and compact context;
- task-status prompt and schema;
- runtime prompt manuals selected by `task_type`;
- tail prompts for compaction, retry, and reflection.

## Related

- [Runtime prompt](runtime-prompt.md)
- [Task status](task-status.md)
- [Context management](context-management.md)
