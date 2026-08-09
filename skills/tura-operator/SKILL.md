---
name: tura-operator
description: >
  Tura operator's manual for AI agents. Use when an agent needs to understand how
  Tura works, configure providers/agents/personas, operate the CLI, interpret
  command_run, manage sessions, choose model tiers, or troubleshoot the
  router→runtime→gateway→session_log pipeline. Covers settings, CLI options,
  environment variables, architecture, unintuitive opinions, and trade-offs.
  Not for writing Tura source code — use the architecture docs and AGENTS.md
  files for that.
---

# Tura Operator's Manual

This is the user manual for operating Tura as an AI agent. It turns you into a
Tura operation expert: what each part does, when to use it, and the trade-offs
that are not obvious from the help text.

## When to use

- Configuring providers, agents, personas, or model routes
- Operating the CLI (`tura`, `tura run`, `tura exec`, `tura provider`, `tura config`)
- Interpreting `command_run` batches, steps, and tool results
- Managing sessions, compaction, checkpoints, and session logs
- Choosing between agents (`balanced` vs `direct`), tiers (`thinking` vs `fast`),
  and reasoning levels
- Troubleshooting the process pipeline (router, runtime, gateway, session_db)
- Setting environment variables for sandbox, shell, models.dev, or profiling

## When NOT to use

- Writing or modifying Tura Rust source code — read `ARCHITECTURE.md` and the
  per-crate `ARCHITECTURE.md` files instead.
- Contributing guidelines — read `docs/contributing-guide.md`.
- Benchmark methodology — read `docs/start/overview.md` and the benchmark repo.

## Architecture in 60 seconds

Tura is a local agent runtime with a single backend pipeline and many thin
fronts:

```
TUI / GUI / CLI
  → gateway (HTTP/SSE, session manager, OAuth lifecycle)
    → router (dispatch, registry, supervision, owns session_db)
      → runtime (per-session agent worker, prompt assembly, tool catalog)
        → provider (model access, auth, retry, streaming)
        → tools (command_run → shell, apply_patch, web_discover, ...)
      → session_log (SQLite session store, one owner per home)
```

**Key invariant**: fronts never own the session database. They talk to the
router daemon. One isolated backend per `TURA_HOME`.

| Binary | Role | Instances |
|---|---|---|
| `tura_router` | Dispatch, registry, supervision, owns session_db | 1 per home |
| `tura_runtime` | Per-session agent worker, spawned by router | many |
| `tura_gateway` | HTTP/SSE front (GUI/TUI), holds stdin lifetime lease | 1 per home |
| `tura_exec` / `tura` | CLI thin front | per call |
| `tura_session_db` | SQLite session-store owner | 1 per home |

Process lifecycle: closing the front closes the pipe → gateway exits → router
self-shuts after idle grace. A standalone CLI call uses a long-lived request
socket; if that socket closes mid-turn, the router cancels the active runtime.

## CLI quick reference

### Main entry points

| Command | Use |
|---|---|
| `tura` | Open the TUI (default) |
| `tura run "prompt"` | Gateway-backed non-interactive prompt |
| `tura exec "prompt"` | Direct Rust CLI prompt runner |
| `tura bash/zsh/shel "prompt"` | Gateway-backed prompt with forced shell |
| `tura session list` | List sessions |
| `tura resume SESSION_ID` | Show or append to a session |
| `tura provider list` | List providers and auth state |
| `tura config get` | Print full session config as JSON |

### `tura run` key options

| Option | Meaning |
|---|---|
| `-m MODEL` | Model override (`PROVIDER/MODEL`) |
| `-a ID` | Agent override (default: `balanced`) |
| `--model-variant LEVEL` | Reasoning effort: `low`/`medium`/`high`/`xhigh`/`max` |
| `-p` | Enable priority model routing |
| `--session ID` | Append to existing session |
| `--output text\|json\|ndjson` | Output mode (default: `text`) |
| `--timeout SEC` | Abort after N seconds (default: 600) |
| `-c KEY=VALUE` | Runtime config override |

### `tura exec` key options

| Option | Meaning |
|---|---|
| `-C PATH` | Workspace directory |
| `-m MODEL` | Model override |
| `--session-id ID` | Reuse a deterministic session id |
| `--goal` | Keep running until `task_status` marks `done` or `question` |
| `--json` | Emit JSONL events on stdout |
| `--embedded` | Run runtime in-process (diagnostic, skips router daemon) |
| `--sandbox` | Restrict `command_run` to workspace |
| `--planning auto\|on\|off` | Planning override |

### Config commands

```bash
tura config get                          # Full config as JSON
tura config set agent=direct model_variant=medium
tura config model-tiers                  # List tiers
tura config model-tier thinking codex/gpt-5.6-sol
```

Config keys: `agent`, `persona`, `model`, `session_type`, `model_variant`,
`model_acceleration_enabled`, `context_message_limit`, `kill_processes_on_start`,
`validator_enabled`, `language`, `command_run_stall_guard_profile`.

### Provider commands

```bash
tura provider list                       # Providers + auth state
tura provider status                     # Auth status for all
tura provider login codex                # OAuth login
tura provider set-auth openai --key sk-...  # Store API key (validated)
tura provider logout anthropic           # Remove saved auth
```

### Session commands

```bash
tura session list [--all] [--json]
tura session show SESSION_ID
tura session abort SESSION_ID
tura resume SESSION_ID [PROMPT...]
tura resume --last [PROMPT...]
```

## Configuration

### File locations

| Setting | Location |
|---|---|
| Workspace runtime settings | `<workspace>/.tura/config.conf` |
| Provider config | `TURA_PROVIDER_CONFIG` or `crates/provider/config/provider_config.json` |
| Provider credentials | `.env` (resolved from `TURA_ENV_PATH` or project root) |
| Custom agents | `agents/src/<agent_id>/agent_config.json` + `prompt.md` |
| Session DB | `<TURA_HOME>/db/session_log/index.sqlite3` |
| Workspace history | `<workspace>/.tura/session_log.sqlite3` |
| Provider call logs | `log/provider/YYYY-MM-DD/HHMMSS_mmm_<call_id>.json` |

### Critical environment variables

| Variable | Meaning |
|---|---|
| `TURA_HOME` | Instance home — derives all sockets/locks/db. Isolates dev/release |
| `TURA_PROJECT_ROOT` | Project root for agent/persona/provider discovery |
| `TURA_PROVIDER_CONFIG` | Override provider config file path |
| `TURA_ENV_PATH` | Path to `.env` for secrets |
| `TURA_MODELS_DEV_CATALOG` | Path to a models.dev API snapshot for dynamic provider import |
| `TURA_COMMAND_RUN_SHELL` | Force `shell_command`, `bash`, or `zsh` |
| `TURA_COMMAND_RUN_SANDBOX` | `=1` enables workspace-bound sandbox |
| `TURA_ZSH_PATH` | Custom zsh binary path |
| `TURA_STRICT_SHELL_TOOL_COVERAGE` | `=1` turns shell coverage warnings into failures |
| `TURA_PROFILE_TURN_TIMINGS` | Enable runtime timing events |
| `TURA_FRONTEND_SOURCE` | `cli` triggers CLI communication style |
| `TURA_SESSION_PERSONA` | Select persona at runtime |
| `TURA_GATEWAY_PORT` / `PORT` | Gateway port (debug=4125, release=4126) |

### `.tura/config.conf` keys

| Key | Default | Purpose |
|---|---|---|
| `session_type` | `coding` | `coding`, `business`, `research`, `planning` |
| `model_variant` | `high` | `low`, `medium`, `high`, `xhigh`, `max` |
| `model_acceleration_enabled` | `false` | Priority routing flag |
| `active_agent` | `balanced` | Default agent id |
| `language` | `en` | `en` or `zh-CN` |
| `command_run_stall_guard_profile` | `balanced_20s` | `fast_10s`, `patient_30s`, `long_io_60s`, `off` |
| `kill_processes_on_start` | — | Clean up child processes on session start |

## Providers and auth

### Common providers

| Provider id | Auth | Credential env |
|---|---|---|
| `codex` | OAuth (ChatGPT subscription) | `OPENAI_API_KEY` (managed) |
| `openai` | API key | `OPENAI_API_KEY` |
| `anthropic` | API key | `ANTHROPIC_API_KEY` |
| `claude-code` | OAuth / local CLI token | `CLAUDE_CODE_OAUTH_TOKEN` |
| `google` / `gemini` | API key or OAuth | `GOOGLE_API_KEY` / `GEMINI_API_KEY` |
| `openrouter` | API key | `OPENROUTER_API_KEY` |
| `deepseek` | API key | `DEEPSEEK_API_KEY` |
| `opencode-go` | API key | `OPENCODE_API_KEY` |
| `cline-pass` | API key | `CLINE_API_KEY` |

### Model tiers and routes

Bundled tiers: `thinking` (main reasoning), `fast` (lower-latency),
`embedding_high`, `embedding_low`.

A route is an ordered list of provider/model candidates. Runtime tries the
first; falls back on retryable failure.

**`thinking` route** (first candidates):
`codex/gpt-5.6-sol` → `codex/gpt-5.6-terra` → `openai/gpt-5.6-sol` →
`openai/gpt-5.6-terra` → `codex/gpt-5.5` → `openai/gpt-5.5-pro` →
`anthropic/claude-opus-4-7` → `antigravity/gemini-3.1-pro-preview` → ...

**`fast` route**: `codex/gpt-5.6-luna` → `openai/gpt-5.6-luna` →
`codex/gpt-5.3-codex-spark` → `deepseek/deepseek-v4-flash` →
`qwen/qwen3.6-flash` → `google/gemini-3.5-flash` → ...

### models.dev integration

Set `TURA_MODELS_DEV_CATALOG` to a downloaded `models.dev/api.json` snapshot.
Tura projects OpenAI-compatible providers from the catalog into the runtime
config — no need to hardcode provider metadata for catalog-backed providers.
Imported models land in a `models_dev` bucket and are skipped if the provider id
is already occupied by a static config entry.

### Auth methods

`tura provider set-auth` validates before saving. It writes to the `.env` file
resolved by `TURA_ENV_PATH` (or project root `.env` by default). That file is
gitignored.

## Agents

Agents are runtime work profiles. Each agent controls: prompt resources,
allowed command capabilities, provider/model route, operation-manual policy,
and reporting/validation flags.

### Built-in agents

| Agent | Aliases | Intent | Tier |
|---|---|---|---|
| `balanced` | `thinking` | Verification + reflective checks; default | `thinking` |
| `direct` | `fast` | Lower-friction direct work | `fast` |
| `direct-text-only` | `fast-text-only` | Quick, light verification | `fast` |

All three use `codex/gpt-5.6-sol`, reasoning `high`, priority acceleration,
streaming, temperature 0.2, timeout 120s — unless overridden by route or
`current_model`.

**Trade-off**: `balanced` does more self-reflection and verification → higher
success rate, more tokens. `direct` skips reflection → faster, cheaper, lower
success on hard tasks. Choose `balanced` for unfamiliar or risky work; `direct`
for well-understood mechanical changes.

### Agent vs Persona vs Runtime Prompt

| Layer | Owns | Does not own |
|---|---|---|
| Agent prompt | Work style, tool discipline, model route | UI identity, task manuals |
| Persona | Visible identity, tone, media expressions | Engineering capabilities |
| Runtime Prompt manual | Task-specific rules selected by `task_type` | Agent identity or model route |

You can change a persona without giving it more tools. You can enable a manual
without copying rules into every agent. You can switch a model route without
editing behavior prose.

## command_run — the single model-visible tool

The provider sees one tool (`command_run`) instead of many. It wraps a
`commands` array, each with `command_type`, `command_line`, and `step`.

### Why one tool

Ordinary tool calling makes every file read, patch, test, and status update
compete for another provider-visible turn. `command_run` batches them:

| Work | Ordinary agent | Tura `command_run` |
|---|---|---|
| Read + patch + test + status | ~4 LLM turns | 1 LLM turn (3 steps) |

At 40k input tokens per turn, four ordinary turns replay ~160k tokens; one
`command_run` turn replays 40k once. The gain appears when several local actions
are already known and can be batched.

### Step semantics

`step` is a **dependency group**, not a serial command number:
- Same-step read-only commands run concurrently as a `macro_command` batch
- Different steps run in ascending order
- Mutating commands need compatible file locks
- A failed `apply_patch` cancels later commands

**Opinion**: the schema says `minItems: 5`. The model should batch aggressively
— prefer 5+ commands during real task execution. Put independent reads/searches
in the same step. Do not invent probes that depend on unknown earlier output.

### Built-in command types

| Command | Shell-backed | Purpose |
|---|---|---|
| `shell_command` / `bash` / `zsh` | Yes | Run local shell commands |
| `apply_patch` | No | Structured patch via Tura's patch parser |
| `task_status` | No | Update task state, task type, compaction handoff |
| `planning` | No | Optional structured planning |
| `web_discover` | External | Search/fetch websites and media |
| `read_media` | External | Inspect images, PDFs, audio, video |
| `generate_media` | External | Generate images or speech |
| `compact_context` | Internal | Structured handoff for long sessions |

### Shell surface selection

| OS | Default shell |
|---|---|
| Windows | PowerShell-backed `shell_command` |
| macOS | `zsh` |
| Other Unix | `bash` |

Override with `TURA_COMMAND_RUN_SHELL`. Use `TURA_ZSH_PATH` for a custom zsh
binary.

### Sandbox

`TURA_COMMAND_RUN_SANDBOX=1`:
- `apply_patch` paths must stay inside the workspace
- Shell `workdir`/`cwd` must stay inside the workspace
- Violations return exit code 126 (`SandboxViolation`)

### File locks

- Reads acquire shared locks; writes acquire exclusive locks
- `apply_patch` declares affected paths before execution
- Unknown mutating shell commands acquire workspace-wide exclusive lock
- Locks acquired in sorted path order; released on all outcomes

## Sessions and lifecycle

### Session types

| Type | Use |
|---|---|
| `coding` | Default — code changes with command_run |
| `business` | Business logic flows |
| `research` | Investigation without code changes |
| `planning` | Multi-task planning/delegation |

### Compaction (compact_context)

For long coding sessions, `compact_context` is injected in the last step. The
model produces a structured handoff summary: progress, requirements, files,
completed/remaining work, validation status, next steps. After completion:
runtime removes prior tool-call history, converts the summary into the next
user-context item, preserves session/task state, and reinjects a workspace
snapshot.

**When it triggers**: automatically when context approaches limits in long
coding sessions. The model calls `task_status` with `compact_context` as the
last command in a batch.

### Checkpoint

Runtime checkpoints session state so a crashed or cancelled worker can resume.
The session log queue is durable — if the session_db service restarts, queued
writes are replayed.

### Stall guard

Prevents infinite loops on stalled commands. Profiles:

| Profile | Check interval | Identical checks |
|---|---|---|
| `fast_10s` | 10s | Fewer |
| `balanced_20s` | 20s | Default |
| `patient_30s` | 30s | More |
| `long_io_60s` | 60s | Long I/O |
| `off` | — | Disabled |

## Unintuitive opinions and trade-offs

### 1. One tool, not many

Tura intentionally exposes one `command_run` tool to the provider, not a pile
of individual tools. This is not about JSON aesthetics — it is fewer LLM round
trips. If you are used to agents that call `read_file`, then `edit_file`, then
`run_tests` in separate turns, Tura's model should batch all of that into one
`command_run` with multiple steps.

### 2. `minItems: 5` is serious

The command schema requires at least 5 commands. This is not a typo. The model
should plan enough work to fill a batch. If you only need one command, you
probably have not thought ahead enough — batch the reads, the patch, the
validation, and the status update together.

### 3. Agent ≠ persona ≠ prompt

In many frameworks, a "custom agent" is a large system prompt plus a model
name. Tura splits these: agent (work style + tools + route), persona (voice +
identity), runtime prompt manual (task-specific rules). Do not paste behavior
rules into a persona, and do not paste tool policy into a prompt manual. Each
concern has its own field.

### 4. `current_model` overrides the tier

An agent says "use the thinking tier" via `default_model_tier`. But
`current_model` (an exact `provider/model` pair) wins over the tier. This means
you can pin an agent to a specific model without changing the route catalog.

### 5. `--embedded` is diagnostic only

`tura exec --embedded` runs the runtime in-process instead of through the
router daemon. This skips the normal process pipeline. Use it for debugging
router dispatch issues, not for production work.

### 6. Provider validation is strict

`tura provider set-auth` validates the key before saving. A configured provider
or a mocked request is not proof of production compatibility. The benchmark
evidence is concentrated on GPT-5.6 SOL and Codex/OpenAI configurations — other
providers have less long-horizon evidence.

### 7. `TURA_HOME` isolates everything

`TURA_HOME` derives all sockets, locks, and database paths. This is how dev and
release builds coexist. If you are debugging "why can't the router find the
session_db," check `TURA_HOME` first.

### 8. Fronts never own the session database

The TUI, GUI, and CLI never write to SQLite directly. They talk to the router
daemon, which owns the session_db. If you see a `.tura/session_log.sqlite3` in
a workspace, that is the workspace-level history — the canonical index is under
`TURA_HOME`.

### 9. models.dev is the preferred way to add providers

Instead of hardcoding provider catalog metadata for every OpenAI-compatible
provider, set `TURA_MODELS_DEV_CATALOG` to a models.dev snapshot. Tura imports
the provider, models, base URL, and env names automatically. Hardcoded static
entries take precedence (they occupy the id first), so use the catalog for
new/uncatalogued providers and static entries for providers needing custom
metadata.

### 10. `kill_processes_on_start` is destructive

When enabled, Tura kills child processes from previous sessions on session
start. This is useful for cleanup but will terminate anything the previous
session left running. Enable deliberately.

## Troubleshooting

- **Router can't find session_db**: Check `TURA_HOME`. Router and session_db
  must agree on the home directory. Dev and release builds have different
  defaults.
- **Provider auth fails after `set-auth`**: The key was validated at set time.
  Re-run `tura provider set-auth` or `tura provider login` for OAuth.
- **`command_run` returns `SandboxViolation`**: `TURA_COMMAND_RUN_SANDBOX=1`
  is active. All paths must be inside the workspace.
- **Gateway won't start**: Check port conflicts (`TURA_GATEWAY_PORT` / `PORT`).
  Debug=4125, release=4126. Verify `TURA_GUI_DIST` for desktop GUI.
- **Session is stuck**: Use `tura session abort SESSION_ID`. If unresponsive,
  check `tura inspect status` for orphaned processes.

## Reference routing

| Need | Read |
|---|---|
| Full architecture | `ARCHITECTURE.md` |
| CLI parameters (complete) | `docs/start/cli-parameters.md` |
| Installation | `docs/start/install.md` |
| Provider setup | `docs/start/providers.md` |
| Custom providers | `docs/customization/custom-providers.md` |
| Custom agents | `docs/customization/custom-agents.md` |
| command_run deep dive | `docs/core/command-run.md` |
| Commands reference | `docs/core/commands.md` |
| Agents internals | `docs/core/agents.md` (also `docs/core/AGENTS.md`) |
| Personas | `docs/core/personas.md` |
| Known issues | `docs/KNOWN_ISSUES.md` |
| Provider config schema | `crates/provider/config/provider_config.json` |
| Agent config fields | `agents/src/<agent_id>/agent_config.json` |

## Completion

You are operating Tura correctly when:
- You batch work into `command_run` with 5+ commands and appropriate steps
- You choose `balanced` for unfamiliar work, `direct` for mechanical changes
- You use `thinking` tier for hard reasoning, `fast` for simple tasks
- You configure providers via `tura provider set-auth` / `login`, not by
  hand-editing `.env`
- You use `TURA_MODELS_DEV_CATALOG` for catalog-backed providers
- You check `TURA_HOME` when the pipeline can't find its services
- You use `tura session abort` instead of killing processes manually
