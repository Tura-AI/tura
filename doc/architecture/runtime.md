# Runtime

Runtime owns the agent turn loop: session bootstrap, agent activation, prompt
assembly, provider streaming, command callbacks, runtime manuals, context
compaction, checkpoints, and final response shaping.

Primary references:

- [crates/runtime/ARCHITECTURE.md](../../crates/runtime/ARCHITECTURE.md)
- [Runtime prompt](../core/runtime-prompt.md)
- [Dynamic prompt injection](../core/dynamic-prompt-injection.md)

## Role

- Resolves active agent, persona, model, and session state.
- Builds provider messages and tool schemas.
- Runs provider turns and handles tool callbacks.
- Updates session state through task status, context records, and command evidence.

## Related

- [Tool](tool.md)
- [Agents](../core/agents.md)
- [Context management](../core/context-management.md)
