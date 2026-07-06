# Full-chain E2E stress tests

The backend owner entrypoint is `tests/performance/full_chain_e2e_stress.mjs`.
It starts the local OpenAI-compatible provider plus real
gateway/router/runtime/session_db binaries, creates the stress session set, and
verifies backend visibility through `/session-log`, `/session/{id}`, and
`/session/{id}/message`.

Frontend owner entrypoints live in their app directories and are independent
scripts, not phase wrappers:

- Gateway/backend: `cargo test -p gateway --features performance-tests --test full_chain_e2e_stress -- --nocapture`
- GUI: `npm run test:performance:full-chain` from `apps/gui`
- TUI: `npm run test:performance:full-chain` from `apps/tui`

GUI and TUI scripts create the same backend stress environment, keep background
session message reads active, then open exactly two target sessions by default.
Their summaries report per-session read time, render time, total open time,
frame count, FPS, and frame-gap metrics, plus average metrics across the two
opened sessions. The frontend gates assert on those averages instead of
requiring every frontend page or every session to open within a short window.

Default workload:

- `TURA_FULL_CHAIN_WORKSPACES=10`
- `TURA_FULL_CHAIN_TASKS_PER_WORKSPACE=20`
- `TURA_FULL_CHAIN_TURNS_PER_SESSION=5`
- `TURA_FULL_CHAIN_LIVE_SESSIONS=20`

That creates 200 sessions and 2000 rich user/assistant records. Live sessions
exercise provider/runtime turns; historical sessions are written through the
session_db IPC protocol and read back through gateway APIs.

Useful smoke overrides:

```powershell
$env:TURA_FULL_CHAIN_WORKSPACES='1'
$env:TURA_FULL_CHAIN_TASKS_PER_WORKSPACE='2'
$env:TURA_FULL_CHAIN_TURNS_PER_SESSION='1'
$env:TURA_FULL_CHAIN_LIVE_SESSIONS='1'
$env:TURA_FULL_CHAIN_E2E_RUN_ID='smoke-full-chain'
node tests/performance/full_chain_e2e_stress.mjs
```

Useful setup command when artifacts are missing:

```powershell
$env:TURA_FULL_CHAIN_ENSURE_BUILDS='1'
$env:TURA_FULL_CHAIN_WORKSPACES='1'
$env:TURA_FULL_CHAIN_TASKS_PER_WORKSPACE='2'
$env:TURA_FULL_CHAIN_TURNS_PER_SESSION='1'
$env:TURA_FULL_CHAIN_LIVE_SESSIONS='1'
node tests/performance/full_chain_e2e_stress.mjs
```

Frontend-specific budgets:

- GUI: `TURA_FULL_CHAIN_GUI_OPEN_BUDGET_MS` defaults to `3000`, `TURA_FULL_CHAIN_GUI_MIN_AVG_FPS` defaults to `30`, plus `TURA_FULL_CHAIN_GUI_READ_BUDGET_MS` and `TURA_FULL_CHAIN_GUI_RENDER_BUDGET_MS`
- TUI: `TURA_FULL_CHAIN_TUI_OPEN_BUDGET_MS` defaults to `6000`, `TURA_FULL_CHAIN_TUI_MIN_AVG_FPS` defaults to `30`, plus `TURA_FULL_CHAIN_TUI_READ_BUDGET_MS` and `TURA_FULL_CHAIN_TUI_RENDER_BUDGET_MS`
- Shared frontend pressure: `TURA_FULL_CHAIN_FRONTEND_MEASURED_SESSIONS`, `TURA_FULL_CHAIN_FRONTEND_READ_CONCURRENCY`, `TURA_FULL_CHAIN_FRONTEND_READ_REQUESTS`

Every run writes `target/full-chain-e2e-stress/<run-id>/summary.json`, logs, and
the relevant frontend screenshot when a GUI/TUI owner script is used.
