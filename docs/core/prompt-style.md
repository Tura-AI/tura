# Prompt Style and Dynamic Prompt Injection

Prompt style is Tura's runtime-controlled prompt injection layer. It adds the
right operational instructions to a turn when session state says they are needed,
instead of forcing every agent to carry one static mega-prompt forever.

This is not "prompt injection" in the security-bug sense. User text is not
allowed to become privileged system policy just because it is persuasive. Tura's
dynamic prompt injection is runtime-owned: the runtime chooses prompt fragments
from structured state, agent configuration, persona configuration, task status,
context pressure, and tool execution results.

The implementation is spread across
[`crates/runtime/src/prompt_style`](../../crates/runtime/src/prompt_style), with
turn assembly in
[`crates/runtime/src/manas/runtime_turn.rs`](../../crates/runtime/src/manas/runtime_turn.rs)
and session-context rebuilding in
[`crates/runtime/src/context/build.rs`](../../crates/runtime/src/context/build.rs).

## Why it exists

Most agent stacks start with a large static system prompt and keep adding more:
persona rules, tool policy, coding standards, memory summaries, UI formatting,
task-specific manuals, retry reminders, and safety warnings. It feels simple
until every task receives instructions for every other task. Then the model has
to infer which paragraphs matter while paying tokens for all of them. Elegant,
like moving house by wearing every coat you own.

Tura uses prompt style to keep prompt behavior conditional:

- a CLI session gets CLI communication rules;
- a GUI persona session gets persona and rich-text communication rules;
- a visual task gets visual operation manuals and media commands;
- a refactor task gets refactoring discipline;
- a crowded context gets a compact-context checkpoint instruction;
- a no-tool retry gets an explicit active-goal reminder;
- a reflective agent gets reflection guidance at the tail of the turn.

The goal is not to make prompts clever. The goal is to make prompt assembly
auditable, state-driven, and small enough to stay relevant.

## Difference from ordinary agents

| Problem | Ordinary agent pattern | Tura prompt style |
| --- | --- | --- |
| Base prompt growth | Paste every useful instruction into one system prompt. | Keep prompt fragments separate and inject them only when their state condition is true. |
| Task-specific behavior | Trigger loose skills or rely on the model to remember task type. | Use `task_status.task_type` to select Runtime Prompt manuals and capabilities. |
| Persona and formatting | Mix tone, UI formatting, and work policy in one prompt. | Compose persona, communication style, agent prompt, and operation manuals as separate layers. |
| Long sessions | Replay a transcript and hope old instructions survive. | Persist prompt records in session log and reinsert active manuals after compaction. |
| Tool capability changes | Expose broad tools and tell the model not to misuse them. | Extend `command_run` command types through active session capabilities. |
| Retry behavior | Ask the model to "try again" with vague text. | Inject a targeted retry prompt with the active goal and operation manual. |
| Context pressure | Let the prompt bloat until the provider fails or forgets. | Inject a compact-context requirement when provider input tokens cross the prompt-injection threshold. |

The important difference is ownership. Other agents often treat prompts as a
pile of text near the model call. Tura treats prompt fragments as runtime
artifacts with clear owners, insertion points, and session-state effects.

## Prompt-style layers

Prompt style is not one file. It is a set of runtime prompt fragments with
different lifetimes.

| Layer | Owner | Typical role |
| --- | --- | --- |
| Runtime identity | `agent_identity.rs` and `runtime_turn.rs` | Names the active agent/persona, model/provider, user, language, and context limit. |
| Persona and communication style | `agent_prompts.rs` and `personas` | Adds visible identity, GUI rich-text rules, or CLI-safe output rules. |
| Agent prompt resources | agent config | Adds agent-specific capability posture and working behavior. |
| Session log context | `context/build.rs` | Replays durable messages, compact context, tool-result summaries, and prompt records. |
| Task-status prompt and schema | `task_status.rs` | Tells the model how to update task state and select `task_type`. |
| Runtime Prompt manuals | `runtime_prompt_manual.rs` | Injects task-specific operation manuals and command-run capability extensions. |
| Tail prompts | `tail_injection.rs` | Appends high-priority per-turn reminders such as compaction, retry, or reflection. |
| Compact-context prompt | `compact_context.rs` | Forces a checkpoint when the active context is crowded. |
| User-new-command prompt | `user_new_command.rs` | Adds queued user commands that arrived while the agent was working. |

These layers are composed for a turn. They are not all always present.

## Runtime mechanism

The normal turn path is:

1. Runtime selects the active agent and resolves provider/model settings.
2. `runtime_turn.rs` creates the base runtime identity system message.
3. `load_agent_system_prompt_messages` adds persona messages, communication
   style, and agent prompt resources.
4. `build_context` rebuilds the conversation from session log records, including
   compact context records, tool-result context, and persisted prompt records.
5. `messages_for_turn_with_context_limit` appends queued user commands as
   developer messages when the router has them.
6. Runtime appends optional tail prompts: explicit extra system prompt,
   compact-context requirement, self-reflection prompt, or no-tool retry prompt.
7. Runtime builds the provider tool list. `command_run` is restricted to the
   agent's base commands plus `session_capabilities` injected by active manuals.
8. The provider receives the assembled messages and allowed tool schema.
9. Tool results are applied back into session state. A successful `task_status`
   result can update `task_group`, `task_type`, task state, and compact context.
10. When `task_type` changes, runtime normalizes manual ids, expands parent
    manuals, appends missing manual records, and appends missing capability
    records for later turns.

That loop is why prompt style is dynamic: the next turn is assembled from the
new session state, not from a fixed prompt string baked into the agent.

## Runtime Prompt manuals as dynamic injection

Runtime Prompt manuals are the most visible prompt-style injection path. They
are selected by `task_status.task_type`, not by fuzzy keyword matching.

Each manual has:

| File | Purpose |
| --- | --- |
| `prompt_identity.json` | Id, display name, description, parent manuals, and added command capabilities. |
| `prompt.md` | The operation manual text injected for that task mode. |

When a task type is active, runtime can append two kinds of session-log records:

- `runtime_prompt_manual`: the manual text as a system message;
- `runtime_prompt_command_run_capabilities`: command-line format extensions and
  capability names for `command_run`.

Those records are not pasted blindly on every request. Runtime checks whether a
manual record already exists since the latest compaction boundary. If it exists,
it is not duplicated. After compaction, active manuals are appended again so the
operating mode survives the history cut.

See the narrower manual-specific documentation in
[`docs/core/runtime-prompt.md`](runtime-prompt.md).

## Tail prompts

Tail prompts are short, conditional messages appended near the end of the turn
prompt. They are useful when the latest runtime condition should be hard to miss.

Examples:

- `compact_context_required`: appended when provider input tokens cross the
  prompt-injection threshold.
- `self_reflection_tail_prompt`: appended for reflective agents so progress
  updates reason backward from the goal.
- `no_tool_retry`: appended when the model needs to continue toward the active
  goal instead of stopping without a required tool call.
- `USER_NEW_COMMAND`: appended when a user sends new instructions while the
  agent is still working.

Tail prompts are still runtime-owned. They are not a license for random prompt
patches; they are bounded by session state and explicit runtime code paths.

## Capability injection

Prompt style can change tool availability, but only through structured session
capabilities.

For example, a visual manual can add `read_media` and `generate_media`. Runtime
records those capabilities in `SessionManagement.session_capabilities`. Later,
when `runtime_turn.rs` builds the tool list, it starts from the agent's allowed
commands and extends them with the active session capabilities. Tool execution
still rechecks the allowed command-run command set before dispatch.

This is different from telling the model "you may use image tools now" while the
actual tool schema remains unchanged. In Tura, the prompt text and command schema
move together.

## State that can affect prompt style

Prompt-style injection can depend on:

- selected agent and provider route;
- active persona and frontend source (`TURA_SESSION_PERSONA`,
  `TURA_FRONTEND_SOURCE`);
- current user name, language, shell, workspace, date, and context limit;
- `SessionManagement.task_type`, `task_group`, `goal_mode`,
  `reflection_enabled`, and `op_manual_enabled`;
- `session_capabilities` already injected into the current compaction window;
- provider-reported input token usage;
- compact-context records and tool-result context caches;
- queued user commands received through the router.

The state is explicit so prompt assembly can be inspected and tested. If a
prompt fragment appears, there should be a runtime condition explaining why.

## Safety boundary

Dynamic prompt injection has a bad reputation because the phrase also describes
attacks where untrusted content overrides instructions. Tura's prompt style is
the opposite boundary:

- user input is represented as user/developer content, not silently upgraded into
  system policy;
- runtime-owned fragments come from code, configs, or session state;
- task mode changes go through `task_status` and valid runtime manual ids;
- command availability is enforced by command schemas and execution checks, not
  by prose alone;
- compaction preserves active state instead of trusting a loose chat summary.

The model still reads natural-language instructions, so this is not magic armor.
It is simply less sloppy than letting every subsystem paste text into the next
provider call.

## Debugging checklist

When a prompt-style fragment is missing or unexpectedly present, check:

- which agent and persona were selected;
- whether `TURA_FRONTEND_SOURCE=cli` disabled GUI persona communication style;
- whether the session has `task_type` set;
- whether the requested `task_type` exists in the Runtime Prompt catalog;
- whether parent manual expansion changed the active manual order;
- whether a manual record already exists since the latest compaction;
- whether `op_manual_enabled`, `goal_mode`, or `reflection_enabled` changes
  manual text injection;
- whether provider input tokens crossed the compact-context threshold;
- whether `session_capabilities` contains the expected command names;
- whether the tool schema and command execution allow the same commands.

The healthy path is boring and traceable: state changes, runtime records prompt
fragments, context rebuilds them, and the provider sees only the pieces needed
for this turn.
