# Runtime Crate Architecture

`crates/runtime` is where an agent session becomes an execution. It owns
agent/session orchestration, state machines, prompt assembly, provider turns,
tool-call execution flow, context compaction, and final response behavior. It
coordinates those owners; it should not quietly absorb their storage, provider,
or routing responsibilities.

Cargo target names:

```text
package = runtime
library = runtime
binary  = tura_runtime   (src/bin/tura_runtime.rs -> runtime::worker::run)
```

## Runtime worker binary (`tura_runtime`)

The runtime is run as a per-session worker by the **standalone `tura_runtime`
binary** (no longer the gateway binary re-invoked by role). `runtime::worker`
hosts the line-protocol loop the router drives: read `{ "kind", "payload" }`,
write one JSON reply per line. `health_check` carries `tura_path::instance_version()`
so the router performs a **version handshake** before dispatching. The worker
activates the agent spec, runs one prompt via `mano::process_from_gateway_session_in_directory`,
and exits (complete-and-die). It reaches the database only through the single
`tura_session_db` owner's socket — never `open_default()`.

## Layout

```text
crates/runtime/
  src/
    lib.rs
    mod.rs

    mano/
      mod.rs
      process.rs

    session_bootstrap/
      mod.rs
      load.rs
      prepare_turn.rs
      persisted.rs
      initial_messages.rs

    manas/
      mod.rs
      process.rs
      agent_prompts.rs
      child_dispatch.rs        # child session dispatch through router CLI subprocesses
      constants.rs
      prompt_messages.rs
      runtime_turn.rs
      tool_catalog.rs
      tool_arguments.rs
      final_response.rs

    turn_loop/
      mod.rs
      provider_step.rs
      tool_step.rs
      retry_policy.rs
      no_tool_policy.rs
      task_progress.rs

    checkpoint/
      mod.rs
      client.rs
      event.rs
      command_run.rs
      session_snapshot.rs

    provider_flow/
      mod.rs
      call.rs
      provider_response.rs
      request_options.rs
      streamed_command_run.rs
      usage.rs
      errors.rs

    tool_flow/
      mod.rs
      execute.rs
      command_run_result.rs
      permission.rs
      task_status.rs

    gateway_events/
      mod.rs
      agent_message.rs
      cli_live.rs
      progress.rs
      tool_message.rs

    session/
      mod.rs
      activate_session.rs
      create_session.rs

    state_machine/
      agent_management.rs

    runtime_event_writer.rs

    agent_router/
      mod.rs

    runtime/
      create_runtime.rs
      call_runtime.rs            # re-export for provider_flow/call.rs
      runtime_receive.rs

    context/
      build.rs
      command_run_streams.rs   
      compaction.rs
      media.rs
      text_truncate.rs
      token_budget.rs
      tool_results.rs
      workspace.rs

    prompt_style/
      mod.rs
      agent_identity.rs
      compact_context.rs
      runtime_fallback.rs
      task_status.rs
      tool_progress.rs
      user_new_command.rs

    tool_router/
      execute_tool.rs
      send_calldata.rs

  tests/
    business/
      claude_code_mock_e2e.rs
      coding_agent_mock_e2e.rs
    live/
      claude_code_live_e2e.rs
    override_manas_direct_test.rs
    override_mano_and_manas_test.rs
    process_from_user_default_test.rs
```

The module names may keep `mano` and `manas` internally because they describe
the orchestration layers, but the directory owner is `crates/runtime`.

Keep file names aligned with the current Tura implementation. In particular,
Use `runtime_receive.rs` for provider stream receive/normalization helpers.
If the spelling is ever corrected, rename the source file, module declaration,
tests, and all architecture docs in one change.

## Public Entrypoints

`mano/mod.rs` is the user/session entry API:

- Create or resume session.
- Load gateway session payload.
- Infer topic.
- Select session directory.
- Activate agents.
- Initialize state.
- Call MANAS processing.

`manas/mod.rs` is the agent/session execution entry API:

- Run one active session.
- Execute provider turns.
- Execute tool calls.
- Force final response when needed.

Keep both `mod.rs` files as declaration and override seams. Do not place large
loops, prompt assembly, tool filtering, or JSON parsing there.

## MANO Layer

`mano/process.rs` is the bootstrap facade for high-level user-turn
orchestration:

1. Ask `session_bootstrap` to create or resume the session and prepare a user turn.
2. Activate selected agents.
3. Derive session feature flags from the active agent: `planning_enabled` tracks
   planning tool availability, while `reflection_enabled` tracks whether the
   active agent requests reflective task-status/objective prompt style.
4. Initialize session and agent state.
5. Ask `session_bootstrap::initial_messages` for initial workspace/user messages.
6. Call MANAS.
7. Return runtime result to gateway.

Session bootstrap and gateway session loading stay in focused modules:
`session_bootstrap/load.rs`, `session_bootstrap/prepare_turn.rs`,
`session_bootstrap/persisted.rs`, and `session_bootstrap/initial_messages.rs`.

Runtime gateway-session persistence goes through `crates/session_log`, not
workspace-local JSON files. Each session workspace is also initialized as a
local Git repository when it is prepared, and terminal runtime exits create a
workspace commit whose message includes the session id and task group.
`checkpoint/session_snapshot.rs` uses `SessionDeltaWriter` and
`SessionLogClient::persist_session_delta` to append idempotent context records
and refresh the runtime-owned management projection without replacing session
history.
`session_bootstrap/persisted.rs` uses `SessionLogClient::get_session` to resume
an existing gateway session. Resumed sessions must match the requested workspace
to avoid cross-workspace reuse of a repeated session id.

Useful session-log queries while debugging runtime resume behavior:

```powershell
'{"command":"get_session","session_id":"session-id"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\tura_gateway.exe session-log
```

## MANAS Layer

`manas/process.rs` owns the MANAS runtime loop while focused modules own the
state-machine phases:

1. Transition session to running.
2. Delegate provider output accumulation to `turn_loop/provider_step.rs`.
3. Execute returned tool calls through `tool_flow/execute.rs` and focused
   `tool_flow` helpers.
4. Delegate command-run compaction cleanup to `turn_loop/tool_step.rs`.
5. Rebuild context through `context/build.rs`.
6. Delegate retry/no-tool decisions to `turn_loop/retry_policy.rs` and
   `turn_loop/no_tool_policy.rs`.
7. Persist session snapshots through `checkpoint/session_snapshot.rs`.
8. Return the canonical Session result without constructing a synthetic
   completion Runtime.

Helper modules own loading, filtering, normalization, checkpoint helpers, and
final response details. Gateway-visible publishing lives in `gateway_events/`.

## State Machines

### Session

The data model and transition rules are owned by `lifecycle::SessionAggregate`
and `lifecycle::SessionManagement` in `crates/lifecycle`. Runtime drives typed
commands but does not define a second Session state machine. Task-management
JSON projection lives in `crates/lifecycle/src/session_projection.rs`.

States:

- `created`
- `running`
- `paused`
- `completed`
- `failed`
- `cancelled`
- `interrupted`

### Agent

`state_machine/agent_management.rs` owns agent configuration: identity, prompt
and capability bindings, provider selection, and validator settings. It does
not own execution lifecycle state. Session progress is represented by the
canonical Session state machine, while each provider invocation is represented
by the canonical Runtime state machine in `crates/lifecycle`.

### Runtime

Owned by `lifecycle::RuntimeAggregate` in `crates/lifecycle/src/runtime.rs` and
driven by Runtime provider/tool orchestration.

States:

- `created`
- `dispatching`
- `waiting_first_token`
- `streaming`
- `finished`
- `failed`
- `timed_out`
- `cancelled`

Use transition methods instead of assigning states directly except in narrow
initialization or test setup paths.

### Task Management

Session task-management state is stored by
`lifecycle::SessionManagement.task_plan`; its JSON projection belongs to
`crates/lifecycle/src/session_projection.rs`. Runtime task-management structs
are product state and must not contain benchmark-specific fields or evaluator
names. Benchmark contracts should live in e2e fixtures and hidden evaluators.

The model-facing compact state is produced by:

```text
SessionManagement::task_management_json()
```

Single-task mode serializes `task_management` as one object with:

```text
task_id
step
plan_summary
task_summary
deliverable
sub_session_id
start_at
poll_interval
start_condition
task_status
```

Multi-task mode serializes `task_management.tasks[]`.

`task_status` is not a standalone top-level model tool. It is an internal
`command_run` command. Its model-visible JSON has only optional
`task_summary` and optional `status`; `status` accepts `doing`, `question`, or `done`.
The runtime may create the first single task from this state update. After a
task summary already exists, rename attempts are rejected and reported back in
the tool result unless the user clearly changed the task.

`planning` is also routed through `command_run` and only appears when
multi-task mode is enabled. Its schema is an array of tasks with required
`task_summary` and `deliverable`. It rejects single-goal input. When a plan
state machine already exists, runtime replaces the active task with the ordered
incoming tasks and preserves queued tasks omitted from the update.

Compact context is written by `context/compaction.rs` and rebuilds the next turn
from the compaction summary, workspace snapshot, environment context, and active
planning objective. Runtime owns this prompt state; gateway should not assemble
runtime prompts.

After a compact checkpoint, `SessionManagement.session_log_retention` records the
absolute compact boundary and the number of omitted `session_log` entries.
Runtime trims the in-memory `session_log` before the retained boundary so the
persisted `management_json` used for resume contains only entries still needed to
rebuild provider context. The retained slice starts at the immediately preceding
tail of `tool_result` entries, if any, because compact rebuild still replays that
tool context.

The active compact threshold is capped at 260,000 tokens. Runtime still asks the
agent to provide a `task_status.compact_context` handoff when provider-reported
input reaches the active threshold, but it also applies an automatic checkpoint
after a turn if `provider_input_tokens + newly_persisted_context_bytes / 3`
would exceed that threshold. Automatic checkpoints include the current turn's
persisted tool results in the rebuild timeline so completed work is not lost.
When rebuilt compact text itself exceeds roughly 12,000 estimated tokens using
the same bytes/3 estimate, older timeline entries are omitted before writing the
checkpoint.

Task-status guidance should use `doing` only when more `command_run` calls are
required, `done` for finished work, and `question` when
the active task is blocked on user input.

## Agent Loading

Runtime loads agents from `agents`.

Preferred order:

1. `agents/src/<agent_id>/agent_config.json`.
2. Optional `agents/src/<agent_id>/prompt.md`.
3. Test override loader when a runtime test injects agent config directly.

Provider defaults and command lists come from agent config.

## Tool Catalog

`manas/tool_catalog.rs` owns:

- Active command loading from `crates/tools`.
- Provider schema conversion.
- Command prompt compaction and provider-tool description injection.
- Final-turn filtering.
- Command-run placement at the end of the tool list.
- Cache-stable tool-set identity.

Tool execution is delegated to `crates/tools`; runtime does not implement shell,
patch, file lock, or package environment behavior.

## Agent Prompts

`manas/agent_prompts.rs` owns loading non-tool agent prompt resources from the
selected agent prompt directory:

1. `persona.md`
2. shared `personas/src/communication_style/communication_style.md`
3. `prompt.md`

Legacy per-persona `communication_style.md` files are accepted only as a
fallback. `prompt` and `fallback_agent.md` are accepted main-prompt resource
names. Agent prompt loading must stay separate from command/tool prompt loading
so agent identity and communication behavior do not depend on the active tool
set.

## Prompt Assembly

Fixed runtime prompt fragments live in:

```text
crates/runtime/src/prompt_style/
```

Dynamic values are injected by builder sections such as:

- `parent_user_task`
- `latest_tool_result`
- `response_language`
- `runtime_state`

Do not load runtime prompt fragments from markdown by string name.

Provider-facing turn assembly happens in `manas/runtime_turn.rs`: runtime
identity from `prompt_style/` is injected first, then agent prompt resources from
`manas/agent_prompts.rs`, then dynamic session context and user/task messages.

## Provider Integration

Runtime creates one provider turn through `crates/provider`.

Provider owns **all** per-provider format/wire handling. Runtime contains
**zero** provider-name branches. The contract:

| Concern | Owner |
|---|---|
| Provider route lookup | provider (`route_by_name`) |
| Model request construction | provider |
| Streaming normalization (OpenAI Chat / OpenAI Responses / Anthropic / Google / MiniMax XML) | provider (`normalize_response_content`) |
| Response text extraction from normalized content | provider (`extract_response_text`) |
| Tool-call extraction from normalized content | provider (`extract_tool_calls` → `ProviderToolCall`) |
| `<thought>` block stripping | provider (`strip_thought_blocks`) |
| prompt-cache key support flag | provider (`prompt_cache_key_supported`) |
| OpenAI-compatible SSE usage support flag | provider (`openai_compatible_usage_stream_supported`) |
| Provider error → unsupported-content-type fallback | provider (`provider_unsupported_content_type`, `replace_unsupported_content_type_in_messages`) |
| Usage accounting | provider |

Runtime owns what to do with the normalized output (state update, tool
dispatch, context compaction, final response). It emits and consumes the
canonical OpenAI Responses-API content shape (`input_image`, `input_audio`,
`input_file`, `tool_calls`); the provider crate translates that shape into and
out of each provider's wire format.

Provider call orchestration lives in `provider_flow/call.rs`; provider request
options and message normalization live in `provider_flow/request_options.rs`.
`runtime/call_runtime.rs` re-exports the provider call entrypoint.

## Child Sub-Session Dispatch

When the manas loop produces a `TaskStep` carrying `step_agent_name` (from a
`planning` tool result with `child_agent_names`), runtime spawns a child
sub-session through `manas::child_dispatch`. Dispatch always uses the router
CLI subprocess (`tura_router run-agent`) over stdin/stdout JSON — never an
HTTP/URL call. This holds the project rule:

- internal runtime ↔ router communication is **CLI only**;
- router and gateway are a single process;
- all runtimes are subprocesses.

`child_dispatch` exposes:

- `dispatch_child_agent(req)` — single child, blocking;
- `dispatch_child_agents_concurrent(reqs)` — N parallel children (each is its
  own router-CLI subprocess); collects summaries.

The router CLI mirrors the HTTP handler exactly (same `dispatch_run_agent`
core), so child-session depth/concurrency caps (`MAX_PLANNING_DEPTH`,
`MAX_RUNTIME_WORKERS`) apply uniformly.

## Final Response

The final user-visible completion path is owned by runtime. Final turns should
restrict tool exposure to the final-response path so the session ends with a
clear answer.

## Checks

Use:

```text
cargo fmt -p runtime
cargo check -p runtime
```
