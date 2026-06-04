# Runtime Crate Architecture

`crates/runtime` is the renamed Mano/MANAS runtime. It owns agent/session
orchestration, state machines, prompt assembly, provider turns, tool-call
execution flow, context compaction, and final response behavior.

The Cargo package name should stay compatible with Tura:

```text
package = runtime
library = runtime
```

## Layout

```text
crates/runtime/
  src/
    lib.rs
    mod.rs
    agent_router.rs
    prompt_style.rs
    session.rs

    mano/
      mod.rs
      process.rs
      session_bootstrap.rs
      gateway_session.rs

    manas/
      mod.rs
      process.rs
      agent_prompts.rs
      child_dispatch.rs        # 子 session 派发：经 router CLI 子进程拉起子 agent
      constants.rs
      prompt_messages.rs
      runtime_turn.rs
      tool_catalog.rs
      tool_arguments.rs
      tool_execution.rs
      gateway_events.rs
      final_response.rs
      change_tracker.rs
      permission_gate.rs
      validator_feedback.rs    # 校验器可靠性反馈 → alaya 注册表

    session/
      activate_session.rs
      create_session.rs

    state_machine/
      session_management.rs
      agent_management.rs
      runtime_management.rs

    agent_router/
      mod.rs
      activate_agent.rs

    runtime/
      create_runtime.rs
      call_runtime.rs
      runtime_receive.rs

    context/
      context_management.rs    
      command_run_streams.rs   
      text_truncate.rs        
      docker_snapshot.rs
      process_snapshot.rs
      workspace_snapshot.rs

    prompt_style/
      agent_identity.rs
      compact_context.rs
      runtime_fallback.rs
      task_continuity.rs
      task_status.rs
      tool_progress.rs
      user_new_command.rs

    tool_router/
      execute_tool.rs
      send_calldata.rs

  tests/
    coding_agent_live_test.rs
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

`mano/process.rs` owns high-level user-turn orchestration:

1. Create or resume session.
2. Activate selected agents.
3. Initialize session and agent state.
4. Build initial workspace/user messages.
5. Call MANAS.
6. Return runtime result to gateway.

Session bootstrap and gateway session loading stay in focused modules.

Runtime gateway-session persistence goes through `crates/session_log`, not
workspace-local JSON files. `mano/gateway_session.rs` uses
`SessionLogClient::get_session` to resume an existing gateway session and
`SessionLogClient::upsert_session` to persist the initial runtime session
snapshot. Resumed sessions must match the requested workspace to avoid
cross-workspace reuse of a repeated session id.

Useful session-log queries while debugging runtime resume behavior:

```powershell
'{"command":"get_session","session_id":"session-id"}' | target\debug\gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\gateway.exe session-log
```

## MANAS Layer

`manas/process.rs` owns the runtime loop:

1. Transition session to running.
2. Build one runtime turn.
3. Call provider.
4. Extract text and tool calls.
5. Execute returned tool calls through `crates/tools`.
6. Store compact tool results.
7. Rebuild context.
8. Repeat if more work is required.
9. Force final response when needed.
10. Mark agent/session completed.

Helper modules own loading, filtering, normalization, publishing, and final
response details.

## State Machines

### Session

Owned by `state_machine/session_management.rs`.

States:

- `created`
- `initializing`
- `ready`
- `running`
- `waiting_for_permission`
- `waiting_for_command`
- `cancelling`
- `completed`
- `failed`

### Agent

Owned by `state_machine/agent_management.rs`.

States:

- `inactive`
- `activating`
- `ready`
- `thinking`
- `tooling`
- `delegating`
- `summarizing`
- `completed`
- `failed`

### Runtime

Owned by `state_machine/runtime_management.rs`.

States:

- `created`
- `context_building`
- `tool_catalog_building`
- `provider_pending`
- `provider_streaming`
- `provider_completed`
- `tool_calls_pending`
- `tools_running`
- `tools_completed`
- `finalizing`
- `completed`
- `failed`

Use transition methods instead of assigning states directly except in narrow
initialization or test setup paths.

### Task Management

Session task-management state is stored inside
`state_machine/session_management.rs` as `SessionManagement.task_plan`.
Runtime task-management structs are product state and must not contain
benchmark-specific fields or evaluator names. Benchmark contracts should live in
e2e fixtures and hidden evaluators.

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
`task_summary` and optional `status`; `status` accepts `question` or `done`.
The runtime may create the first single task from this state update. After a
task summary already exists, rename attempts are rejected and reported back in
the tool result unless the user clearly changed the task.

`planning` is also routed through `command_run` and only appears when
multi-task mode is enabled. Its schema is an array of tasks with required
`task_summary` and `deliverable`. It rejects single-goal input. When a plan
state machine already exists, runtime replaces the active task with the ordered
incoming tasks and preserves queued tasks omitted from the update.

Compact context writes the current `task_management_json()` into the compaction
log and rebuilds the next turn with a `TASK_MANAGEMENT_STATE` user-context
tail. Keep this behavior in runtime; gateway should not assemble runtime
prompts.

The old standalone delivery-status tool surface is removed. Legacy wording such as
"delivered" may still appear in existing multi-task completion logs while new
task-status guidance should prefer `done` or `completed`.

## Agent Loading

Runtime loads agents from `agents`.

Preferred order:

1. `agents/<agent>/interface/I<agent>.json`.
2. Generated Rust definitions from `agents`.
3. Test override loader.

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
2. `communication_style.md`
3. `prompt.md`

Legacy `prompt` and `fallback_agent.md` remain compatibility fallbacks for the
main prompt resource. Agent prompt loading must stay separate from command/tool
prompt loading so agent identity and communication behavior do not depend on the
active tool set.

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
