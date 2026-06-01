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

Historical generated command-run records from the old layout now live under
`target/command-run-codex-two-way-records/`.
