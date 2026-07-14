# HTML Rich Text

Tura uses a small HTML-flavored rich text protocol for normal interactive
messages and a separate plain terminal style for CLI sessions. That split is not
cosmetic. The model receives communication rules for the frontend in use, and
the clients render a deliberately limited subset instead of trusting arbitrary
markup. Asking every surface to make its own guess about Markdown would be
easier right up to the first streaming table.

The split is simple:

| Mode | Prompt source | Output style | Main renderer |
| --- | --- | --- | --- |
| Normal interactive mode | Persona prompt plus shared `communication_style.md` | Messaging-app HTML subset, media tokens, emoji reaction/sticker tokens | GUI `RichText`, TUI rich renderer |
| CLI mode | Shared `cli_communication_style.md` | Short plain paragraphs, minimal bullets, no HTML, no complex Markdown | TUI terminal renderer |
| Ordinary Markdown agent | Usually one generic Markdown instruction | Markdown syntax emitted directly by the model | Whatever the hosting app guesses |

## Why HTML rich text exists

Markdown is convenient for README files, but it is a loose conversation protocol.
Different clients disagree on tables, links, nested lists, HTML passthrough,
local paths, code fences, and partial streaming states. That is tolerable in a
static document. It is annoying in a live agent UI where messages can contain
local file paths, web links, command summaries, media references, tables, code,
stickers, and progress updates.

Tura's normal interaction style instead asks the model to use a restricted HTML
surface:

```html
Use <b>bold</b>, <i>italic</i>, <code>inline code</code>,
<a href='https://example.com'>web links</a>,
<blockquote>quoted text</blockquote>, and
<pre><code class='language-python'>print('hello')</code></pre>.
```

It also defines explicit non-HTML tokens for media and lightweight expression:

```text
[MEDIA:relative/or/absolute/path.png:MEDIA]
[EMOJI:react:👍:EMOJI]
[EMOJI:sticker:😂:EMOJI]
```

The important detail: these are communication tokens, not browser DOM access.
Clients parse and render them through a small allowlist. Unsupported HTML stays
escaped or is reduced to text. Arbitrary scripts and local-path links are not
made clickable because that would be less a feature and more a trap wearing a
hat.

## Difference from ordinary Markdown agents

An ordinary Markdown-based agent usually relies on the host application to infer
meaning from raw text. That has several failure modes:

- local paths can be confused with URLs;
- Markdown links can make unsafe or nonsensical targets look clickable;
- Markdown tables often overflow or collapse badly in terminals;
- HTML passthrough behavior varies by renderer;
- attachments need an out-of-band UI convention;
- streamed partial Markdown can flicker between parsed and unparsed states;
- the model has no clear distinction between conversational emphasis and
  document Markdown.

Tura's HTML rich text path is narrower but more predictable:

- emphasis maps directly to UI roles: `<b>`, `<i>`, `<u>`, `<s>`, `<code>`;
- web links are explicit `<a href='https://...'>label</a>` links;
- code blocks can carry language metadata through `<pre><code
  class='language-...'>`;
- media is explicit through `[MEDIA:...:MEDIA]`, so clients can group, preview,
  or open assets without guessing from prose;
- reactions and stickers are explicit through `[EMOJI:...:EMOJI]`, so they can
  be shown without leaking protocol text;
- renderers can safely downgrade to plain text or basic ANSI output.

Markdown is still supported where it is useful, especially tables and common
inline emphasis. It is not the primary contract for normal messaging style.

## Normal interactive mode

Normal interactive sessions load persona material and the shared communication
style. The shared style tells the assistant that it is chatting in a messaging
app and may use HTML tags for readability. It also defines media attachments,
stickers, reactions, and final-delivery expectations.

Relevant source:

- [`personas/src/communication_style/communication_style.md`](../../personas/src/communication_style/communication_style.md)
  uses the "Rich Text Formatting", "Attachments", and "Stickers And Reactions"
  sections to define the HTML, media, reaction, and sticker conventions.
- [`personas/src/store.rs`](../../personas/src/store.rs) defines
  `COMMUNICATION_STYLE_DIR`, `COMMUNICATION_STYLE_FILE`, and
  `CLI_COMMUNICATION_STYLE_FILE` for shared communication style discovery.
- [`personas/src/store.rs`](../../personas/src/store.rs) stores
  `communication_style` and `cli_communication_style` separately on
  `StoredPersona`.
- [`personas/src/store.rs`](../../personas/src/store.rs) uses
  `read_communication_style` and `read_cli_communication_style` to load shared
  and persona-local communication style files.
- [`crates/runtime/src/manas/agent_prompts.rs`](../../crates/runtime/src/manas/agent_prompts.rs)
  uses `load_session_persona_messages` to append the active persona and normal
  communication style as system prompt messages when the frontend is not CLI.

In this mode, the assistant's final answer can contain HTML tags and protocol
tokens. The client is responsible for rendering them safely.

## CLI mode

CLI sessions deliberately do not use the normal messaging-app rich text style.
When `TURA_FRONTEND_SOURCE=cli`, runtime skips the active persona rich style and
loads the shared CLI communication style instead.

Relevant source:

- [`personas/src/communication_style/cli_communication_style.md`](../../personas/src/communication_style/cli_communication_style.md)
  uses its "Sending Text" section to explicitly forbid HTML and complex
  Markdown.
- [`crates/runtime/src/manas/agent_prompts.rs`](../../crates/runtime/src/manas/agent_prompts.rs)
  uses `load_session_persona_messages` to choose the CLI communication style
  when the frontend source is CLI.
- [`crates/runtime/src/manas/agent_prompts.rs`](../../crates/runtime/src/manas/agent_prompts.rs)
  uses `active_session_persona` to skip persona loading for CLI sessions.
- [`crates/runtime/src/manas/agent_prompts.rs`](../../crates/runtime/src/manas/agent_prompts.rs)
  uses `load_shared_cli_communication_style` to load
  `cli_communication_style.md` from the shared persona root.
- [`crates/runtime/src/manas/agent_prompts.rs`](../../crates/runtime/src/manas/agent_prompts.rs)
  uses `frontend_source_is_cli` to detect CLI mode from `TURA_FRONTEND_SOURCE`.

That separation matters because terminal output must be script-friendly and
quiet. Raw HTML tags in a pipe, CI log, or plain terminal are not rich text; they
are just noise with angle brackets.

## TUI rendering

The TUI detects terminal capability and chooses one of three rich text levels:

| Capability | `richText` value | Behavior |
| --- | --- | --- |
| Plain | `none` | Strip/downgrade HTML and links to readable text. |
| ANSI | `basicMarkdown` | Render safe basic styling without full rich effects. |
| Rich terminal | `richMarkdown` | Render HTML subset, Markdown tables, OSC 8 web links, media labels, and emoji tokens. |

Relevant source:

- [`apps/tui/src/tui/capabilities.ts`](../../apps/tui/src/tui/capabilities.ts)
  uses `detectTerminalCapabilities` to detect terminal capability.
- [`apps/tui/src/tui/capabilities.ts`](../../apps/tui/src/tui/capabilities.ts)
  defines `plainCapabilities`, `ansiCapabilities`, and `richCapabilities` for
  renderer selection.
- [`apps/tui/src/tui/render-rich-text.ts`](../../apps/tui/src/tui/render-rich-text.ts)
  uses `renderRichText` to route rich text rendering by active capability.
- [`apps/tui/src/tui/render-rich-text.ts`](../../apps/tui/src/tui/render-rich-text.ts)
  uses `plainRichText` and `basicRichText` for downgrade behavior.
- [`apps/tui/src/tui/render-rich-text.ts`](../../apps/tui/src/tui/render-rich-text.ts)
  uses `renderHtmlSubset` to render the allowed HTML subset.
- [`apps/tui/src/tui/render-rich-text.ts`](../../apps/tui/src/tui/render-rich-text.ts)
  uses `renderMediaToken`, `renderLinkTarget`, and `terminalLink` for media
  tokens, links, and OSC 8 terminal links.
- [`apps/tui/src/tui/render-rich-text.ts`](../../apps/tui/src/tui/render-rich-text.ts)
  defines `supportedHtmlTags` and `stripUnsupportedHtml` to enforce the allowed
  HTML tag set.

The TUI is intentionally conservative with links. Web URLs can become OSC 8
links in capable terminals. Local paths are displayed as paths, not silently
turned into clickable Markdown links.

## GUI rendering

The GUI parses the same protocol into typed rich nodes and renders them as Solid
components. It uses `DOMParser` only after escaping unsupported tags, then maps
allowed elements into internal node types.

Relevant source:

- [`apps/gui/app/src/conversation/message-rich-protocol.ts`](../../apps/gui/app/src/conversation/message-rich-protocol.ts)
  defines `RICH_TOKEN_PATTERN` for shared `[MEDIA:...:MEDIA]` and
  `[EMOJI:...:EMOJI]` tokens.
- [`apps/gui/app/src/conversation/message-rich-text.tsx`](../../apps/gui/app/src/conversation/message-rich-text.tsx)
  exposes the `RichText` component and calls `parseRichText`.
- [`apps/gui/app/src/conversation/message-rich-text.tsx`](../../apps/gui/app/src/conversation/message-rich-text.tsx)
  uses `parseRichText` and `parseInlineRichText` to tokenize media/emoji, parse
  Markdown tables, and parse inline HTML with `DOMParser`.
- [`apps/gui/app/src/conversation/message-rich-text.tsx`](../../apps/gui/app/src/conversation/message-rich-text.tsx)
  defines `SUPPORTED_HTML_TAGS` and escapes unsupported tags before parsing.
- [`apps/gui/app/src/conversation/message-rich-text.tsx`](../../apps/gui/app/src/conversation/message-rich-text.tsx)
  uses `readDomNode` and `readTableElement` to map DOM nodes into internal rich
  nodes, including tables and code blocks.
- [`apps/gui/app/src/conversation/message-rich-text.tsx`](../../apps/gui/app/src/conversation/message-rich-text.tsx)
  uses `isSafeUrl` to restrict safe links to `http` and `https` URLs.

The GUI can also group adjacent media nodes into galleries and open image media
in a lightbox. That behavior depends on explicit media tokens, not Markdown image
syntax.

## Supported surface

The practical authoring surface is:

| Need | Preferred syntax |
| --- | --- |
| Strong emphasis | `<b>important</b>` |
| Emphasis | `<i>note</i>` |
| Inline code or paths | `<code>C:/repo/src/main.rs</code>` |
| Web link | `<a href='https://example.com'>Example</a>` |
| Quote or cited excerpt | `<blockquote>quoted text</blockquote>` |
| Code block | `<pre><code class='language-rust'>fn main() {}</code></pre>` |
| Media attachment | `[MEDIA:path-or-url:MEDIA]` |
| Lightweight reaction | `[EMOJI:react:👍:EMOJI]` |
| Sticker-like expression | `[EMOJI:sticker:😂:EMOJI]` |

Tables may be emitted as Markdown tables because both clients have table support.
For most normal prose, prefer short paragraphs and small flat lists. Rich text is
there to preserve structure and intent, not to decorate every sentence until the
message looks like a festival poster.

## Tests and regression coverage

The TUI has focused unit coverage for the protocol:

- [`apps/tui/tests/unit/tui/render-rich-content.test.ts`](../../apps/tui/tests/unit/tui/render-rich-content.test.ts)
  has a `render applies communication style rich text without leaking protocol
  markup` test for communication-style HTML, media, links, and emoji rendering.
- [`apps/tui/tests/unit/tui/render-rich-content.test.ts`](../../apps/tui/tests/unit/tui/render-rich-content.test.ts)
  has a `render gracefully downgrades rich text across display levels` test for
  downgrade behavior.
- [`apps/tui/tests/unit/tui/render-rich-content.test.ts`](../../apps/tui/tests/unit/tui/render-rich-content.test.ts)
  has a `render recognizes common code tag attributes and markdown fence info
  strings` test for HTML code tags and Markdown fence metadata.
- [`apps/tui/tests/unit/tui/render-rich-content.test.ts`](../../apps/tui/tests/unit/tui/render-rich-content.test.ts)
  has a `render supports markdown tables while keeping markdown and local paths
  non-clickable` test for Markdown table support and local-path safety.

When changing the rich text protocol, update the persona prompt, the GUI parser,
the TUI renderer, and the tests together. Changing only the prompt creates a
renderer mismatch; changing only the renderer creates markup the model does not
know how to use. Both paths are boring failures, which is their worst quality.

## Design rule of thumb

Use HTML rich text for normal app conversations because the UI can render a
controlled semantic subset. Use plain text for CLI because terminal output must
survive pipes, logs, low-capability terminals, and humans reading quickly. Keep
Markdown as a compatibility layer, not the main conversation contract.
