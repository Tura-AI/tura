# Development architecture

Development architecture is owned by source modules first and summarized here for
navigation.

The whole-project owner reference is [ARCHITECTURE.md](../../ARCHITECTURE.md).

## Ownership map

| Area | Owner reference |
| --- | --- |
| Session DB | [docs/architecture/session-db.md](../architecture/session-db.md) |
| Gateway | [crates/gateway/ARCHITECTURE.md](../../crates/gateway/ARCHITECTURE.md) |
| Router | [crates/router/ARCHITECTURE.md](../../crates/router/ARCHITECTURE.md) |
| Runtime | [crates/runtime/ARCHITECTURE.md](../../crates/runtime/ARCHITECTURE.md) |
| Tools | [crates/tools/ARCHITECTURE.md](../../crates/tools/ARCHITECTURE.md) |
| TUI | [apps/tui/ARCHITECTURE.md](../../apps/tui/ARCHITECTURE.md) |
| GUI | [apps/gui/ARCHITECTURE.md](../../apps/gui/ARCHITECTURE.md) |
| Scripts | [scripts/ARCHITECTURE.md](../../scripts/ARCHITECTURE.md) |

## Rules

- Keep behavior docs near the source owner, then link from this GitBook tree.
- Update docs when changing setup, command behavior, architecture, or package
  layout.
- Prefer focused subsystem tests before broad release checks.

## Related

- [Architecture overview](../architecture/session-db.md)
- [Scripts](scripts.md)
- [Testing](testing.md)
- [Environment](environment.md)
