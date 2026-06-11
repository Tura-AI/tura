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

Install user-local dependency tools and package-owned dependencies first. This
does not build binaries: it installs `uv`, `bun`, command-local Python `.venv`
directories, and Bun workspaces in place.

```powershell
.\scripts\install.ps1
```

```bash
./scripts/install.sh
```

On macOS, the POSIX install/start scripts assert that zsh is available for the
default shell surface. `scripts/register-cli.sh` updates `.zprofile` and
`.zshrc` when needed so new Terminal sessions can find the release binaries.

Production build writes release binaries into Cargo's standard
`target/release` directory and registers that directory on PATH. The registered
CLI command is `tura exec`; the TUI entry remains `tura`:

```powershell
.\scripts\build-release.ps1; .\scripts\register-cli.ps1
```

```bash
./scripts/build-release.sh; scripts/register-cli.sh
```

Development build writes debug binaries into `target/debug` and keeps frontend
artifacts in their source workspaces:

```powershell
.\scripts\build-debug.ps1
```

```bash
./scripts/build-debug.sh
```

Run a direct CLI prompt:

```powershell
.\scripts\start.ps1 "Inspect the workspace"
```

```bash
./scripts/start.sh "Inspect the workspace"
```

Start the TUI, GUI, or desktop shell (each auto-starts/attaches to its own
`tura_gateway` on port 4126):

```powershell
.\scripts\start.ps1 -Tui --help
.\scripts\start.ps1 -Gui
.\scripts\start.ps1 -Desktop
```

```bash
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

NPM package checks:

```bash
npm run install:deps -- --check-only
npm run pack:check
```

Script install/release checks:

```powershell
.\scripts\tests\scripts\test-install.ps1
.\scripts\tests\scripts\test-build-release.ps1 -SkipTui -ReleaseProbe release-v0.0.0-ci
```

```bash
sh scripts/tests/scripts/test-install.sh
sh scripts/tests/scripts/test-build-release.sh --skip-tui --release-probe release-v0.0.0-ci
```

## Business Tests And Benchmarks

Business release-entry tests live under `tests/business/` and exercise the
registered release command surfaces. The CLI scripts are under
`tests/business/release-entry/`; TUI and GUI release-entry scripts live with
their app-owned E2E suites under `apps/tui/e2e/business/` and
`apps/gui/e2e/business/`.

Backend Rust tests that need provider credentials, external network access, or
long business flows live under `crates/*/tests/business/` and run only through
`scripts/run-backend-business-tests.*`; their suite is the first directory under
`tests/business` (`flow`, `live`, or `long-e2e`). Backend compatibility,
concurrency, stress, and stability tests live under
`crates/*/tests/performance/` and run through
`scripts/run-backend-performance-tests.*`. CI and default `cargo test
--workspace --exclude src-tauri` run only unit tests, default crate tests, and
non-foldered root flow tests.

Benchmark and comparison suites live under `tests/benchmark/`. They are manual,
long-running flows for frontend repair, ProgramBench-style cleanroom rebuilds,
SWE-bench-style issue patching, and source-port rewrites.

Run the release-entry business suite after `build-release` and registration:

```powershell
.\scripts\build-release.ps1; .\scripts\register-cli.ps1
node .\tests\business\release-entry\run_all_cli_release.mjs
npm --prefix apps\tui run test:business:release
bun run --cwd apps\gui e2e:business:release
```

```bash
./scripts/build-release.sh; scripts/register-cli.sh
node ./tests/business/release-entry/run_all_cli_release.mjs
npm --prefix apps/tui run test:business:release
bun run --cwd apps/gui e2e:business:release
```

## More Information

- [Full operational overview](docs/overview.md)
- [Architecture boundaries](ARCHITECTURE.md)
- [Scripts architecture](scripts/ARCHITECTURE.md)
- [Business test guide](tests/business/README.md)
- [Benchmark guide](tests/benchmark/README.md)
- [Test guide](tests/README.md)
- [TUI guide](apps/tui/README.md)
- [GUI guide](apps/gui/README.md)
- [Project roles](docs/roles.md)
- License: AGPL-3.0-or-later
