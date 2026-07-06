# Commands

Commands are Tura-owned local execution units selected inside `command_run`.
Some are internal Rust handlers; others are external command packages launched
through a JSON protocol.

The full reference is [docs/core/commands.md](../../docs/core/commands.md).
Customization instructions are in [Custom commands](../customization/custom-commands.md).

## Command families

| Family | Examples | Owner |
| --- | --- | --- |
| Internal | `shell_command`, `apply_patch`, `task_status`, `planning` | `crates/tools/src/commands` |
| External | `web_discover`, `read_media`, `generate_media` | `commands/<id>` |

## Related

- [Command run](command-run.md)
- [Tool architecture](../architecture/tool.md)
- [Runtime prompt](runtime-prompt.md)
