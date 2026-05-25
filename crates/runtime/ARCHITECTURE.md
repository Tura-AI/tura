# Runtime Crate Architecture

`crates/runtime` is the renamed Mano/MANAS runtime. It owns agent/session
orchestration, state machines, prompt assembly, provider turns, tool-call
execution flow, context compaction, and final response behavior.

The Cargo package name should stay compatible with Tura:

```text
package = code-tools-suite
library = code_tools_suite
```

## Layout

```text
crates/runtime/
  src/
    lib.rs
    mod.rs

    mano/
      mod.rs
      process.rs
      session_bootstrap.rs
      gateway_session.rs

    manas/
      mod.rs
      process.rs
      agent_prompts.rs
      constants.rs
      prompt_messages.rs
      runtime_turn.rs
      tool_catalog.rs
      tool_arguments.rs
      tool_execution.rs
      gateway_events.rs
      final_response.rs

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
      runtime_recieve.rs

    context/
      context_management.rs

    prompt_style/
      task_continuity.rs
      command_evaluation.rs

    tool_router/
      execute_tool.rs
      send_calldata.rs
```

The module names may keep `mano` and `manas` internally because they describe
the orchestration layers, but the directory owner is `crates/runtime`.

Keep file names aligned with the current Tura implementation. In particular,
use `runtime_recieve.rs` consistently while the codebase keeps that spelling.
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

## Agent Loading

Runtime loads agents from `crates/agents`.

Preferred order:

1. `crates/agents/<agent>/interface/I<agent>.json`.
2. Generated Rust definitions from `crates/agents`.
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

Provider owns:

- Provider route lookup.
- Model request construction.
- Streaming normalization.
- Tool-call extraction normalization.
- Usage accounting.

Runtime owns what to do with provider output.

## Final Response

The final user-visible completion path is owned by runtime. Final turns should
restrict tool exposure to the final-response path so the session ends with a
clear answer.

## Checks

Use:

```text
cargo fmt -p code-tools-suite
cargo check -p code-tools-suite
```
