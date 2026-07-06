# Scripts

Scripts install dependencies, build debug/release artifacts, register CLI paths,
run local CI, and package release outputs.

The owner reference is [scripts/ARCHITECTURE.md](../../scripts/ARCHITECTURE.md).

## Common commands

```powershell
.\scripts\install.ps1
.\scripts\build-debug.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
.\scripts\run-ci.ps1
```

```bash
./scripts/install.sh
./scripts/build-debug.sh
./scripts/build-release.sh
./scripts/register-cli.sh
./scripts/run-ci.sh
```

## Rules

- Install scripts install dependencies only; build scripts create artifacts.
- Registration adds the release target directory to the user's PATH.
- Release packaging and npm packaging live under `scripts/npm/`.
- Test orchestration lives in script owners and typed test directories, not in
  ad hoc one-off shell snippets.

## Related

- [Install](../start/install.md)
- [Testing](testing.md)
- [Environment](environment.md)
- [Benchmark](benchmark.md)
