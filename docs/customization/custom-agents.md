# Custom agents

Agents are Tura's runtime work profiles. An agent controls prompt resources,
allowed command-run capabilities, provider/model route preferences, operation
manual policy, reporting flags, and validator settings.

Keep the boundary clear:

| Layer | Controls |
| --- | --- |
| Agent | Tools, model route, agent prompt, operation-manual enablement. |
| Persona | Voice, communication style, avatar/expression behavior. |
| Runtime prompt | Task-specific operation manuals selected by `task_type`. |
| Provider config | Concrete providers, models, base URLs, credentials, fallback routes. |

This document explains custom agents from both release and source views.

## Agent root

Agents are discovered under:

```text
<project-root>/agents/src/<agent_id>/
```

Each agent normally has:

```text
agents/src/<agent_id>/
  agent_config.json
  prompt.md
```

Unlike personas, the current agent registry uses `agents/src` for registry-backed
agents in both release and source layouts. Use a unique id and keep
`default_config: false` for custom agents.

## Release view

Release builds copy built-in agents to:

```text
<release-root>/agents/src/
```

Add a custom agent next to the built-ins:

```text
<release-root>/
  agents/
    src/
      code-reviewer/
        agent_config.json
        prompt.md
```

Start with the release root set:

```powershell
$env:TURA_PROJECT_ROOT = "C:\\path\\to\\tura-release"
tura --agent code-reviewer "Review this diff."
```

If your release was built with the `--binary` / `-Binary` option, runtime prompt
and registry files may not be copied beside the binaries. In that case, either
use a full release layout or point `TURA_PROJECT_ROOT` at a directory containing
the expected `agents/src` tree.

## Source view

From a checkout, create:

```text
agents/src/code-reviewer/agent_config.json
agents/src/code-reviewer/prompt.md
```

Then run:

```powershell
$env:TURA_PROJECT_ROOT = "C:\\Users\\you\\Documents\\tura"
cargo test -q -p agents
cargo run -p gateway --bin tura_exec -- --agent code-reviewer "Review this code."
```

If the custom agent changes runtime activation, capability selection, or provider
route behavior, add or run focused runtime tests too.

## Minimal custom agent

`agents/src/code-reviewer/agent_config.json`:

```json
{
  "agent_name": "code-reviewer",
  "description": "Reviews changes for bugs, regressions, and missing tests.",
  "aliases": ["reviewer"],
  "icon_emoji": "R",
  "agent_directory": "agents/src/code-reviewer",
  "parent_agent_id": null,
  "report_to_user": true,
  "default_config": false,
  "reflection": false,
  "op_manual": true,
  "self_reflection": false,
  "provider": {
    "current_model": null,
    "default_model_tier": "thinking",
    "max_tokens": 0,
    "model_acceleration_enabled": true,
    "model_reasoning_effort": "high",
    "service_tier": "priority",
    "stream": true,
    "temperature": 0.2,
    "time_out_ms": 120000,
    "tool_choice": "Auto",
    "tura_llm_name": "thinking"
  },
  "agent_prompt": [
    {
      "agent_prompt": "code-reviewer",
      "prompt_directory": "agents/src/code-reviewer"
    }
  ],
  "agent_capabilities": [
    { "capability_name": "shells" },
    { "capability_name": "web_discover" },
    { "capability_name": "task_status" }
  ],
  "validator": {
    "need_validator": false,
    "validator_name": null
  }
}
```

`agents/src/code-reviewer/prompt.md`:

```md
# Code reviewer agent

Review code in a risk-first order.

Lead with bugs, regressions, security issues, missing tests, or broken user
contracts. Cite file paths and line numbers for every finding. Do not summarize
before findings. If there are no findings, say that clearly and list remaining
test gaps.
```

## Important fields

| Field | Meaning |
| --- | --- |
| `agent_name` | Canonical id. Should match the directory name. |
| `description` | UI/registry description. |
| `aliases` | Alternate ids matched case-insensitively. |
| `agent_directory` | Project-root-relative agent directory. |
| `report_to_user` | Whether normal progress/final reporting should be user-visible. |
| `default_config` | Built-ins use `true`; custom agents should use `false`. |
| `reflection` | Enables reflective task/objective prompt behavior. |
| `op_manual` | Allows runtime prompt operation manuals selected by `task_status.task_type`. |
| `self_reflection` | Enables self-reflection behavior where supported. |
| `provider` | Model tier or exact model override plus call options. |
| `agent_prompt` | Prompt resource directories. Runtime reads `prompt.md` from each directory. |
| `agent_capabilities` | Command-run capabilities available to the agent. |
| `validator` | Validator dispatch settings. Usually disabled unless you wire a validator path. |

## Provider block

The provider block can either route through a tier or pin a concrete model.

Tier route:

```json
{
  "default_model_tier": "thinking",
  "tura_llm_name": "thinking",
  "current_model": null
}
```

Exact override:

```json
{
  "current_model": "openai/gpt-example-pro",
  "default_model_tier": "thinking",
  "tura_llm_name": "thinking"
}
```

If `current_model` is set, it wins over `routes.<tier>` in provider config. That
is useful for a specialized agent and confusing if forgotten. Check this first
when route changes seem ignored.

## Command capabilities

Common capability names:

| Capability | Effect |
| --- | --- |
| `shells` | Enables the active platform shell command alias. |
| `shell_command` | Enables explicit shell command execution. |
| `apply_patch` | Enables workspace file edits through patch grammar. |
| `web_discover` | Enables web/media discovery. |
| `task_status` | Enables task state, operation-manual selection, and compaction. |
| `read_media` | Enables media/document inspection when allowed. |
| `generate_media` | Enables media generation when allowed. |
| `planning` | Enables optional planning/delegation flow. |

Keep the list narrow. Do not give an agent `apply_patch` if its job is only to
review. Do not give media generation to a backend-only agent. The model will use
what you expose; shocking, but consistent.

Runtime prompt manuals can add session capabilities later. For example, visual
manuals can add `read_media` and `generate_media` after `task_status.task_type`
selects a visual task.

## Operation manual policy

Set `op_manual: true` when the agent should use task-specific runtime manuals.
This is appropriate for general engineering agents.

Set `op_manual: false` for lightweight text-only agents that should not load
large task manuals. If this is disabled, `task_type` may still be recorded, but
manual injection depends on session policy.

## Selecting an agent

Common entry points:

```sh
tura --agent code-reviewer "Review this branch."
tura --agent reviewer "Review this branch."
```

The second command works because `reviewer` is an alias.

Clients and gateway sessions can also pass the agent id in session input.

## Validation

From source:

```sh
cargo test -q -p agents
cargo test -q -p runtime agent_router
```

Manual smoke checks:

```sh
tura --agent code-reviewer "Say which agent prompt is active in one sentence."
tura --agent reviewer "Do a read-only review of this repository."
```

Check that:

- the agent appears in registry/UI listings;
- aliases resolve;
- `prompt.md` is read from the configured `prompt_directory`;
- command-run only exposes intended commands;
- provider routing uses either `current_model` or the expected tier;
- `op_manual` behavior matches the agent's purpose.

## Common failures

| Symptom | Likely cause |
| --- | --- |
| Unknown agent | Wrong `TURA_PROJECT_ROOT`, missing `agent_config.json`, or id mismatch. |
| Prompt not loaded | `agent_prompt[].prompt_directory` points to the wrong directory or lacks `prompt.md`. |
| Tool unavailable | Missing `agent_capabilities` entry or no active runtime manual injected it. |
| Provider route ignored | `provider.current_model` is set and overrides the tier route. |
| Agent is hard to delete/update | You set `default_config: true`; custom agents should not do that. |
