# Live Tests

This directory contains opt-in workspace tests that may use provider credentials,
public network access, model quota, live gateway calls, or third-party services.
Keep files directly under `tests/live`; do not create child directories.

`tests/live`, `tests/business`, `tests/performance`, `tests/benchmark`, and
`tests/release` are peer test types. Local deterministic tests belong in
`tests/business`; release-binary validation belongs in `tests/release`; scoring
and comparison suites belong in `tests/benchmark`; performance, load, soak, and
stress tests belong in `tests/performance`.

The kept entry points cover each required live surface without duplicate wrapper
scripts:

| Profile | Surface | Entry point |
| --- | --- | --- |
| `target/debug` | CLI | `node ./tests/live/media_internet_official_docs_search_smoke_harness.mjs` |
| `target/debug` | TUI | `npm --prefix apps/tui run test:live` |

Backend live runners ignore app-owned TUI/GUI scripts. TUI live scripts live
under `apps/tui/e2e/live/` and are run only through the app package commands.

```powershell
npm --prefix apps\tui run test:live
npm --prefix apps\tui run test:live:snake
```

```bash
npm --prefix apps/tui run test:live
npm --prefix apps/tui run test:live:snake
```

Backend crate live tests are selected by directory scan:

```powershell
.\scripts\run-backend-live-tests.ps1 -List
.\scripts\run-backend-live-tests.ps1 -Crate provider -TimeoutSeconds 300
```

```bash
./scripts/run-backend-live-tests.sh --list
./scripts/run-backend-live-tests.sh --crate provider --timeout-seconds 300
```

Release-binary tests live in `tests/release`; see `tests/release/README.md`.
