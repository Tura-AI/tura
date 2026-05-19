# Scripts Architecture

`scripts/` owns setup, startup, package environments, auto-install manifests,
and persistent reusable CLI workflows used by router/tools commands.

## Layout

```text
scripts/
  install.ps1
  install.sh
  start.ps1
  start.sh

  installers/
    apps.toml
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
- Install frontend dependencies.
- Run `cargo fetch`.
- Build core crates.
- Build tools.
- Run frontend typecheck.
- Respect a skip-tool-install flag.
- Avoid writing generated logs or screenshots into tracked paths.

## Start Scripts

Start scripts should:

- Prefer CLI-driven startup.
- Start the router binary when CLI forwarding or managed lifecycle is needed,
  for example
  `cargo run -p tura_router -- forward <command> [args...]`.
- Pass router/gateway/frontend overrides when a local UI flow needs them.
- Avoid introducing independent service startup paths.
- Let router pull up and monitor managed local services/processes.
- Print CLI endpoints, UI URLs, and health status when applicable.

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
