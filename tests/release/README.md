# Release Tests

This directory contains release-binary tests for built `target/release`
artifacts. These tests are separated from `tests/business` and `tests/live` so
ordinary business/live runs do not start, reuse, or shut down release daemons.

The backend release runner scans files directly under `tests/release`, while
skipping `tui_*` and `gui_*` entrypoints so UI/provider release flows remain
explicit:

```powershell
.\xtask\scripts\run-backend-release-tests.ps1 -List
.\xtask\scripts\run-backend-release-tests.ps1 -TimeoutSeconds 600
```

```bash
./xtask/scripts/run-backend-release-tests.sh --list
./xtask/scripts/run-backend-release-tests.sh --timeout-seconds 600
```

Release-entry scripts run after `scripts/build-release.*` and
`scripts/register-cli.*`. The CLI surface uses the registered `tura exec`
command. TUI/GUI release scripts also live here, rather than under
`tests/live` or app-local business/live directories, because they all start or
depend on `target/release` artifacts.

The kept entry points cover each release surface:

| Profile | Surface | Entry point |
| --- | --- | --- |
| `target/release` | CLI | `.\xtask\scripts\run-backend-release-tests.ps1` / `./xtask/scripts/run-backend-release-tests.sh` |
| `target/release` | TUI | `node .\tests\release\tui_release_run_all.mjs` / `node ./tests/release/tui_release_run_all.mjs` |
| `target/release` | GUI | `node .\tests\release\gui_release_run_all.mjs` / `node ./tests/release/gui_release_run_all.mjs` |

The app package aliases point at the same root scripts:

```powershell
npm --prefix apps\tui run test:live:release
bun run --cwd apps\gui e2e:live:release
```

```bash
npm --prefix apps/tui run test:live:release
bun run --cwd apps/gui e2e:live:release
```

Release-entry outputs default to:

```text
target/business/{profile}/{surface}/{case}/{run_id}/summary.json
```

The release-entry summary schema is `tura.business.release-entry.v1`. It records
the surface, case name, binary profile, binary directory, model, agent, command,
logs, final message, validation checks, workspace, and artifact root.

Default release-entry timeouts are bounded: single request uses 180 seconds,
snake uses 240 seconds, and the password-zip CLI refactor uses 600 seconds. TUI
receives a 30 second process cleanup buffer and GUI receives a 60 second
gateway/browser cleanup buffer. Override a single case with
`TURA_BUSINESS_SNAKE_TIMEOUT_MS`, `TURA_BUSINESS_SINGLE_REQUEST_TIMEOUT_MS`, or
`TURA_BUSINESS_PASSWORD_ZIP_TIMEOUT_MS`; override all release-entry cases with
`TURA_BUSINESS_TIMEOUT_MS`.
