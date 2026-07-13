# Install and Uninstall

This page describes how to install Tura from the source repository, how to run
the GitHub-hosted installer with `curl`, how to unregister or remove a local
installation, and how to clean local runtime state when you need a fresh
environment.

Tura is built from the repository. The dependency installers in `scripts/` are
not standalone package managers: they expect to run from a Tura checkout so they
can find `commands/`, `apps/`, `crates/`, and release scripts.

## Requirements

The source installer checks these tools and installs the missing ones when the
current platform has a supported installer. In `CheckOnly` or `Offline` mode it
only verifies them and fails with the manual step needed.

| Tool | Required for | Notes |
| --- | --- | --- |
| Git | cloning the repository | Windows users can use Git for Windows. |
| Rust and Cargo | backend binaries | `scripts/install.*` can install the minimal rustup toolchain and add the Cargo bin directory to PATH. |
| PowerShell | Windows scripts | Windows PowerShell or PowerShell 7 works. On Windows, `scripts/install.*` and npm postinstall ensure PowerShell is discoverable; npm does not require Rust/Cargo. |
| POSIX shell and Bash | Linux/macOS scripts and command execution | Linux normally has these already. |
| zsh | macOS default command surface | On Windows, MSYS2 zsh can be used. |
| Bun | TUI, GUI, and Tauri release builds | `scripts/install.*` can install user-local Bun and add it to PATH. |
| uv and Python 3.12 | command package environments | `scripts/install.*` can install user-local uv, Python 3.12, and command virtual environments. |

The source install scripts check Git, Rust/Cargo, PowerShell, `shell_command`,
`bash`, `zsh`, Bun, uv, and Python 3.12 coverage. They update PATH for the
current process, GitHub Actions, and user-local tool locations where the platform
supports safe user PATH/profile updates. The npm package postinstall is narrower:
it installs or copies release binaries, registers the Tura CLI path, and checks
only runtime dependencies such as PowerShell on Windows and basic shell/archive
tools on Unix. It does not check or install Rust/Cargo, Bun, uv, or Python.
Set `TURA_STRICT_SHELL_TOOL_COVERAGE=1` if you want missing optional shell
surfaces to fail instead of warn.

## Install from a Git checkout

Clone the repository and run the platform installer from the checkout root.

```powershell
git clone https://github.com/Tura-AI/tura.git
cd tura
.\scripts\install.ps1
tura exec "Inspect this workspace"
```

```bash
git clone https://github.com/Tura-AI/tura.git
cd tura
./scripts/install.sh
tura exec "Inspect this workspace"
```

By default, `scripts/install.*` installs source-checkout dependencies, builds the
full release into `target/release`, and registers that directory on the user
PATH. `scripts/build-release.*` and `scripts/register-cli.*` remain available
for targeted development and release work.

## Install with curl from GitHub

Use this when you want a single terminal flow that downloads the source archive
from GitHub and runs the repository installer. It uses the public
`https://github.com/Tura-AI/tura` repository as the source.

### Linux and macOS

```bash
set -eu
curl -L https://github.com/Tura-AI/tura/archive/refs/heads/main.tar.gz -o tura-main.tar.gz
mkdir -p tura-install
tar -xzf tura-main.tar.gz -C tura-install --strip-components=1
cd tura-install
./scripts/install.sh
tura exec "Inspect this workspace"
```

If you already have a checkout and only want to run the GitHub-hosted install
file, download the installer into that checkout before executing it:

```bash
curl -L https://raw.githubusercontent.com/Tura-AI/tura/main/scripts/install.sh -o scripts/install.sh
chmod +x scripts/install.sh
./scripts/install.sh
```

Do not pipe the installer directly into `sh`: the script resolves paths relative
to its own file location and expects the rest of the repository to be present.
Piping loses that context. Tiny detail, large faceplant.

### Windows PowerShell

```powershell
$ErrorActionPreference = "Stop"
curl.exe -L "https://github.com/Tura-AI/tura/archive/refs/heads/main.zip" -o "tura-main.zip"
Expand-Archive -Path "tura-main.zip" -DestinationPath "tura-main-expanded" -Force
$repo = Get-ChildItem -Path "tura-main-expanded" -Directory | Select-Object -First 1
Set-Location $repo.FullName
.\scripts\install.ps1
tura exec "Inspect this workspace"
```

If you already have a checkout and only want the GitHub-hosted install file:

```powershell
curl.exe -L "https://raw.githubusercontent.com/Tura-AI/tura/main/scripts/install.ps1" -o "scripts\install.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

## Installer options

Only `-EnvironmentOnly` or `--environment-only` changes the installer into a
dependency-only flow. Dependency-only switches must be paired with that option.

| PowerShell | Bash | Meaning |
| --- | --- | --- |
| `-EnvironmentOnly` | `--environment-only` | Install or verify dependencies only; skip release build and PATH registration. |
| `-SkipCommands` | `--skip-commands` | Skip `commands/*/install.*` command package setup. |
| `-SkipApps` | `--skip-apps` | Skip JavaScript installs for TUI, GUI, and Tauri workspaces. |
| `-SkipUv` | `--skip-uv` | Do not install or verify uv. Requires skipping command installers. |
| `-SkipBun` | `--skip-bun` | Do not install or verify Bun. Requires skipping app installers. |
| `-CheckOnly` | `--check-only` | Verify expected tools and environments without installing. |
| `-Offline` | `--offline` | Use offline or cache-only flags where supported. |

Examples:

```powershell
.\scripts\install.ps1 -EnvironmentOnly -CheckOnly
.\scripts\install.ps1 -EnvironmentOnly -SkipApps
```

```bash
./scripts/install.sh --environment-only --check-only
./scripts/install.sh --environment-only --skip-apps
```

## Build options

The normal release build includes backend binaries, the TUI executable, the GUI
web dist, the Tauri desktop bundle, provider config, prompts, command metadata,
and command source files.

```powershell
.\scripts\build-release.ps1
```

```bash
./scripts/build-release.sh
```

Useful build switches:

| PowerShell | Bash | Meaning |
| --- | --- | --- |
| `-BackendOnly` | `--backend-only` | Build Rust backend release artifacts only. |
| `-Binary` | `--binary` | Keep only binaries and minimal provider config in release output. |
| `-SkipTui` | `--skip-tui` | Skip the compiled TUI executable. |
| `-SkipGui` | `--skip-gui` | Skip the GUI web build. |
| `-SkipTauri` | `--skip-tauri` | Skip the Tauri desktop bundle. |
| `-Clean` | `-clean` or `--clean` | Remove repository-local runtime state before building. |

## Verify the install

After registration, open a new terminal and run:

```powershell
tura --help
tura exec "Say hello from this installation"
```

```bash
tura --help
tura exec "Say hello from this installation"
```

For a local dependency-only check without changing the build output:

```powershell
.\scripts\install.ps1 -EnvironmentOnly -CheckOnly
```

```bash
./scripts/install.sh --environment-only --check-only
```

## Uninstall the CLI registration

Unregistering removes Tura release PATH entries and stale `cli-bin` wrappers. It
does not delete the checkout, build artifacts, provider logs, or session data.

```powershell
.\scripts\unregister-cli.ps1
```

```bash
./scripts/unregister-cli.sh
```

Open a new terminal after unregistering so the shell reloads PATH.

## Remove local files

After unregistering, delete the checkout directory if you want to remove the
source tree and build artifacts:

```powershell
Set-Location ..
Remove-Item -LiteralPath .\tura -Recurse -Force
```

```bash
cd ..
rm -rf ./tura
```

Only run those commands from the parent directory of the checkout you intend to
delete. Obvious, yes. Still where people lose afternoons.

## Clean local runtime state

Tura stores runtime and session state in two places:

| Location | Contents |
| --- | --- |
| `<TURA_HOME>/db/session_log/index.sqlite3` | per-home session DB index and queue |
| `<TURA_HOME>/.tura/` | sockets, locks, active gateway file, and per-home runtime state |
| `<workspace>/.tura/session_log.sqlite3` | workspace session history |
| `<workspace>/.tura/config.conf` | workspace settings |
| `log/provider/YYYY-MM-DD/*.json` | provider request and response diagnostics |

When `TURA_HOME` is not set and you run from the source checkout, the checkout
itself is the instance home. The release build cleanup switch removes only these
repository-local runtime files before building:

```text
db/session_log
.tura/config.conf
.tura/session_log.sqlite3
.tura/session_log.sqlite3-wal
.tura/session_log.sqlite3-shm
.tura/session_log.sqlite3.init.lock
```

Run the cleanup build like this:

```powershell
.\scripts\build-release.ps1 -Clean
```

```bash
./scripts/build-release.sh --clean
```

Use `-Clean` or `--clean` when you need a fresh local runtime state before a
release build. Do not use it if you need to preserve local sessions or workspace
settings.

## Manual environment cleanup

If you want to clean a custom `TURA_HOME`, stop Tura processes first, then remove
only the home you explicitly selected.

PowerShell example:

```powershell
$env:TURA_HOME = "C:\Users\you\tura-home"
Get-Process tura,tura_gateway,tura_router,tura_session_db,tura_runtime,tura_exec -ErrorAction SilentlyContinue |
  Stop-Process -Force
Remove-Item -LiteralPath $env:TURA_HOME -Recurse -Force
```

Bash example:

```bash
export TURA_HOME="$HOME/tura-home"
pkill -f "$TURA_HOME" || true
rm -rf -- "$TURA_HOME"
```

For workspace-only cleanup, remove the workspace `.tura` directory after
confirming you no longer need its session history or settings:

```powershell
Remove-Item -LiteralPath .\.tura -Recurse -Force
```

```bash
rm -rf -- ./.tura
```

Provider logs are separate from session history. Remove `log/provider/` only if
you no longer need request diagnostics.

## Update an existing checkout

```powershell
git pull
.\scripts\install.ps1
```

```bash
git pull
./scripts/install.sh
```

Use the cleanup build switch during update only when you intentionally want to
drop repository-local runtime state.
