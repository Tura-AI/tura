# How to start

Use the smallest front end that fits the job. The detailed reference is
[docs/start/how-to-start.md](../../docs/start/how-to-start.md).

Before starting Tura, make sure the operating system can resolve both the Tura
executables and the shell executors used by `command_run`. PATH problems look
like agent problems, which is rude but traditional.

## OS PATH and executor support

| OS | PATH should resolve | If it is missing | Register Tura on PATH |
| --- | --- | --- | --- |
| Windows | `powershell.exe` or `pwsh`, `git`, Git/MSYS `bash`, optional MSYS2 `zsh`, and Tura release binaries. | Install PowerShell 7 with `winget install --id Microsoft.PowerShell --source winget`; install Git with `winget install --id Git.Git --source winget`; install MSYS2 with `winget install --id MSYS2.MSYS2 --source winget`, then add `C:\msys64\usr\bin` to PATH or set `TURA_ZSH_PATH` to `zsh.exe`. If PowerShell is installed outside PATH, set `TURA_POWERSHELL_PATH`. | Run `./scripts/build-release.ps1`, then `./scripts/register-cli.ps1`, then open a new terminal. |
| macOS | `git`, `bash`, `zsh`, package-manager paths such as `/opt/homebrew/bin` or `/usr/local/bin`, and Tura release binaries. | Install Apple command line tools with `xcode-select --install`. If Homebrew tools are needed, run `brew install git bash zsh` and make sure Homebrew's bin directory is in your shell profile. If zsh is custom-installed, set `TURA_ZSH_PATH`. | Run `./scripts/build-release.sh`, then `./scripts/register-cli.sh`, then open a new terminal. |
| Linux | `git`, `bash`, optional `zsh`, normal system bin paths, and Tura release binaries. | Debian/Ubuntu: `sudo apt-get install git bash zsh`. Fedora: `sudo dnf install git bash zsh`. Arch: `sudo pacman -S git bash zsh`. If zsh lives outside PATH, set `TURA_ZSH_PATH`. | Run `./scripts/build-release.sh`, then `./scripts/register-cli.sh`, then open a new terminal. |

`scripts/install.*` checks `shell_command`, `bash`, `zsh`, and `git` coverage.
Set `TURA_STRICT_SHELL_TOOL_COVERAGE=1` when missing optional shell support
should fail instead of warn. For the complete install matrix, see
[Install](install.md).

If PATH registration is not available yet, start by direct binary path:

```powershell
.\target\release\tura.exe
.\target\release\tura.exe exec "Inspect this workspace"
```

```sh
./target/release/tura
./target/release/tura exec "Inspect this workspace"
```

| Start method | Best for | Command |
| --- | --- | --- |
| TUI | Interactive terminal work | `tura` |
| CLI one-shot | Direct prompt from a shell or script | `tura exec "..."` |
| CLI via gateway | Scriptable prompt with gateway streaming/history | `tura run "..."` |
| GUI desktop | Visual workspace and session management | `tura_gui` |
| Web GUI/gateway | Browser GUI and HTTP/SSE API | `tura_gateway` |
| Source shortcut | Start from the checkout | `scripts/start.*` |

## Common starts

```sh
tura
tura "Inspect this repository"
tura exec "Find the riskiest area in this workspace"
tura run "Summarize the current session"
```

For flags and low-level binaries, continue to [CLI parameters](cli-parameters.md).
