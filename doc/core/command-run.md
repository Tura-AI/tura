# Command run

`command_run` is Tura's compact tool surface. A single provider-visible tool can
batch ordered local commands such as shell reads, patches, web discovery, media
inspection, task-state updates, and validation checks.

The full reference is [docs/core/command-run.md](../../docs/core/command-run.md).

## Why it exists

- Fewer provider-visible tools.
- Ordered steps with safe parallelism for independent commands.
- Command-specific timeouts, process cleanup, locks, and output shaping.
- One auditable execution record for each batch.

## Related

- [Commands](commands.md)
- [Task status](task-status.md)
- [Tool architecture](../architecture/tool.md)
- [Custom commands](../customization/custom-commands.md)
