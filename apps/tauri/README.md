# Tura Tauri Shell

This directory contains only the Tauri desktop shell for the existing GUI.
The web frontend stays in `apps/gui`, and Tauri points at that build output.

Development:

```sh
bun install
bun run dev
```

Windows quickstart that always rebuilds the current router/gateway and starts
the latest desktop GUI:

```powershell
.\run-latest-gui.cmd
```

Release builds are driven from the repository-level `scripts/build-bin.*`
scripts so the generated executable is copied into the shared `bin` layout.
