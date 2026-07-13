# Live Tests

This directory contains the tests that leave the building. They are opt-in and
may use provider credentials, public network access, model quota, live gateway
calls, or third-party services. Keep files directly under `tests/live`; do not
create child directories.

`tests/live`, `tests/business`, `tests/os_testing`, `tests/performance`,
`benchmark`, and `tests/release` are peer test types. Local deterministic
tests belong in `tests/business`; process/OS-sensitive deterministic tests
belong in `tests/os_testing`; all release-binary validation belongs in
`tests/release`, including TUI/GUI release flows that drive real provider
execution. Scoring and comparison suites belong in `benchmark`;
performance, load, soak, and stress tests belong in `tests/performance`.

The entry points below cover each required live surface without growing a second
set of wrapper scripts. One route to a live failure is quite enough:

| Profile | Surface | Entry point |
| --- | --- | --- |
| `target/debug` | CLI | `node ./tests/live/media_internet_official_docs_search_smoke_harness.mjs` |
| `target/debug` | TUI | `npm --prefix apps/tui run test:live` |
| `target/release` | TUI | `node ./tests/release/tui_release_run_all.mjs` |
| `target/release` | GUI | `node ./tests/release/gui_release_run_all.mjs` |

Backend live runners deliberately ignore release-binary scripts. Use
`tests/release` or the app package aliases for release surfaces; a live provider
call and a packaged binary are different claims.

Dedicated compact context live coverage:

```powershell
node .\tests\live\compact_context_live_harness.mjs
```

```bash
node ./tests/live/compact_context_live_harness.mjs
```

The compact live harness defaults to one compact-resume flow: one turn explicitly
finishes a `command_run` batch with `compact_context`, and the next turn verifies
the compacted handoff can still run tools and backfill results. Each turn gets
one model/tool attempt, defaults to a 30s timeout, and the harness stops on the
first failed assertion.

Set `COMMAND_RUN_COMPACT_LIVE_SCENARIOS=auto` to run only the automatic
threshold scenario, or `COMMAND_RUN_COMPACT_LIVE_SCENARIOS=all` to run both
flows. The automatic scenario uses `TURA_CONTEXT_LIMIT_TOKENS` through the
harness default `COMMAND_RUN_COMPACT_AUTO_CONTEXT_LIMIT_TOKENS=16000`, so it
exercises the same runtime injection path without sending a provider-scale
250k-token live request. Use `COMMAND_RUN_COMPACT_AUTO_TARGET_TOKENS` to tune
the long payload, `COMMAND_RUN_COMPACT_AUTO_CONTEXT_LIMIT_TOKENS` to tune the
active context threshold, or `COMMAND_RUN_COMPACT_AUTO_MODE=hard-over-limit` for
a provider hard-limit stress variant.

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

Release-binary tests live in `tests/release`; this live directory should not
contain release-entry wrappers.
