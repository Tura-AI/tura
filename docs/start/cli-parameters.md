# CLI Parameters

This is the command-line reference for starting Tura from a shell and calling
the CLI surfaces directly. It assumes Tura is already available as a binary or
through the npm package entry. Installation and interactive TUI/GUI usage live
elsewhere so this page can remain a reference instead of becoming a small novel.

## Command surfaces

| Surface | Use it for | Basic form |
| --- | --- | --- |
| `tura` | Terminal client entry point | `tura [GLOBAL_OPTIONS] <command>` |
| `tura run` | Gateway-backed non-interactive prompt | `tura run [RUN_OPTIONS] "prompt"` |
| `tura exec` | Direct Rust CLI prompt runner | `tura exec [EXEC_OPTIONS] "prompt"` |
| `tura bash`, `tura zsh`, `tura shel` | Gateway-backed prompt with a forced `command_run` shell | `tura zsh "prompt"` |
| `tura_exec` | Direct Rust CLI binary | `tura_exec [EXEC_OPTIONS] "prompt"` |
| `tura_gateway` | Local HTTP/SSE gateway and optional web GUI serving | `tura_gateway` |
| `tura_gui` | Desktop GUI launcher hints | `tura_gui --gateway-url URL --workspace PATH` |
| `tura_router` | Internal router daemon and registry CLI | `tura_router serve-socket` |
| `tura_session_db` | Internal session database owner | `tura_session_db` |

Most users only need `tura run`, `tura exec`, and the management subcommands
under `tura`. The router and session database commands are mainly for service
startup, tests, diagnostics, and release plumbing.

## Global `tura` options

Global options may appear before any `tura` subcommand. They are parsed by the
TypeScript terminal client before dispatching to the selected command.

| Option | Meaning |
| --- | --- |
| `--gateway-url URL` | Use an explicit gateway instead of auto-starting or discovering one. |
| `--cwd PATH` | Workspace directory sent to the gateway. Defaults to the current directory. |
| `--initial-session ID` | Open the terminal UI on a specific session. Also read from `TURA_TUI_INITIAL_SESSION_ID`. |
| `--json` | Request JSON output where the selected command supports it. |
| `--verbose` | Print gateway request diagnostics to stderr. |
| `--mock` | Use mock client behavior. Also enabled by `TURA_TUI_MOCK=1` or `true`. |
| `--dev` | Use development gateway startup behavior. Also enabled by `TURA_DEV=1` or `true`. |
| `--color auto|always|never` | Control ANSI color output. |
| `--plain` | Force plain/safe terminal rendering and disable color. |
| `--rich` | Force rich terminal rendering. |
| `--lang en|zh-CN`, `--language en|zh-CN` | Set CLI display language. |

Examples:

```bash
tura --cwd /work/repo run "Inspect the workspace"
tura --gateway-url http://127.0.0.1:4126 session list --json
tura --plain provider list
```

With no subcommand, `tura` opens the interactive terminal client. If the first
argument is not a known command, the remaining text is treated as the initial TUI
prompt:

```bash
tura
tura "Inspect this repository"
```

## Prompt commands

### `tura run`

`tura run` creates or reuses a gateway session, sends one prompt, streams or
polls until the turn completes, then prints the result.

```bash
tura run [RUN_OPTIONS] "Fix the failing test and verify it"
```

| Option | Meaning |
| --- | --- |
| `--session ID` | Append the prompt to an existing session. |
| `-m MODEL`, `--model MODEL`, `--model=MODEL` | Request-scoped model override. Use `PROVIDER/MODEL` when selecting a provider explicitly. |
| `-a ID`, `--agent ID`, `--agent-id ID`, `--agent-name ID` | Request-scoped agent override. Defaults to `balanced`. |
| `--session-type TYPE` | Session type passed to the gateway session. |
| `--model-variant LEVEL`, `--variant LEVEL`, `--reasoning-effort LEVEL`, `--model-reasoning-effort LEVEL` | Reasoning/model variant override. Defaults to `high`. |
| `-p`, `--priority`, `--model-acceleration`, `--accelerated` | Enable priority model routing for this run. |
| `--no-model-acceleration`, `--no-accelerated` | Disable priority model routing for this run. |
| `--bash`, `--zsh`, `--shel` | Force the `command_run` shell surface for this prompt. |
| `--output text|json|ndjson` | Output mode. Default is `text`; root `--json` changes the default to `json`. |
| `--json` | Alias for `--output json`. |
| `--stream`, `--no-stream` | Stream gateway events or poll for completion. Streaming is the default. |
| `--timeout SEC` | Abort the turn after this many seconds. Default is `600`. |
| `--last-message-file PATH` | Write the final assistant message to a file. |
| `-c KEY=VALUE`, `--config KEY=VALUE`, `--config=KEY=VALUE` | Runtime override. Supported keys are listed below. |

Runtime override keys accepted by `tura run -c`:

| Key | Meaning |
| --- | --- |
| `model` | Model override. |
| `agent`, `active_agent` | Agent override. |
| `session_type` | Session type override. |
| `model_variant`, `variant`, `reasoning_effort`, `model_reasoning_effort` | Reasoning/model variant override. |
| `model_acceleration_enabled`, `acceleration`, `accelerated`, `model_acceleration`, `service_tier` | Priority/acceleration routing toggle. |
| `kill_processes_on_start` | Request process cleanup behavior for the created session. |
| `validator_enabled` | Enable or disable validator behavior for the created session. |
| `command_run_shell` | `bash`, `zsh`, or `shel`. |

Examples:

```bash
tura run "Summarize this repository"
tura run --session SESSION_ID "Continue from the previous result"
tura run -m openai/gpt-5 -a balanced --reasoning-effort high "Fix tests"
tura run --output ndjson --timeout 1200 "Run the release verifier"
tura run -c command_run_shell=zsh "Check zsh-specific startup behavior"
```

### Shell-specific prompt aliases

These are aliases for `tura run` with the `command_run` shell surface forced for
the turn:

```bash
tura bash "Use bash semantics for command_run"
tura zsh "Use zsh semantics for command_run"
tura shel "Use the default shell_command surface"
```

The spelling is `shel` on the TypeScript `tura` client. The direct Rust CLI uses
`shll`; yes, the double spelling exists. Very charming, in the way legacy knobs
are charming.

### `tura exec`

`tura exec` forwards to the Rust CLI front, `tura_exec`. It is the compact
one-shot path for shell scripts and direct terminal prompts. If no prompt is
provided as arguments, it reads the prompt from stdin.

```bash
tura exec [EXEC_OPTIONS] "Inspect the workspace"
echo "Summarize the architecture" | tura exec --json
```

Direct binary forms are equivalent when `tura_exec` is on PATH:

```bash
tura_exec [EXEC_OPTIONS] "Inspect the workspace"
tura_exec exec [EXEC_OPTIONS] "Inspect the workspace"
```

Shell-specific direct forms:

```bash
tura exec bash "Run command tools through bash"
tura exec zsh "Run command tools through zsh"
tura exec shll "Run command tools through shell_command"
```

| Option | Meaning |
| --- | --- |
| `-C PATH`, `--cwd PATH` | Workspace directory for the session. |
| `-m MODEL`, `--model MODEL`, `--model=MODEL` | Model override. Bare names are treated as OpenAI model names by the Rust CLI. |
| `-p`, `--priority` | Enable priority model routing for this model. |
| `-a ID`, `--agent ID`, `--agent-id ID`, `--agent-name ID` | Agent id loaded from `agents/src/`. |
| `--session-id ID` | Reuse a deterministic session id. |
| `--goal` | Keep the CLI session running until `task_status` marks `done` or `question`. |
| `--no-op` | Disable operation manual injection unless goal/reflection behavior overrides it. |
| `--json` | Emit JSONL events on stdout instead of final text only. |
| `-log`, `--log` | Write current-turn token, timing, tool, and text diagnostics to stderr. |
| `--quiet`, `--silent` | Suppress progress on stderr. |
| `--output-last-message PATH` | Write the final assistant message to a file. |
| `--model-reasoning-effort LEVEL`, `--reasoning-effort LEVEL` | Reasoning effort override. |
| `--planning auto|on|off` | Planning override. Default is `auto`, following the selected agent config. |
| `--bash`, `--zsh`, `--shll` | Force the command-run shell surface for this turn. |
| `--sandbox` | Restrict `command_run` writes and working directories to the workspace. |
| `--embedded` | Run the runtime in-process instead of dispatching through the detached router daemon. Mostly diagnostic. |
| `-c KEY=VALUE`, `--config KEY=VALUE` | Runtime override. Supported keys are listed below. |
| `--skip-git-repo-check` | Compatibility flag; accepted and ignored. |
| `--dangerously-bypass-approvals-and-sandbox` | Codex CLI compatibility flag; accepted but does not enable sandboxing. |
| `-h`, `--help`, `help` | Show Rust CLI help. |

Runtime override keys accepted by `tura exec -c`:

| Key | Meaning |
| --- | --- |
| `model_reasoning_effort`, `reasoning_effort`, `model_variant` | Reasoning/model variant override. |
| `model_acceleration_enabled` | Enables priority routing when truthy. |
| `max_tokens`, `model_max_tokens` | Maximum model token override. |
| `service_tier=priority` | Enables priority routing. |
| `planning=auto|on|off` | Planning override. |
| `command_run_shell=bash|zsh|shll` | Command-run shell surface. |

Examples:

```bash
tura exec -C . -m openai/gpt-5 "Inspect the workspace"
tura exec --quiet "Return only the final answer"
tura exec --sandbox --bash "Run the local checks safely"
tura exec --goal "Finish this task and stop when task_status is done"
echo "Summarize the architecture" | tura exec --json
```

## Session commands

Session commands inspect and modify gateway sessions.

| Command | Meaning |
| --- | --- |
| `tura session list [--all] [--json]` | List sessions. `--all` includes all workspaces and child sessions. |
| `tura session show SESSION_ID [--json]` | Show one session and its messages. |
| `tura session update SESSION_ID --data JSON [--json]` | Patch session metadata with a JSON object. |
| `tura session task-management SESSION_ID --data JSON [--json]` | Patch task-management state with a JSON object. |
| `tura session abort SESSION_ID [--json]` | Request cancellation for a busy session. |
| `tura resume SESSION_ID [PROMPT...]` | Show a session, or append a prompt if prompt text is supplied. |
| `tura resume --last [PROMPT...]` | Use the most recently updated session. |

Resume-specific options:

| Option | Meaning |
| --- | --- |
| `--last` | Select the newest session. |
| `--output text|json|ndjson` | Output mode when a follow-up prompt is sent. |
| `--json` | Alias for `--output json`. |

Examples:

```bash
tura session list --all --json
tura session show SESSION_ID
tura resume --last "Continue and verify the fix"
tura session abort SESSION_ID
```

## Config commands

Config commands read and update workspace session config or model-tier defaults.

| Command | Meaning |
| --- | --- |
| `tura config get [KEY]` | Print the full session config as JSON, or one key value. |
| `tura config set KEY=VALUE...` | Patch session config. Values parse as booleans, numbers, JSON, or strings. |
| `tura config model-tiers [--json]` | List configured model tiers. Alias: `tiers`. |
| `tura config model-tier TIER [--json]` | Show options for one model tier. Alias: `tier`. |
| `tura config model-tier TIER PROVIDER/MODEL` | Set the model for a tier and update session model config. |
| `tura config model-tier TIER PROVIDER MODEL` | Same as above, with provider and model split into two arguments. |

Session config assignment keys accepted by `tura config set`:

| Key | Meaning |
| --- | --- |
| `agent`, `active_agent` | Active agent. |
| `persona`, `active_persona` | Active persona. |
| `model`, `active_model`, `active_provider` | Active model/provider settings. `model=PROVIDER/MODEL` also fills provider/model fields. |
| `session_type` | Session type. |
| `model_variant`, `variant`, `reasoning_effort`, `model_reasoning_effort` | Reasoning/model variant. |
| `model_acceleration_enabled`, `acceleration`, `accelerated`, `model_acceleration`, `service_tier` | Priority/acceleration setting. |
| `context_message_limit` | Context history message limit. |
| `command_run_stall_guard_check_secs` | Stall guard interval. |
| `command_run_stall_guard_identical_checks` | Stall guard identical-output threshold. |
| `command_run_stall_guard_profile` | Stall guard profile name. |
| `kill_processes_on_start` | Whether a session should kill owned processes on start. |
| `validator_enabled` | Whether validator behavior is enabled. |
| `show_command_instructions`, `show_commands`, `show_command`, `display_commands` | Command instruction visibility. |
| `language` | Session language setting. |

Examples:

```bash
tura config get
tura config get active_agent
tura config set model=openai/gpt-5 active_agent=balanced validator_enabled=true
tura config model-tier high openai/gpt-5
```

## Provider commands

Provider commands list provider catalog data and manage local authentication.

| Command | Meaning |
| --- | --- |
| `tura provider list [--json]` | List providers, connection state, and default model hints. |
| `tura provider status [PROVIDER]` | Print auth status for one provider, or all providers when omitted. |
| `tura provider login PROVIDER [--method N] [--no-open]` | Start OAuth login. `--no-open` prints the URL without opening a browser. |
| `tura provider oauth PROVIDER [--method N] [--no-open]` | Alias for `login`. |
| `tura provider set-auth PROVIDER --key KEY [--type api]` | Store an API key after validation. |
| `tura provider set-auth PROVIDER --access TOKEN [--refresh TOKEN] [--expires UNIX] [--account-id ID] [--metadata JSON_OR_PATH]` | Store token-style auth after validation. |
| `tura provider set-auth PROVIDER --auth JSON_OR_PATH` | Store auth from a JSON object or JSON file. |
| `tura provider logout PROVIDER` | Remove saved auth for the provider. |

Examples:

```bash
tura provider list --json
tura provider status openai
tura provider login openai --no-open
tura provider set-auth openai --key "$OPENAI_API_KEY"
```

## Agent commands

Agent commands work with the gateway agent registry.

| Command | Meaning |
| --- | --- |
| `tura agent list [--json]` | List agents. |
| `tura agent show AGENT_ID [--json]` | Show one agent. |
| `tura agent create AGENT_ID [--config JSON_OR_PATH] [--prompt TEXT | --prompt-file PATH]` | Create an agent. |
| `tura agent update AGENT_ID [--config JSON_OR_PATH] [--prompt TEXT | --prompt-file PATH]` | Update an agent. |
| `tura agent delete AGENT_ID [--json]` | Delete an agent. |
| `tura agent model AGENT_ID [PROVIDER/MODEL] [--reasoning LEVEL] [--priority | --no-priority]` | Show or set an agent model. Alias: `tier`. |
| `tura agent model AGENT_ID PROVIDER MODEL [--reasoning LEVEL] [--priority | --no-priority]` | Same model setter with provider and model split. |

Examples:

```bash
tura agent list
tura agent show balanced --json
tura agent model balanced openai/gpt-5 --reasoning high --priority
```

## Persona commands

Persona commands work with the gateway persona registry.

| Command | Meaning |
| --- | --- |
| `tura persona list [--json]` | List personas. |
| `tura persona show PERSONA_ID [--json]` | Show one persona. |
| `tura persona create PERSONA_ID [--config JSON_OR_PATH] [--persona TEXT | --persona-file PATH] [--communication-style TEXT | --communication-style-file PATH]` | Create a persona. |
| `tura persona update PERSONA_ID [--config JSON_OR_PATH] [--persona TEXT | --persona-file PATH] [--communication-style TEXT | --communication-style-file PATH]` | Update a persona. |
| `tura persona delete PERSONA_ID [--json]` | Delete a persona. |

Examples:

```bash
tura persona list
tura persona show tura --json
```

## Project commands

Project commands inspect or create gateway workspaces.

| Command | Meaning |
| --- | --- |
| `tura project current [--json]` | Show the current gateway workspace. |
| `tura project list [--json]` | List known gateway workspaces. |
| `tura project create [NAME] [--json]` | Create a workspace record. |
| `tura project default [--json]` | Select or create the default workspace. |
| `tura project select-local [--title TEXT] [--json]` | Ask the local desktop picker to select a workspace. |

Examples:

```bash
tura project current
tura project create "release-checkout"
```

## File commands

File commands operate through the gateway against the selected workspace.

| Command | Meaning |
| --- | --- |
| `tura file list [PATH] [--json]` | List files under a workspace path. |
| `tura file read PATH [--json]` | Read a text file, or print structured JSON for non-text content. |
| `tura file open PATH [--json]` | Ask the OS to open a file. |
| `tura file reveal PATH [--json]` | Ask the OS to reveal a file location. |

Examples:

```bash
tura file list docs/start
tura file read docs/start/how-to-start.md
```

## Command registry commands

These call registered gateway commands, not arbitrary shell commands.

| Command | Meaning |
| --- | --- |
| `tura command list [--json]` | List registered commands. |
| `tura command run COMMAND [ARGS...] [--json]` | Execute one registered command. |

Example:

```bash
tura command list --json
```

## Inspect commands

Inspect commands expose gateway state useful for diagnostics.

| Command | Meaning |
| --- | --- |
| `tura inspect status [--json]` | Show service status for internal services. |
| `tura inspect path [--json]`, `tura inspect paths [--json]` | Show resolved gateway/runtime paths. |
| `tura inspect sessions [--json]` | List sessions including child sessions. |
| `tura inspect messages SESSION_ID [--json]` | List message metadata for one session. |

Examples:

```bash
tura inspect status
tura inspect paths --json
```

## Raw gateway command

`tura gateway` sends one raw HTTP request through the CLI gateway client. The
path is a real gateway HTTP path, not the dotted names shown by some older help
text.

```bash
tura gateway METHOD PATH [-d JSON]
```

| Argument or option | Meaning |
| --- | --- |
| `METHOD` | `GET`, `POST`, `PATCH`, `PUT`, or `DELETE`. Defaults to `GET` if omitted before the path. |
| `PATH` | Gateway path such as `/global/health`, `/session`, or `/command`. A leading slash is optional. |
| `-d JSON`, `--data JSON` | JSON request body. |

Examples:

```bash
tura gateway GET /global/health
tura gateway GET /path
tura gateway POST /command --data '{"command":"name","args":[]}'
```

## Completion command

Generate shell completion snippets for the top-level `tura` command:

```bash
tura completion bash
tura completion zsh
tura completion fish
```

## Gateway and GUI launch parameters

### `tura_gateway`

`tura_gateway` starts the local HTTP/SSE gateway. It takes no normal CLI
options. Choose the port through environment variables:

| Environment variable | Meaning |
| --- | --- |
| `PORT` | Explicit gateway port. If occupied by another process, startup fails. |
| `TURA_GATEWAY_PORT` | Gateway port hint used when `PORT` is not set. |
| `TURA_GUI_DIST` | Directory containing a built web GUI (`index.html` and assets) to serve. |

Default ports are build-kind based: debug uses `4125`, release uses `4126`.
`tura_gateway session-log ...` dispatches to the session-log admin CLI described
below; normal users should not need that form.

### `tura_gui`

`tura_gui` accepts startup hints and then runs the desktop application:

| Option | Meaning |
| --- | --- |
| `--gateway-url URL` | Gateway URL to use or remember. |
| `--workspace PATH`, `--directory PATH`, `--cwd PATH` | Workspace to open. |
| `--session-id ID`, `--initial-session ID` | Session startup hint. |

Example:

```bash
tura_gui --gateway-url http://127.0.0.1:4126 --workspace /work/repo --initial-session SESSION_ID
```

## npm package entry

The npm package entry is also named `tura`. It resolves the platform release
binary, sets release-related environment variables, and forwards all other
arguments to the real binary.

Package-entry-only commands:

| Command | Meaning |
| --- | --- |
| `tura register-cli` | Register the resolved release directory on the user PATH. |
| `tura unregister-cli` | Remove registered Tura CLI PATH entries. |
| `tura doctor-cli-path` | Exit successfully only when the CLI path is registered. |

Forwarded examples:

```bash
tura exec "Inspect this workspace"
tura run "Summarize the current project"
```

## Internal service CLIs

These binaries are part of the runtime plumbing. They are documented here so the
call surface is explicit, not because they are pleasant public UX.

### `tura_router`

When no command is supplied, `tura_router` defaults to `serve`.

| Command | Input | Output |
| --- | --- | --- |
| `tura_router serve` | stdio IPC | Router stdio service. |
| `tura_router serve-socket` | socket IPC | Router socket service. |
| `tura_router run-agent` | `RunAgentRequest` JSON on stdin | Result JSON on stdout. |
| `tura_router registry-agents-list` | none | Agent catalog JSON. |
| `tura_router registry-agent-get AGENT_ID` | none | Agent JSON. |
| `tura_router registry-agent-create` | `UpsertAgentRequest` JSON on stdin | Created agent JSON. |
| `tura_router registry-agent-update AGENT_ID` | `UpsertAgentRequest` JSON on stdin | Updated agent JSON. |
| `tura_router registry-agent-delete AGENT_ID` | none | Delete result JSON. |
| `tura_router registry-personas-list` | none | Persona list JSON. |
| `tura_router registry-persona-get PERSONA_ID` | none | Persona JSON. |
| `tura_router registry-persona-create` | `UpsertPersonaRequest` JSON on stdin | Created persona JSON. |
| `tura_router registry-persona-update PERSONA_ID` | `UpsertPersonaRequest` JSON on stdin | Updated persona JSON. |
| `tura_router registry-persona-delete PERSONA_ID` | none | Delete result JSON. |
| `tura_router registry-commands-list` | optional `{ "directory": "..." }` JSON on stdin | Command list JSON. |
| `tura_router registry-command-execute` | `{ "directory": "...", "command": "...", "args": [...] }` JSON on stdin | Command execution JSON. |

### `tura_session_db` and gateway `session-log`

`tura_session_db` normally runs as the SQLite owner service. The session-log CLI
is reachable through `tura_gateway session-log ...` for diagnostics. Add
`--admin` to bypass a running service and inspect the SQLite store directly.

| Command | Input |
| --- | --- |
| `tura_gateway session-log upsert-session` | Session upsert JSON on stdin. |
| `tura_gateway session-log list-workspaces` | none. |
| `tura_gateway session-log get-session` | `GetSessionRequest` JSON on stdin. |
| `tura_gateway session-log list-sessions` | `ListSessionsRequest` JSON on stdin. |
| `tura_gateway session-log list-session-records` | `ListSessionRecordsRequest` JSON on stdin. |
| `tura_gateway session-log mark-session-interrupted` | `MarkSessionInterruptedRequest` JSON on stdin. |
| `tura_gateway session-log delete-session` | `DeleteSessionRequest` JSON on stdin. |
| `tura_gateway session-log delete-workspace` | `DeleteWorkspaceRequest` JSON on stdin. |

Every session-log command prints a JSON response and exits non-zero when the
response is an error.
