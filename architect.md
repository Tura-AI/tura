# Prompt Style Task State Injection Architecture

## Objective

Add a prompt-style rule that prevents agents from starting real work before the active session has a defined task type. The gate is driven by `SessionManagement.task_type`: if it is empty, the agent must infer `task_type` from the current context and the user's request, include `task_group` when the broad work area is missing or wrong, then call `task_status` before any work command.

## Module Design

- `crates/runtime/src/prompt_style/task_status.rs` owns the runtime-injected task-state prompt and schema description. This is the authoritative model-facing injection used by `command_run` for `task_status`.
- `crates/runtime/src/manas/tool_catalog.rs` embeds the runtime task-status prompt and schema into the compact `command_run` command description, using the live `SessionManagement.task_type` state to decide whether the startup gate is injected. No new command surface is added.
- `crates/tools/src/commands/task_status/prompt.md` and `schema.json` remain the command-level fallback/reference prompts. They should mirror the runtime wording closely enough that direct tool docs do not drift.
- `docs/runtime-prompts-vs-skills.md` and `README.md` describe the session-state contract for humans.

## Data Flow

1. The agent is activated with `command_run` and its allowed internal commands.
2. `runtime_turn` passes the live `SessionManagement` state into `tool_catalog` while loading command-run tools.
3. `tool_catalog` asks `prompt_style::task_status` for the task-status prompt and schema, with `require_startup_task_state` set from `session.task_type.is_empty()`.
4. The model sees the injected rule in the `task_status` command format only when the session task type is empty.
5. `tool_flow::task_status` persists `task_group` into the active task summary and applies `task_type`, which triggers runtime prompt manual injection for the selected task type.

## Command/API Entry Points

| Entry point | Role |
| --- | --- |
| `prompt_style::task_status::task_status_prompt(require_startup_task_state)` | Builds the injected `task_status` prompt text. |
| `prompt_style::task_status::task_status_schema(require_startup_task_state)` | Builds the model-visible `task_status` schema with current task type IDs. |
| `manas::tool_catalog::command_run_command_format_line("task_status", require_startup_task_state)` | Places the injected prompt/schema into `command_run`. |
| `manas::tool_catalog::load_agent_capabilities_with_commands(agent, session, commands)` | Reads live `SessionManagement.task_type` and decides whether the startup gate is required. |
| `tools::commands::task_status::normalize_output()` | Normalizes model-provided task-status arguments. |
| `tool_flow::task_status::apply_tool_result_session_state_update()` | Applies task group, task type, status, and compact context to session state. |

## Behavior Table

| Case | Input/session state | Expected prompt/API behavior | Implementation file |
| --- | --- | --- | --- |
| Missing task type | `SessionManagement.task_type` is empty | Prompt tells the agent to define `task_type` with `task_status` before work commands and include `task_group` when the broad work area is missing or wrong | `crates/runtime/src/manas/tool_catalog.rs`, `crates/runtime/src/prompt_style/task_status.rs` |
| Existing task type | `SessionManagement.task_type` is non-empty | Prompt still allows correction/update, but does not inject the startup gate | `crates/runtime/src/manas/tool_catalog.rs`, `crates/runtime/src/prompt_style/task_status.rs` |
| Task type selected | `task_status.task_type` contains valid IDs | Runtime normalizes task type and injects matching operation manuals | `crates/runtime/src/tool_flow/task_status.rs` |
| Invalid task type | `task_status.task_type` contains an unknown ID | Tool normalizer rejects the command | `crates/tools/src/commands/task_status/mod.rs` |
| Schema display | `command_run` exposes `task_status` | Schema description includes the same startup-gating rule and current task-type enum | `crates/runtime/src/prompt_style/task_status.rs` |

## Compatibility Risks

- Over-requiring `task_status` for simple conversation would add useless tool calls, so the rule is scoped to sessions whose state machine has no task type before work starts.
- The runtime prompt and command fallback prompt can drift; tests lock the runtime prompt/schema because that is what `command_run` injects.
- Runtime behavior is prompt-driven, not a hard validator. Enforcing command ordering inside `command_run` would require session-state awareness inside the command runner and is outside this prompt-style injection.
