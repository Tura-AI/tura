# Runtime Prompt

Runtime prompts are Tura-owned operation manuals selected by `task_type`. They
are not generic skills, not user preferences, and not decorative system text.
They are the runtime's way to load task-specific discipline, completion rules,
and command-run capability extensions only when the current task needs them.

The implementation is centered in
[`crates/runtime/src/prompt_style/runtime_prompt_manual.rs`](../../crates/runtime/src/prompt_style/runtime_prompt_manual.rs).
Bundled manuals live under
[`crates/runtime/src/runtime_prompt`](../../crates/runtime/src/runtime_prompt).

## Runtime prompt manual structure

Each manual is a directory with two files:

| File | Purpose |
| --- | --- |
| `prompt_identity.json` | Machine-readable identity: id, display name, description, parent manuals, and capabilities. |
| `prompt.md` | The actual operation manual text injected into the provider context. |

The identity shape is represented by `RuntimePromptIdentity`:

```json
{
  "id": "frontend",
  "display_name": "Frontend Operation Manual",
  "description": "Use for frontend, TUI, browser UI...",
  "father_ids": ["visual"],
  "capabilities": ["read_media"]
}
```

At runtime, this becomes a `RuntimePromptManual` containing:

- `id`
- `display_name`
- `description`
- `father_ids`
- `capabilities`
- `prompt`

## Manual discovery

Runtime manual discovery uses this order:

1. `TURA_RUNTIME_PROMPT_ROOT`, when set;
2. the runtime crate's `src/runtime_prompt` directory;
3. embedded manuals generated at build time as a fallback.

`available_manuals()` reads manual directories from disk and falls back to
embedded manuals when the disk catalog is unavailable or empty. This matters for
release builds: manuals can still exist even when source files are not laid out
like the checkout.

The same catalog feeds:

- valid `task_type` ids;
- task-status schema enum values;
- task-status prompt catalog text;
- manual dependency expansion;
- command-run capability injection.

## Current bundled task types

The current bundled manual ids are discovered from
`prompt_identity.json` files. They include:

| id | Purpose |
| --- | --- |
| `data_research` | Charts, dashboards, visual analysis, data storytelling, and data visualization deliverables. |
| `debug` | Bugs, failing tests, regressions, brittle edge cases, and failure analysis. |
| `devops` | CI, release, deployment, cloud infrastructure, hosted runners, and operational debugging. |
| `editorial` | Slide decks, presentations, visual PDFs, print documents, and editorial layouts. |
| `frontend` | Frontend/TUI/browser UI tasks, application UI behavior, state, and implementation. |
| `interactive_and_3d` | Simulations, 3D scenes, shader effects, and real-time interactive visuals. |
| `new_build` | New frontend/backend/full-stack implementation from scratch or a plan. |
| `refactoring` | Rebuilds, refactors, compatibility ports, and clean-room style restructuring. |
| `visual` | Visual systems, static layout polish, media assets, and visual deliverables. |
| `website` | Landing pages, marketing pages, portfolios, brand sites, and static content websites. |

The exact list should be treated as dynamic. The authoritative catalog is the
runtime prompt root, not this table.

## Selection through `task_type`

The active manual set is selected by `SessionManagement.task_type`.

The usual path is:

1. The assistant recognizes the task type from the user's request.
2. It calls `command_run` with `task_status`, setting `task_group` and
   `task_type`.
3. Runtime validates the task type ids against the manual catalog.
4. Runtime normalizes the ids and expands parent manuals.
5. Runtime appends missing manual records and command-run capability records to
   the session log.

`task_type` is an array because tasks can need multiple manuals. A slide deck
uses `editorial`, which depends on `visual`. A 3D frontend demo can expand to
`visual`, `frontend`, and `interactive_and_3d`.

## Parent manual expansion

Manuals can declare `father_ids`. Runtime expands parents before children using
`normalize_task_type_ids`.

Examples from the tests:

| Requested | Normalized order |
| --- | --- |
| `interactive_and_3d` | `visual`, `frontend`, `interactive_and_3d` |
| `frontend` | `visual`, `frontend` |
| `website` | `visual`, `website` |
| `data_research` | `visual`, `new_build`, `data_research` |
| `editorial` | `visual`, `editorial` |

The order matters. Parent manuals establish shared visual/build discipline;
child manuals add narrower rules.

## Injection conditions

Runtime only injects active operation manual text when manual injection is
enabled for the session. The check is:

```text
session.goal_mode || session.reflection_enabled || session.op_manual_enabled
```

`op_manual_enabled` defaults to true, but agents and run options can disable it.
`goal_mode` or reflection mode can still require manual context because those
modes need stricter completion behavior.

The active manual text can be queried through `active_operation_manual_text`,
which joins the active manuals' `prompt.md` content into one operation-manual
block.

## Session-log records

Runtime prompt injection is persisted as session-log records rather than pasted
ephemerally into one request.

Manual text records use:

```json
{
  "type": "runtime_prompt_manual",
  "task_type": "debug",
  "manual_name": "Debug Operation Manual",
  "role": "system",
  "content": "...manual text...",
  "timestamp": "..."
}
```

Capability extension records use:

```json
{
  "type": "runtime_prompt_command_run_capabilities",
  "task_type": "visual",
  "manual_name": "Visual Operation Manual",
  "role": "system",
  "capabilities": ["read_media", "generate_media"],
  "content": "[runtime_prompt_command_run_capabilities]...",
  "timestamp": "..."
}
```

The context builder later replays these records as provider messages.

## Duplicate prevention

`append_missing_runtime_prompt_manuals` checks whether a manual record is already
present since the latest `context_compaction` record. If it is present, runtime
does not append it again.

That rule prevents repeated manual text from accumulating during normal turns.
After compaction, the old manual record is considered behind the boundary, so
runtime appends fresh manual records. This keeps the active operating mode alive
while allowing older transcript bulk to be omitted.

## Command-run capability injection

Manual identities may list command capabilities. For each active manual,
`command_run_capability_content`:

1. canonicalizes each capability command name;
2. ignores `command_run` itself;
3. skips capabilities already present in `SessionManagement.session_capabilities`;
4. looks up the command-run command-line format for the capability;
5. writes a system message explaining that the active manual adds those command
   formats;
6. records the capability names in `session_capabilities`.

This is how visual/editorial work can add media commands such as `read_media` or
`generate_media` without exposing every possible command to every task forever.

The allowed command set during tool execution is assembled in
[`crates/runtime/src/tool_flow/execute.rs`](../../crates/runtime/src/tool_flow/execute.rs):
agent command-run commands are extended with `session.session_capabilities`.

## Relationship to `task_status`

`task_status.task_type` is the normal control plane for runtime prompts.

When a `task_type` update is applied:

- runtime replaces the session's active task-type list;
- runtime enables operation manuals unless the session explicitly disables them;
- runtime appends any missing manual records;
- runtime appends any missing command-run capability records;
- the task-status prompt and schema can expose the updated catalog to the model.

If a session has no `task_type`, the startup task-state gate tells the agent to
set it before write-producing commands. This prevents the agent from editing a
frontend, visual, refactor, or debug task before the correct operating manual is
active.

## Relationship to context compaction

Runtime prompts are context-managed records. During compaction:

1. old prompt records can fall behind the compaction boundary;
2. session capabilities are reset to the baseline capability set;
3. `append_runtime_prompt_manuals_after_compact` re-appends active manuals;
4. capability records are re-added for active manuals as needed.

The important invariant: compaction reduces history, but it does not silently
drop the active operation manual.

## Runtime prompts vs skills

Runtime prompts differ from environment skills:

| Question | Runtime prompt manual | Skill |
| --- | --- | --- |
| Owner | Tura runtime | Environment/plugin/tooling provider |
| Selection | `task_status.task_type` and session state | Skill trigger rules or external environment |
| Persistence | Session-log records | Depends on skill system |
| Command capability impact | Directly extends `command_run` commands | Indirect, if the skill provides tools or instructions |
| Best use | Task-mode discipline and completion criteria | External workflows, specialized assets, connector knowledge |

See also
[`docs/runtime-prompts-vs-skills.md`](../runtime-prompts-vs-skills.md).

## Customization

To add or override manuals:

1. Create a manual directory under the runtime prompt root.
2. Add `prompt_identity.json` with a unique `id`.
3. Add `prompt.md` with the operation manual text.
4. Include `father_ids` when the manual should inherit another manual.
5. Include `capabilities` only for commands the manual truly needs.
6. Set `TURA_RUNTIME_PROMPT_ROOT` when testing an alternate catalog.
7. Verify `task_status` schema/prompt sees the new id and that capability records
   are appended only once per compaction boundary.

Avoid making every manual depend on every other manual. Prompt hoarding is how a
runtime grows a junk drawer and calls it architecture.

## Debugging checklist

When a manual is missing from a prompt, check:

- is `SessionManagement.task_type` set?
- did `task_status` use a valid catalog id?
- did parent expansion normalize the id into the expected order?
- is `op_manual_enabled`, `goal_mode`, or `reflection_enabled` true?
- is there already a `runtime_prompt_manual` record since the last compaction?
- did compaction reappend manuals after the checkpoint?
- are expected capabilities present in `session_capabilities`?
- did the agent's allowed command-run command set include those capabilities?

Those checks follow the runtime path exactly: catalog, selection, injection,
recording, context rebuild, command execution.
