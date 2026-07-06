# CLI parameters

The full CLI reference is maintained in
[docs/start/cli-parameters.md](../../docs/start/cli-parameters.md).

## Main command surfaces

| Surface | Use it for |
| --- | --- |
| `tura` | Interactive terminal client. |
| `tura run` | Gateway-backed prompt execution. |
| `tura exec` | Direct Rust prompt execution. |
| `tura bash`, `tura zsh`, `tura shell` | Prompt execution with a forced command-run shell. |
| `tura_gateway` | HTTP/SSE gateway and optional web GUI serving. |
| `tura_router` | Router daemon and registry diagnostics. |
| `tura_session_db` | SQLite session-log owner. |

Useful related docs:

- [Command run](../core/command-run.md)
- [Sessions](sessions.md)
- [Terminal user interface](../architecture/terminal-user-interface.md)
- [Gateway](../architecture/gateway.md)
