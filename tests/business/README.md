# Business Benchmark Tests

This directory contains manual business benchmarks. They can launch real CLI
agents, call live providers, require private keys, consume quota, run browsers,
and write large artifacts.

These scripts are committed as benchmark assets, but they are not part of
GitHub CI or default `cargo test --workspace`. Do not make crate tests or CI
workflows read scripts from this root `tests/` tree as fixtures.

Business-test outputs default to:

```text
~/Documents/tura workspace/target/{test_name}/{run_id}/summary.json
```

Override the artifact root with `TURA_BUSINESS_TARGET_ROOT` or
`COMMAND_RUN_BUSINESS_TARGET_ROOT`.
