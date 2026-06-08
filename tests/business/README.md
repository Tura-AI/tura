# Business Tests

This directory is intentionally kept free of live business benchmark entry
scripts.

Tests that require real provider keys, authenticated CLI agents, live model
calls, browser/provider quota, or long-running external services belong under
`tests/business_old/`. Keeping those scripts outside this directory prevents
default CI and sandboxed `cargo test --workspace` runs from picking up tests
that cannot pass without private credentials.

Use `tests/business_old/README.md` for the archived live benchmark layout and
manual execution notes.
