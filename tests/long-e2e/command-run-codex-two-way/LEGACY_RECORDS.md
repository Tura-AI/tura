# Legacy Command-Run Records

Historical generated records and target output from the previous test layout
were moved out of `tests/` so the test tree only contains scripts and docs.

The archived generated records now live under:

```text
target/command-run-codex-two-way-records/
```

Nested target output from the old layout was merged into the repository root
`target/` directory.

Active scripts moved to:

- `tests/unit/command-run/`
- `tests/long-e2e/command-run-codex-two-way/`
