# Tura Tauri Shell

This directory contains only the Tauri desktop shell for the existing GUI.
The web frontend stays in `apps/gui`, and Tauri points at that build output.

Development:

```sh
bun install
bun run dev
```


Release builds are driven from the repository-level `scripts/build-debug.*`
scripts so the generated executable is copied into the shared `bin` layout. The
default `scripts/install.*` (no `dev` argument) invokes the same packaging.
