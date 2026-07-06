# Terminal user interface

The terminal user interface is the TypeScript CLI/TUI front end. It talks to the
gateway over HTTP/SSE and keeps runtime, provider, tool, and session-storage
logic in the backend.

Primary references:

- [apps/tui/README.md](../../apps/tui/README.md)
- [apps/tui/ARCHITECTURE.md](../../apps/tui/ARCHITECTURE.md)
- [CLI parameters](../start/cli-parameters.md)

## Role

- Interactive terminal conversations.
- Non-interactive `tura run` gateway-backed prompts.
- Provider/model/auth/session commands.
- Plain, ANSI, and richer terminal rendering modes.

## Related

- [Gateway](gateway.md)
- [Rich text](../core/rich-text.md)
- [Settings](../start/settings.md)
