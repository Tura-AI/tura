# Personas Crate Architecture

`personas` owns persona identity, prompt resources, avatar media, and
expression-to-media mapping. Agents bind to personas by `persona_name`; runtime
loads the prompt fragments and gateway/front-end surfaces can read the media
metadata.

Personas do not own runtime diagnostics. Session and task history belongs in
`crates/session_log`; provider-call diagnostics live under `log/provider/` or
`LOG_PATH`.

## Current Layout

Built-in personas live under `personas/src/<persona_id>`.

```text
personas/
  Cargo.toml
  ARCHITECTURE.md
  src/
    lib.rs
    store.rs
    state_machine.rs
    expression_manifest.json
    <persona_id>/
      persona_config.json
      prompt/
        persona.md
        communication_style.md
      media/
        expressions/
          <expression_id>/
            frames/
              center.png
              up.png
              down.png
              left.png
              right.png
              up-left.png
              up-right.png
              down-left.png
              down-right.png
            grid/
              sheet.png
```

User-created dynamic personas live under project-root `personas/<persona_id>/`
with the same `persona_config.json`, `prompt/`, and optional `media/` shape.
The loader scans dynamic personas first, then static built-ins. The first
persona with a given lowercased id wins.

## Runtime Loading

`personas/src/store.rs` owns discovery:

1. Resolve the project root from `TURA_PROJECT_ROOT` or the current repository.
2. Scan `personas/<persona_id>` for dynamic personas.
3. Scan `personas/src/<persona_id>` for static personas.
4. Load `persona_config.json`.
5. Load optional `prompt/persona.md`.
6. Load optional `prompt/communication_style.md`.
7. Enrich media expressions from `personas/src/expression_manifest.json`.

Static personas with `default_config: true` are protected from deletion.
Dynamic personas are expected to use `default_config: false`.

## Persona Config

`persona_config.json` is the source of truth for:

- `persona_name`: canonical id.
- `display_name`, `description`, and `short_description`.
- `default_config`: `true` for protected built-ins, `false` for user-created
  personas.
- `persona_directory`: repository-relative or project-root-relative persona
  directory.
- `prompt_directory`: directory containing `persona.md` and
  `communication_style.md`.
- `media`: optional avatar media mapping.
- `metadata`: free-form non-secret metadata.

The front-end should not infer media paths from naming conventions. It should
consume the media mapping returned by persona or agent APIs.

## Manual Persona Configuration

To add a built-in persona:

1. Create `personas/src/<persona_id>/`.
2. Add `persona_config.json`.
3. Add `prompt/persona.md`.
4. Add `prompt/communication_style.md`.
5. Set `persona_directory` to `personas/src/<persona_id>`.
6. Set `prompt_directory` to `personas/src/<persona_id>/prompt`.
7. Set `default_config: true` only for protected built-ins.
8. Add optional media under `media/expressions`.
9. Run `cargo test -p tura-personas` if persona loader behavior changes.

To add a user-created persona manually, use `personas/<persona_id>/` instead and
set `default_config: false`.

Minimal dynamic persona example:

```json
{
  "persona_name": "my-persona",
  "display_name": "My Persona",
  "description": "Custom assistant persona.",
  "short_description": "Custom",
  "default_config": false,
  "persona_directory": "personas/my-persona",
  "prompt_directory": "personas/my-persona/prompt",
  "media": null,
  "metadata": {}
}
```

## Binding A Persona To An Agent

Agents reference personas through `agent_persona` in
`agents/src/<agent_id>/agent_config.json`:

```json
{
  "agent_persona": [
    {
      "persona_name": "tura",
      "persona_directory": "personas/src/tura/prompt"
    }
  ]
}
```

`persona_directory` points at the prompt directory used by the runtime prompt
assembly path. The persona config itself still lives at
`personas/src/<persona_id>/persona_config.json` for built-ins or
`personas/<persona_id>/persona_config.json` for dynamic personas.

## Expression Manifest

`personas/src/expression_manifest.json` is the canonical expression and emoji
mapping file. Do not keep per-persona or per-expression emoji mapping files.
Persona loading enriches each expression from this manifest at runtime.

## State Machine

`src/state_machine.rs` mirrors the agent-management style with a small persona
lifecycle:

- `Draft`
- `Active`
- `Archived`
- `Error`

Archived personas are terminal. Static/default personas are loaded as `Active`.

## Router Boundary

Router owns persona registry commands:

- `registry-personas-list`
- `registry-persona-get <id>`
- `registry-persona-create`
- `registry-persona-update <id>`
- `registry-persona-delete <id>`

Gateway exposes HTTP endpoints and delegates registry work to router CLI.
Runtime only consumes resolved agent/persona specs.
