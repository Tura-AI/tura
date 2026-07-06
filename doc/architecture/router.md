# Router

Router is the per-home daemon that owns backend dispatch: session_db startup,
runtime worker launches, command-run execution ownership, registry operations,
and IPC routing.

Primary references:

- [crates/router/ARCHITECTURE.md](../../crates/router/ARCHITECTURE.md)
- [crates/router/README.md](../../crates/router/README.md)
- [root architecture binary topology](../../ARCHITECTURE.md#binary-topology-single-backend-pipeline-many-thin-fronts)

## Role

- Starts or adopts `tura_session_db`.
- Dispatches per-session `tura_runtime` workers.
- Owns router-scoped command execution and cancellation.
- Serves registry and service-management requests.

## Related

- [Runtime](runtime.md)
- [Session DB](session-db.md)
- [Gateway](gateway.md)
