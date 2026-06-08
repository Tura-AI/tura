# Legacy Business Benchmarks

This directory contains older manual benchmark scripts kept for reference and
comparison runs. Like `tests/business/`, these scripts can require private
keys, consume provider quota, write large artifacts, and depend on local agent
checkouts.

The entire root `tests/` tree is intentionally excluded from default CI and
default `cargo test --workspace`. Run these scripts manually only in an
environment that has the required credentials and agent binaries.

Business-test outputs default to:

```text
~/Documents/tura workspace/target/{test_name}/{run_id}/summary.json
```

Override the artifact root with `TURA_BUSINESS_TARGET_ROOT` or
`COMMAND_RUN_BUSINESS_TARGET_ROOT`.

Common agent selection variables include:

- `COMMAND_RUN_AGENT_AGENTS`
- `COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT`
- `COMMAND_RUN_AGENT_CODEX_MAIN_ROOT`
- `COMMAND_RUN_AGENT_TURA_MODEL`
- `COMMAND_RUN_AGENT_CODEX_MODEL`
- `COMMAND_RUN_AGENT_CLAUDE_MODEL`
- `COMMAND_RUN_AGENT_PI_EXE`
- `COMMAND_RUN_AGENT_TIMEOUT_MS`
