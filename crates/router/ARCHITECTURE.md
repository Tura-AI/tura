# Router Crate Architecture

`crates/router` owns CLI forwarding, command registration metadata, and managed
local service/process lifecycle. It does not own command implementation or port
allocation.

The Cargo package and default binary name should stay compatible with Tura:

```text
package = tura_router
default binary = tura_router
```

## Layout

```text
crates/router/
  src/
    main.rs
    lib.rs

    registry/
      command_registry.rs
      aliases.rs
      health.rs

    lifecycle/
      manager.rs
      managed_process.rs
      cleanup.rs
      restart.rs

    monitor/
      status.rs
      health_check.rs
      heartbeat.rs

    routes/
      forward_cli.rs
      resolve_command.rs
      status.rs

    clients/
      runtime_client.rs
      tools_client.rs
      memory_client.rs

    security/
      permission_forwarder.rs
      sandbox_profile.rs
      network_policy.rs

    events/
      runtime_events.rs
      command_events.rs

    utils/
```

## Responsibilities

Router owns:

- Command registry.
- Command aliases.
- CLI forwarding rules.
- Runtime/tool command routing metadata.
- Managed service/process startup and shutdown.
- Managed service status monitoring.
- Health checks that do not depend on port allocation.
- Restart and cleanup policy for router-managed processes.
- Permission forwarding for routed command actions.
- Route status and command-resolution diagnostics.

Router does not own:

- Agent loops.
- Prompt assembly.
- Provider request formatting.
- Command handler logic.
- Shell execution.
- File locks.
- Memory/vector behavior.
- Port allocation.

## Command Registration

Every routed command needs:

- `command_id`
- aliases
- owning crate path
- handler or binary target
- CLI argument schema
- startup mode
- health check
- default timeout
- restart policy
- permission scope
- stdio strategy

Registered command metadata lives in `crates/router`, not under
`crates/tools/src/command_run`.

## Lifecycle Management

Router is responsible for pulling up managed local services or processes when a
routed command needs them. A managed service can be a crate binary, script,
stdio process, or in-process task. It should not require a fixed port.

Lifecycle records should include:

- service id
- owning crate or script path
- startup command
- environment contract
- readiness check
- health check
- stop strategy
- restart policy
- status event shape

Router tracks status for each managed process and exposes it through `status`
and command events.

## CLI Forwarding

Typical routes or internal calls:

- `resolve_command`
- `forward_cli`
- `status`

The exact transport can be in-process, stdio, or a local CLI process. The
architecture rule is that router resolves command requests and owns lifecycle
for any managed process needed to serve them, while the owning crate or
`crates/tools/src/commands/<command>` owns behavior.

Example local path:

```text
cargo run -p tura_router -- forward shell_command -- rg "pattern" crates
```

## Memory Boundary

Memory behavior lives under `crates/memory`. Router can start and monitor a
memory-backed managed process when needed, but the implementation stays in
`crates/memory` and must not require a fixed port or a separate `services/`
directory.

## Checks

Use:

```text
cargo fmt -p tura_router
cargo check -p tura_router
```
