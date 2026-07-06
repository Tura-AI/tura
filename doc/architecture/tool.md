# Tool

The tools crate owns the model-visible tool layer, command-run scheduling,
command handlers, shell execution, patch application, task-status handling,
policy, file locks, cancellation, and output normalization.

Primary references:

- [crates/tools/ARCHITECTURE.md](../../crates/tools/ARCHITECTURE.md)
- [Command run](../core/command-run.md)
- [Commands](../core/commands.md)

## Boundary

Tools own command behavior. Router owns process/service dispatch. Session DB owns
durable history. Provider owns model calls. Keeping those boundaries separate is
why command execution stays auditable instead of becoming one large shell-shaped
blob.

## Related

- [Custom commands](../customization/custom-commands.md)
- [Runtime](runtime.md)
- [Router](router.md)
