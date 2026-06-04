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
    apps.toml
    playwright.toml
    powershell_modules.toml
    node.toml
    python.toml
    system.toml

  packages/
    __stdlib__/
      manifest.toml
      entry.py
    requests/
      manifest.toml
      entry.py
    pandas/
      manifest.toml
      entry.py
    playwright_node/
      manifest.toml

  persistent/
    <workflow_id>/
      manifest.toml
      script.py
      params.schema.json
      README.md
```

Legacy package directories such as `scripts/requests` can be supported during
migration, but the documented target is `scripts/packages/<package>`.

## Install Scripts

Install scripts should:

- Verify Git, Rust, Cargo, Bun, Node, Python, and the platform shell.
- Install `apps/tui` dependencies from `package-lock.json`.
- Install `apps/gui` dependencies when Bun is available.
- Run `cargo fetch`.
- Build `gateway` binaries `tura` and `gateway`.
- Build `tura_router`.
- Check runtime, tools, provider, and agents packages by Cargo package
  name.
- Install Python fallback packages into `scripts/packages/python`, never the
  repository root or a tracked package directory.
- Export `PYTHONPATH` and `LIBCLANG_PATH` for the current script invocation
  when local fallback packages are installed.
- Install Playwright Chromium unless explicitly skipped.
- Respect a skip-tool-install flag.
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
- Support a gateway-server mode for the TypeScript CLI/TUI:
  `cargo run -p gateway --bin gateway`.
- Support a TUI-client mode that runs `node apps/tui/dist/index.js ...`.
- Start the router binary when CLI forwarding or managed lifecycle is needed,
  for example
  `cargo run -p tura_router -- forward <command> [args...]`.
- Pass router/gateway/frontend overrides when a local UI flow needs them.
- Avoid introducing independent service startup paths.
- Let router pull up and monitor managed local services/processes.
- Print CLI endpoints, UI URLs, and health status when applicable.

Supported first-party start entrypoints:

```text
scripts/start.ps1       Windows PowerShell
scripts/start.sh        Linux/macOS POSIX shell
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

`scripts/installers/playwright.toml` declares the Playwright/Chromium setup used
by frontend-debugging E2E flows. Browser installation remains an installer
concern so command handlers can ask for a known capability instead of embedding
ad hoc setup commands in prompts or test fixtures.

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

`scripts/packages/playwright_node/manifest.toml` is the Node package environment
for Playwright-based browser checks. It provides the stable dependency boundary
for command-run sessions that need screenshots, DOM inspection, or local
frontend smoke tests.

## Persistent Scripts

Persistent scripts are reusable CLI workflows stored under
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
