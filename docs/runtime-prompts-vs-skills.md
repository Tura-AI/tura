# Runtime Prompts Vs Skills

Tura uses both external skills and internal runtime prompt manuals, but they
solve different problems.

## Skills

Skills are environment-provided capability packs. A skill may include
instructions, reference files, scripts, tool usage rules, assets, or connector
knowledge. They are useful when the agent needs a specialized workflow that is
not part of Tura's core runtime.

Examples:

- image generation or image editing workflow instructions
- connector-specific usage rules
- a local package with reusable scripts or templates
- a domain playbook that should be explicitly invoked

Skills are loaded because the current environment exposes them and the task
matches their trigger rules.

## Runtime Prompt Manuals

Runtime prompt manuals are Tura's internal operating manuals. They are selected
by structured task type and persisted into the session. The implementation lives
in [runtime_prompt_manual.rs](../crates/runtime/src/prompt_style/runtime_prompt_manual.rs),
and the bundled manuals live under [crates/runtime/src/runtime_prompt](../crates/runtime/src/runtime_prompt).

Current manual families include:

- `debug`
- `frontend`
- `visual`
- `editorial`
- `interactive_and_3d`
- `data_visualization`
- `new_build`
- `refactoring`
- `research_and_learning`
- `creative_and_writing`

The active task can update `task_type` through `task_status`. When a manual is
active, Tura can insert its operating guidance and extend the active
`command_run` command set with needed capabilities such as `read_media`,
`generate_media`, `web_discover`, `apply_patch`, or shell surfaces.
If `SessionManagement.task_type` is empty, the injected task-status prompt tells
the agent to define `task_type` from the current context and the user's request
before starting work, and to include `task_group` when the broad work area is
missing or wrong.

## The Difference

| Question | Skill | Runtime prompt manual |
| --- | --- | --- |
| Who owns it? | Environment or plugin author | Tura runtime |
| How is it selected? | Skill trigger rules | `task_status.task_type` |
| Does it persist in session history? | Depends on the skill flow | Yes, as runtime prompt manual records |
| Can it affect command_run commands? | Indirectly, through tools/connectors | Yes, via manual capabilities |
| Best for | External workflows and integrations | Task-mode discipline and long-running work |

## Why This Matters

The common failure mode is prompt hoarding: every useful instruction gets pasted
into every future prompt. That makes the model slower, more expensive, and less
focused.

Tura avoids that by separating durable operating modes from opportunistic skills:

- The base agent stays small.
- A frontend task gets frontend taste and media inspection tools.
- A refactor task gets source-port discipline.
- A visual task gets visual verification and media commands.
- Compaction can reinsert the active manuals after old context is replaced.

This is one of the main reasons Tura is designed for maintenance-quality work
instead of one-shot generation.
