# Memory Crate Architecture

`crates/memory` owns memory and recall behavior as a crate-level implementation
boundary. It is not an independent service directory.

## Layout

```text
crates/memory/
  src/
    lib.rs
    memory/
    registry/
    session/
    vector_store.rs
    embedding.rs
  tests/
  examples/
```

## Responsibilities

Memory owns:

- Long-lived memory store behavior.
- Vector or registry-backed recall when enabled.
- Memory health and persistence.
- Memory-specific tests and examples.

Memory does not own:

- Runtime session orchestration.
- Provider calls.
- Command routing.
- Tool file locks or sandboxing.
- Independent service startup.

Runtime and tools call memory through explicit clients or memory-backed
commands. Router may start and monitor a memory-backed managed process when a
command needs it, but the memory implementation stays in this crate and does
not move back into a separate service directory.

## Checks

Use the local package name once implementation is added. Until then, keep checks
aligned with the workspace package table in the root architecture document.
