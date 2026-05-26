# Tests

Test scripts are split by runtime cost and blast radius.

## Unit And Probe Scripts

`tests/unit/command-run/` contains focused command-run probes. These scripts are
intended to validate one behavior at a time, such as compact-context prompt
coverage, single-round command execution, streaming command dispatch, provider
fallback parsing, `read_media` handling, and the `multiple_tasks` backend
topology/ordering/session-derivation contract.

## Long E2E Scripts

`tests/long-e2e/command-run-codex-two-way/` contains long-running benchmarks that
spawn real agents and compare Tura with Codex variants. These can take minutes,
consume provider quota, and write large run outputs under `target/`.

Historical generated command-run records from the old layout now live under
`target/command-run-codex-two-way-records/`.
