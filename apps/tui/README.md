# Tura CLI/TUI

`apps/tui` is the TypeScript terminal client for Tura. It contains both the
non-interactive CLI and the interactive terminal UI. The package talks to the
Rust gateway over HTTP and SSE; it does not embed runtime, provider, tool, or
session-storage logic.

When the requested gateway port is already occupied by another process, TUI
autostart now chooses a free loopback port before spawning gateway so the
terminal client waits on the actual gateway URL it owns instead of timing out on
the occupied port.

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
  e2e/
    business/
      run_all_release.mjs
      tui_mock_gateway_stream_flow.mjs
      tui_single_request_release.mjs
      tui_snake_release.mjs
      tui_password_zip_release.mjs
    live/
      tui_real_gateway_session_flow.mjs
      tui_web_terminal_snake_game_flow.mjs
    tui_gateway_cli_e2e.mjs
    tui_real_gateway_snake_playwright.mjs
    tui_zip_password_playwright.mjs
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

## Real Gateway Snake Playwright E2E

Run `npm run test:e2e:real-snake` from `apps/tui` to exercise the TUI against a
real gateway and provider call. The test creates a disposable Snake fixture,
runs `node tools/snake_playwright.mjs` for desktop/mobile screenshots, sends a
networked TUI task through gateway, then captures the web terminal across chat,
sessions, models, settings, and mobile views. Artifacts are written under
`target/tui-real-gateway-snake/<run-id>/`.

## Release Entry Live Acceptance Tests

Run these after the repository release build and CLI registration. They drive
the release `tura` entry and validate a single real request, Snake, and
password-zip CLI refactor task through the TUI command surface.

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

## Development Commands

```text
npm run build
npm test
npm run test:e2e
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

The TUI auto-starts (and attaches to) its own `tura_gateway` on port 4126, so no
separate gateway command is needed.

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
- L1/L2/L3 rendering degradation.
- Session list/show/resume flows.
- Provider/model/auth tables.
- Agent list/show as read-only runtime selection.
- Web-terminal Playwright smoke checks.

GUI-only modules such as task boards, file browsers, persona editors, product
workspaces, plugin managers, and service dashboards should not appear in TUI
tests except as negative exposure checks.
