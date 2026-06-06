# Runtime Refactor Requirements

Status: implemented  
Scope: step 2 of 3  
Related: [step1.gateway-router-session-db-refactor.md](step1.gateway-router-session-db-refactor.md), [step3.tools-refactor.md](step3.tools-refactor.md)

## Goal

Split the current runtime code into clear session bootstrap, turn execution, checkpoint, provider, tool-flow, and finalization modules. Reduce `mano` and `manas` bloat. Runtime should execute a busy session turn as a worker owned by router, emit command-level checkpoints through `SessionDbClient`, and avoid owning durable process or session state.

No single source file should exceed 1000 lines. If an existing file is too large, keep it as a small entry/facade and move logic into focused modules.

## Current Code Reality

Runtime has been split into focused boundaries:

- `crates/runtime/src/mano/process.rs` is the public entry facade.
- `crates/runtime/src/session_bootstrap/` handles persisted session loading, turn preparation, and initial messages.
- `crates/runtime/src/manas/process.rs` owns the MANAS loop facade.
- `crates/runtime/src/tool_flow/` handles tool execution, permissions, command-run result normalization, and task status transitions.
- `crates/runtime/src/provider_flow/` owns provider calls plus streamed command execution handling.
- `crates/runtime/src/checkpoint/` persists session snapshots through `SessionLogClient`.

The old execution/session/persistence coupling is now split across those modules.

## New Runtime Ownership

Runtime owns:

- Loading one session turn input.
- Building context for provider calls.
- Running the agent/provider loop.
- Executing core tools through `code-tools`.
- Resolving non-core tool executable metadata through router registry and launching hidden command CLIs through the tools facade.
- Emitting checkpoints to session DB.
- Producing final turn state.

Runtime does not own:

- Runtime worker lifecycle.
- Router process lifecycle.
- Router-owned browser or long-lived service worker lifecycle.
- Durable queue ownership.
- Gateway frontend projection.
- Long-lived session execution registry.

## Proposed Directory Layout

Target layout:

```text
crates/runtime/src/
  lib.rs

  session_bootstrap/
    mod.rs
    load.rs
    prepare_turn.rs
    persisted.rs
    initial_messages.rs

  session_state/
    mod.rs
    model.rs
    task_plan.rs
    transitions.rs
    user_turn.rs

  turn_loop/
    mod.rs
    driver.rs
    provider_step.rs
    tool_step.rs
    retry_policy.rs
    no_tool_policy.rs
    task_progress.rs
    finalization.rs

  checkpoint/
    mod.rs
    client.rs
    event.rs
    command_run.rs
    session_snapshot.rs
    idempotency.rs

  provider_flow/
    mod.rs
    call.rs
    streamed_command_run.rs
    usage.rs
    errors.rs

  tool_flow/
    mod.rs
    execute.rs
    command_run_result.rs
    permission.rs
    changes.rs
    task_status.rs

  gateway_events/
    mod.rs
    agent_message.rs
    tool_message.rs
    progress.rs

  context/
  prompt_style/
  runtime/
  manas/
  mano/
```

During migration, `mano` and `manas` may remain as compatibility facades, but new logic should move to the focused modules.

## Mano Boundary

`mano` becomes a compatibility/bootstrap layer only.

Allowed responsibilities:

- Load persisted session from session DB.
- Prepare a new user turn.
- Create initial `SessionManagement` for legacy callers.
- Bridge old public APIs to new `session_bootstrap` and `turn_loop`.

Disallowed responsibilities:

- Running the full agent loop.
- Provider retry policy.
- Tool execution.
- Checkpoint timing.
- Runtime worker lifecycle.
- Gateway callback orchestration.

Current shape:

- Persisted session read/write helpers live in `session_bootstrap/persisted.rs` and `checkpoint/session_snapshot.rs`.
- `mano/process.rs` is a small entrypoint that calls session bootstrap helpers and MANAS processing.

## Manas Boundary

`manas` currently acts as the execution brain. It should be reduced to a facade, then gradually replaced by `turn_loop`, `tool_flow`, `checkpoint`, and `gateway_events`.

Allowed responsibilities after migration:

- Re-export or bridge legacy execution calls.
- Hold agent-loop compatibility naming during transition.

New homes:

- Provider retry and no-tool retry: `turn_loop/retry_policy.rs`, `turn_loop/no_tool_policy.rs`.
- Tool calls: `tool_flow/execute.rs`.
- Task status/session task transitions: `tool_flow/task_status.rs` and `session_state/task_plan.rs`.
- Final response: `turn_loop/finalization.rs`.
- Gateway publish functions: `gateway_events/`.
- Persist checkpoints: `checkpoint/`.

## Checkpoint Requirements

Runtime must emit checkpoints through `SessionDbClient`; it should not directly open SQL or rely only on whole-turn snapshot writes.

Required checkpoint moments:

```text
turn_started
provider_call_started
command_run_started
command_ready
command_started
command_finished
command_failed
command_run_finished
provider_call_finished
turn_finished
turn_failed
turn_interrupted
```

`checkpoint/event.rs` defines serializable checkpoint types.

`checkpoint/idempotency.rs` creates keys from:

```text
session_id
turn_id
runtime_worker_id
command_run_id
command_id
event_seq
event_type
```

Command safety rule:

```text
Mutating command_finished must be acknowledged by session DB before runtime continues.
```

Read-only commands may be batched, but `command_run_finished` must flush them.

## Streamed Command Run Safety

Existing streamed command behavior must be made durable earlier.

Current issue:

- Streamed commands may execute before provider call finishes.
- If provider fails or runtime dies before `tool_results` checkpoint, the next agent may not see executed command results.

Required behavior:

- On every streamed command completion, emit `command_finished`.
- On provider timeout/failure after streamed commands completed, synthesize a `command_run_finished` checkpoint with collected results.
- If checkpoint ACK fails after a mutating command, stop the turn and report checkpoint failure.

Target module:

```text
provider_flow/streamed_command_run.rs
checkpoint/command_run.rs
tool_flow/command_run_result.rs
```

## Session State Model

Move session state transition helpers out of long process files.

Target files:

```text
session_state/model.rs
session_state/transitions.rs
session_state/task_plan.rs
session_state/user_turn.rs
```

Responsibilities:

- `SessionManagement` construction and update helpers.
- Running/idle/failed/cancelled transition rules.
- Task plan updates from `task_status` and `planning`.
- Active task selection.
- User turn preparation.

## Turn Loop

`turn_loop/driver.rs` owns the high-level loop:

```text
prepare turn
checkpoint turn_started
loop:
  provider step
  checkpoint provider_call_finished or failed
  tool step if any
  checkpoint command/tool results
  decide next message or finalization
checkpoint turn_finished
```

The driver should delegate:

- Provider call to `provider_step.rs`.
- Tool handling to `tool_step.rs`.
- Retry decisions to `retry_policy.rs`.
- No-tool behavior to `no_tool_policy.rs`.
- Task progress to `task_progress.rs`.
- Final output to `finalization.rs`.

## Tool Flow

Tool execution logic lives in focused files:

```text
tool_flow/execute.rs
tool_flow/permission.rs
tool_flow/command_run_result.rs
tool_flow/task_status.rs
```

`tool_flow/execute.rs` should remain under 1000 lines and orchestrate only. Detailed mutation of session/task state must live in specialized modules.

For non-core tools, `tool_flow` calls the tools facade. The facade may query router for registry metadata and resolved binary paths, then runtime/tools launches the hidden CLI process directly. Non-core command processes are not children of router in this phase.

## Gateway Events

Move gateway callback and UI progress publishing to:

```text
gateway_events/agent_message.rs
gateway_events/tool_message.rs
gateway_events/progress.rs
```

These modules publish visible UI events but do not define durable state truth. Durable truth comes from session DB checkpoints.

## Runtime Worker Entry

`crates/gateway/src/runtime_worker.rs` currently hosts runtime worker mode inside gateway binary. After router owns worker lifecycle, runtime worker hosting should be moved or made explicit.

Options:

1. Keep gateway binary role temporarily for compatibility, but router is the only process that starts it.
2. Add a dedicated runtime worker binary package later.

In either case, gateway must not start it directly.

## File Size And Structure Rules

- No new source file over 1000 lines.
- Existing large files should become facades. This applies to old runtime files too, not only new modules.
- Do not add new behavior to `mano/process.rs`, `manas/process.rs`, or `runtime/call_runtime.rs` unless the behavior is first extracted behind a focused module boundary.
- Do not split by arbitrary chunks.
- Split by responsibility and state-machine phase.
- New modules must include narrow unit tests where practical.

High-risk existing files to split:

- `crates/runtime/src/manas/process.rs`
- `crates/runtime/src/runtime/call_runtime.rs`
- `crates/runtime/src/context/build.rs`
- `crates/runtime/src/gateway_events/`

## Detailed Runtime Split Rules

The current `mano` and `manas` code should be split by state-machine responsibility.

### `mano` Migration

`mano` should become a bootstrap facade. It may expose old public entrypoints, but implementation must move to:

```text
session_bootstrap/load.rs
session_bootstrap/prepare_turn.rs
session_bootstrap/persisted.rs
session_bootstrap/initial_messages.rs
```

Move responsibilities:

- Persisted session loading from `mano/process.rs` to `session_bootstrap/load.rs`.
- New turn preparation from `mano/process.rs` to `session_bootstrap/prepare_turn.rs`.
- Session DB snapshot compatibility through `checkpoint/session_snapshot.rs`.
- Initial context/message construction to `session_bootstrap/initial_messages.rs`.

`mano` must not contain:

- Provider retry loops.
- Tool execution.
- Checkpoint timing.
- Runtime worker lifecycle.
- Gateway/router process calls.

### `manas` Migration

`manas` should stop being the catch-all execution module. Its current responsibilities should move to:

```text
turn_loop/driver.rs
turn_loop/provider_step.rs
turn_loop/tool_step.rs
turn_loop/retry_policy.rs
turn_loop/no_tool_policy.rs
turn_loop/task_progress.rs
turn_loop/finalization.rs
tool_flow/
checkpoint/
gateway_events/
```

Move responsibilities:

- Main agent loop from `manas/process.rs` to `turn_loop/driver.rs`.
- Provider timeout/media/no-tool retry logic to `turn_loop/retry_policy.rs` and `turn_loop/no_tool_policy.rs`.
- `task_status` progression and active-task selection to `turn_loop/task_progress.rs` and `tool_flow/task_status.rs`.
- Tool execution orchestration to `tool_flow/execute.rs`.
- Gateway-visible progress publishing to `gateway_events/`.
- Session DB checkpoint emission to `checkpoint/`.

`manas/process.rs` should become a thin compatibility entrypoint and should not exceed 300 lines after migration.

### Provider Flow Migration

`runtime/call_runtime.rs` should be split into:

```text
provider_flow/call.rs
provider_flow/streamed_command_run.rs
provider_flow/usage.rs
provider_flow/errors.rs
provider_flow/provider_response.rs
```

Rules:

- Provider call orchestration belongs in `call.rs`.
- Streamed command execution and early-finish behavior belongs in `streamed_command_run.rs`.
- Usage aggregation belongs in `usage.rs`.
- Timeout/failure normalization belongs in `errors.rs`.
- Provider response parsing belongs in `provider_response.rs`.

The streamed command module must emit command-level checkpoints before waiting for the final provider response.

### Context Module Migration

Context handling lives in:

```text
context/build.rs
context/compaction.rs
context/media.rs
context/tool_results.rs
context/workspace.rs
context/token_budget.rs
```

The context module must not write session DB directly. It can return records or messages for checkpoint modules to persist.

## Runtime State Machine Requirements

Runtime should model turn execution explicitly:

```text
Created
Preparing
ProviderCalling
CommandRunning
Checkpointing
WaitingForNextStep
Finalizing
Completed
Failed
Interrupted
```

Each transition should be represented in `session_state/transitions.rs` or `turn_loop/driver.rs`. Runtime state transitions should emit checkpoints through `checkpoint/client.rs` when durable session state changes.

Required invariants:

- A session has at most one active turn.
- A turn has ordered provider calls.
- A provider call may produce streamed commands before completion.
- A command that mutates external state must checkpoint before the next command or provider continuation.
- Runtime must stop or fail the turn if a required mutating checkpoint ACK fails.

## Checkpoint Client Requirements

`checkpoint/client.rs` should provide typed helpers:

```text
checkpoint_turn_started
checkpoint_provider_call_started
checkpoint_command_run_started
checkpoint_command_started
checkpoint_command_finished
checkpoint_command_run_finished
checkpoint_turn_finished
checkpoint_turn_failed
```

Rules:

- All helpers generate idempotency keys.
- All helpers include `turn_id`.
- Command helpers include `command_run_id` and `command_id`.
- Mutating command helpers wait for ACK.
- Read-only helpers may batch but must flush before `command_run_finished`.

Runtime must not call raw session DB protocol directly from random modules. Use checkpoint helpers.

## Runtime-Owned External CLI Rule

For non-core tools:

```text
runtime -> tools facade -> router registry lookup -> hidden CLI launch
```

Rules:

- Router returns metadata and executable path only.
- Runtime/tools launches and owns the CLI process for that command invocation.
- CLI process output must be converted into normal command_run result records.
- CLI process start/finish/failure must go through checkpoint helpers.
- CLI processes must be hidden.

This rule applies after step 3, but step 2 module boundaries must leave room for it.

## Migration Steps

1. Add `SessionDbClient` abstraction used by runtime checkpoints.
2. Add checkpoint event types and idempotency helpers.
3. Extract `checkpoint` module while keeping old whole-session snapshot compatibility.
4. Extract `provider_flow/streamed_command_run.rs`.
5. Extract `tool_flow`.
6. Extract `turn_loop` from `manas/process.rs`.
7. Reduce `mano/process.rs` to session bootstrap facade.
8. Update tests to assert command-level checkpoint visibility.

## Tests

Required tests:

- Runtime emits `turn_started` before provider call.
- Runtime emits `command_finished` after streamed command completion.
- Mutating command waits for checkpoint ACK.
- Provider failure after streamed command preserves command result.
- Next turn can load prior command result from session DB.
- Same session retry does not duplicate command checkpoints.
- `mano` facade can still load and run a session through new modules.

## Non-Goals In Step 2

- External command CLI migration.
- Full removal of `mano`/`manas` names.
- Distributed queue or multi-router support.
