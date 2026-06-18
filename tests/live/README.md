# Live Tests

This directory contains opt-in workspace tests that may use provider credentials,
public network access, model quota, live gateway calls, or third-party services.
Keep files directly under `tests/live`; do not create child directories.

`tests/live`, `tests/business`, `tests/os_testing`, `tests/performance`,
`tests/benchmark`, and `tests/release` are peer test types. Local deterministic
tests belong in `tests/business`; process/OS-sensitive deterministic tests
belong in `tests/os_testing`; backend release-binary validation belongs in `tests/release`;
TUI/GUI release flows that drive real provider execution are live release tests
and use the `tui_release_*.mjs` or `gui_release_*.mjs` prefixes here. Scoring and
comparison suites belong in `tests/benchmark`; performance, load, soak, and
stress tests belong in `tests/performance`.

The kept entry points cover each required live surface without duplicate wrapper
scripts:

| Profile | Surface | Entry point |
| --- | --- | --- |
| `target/debug` | CLI | `node ./tests/live/media_internet_official_docs_search_smoke_harness.mjs` |
| `target/debug` | TUI | `npm --prefix apps/tui run test:live` |
| `target/release` | TUI | `npm --prefix apps/tui run test:live:release` |
| `target/release` | GUI | `bun run --cwd apps/gui e2e:live:release` |

Backend live runners ignore app-owned TUI/GUI scripts. TUI/GUI live release
scripts use root `tests/live/*_release_*.mjs` files and are run only through the
app package commands.

```powershell
npm --prefix apps\tui run test:live
npm --prefix apps\tui run test:live:snake
npm --prefix apps\tui run test:live:release
bun run --cwd apps\gui e2e:live:release
```

```bash
npm --prefix apps/tui run test:live
npm --prefix apps/tui run test:live:snake
npm --prefix apps/tui run test:live:release
bun run --cwd apps/gui e2e:live:release
```

Backend crate live tests are selected by directory scan:

```powershell
.\xtask\scripts\run-backend-live-tests.ps1 -List
.\xtask\scripts\run-backend-live-tests.ps1 -Crate provider -TimeoutSeconds 300
```

```bash
sh xtask/scripts/run-backend-live-tests.sh --list
sh xtask/scripts/run-backend-live-tests.sh --crate provider --timeout-seconds 300
```

Crate-owned live Rust tests keep their runnable entrypoints directly under
`<crate>/tests/live/`. Those entrypoints may use target-owned helper modules in
sibling subdirectories, for example `<crate>/tests/live/helpers/`, because Cargo
runs the top-level test target.

Backend release-binary tests live in `tests/release`; TUI/GUI release live tests
use the app package commands above and keep their scripts under this directory.
