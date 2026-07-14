# Personas

Personas are Tura's user-facing voice and expression layer. They decide how an
assistant speaks, formats messages, reacts emotionally, and presents rich
conversation artifacts. They do not change what the agent is allowed to do.
That distinction keeps a warmer voice from becoming an accidental permission
system.

The short version: an agent decides what work can be done; a persona decides how
that work is communicated.

The implementation is owned by the `personas` crate. The main runtime assembly
path is in
[`crates/runtime/src/manas/agent_prompts.rs`](../../crates/runtime/src/manas/agent_prompts.rs),
and persona storage/discovery is in
[`personas/src/store.rs`](../../personas/src/store.rs). A lower-level crate
architecture note is available at
[`personas/ARCHITECTURE.md`](../../personas/ARCHITECTURE.md).

## Why personas exist

Many agents treat personality as a paragraph of style text pasted above the real
system prompt. That usually produces a thin costume: a few catchphrases, a
different greeting, and then the same Markdown wall when the work starts.

Tura personas are meant to solve a different problem. Users do not only need the
right answer; they need the answer delivered in a communication mode that fits
the surface they are using and the work they are doing. A GUI chat, a CLI
terminal, a long code review, and a casual correction should not all receive the
same Markdown-shaped monologue. Tragic that this needs saying, but here we are.

Personas provide:

- a durable identity and voice for the assistant;
- communication-style rules that can be shared or customized;
- rich-text output rules for messaging-app surfaces;
- plain, quiet output rules for CLI surfaces;
- media and expression metadata for reactions, stickers, and avatar states;
- a registry model for built-in and user-created personas.

## Difference from Markdown repeater agents

The practical difference is not that Tura has a "persona prompt" and other
agents do not. The difference is that persona behavior is treated as a runtime
resource with surface-specific output contracts.

| Area | Markdown repeater agent | Tura persona |
| --- | --- | --- |
| Output medium | Assumes Markdown is the universal UI. | Uses messaging-app HTML where supported and CLI-safe plain text in terminals. |
| Tone | Often a generic style paragraph. | Persona voice is separate from shared communication rules and can be customized. |
| Rich interaction | Simulates emotion with extra prose or emoji spam. | Supports concise reactions, stickers, media attachments, and expression metadata. |
| Work updates | Frequently repeats confirmations and process narration. | Communication style defines when to update, how much to say, and what final delivery must include. |
| Formatting | Markdown tables, headers, nested bullets, and code fences everywhere. | Chooses HTML, tables, media tags, links, code spans, or plain paragraphs based on surface and readability. |
| Customization | Usually edits the agent prompt itself. | Stores personas independently from agents; dynamic personas can override built-ins. |
| Runtime boundary | Personality is mixed into capability and task instructions. | Agent capability, runtime manuals, and persona communication remain separate layers. |

Markdown is still useful for documents and source files. It is just a poor
default for every conversational surface. A messaging app can render bold text,
inline code, links, blockquotes, media attachments, and reactions more directly
than a raw Markdown transcript. A terminal, meanwhile, benefits from less markup,
not more. Personas let Tura make that distinction explicitly.

## Customizable communication style

The shared GUI communication style lives at
[`personas/src/communication_style/communication_style.md`](../../personas/src/communication_style/communication_style.md).
It defines the conversation contract for messaging-app style interfaces:

- answer simple questions directly;
- briefly state intent before substantial tool work;
- avoid empty confirmations and decorative roleplay;
- mirror the user's tone without blindly agreeing;
- use HTML tags such as `<b>`, `<i>`, `<code>`, `<blockquote>`, and real links
  when they improve readability;
- use `[MEDIA:file path:MEDIA]` for essential local attachments;
- use `[EMOJI:react:...:EMOJI]` and `[EMOJI:sticker:...:EMOJI]` for lightweight
  emotional beats;
- include changed files, verification commands, and skipped checks in final
  delivery responses when relevant.

The CLI communication style lives at
[`personas/src/communication_style/cli_communication_style.md`](../../personas/src/communication_style/cli_communication_style.md).
It intentionally removes the rich-text layer:

- no HTML;
- no complex Markdown;
- no tables, nested lists, decorative separators, or blockquote-heavy output;
- short plain paragraphs and small flat lists only when they help scanning.

That split is the main advantage over a single Markdown house style. The same
assistant can be expressive in a GUI and quiet in a terminal without pretending
those are the same interface, and the communication layer remains customizable
without rewriting the agent's capability prompt.

## Rich text is a communication API

Tura's rich text rules are not decoration. They are a small communication API
between the assistant and the frontend.

| Markup | Meaning |
| --- | --- |
| `<b>...</b>` | Emphasize the primary fact or result. |
| `<i>...</i>` | Add light secondary emphasis. |
| `<code>...</code>` | Mark commands, file paths, symbols, and exact values. |
| `<pre><code class='language-*'>...</code></pre>` | Preserve readable code blocks. |
| `<a href='https://...'>...</a>` | Link only to real web URLs. |
| `[MEDIA:file path:MEDIA]` | Attach a local file or media artifact. |
| `[EMOJI:react:...:EMOJI]` | Send a compact reaction. |
| `[EMOJI:sticker:...:EMOJI]` | Send a sticker-style expression. |

The benefit is precision. A file attachment is not described as "see attached"
inside a Markdown paragraph; it is represented as an attachment token the app can
render. A reaction is not padded into a sentence; it is a reaction. Code paths
and line references remain visually distinct. The frontend gets structure
instead of guessing from prose.

## Persona mechanism

Personas are stored as structured resources, not as one loose prompt file.

Built-in personas live under `personas/src/<persona_id>/`. User-created dynamic
personas live under `personas/<persona_id>/`. The loader scans dynamic personas
first, then static built-ins, and the first persona with a lowercased id wins.
That gives users a safe override path without editing bundled defaults.

Each persona can contain:

| Resource | Purpose |
| --- | --- |
| `persona_config.json` | Canonical id, display text, paths, default protection, metadata, and optional media config. |
| `prompt/persona.md` | The identity and voice prompt for the persona. |
| shared `communication_style.md` | GUI communication rules applied across personas. |
| shared `cli_communication_style.md` | CLI-specific communication rules. |
| `media/expressions/...` | Optional avatar expression frames and grids. |
| `expression_manifest.json` | Shared expression names, emoji aliases, direction order, and reaction kaomoji. |

The core loader behavior is:

1. Resolve the project root from `TURA_PROJECT_ROOT` or the current repository.
2. Discover dynamic personas from `personas/<persona_id>`.
3. Discover built-in personas from `personas/src/<persona_id>`.
4. Read `persona_config.json` and normalize the persona id.
5. Read optional `prompt/persona.md`.
6. Read shared communication style files, with legacy per-persona style files as
   compatibility fallbacks.
7. Enrich media expressions from `personas/src/expression_manifest.json`.

Runtime then assembles persona prompt messages before the agent prompt. In GUI
sessions, it loads the active persona from `TURA_SESSION_PERSONA`, adds the
persona identity prompt, then adds the GUI communication style. In CLI sessions,
identified by `TURA_FRONTEND_SOURCE=cli`, runtime skips the rich persona layer and
loads the shared CLI communication style instead.

That ordering matters: the persona shapes voice and surface behavior before the
agent-specific working instructions are added. It does not replace the agent,
the runtime prompt manuals, or the tool system.

## Relationship to agents and runtime prompts

Personas are independent from agents.

An agent owns capability: available tools, provider configuration, validation,
and task-oriented prompt resources. Runtime Prompt manuals own task discipline:
debugging, frontend work, visual work, refactoring, and similar operating modes.
A persona owns the communication layer: voice, formatting, reactions, and
surface-specific delivery rules.

Keeping those layers separate prevents two common failures:

1. A friendly persona accidentally weakening engineering rules.
2. A coding agent forcing every user conversation into dense technical Markdown.

The runtime composes the layers instead of merging them into one giant prompt.
That makes persona customization safer: changing the assistant's voice does not
have to change which tools it can run or which completion rules apply.

## Registry and customization

The router exposes persona registry operations for listing, reading, creating,
updating, and deleting dynamic personas:

- `registry-personas-list`
- `registry-persona-get <id>`
- `registry-persona-create`
- `registry-persona-update <id>`
- `registry-persona-delete <id>`

Dynamic personas are stored under the project-root `personas/` directory and are
expected to use `default_config: false`. Static built-ins with
`default_config: true` are protected from deletion. This gives the product a
simple rule: user customization lives beside the project, while bundled personas
remain recoverable.

## Advantages

The main advantages are operational, not theatrical:

- <b>Surface fit:</b> GUI chat can use rich text, reactions, stickers, and media;
  CLI output stays plain and script-friendly.
- <b>Cleaner prompts:</b> communication style is a reusable layer instead of being
  copied into every agent prompt.
- <b>Safer customization:</b> users can change voice and style without editing
  tool policy or task manuals.
- <b>Better UX semantics:</b> attachments, reactions, and code references are
  explicit renderable objects, not prose guesses.
- <b>Consistent delivery:</b> final responses follow a documented contract for
  changed files, URLs, media, tests, and skipped checks.
- <b>Media-ready expression:</b> persona media and expression manifests let the UI
  map emotional states to avatars, stickers, and lightweight reactions.

The result is an assistant that can behave like a working partner in the actual
interface the user chose, instead of dumping the same Markdown-flavored transcript
into every surface and calling it personality.
