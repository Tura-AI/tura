# Graphic user interface

The GUI is a Solid/Vite workspace client hosted by Tauri or gateway static
serving. It manages workspace navigation, sessions, settings, files, plans,
provider auth, and visual inspection surfaces.

Primary references:

- [apps/gui/README.md](../../apps/gui/README.md)
- [apps/gui/ARCHITECTURE.md](../../apps/gui/ARCHITECTURE.md)
- [apps/tauri/README.md](../../apps/tauri/README.md)

## Boundary

The GUI talks to gateway APIs. It should not become a second runtime, provider
router, or session database owner.

## Related

- [Gateway](gateway.md)
- [Settings](../start/settings.md)
- [Rich text](../core/rich-text.md)
