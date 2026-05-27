# Tests

Test scripts are split by runtime cost and blast radius.

## Crate-Owned E2E And Contract Scripts

Focused command-run CLI flows now live with the gateway crate under
`crates/gateway/tests/e2e/command-run/`. Cross-crate command contracts that
aggregate Cargo tests live under `crates/tools/tests/contracts/`. The TUI
gateway CLI fixture lives under `apps/tui/e2e/`.

## Business Tests

`tests/business/command-run-agent-benchmarks/` contains long-running business
benchmarks that spawn real agents and compare Tura with Codex variants. These
can take minutes, consume provider quota, and write large run outputs under
`target/`.

Historical generated command-run records from the old layout now live under
`target/command-run-codex-two-way-records/`.
