# Tests

Test scripts are split by runtime cost and blast radius.

## Crate-Owned E2E And Contract Scripts

Focused command-run CLI flows now live with the gateway crate under
`crates/gateway/tests/e2e/command-run/`. Cross-crate command contracts that
aggregate Cargo tests live under `crates/tools/tests/contracts/`. The TUI
gateway CLI fixture lives under `apps/tui/e2e/`.

Multi-agent dispatch (router-CLI subprocess + concurrent + 2-level recursive
sub-sessions) is covered by `crates/runtime/tests/child_dispatch_test.rs`
against an in-package mock router binary (`mock_router_for_test`). It
verifies the runtime never opens a URL/HTTP channel to the router — all
internal runtime ↔ router traffic is CLI stdin/stdout JSON.

## Business Tests

`tests/business/command-run-agent-benchmarks/` contains long-running business
benchmarks that spawn real agents and compare Tura with Codex variants. These
can take minutes, consume provider quota, and write large run outputs under
`target/`.

`tests/business/tui_real_gateway_business_test.mjs` is the TUI business flow
coverage. It starts the real `target/debug/gateway(.exe)` and runs the TUI CLI
and three web-terminal profiles against that gateway. It does not use a mock
gateway; set `TUI_BUSINESS_LIVE_PROMPT=1` when you also want the business run to
spend provider quota and require a real model reply.

Historical generated command-run records from the old layout now live under
`target/command-run-codex-two-way-records/`.

## Inspecting Logs In Tests

Tests and benchmark scripts should query Tura session history through
`session_log`, not by reading `.tura/sessions/*.json`.

```powershell
'{"command":"get_session","session_id":"session-id"}' | target\debug\gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\gateway.exe session-log
```

Provider-call diagnostics are separate files under
`log/provider/YYYY-MM-DD/*.json` by default, or under `LOG_PATH` when that
environment variable is set. Use provider logs for model request/response
debugging and session_log for session/task/message assertions.
