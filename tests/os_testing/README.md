# OS Testing

`tests/os_testing/` contains local tests that own backend processes, sockets,
locks, service lifecycles, shutdown behavior, or cross-OS policy. These tests
are gated by `os-tests` and run serially.

Use this category for router/session_db ownership, gateway front leases,
runtime worker lifecycle, command_run process-tree cleanup, and OS policy
matrix coverage. Session_db or router stress tests that open IPC services or
local sockets also belong here so they run serially. Keep ordinary business
behavior in `tests/business/`.

```powershell
.\xtask\scripts\run-backend-os-tests.ps1 -List
.\xtask\scripts\run-backend-os-tests.ps1
.\tests\os_testing\local\run-windows.ps1 -List
.\tests\os_testing\local\run-install-release-windows.ps1
```

```bash
sh xtask/scripts/run-backend-os-tests.sh --list
sh xtask/scripts/run-backend-os-tests.sh
sh tests/os_testing/local/run-linux.sh --list
sh tests/os_testing/local/run-macos.sh --list
sh tests/os_testing/local/run-install-release-linux.sh
sh tests/os_testing/local/run-install-release-macos.sh
```

GitHub Actions uses the matching `tests/os_testing/actions/run-*.{sh,ps1}` and
`run-install-release-*.{sh,ps1}` wrappers so the four OS runners share the same
install, release, and backend OS test contracts while still keeping OS-specific
entrypoints explicit. The runner-label wrappers are `run-ubuntu-latest.sh`,
`run-macos-latest.sh`, `run-windows-2022.ps1`, and `run-windows-2025.ps1`, with
matching `run-install-release-*` wrappers.

To conserve Actions quota while debugging, target the OS workflow before running
the final matrix. Push a `codex/**` branch or commit message containing
`os-install`, `os-backend`, `os-tui`, or `os-full`; add `windows`,
`windows-2022`, `windows-2025`, `linux`, or `macos` to narrow the runner set.
Final validation should use `os-full` so install-release, backend OS, and TUI
lifecycle all run.
