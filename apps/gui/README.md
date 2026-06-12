# Tura GUI

`apps/gui` is the Bun/Solid/Vite graphical client and gateway SDK workspace for
Tura. The app talks to `crates/gateway` through `@tura/gateway-sdk`; it does not
call Rust crates, provider code, tools, shell commands, or session storage
directly.

## Layout

```text
apps/gui/
  ARCHITECTURE.md
  README.md
  package.json
  bun.lock
  turbo.json
  e2e/
    business/
      run_all_release.mjs
      gui_single_request_release.mjs
      gui_snake_release.mjs
      gui_password_zip_release.mjs
  app/
    package.json
    vite.config.ts
    index.html
    src/
  sdk/
    gateway/
      package.json
      src/
```

## Install And Build

From the repository root, `build-release` writes production binaries into
`target/release`; `build-debug` writes local debug binaries into `target/debug`:

```powershell
.\scripts\build-release.ps1; .\scripts\register-cli.ps1          # production (release -> target/release)
.\scripts\build-debug.ps1     # debug build
```

```sh
./scripts/build-release.sh; scripts/register-cli.sh           # production (release -> target/release)
./scripts/build-debug.sh       # debug build
```

The install scripts verify or install Bun where possible, install GUI
dependencies, and build/check the Rust services needed by the gateway.

Manual GUI-only commands:

```text
bun install --cwd apps/gui --frozen-lockfile
bun run --cwd apps/gui build
bun run --cwd apps/gui typecheck
```

## Running Locally

Start the GUI dev server — it starts (and attaches to) its own `tura_gateway` on
port 4126 automatically:

```powershell
.\scripts\start.ps1 -Gui
```

```sh
./scripts/start.sh --gui
```

The GUI Vite dev server binds to `127.0.0.1` and defaults to port `5174` with
`strictPort: false`, so Vite may choose another free port if `5174` is busy.

Direct package command:

```text
bun run --cwd apps/gui dev
```

## Gateway URL Configuration

The SDK resolves the gateway URL in this order:

1. `?gatewayUrl=<url>` query parameter.
2. `localStorage["tura.gatewayUrl"]`.
3. `VITE_TURA_GATEWAY_URL`.
4. `http://127.0.0.1:4126`.

When using the start scripts with `-Gui` or `--gui`, the scripts set
`VITE_TURA_GATEWAY_URL` from `TURA_GATEWAY_URL` when present, otherwise from
the selected `-Port` / `--port` value (default 4126). The dev server starts
`tura_gateway` on that port.

Example with a non-default gateway port:

```powershell
.\scripts\start.ps1 -Gui -Port 4100
```

```sh
./scripts/start.sh --gui --port 4100
```

## Environment And Secrets

Provider secrets belong in the repository-root `.env`, loaded by backend
services through `TURA_ENV_PATH`. GUI code must not read `.env`,
`provider_config.json`, provider logs, `db/session_log`, or `.tura/sessions`
directly.

Runtime choices such as selected model, selected agent, provider auth status,
workspace directory, and session config are read and updated through gateway
HTTP APIs.

## Checks

```text
bun run --cwd apps/gui format:check
bun run --cwd apps/gui typecheck
bun run --cwd apps/gui build
bun run --cwd apps/gui test
```

Focused app checks:

```text
bun run --cwd apps/gui/app typecheck
bun run --cwd apps/gui/app unused:check
```

E2E scripts under `apps/gui/e2e` expect the gateway and any required provider
credentials to be available.

Release-entry live acceptance tests under `apps/gui/e2e/business` start the
release gateway and validate a single real request, Snake, and password-zip CLI
refactor task through the GUI command surface:

```text
bun run --cwd apps/gui e2e:live:release
bun run --cwd apps/gui e2e:live:release:single
bun run --cwd apps/gui e2e:live:release:snake
bun run --cwd apps/gui e2e:live:release:password-zip
```
