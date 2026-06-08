# Scripts Architecture

`scripts/` owns setup, startup, package environments, auto-install manifests,
and persistent reusable CLI workflows used by router/tools commands.
Scripts may print diagnostic commands, but they must not create another
session store. Session/task history is queried through `session_log`; provider
call logs live under `log/provider/` or `LOG_PATH`.

## Layout

```text
scripts/
  install.ps1
  install.sh
  start.ps1
  start.sh

  installers/
    media.toml
    playwright.toml

  packages/
    playwright_node/
      manifest.toml
    python/
      ...
    read_media/
      entry.py
      manifest.toml
```

There is no active top-level `scripts/persistent/` directory in the current
tree. Add persistent reusable workflows only when a router/tools command has a
real reuse case and a documented output contract.

## Install Scripts

Install scripts should:

- Detect installed versions and verify minimum versions where the project has
  one, such as Node.js 20+ and Python 3.10+.
- Automatically install Rust/Cargo through rustup when possible; if company
  policy, network, or permissions block installation, print rustup guidance.
- Automatically install other toolchains when possible:
  - Windows: `winget`.
  - Linux: `apt-get`, `dnf`, `yum`, `pacman`, or `apk`.
  - macOS: Homebrew.
- Install or verify Git, Node/npm, Python, Bun, ffmpeg, native build tools, and
  the platform shell. On Windows this includes MSVC Build Tools for Rust's
  `*-msvc` linker, MSYS2/UCRT64 for POSIX/native command surfaces, and WebView2
  for the Tauri desktop shell.
- Install `apps/tui` dependencies from `package-lock.json`.
- Install and build `apps/gui` when Bun is available.
- Install `apps/tauri` dependencies and verify platform prerequisites when Bun
  and the desktop shell are available.
- Default to the **production** route (no `dev` argument): build release
  binaries and package them into a self-contained `bin/` via `build-bin.*`
  (`gateway`, `tura`, `tura_router`, `tura-tui`, `tura-gui` plus runtime
  resources). `bin/` must include `tura_router` so the packaged gateway resolves
  the router next to its own executable.
- With a `dev` argument (`-Dev` / `dev`), build the debug route instead:
  - Run `cargo fetch`.
  - Build `gateway` binaries `tura` and `gateway` (debug) plus `tura_router`.
  - Build `apps/tui` into `apps/tui/dist`.
  - Check runtime, tools, provider, and agents packages by Cargo package name.
- Install Python fallback packages into `scripts/packages/python`, never the
  repository root or a tracked package directory.
- Export `PYTHONPATH` and `LIBCLANG_PATH` for the current script invocation
  when local fallback packages are installed.
- Install Playwright Chromium unless explicitly skipped, then verify a headless
  Chromium launch with Playwright. Verification failures must print platform
  guidance for proxy, endpoint security, sudo/system-library, and browser-cache
  issues.
- Respect skip flags for frontend, Playwright, Python fallback packages, and
  Rust build work. Check-only mode must verify without installing.
- Avoid writing generated logs or screenshots into tracked paths.
- Fail with actionable messages when required system toolchains are missing.

Supported first-party install entrypoints:

```text
scripts/install.ps1     Windows PowerShell
scripts/install.sh      Linux/macOS POSIX shell
```

## Start Scripts

Start scripts should:

- Prefer CLI-driven startup.
- Default to running `cargo run -p gateway --bin tura -- exec ...`.
- Preserve the direct Rust CLI output contract: final assistant text on
  `stdout`, default lightweight progress on `stderr`, `--quiet`/`--silent`
  suppressing progress, and `--json` switching `stdout` to JSONL events.
- Support a gateway-server mode for the TypeScript CLI/TUI:
  `cargo run -p gateway --bin gateway`.
- Support a TUI-client mode that runs `node apps/tui/dist/index.js ...`.
- Support a GUI dev-server mode that runs `bun run dev` from `apps/gui`.
- Support a desktop quickstart mode that runs `bun run dev` from
  `apps/tauri`, letting Tauri own the Vite frontend startup.
- Start the router binary when CLI forwarding or managed lifecycle is needed,
  for example
  `cargo run -p router --bin tura_router -- forward <command> [args...]`.
- Pass router/gateway/frontend overrides when a local UI flow needs them.
- For GUI startup, set `VITE_TURA_GATEWAY_URL` from `TURA_GATEWAY_URL` when
  present, otherwise from the selected gateway port.
- Avoid introducing independent service startup paths.
- Let router pull up and monitor managed local services/processes.
- Print CLI endpoints, UI URLs, and health status when applicable.

Supported first-party start entrypoints:

```text
scripts/start.ps1       Windows PowerShell
scripts/start.sh        Linux/macOS POSIX shell
```

Supported first-party start modes:

```text
scripts/start.ps1 "Prompt"                  Rust CLI exec
scripts/start.ps1 -Gateway -Port 4096       gateway HTTP server
scripts/start.ps1 -Tui --help               TypeScript CLI/TUI
scripts/start.ps1 -Gui                      GUI Vite dev server
scripts/start.ps1 -Desktop                  Tauri desktop shell

scripts/start.sh "Prompt"                   Rust CLI exec
scripts/start.sh --gateway --port 4096      gateway HTTP server
scripts/start.sh --tui --help               TypeScript CLI/TUI
scripts/start.sh --gui                      GUI Vite dev server
scripts/start.sh --desktop                  Tauri desktop shell
```

## Auto Install Manifests

Installer manifests define what router/tools commands may install
automatically:

- Tool id.
- Platform support.
- Installer command.
- Verification command.
- Timeout.
- Permission requirement.
- Cache key.
- Environment variables.

The model should not invent installer commands when a manifest exists.

Current installer manifests:

- `scripts/installers/media.toml`: media tool support, including ffmpeg
  verification.
- `scripts/installers/playwright.toml`: Playwright/Chromium setup and headless
  Chromium verification for frontend-debugging E2E flows.

Browser installation remains an installer concern so command handlers can ask
for a known capability instead of embedding ad hoc setup commands in prompts or
test fixtures.

## Package Environments

Each package environment manifest defines:

- Package id.
- Python version requirement.
- Dependencies.
- Entry script.
- Allowed network during setup.
- Cached virtual environment path.
- Rebuild trigger.

Router/tools should route `py:<package>` commands through this layer.

Current package environments:

- `scripts/packages/playwright_node/manifest.toml`: Node package environment
  for Playwright-based browser checks.
- `scripts/packages/read_media/manifest.toml`: Python entrypoint metadata for
  media reading.
- `scripts/packages/python/`: local fallback Python packages installed by
  setup scripts when the global environment is missing optional media
  dependencies.

The Playwright package environment provides the stable dependency boundary for
command-run sessions that need screenshots, DOM inspection, or local frontend
smoke tests.

## Persistent Scripts

Persistent scripts, when added, are reusable CLI workflows stored under
`scripts/persistent/<workflow_id>`.

Each persistent script needs:

- `manifest.toml`
- `script.py` or another declared entrypoint.
- `params.schema.json`
- Output contract.
- Permission category.
- Provenance fields.
- Example command request.

Scripts must read task-specific values from stdin JSON or
`TURA_COMMAND_PARAMS`. They must not hard-code one run's workspace paths,
markers, output paths, or entity names.

## Command Integration

Router/tools commands use scripts for:

- Python package execution.
- Auto-install verification.
- Persistent reusable workflows.
- Script provenance and reuse.
- Environment isolation.

Command search should return records whose examples call the routed command.
Raw source commands are debug/reference fields only.
