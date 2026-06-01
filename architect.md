# Tura Session Plan Architecture

## Purpose

This file is the startup document for the current session-plan refactor. It
records the actual code state found in `apps/gui`, `crates/gateway`, and
`crates/runtime`, then fixes the gateway-facing requirements before more source
changes are made.

The current goal is to make task management a session state machine projected
through gateway, not a separate frontend or tool-only concept.

This document is also the cross-workspace contract for the current session,
task, and command-run topology. Keep it aligned with root `README.md` and
`ARCHITECTURE.md` whenever provider auth, command execution, task planning, or
gateway session projection changes.

## Hard Boundaries

- GUI talks to backend code only through `apps/gui/sdk/gateway`.
- Gateway owns session scanning, persistence, message history, task-management
  patches, status projection, workspace scoping, router launch, and forwarding
  agent turns to the router. Gateway runs no in-process agent loop.
- Router owns the agent registry, CLI forwarding, and runtime-worker dispatch
  (`POST /run_agent`). Command alias canonicalization stays in `crates/tools`.
  Spawning is single-direction: gateway → router → runtime worker.
- Runtime owns agent prompt/tool exposure, MANAS loop behavior, compact context,
  and task-management state transitions caused by model tool results. It runs as
  a library inside a runtime worker (gateway binary with
  `TURA_ROLE=runtime_worker`), dispatched by the router.
- Tools own command execution primitives. `task_status` is a command inside
  `command_run`; it must not become an independent top-level tool.
- Tool command `policy.toml` configurables use one shared shape under
  `[configurable]`: `name = { default = "...", enum = ["...", "..."] }`.
  `read_media` and `web_discover` both follow this contract for bounded
  command-local defaults.
- Provider owns OAuth discovery and refresh. Gateway may expose auth status and
  refresh endpoints, but runtime and tools must not paper over missing provider
  credentials.
- No database or new architecture layer is introduced for this phase. Sessions
  remain file-backed and hydrated by gateway from each workspace directory.

## Actual Current Implementation

### Runtime

`crates/runtime/src/state_machine/session_management.rs` already contains the
task-management state fields:

- `TaskStatus`: `todo`, `doing`, `question`, `done`, `archived`
- `StartCondition`: `session_idle`, `user_action`, `scheduled_task`,
  `polling_task`
- `PollInterval`: `m`, `d`, `h`, `s`
- `TaskStep`: `nonce_id`, `step`, `sub_session_id`, `start_at`,
  `poll_interval`, `start_condition`, `task_summary`, `delivery`, and
  execution metadata
- `TaskPlan`: session-level `plan_summary` plus `detailed_tasks`
- `SessionManagement::task_management_json()`: returns an object for zero/one
  task and an object with `tasks` for multi-task sessions

`command_run` already exposes `task_status` as an internal command. It accepts
only optional `task_summary` and optional `status`, where status is limited to
`question` or `done`. It rejects other statuses.

`command_run` is the only model-visible batch execution surface. Same-step
read-only commands may run concurrently; mutating commands and unknown commands
act as ordered barriers. Command execution and file access must reuse the
existing command queue and file-lock behavior.

For `shell_command` and `bash`, the command-specific prompts are now injected
into the `command_run` provider description. Background services must keep a
process handle or PID, write stdout/stderr logs, poll readiness and process
exit together, fail immediately with exit code/log tail if the service exits
before readiness, and clean up only the started process tree on timeout.

`multiple_tasks` uses array input with required `nonce_id` and optional
`step`, `task_summary`, and `delivery`. Runtime rejects updates when a plan already
exists unless the task has clearly changed, and the current implementation
returns an error to the model result.

`task_delivered` is no longer wired as a live top-level command/tool in the
searched source tree. The worktree shows deleted legacy files for
`task_delivered`, so a cleanup pass still needs to verify that the deleted files
are committed and that no empty legacy directory or docs reference remains.

### Gateway

`crates/gateway/src/api/session.rs` and
`crates/gateway/src/session/store.rs` already provide the main contract:

- `GET /session?directory=...` hydrates sessions from the workspace sessions
  directory, filters by workspace, and returns each session with
  `task_management`, `plan_summary`, and `session_display_name`.
- `GET /session/{sessionID}` returns the same task-management projection.
- `GET /session/status` returns all sessions with status plus
  `task_management`, `plan_summary`, and `session_display_name`.
- `POST /session` accepts `task_management` when creating a session.
- `PATCH /session/{sessionID}` accepts `task_management` and applies patches
  into `SessionManagement.task_plan`.
- Gateway applies `task_management` object patches to the first task and array
  or `tasks` patches by `nonce_id`.
- Gateway accepts `start_at` as RFC3339 or epoch milliseconds and projects it
  back as UTC/RFC3339 JSON.
- User messages are appended to message history and also logged into
  `SessionManagement.session_log`.

Gateway remains the API boundary for GUI and TUI. Apps use the gateway SDK or
gateway HTTP API; they do not patch runtime files or call provider/tool crates
directly.

### GUI

`apps/gui/app/src/app.tsx` and `apps/gui/sdk/gateway/src/types.ts` already have
most of the plan UI surface:

- Task-management DTOs include `nonce_id`, `step`, `task_summary`, `delivery`,
  `sub_session_id`, `start_at`, `poll_interval`, `start_condition`,
  `task_status`, `plan_summary`, and `tasks`.
- Plan view filters sessions by current workspace directory.
- Board mode has four visible lanes: `todo`, `doing`, `question`, `done`.
- Archived tickets are hidden from the board and shown under the left archived
  group.
- Ticket cards display a short session id, display name, start condition, and
  local formatted time.
- Tickets can be dragged between board lanes and patched through gateway.
- Plan has icon buttons for gantt, calendar, todo list, and split
  collaboration.
- Gantt and calendar modes render timed sessions and support drag scheduling.
- Plan ticket click opens a right-side conversation panel instead of
  navigating directly to the full conversation page.
- The right panel reuses the conversation view in compact mode and hides the
  command-run sidebar by routing command/tool open actions to the full
  conversation.
- New tickets are created from the right panel composer only after a lane or
  calendar slot is selected.
- The new-ticket flow can choose an existing session in the current workspace
  or create a new session.
- Scheduled and polling task controls convert local `datetime-local` input to
  UTC ISO for gateway and display UTC timestamps as local system time.

## Required Gateway Contract

### Session DTO

Every session returned by gateway must include:

```json
{
  "id": "sess-...",
  "name": "release checklist",
  "directory": "C:/workspace",
  "status": "idle",
  "force_multiple_tasks": false,
  "task_management": {},
  "plan_summary": "release checklist",
  "session_display_name": "release checklist"
}
```

`session_display_name` is chosen from `plan_summary`, then first
`task_summary`, then session `name`. GUI and TUI must prefer it over raw
session id.

Gateway JSON is snake_case at the API boundary. GUI/TUI may map fields to local
language conventions internally, but persisted session state and backend DTOs
should not carry duplicate camelCase aliases.

### Single-Task `task_management`

When multi-task mode is not active, `task_management` is a single object:

```json
{
  "nonce_id": "primary key",
  "step": 0,
  "plan_summary": "frontend/session display name",
  "task_summary": "agent-visible task state-machine name",
  "delivery": "verified release checklist",
  "sub_session_id": "",
  "start_at": "2026-05-25T08:30:00.000Z",
  "poll_interval": { "m": 0, "d": 0, "h": 1, "s": 0 }
}
```

No field is required for a patch. Gateway should patch only fields provided by
the caller. `nonce_id` is the primary task key. `step` is a non-negative
integer.

`plan_summary` and `task_summary` are intentionally different names:

- `plan_summary` names the session/ticket for frontend and TUI display.
- `task_summary` names the agent-visible task state.

Execution lifecycle is not a second naming system. Session `status` describes
backend execution state (`idle`, `busy`, `error`); task-management status
describes ticket state (`todo`, `doing`, `question`, `done`, `archived`) and
must be represented in `task_management`, not as extra alias fields on the
session object.

### Multi-Task Planning

When multi-task mode is enabled, the dynamic command-run text exposes
`multiple_tasks` only as the planning mechanism for the most complex 10% of
requests. The `multiple_tasks` command input is an array:

```json
[
  {
    "nonce_id": "inspect",
    "step": 0,
    "task_summary": "Inspect current wiring",
    "delivery": "Find files, gaps, and verification criteria."
  }
]
```

Only `nonce_id` is required. Runtime should reject mid-execution planning
updates when a planning state machine already exists, return that error to the
agent, and keep the previous state.

### Frontend-To-Gateway Flow

Plan board load:

```text
GUI -> GET /session?directory=<workspace>&includeChildren=true
gateway -> hydrate workspace sessions
gateway -> return sessions with task_management projection
GUI -> group by task-management status and hide archived from normal board
```

Ticket status drag:

```text
GUI -> PATCH /session/{sessionID}
body: { "task_management": { "status": "done" } }
gateway -> patch first task or matching nonce_id
gateway -> persist session file
gateway -> emit session.updated
GUI -> update board from response/event
```

Create new ticket using new session:

```text
GUI -> POST /session
body: {
  "directory": "<workspace>",
  "task_management": {
    "plan_summary": "<ticket title>",
    "task_summary": "agent-visible task name",
    "start_at": "<UTC ISO>",
    "poll_interval": { "m": 0, "d": 0, "h": 1, "s": 0 }
  }
}
gateway -> create session, patch task_management, persist, emit session.created
```

Create new ticket on existing session:

```text
GUI -> PATCH /session/{sessionID}
body: { "task_management": { ...same fields... } }
gateway -> preserve existing context/messages and update task state
```

Submit user input while a session is busy:

```text
GUI -> POST /session/{sessionID}/prompt_async
gateway -> store message with kind=user_new_command
gateway -> append command to user-commands queue
runtime -> fetch /session/{sessionID}/user-commands and inject into next turn
```

## Verified Integration State

Recent local runs prove the main path is connected:

- `frontend-playwright-1779801301855`: `ok:true`, `16/16`, two turns, nine
  command-run commands, zero command failures.
- `programbench-tura-full`: `ok:true`, `25/25`, including ProgramBench
  reconstruction, release executable, docs/submission/eval artifacts, and
  frontend validation.
- `multiple-tasks-backend-topology-full`: `ok:true`, `16/16`, two turns,
  seventeen command-run commands, zero command failures.
- `tui-full-web-terminal-screenshots`: `ok:true`, `16/16`.

These runs cover command-run queue execution, ordered multi-step work, visible
frontend Playwright verification, ProgramBench task decomposition, and TUI
gateway execution. Push-time checks also covered:

```text
cargo test -p code-tools-suite command_run_provider_description_exposes
cargo check -p code-tools-suite
cargo test -p tura-llm-rust oauth
cargo check -p tura-llm-rust
cargo build -p gateway --bin tura --bin gateway
```

OAuth-specific verification showed that local Codex auth discovery is used by
Provider and no longer falls through to an empty OpenAI API key.

## Remaining Work

### Runtime Gaps

- Compact-context injection prompts do not yet append the full
  `task_management` JSON state machine at the fixed-context tail above extra
  user info.
- `task_status` prompt text exists, but the prompt-style interception should be
  audited so it only reminds the agent when neither `question` nor `done` was
  provided and otherwise lets `command_run` continue.
- Delayed, scheduled, and polling tasks are represented as data but not
  executed by a scheduler.
- `start_at` is UTC in state, but the original request said "etc timestamp";
  this should be clarified as UTC/RFC3339 unless "etc" means a different
  domain-specific timestamp.
- The runtime still uses wording such as "delivered" in some user-visible
  internal messages; that is not the old `task_delivered` tool, but the wording
  should be renamed to `done` for consistency.
- Focused tests should assert sub-session context inheritance field by field
  instead of relying only on topology e2e success.

### Gateway Gaps

- Session directory scanning only hydrates the requested workspace. There is no
  cross-workspace archived aggregation endpoint; the GUI currently derives the
  archived group from loaded sessions.
- Invalid `task_management` patches are logged and ignored, but the HTTP
  response does not surface a structured validation error to the GUI.
- There is no scheduler worker to trigger `scheduled_task` or `polling_task`
  sessions later.
- There is no API-level nonce-specific patch shape documented beyond current
  array/object behavior.
- There is no dedicated pending-task queue endpoint for the split plan panel.
- Session status maps only to `idle`, `busy`, and `error`; task status provides
  the richer ticket state.
- GUI/gateway live regression should be rerun after the latest documentation
  and provider/tool prompt changes before release tagging.

### GUI Gaps

- Split panel uses the compact conversation view, but pending follow-up tasks
  are not rendered as a dedicated panel section.
- Clicking a command in the split view routes to the full conversation, but the
  plan panel still depends on generic conversation behavior.
- Calendar/gantt schedule edits update `start_at`; changing `start_condition`
  during date drag is not guaranteed unless the ticket already has a timed
  trigger.
- The plan board is workspace-scoped, but selecting a workspace only displays
  already-hydrated sessions for that workspace; cross-workspace session
  discovery depends on gateway hydration calls.
- Archived display is implemented in the left tree, not as a full archived page
  with per-directory sections loaded independently from gateway.
- The plan composer creates tickets from the right panel, but there is no real
  delayed activation path after creation.

### Test Gaps

- GUI has a Playwright session-plan e2e fixture covering many layout and drag
  behaviors, but it is fixture-driven and should be complemented by a live
  gateway-backed Playwright flow.
- TUI/gateway e2e covers CLI plan list/create/update and archived filtering,
  but GUI data-state tests should assert the gateway persisted JSON after each
  drag/edit.
- Missing tests for malformed `task_management` patches returning useful GUI
  errors.
- Missing tests for scheduler edge cases because scheduler execution does not
  exist yet.
- Missing tests for compact-context tail injection of full task-management JSON.
- Missing focused tests that prove provider auth status shown by GUI/TUI cannot
  drift from the Provider-owned OAuth source of truth.

## Validation Entry Points

Use these after implementation changes:

```text
cargo fmt -p code-tools-suite
cargo check -p code-tools-suite
cargo test -p code-tools-suite
cargo fmt -p gateway
cargo check -p gateway
cargo test -p gateway
cd apps/gui && bun run format:check
cd apps/gui && bun run typecheck
cd apps/gui && bun run build
python apps/gui/e2e/session_plan_e2e.py
node apps/tui/e2e/tui_gateway_cli_e2e.mjs
```

## Refactor Rule

Do not rewrite the whole repository in one change. Add one bounded capability
at a time, update this architecture file with the observed contract, then add
tests at the same boundary.
