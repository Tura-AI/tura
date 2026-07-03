# Install And Start

This page keeps operational startup details out of the project homepage. It
covers dependency install, release/debug builds, launcher registration, local
clients, and CI-style checks.

## Install Dependencies

Install user-local dependency tools and package-owned dependencies first. This
does not build binaries: it installs `uv`, ensures Python 3.12 is available to
`uv`, installs `bun`, command-local Python `.venv` directories, and Bun
workspaces in place.

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

## Release Build

Production build writes release binaries into Cargo's standard `target/release`
directory and registers that directory on PATH. The registered CLI command is
`tura exec`; the TUI entry remains `tura`.

```powershell
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
```

```bash
./scripts/build-release.sh
scripts/register-cli.sh
```

Release builds preserve repository-local session DB/cache state by default. Use
`-Clean` on PowerShell or `-clean`/`--clean` on POSIX shells only when you want
the build script to remove that local state before building.

## Debug Build

Development build writes debug binaries into `target/debug` and keeps frontend
artifacts in their source workspaces.

```powershell
.\scripts\build-debug.ps1
```

```bash
./scripts/build-debug.sh
```

## Run A Prompt

Direct CLI prompt:

```powershell
.\scripts\start.ps1 "Inspect the workspace"
```

```bash
./scripts/start.sh "Inspect the workspace"
```

After release registration:

```powershell
tura exec "Inspect the workspace"
```

```bash
tura exec "Inspect the workspace"
```

## Choose A Shell Surface

Per-run command tool shell overrides are available as CLI commands and flags.

```powershell
tura bash "Inspect the workspace using bash command tools"
tura zsh "Inspect shell startup files with zsh command tools"
tura shel "Use the system shell_command surface"
tura exec --zsh "Run through the Rust CLI front with zsh command tools"
```

```bash
tura bash "Inspect the workspace using bash command tools"
tura zsh "Inspect shell startup files with zsh command tools"
tura shel "Use the system shell_command surface"
tura exec --zsh "Run through the Rust CLI front with zsh command tools"
```

## Start Clients

Start `tura_gateway` first, then launch the TUI, GUI, or desktop shell. Clients
only connect to an existing gateway; they fail if gateway is not already running.

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

## Launcher Maintenance

```powershell
.\scripts\register-cli.ps1
.\scripts\unregister-cli.ps1
```

```bash
./scripts/register-cli.sh
./scripts/unregister-cli.sh
```

## Package And Script Checks

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

## CI-Style Flow

```powershell
.\scripts\check-backend-quality.ps1
.\scripts\run-ci.ps1
.\scripts\run-release-dry-run.ps1
```

```bash
sh scripts/check-backend-quality.sh
sh scripts/run-ci.sh
sh scripts/run-release-dry-run.sh
```

`check-backend-quality` is the smell gate: backend Rust test layout, Rust
formatting, TUI formatting, dependency policy, and spelling. It does not run
`cargo test --workspace`. The CI flow runs crate clippy/tests, backend business
tests, and TUI business tests in parallel after the smell gate passes.

## Release-Entry Tests

Run the release binary suite after `build-release` and registration:

```powershell
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
.\xtask\scripts\run-backend-release-tests.ps1
npm --prefix apps\tui run test:live:release
bun run --cwd apps\gui e2e:live:release
```

```bash
./scripts/build-release.sh
scripts/register-cli.sh
sh xtask/scripts/run-backend-release-tests.sh
npm --prefix apps/tui run test:live:release
bun run --cwd apps/gui e2e:live:release
```
