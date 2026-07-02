# apps/tui Architecture

## Goal

`apps/tui` provides the terminal-facing Tura client. Both the non-interactive
CLI and the interactive TUI are TypeScript/Node applications in the same package.
They should follow the repository's own gateway/session/runtime boundaries and
reuse terminal interaction patterns only when they fit the current code.

The TypeScript CLI/TUI must not embed the agent runtime, provider calls, command
execution, config persistence, or session store. Those responsibilities already
exist in `crates/gateway`, `crates/runtime`, `crates/router`,
`crates/provider`, and `crates/tools`.

The first implementation target is a conservative, non-interactive CLI path.
The interactive TUI should share the same TypeScript gateway client, event
stream, output formatting, and API types after the command path is stable.

## Non-Goals

- Do not call `crates/runtime` directly from the CLI/TUI app.
- Do not call `crates/provider` directly from the CLI/TUI app.
- Do not run shell commands, tools, service workers, or managed services from the
  CLI/TUI app.
- Do not read or write `.tura/config.conf`, `.env`, or provider config files
  directly.
- Do not create a separate local session/message database for the CLI/TUI app.
- Do not make `/tui/*` shortcut routes the main implementation path.

## Terminal Interaction Goals

Tura should keep a compact transcript, clear status, resume flow,
model/session selectors, permission prompts, useful exit messages, and
deterministic automation output. All durable state and backend work still goes
through gateway HTTP and SSE calls.

## Session Log And Provider Diagnostics

The CLI/TUI queries past sessions through gateway APIs or the gateway
CLI bridge. It must not read `.tura/sessions`, `db/session_log`, provider logs,
or backend config files directly.

HTTP session-log routes:

```text
GET /session-log/workspaces
GET /session-log/sessions?workspace=<workspace>&page=0&page_size=50
GET /session-log/{sessionID}/records?page=0&page_size=100
```

Raw CLI bridge for scripts:

```powershell
'{"command":"get_session","session_id":"session-id"}' | target\debug\tura_gateway.exe session-log
'{"command":"list_session_records","session_id":"session-id","page":0,"page_size":100}' | target\debug\tura_gateway.exe session-log
```

Provider call logs are backend diagnostics under
`log/provider/YYYY-MM-DD/*.json` or `LOG_PATH`; CLI/TUI commands should surface
provider status/usage from gateway/provider APIs unless a developer explicitly
asks for local diagnostic files.

## Package And Directory Strategy

Keep CLI and TUI in one TypeScript package under `apps/tui` for the first
implementation. The non-interactive CLI and the interactive TUI share most of
the important code:

- `gateway/client.ts` for HTTP calls.
- `gateway/events.ts` for `/event` SSE parsing and normalization.
- `types/` for gateway DTOs and terminal-facing event types.
- `output/` for human, JSON, and NDJSON formatting.
- `commands/` for non-interactive CLI subcommands.
- `tui/` for interactive terminal UI state, reducers, rendering, and widgets.

Do not split `apps/cli` and `apps/tui` into separate implementations unless the
shared TypeScript gateway client first becomes a dedicated internal package.
Otherwise DTOs, event handling, completion detection, and permission behavior
will drift.

`apps/tui` should be a Node/TypeScript package, with dependencies limited to
client/UI concerns:

- `typescript` plus the repo's chosen runner/build tool, such as `tsx`,
  `tsup`, or the existing workspace default.
- `commander`, `clipanion`, or `yargs` for command parsing. Prefer the package
  already used elsewhere in Tura if one exists when implementation starts.
- Node `fetch` or `undici` for gateway HTTP.
- `eventsource-parser` or a small local parser for `/event` SSE.
- `zod` only where runtime validation materially improves gateway error
  reporting; otherwise keep DTOs as TypeScript interfaces.
- `ink` or another React-style terminal UI library for the interactive TUI.
- `chalk`/`kleur` only for human terminal color output.

Keep DTOs local to `apps/tui/src/types` at first. The gateway JSON shape is the
client contract; generated SDK types should only be introduced after the gateway
has stable OpenAPI for these routes.

## Current Layout

```text
apps/tui/
  ARCHITECTURE.md
  README.md
  package.json
  tsconfig.json
  scripts/
    web-terminal.mjs
  tests/
    unit/
    e2e/
      business/
      live/
      tui_gateway_cli_e2e.mjs
      tui_real_gateway_snake_playwright.mjs
      tui_zip_password_playwright.mjs
    live/
  test-results/
    unit-dist/
    <suite>/<run-id>/
  src/
    index.ts
    cli.ts
    gateway/
      client.ts
      directory.ts
      errors.ts
      events.ts
    commands/
      agent.ts
      command-registry.ts
      completion.ts
      config-values.ts
      config.ts
      file.ts
      gateway.ts
      inspect.ts
      persona.ts
      project.ts
      provider.ts
      run.ts
      resume.ts
      session.ts
    output/
      final-result.ts
      help.ts
      human.ts
      json.ts
      ndjson.ts
    tui/
      app.ts
      capabilities.ts
      reducer.ts
      render.ts
    types/
      agent.ts
      common.ts
      config.ts
      event.ts
      gateway.ts
      provider.ts
      permission.ts
      session.ts
    locales/
      en.json
      zh-CN.json
```

Localized user-facing runtime strings may live under `src/locales/`. Repository
documentation remains English.

## Gateway-Only Communication

All frontend/backend communication must go through `crates/gateway`.

Default gateway URL resolution:

1. `--gateway-url`
2. `TURA_GATEWAY_URL`
3. `.tura/gateway-active.env`
4. Build default: `http://127.0.0.1:4125` in dev, `http://127.0.0.1:4126` in release.

Every request that is workspace-scoped must send the current directory through
both compatible mechanisms until the API is fully documented:

```text
query:  ?directory=<workspace path>
header: x-opencode-directory: <percent-encoded workspace path>
```

The CLI/TUI app may only health-check and connect to gateway. Gateway is owned
by the gateway process tree, not by TUI launchers or direct crate calls.

Core endpoint map:

```text
health                         GET    /global/health
global events                  GET    /event
global config                  GET    /config
global config patch            PATCH  /config
workspace session config       GET    /session/config?directory=...
workspace session config patch PATCH  /session/config?directory=...
list sessions                  GET    /session?directory=...
create session                 POST   /session
session status                 GET    /session/status
get session                    GET    /session/{sessionID}
update session                 PATCH  /session/{sessionID}
delete session                 DELETE /session/{sessionID}
list messages                  GET    /session/{sessionID}/message
send prompt sync               POST   /session/{sessionID}/message
send prompt async              POST   /session/{sessionID}/prompt_async
abort turn                     POST   /session/{sessionID}/abort
todos                          GET    /session/{sessionID}/todo
permissions                    GET    /permission
permission reply               POST   /permission/{requestID}/reply
questions                      GET    /question
question reply                 POST   /question/{requestID}/reply
question reject                POST   /question/{requestID}/reject
agents                         GET    /agent
providers/models               GET    /provider
provider auth status           GET    /provider/{providerID}/auth/status
provider auth validate         POST   /provider/{providerID}/auth/validate
provider logout                POST   /provider/{providerID}/auth/logout
provider oauth authorize       POST   /provider/{providerID}/oauth/authorize
commands                       GET    /command
execute slash command          POST   /command
vcs info/diff                  GET    /vcs, GET /vcs/diff
services status                GET    /service/status
skills/plugins                 GET    /skill, GET /plugin
paths                          GET    /path
```

The `/tui/*` routes in gateway are shortcuts. The TypeScript CLI/TUI should
prefer the richer session/message/config endpoints, and only use `/tui/*` for
very small smoke tests.

`scripts/web-terminal.mjs` is a browser debugging wrapper around the built TUI.
Its pty shell can be forced with `TURA_WEB_TERMINAL_SHELL`; otherwise it uses
the user's shell, macOS `/bin/zsh`, then bash/sh fallbacks.

## Session Plan Commands

The CLI/TUI plan surface is a terminal projection of gateway session
task-management state. It must use the same routes as GUI and must not read
session files directly.
Terminal API types must stay benchmark-agnostic. Benchmark-specific prompts,
artifacts, and evaluator checks belong in e2e scripts, not TUI session models.

Current plan commands are backed by:

```text
GET    /session?directory=<workspace>&includeChildren=true
POST   /session
GET    /session/{sessionID}
PATCH  /session/{sessionID}
PATCH  /session/{sessionID}/task-management
GET    /session/status
GET    /session/{sessionID}/todo
```

Terminal plan display should prefer `session_display_name`, then
`plan_summary`, then task `task_summary`, then session `name`. A short session
id can be shown as metadata.

Task status values are:

```text
todo
doing
question
done
archived
```

Archived tickets are hidden from normal plan output unless the user passes an
explicit archived flag. The TUI may show archived counts in compact status
rendering.

Single-task patches use object `task_management`. Multi-task nonce-specific
patches should use `task_management.tasks[]` entries with `nonce_id`, matching
gateway's current patch semantics.

Schedule fields are UTC in gateway responses and requests. Terminal output may
format them in local time for humans, while JSON/NDJSON output should preserve
the gateway value.

## Non-Interactive CLI

The first command surface should be small and automation-friendly:

```text
tura run [PROMPT...] [--cwd PATH] [--session ID]
         [--model PROVIDER/MODEL] [--agent NAME]
         [--output text|json|ndjson] [--json]
         [--no-stream] [--timeout SEC]
         [--last-message-file PATH]

tura resume [SESSION_ID|--last] [PROMPT...]
            [--output text|json|ndjson] [--json]

tura session list [--cwd PATH] [--all] [--json]
tura session show SESSION_ID [--json]
tura session delete SESSION_ID

tura config get [--cwd PATH] [--json]
tura config set KEY=VALUE... [--cwd PATH]

tura provider list [--json]
tura provider status [PROVIDER] [--json]

tura permission list [--json]
tura permission reply REQUEST_ID --approve|--deny

tura command list [--json]
tura command run NAME [ARGS...]

tura status [--json]
tura completion SHELL
```

Root flags:

```text
--gateway-url URL
--cwd PATH
--json
--color auto|always|never
--verbose
```

`--json` is an alias for `--output json` where the command has an output mode.
Per-run `--model` and `--agent` flags are request-scoped and must not persist
settings. Persistent changes belong to `tura config set`.

## Run Flow

Use `/prompt_async` as the default execution endpoint because it returns
immediately and lets the CLI stream `/event`. Use `/message` only for simple
synchronous calls.

```text
parse args
  -> resolve cwd and gateway url
  -> GET /global/health
  -> GET /session/config?directory=...
  -> optionally validate --model through /provider/model/validate
  -> POST /session when --session is absent
  -> POST /session/{sessionID}/prompt_async
  -> subscribe GET /event unless --no-stream
  -> filter envelopes by directory and sessionID
  -> render events through selected output mode
  -> hydrate final messages through GET /session/{sessionID}/message
  -> exit with deterministic code
```

`/event` currently emits SSE data as:

```json
{
  "directory": "./workspace",
  "payload": {
    "type": "message.updated",
    "properties": {}
  }
}
```

The TypeScript client should parse this envelope first, then narrow on
`payload.type`.

## Prompt Payload

The CLI should send a frontend-compatible prompt payload to `/prompt_async`:

```json
{
  "messageID": "msg_cli_<uuid>",
  "parts": [
    {
      "id": "part_cli_<uuid>",
      "type": "text",
      "text": "user prompt"
    }
  ],
  "model": "openai/gpt-5.5",
  "agent": "thoughtful",
  "source": "cli"
}
```

Gateway already extracts:

- text from `parts[].text`
- frontend message/part ids from `messageID` and `parts[].id`
- runtime model override from `model`
- runtime options from fields like `variant` and
  `model_acceleration_enabled`

The same prompt shape should be reused by the later interactive TUI so terminal
clients behave consistently. Both CLI `run` and the interactive TUI default
`model_acceleration_enabled` to `false`; explicit saved config or run flags may
still set it to `false`.

## Completion Detection

Until gateway exposes an explicit `turn.completed` event, use conservative
completion rules:

1. Record the submitted frontend message id and initial message count.
2. Treat `session.status` with `idle` after submission as a completion signal.
3. Also accept a new assistant `message.updated` after the submitted user
   message when no busy status remains.
4. On timeout, call `POST /session/{sessionID}/abort` and exit `4`.
5. If gateway returns or emits error status, exit `1`.

After completion, fetch `GET /session/{sessionID}/message` and extract the last
assistant text part as the final result.

## Output And Exit Codes

`text` output:

- human-readable status lines on stderr
- final assistant text on stdout
- compact tool summaries, permission notices, and final resume command

`json` output:

```json
{
  "sessionID": "...",
  "status": "completed",
  "finalText": "...",
  "messages": [],
  "usage": null
}
```

`ndjson` output:

- one JSON object per event
- include raw gateway envelope and normalized event fields
- finish with a final `{"type":"cli.completed",...}` or
  `{"type":"cli.failed",...}` record

Exit codes:

```text
0 completed
1 gateway/runtime/provider error
2 invalid CLI usage
3 permission denied or cancelled
4 timeout
5 gateway unavailable
```

## Interactive TUI

The interactive TUI should reuse the same TypeScript `GatewayClient`, event
stream parser, gateway types, command parser, output formatting primitives,
completion detection, and permission semantics. Its job is local terminal state
and rendering only.

Recommended state model:

```text
GatewayClient
  -> EventIngestor maps /event SSE payloads into typed events
  -> AppState stores selected workspace, session, messages, todos, permissions
  -> reducer updates state
  -> Ink/terminal renderer draws transcript, composer, status bar, overlays
```

UI surfaces to port from the Codex TUI idea:

- transcript cells for user messages, assistant messages, tool calls, diffs,
  command output, and errors
- composer with multiline input and slash-command completion
- status header with model, agent, cwd, branch, service status, and token usage
- session picker for resume/fork/open
- model/provider picker
- permission approval pane
- todo pane
- diff viewer
- help/keymap overlay

The TUI should not invent hidden state. Every durable choice should be reflected
through gateway config APIs.

## Long-Conversation Rendering (Bounded Transcript Scroll)

Requirement: the transcript must stay responsive and fully navigable for
sessions with thousands of messages. A long history must never make the TUI
unresponsive or force the user to switch/abandon the session to recover.

The Codex-inspired repair plan for streaming smoothness, scroll stability, and
delta-only live rendering is documented in
[`docs/tui-streaming-render-plan.md`](../../docs/tui-streaming-render-plan.md).

Existing scroll system — preserve exactly, do not regress:

- `tui/render.ts:transcriptLines()` builds lines only from a tail of the message
  array, then `viewportLines()` / `smartViewportLines()` window the rows by
  `state.scrollOffset`.
- Scroll input: the reducer `scroll` action (`tui/reducer.ts`) clamps
  `scrollOffset >= 0`; Up/Down = ±1 line, PageUp/PageDown = ±10.
- `draw()` (`tui/app.ts`) repaints absolute per-line
  (`\x1b[{row};1H\x1b[2K{line}`) and emits **no newlines**, so the screen can
  never push lines into terminal scrollback.

These three properties — tail-bounded render, `scrollOffset` row windowing, and
newline-free absolute repaint — are the scroll contract. Do not switch to
newline-emitting frames, alternate-screen scroll regions, or unbounded line
building.

Gap to close: the render tail is a fixed `state.messages.slice(-100)`. It bounds
cost but makes older history unreachable — the user cannot scroll above the last
100 messages and `scrollOffset` saturates at the top of that window even though
the history exists in `state.messages` and in the gateway. For long sessions
that is the failure this requirement targets.

Required design:

- Replace the fixed `slice(-100)` tail with a **sliding message window** anchored
  on scroll position. The window keeps a fixed render budget (enough messages to
  fill a few screens), but its start index moves earlier as `scrollOffset`
  reaches the top of the rendered set and later as the user scrolls back toward
  the bottom.
- Per-frame line building stays bounded: never wrap/lay out more than the
  windowed message set, regardless of `state.messages.length`. Frame cost must be
  O(window), not O(history).
- When the window reaches messages not yet held locally, fetch older messages
  from gateway (`GET /session/{sessionID}/message`, paged) and splice them into
  `state.messages` **without moving the visible anchor** (no scroll jump). Until
  paged message retrieval exists, cap the in-memory window and document the cap,
  but never block the render loop on history size.
- Keep "stick to bottom on new activity" exactly: `scrollOffset === 0` keeps
  meaning "follow the latest", and streaming `message.updated` while at the
  bottom must keep the newest content visible with no jump.

Acceptance criteria:

- A session with 5,000+ messages keeps keystroke-to-repaint latency flat and
  equal to a short session.
- The user can scroll continuously from the latest message back to the first
  with no hard stop at 100 and no freeze.
- No regression in `npm run test:e2e` / `npm run test:stream` or the
  absolute-repaint no-scrollback guarantee.

## Config And Settings

Use the current Tura architecture:

- Workspace/session settings live behind
  `GET/PATCH /session/config?directory=...`, backed by
  `.tura/config.conf` through `crates/gateway/src/session/config.rs`.
- Global UI settings live behind `GET/PATCH /config`.
- Provider/model catalog and auth state live behind `/provider` and
  `/provider/{providerID}/auth/*`.
- Provider credentials and `provider_config.json` updates are owned by
  gateway/provider code.
- Commands are discovered by gateway from `.tura/commands`, `.opencode/*`,
  `command`, and `commands`.
- Skills/plugins are discovered by gateway through `/skill` and `/plugin`.

The TypeScript CLI/TUI may cache config for rendering, but it must treat gateway
as source of truth and invalidate local state after config patches.

## Session And Event Semantics

The TUI should treat `sessionID` as the stable root of a conversation. It should
hydrate on startup by calling:

```text
GET /session/{sessionID}
GET /session/{sessionID}/message
GET /session/{sessionID}/todo
GET /permission
GET /service/status
```

Then it should attach to `/event` and filter by session directory and
`sessionID`. Events should be idempotent because `/event` may replay or scan new
messages from the store.

Non-interactive `run` and interactive TUI turns should share completion
detection until gateway adds explicit final-turn events.

## Permissions

Permission handling remains gateway-owned.

Interactive TUI:

- poll/subscribe for permission events
- render a permission pane
- reply through `POST /permission/{requestID}/reply`

Non-interactive CLI:

- default to fail fast when permission is required
- show pending permission details in text mode
- emit the raw permission request in JSON/NDJSON modes
- exit `3` unless a future gateway-backed policy explicitly supports
  non-interactive auto approval
- never bypass gateway permission APIs

## Testing Strategy

The test suite should be layered. Keep fast invariant tests in
`apps/tui/tests/unit/**/*.test.ts` and use Playwright only where a
browser/xterm/user-agent boundary is required. Do not use live provider calls
for ordinary regressions; mock gateway scripts own the terminal surface, and
root live tests own release acceptance.

Fast unit and edge tests:

- unit tests for CLI parsing and output formatting
- gateway client tests against a mock HTTP server
- SSE envelope parser tests
- NDJSON golden tests for non-interactive `run`
- timeout and gateway-unavailable tests
- terminal capability tests for CI/non-TTY, dumb/unknown terminals, ANSI
  terminals, and rich user-agent signals (`TERM_PROGRAM`, WezTerm, Kitty,
  Ghostty, Windows Terminal, VS Code, xterm-256color)
- keyboard input tests for printable Unicode, control characters, escape
  sequences, and malformed key payloads
- terminal rendering edge tests for ANSI preservation, truncation, CJK width,
  emoji width, combining marks, narrow columns, and plain/rich fallbacks
- reducer tests for idempotent event replay, cross-workspace filtering,
  out-of-order streaming deltas, session picker stability, setting selection,
  error/notice state, and stale-session transcript clearing
- lightweight performance smoke tests for large wrapped/streamed terminal output
  so rendering regressions are caught before they become “why is my terminal a
  toaster” incidents

Interactive and browser tests:

- reducer tests for gateway event ingestion
- terminal UI snapshot tests for transcript/status/permission panes
- resize tests for compact and wide terminal widths
- Playwright web-terminal profile smoke for `/plain`, `/ansi`, `/rich`
- Playwright mobile user-agent smoke for small viewport wrapping and horizontal
  overflow checks
- Playwright regression tests for transcript history, composer wrapping, colors,
  xterm rendering, and raw ANSI/control leak prevention
- mock-gateway business tests for streaming, multi-session, refresh/replay, and
  local task workflows

Current app-owned commands:

```text
npm test                         # build + all tests/unit suites
npm run test:e2e                 # mock gateway CLI and web-terminal e2e
npm run test:e2e:profiles        # Playwright profile + mobile user-agent smoke
npm run test:stream              # mock gateway stream flow
npm run test:business            # local business suite
npm run test:live:*              # real gateway/provider acceptance, opt-in only
```

Coverage expectations by boundary:

```text
CLI parser/output        unit tests + mock gateway e2e
Gateway HTTP client      mock HTTP unit tests: success, HTTP error, timeout, concurrency
SSE parsing              parser/normalizer unit tests + stream e2e
Reducer/event state      unit tests for replay, filters, panels, sessions, settings
Renderer                 unit tests for width/wrap/truncate + render snapshots
Keyboard/composer        unit tests + Playwright xterm smoke
Terminal capabilities    unit tests for env/user-agent signals + profile e2e
Web terminal wrapper     Playwright profile/mobile/regression tests
Live release surface     root live tests only
```

App-local TUI tests live under `apps/tui/tests`: `unit/` for Node test suites,
`e2e/business/` for local business harnesses, `e2e/live/` for provider-backed
flows, and `live/` for app-owned live checks. App-local test outputs must go
under `apps/tui/test-results/<suite>/<run-id>/`. TUI release-entry scripts live
under root `tests/release/tui_release_*.mjs`, but their logs, summaries,
screenshots, and workspaces default to
`apps/tui/test-results/release/<profile>/tui/<case>/<run-id>/`. TUI benchmarks
should also archive outputs under `apps/tui/test-results/benchmark/...`; only
the compiled debug/release binaries are read from `target/<profile>`.

Release-entry acceptance tests that validate the registered release
command surface belong in root `tests/release/tui_release_*.mjs` for the TUI
surface. Root `tests/release/release_entry_*.mjs` owns CLI release-entry scripts;
`benchmark/` owns comparison and scoring benchmarks.

## Implementation Phases

1. Create the TypeScript package, root CLI parser, gateway client, DTOs,
   health/status, and config commands.
2. Implement `session list/show/delete` and provider status commands.
3. Implement `run --output ndjson` with `/prompt_async`, `/event`, timeout, and
   final message hydration.
4. Add human text output, JSON output, `resume`, and deterministic completion
   detection.
5. Add permission list/reply, slash-command execution, and completion
   generation.
6. Introduce interactive TUI state/reducer/rendering on the same client layer.
7. Add richer transcript rendering, diff rendering, model picker, session
   picker, and help overlay.

## Open Gateway Gaps

The current gateway is already close, but these refinements would make TUI/CLI
cleaner:

- explicit final-turn event or stable turn id in `/prompt_async` and `/event`
- `turn.started`, `turn.completed`, `turn.failed`, and `turn.cancelled` events
- SSE `id:` fields for reliable stream resume
- documented request/response DTOs for `/session/{id}/message` and
  `/prompt_async`
- one prompt payload for CLI/GUI/TUI
- stable error envelope for all endpoints
- explicit non-interactive session marker so `resume --include-non-interactive`
  can be faithfully supported
