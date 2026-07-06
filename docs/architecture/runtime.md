# Runtime

Runtime owns the agent turn loop: session bootstrap, context building, prompt
assembly, provider streaming, tool callbacks, runtime manuals, checkpoints,
compaction, and final response shaping.

Primary references:

- [crates/runtime/ARCHITECTURE.md](../../crates/runtime/ARCHITECTURE.md)
- [Runtime prompt](../core/runtime-prompt.md)
- [Dynamic prompt injection](../core/dynamic-prompt-injection.md)

## Boundary

Runtime decides what the agent sees and how tool calls are handled. It delegates
provider calls to the provider crate, durable records to session DB, and concrete
command execution to tools/router infrastructure.

## Related

- [Tool](tool.md)
- [Agents](../core/agents.md)
- [Context management](../core/context-management.md)
