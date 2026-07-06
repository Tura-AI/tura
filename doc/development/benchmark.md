# Benchmark

Benchmarks are manual long-horizon comparison suites for real agents and real
task workspaces. They can consume provider quota, clone or rebuild fixtures,
launch external CLIs, and write large artifacts.

The owner reference is [benchmark/README.md](../../benchmark/README.md).

## Scope

- Build tasks cover new artifact generation and app-building workflows.
- Debug tasks cover reproduction, repair, and verification workflows.
- Refactoring tasks cover rebuild, port, and compatibility work.
- Reports normalize CLI metadata, agent rounds, task reports, harness reports,
  token usage, command usage, and scored outcomes.

## Policy

- Benchmarks are not default CI.
- Benchmark tasks must declare their contract with `benchmark.task.json`.
- Agent executable and model overrides use `COMMAND_RUN_AGENT_*` environment
  variables.

## Related

- [Testing](testing.md)
- [Environment](environment.md)
- [Command run](../core/command-run.md)
- [Runtime architecture](../architecture/runtime.md)
