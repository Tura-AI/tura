# Persona Crate Architecture

`personas` owns persona identity, prompt resources, avatar media, and expression-to-media mapping. Agents bind to personas by `persona_name`; the router resolves the persona config and returns media metadata to the gateway/front-end.
Personas do not own runtime diagnostics. Session/task history is queried
through `session_log`; provider call diagnostics are files under
`log/provider/` or `LOG_PATH`.

## Layout

```text
personas/
  Cargo.toml
  ARCHITECTURE.md
  src/
    lib.rs
    store.rs
    state_machine.rs
    <persona_name>/
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

Dynamic user-created personas live under project-root `personas/<persona_id>/` with the same `persona_config.json`, `prompt/`, and optional `media/` shape.

## Config

`persona_config.json` is the source of truth for:

- `persona_name`, `display_name`, `description`.
- `default_config`: protected built-in configs use `true`; user-created configs must use `false` and are the only deletable personas.
- `persona_directory` and `prompt_directory`.
- `media.root_directory`, `media.expression_directory`, defaults, and expression media records.
- `media.expression_manifest`, which points to the single canonical expression/emoji mapping file.
- Per-expression `grid_path` and `frames` paths.

`personas/src/expression_manifest.json` is the canonical expression mapping stored in the persona crate. Do not keep per-persona or per-expression emoji mapping files; persona loading enriches each expression from this manifest at runtime.

The front-end should not infer media paths from naming conventions. It should consume the media mapping returned by the persona or agent APIs.

## State Machine

`src/state_machine.rs` mirrors the agent-management style with a small persona lifecycle:

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

Gateway exposes HTTP endpoints and delegates to router CLI. Runtime only consumes resolved agent/persona specs.
