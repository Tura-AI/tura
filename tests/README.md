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

`tests/business/` is reserved for non-live business test documentation and must
not contain entry scripts that require private provider keys, authenticated CLI
agents, live model calls, or long-running external services.

Archived live business benchmarks live under `tests/business_old/`. They are
manual-only and excluded from default CI because they can consume provider
quota, require local agent checkouts, and write large artifacts. Their outputs
default to `~/Documents/tura workspace/target/{test_name}/{run_id}/summary.json`.

See `tests/business_old/README.md` for manual execution notes.

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
