# Gateway

Gateway is the HTTP/SSE front used by Tura clients. It translates TUI/GUI
requests into provider, router, runtime, session, config, registry, and file
operations, then streams events back to clients.

Primary references:

- [crates/gateway/ARCHITECTURE.md](../../crates/gateway/ARCHITECTURE.md)
- [apps/gui/docs/gateway-adjustments.md](../../apps/gui/docs/gateway-adjustments.md)
- [CLI parameters](../start/cli-parameters.md)

## Boundary

Gateway does not own the agent loop, provider logic, command execution, or
SQLite. Those belong to [Runtime](runtime.md), providers, [Tool](tool.md), and
[Session DB](session-db.md). Gateway is the API and event surface.

## Related

- [Graphic user interface](graphic-user-interface.md)
- [Terminal user interface](terminal-user-interface.md)
- [Router](router.md)
