# Custom personas

Personas control Tura's user-facing voice, communication style, and optional
avatar/expression metadata. They do not control tool access, model choice, or
task-specific operation manuals. That boundary is intentional: changing the
assistant's voice should not accidentally grant shell access. Small mercy.

This document explains persona customization from two views:

- **release view**: configure personas in a built release directory;
- **source view**: configure or develop personas in the repository checkout.

## Persona roots

Persona discovery uses the project root and scans in this order:

| Priority | Root | Purpose |
| --- | --- | --- |
| 1 | `<project-root>/personas/<persona_id>` | User/dynamic personas. These override built-ins with the same id. |
| 2 | `<project-root>/personas/src/<persona_id>` | Built-in/static personas shipped with Tura. |

The project root normally comes from `TURA_PROJECT_ROOT`. If it is not set,
runtime tries to infer a root from the active agent path or executable layout.
Set the variable explicitly when customizing. Guesswork is not a configuration
strategy.

## Release view

Release builds copy built-in personas to:

```text
<release-root>/personas/src/<persona_id>/
```

Create custom personas beside that static directory, not inside it:

```text
<release-root>/
  personas/
    my-reviewer/
      persona_config.json
      prompt/
        persona.md
    src/
      tura/
      pidan/
      wonderful/
```

Start with:

```powershell
$env:TURA_PROJECT_ROOT = "C:\\path\\to\\tura-release"
$env:TURA_SESSION_PERSONA = "my-reviewer"
tura exec "Review this answer style."
```

For CLI sessions, the rich persona identity layer is skipped when
`TURA_FRONTEND_SOURCE=cli`; runtime loads the shared CLI communication style
instead. GUI/messaging sessions load the active persona prompt plus the shared
GUI communication style.

## Source view

Built-in personas live here:

```text
personas/src/<persona_id>/
```

Dynamic personas live here:

```text
personas/<persona_id>/
```

For source development, use dynamic personas unless you intentionally want to
change a built-in persona:

```powershell
$env:TURA_PROJECT_ROOT = "C:\\Users\\you\\Documents\\tura"
$env:TURA_SESSION_PERSONA = "my-reviewer"
cargo test -q -p personas
```

## Minimal persona

Create this structure:

```text
personas/my-reviewer/
  persona_config.json
  prompt/
    persona.md
```

`persona_config.json`:

```json
{
  "persona_name": "my-reviewer",
  "display_name": "My Reviewer",
  "description": "Direct review voice for internal engineering work.",
  "short_description": "Direct reviewer",
  "default_config": false,
  "persona_directory": "personas/my-reviewer",
  "prompt_directory": "personas/my-reviewer/prompt",
  "metadata": {
    "owner": "local"
  }
}
```

`prompt/persona.md`:

```md
# Persona

You are a concise engineering reviewer.

Keep a direct tone. Lead with the risk or conclusion. Do not add theatrical
encouragement. When code is involved, cite file paths and line numbers. If the
user is casual, stay light; if the work is serious, stay serious.
```

The `persona_name` should match the directory name. Use lowercase ASCII letters,
numbers, `_`, or `-` for ids.

## Persona config fields

| Field | Meaning |
| --- | --- |
| `persona_name` | Canonical id. Runtime selects this via `TURA_SESSION_PERSONA`. |
| `display_name` | UI-facing name. |
| `description` | Longer UI description. |
| `short_description` | Compact label for pickers. |
| `default_config` | Built-ins use `true`; user personas should use `false`. |
| `persona_directory` | Project-root-relative persona directory. |
| `prompt_directory` | Directory containing `persona.md`. |
| `media` | Optional avatar/expression metadata. |
| `metadata` | Free-form extra metadata. |

Dynamic personas with the same id as a built-in persona override the built-in.
This is the safest way to customize a shipped persona without editing release
files that may be replaced on update.

## Communication style files

Shared communication style files live under the static persona root:

```text
personas/src/communication_style/communication_style.md
personas/src/communication_style/cli_communication_style.md
```

They define surface-level behavior such as GUI HTML formatting, media attachment
tokens, reactions, final delivery rules, and CLI-safe output. Edit them only when
you want to change behavior across personas.

Use persona prompts for identity and voice. Use communication style files for
output contract and interface behavior.

## Optional media and expressions

A persona can include avatar media:

```text
personas/my-reviewer/
  media/
    expressions/
      vigilant/
        frames/
          center.png
          right.png
        grid/
          sheet.png
```

Then add a `media` block:

```json
{
  "media": {
    "name": "My Reviewer avatar media",
    "root_directory": "personas/my-reviewer/media",
    "expression_directory": "personas/my-reviewer/media/expressions",
    "direction_order": ["center", "right"],
    "default_expression": "vigilant",
    "default_direction": "right",
    "expressions": [
      {
        "id": "vigilant",
        "source_directory": "personas/my-reviewer/media/expressions/vigilant",
        "grid_path": "personas/my-reviewer/media/expressions/vigilant/grid/sheet.png",
        "frames": {
          "center": "personas/my-reviewer/media/expressions/vigilant/frames/center.png",
          "right": "personas/my-reviewer/media/expressions/vigilant/frames/right.png"
        }
      }
    ]
  }
}
```

If you use the shared expression manifest, keep its path project-root-relative:

```json
{
  "expression_manifest": "personas/src/expression_manifest.json"
}
```

Media is optional. A persona without media is valid.

## Selecting a persona

Use an environment variable for runtime sessions:

```powershell
$env:TURA_SESSION_PERSONA = "my-reviewer"
```

For CLI mode:

```powershell
$env:TURA_FRONTEND_SOURCE = "cli"
```

In CLI mode, runtime intentionally prefers the shared CLI communication style and
does not load the rich GUI persona layer.

## Registry operations

The router exposes persona registry operations for clients:

```text
registry-personas-list
registry-persona-get <id>
registry-persona-create
registry-persona-update <id>
registry-persona-delete <id>
```

Static built-ins and `default_config: true` personas are protected from deletion.
Dynamic personas are stored under `<project-root>/personas/<id>`.

## Validation

From source:

```sh
cargo test -q -p personas
cargo test -q -p runtime persona
```

Manual smoke check:

```sh
TURA_PROJECT_ROOT=/path/to/root TURA_SESSION_PERSONA=my-reviewer tura exec "Say one sentence in your active persona."
```

Check that:

- the persona id is listed by the registry/UI;
- `prompt/persona.md` is loaded in GUI sessions;
- CLI sessions use `cli_communication_style.md` instead of GUI rich-text rules;
- a dynamic persona overrides a built-in only when the ids match exactly after
  lowercase normalization;
- media paths are project-root-relative and point to existing files.

## Common failures

| Symptom | Likely cause |
| --- | --- |
| Persona not found | Wrong `TURA_PROJECT_ROOT`, missing `persona_config.json`, or wrong directory id. |
| Built-in wins over custom | Custom persona was placed under the wrong root or has a different `persona_name`. |
| CLI ignores persona voice | `TURA_FRONTEND_SOURCE=cli`; this is expected. CLI uses the shared CLI style. |
| Delete fails | Persona is static or has `default_config: true`. |
| Avatar missing | `media` paths are wrong or files are absent. |
