# Agents Crate Architecture

`agents` is the canonical home for agent definitions. The runtime comes here for
agent configuration, prompt resources, provider route preferences, and tool
capabilities instead of burying them in the MANO/MANAS runtime loop. That keeps
an agent inspectable as a configuration, not a scavenger hunt through execution
code.

The Cargo package and library names stay compatible with Tura:

```text
package = agents
library = tura_agents
```

## Current Layout

All runtime-loaded agent definitions live under `agents/src/<agent_id>`.

```text
agents/
  Cargo.toml
  ARCHITECTURE.md
  src/
    lib.rs
    coding_agent.rs
    store.rs
    direct/
      agent_config.json
      prompt.md
    direct-text-only/
      agent_config.json
      prompt.md
    balanced/
      agent_config.json
      prompt.md
    thoughtful/
      agent_config.json
      prompt.md
```

Legacy root-level layouts such as `agents/<agent_id>/` are not loaded. Agent
discovery, creation, updates, deletion, and API listing all use
`agents/src/<agent_id>` as the single source of truth.

## Runtime Loading

`agents/src/store.rs` owns discovery:

1. Resolve the project root from `TURA_PROJECT_ROOT` or the current repository.
2. Scan direct child directories under `agents/src`.
3. Load `agent_config.json`.
4. Load optional `prompt.md` from the same directory.
5. Mark configs with `default_config: true` as static and protected.
6. Match agent ids and aliases case-insensitively.

There is no priority order between two agent directories. Duplicate ids collapse
by lowercased `agent_name`, with the first discovered entry retained.

## Agent Config

Each agent directory contains:

- `agent_config.json`: runtime-loaded JSON config.
- `prompt.md`: task, behavior, and tool-use guidance for the model.

`agent_config.json` fields used by the current loader include:

- `agent_name`: canonical id. It should match the directory name.
- `description`: human-readable summary for gateway/TUI listings.
- `aliases`: optional accepted names, such as `coding_agent`.
- `agent_directory`: repository-relative path to the agent directory.
- `default_config`: `true` for built-in protected agents, `false` for
  user-created agents.
- `reflection`: controls whether runtime prompt assembly appends reflective
  task-status/objective prompt style for the active agent. Built-ins set this to
  `true` only for `thoughtful`; planning tool availability is configured
  separately through capabilities.
- `provider.tura_llm_name`: named route from
  `crates/provider/config/provider_config.json`.
- `agent_prompt[]`: prompt resources, normally pointing at the agent directory.
- `agent_capabilities[]`: enabled command/tool capability ids.
- `validator`: validator settings; `need_validator: false` disables validator
  dispatch.

The loader summarizes capabilities from `agent_capabilities[].capability_name`
and the provider route from `provider.tura_llm_name`.

## Manual Agent Configuration

To add or edit an agent manually:

1. Create `agents/src/<agent_id>/`.
2. Add `agent_config.json`.
3. Add `prompt.md`.
4. Set `agent_directory` to `agents/src/<agent_id>`.
5. Set `agent_prompt[0].prompt_directory` to the same directory.
6. Choose a provider route through `provider.tura_llm_name`.
7. Enable only the command capabilities the agent should receive.
8. Run `cargo test -p agents` after changing loader-visible fields.

Minimal custom agent example:

```json
{
  "agent_name": "my-agent",
  "description": "Custom Tura agent.",
  "aliases": [],
  "agent_directory": "agents/src/my-agent",
  "report_to_user": true,
  "default_config": false,
  "reflection": false,
  "provider": {
    "tura_llm_name": "thinking",
    "stream": true,
    "temperature": 0.2,
    "max_tokens": 0,
    "tool_choice": "Auto",
    "time_out_ms": 120000
  },
  "agent_prompt": [
    {
      "agent_prompt": "my-agent",
      "prompt_directory": "agents/src/my-agent"
    }
  ],
  "agent_capabilities": [
    {
      "capability_name": "command_run",
      "capability_directory": "crates/tools/src"
    },
    {
      "capability_name": "apply_patch",
      "capability_directory": "crates/tools/src"
    },
    {
      "capability_name": "shell_command",
      "capability_directory": "crates/tools/src"
    },
    {
      "capability_name": "zsh",
      "capability_directory": "crates/tools/src"
    },
    {
      "capability_name": "read_media",
      "capability_directory": "crates/tools/src"
    },
    {
      "capability_name": "web_discover",
      "capability_directory": "crates/tools/src"
    },
    {
      "capability_name": "task_status",
      "capability_directory": "crates/tools/src"
    }
  ],
  "validator": {
    "need_validator": false,
    "validator_name": null
  }
}
```

`planning` is optional and should only be enabled for agents that are expected to
use the multi-task planning runtime path. It does not control reflective
task-status prompt style; use `reflection` for that.

## Provider Route Selection

Agents should reference provider routes by stable tier names such as:

```text
balanced
direct
embedding_high
embedding_low
```

The actual provider/model candidates for those tiers belong in
`crates/provider/config/provider_config.json`. Agent configs express preference;
provider config remains the fixed route/catalog layer.

## Prompt Ownership

Agent prompt resources live with the selected agent:

```text
agents/src/<agent_id>/prompt.md
```

Persona resources are independent from agents and are not loaded by agent
configuration.

## Tests

Agent changes should include the narrowest useful checks:

- `cargo test -p agents` for config discovery and loader behavior.
- Runtime activation tests when provider route, capability selection, or prompt
  assembly behavior changes.
- Gateway/TUI smoke tests when an agent id, alias, or listing behavior changes.
