# Graphic user interface

The graphical user interface is the Solid/Vite app and Tauri desktop surface. It
uses the gateway SDK and never calls Rust crates, provider code, tools, shell
commands, or session storage directly.

Primary references:

- [apps/gui/README.md](../../apps/gui/README.md)
- [apps/gui/ARCHITECTURE.md](../../apps/gui/ARCHITECTURE.md)
- [apps/tauri/README.md](../../apps/tauri/README.md)

## Role

- Workspace navigation and session management.
- Provider auth and model settings.
- Agent/persona configuration surfaces.
- Rich transcript rendering, files, plans, and project UI.

## Related

- [Gateway](gateway.md)
- [Settings](../start/settings.md)
- [Rich text](../core/rich-text.md)
