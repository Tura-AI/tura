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
.\scripts\run-backend-os-tests.ps1 -List
.\scripts\run-backend-os-tests.ps1
```

```bash
sh scripts/run-backend-os-tests.sh --list
sh scripts/run-backend-os-tests.sh
```
