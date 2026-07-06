# Testing

Testing is split by risk type instead of hidden behind one giant test command.
Business tests stay deterministic; OS, live, performance, release, app, and
benchmark checks are separate on purpose.

The owner reference is [scripts/ARCHITECTURE.md](../../scripts/ARCHITECTURE.md).

## Main lanes

- `scripts/check-backend-quality.*` runs formatting, dependency policy, spelling,
  and backend quality gates.
- `scripts/run-ci.*` runs the local CI orchestration flow.
- `xtask/scripts/run-backend-business-tests.*` discovers deterministic backend
  business tests.
- `xtask/scripts/run-backend-os-tests.*` runs serial OS/process tests.
- `xtask/scripts/run-backend-live-tests.*` runs opt-in provider or network tests.
- App tests live under [apps/tui](../../apps/tui/README.md) and
  [apps/gui](../../apps/gui/README.md).

## Policy

- Do not mix live provider calls into deterministic business tests.
- Process-global tests should be serial and isolated.
- Benchmarks are not CI tests; see [Benchmark](benchmark.md).

## Related

- [Scripts](scripts.md)
- [Command run](../core/command-run.md)
- [Runtime architecture](../architecture/runtime.md)
- [Benchmark](benchmark.md)
