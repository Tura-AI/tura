# Custom runtime prompt manuals

Runtime prompt manuals are task-specific operation manuals selected by
`task_status.task_type`. They are neither personas nor agent prompts. They define
how Tura should work for a class of task: debugging, frontend, visual work,
editorial work, refactoring, DevOps, and similar modes. The distinction matters
because temporary task discipline should not become permanent personality.

Use a runtime prompt manual when the instruction should apply only while a task
type is active. Do not paste every preference into every agent prompt. That is
how prompts become furniture storage.

## Manual structure

Each manual is a directory containing:

```text
<runtime-prompt-root>/<manual_id>/
  prompt_identity.json
  prompt.md
```

`prompt_identity.json` defines the catalog entry:

```json
{
  "id": "security_review",
  "display_name": "Security Review Operation Manual",
  "description": "Use for security-focused code and configuration review.",
  "father_ids": ["debug"],
  "capabilities": ["shells"]
}
```

`prompt.md` contains the manual text injected into the provider context when the
manual is active.

## Discovery order

Runtime manual discovery uses this order:

| Priority | Root |
| --- | --- |
| 1 | `TURA_RUNTIME_PROMPT_ROOT`, when set |
| 2 | `<runtime crate>/src/runtime_prompt` in a source/release runtime layout |
| 3 | Embedded manuals compiled into the runtime binary |

If the disk catalog is missing or empty, Tura falls back to embedded manuals.
That keeps binary releases usable, but it also means a custom manual directory
must be discoverable if you want to add new manual ids.

## Release view

Full release builds copy bundled manuals to:

```text
<release-root>/crates/runtime/src/runtime_prompt/
```

You have two sane options.

### Option A: external manual root, recommended

Create a separate manual catalog:

```text
D:\\tura-runtime-prompts\\
  debug/
    prompt_identity.json
    prompt.md
  security_review/
    prompt_identity.json
    prompt.md
```

Set:

```powershell
$env:TURA_RUNTIME_PROMPT_ROOT = "D:\\tura-runtime-prompts"
```

Important: when `TURA_RUNTIME_PROMPT_ROOT` is set, it becomes the catalog root.
Include any bundled manuals you still need, or copy the shipped catalog first
and then add your custom manual. If your catalog contains only
`security_review`, then only that id is available from that root.

### Option B: edit the release catalog

Add a directory under:

```text
<release-root>/crates/runtime/src/runtime_prompt/security_review/
```

This is simple, but release updates may replace the directory. Keep a copy of
your custom manuals outside the release if you choose this path.

If the release was built with `--binary` / `-Binary`, runtime files are not
copied beside the binary. Use `TURA_RUNTIME_PROMPT_ROOT` for customization.

## Source view

Bundled manuals live under:

```text
crates/runtime/src/runtime_prompt/<manual_id>/
```

For product changes, add the manual there and commit it. For experiments, prefer
an external root:

```powershell
$env:TURA_RUNTIME_PROMPT_ROOT = "C:\\tmp\\tura-runtime-prompts"
cargo test -q -p runtime runtime_prompt
```

When adding a bundled manual, run runtime tests because the manual catalog also
feeds the `task_status` schema, prompt catalog, parent expansion, and command-run
capability injection.

## Minimal custom manual

`prompt_identity.json`:

```json
{
  "id": "security_review",
  "display_name": "Security Review Operation Manual",
  "description": "Use for security-sensitive code, config, dependency, and deployment review.",
  "father_ids": ["debug"],
  "capabilities": ["shells"]
}
```

`prompt.md`:

```md
## Security Review Operation Manual

Use this prompt for security-sensitive review.

First identify trust boundaries, secret handling, authentication, authorization,
input parsing, filesystem access, network calls, dependency execution, and
deployment exposure. Do not propose broad rewrites before locating the concrete
risk surface. Every finding must include file path, line number, exploitability,
impact, and the smallest safe fix.

Before marking the work complete, verify that the changed or reviewed surface is
covered by a relevant test, static check, or explicit manual inspection note.
```

## Identity fields

| Field | Meaning |
| --- | --- |
| `id` | `task_type` id. Use lowercase ASCII, `_`, or `-`. |
| `display_name` | Human-readable manual name. |
| `description` | Catalog description shown to the model and task-status schema. |
| `father_ids` | Parent manuals to load before this manual. |
| `capabilities` | Command-run capabilities this manual adds while active. |

Parent manuals are expanded before children. If `security_review` has
`father_ids: ["debug"]`, selecting `security_review` loads `debug` first, then
`security_review`.

## Capability injection

Manual capabilities extend the active session's command-run command set. For
example:

```json
{
  "capabilities": ["read_media", "generate_media"]
}
```

This does not expose the commands globally forever. Runtime records the added
capabilities for the active session and avoids duplicate capability prompt
records until the next compaction boundary.

Only list commands the manual truly needs. A text-only review manual does not
need image generation. Shocking restraint, useful results.

## Selecting a manual

Manuals are normally selected through `task_status.task_type` inside a session:

```json
{
  "task_group": "security review",
  "task_type": ["security_review"],
  "status": "doing"
}
```

An agent must allow operation manual injection for the manual text to be added
normally. In agent config, keep:

```json
{
  "op_manual": true
}
```

Runtime may also inject manuals when goal or reflection mode enables manual
injection, but for normal custom work, set `op_manual: true` on the agent.

## Override an existing manual

To override `debug`, create a custom root containing a `debug` directory with the
same id:

```text
custom-runtime-prompts/
  debug/
    prompt_identity.json
    prompt.md
```

Then set `TURA_RUNTIME_PROMPT_ROOT` to `custom-runtime-prompts`.

Because this replaces the disk catalog root, copy every other manual you still
need into the same custom root. Otherwise valid `task_type` ids shrink to only
the manuals present in your custom catalog.

## Validation

From source:

```sh
cargo test -q -p runtime runtime_prompt
cargo test -q -p runtime task_status
```

Manual smoke checks:

```sh
TURA_RUNTIME_PROMPT_ROOT=/path/to/runtime-prompts tura exec "Use task_type security_review and inspect this repo."
```

Check that:

- the new id appears in the task-status catalog/schema;
- parent manuals are expanded in the expected order;
- manual text appears only after `task_type` is selected;
- capabilities listed in `prompt_identity.json` are injected once per compaction
  boundary;
- the active agent has `op_manual: true` if manual text should be loaded.

## Common failures

| Symptom | Likely cause |
| --- | --- |
| `task_type` id rejected | Manual directory missing, invalid `prompt_identity.json`, or wrong `TURA_RUNTIME_PROMPT_ROOT`. |
| Built-in task types disappear | Custom root contains only your new manual; copy the full catalog if needed. |
| Manual text not injected | Agent has `op_manual: false` or session manual injection is disabled. |
| Capability not available | Capability name is wrong or the command is not registered/allowed. |
| Release ignores manual files | You are using a binary-only release layout; set `TURA_RUNTIME_PROMPT_ROOT`. |
