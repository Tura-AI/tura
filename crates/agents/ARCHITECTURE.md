# Agents Crate Architecture

`crates/agents` is the canonical home for agent definitions. Runtime code loads
agents from here; it should not hard-code provider defaults, prompt text, or
command lists inside the MANO/MANAS loop.

The Cargo package and library names stay compatible with Tura:

```text
package = tura-agents
library = tura_agents
```

## Layout

All agent-owned runtime files live under `crates/agents/src/{agent_name}`.

```text
crates/agents/
  Cargo.toml
  ARCHITECTURE.md
  src/
    lib.rs
    coding_agent.rs
    coding_agent/
      agent_config.json
      prompt.md
    coding_agent_fast/
      agent_config.json
      prompt.md
```

Legacy `crates/agents/{agent_name}/interface/I{agent_name}.json` and
`agents/interface/I{agent_name}.json` may be supported by compatibility loaders,
but new agent work must use `crates/agents/src/{agent_name}`.

## Agent Config

Each agent owns exactly two runtime-loaded files:

- `agent_config.json`: JSON config consumed by `crates/runtime`.
- `prompt.md`: exact model-facing system prompt text.

`agent_config.json` defines:

- `agent_name`.
- `agent_directory`.
- `provider`.
- `agent_prompt`, whose `prompt_directory` points to this agent directory.
- `agent_capabilities`, currently only `command_run`.
- `validator`.

The coding agents must keep identical capabilities. `coding_agent_fast` differs
from `coding_agent` only by its `prompt.md` content.

## Prompt Ownership

The default coding agent prompt lives at:

```text
crates/agents/src/coding_agent/prompt.md
```

The fast coding agent prompt lives at:

```text
crates/agents/src/coding_agent_fast/prompt.md
```

Runtime prompt loading must send the selected `prompt.md` file text to the provider
unchanged. Moving prompt ownership into `crates/agents/src/{agent_name}` must not
alter the model-facing context.

## Command Selection

The agent selects capabilities by id. The tool system decides how they are
exposed to the model. This version exposes only `command_run`, and
`command_run` internally supports only the active shell command surface plus
`apply_patch`.

Recommended coding-agent config:

```json
{
  "agent_capabilities": [
    {
      "capability_name": "command_run",
      "capability_directory": "crates/tools/src"
    }
  ]
}
```

Additional command groups such as LSP, web, media, or planning are intentionally
disabled for this version.

## Runtime Loading

`crates/runtime` loads agents through the registry loader:

1. Resolve project root.
2. Check `TURA_PROJECT_ROOT` when present.
3. Load `crates/agents/src/{agent_name}/agent_config.json`.
4. Fall back to legacy interface files only for migration.
5. Resolve relative config paths against the project root.
6. Return an activated agent record to the agent state machine.

## Tests

Agent changes should include:

- Config parse test.
- Prompt assembly smoke test.
- Command selection test.
- Runtime activation test through `crates/runtime` when behavior changes.
- E2E context check showing provider-visible prompt and tool schema remain
  unchanged for the selected agent.
