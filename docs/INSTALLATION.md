# Installation And Startup

This document describes the production-ready local setup path for Tura on
Windows, Linux, and macOS.

Tura has two runnable terminal surfaces:

- Rust CLI: `cargo run -p gateway --bin tura -- exec "prompt"`.
- TypeScript CLI/TUI: `node apps/tui/dist/index.js ...`, normally used against
  a running gateway server.

The scripts keep dependencies project-local where possible. Python fallback
packages are installed into `scripts/packages/python`, TypeScript dependencies
are installed from package lockfiles, and Rust crates are built through Cargo
package names rather than directory names.

## Requirements

Install these system toolchains before running the scripts:

- Git.
- Rust and Cargo from rustup.
- Node.js 20 or newer with npm.
- Python 3.10 or newer with pip.
- Bun, only if you need `apps/gui`.

Optional tools:

- `ffmpeg`, for faster video/audio media inspection.
- Playwright Chromium, installed by the scripts unless skipped.

## One-Time Install

Windows PowerShell:

```powershell
.\scripts\install.ps1
```

Linux/macOS:

```bash
./scripts/install.sh
```

The install scripts perform these steps:

- verify required toolchains;
- install Python fallback packages into `scripts/packages/python`;
- install and build `apps/tui`;
- install `apps/gui` when Bun is available;
- install Playwright Chromium unless skipped;
- run `cargo fetch`;
- build `gateway` binaries `tura` and `gateway`;
- build `tura_router`;
- check the runtime, tools, provider, agents, and utils crates.

Useful install flags:

```powershell
.\scripts\install.ps1 -CheckOnly
.\scripts\install.ps1 -SkipFrontend -SkipPlaywright
.\scripts\install.ps1 -Release
```

```bash
./scripts/install.sh --check-only
./scripts/install.sh --skip-frontend --skip-playwright
./scripts/install.sh --release
```

## Start Commands

Run a prompt through the Rust CLI:

```powershell
.\scripts\start.ps1 "Inspect the workspace"
```

```bash
./scripts/start.sh "Inspect the workspace"
```

Run the gateway server:

```powershell
.\scripts\start.ps1 -Gateway -Port 4096
```

```bash
./scripts/start.sh --gateway --port 4096
```

Run the TypeScript terminal client:

```powershell
.\scripts\start.ps1 -Tui --help
```

```bash
./scripts/start.sh --tui --help
```

Build without starting:

```powershell
.\scripts\start.ps1 -BuildOnly
```

```bash
./scripts/start.sh --build-only
```

## Direct Development Commands

Rust checks:

```bash
cargo check
cargo test -p code-tools
cargo run -p gateway --bin tura -- exec "Summarize the architecture"
cargo run -p gateway --bin gateway
cargo run -p tura_router
```

TUI checks:

```bash
npm --prefix apps/tui ci
npm --prefix apps/tui run build
npm --prefix apps/tui test
node apps/tui/dist/index.js --help
```

GUI checks, when Bun is installed:

```bash
bun --cwd apps/gui install
bun --cwd apps/gui run typecheck
bun --cwd apps/gui run build
```

## Environment And Secrets

Local secrets must stay in `.env` or another ignored local file. Do not commit
API keys, OAuth tokens, provider tokens, cloud credentials, local sessions, or
provider call logs.

Provider configuration can be supplied through:

- `.env`;
- `TURA_ENV_PATH`;
- `TURALLM_CONFIG`;
- `crates/provider/config/tura_llm_config.json`;
- request-scoped CLI overrides such as `--model`.

Environment variables override file config. Request/session overrides take
precedence over defaults.

## Cross-Platform Notes

Windows:

- Use PowerShell 5.1 or PowerShell 7.
- If script execution is restricted, run:
  `powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1`.
- Generated Python packages under `scripts/packages/python` are ignored by git.

Linux:

- The scripts use POSIX `sh`, not Bash-specific arrays.
- Install OS packages with your distribution package manager before running the
  script if Git, Cargo, Node, npm, or Python are missing.

macOS:

- Install Xcode Command Line Tools if Cargo cannot link native crates.
- Homebrew is recommended for Git, Node, Python, Bun, and ffmpeg.

## Troubleshooting

`cargo` is missing:

- Install Rust through rustup and reopen the terminal.

`npm ci` fails in `apps/tui`:

- Delete `apps/tui/node_modules` and rerun the install script.

`libclang` cannot be found while building media dependencies:

- Rerun the install script. It installs the `libclang` Python wheel locally and
  exports `LIBCLANG_PATH` for that script invocation.

Gateway is unavailable from the TypeScript CLI:

- Start it with `scripts/start.ps1 -Gateway` or
  `scripts/start.sh --gateway`.
- Override the URL with `--gateway-url` or `TURA_GATEWAY_URL`.
