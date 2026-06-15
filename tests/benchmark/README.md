# Benchmarks

This directory contains manual benchmark, comparison, and scoring suites. These
scripts can launch real agents, clone or rebuild external fixtures, run browser
evaluators, consume provider quota, and write large artifacts.

Benchmarks are not part of GitHub CI or default `cargo test --workspace`.
Crate-owned correctness tests belong under the owning crate, and release-entry
validation belongs under `tests/release/` or the app-local `e2e/business/`
directories. Rust business, OS, and performance tests belong under
`crates/*/tests/business/`, `crates/*/tests/os_testing/`, and
`crates/*/tests/performance/`.

Workspace benchmark scripts keep their historical second-level categories:

```text
tests/benchmark/bug-fix/
tests/benchmark/frontend-playwright/
tests/benchmark/lib/
tests/benchmark/project-rebuild-refactor/
tests/benchmark/tui/
```

Shared benchmark helper re-exports live under `tests/benchmark/lib/`.
