# Business Tests

This directory contains root backend business tests and shared business-test
helpers. Backend business runners only execute Rust tests from this directory;
they do not execute `.mjs` TUI, GUI, browser, or app scripts.

Root Rust tests in this directory are part of the backend business runner.
Shared `.mjs` helper files are not executable app tests and must not be wired
into CI or crate tests as one-off fixtures.

App-owned scripts belong under their app package, such as
`apps/tui/e2e/business/` or `apps/gui/e2e/business/`, and must be run through
those app scripts explicitly.

The workspace test types are peers: `tests/business`, `tests/performance`,
`tests/live`, and `tests/benchmark`. Crate-owned Rust tests follow the same
peer layout under each package's `tests/` directory. Business tests may use
local sockets, local HTTP fixtures, files, and subprocesses, but must not
require third-party services, provider tokens, API keys, paid providers, or
public live systems. Required non-network integration tests that should run with
plain `cargo test` live directly under each package's `tests/` directory; do
not create `tests/e2e` directories. Do not keep empty
`business`, `performance`, `live`, or `benchmark` directories. Keep files
directly under each typed directory; do not create category subdirectories under
`tests/business`, `tests/performance`, `tests/live`, or `tests/benchmark`.
Encode categories in filenames. Runners should select cases by test type and a
one-level directory scan rather than by hardcoded one-off script paths whenever
the directory layout can express the suite.

Do not write production logic or tests that pass by matching arbitrary
exact-response prompt wording. Business assertions must be based on structured
outputs, command exit/result shape, protocol fields, files, or explicit parser
contracts.

```powershell
.\scripts\run-backend-business-tests.ps1 -List
.\scripts\run-backend-business-tests.ps1 -Crate tools
```

```bash
./scripts/run-backend-business-tests.sh --list
./scripts/run-backend-business-tests.sh --crate tools
```

Manual backend business script outputs default to:

```text
~/Documents/tura_workspace/target/{test_name}/{run_id}/summary.json
```

Override the artifact root with `TURA_BUSINESS_TARGET_ROOT` or
`COMMAND_RUN_BUSINESS_TARGET_ROOT`.

Long-running comparison and scoring suites belong under `tests/benchmark/`, not
this directory.

Root process business tests include a real-process state flow, a process-scope
tree-kill flow, and a cross-OS lifecycle policy matrix. The matrix simulates
Windows, Linux, macOS, and fallback OS families so router/session_db owner
adoption, gateway front leases, runtime worker scopes, and command_run scopes
stay explicit even when the current developer machine only exercises one OS.
