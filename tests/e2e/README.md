# Backend E2E

This directory contains required local full-chain tests. Entrypoints are
top-level `*.mjs` files; reusable support modules end in `_fixture.mjs`.

The suite must use real local Tura backend processes while keeping providers
fully local and deterministic. Public provider endpoints, provider API keys,
OAuth state, paid services, and live network access are forbidden.

Run every backend E2E entrypoint with:

```powershell
.\xtask\scripts\run-backend-e2e-tests.ps1
```

```bash
sh xtask/scripts/run-backend-e2e-tests.sh
```
