# Tura

Tura is a local AI coding system built around a Rust workspace. It provides a
direct Rust CLI, an HTTP/SSE gateway, router-managed background processes,
agent/persona orchestration, provider integration, command execution tools, a
TypeScript terminal client, and a Bun/Solid GUI.

Rust builds use the pinned toolchain in `rust-toolchain.toml`. The repository is
licensed under AGPL-3.0-or-later; see `LICENSE`.

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

Install/start scripts check all shell command-run surfaces on every platform:
`shell_command`, `bash`, and `zsh`. The install scripts try to install missing
bash/zsh dependencies with the platform package manager: Windows uses MSYS2
through winget/pacman, macOS uses Homebrew, and Linux uses common system package
managers. Set `TURA_STRICT_SHELL_TOOL_COVERAGE=1` to turn optional shell
warnings into failures. `scripts/register-cli.sh` updates `.zprofile` and
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

Per-run command tool shell overrides are available as CLI commands and flags:

```powershell
tura bash "Inspect the workspace using bash command tools"
tura zsh "Inspect shell startup files with zsh command tools"
tura shll "Use the system shell_command surface"
tura exec --zsh "Run through the Rust CLI front with zsh command tools"
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

Workspace tests are classified by top-level peer directory:
`tests/business`, `tests/performance`, `tests/live`, `tests/release`, and
`tests/benchmark`.
Crate-owned Rust tests use the same peer names under each package `tests/`
directory. Business tests are required local business/link flows and must not
depend on third-party services, provider tokens, API keys, paid providers, or
public live systems. Live tests contain those external/key-dependent checks and
are opt-in. Performance tests contain stress, load, soak, and stability checks.
Benchmark tests contain scoring and comparison suites.

Backend business and performance runners discover tests by package and
directory type; keep typed test files directly under `business`, `performance`,
`live`, or `benchmark`, do not add empty type directories, and do not hard-code
individual test script paths when a one-level directory scan can select the
suite. CI and default
`cargo test --workspace --exclude src-tauri` run only unit tests, default crate
tests, and required non-foldered integration tests.

Run the release binary suite after `build-release` and registration:

```powershell
.\scripts\build-release.ps1; .\scripts\register-cli.ps1
.\scripts\run-backend-release-tests.ps1
npm --prefix apps\tui run test:live:release
bun run --cwd apps\gui e2e:live:release
```

```bash
./scripts/build-release.sh; scripts/register-cli.sh
./scripts/run-backend-release-tests.sh
npm --prefix apps/tui run test:live:release
bun run --cwd apps/gui e2e:live:release
```

## More Information

- [Full operational overview](docs/overview.md)
- [Architecture boundaries](ARCHITECTURE.md)
- [Scripts architecture](scripts/ARCHITECTURE.md)
- [Business test guide](tests/business/README.md)
- [Live test guide](tests/live/README.md)
- [Release test guide](tests/release/README.md)
- [Benchmark guide](tests/benchmark/README.md)
- [Test guide](tests/README.md)
- [TUI guide](apps/tui/README.md)
- [GUI guide](apps/gui/README.md)
- License: AGPL-3.0-or-later
