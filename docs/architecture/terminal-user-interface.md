# Terminal user interface

The TypeScript TUI is the terminal client for sessions, prompt submission,
provider and settings flows, rich/plain rendering, and gateway-backed streaming.

Primary references:

- [apps/tui/README.md](../../apps/tui/README.md)
- [apps/tui/ARCHITECTURE.md](../../apps/tui/ARCHITECTURE.md)
- [CLI parameters](../start/cli-parameters.md)

## Boundary

The TUI is a client. It should not duplicate provider routing, runtime prompt
assembly, durable session storage, or command execution policy.

## Related

- [Gateway](gateway.md)
- [Rich text](../core/rich-text.md)
- [Settings](../start/settings.md)
