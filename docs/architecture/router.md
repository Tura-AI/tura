# Router

Router is the per-home daemon that owns session DB startup/adoption, runtime
worker dispatch, command-run execution routing, registry operations, and IPC
coordination.

Primary references:

- [crates/router/ARCHITECTURE.md](../../crates/router/ARCHITECTURE.md)
- [crates/router/README.md](../../crates/router/README.md)
- [root architecture binary topology](../../ARCHITECTURE.md#binary-topology-single-backend-pipeline-many-thin-fronts)

## Boundary

Router coordinates local backend ownership. It is not the user-facing API, not
the provider client, and not the agent loop.

## Related

- [Runtime](runtime.md)
- [Session DB](session-db.md)
- [Gateway](gateway.md)
