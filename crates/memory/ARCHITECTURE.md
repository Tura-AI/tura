# Memory Crate Architecture

`crates/memory` documents the intended memory and recall boundary. In the
current tree this directory is documentation-only: it has no `Cargo.toml`, no
`src/`, and is not a workspace member.

## Layout

```text
crates/memory/
  ARCHITECTURE.md
```

## Responsibilities

When implemented, memory owns:

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

There is no memory package check yet. Use documentation review until a Cargo
package is added to the workspace.
