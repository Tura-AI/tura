# Tura Tauri Shell

This directory contains only the Tauri desktop shell for the existing GUI. The
web frontend stays in `apps/gui`; Tauri points at that build output instead of
growing a second frontend. The desktop process and packaged binary are named
`tura_gui`.

Development:

```sh
bun install
bun run dev
```

Tests:

```sh
bun run test:unit
```

The unit tests cover gateway startup helpers, endpoint parsing, health probing,
and runtime-root detection for the desktop shell.


Release builds are driven from the repository-level `scripts/build-release.*`
scripts. A default release build now builds the web GUI first and then runs the
Tauri bundle build from this workspace. Use `-BackendOnly` / `--backend-only` on
the root release script only when you intentionally want Rust backend artifacts
without GUI or desktop bundles.

Before invoking Tauri, `bun run build` stages the release gateway, router,
runtime, session database, provider configuration, agents, personas, and command
metadata under `src-tauri/resources/tura-runtime`. Tauri copies that directory
into the native application bundle. The staging directory is generated and is
not committed.

On macOS, a packaged application resolves those files from
`Tura.app/Contents/Resources/tura-runtime` and defaults writable state to
`~/Library/Application Support/Tura`. An explicit `TURA_HOME` still wins.
