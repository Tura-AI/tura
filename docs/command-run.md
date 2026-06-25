# Command Run

`command_run` is Tura's model-visible execution surface. The provider sees one
tool, while Tura routes each item inside the batch to internal commands such as
`shell_command`, `bash`, `zsh`, `apply_patch`, `read_media`, `web_discover`,
`generate_media`, `task_status`, and optional `planning`.

The source of truth starts here:

- [command_run schema](../crates/tools/src/command_run/schema.json)
- [command_run handler](../crates/tools/src/command_run/handler.rs)
- [provider schema injection](../crates/runtime/src/manas/tool_catalog.rs)
- [streamed command-run extraction](../crates/runtime/src/provider_flow/streamed_command_run.rs)
- [tools crate architecture](../crates/tools/ARCHITECTURE.md)

## Tool Call vs Command Run

Normal tool-call design:

1. Expose many tools and their schemas to the model.
2. Ask the model to choose one or more direct calls.
3. Return one callback per call.
4. Repeat the full provider loop as the agent discovers each next step.

Tura's command-run design:

1. Expose one compact `command_run` schema.
2. Put several already-known commands in one batch.
3. Use `step` as a dependency group: same-step reads can run together, later
   steps wait for earlier steps.
4. Normalize all results into one command-run result shape.
5. Stream progress for UI/CLI without forcing every internal command to become
   a separate provider-visible tool call.

That is why command-heavy tasks can use dramatically fewer tokens. Direct
multi-tool loops spend tokens on repeated schemas, repeated tool call wrappers,
repeated assistant narration, and repeated callback history. `command_run`
keeps the provider surface stable and small, then lets the local runtime do the
scheduling.

The 70%+ token-saving claim should be read as the target and recurring shape of
command-heavy benchmark wins, not a universal law. The benchmark harnesses below
record the numbers needed to verify it for a concrete task, model, and provider:
input tokens, cached tokens, output tokens, provider time, wall time, command
executions, and artifacts.

## What The Model Sees

The core schema is intentionally small:

```json
{
  "name": "command_run",
  "description": "Run tools as a pure batch+step command runner.",
  "input_schema": {
    "required": ["commands"],
    "properties": {
      "commands": {
        "type": "array",
        "minItems": 5,
        "maxItems": 20
      }
    }
  }
}
```

At runtime, Tura injects the actual allowed `command_type` enum for the active
agent and task. This matters: the model does not get an abstract blob of every
possible command. It gets the current command set, the active shell surface, and
compact command-specific format lines.

## What The Runtime Does

Inside the tools crate, command_run handles work that would otherwise be left to
model luck:

- Parses loose provider arguments into current command shape.
- Canonicalizes aliases such as shell names and legacy command fields.
- Preserves step dependency groups and repairs backwards step numbers.
- Runs same-step read-only macro commands concurrently when safe.
- Serializes mutating commands through file locks.
- Stops later commands after failed `apply_patch`.
- Normalizes command results into one stable output envelope.
- Supports streamed command execution while the provider is still producing
  content.
- Rejects commands outside the active agent capability set.

Relevant tests:

- [command_run_current_flow](../crates/tools/tests/business/command_run_current_flow.rs)
- [command shapes](../crates/tools/tests/business/command_run_current/command_shapes.rs)
- [apply_patch and file locks](../crates/tools/tests/business/command_run_current/apply_patch_streaming_locks.rs)
- [command-run pressure test](../crates/tools/tests/performance/command_run_pressure_test.rs)

## Why It Is Faster

`command_run` reduces latency in three places:

- Same-step discovery commands can execute together instead of one provider turn
  at a time.
- The provider does not need to decide among a large tool menu on every turn.
- Streaming command-run support starts local command execution as command items
  become available, rather than waiting for the final response body.

It also reduces wasted turns. The schema tells the model to put all currently
known reads, patches, checks, and status updates in one batch. The runtime then
enforces the ordering rules locally.

## Benchmark Entrypoints

The benchmark harnesses live under [tests/benchmark](../tests/benchmark/README.md).
Good starting points:

- [PDF cost comparison](../tests/benchmark/media-presentation/ogas_pdf_cost_comparison.mjs)
- [source-port benchmark](../tests/benchmark/project-rebuild-refactor/rust_cli_python_port_suite.mjs)
- [defined-workflow source-port benchmark](../tests/benchmark/project-rebuild-refactor/rust_cli_python_port_suite_defined_workflow.mjs)
- [apply_patch command benchmark](../tests/benchmark/commands/apply_patch_single_block_contract_harness.mjs)

The benchmark scripts are intentionally outside default CI because they can
launch real agents, consume provider quota, and write large artifacts.
