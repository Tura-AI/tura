# Tura

Tura is a local AI coding system built around a Rust workspace. It provides a
direct Rust CLI, an HTTP/SSE gateway, router-managed background processes,
agent/persona orchestration, provider integration, command execution tools, a
TypeScript terminal client, and a Bun/Solid GUI.

The repository is organized by ownership boundaries: Rust crates own runtime,
gateway, provider, router, tools, agents, and session logging; `apps/tui` and
`apps/gui` provide the interactive clients; `scripts/` owns install, build,
startup, and CLI launcher registration.

## Quick Start

Production install builds release binaries into `bin/` and registers the
`tura-tui` / `tura-gateway` launchers on PATH:

```powershell
.\scripts\install.ps1
```

```bash
./scripts/install.sh
```

Development install builds debug binaries in `target/debug` and keeps frontend
artifacts in their source workspaces:

```powershell
.\scripts\install.ps1 -Dev
```

```bash
./scripts/install.sh dev
```

Run a direct CLI prompt:

```powershell
.\scripts\start.ps1 "Inspect the workspace"
```

```bash
./scripts/start.sh "Inspect the workspace"
```

Start the gateway, TUI, GUI, or desktop shell:

```powershell
.\scripts\start.ps1 -Gateway -Port 4096
.\scripts\start.ps1 -Tui --help
.\scripts\start.ps1 -Gui
.\scripts\start.ps1 -Desktop
```

```bash
./scripts/start.sh --gateway --port 4096
./scripts/start.sh --tui --help
./scripts/start.sh --gui
./scripts/start.sh --desktop
```

Launcher maintenance:

```powershell
.\scripts\register-cli.ps1
.\scripts\unregister-cli.ps1
```

```bash
./scripts/register-cli.sh
./scripts/unregister-cli.sh
```

## Benchmarks

The benchmark suite lives under `tests/business/`. It contains manual,
long-running business benchmarks for frontend repair, ProgramBench-style
cleanroom rebuilds, SWE-bench-style issue patching, source-port rewrites, media
research, and TUI flows.

For development, benchmark and debug flows should use the dev install path and
debug gateway binaries (`target/debug`), not production `bin/` packaging.

```powershell
.\scripts\install.ps1 -Dev
$env:COMMAND_RUN_AGENT_SMOKE_ONLY='1'
node .\tests\business\frontend-playwright\react_ops_board_playwright_repair_lite.mjs
```

```bash
./scripts/install.sh dev
COMMAND_RUN_AGENT_SMOKE_ONLY=1 node ./tests/business/frontend-playwright/react_ops_board_playwright_repair_lite.mjs
```

## More Information

- [Full operational overview](docs/overview.md)
- [Architecture boundaries](ARCHITECTURE.md)
- [Scripts architecture](scripts/ARCHITECTURE.md)
- [Business benchmark guide](tests/business/README.md)
- [Test guide](tests/README.md)
- [TUI guide](apps/tui/README.md)
- [GUI guide](apps/gui/README.md)
- [Project roles](docs/roles.md)
