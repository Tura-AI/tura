# Release Tests

This directory contains release-binary tests for built `target/release`
artifacts. These tests are separated from `tests/business` and `tests/live` so
ordinary business/live runs do not start, reuse, or shut down release daemons.

The release runner discovers tests only by scanning files directly under
`tests/release`:

```powershell
.\scripts\run-backend-release-tests.ps1 -List
.\scripts\run-backend-release-tests.ps1 -TimeoutSeconds 600
```

```bash
./scripts/run-backend-release-tests.sh --list
./scripts/run-backend-release-tests.sh --timeout-seconds 600
```

Release-entry scripts run after `scripts/build-release.*` and
`scripts/register-cli.*`. The CLI surface uses the registered `tura exec`
command. TUI/GUI release live scripts live under
`tests/live/{tui,gui}_release_*.mjs` and are run through their app package
commands. These flows are live because they drive real model/provider execution.

The kept entry points cover each release surface:

| Profile | Surface | Entry point |
| --- | --- | --- |
| `target/release` | CLI | `.\scripts\run-backend-release-tests.ps1` / `./scripts/run-backend-release-tests.sh` |
| `target/release` | TUI | `npm --prefix apps/tui run test:live:release` |
| `target/release` | GUI | `bun run --cwd apps/gui e2e:live:release` |

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
