# Tura CLI/TUI

`apps/tui` is Tura's TypeScript terminal client. It contains both the
non-interactive CLI and the interactive terminal UI, and both talk to the Rust
gateway over HTTP and SSE. The package does not embed runtime, provider, tool,
or session-storage logic. A terminal is a front end, however persuasive it may
look in monospace.

The TUI never owns gateway. It probes the requested, active, and default gateway
URLs, then fails if none is already healthy.

## Scope

The terminal client stays intentionally small:

- Conversation: send prompts, display user/assistant messages, show compact
  tool summaries, and abort the active turn.
- Sessions: create, resume, list, and inspect session messages.
- Model, agent, and provider settings: list providers/models, select the
  current session model or agent, and read/update workspace session config
  through gateway APIs.
- OAuth/provider auth: list auth methods, show auth status, start OAuth, and
  log out.
- Basic status: gateway health, current working directory, and current session
  state.

The terminal client does not expose GUI-only product areas such as task boards,
file browsing, project pages, persona management, service dashboards,
skills/plugins management, or arbitrary gateway slash-command browsing.

## Package Layout

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
      tui_web_terminal_profiles_playwright.mjs
      business/
        tui_mock_gateway_stream_flow.mjs
      live/
        tui_real_gateway_session_flow.mjs
        tui_web_terminal_snake_game_flow.mjs
      tui_gateway_cli_e2e.mjs
      tui_real_gateway_snake_playwright.mjs
      tui_zip_password_playwright.mjs
    live/
      tui_greeting_stream_visibility_live.mjs
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
      resume.ts
      run.ts
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
      permission.ts
      provider.ts
      session.ts
    locales/
      en.json
      zh-CN.json
```

Localized runtime strings may exist under `src/locales/`. Repository README and
architecture documentation should stay in English.

## Gateway API Allowlist

The terminal client should use existing gateway endpoints only.

| Feature              | Method  | Endpoint                                                | Purpose                                      |
| -------------------- | ------- | ------------------------------------------------------- | -------------------------------------------- |
| health               | `GET`   | `/global/health`                                        | Check gateway availability                   |
| current project sync | `GET`   | `/project/current?directory=...`                        | Sync workspace-scoped state                  |
| session config       | `GET`   | `/session/config?directory=...`                         | Read model/agent/provider settings           |
| session config patch | `PATCH` | `/session/config?directory=...`                         | Update runtime session settings              |
| list sessions        | `GET`   | `/session?directory=...&includeChildren=true&limit=...` | Select and resume sessions                   |
| create session       | `POST`  | `/session`                                              | Start a conversation session                 |
| update session       | `PATCH` | `/session/{sessionID}`                                  | Set current session model/agent              |
| list messages        | `GET`   | `/session/{sessionID}/message`                          | Hydrate transcript history                   |
| prompt async         | `POST`  | `/session/{sessionID}/prompt_async`                     | Send a user message                          |
| abort                | `POST`  | `/session/{sessionID}/abort`                            | Stop the current turn                        |
| event stream         | `GET`   | `/event`                                                | Subscribe to message/session/provider events |
| providers            | `GET`   | `/provider`                                             | Read provider/model catalog                  |
| auth methods         | `GET`   | `/provider/auth`                                        | List provider auth methods                   |
| auth status          | `GET`   | `/provider/{providerID}/auth/status`                    | Read provider login state                    |
| OAuth authorize      | `POST`  | `/provider/{providerID}/oauth/authorize`                | Start OAuth                                  |
| provider logout      | `POST`  | `/provider/{providerID}/auth/logout`                    | Log out from a provider                      |
| agents               | `GET`   | `/agent`                                                | List available runtime agents                |
| agent detail         | `GET`   | `/agent/{agentID}`                                      | Inspect an existing agent                    |

The CLI/TUI should not read `.tura/sessions`, `db/session_log`, `.env`,
`provider_config.json`, provider logs, or backend config files directly.

## Mock Stream E2E

Run `npm run test:stream` from `apps/tui` to exercise the web-terminal UI
against an app-local mock gateway. This script is app-owned and is intentionally
not part of the root backend business runner.

All app-local TUI tests live under `apps/tui/tests`. Unit tests compile to
`apps/tui/test-results/unit-dist`, and browser/business/live scripts should
write screenshots, summaries, and other artifacts under
`apps/tui/test-results/<suite>/<run-id>/`. TUI release-entry scripts invoked
through this package use `apps/tui/test-results/release/<profile>/tui/...` for
their run directories; they still read binaries from `target/<profile>`.

## Web Terminal Profile / User-Agent E2E

Run `npm run test:e2e:profiles` from `apps/tui` to exercise the browser wrapper
around the built TUI in mock mode. It opens `/plain`, `/ansi`, `/rich`, and a
mobile Chromium user-agent profile, verifies that the terminal renders visible
content, checks that raw ANSI controls are not leaked into xterm text, checks for
horizontal overflow, and stores screenshots under
`apps/tui/test-results/tui-web-terminal-profiles/<run-id>/`.

The web terminal supports dragging local files onto the terminal window. When
the browser exposes a local file URI or native path, the composer receives that
path as a rich local link. When a normal browser only exposes the dropped file
contents, the wrapper saves a copy under `.tura/attachments/` in the active
workspace and pastes a `file://` link, or a `[MEDIA:...:MEDIA]` token for media
files. The gateway CLI E2E exercises this through real Playwright drag/drop
events before submitting the pasted composer text.

Run `npm run test:e2e:drop` from `apps/tui` for the focused drag/drop coverage.
It drives browser `DragEvent`/`DataTransfer` input, verifies the composer text,
checks uploaded fallback copies under `.tura/attachments/`, and captures a
screenshot under `apps/tui/test-results/tui-web-terminal-drop/<run-id>/`.

## Real Gateway Snake Playwright E2E

Run `npm run test:e2e:real-snake` from `apps/tui` to exercise the TUI against a
real gateway and provider call. The test creates a disposable Snake fixture,
runs `node tools/snake_playwright.mjs` for desktop/mobile screenshots, sends a
networked TUI task through gateway, then captures the web terminal across chat,
sessions, models, settings, and mobile views. Artifacts are written under
`apps/tui/test-results/tui-real-gateway-snake/<run-id>/`.

## Release Entry Live Acceptance Tests

Run these after the repository release build and CLI registration. They drive
the release `tura` entry and validate a single real request, Snake, and
password-zip CLI refactor task through the TUI command surface. The release
scripts themselves live under root `tests/release/tui_release_*.mjs`.
Their summaries, logs, screenshots, and workspaces are written under
`apps/tui/test-results/release/<profile>/tui/<case>/<run-id>/`.

```text
npm run test:live:release
npm run test:live:release:single
npm run test:live:release:snake
npm run test:live:release:password-zip
```

## Capability Levels

The renderer supports three terminal capability levels.

| Level | Name           | Environment                                                                                                             | Goal                                                       |
| ----- | -------------- | ----------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| L1    | Plain / Safe   | `TERM=dumb`, CI, non-TTY, poor SSH, or explicit `--plain`                                                               | Text-only output that is safe for logs                     |
| L2    | ANSI / Default | Normal macOS/Linux/Windows terminals, SSH, tmux/screen/zellij                                                           | Compact default UI with ANSI color and basic redraw        |
| L3    | Rich / Modern  | iTerm2, WezTerm, Kitty, Ghostty, VS Code terminal, JetBrains terminal, Windows Terminal, xterm.js, or explicit `--rich` | Richer layout, links, and markdown treatment when detected |

### L1 Plain / Safe

- No color, bold, italic, underline, reverse video, cursor control, or screen
  clearing.
- No Unicode dependency; symbols need ASCII fallbacks.
- No OSC 8 terminal hyperlinks, emoji, HTML rendering, or inline media.
- URLs, file paths, and `file:line:col` references are printed as text.
- Output is append-only.
- Non-TTY runs should not enter the interactive TUI.

### L2 ANSI / Default

- ANSI SGR colors and basic cursor positioning are allowed.
- Full-screen redraw may be used.
- URLs and file references remain plain text.
- Markdown is simplified into readable terminal text.
- Inline media is represented by paths or external URLs.
- Raw-mode input, Enter send, Ctrl+J newline, Backspace, Tab completion,
  Escape close, and arrow-key selection are supported.

### L3 Rich / Modern

- 256-color or truecolor output may be used.
- Unicode borders and OSC 8 links are optional and must degrade cleanly.
- Richer markdown, collapsible tool summaries, and external media open actions
  may be enabled.
- xterm.js wrappers may use browser capabilities for richer local interaction.

## Mode Selection

Mode selection should follow this order:

1. Use L1 when the user passes `--plain`.
2. Use L1 for non-TTY, CI, `TERM=dumb`, or `TERM=unknown`.
3. Try L3 when the user passes `--rich`.
4. Try L3 in auto mode when modern terminal signals are detected, such as
   `TERM_PROGRAM`, `WEZTERM_EXECUTABLE`, `KITTY_WINDOW_ID`,
   `GHOSTTY_RESOURCES_DIR`, `WT_SESSION`, or `xterm-256color`.
5. Use L2 for other interactive TTYs.

`src/tui/capabilities.ts` owns runtime detection. Capability switches should
remain independent from level names, including color, cursor control, Unicode,
OSC 8, markdown, media-open support, and raw-mode interactivity.

## Web Terminal Debug Profiles

`npm run web` starts `scripts/web-terminal.mjs`.

- `/plain` starts the L1 page with `--plain` and `TERM=dumb`.
- `/ansi` starts the L2 page with `TERM=vt100`.
- `/rich` starts the L3 page with `--rich` and `xterm-256color`.

Each page owns an independent pty and SSE client set.
The pty shell can be overridden with `TURA_WEB_TERMINAL_SHELL`; otherwise the
script uses the user's shell, macOS `/bin/zsh`, then bash/sh fallbacks.
The browser wrapper treats TUI absolute repaint sequences (`ESC[?25l` or
`ESC[1;1H ESC[2K`) as frame boundaries so bursty streaming refreshes are
coalesced instead of appended into xterm scrollback.

## Development Commands

```text
npm run build
npm test
npm run test:e2e
npm run test:e2e:profiles
npm run test:live
npm run test:live:release
npm run web
```

Build and run the Rust gateway separately when using this package against a
local backend:

```text
npm run build
node apps/tui/dist/index.js --help
```

Start `tura_gateway` before launching the TUI. The TUI only attaches to an
existing gateway and fails when none is reachable.

Repository start-script flow:

```powershell
.\scripts\start.ps1 -Tui --help
```

```sh
./scripts/start.sh --tui --help
```

## Gateway URL Configuration

The TypeScript CLI/TUI resolves the gateway URL in this order:

1. `--gateway-url <url>`.
2. `TURA_GATEWAY_URL`.
3. `http://127.0.0.1:4126`.

Workspace-scoped commands should send the current working directory through the
gateway client so backend config, sessions, files, and events remain scoped to
the selected workspace.

## Testing Focus

TUI tests should cover only terminal-owned behavior:

- CLI parsing and output modes.
- Gateway client request/response handling.
- SSE event parsing and final-result extraction.
- Terminal capability detection across CI/non-TTY, `TERM`, `TERM_PROGRAM`, and
  modern terminal user-agent signals such as WezTerm, Kitty, Ghostty, Windows
  Terminal, VS Code, and xterm-256color.
- Keyboard input normalization, including printable characters, control
  sequences, and non-string key payloads.
- Terminal width, truncation, wrapping, CJK/emoji/combining mark display width,
  ANSI preservation, and large streamed-output performance smoke checks.
- L1/L2/L3 rendering degradation.
- Session list/show/resume flows.
- Provider/model/auth tables.
- Agent list/show as read-only runtime selection.
- Web-terminal Playwright smoke checks for desktop, mobile user agents, profile
  pages, raw-control leaks, and horizontal overflow.
- Gateway timeout, HTTP error, network failure, and concurrent request handling.

GUI-only modules such as task boards, file browsers, persona editors, product
workspaces, plugin managers, and service dashboards should not appear in TUI
tests except as negative exposure checks.

Recommended local confidence ladder:

```text
npm test
npm run test:e2e:profiles
npm run test:stream
npm run test:business
npm run test:performance:live-growth
```

`test:performance:live-growth` grows one active session to 500 independent live
messages, measures redraw and terminal-write pressure, then hydrates the same
messages as stable history to compare the recovered performance after reentry.
The JSON report is written under `test-results/performance/live-growth/`.

Live/release tests still require a real gateway/provider setup and should be run
only when validating the installed release surface.
