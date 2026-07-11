# Agents

Agents are Tura's configurable runtime profiles. They do not just rename the
assistant. An agent selects the prompt resources, the command capabilities, the
provider/model route, the operation-manual policy, and a few reporting and
validation flags used when the runtime builds a turn.

That split is deliberate. In many agent frameworks, a custom agent is mostly a
large system prompt plus a model name. The result is easy to create, but hard to
audit: tool access, prompt behavior, model choice, and completion discipline are
mixed into one text blob. Tura keeps those concerns in separate fields so each
one can be inspected, surfaced in clients, restricted by runtime, and changed
without rewriting the whole prompt.

The canonical implementation lives in
[`agents/src/store.rs`](../../agents/src/store.rs). Runtime activation converts
stored configs into `AgentManagement` in
[`crates/runtime/src/agent_router/mod.rs`](../../crates/runtime/src/agent_router/mod.rs).

## What an agent controls

Each runtime-loaded agent lives under `agents/src/<agent_id>` and normally has
two files:

| File | Purpose |
| --- | --- |
| `agent_config.json` | Machine-readable runtime configuration. |
| `prompt.md` | Agent-specific instructions injected as prompt resources. |

The main `agent_config.json` fields are:

| Field | Meaning |
| --- | --- |
| `agent_name` | Canonical id. It should match the directory name. |
| `description` | Human-readable summary for gateway, TUI, and GUI listings. |
| `aliases` | Alternate names matched case-insensitively. |
| `icon_emoji` | Optional display hint for clients. It is not behavior. |
| `agent_directory` | Repository-relative directory containing the agent files. |
| `parent_agent_id` | Optional parent id reserved for hierarchy. |
| `report_to_user` | Whether the agent should surface normal progress/reporting to the user. |
| `default_config` | Marks built-ins as protected static configs. |
| `reflection` | Enables reflective task/objective prompt style for the session. |
| `op_manual` | Allows Runtime Prompt operation manuals for selected `task_type` values. |
| `self_reflection` | Separate self-reflection behavior flag. |
| `provider` | Model tier, optional exact model override, streaming, temperature, tool-choice, timeout, reasoning, and acceleration options. |
| `agent_prompt` | Prompt resources associated with this agent, normally `prompt.md` in the same directory. |
| `agent_capabilities` | Command capability ids exposed through `command_run`. |
| `validator` | Validator dispatch settings. Built-ins currently disable validator dispatch. |

Built-in examples show the separation:

| Agent | Model route | Operation manuals | Capability set | Behavior intent |
| --- | --- | --- | --- | --- |
| `balanced` | `thinking`, with `current_model` set when configured | enabled | patching, shell commands, web discovery, task status | Careful engineering default with verification discipline. |
| `direct` | `fast` route preference, with current model override support | disabled | same base command set | Lower-friction direct work. |
| `direct-text-only` | `fast` route preference, with current model override support | disabled | same base command set | Direct response mode with lighter operating-manual overhead. |

The important point is that these are not three copied prompts with vague names.
They are different runtime contracts.

## Customizable capabilities

`agent_capabilities` lists the command ids the agent may use. Tura exposes them
through one model-visible `command_run` tool instead of handing the provider a
large pile of independent tools.

Typical capability ids include:

| Capability | Runtime effect |
| --- | --- |
| `apply_patch` | Enables focused patch edits through the command-run patch command. |
| `shells` | Enables the active shell command alias for the host platform. |
| `shell_command` | Enables explicit shell command execution when declared directly. |
| `web_discover` | Enables public website/media discovery. |
| `task_status` | Enables structured task state, operation-manual selection, and compaction handoffs. |
| `read_media` | Enables inspected media/document reads when the capability is present or injected by an active manual. |
| `generate_media` | Enables media generation when the capability is present or injected by an active manual. |
| `planning` | Optional multi-task planning/delegation path for agents meant to use it. |

Capabilities are canonicalized before execution. `command_run` itself is the
outer macro tool; capability names inside it become the allowed command types.
If a capability list is present but does not produce concrete command names,
runtime falls back to the default command-run set: patching, the active shell,
web discovery, and task status.

This is an advantage over ordinary agent stacks that expose every installed tool
to every agent and trust the prompt to say "do not use that one". In Tura, the
allowed command list is part of the provider schema and execution path. The model
gets fewer irrelevant actions, and runtime has a narrower surface to validate.

## Capability parameters

Agent capability items currently need only `capability_name`; custom configs may
also include `capability_directory` to point at the tool implementation root:

```json
{
  "capability_name": "web_discover",
  "capability_directory": "crates/tools/src"
}
```

The directory is resolved relative to the project root unless it is already
absolute. Runtime uses it to find the command-run schema and command prompt
formats. Built-in configs omit the directory because the runtime can resolve the
standard tools root from the project.

Runtime Prompt manuals can extend the session with additional command
capabilities after an agent is selected. For example, visual/editorial manuals
can add `read_media` or `generate_media` only when the active `task_type` needs
them. The final allowed command set for a turn is:

```text
agent capabilities + session capabilities injected by active manuals
```

That mechanism keeps the base agent small while still allowing specialized work
to load specialized tools at the moment they are actually required.

## Prompt resources

`agent_prompt` binds the agent to prompt resources:

```json
{
  "agent_prompt": "balanced",
  "prompt_directory": "agents/src/balanced"
}
```

The loader reads `prompt.md` from the agent directory. This prompt describes how
the agent should work: coding discipline, verification behavior, communication
style, tool-use rules, and collaboration expectations.

Prompt resources are intentionally separate from personas and Runtime Prompt
manuals:

| Layer | Owns | Does not own |
| --- | --- | --- |
| Agent prompt | Work style, tool discipline, default behavior. | UI avatar identity, task-type manuals, provider catalog. |
| Persona | Visible identity, tone, optional media expressions. | Engineering capabilities or provider route. |
| Runtime Prompt manual | Task-specific operating rules selected by `task_type`. | The agent's base identity or default model route. |

This separation is where Tura differs from prompt-only agents. You can change a
persona without giving it more tools. You can enable a visual manual without
copying visual rules into every agent prompt. You can switch a model route
without editing behavior prose. Less soup, fewer mysterious side effects.

## Provider and model configuration

The `provider` object is the agent's default runtime model policy. Common fields
are:

| Field | Meaning |
| --- | --- |
| `default_model_tier` | Stable route name such as `thinking` or `fast`. Runtime uses this first. |
| `tura_llm_name` | Legacy/stable route name fallback when `default_model_tier` is absent. |
| `current_model` | Optional exact provider/model override such as `codex/gpt-5.6`. |
| `stream` | Whether responses should stream. |
| `temperature` | Provider call temperature when supported. |
| `max_tokens` | Output limit; `0` means defer to provider/runtime defaults. |
| `tool_choice` | Tool-call policy, usually `Auto`. |
| `time_out_ms` | Base timeout before tier-specific runtime timeout logic is applied. |
| `model_reasoning_effort` | Reasoning-effort hint, commonly `low`, `medium`, `high`, `xhigh`, or `max`; `max` is sent only to GPT-5.6 models and maps to `xhigh` for older models. |
| `model_acceleration_enabled` | Client/runtime hint that priority acceleration is enabled. |
| `service_tier` | Provider-specific acceleration tier, forwarded only when supported. |

Runtime resolves provider selection in two stages:

1. Select the route from `default_model_tier` or `tura_llm_name`.
2. Override the selected provider/model with `current_model` or a session-level
   model override when present.

The route catalog, provider credentials, model limits, latency tiers, and
fallback candidates belong to provider configuration, not to the agent. The
agent says "use the thinking tier" or "use this exact current model"; the
provider layer decides which concrete provider config, base URL, timeout policy,
and model metadata are valid.

This gives Tura a cleaner comparison point against other agents that hard-code a
model directly in each bot profile. Tura can keep an agent stable while changing
route candidates, provider credentials, or latency policy centrally.

## Difference from ordinary agents

The same task in a conventional agent stack often needs several loosely coupled
configuration locations: a system prompt, a tool allowlist, a model selector, a
few UI settings, and a memory policy. They may all be real, but they are usually
not one auditable runtime contract.

Tura's agent config makes the contract explicit:

| Same problem | Ordinary agent behavior | Tura behavior |
| --- | --- | --- |
| Tool access | Expose broad tools and rely on prompt wording to discourage misuse. | Restrict `command_run` commands from `agent_capabilities` plus active session capabilities. |
| Task-specific instructions | Paste many skills/manuals into every turn or trigger them loosely. | Load Runtime Prompt manuals through `task_status.task_type`, only when active. |
| Model selection | Store a model name inside the agent profile. | Store route intent in the agent; resolve exact provider/model through provider config and overrides. |
| Prompt customization | Put behavior, persona, task rules, and tool policy into one long prompt. | Split agent prompt, persona, runtime manual, provider route, and command capabilities. |
| Long sessions | Hope the transcript carries state. | Persist session state, task type, injected capabilities, and compact context records. |
| Client visibility | UI often sees only a name and model. | Gateway/TUI/GUI can list id, aliases, description, provider summary, path, source, and capabilities. |

The advantage is not that every field is unique. The advantage is that the
fields are separate and runtime-owned. That makes custom agents easier to audit,
safer to narrow, and cheaper to run because specialized prompt text and tools do
not have to be loaded for every turn forever.

## Runtime mechanism

The normal flow is:

1. A caller selects an agent by id or alias. CLI uses `--agent` / `--agent-id`;
   gateway and clients can also pass agent settings.
2. `agents/src/store.rs` discovers `agents/src/<agent_id>`, loads
   `agent_config.json`, reads optional `prompt.md`, marks `default_config`
   agents as static, and builds a summary.
3. `crates/runtime/src/agent_router/mod.rs` converts the stored config into
   `AgentManagement`, resolving prompt and capability directories relative to
   the project root.
4. Runtime builds the provider-visible prompt from session records, agent prompt
   resources, persona/runtime context, task status, and any active Runtime
   Prompt manuals.
5. Runtime builds a single `command_run` provider tool. The command schema is
   restricted to the agent's allowed commands plus session capabilities injected
   by active manuals.
6. Provider routing resolves the agent's model tier or exact current model into
   a concrete provider, model name, base URL, and timeout behavior.
7. Tool execution rechecks the allowed command-run commands before dispatching
   local tools.
8. `task_status` updates can change `task_type`, which may inject new operation
   manuals and new session capabilities for later turns.

The design gives Tura three control planes that work together:

| Control plane | Runtime object | Primary purpose |
| --- | --- | --- |
| Agent config | `AgentManagement` | Default prompt, tools, model route, reporting, validator policy. |
| Session state | `SessionManagement` | Active task type, injected capabilities, compaction, goal/task state. |
| Provider config | `Settings` / route catalog | Concrete providers, models, auth, limits, latency, fallback behavior. |

That is the mechanism behind custom agents in Tura: the agent chooses the
default contract, the session narrows or extends it for the current task, and the
provider layer turns route intent into a real model call.

## Minimal custom agent

```json
{
  "agent_name": "code-reviewer",
  "description": "Reviews changes for bugs, risk, and missing tests.",
  "aliases": ["reviewer"],
  "agent_directory": "agents/src/code-reviewer",
  "report_to_user": true,
  "default_config": false,
  "reflection": false,
  "op_manual": true,
  "self_reflection": false,
  "provider": {
    "default_model_tier": "thinking",
    "tura_llm_name": "thinking",
    "stream": true,
    "temperature": 0.2,
    "max_tokens": 0,
    "tool_choice": "Auto",
    "time_out_ms": 120000,
    "model_reasoning_effort": "high",
    "model_acceleration_enabled": true,
    "service_tier": "priority"
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

Add `agents/src/code-reviewer/prompt.md` next to it. Run `cargo test -p agents`
after changing loader-visible fields. If the new agent changes runtime
activation, capability selection, or provider route behavior, add runtime tests
for those paths as well.
