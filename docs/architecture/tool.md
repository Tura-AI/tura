# Tool

The tool layer defines `ToolCall`, `ToolPayload`, `ToolContext`, cancellation,
file locks, command routing, command-run batching, and concrete command
implementations.

Primary references:

- [crates/tools/ARCHITECTURE.md](../../crates/tools/ARCHITECTURE.md)
- [Command run](../core/command-run.md)
- [Commands](../core/commands.md)

## Boundary

Tools execute local capabilities under schemas, policies, timeouts, output
shaping, and workspace constraints. Runtime chooses when a tool call is allowed;
tools enforce the local command contract.

## Related

- [Custom commands](../customization/custom-commands.md)
- [Runtime](runtime.md)
- [Router](router.md)
