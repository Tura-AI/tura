# How to Start Tura

This page begins where installation ends: the executable is already available,
and you want to start Tura without guessing which front end owns what. Setup,
removal, provider details, and the full CLI parameter reference stay in their
own documents. For the full command surface, see
[CLI Parameters](cli-parameters.md).

Before starting Tura, make sure the operating system can resolve both the Tura
executables and the shell executors used by `command_run`. PATH problems look
like agent problems, which is rude but traditional.

## OS PATH and executor support

| OS | PATH should resolve | If it is missing | Register Tura on PATH |
| --- | --- | --- | --- |
| Windows | `powershell.exe` or `pwsh`, `git`, Git/MSYS `bash`, optional MSYS2 `zsh`, and Tura release binaries. | Install PowerShell 7 with `winget install --id Microsoft.PowerShell --source winget`; install Git with `winget install --id Git.Git --source winget`; install MSYS2 with `winget install --id MSYS2.MSYS2 --source winget`, then add `C:\msys64\usr\bin` to PATH or set `TURA_ZSH_PATH` to `zsh.exe`. If PowerShell is installed outside PATH, set `TURA_POWERSHELL_PATH`. | Run `.\scripts\install.ps1`, then open a new terminal. |
| macOS | `git`, `bash`, `zsh`, package-manager paths such as `/opt/homebrew/bin` or `/usr/local/bin`, and Tura release binaries. | Install Apple command line tools with `xcode-select --install`. If Homebrew tools are needed, run `brew install git bash zsh` and make sure Homebrew's bin directory is in your shell profile. If zsh is custom-installed, set `TURA_ZSH_PATH`. | Run `./scripts/install.sh`, then open a new terminal. |
| Linux | `git`, `bash`, optional `zsh`, normal system bin paths, and Tura release binaries. | Debian/Ubuntu: `sudo apt-get install git bash zsh`. Fedora: `sudo dnf install git bash zsh`. Arch: `sudo pacman -S git bash zsh`. If zsh lives outside PATH, set `TURA_ZSH_PATH`. | Run `./scripts/install.sh`, then open a new terminal. |

`scripts/install.*` checks and installs source-checkout dependencies, including
Git, Rust/Cargo, PowerShell, `shell_command`, `bash`, `zsh`, Bun, uv, and Python
3.12, builds the release, and updates PATH where the platform allows safe
user-local registration. Use the explicit environment-only option when you do
not want the build and registration stages.
The npm package install path is intentionally narrower: it checks runtime
dependencies and registers the release CLI path, but it does not check or install
Rust/Cargo, Bun, uv, or Python. Set `TURA_STRICT_SHELL_TOOL_COVERAGE=1` when
missing optional shell support should fail instead of warn. For the complete
install matrix, see [Install](install.md).

If PATH registration is not available yet, start by direct binary path:

```powershell
.\target\release\tura.exe
.\target\release\tura.exe exec "Inspect this workspace"
```

```bash
./target/release/tura
./target/release/tura exec "Inspect this workspace"
```

Use the smallest front end that fits the job:

| Start method | Best for | Command |
| --- | --- | --- |
| TUI | Interactive terminal work | `tura` |
| CLI one-shot | Direct prompt from a shell or script | `tura exec "..."` |
| CLI via gateway | Scriptable prompt with gateway streaming/history | `tura run "..."` |
| GUI desktop | Visual workspace and session management | `tura_gui` |
| Web GUI/gateway | Browser-based GUI and HTTP/SSE API | `tura_gateway` |
| TUI with initial prompt | Open the terminal UI from a command line prompt | `tura "..."` |
| Source shortcut | Start from the checkout | `scripts/start.*` |
| Source GUI dev | Run the GUI frontend during local development | `bun --cwd apps/gui dev` |
| Source desktop dev | Run the Tauri desktop app during local development | `bun --cwd apps/tauri dev` |

## Start the TUI

Run `tura` with no subcommand to open the interactive terminal interface.

```powershell
tura
```

```bash
tura
```

Use the TUI when you want an ongoing terminal conversation with the agent while
keeping the current workspace as the operating context.

You can also open the TUI with an initial prompt from the command line:

```powershell
tura "Inspect this workspace"
```

```bash
tura "Inspect this workspace"
```

To start the TUI from a source checkout without relying on a registered release
binary, use the source shortcut with the TUI switch:

```powershell
.\scripts\start.ps1 -Tui
```

```bash
./scripts/start.sh --tui
```

## Start a CLI task

Use `tura exec` for the direct Rust CLI front. This is the most compact way to
invoke Tura from a command line.

```powershell
tura exec "Inspect this workspace and summarize the risky parts"
```

```bash
tura exec "Inspect this workspace and summarize the risky parts"
```

If you are calling the backend executable directly, use `tura_exec` with the
same `exec` subcommand:

```powershell
tura_exec exec "Inspect this workspace"
```

```bash
tura_exec exec "Inspect this workspace"
```

Use `tura run` when you want the terminal client to send the prompt through the
gateway and stream the result back to the shell.

```powershell
tura run "Fix the failing test and verify it"
```

```bash
tura run "Fix the failing test and verify it"
```

Keep prompt text quoted when it contains spaces. For full command details, use
`tura --help` or [CLI Parameters](cli-parameters.md); this page stays on startup
paths, not every knob in the cockpit.

## Invoke Tura with a specific shell surface

Tura also exposes command-line launch forms that select the shell used by
`command_run` during that prompt:

```bash
tura bash "Run the Linux shell checks"
tura zsh "Inspect zsh startup behavior"
tura shel "Use the default shell_command surface"
```

These forms are useful when the task depends on shell semantics. They are still
CLI starts: they send a prompt, wait for completion, and return output to the
terminal.

## Start from scripts in the source checkout

The `scripts/start.*` helpers are for running from the repository checkout. They
check shell coverage, build missing debug artifacts, then launch Tura.

Default behavior starts a CLI `exec` task:

```powershell
.\scripts\start.ps1 "Inspect this checkout"
```

```bash
./scripts/start.sh "Inspect this checkout"
```

Start the release build instead of the debug build:

```powershell
.\scripts\start.ps1 -Release "Inspect this checkout"
```

```bash
./scripts/start.sh --release "Inspect this checkout"
```

Start the TUI through the same helper:

```powershell
.\scripts\start.ps1 -Tui
```

```bash
./scripts/start.sh --tui
```

For frontend development, start the web GUI directly from the GUI workspace:

```powershell
bun --cwd apps\gui dev
```

```bash
bun --cwd apps/gui dev
```

For desktop GUI development, start the Tauri app from its workspace:

```powershell
bun --cwd apps\tauri dev
```

```bash
bun --cwd apps/tauri dev
```

Those development starts assume the checkout dependencies are already present;
this page intentionally does not cover preparing them.

## Start the desktop GUI

Use `tura_gui` to start the desktop application when the release GUI bundle is
available on PATH or you are launching it from the release artifact directory.

```powershell
tura_gui
```

```bash
tura_gui
```

The desktop GUI starts or attaches to a local gateway as needed. It is the best
entry point when you want visual workspace navigation, files, sessions, plans,
and settings without staying inside a terminal UI.

You can also wake or focus the GUI from a command line and point it at an
already-running gateway:

```powershell
tura_gui --gateway-url http://127.0.0.1:4126 --workspace C:\path\to\workspace
```

```bash
tura_gui --gateway-url http://127.0.0.1:4126 --workspace /path/to/workspace
```

If a session is already known, the GUI accepts `--initial-session SESSION_ID` as
a startup hint, but session management itself belongs in the session docs.

## Start the gateway and web GUI

Use `tura_gateway` when you want the local HTTP/SSE gateway running explicitly.
The gateway is also what TUI and GUI clients use behind the scenes.

```powershell
tura_gateway
```

```bash
tura_gateway
```

By default, the release gateway listens on `http://127.0.0.1:4126`. A debug
gateway uses `http://127.0.0.1:4125`. If the bundled web GUI is present, open the
gateway URL in a browser to use the web GUI.

To choose a port for a gateway launch, set `PORT` for that process:

```powershell
$env:PORT = "4210"
tura_gateway
```

```bash
PORT=4210 tura_gateway
```

## Start by direct binary path

If the release directory is not on PATH, launch the binaries directly.

```powershell
.\target\release\tura.exe
.\target\release\tura.exe exec "Inspect this workspace"
.\target\release\tura_exec.exe exec "Inspect this workspace"
.\target\release\tura_gateway.exe
.\target\release\tura_gui.exe
```

```bash
./target/release/tura
./target/release/tura exec "Inspect this workspace"
./target/release/tura_exec exec "Inspect this workspace"
./target/release/tura_gateway
./target/release/tura_gui
```

From a debug checkout, use `target/debug` instead of `target/release`.

## Start from the npm package entry

When you are using the npm package entry, the `tura` command forwards arguments
to the platform release binary. Use the same starts:

```bash
tura
tura exec "Inspect this workspace"
tura run "Summarize the current project"
```

## Which start should I use?

| Need | Use |
| --- | --- |
| Ongoing terminal conversation | `tura` |
| Terminal conversation opened with an initial prompt | `tura "..."` |
| One command from a shell, script, CI job, or terminal shortcut | `tura exec "..."` |
| Gateway-backed terminal run with streamed events | `tura run "..."` |
| Shell-specific command execution behavior | `tura bash`, `tura zsh`, or `tura shel` |
| Visual app for workspace/session/file/plan work | `tura_gui` |
| Explicit local HTTP/SSE service or browser GUI | `tura_gateway` |
| Running from the source checkout | `scripts/start.ps1` or `scripts/start.sh` |
| Frontend development GUI | `bun --cwd apps/gui dev` |
| Desktop development GUI | `bun --cwd apps/tauri dev` |
