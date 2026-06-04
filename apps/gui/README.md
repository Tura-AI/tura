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

From the repository root:

```powershell
.\scripts\install.ps1
```

```sh
./scripts/install.sh
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

Start the gateway server in one terminal:

```powershell
.\scripts\start.ps1 -Gateway -Port 4096
```

```sh
./scripts/start.sh --gateway --port 4096
```

Start the GUI dev server in another terminal:

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
4. `http://127.0.0.1:4096`.

When using the start scripts with `-Gui` or `--gui`, the scripts set
`VITE_TURA_GATEWAY_URL` from `TURA_GATEWAY_URL` when present, otherwise from
the selected `-Port` / `--port` value.

Example with a non-default gateway port:

```powershell
.\scripts\start.ps1 -Gateway -Port 4100
.\scripts\start.ps1 -Gui -Port 4100
```

```sh
./scripts/start.sh --gateway --port 4100
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
