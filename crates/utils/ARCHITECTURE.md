# Utils Crate Architecture

`crates/utils` contains shared helper code that is intentionally generic. It
should not accumulate gateway, runtime, provider, router, or tools domain
behavior.

The Cargo package and library names are:

```text
package = utils
library = utils
```

## Current Layout

```text
crates/utils/
  Cargo.toml
  ARCHITECTURE.md
  src/
    lib.rs
    media_processor.rs
    md_manager.rs
    stream_text_processor.rs
```

## Ownership

Utils owns reusable media, Markdown, and streaming text helpers. Domain-specific
policies, API behavior, command execution, provider routing, session state, and
agent orchestration belong in their owning crates.
