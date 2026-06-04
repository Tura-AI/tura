# apps/gui Architecture

## Goal

`apps/gui` is the graphical Tura client and the only browser/desktop-facing UI
for the gateway. It must preserve the current Tura architecture:

- the GUI talks to `crates/gateway` only through the TypeScript gateway SDK
- runtime execution, provider auth, PTY, file IO, command execution, session
  persistence, and credential persistence stay behind gateway-owned APIs
- frontend state is a projection of gateway state plus short-lived UI drafts

This document also defines the Multica-compatible product surface that must be
recreated in this GUI without bypassing Tura's gateway/session/runtime system.
The goal is functional parity with Multica's collaboration platform: workspaces,
issue board/list/gantt, projects, agents, runtimes/daemon, skills, autopilots,
chat, inbox, settings, search, invitations, members, squads, usage dashboards,
GitHub integration, attachments, reactions, comments, notifications, onboarding,
analytics/feedback/contact-sales surfaces, i18n, CLI-supporting token flows,
public informational routes, and desktop-specific overlays.

The first screen must be a usable workbench/dashboard, never a marketing page.

## Non-Goals

- Do not import or call Rust backend crates directly from the GUI.
- Do not connect directly to `crates/runtime`, `crates/provider`,
  `crates/router`, `crates/tools`, or shell processes.
- Do not persist provider credentials, session records, messages, issue data,
  workspace membership, or task queue state directly from the GUI.
- Do not create a second local database in the GUI.
- Do not make `/tui/*` compatibility routes the main GUI path.
- Do not hide Multica features behind placeholder text. Every route described
  here must map to a real gateway contract before code implementation.

## Existing Directory Assumption

`apps/gui` currently has this high-level shape:

```text
apps/gui/
  app/
  sdk/
  ARCHITECTURE.md
```

The target layout keeps the same separation and can grow incrementally:

```text
apps/gui/
  app/
    src/
      entry.tsx
      app.tsx
      routes/
        auth/
        workspace/
        issues/
        projects/
        agents/
        runtimes/
        skills/
        autopilots/
        chat/
        inbox/
        dashboard/
        settings/
        search/
      state/
        global-store.ts
        workspace-store.ts
        issue-store.ts
        event-reducer.ts
        query-keys.ts
        optimistic.ts
        drafts.ts
      context/
        gateway.tsx
        auth.tsx
        workspace.tsx
        realtime.tsx
        navigation.tsx
      components/
        layout/
        workbench/
        issues/
        projects/
        agents/
        runtimes/
        skills/
        autopilots/
        chat/
        inbox/
        settings/
        editor/
        attachments/
        search/
        modals/
        common/
      styles/
        index.css
  sdk/
    gateway/
      src/
        client.ts
        event-source.ts
        errors.ts
        types.ts
        multica-types.ts
```

Shared UI packages may be added under `packages/*` later, but the IO boundary
remains `sdk/gateway`.

## Compatibility Strategy

Tura and Multica have different cores:

- Tura currently centers on local coding sessions, PTY, files, providers, VCS,
  commands, and gateway SSE.
- Multica centers on workspace collaboration where people and agents share
  issues, projects, comments, tasks, skills, chat, notifications, and scheduled
  automation.

The GUI must combine them by treating Multica objects as gateway-backed product
domains and treating Tura sessions as the execution transcript for agent work.

```text
Multica issue / chat / autopilot trigger
  -> gateway creates or resumes an agent task
  -> gateway starts or resumes a Tura session/turn
  -> runtime/router/provider/tool work stays inside Tura backend
  -> gateway emits task/message/activity events
  -> GUI updates issue board, chat, inbox, task transcript, and usage views
```

No Multica feature should spawn local processes from the GUI. When a feature
needs execution, the GUI asks gateway to create/update an agent task or session.

## Gateway URL And Auth

Default gateway URL resolution:

1. explicit app setting
2. `TURA_GATEWAY_URL`
3. `http://127.0.0.1:4096`

The Multica-compatible API must support:

- cookie/JWT auth for web-like sessions
- PAT auth for CLI/desktop automation
- daemon token auth for local daemon/runtimes
- workspace scoping by `X-Workspace-ID`, `X-Workspace-Slug`, or
  `workspace_id`/`workspace_slug` query params
- directory scoping for existing Tura session/file/PTY APIs via
  `?directory=...` or `x-opencode-directory`

The GUI stores only short-lived auth state and submitted token strings before
they are sent to gateway. Durable secrets belong to gateway.

## Session Log And Provider Diagnostics

The GUI queries session history only through gateway APIs. It must not read
`.tura/sessions`, `db/session_log`, provider logs, or backend config files
directly.

Session-log API:

```text
GET /session-log/workspaces
GET /session-log/sessions?workspace=<workspace>&page=0&page_size=50
GET /session-log/{sessionID}/records?page=0&page_size=100
```

The gateway SDK exposes these as `sessionLogWorkspaces`,
`sessionLogSessions`, and `sessionLogRecords`. Provider call logs remain
backend diagnostics under `log/provider/YYYY-MM-DD/*.json`; GUI views should
request summarized provider status/usage through gateway/provider APIs instead
of opening those files.

## Core Tura Endpoint Map

The existing coding workbench keeps using these gateway routes:

```text
health                         GET    /global/health
event stream                   GET    /event
global config                  GET    /config
global config patch            PATCH  /config
paths                          GET    /path
workspace config               GET    /session/config?directory=...
workspace config patch         PATCH  /session/config?directory=...
projects/worktrees             GET    /project, GET /project/current
sessions                       GET    /session, POST /session
session detail                 GET/PATCH/DELETE /session/{sessionID}
messages                       GET    /session/{sessionID}/message
send prompt sync               POST   /session/{sessionID}/message
send prompt async              POST   /session/{sessionID}/prompt_async
abort                          POST   /session/{sessionID}/abort
fork/revert/unrevert           POST   /session/{sessionID}/fork|revert|unrevert
todos                          GET/POST /session/{sessionID}/todo
permissions                    GET    /permission
permission reply               POST   /permission/{requestID}/reply
questions                      GET    /question
question reply/reject          POST   /question/{requestID}/reply|reject
providers/models               GET    /provider
provider auth                  /provider/{providerID}/auth/* and /auth/{providerID}
files                          GET/POST /file, /file/content, /file/status
find                           GET    /find, /find/file, /find/symbol
vcs                            GET    /vcs, /vcs/diff
pty                            /pty and /pty/{ptyID}/connect
mcp                            /mcp/*
agents                         GET    /agent
commands                       GET/POST /command
services                       GET    /service/status
skills/plugins                 GET    /skill, GET /plugin
formatter/log                  POST   /formatter, POST /log
```

## Session Plan And Task Management

The plan surface is a gateway-backed projection of session task-management
state. GUI code must treat `crates/gateway` as the source of truth and must not
scan session files directly.
GUI product types and rendering paths must remain benchmark-agnostic. Long e2e
fixtures may define benchmark prompts and evaluators, but those names must not
enter SDK types or normal application state.

Current session-plan data comes from:

```text
GET /session?directory=<workspace>&includeChildren=true
GET /session/{sessionID}
GET /session/status
GET /session/{sessionID}/todo
PATCH /session/{sessionID}
PATCH /session/{sessionID}/task-management
POST /session
```

The GUI SDK consumes the gateway session fields directly.
Rendering should prefer:

1. `session_display_name`
2. `plan_summary`
3. task `task_summary`
4. session `name`
5. `New Session`

Plan tickets are workspace/directory scoped. The short session id is metadata;
the visible name should come from the display-name chain above.

Task status values are:

```text
todo
doing
question
done
archived
```

The plan board shows `todo`, `doing`, `question`, and `done` as normal lanes.
`archived` is hidden from normal lanes and appears under the workspace-aware
archived group in the left tree.

Plan modes are implemented as icon controls in the upper-right page actions:

```text
board/todo-list
gantt
calendar
split collaboration
```

The right split panel reuses the compact conversation view. It shows
conversation history, schedule controls, and pending task controls derived from
`task_management` and `/todo`. The command-run inspector is hidden in this
compact panel; opening concrete command/tool text navigates to the full
conversation page.

Schedule controls display local system time. Before sending patches, the GUI
converts `datetime-local` values to UTC ISO strings. Gateway/runtime store UTC.
Polling intervals preserve the existing `{ m, d, h, s }` shape.

Single-task patches are sent as an object. Multi-task updates that need precise
task matching should send `task_management.tasks[]` entries with `nonce_id`.
The GUI may optimistically update local state for responsiveness, but it must
reconcile from the gateway response or a safe session refresh.

## Multica-Compatible Endpoint Map

The GUI must also be designed around this product API surface. Route names are
kept Multica-compatible so existing mental models, tests, and CLI flows can be
ported with minimal translation.

```text
public health/config           GET    /health, /readyz, /api/config
auth                           POST   /auth/send-code, /auth/verify-code, /auth/google, /auth/logout
current user                   GET/PATCH /api/me
onboarding                     PATCH/POST /api/me/onboarding/*
cli token                      POST   /api/cli-token
file upload                    POST   /api/upload-file
feedback/contact sales         POST   /api/feedback, /api/contact-sales
workspaces                     GET/POST /api/workspaces
workspace detail               GET/PUT/PATCH/DELETE /api/workspaces/{id}
members                        GET/POST /api/workspaces/{id}/members
member update/remove           PATCH/DELETE /api/workspaces/{id}/members/{memberId}
invitations                    GET /api/invitations, GET/POST /api/invitations/{id}/accept|decline
workspace invitations          GET/DELETE /api/workspaces/{id}/invitations/{invitationId}
tokens                         GET/POST/DELETE /api/tokens
issues                         GET/POST /api/issues
issue search/grouped/progress  GET /api/issues/search|grouped|child-progress
quick create/batch             POST /api/issues/quick-create|batch-update|batch-delete
issue detail                   GET/PUT/DELETE /api/issues/{id}
comments/timeline              GET/POST /api/issues/{id}/comments, GET /api/issues/{id}/timeline
subscribers                    GET/POST /api/issues/{id}/subscribers|subscribe|unsubscribe
issue task controls            GET /api/issues/{id}/active-task|task-runs|usage
issue rerun/cancel             POST /api/issues/{id}/rerun, /api/issues/{id}/tasks/{taskId}/cancel
issue labels                   GET/POST/DELETE /api/issues/{id}/labels
issue metadata                 GET/PUT/DELETE /api/issues/{id}/metadata/{key}
issue PRs                      GET /api/issues/{id}/pull-requests
reactions                      POST/DELETE /api/issues/{id}/reactions, /api/comments/{id}/reactions
comments                       PUT/DELETE /api/comments/{commentId}
comment resolve                POST/DELETE /api/comments/{commentId}/resolve
attachments                    GET/DELETE /api/attachments/{id}, GET /api/attachments/{id}/content
labels                         GET/POST/GET/PUT/DELETE /api/labels
projects                       GET/POST /api/projects
project search/detail          GET /api/projects/search, GET/PUT/DELETE /api/projects/{id}
project resources              GET/POST/DELETE /api/projects/{id}/resources
squads                         GET/POST/GET/PUT/DELETE /api/squads
squad members                  GET/POST/DELETE/PATCH /api/squads/{id}/members/*
agents                         GET/POST /api/agents
agent templates                GET /api/agent-templates, POST /api/agents/from-template
agent detail                   GET/PUT /api/agents/{id}
agent archive/restore          POST /api/agents/{id}/archive|restore
agent task controls            POST /api/agents/{id}/cancel-tasks, GET /api/agents/{id}/tasks
agent skills                   GET/PUT /api/agents/{id}/skills
skills                         GET/POST /api/skills
skill import/detail/files      POST /api/skills/import, GET/PUT/DELETE /api/skills/{id}/*
runtimes                       GET /api/runtimes
runtime update/detail          PATCH/DELETE /api/runtimes/{runtimeId}
runtime usage/activity         GET /api/runtimes/{runtimeId}/usage|usage/by-agent|usage/by-hour|activity
runtime update request         POST/GET /api/runtimes/{runtimeId}/update
runtime model list             POST/GET /api/runtimes/{runtimeId}/models
runtime local skills           POST/GET /api/runtimes/{runtimeId}/local-skills/*
cloud runtime                  /api/cloud-runtime/*
autopilots                     GET/POST /api/autopilots
autopilot detail               GET/PATCH/DELETE /api/autopilots/{id}
autopilot run/trigger          POST /api/autopilots/{id}/trigger
autopilot runs                 GET /api/autopilots/{id}/runs/{runId?}
autopilot triggers             POST/PATCH/DELETE /api/autopilots/{id}/triggers/{triggerId?}
autopilot webhook security     POST/PUT /rotate-webhook-token, /signing-secret
autopilot deliveries           GET/POST /api/autopilots/{id}/deliveries/{deliveryId?}/replay
chat sessions                  GET/POST /api/chat/sessions
chat detail                    GET/PATCH/DELETE /api/chat/sessions/{sessionId}
chat messages                  GET/POST /api/chat/sessions/{sessionId}/messages
chat pending/read              GET /pending-task, POST /read, GET /api/chat/pending-tasks
inbox                          GET /api/inbox, GET /api/inbox/unread-count
inbox actions                  POST /api/inbox/mark-all-read|archive-all|archive-all-read|archive-completed
inbox item actions             POST /api/inbox/{id}/read|archive
notification preferences       GET/PUT /api/notification-preferences
dashboard usage                GET /api/dashboard/usage/daily|by-agent|agent-runtime|runtime/daily
agent presence snapshots       GET /api/agent-task-snapshot, /api/agent-activity-30d, /api/agent-run-counts
GitHub integration             GET/DELETE /api/workspaces/{id}/github/*
webhooks                       POST /api/webhooks/autopilots/{token}, /api/webhooks/github
daemon                         /api/daemon/*
WebSocket/SSE realtime         GET /ws and/or GET /event
```

## Product Routes

The GUI route map must cover all Multica pages:

```text
/                                  route resolver or dashboard
/login                             email code + Google login
/auth/callback                     OAuth callback
/workspaces/new                    create workspace
/invite/:id                        accept/decline invite
/invitations                       personal invitation inbox
/onboarding                        welcome, workspace, runtime, agent, complete
/:workspace/issues                 issue list/board/gantt
/:workspace/issues/:id             issue detail
/:workspace/my-issues              assigned/created/my-agent scopes
/:workspace/projects               project list
/:workspace/projects/:id           project detail
/:workspace/autopilots             autopilot list
/:workspace/autopilots/:id         autopilot detail, triggers, runs, deliveries
/:workspace/agents                 agent list
/:workspace/agents/:id             agent detail/profile/activity/config
/:workspace/members/:id            member profile/detail
/:workspace/squads                 squad list
/:workspace/squads/:id             squad detail
/:workspace/runtimes               runtime list
/:workspace/runtimes/:id           runtime detail, usage, activity, updates
/:workspace/skills                 skill library
/:workspace/skills/:id             skill detail/editor/files
/:workspace/inbox                  inbox
/:workspace/settings               account + workspace settings
/:workspace/usage                  usage dashboard
/:workspace/attachments/:id/preview attachment preview
/download                          optional public download page
/changelog                         optional public changelog page
/about                             optional public about page
/contact-sales                     optional public contact-sales page
```

Desktop overlays, when implemented, must not fork business logic:

- create workspace overlay
- invite accept overlay for `tura://invite/{id}` or future deep link
- onboarding overlay
- local daemon/runtime status panel
- update dialog
- immersive mode toggle

## State Model

Use a layered state model:

```text
GlobalGatewayProvider
  -> gateway URL, auth user, config, health, workspaces, invitations
  -> global realtime connection

WorkspaceProvider(workspace)
  -> workspace detail, members, agents, runtimes, labels, projects
  -> issue board/list/gantt caches
  -> chat sessions, inbox counts, pins, notification preferences
  -> filters realtime events by workspace id/slug

ExecutionProvider(directory/session)
  -> existing Tura sessions, messages, todos, permissions, questions
  -> file tree, VCS diff/status, PTY, MCP, provider/model state

RouteLocalState
  -> filters, selected tabs, modals, drafts, optimistic drag order
```

Server state should be cached by query keys. Local drafts may persist in browser
storage only for UX recovery: issue create draft, comment draft, chat draft,
new project draft, filters, last viewed workspace/issue, and theme. Durable
domain data belongs to gateway.

## Startup Hydration

On app startup:

```text
GET /global/health
GET /api/config
GET /api/me
GET /api/workspaces
GET /api/invitations
GET /path
connect realtime (/ws or /event)
resolve destination:
  - pending invite -> /invitations or invite overlay
  - no workspace -> /onboarding or /workspaces/new
  - selected workspace -> /:workspace/issues
```

When a workspace opens:

```text
GET /api/workspaces/{id}
GET /api/workspaces/{id}/members
GET /api/agents
GET /api/runtimes
GET /api/projects
GET /api/labels
GET /api/pins
GET /api/inbox/unread-count
GET /api/agent-task-snapshot
GET /api/issues/grouped or /api/issues
GET /provider, /agent, /command, /service/status when execution panels are visible
```

When an issue opens:

```text
GET /api/issues/{id}
GET /api/issues/{id}/comments
GET /api/issues/{id}/timeline
GET /api/issues/{id}/subscribers
GET /api/issues/{id}/children
GET /api/issues/{id}/labels
GET /api/issues/{id}/attachments
GET /api/issues/{id}/active-task
GET /api/issues/{id}/task-runs
GET /api/issues/{id}/usage
GET /api/issues/{id}/pull-requests
```

When a Tura session transcript opens:

```text
GET /session/{sessionID}
GET /session/{sessionID}/message
GET /session/{sessionID}/todo
GET /permission
GET /question
GET /vcs/diff
```

## Realtime Event Ingestion

The current Tura SSE envelope is:

```json
{
  "directory": "./workspace",
  "payload": {
    "type": "session.updated",
    "properties": {}
  }
}
```

The Multica-compatible layer may also use WebSocket events. The reducer must
normalize both transports into one internal event shape:

```text
workspace_id / workspace_slug
directory
type
payload
received_at
event_id optional
```

Existing Tura event types:

```text
server.connected
server.instance.disposed
project.updated
session.created
session.updated
session.deleted
session.status
message.updated
message.removed
message.part.delta
message.part.updated
todo.updated
permission.asked / replied
question.asked / replied / rejected
vcs.branch.updated
```

Multica-compatible event types:

```text
issue:created
issue:updated
issue:deleted
issue_metadata:changed
comment:created
comment:updated
comment:deleted
comment:resolved
comment:unresolved
reaction:added
reaction:removed
issue_reaction:added
issue_reaction:removed
agent:created
agent:status
agent:archived
agent:restored
task:queued
task:dispatch
task:running
task:progress
task:message
task:completed
task:failed
task:cancelled
inbox:new
inbox:read
inbox:archived
inbox:batch-read
inbox:batch-archived
workspace:updated
workspace:deleted
member:added
member:updated
member:removed
subscriber:added
subscriber:removed
activity:created
skill:created
skill:updated
skill:deleted
chat:message
chat:done
chat:session_read
chat:session_deleted
chat:session_updated
project:created
project:updated
project:deleted
project_resource:created
project_resource:deleted
label:created
label:updated
label:deleted
issue_labels:changed
pin:created
pin:deleted
pin:reordered
invitation:created
invitation:accepted
invitation:declined
invitation:revoked
autopilot:created
autopilot:updated
autopilot:deleted
autopilot:run_start
autopilot:run_done
squad:created
squad:updated
squad:deleted
daemon:heartbeat
daemon:heartbeat_ack
daemon:register
daemon:task_available
github_installation:created
github_installation:deleted
pull_request:linked
pull_request:updated
pull_request:unlinked
```

Reducers must be idempotent. Key data by stable ids: workspace id, issue id,
comment id, task id, session id, message id, part id, project id, agent id,
runtime id, skill id, inbox id, invitation id, and pin `(item_type,item_id)`.

## Workbench And Navigation

The first screen inside a workspace is a dense workbench:

```text
left sidebar     workspace switcher, nav, pinned issues/projects, quick create
top bar          search, unread, current agent/runtime health, user menu
main             selected route: issues board/list/gantt or dashboard
right drawer     issue/task transcript, chat, details, filters, activity
modal layer      create/edit dialogs, pickers, command palette, previews
bottom/overlay   composer when in chat/session/issue comment context
```

Navigation must be keyboard-friendly:

- Cmd/Ctrl+K command palette for issues, projects, workspaces, navigation,
  actions, recent issues, and theme switching
- quick create issue/project
- issue detail deep links and comment anchors
- copy link commands
- sidebar pinned item reordering
- workspace switcher
- unread inbox jump

## Design System Rules

Adopt the Multica design language while fitting Tura's current UI:

- information-dense, restrained, work-focused layout
- neutral palette first; semantic colors only for status, priority, errors,
  success, warnings, and brand
- use design tokens for colors, not hardcoded grays/blues/reds
- text sizes stay in the compact tool range: `text-xs`, `text-sm`, `text-base`
- use `font-normal` and `font-medium`; avoid heavy bold styling
- hover must be lighter than selected/active state
- active state must remain visible while hovered
- Lucide icons for controls where available
- no marketing hero as the authenticated first screen
- no decorative gradients/orbs
- cards are for repeated items, dialogs, and framed tools; page sections should
  be full-width bands or unframed layouts
- every control must fit on mobile and desktop without overlapping text

### Conversation And Explorer Structure

The GUI keeps conversation-specific behavior outside `app.tsx`:

- `app/src/conversation/conversation-view.tsx` owns the transcript,
  execution summary, tool inspector, typewriter text, and composer.
- `app/src/conversation/message-tools.ts` owns message part parsing, command
  duration formatting, console output extraction, and patch diff line mapping.
- `app/src/i18n.ts` owns fixed UI copy. The current build uses a static
  language selection, but every visible label added to the GUI should go
  through this dictionary so a later language middleware can switch languages
  without rewriting components.
- `app.tsx` remains the workbench shell and data orchestration layer for
  sessions, workspaces, files, and gateway calls.

Conversation execution records must remain collapsed in the main transcript.
The transcript shows only a quiet gray command summary with total run time.
Clicking that summary opens a right-side sliding inspector panel containing the
full command, console output, and replacement diff. User messages render only
the text content inside the inverted message surface.

The rail is disclosure-based: the active workspace opens by default, clicking
the same workspace collapses it, and nested groups that reveal children should
follow the same click-to-toggle behavior.

Explorer rows use the same surface-list primitive as the rest of the app. The
columns are `Name`, `Git`, `Size`, and `Modified`; gateway file responses expose
`git_status`, `size_bytes`, and `modified_at` so the UI does not invent file
metadata locally.

Quality gates for GUI work are:

- `bun run format:check`
- `bun run typecheck`
- `bun run build`

## Domain Requirements

### Workspace

GUI must support:

- list, create, switch, update, leave, and delete workspace
- workspace name, slug, description, avatar/initial, issue prefix/counter
- workspace context for all agents
- repository allowlist
- GitHub installation connect/list/disconnect
- workspace access errors and no-access recovery
- zero-workspace flow into onboarding or create workspace

### Members And Invitations

GUI must support:

- member list with avatar, email, role, and action menu
- owner/admin/member role rules
- invite by email and role
- pending invitations with resend/revoke when gateway supports resend
- personal invitations list
- accept, decline, expired, and revoked states
- member profile page
- remove member and leave workspace flows

### Issues

Issue is the core collaborative object. GUI must support:

- list view with pagination, open/closed grouping, filter chips, search, sort
- board/Kanban view grouped by status with drag/drop status and manual position
- gantt/timeline view for `start_date`/`due_date`
- my issues scopes: assigned to me, created by me, my agents/squads involved
- quick create from sidebar or modal
- rich create/edit dialog with title, description, status, priority, assignee,
  project, parent, start date, due date, labels, attachments, acceptance
  criteria, and metadata when present
- batch select, batch status/priority/assignee/project/label updates, delete
- issue detail with header, description editor, properties, labels, project,
  parent/children, dependencies if gateway exposes them, progress ring for
  children, metadata dialog, pull request list, usage, task runs, active task
- comments with rich text, markdown rendering, one-level replies, resolve,
  edit, delete, reactions, attachments, mentions
- `@agent` mention creating an agent task
- subscribe/unsubscribe and subscriber list
- issue and comment emoji reactions
- attachment upload, inline image preview, downloadable file preview
- pin/unpin issue
- copy link and comment anchor navigation
- duplicate detection hints when gateway returns candidate duplicate issues

Issue statuses and priorities must be stable enums in the SDK. UI should not
hardcode only one locale; text labels should be localizable.

### Labels

GUI must support:

- label list/create/update/delete
- attach/detach labels on issues
- label filters and label chips
- label color/swatch if gateway exposes a color

### Projects

GUI must support:

- project list with search/filter/status/priority
- create/update/delete
- title, description, icon, status, priority, lead as member/agent/squad where
  gateway supports it
- project detail with issue list/board/gantt subset
- resources section with add/remove external URLs or repository references
- project pinning
- breadcrumb integration from issue detail

### Agents

GUI must support:

- agent list with status, avatar, provider, model, runtime, visibility,
  recent activity, 7/30-day run counts
- create agent from blank form or template catalog
- edit name, description, instructions, provider, model, runtime, custom env,
  custom args, MCP config, max concurrent tasks, visibility, thinking level,
  skills
- archive/restore
- cancel active tasks
- agent detail/profile with activity, task list, live peek, configuration tabs,
  skill bindings, runtime health, and task transcript access
- agent as a first-class actor in assignee, lead, creator, comments,
  subscribers, inbox, chat, and activity timeline

### Squads

GUI must support:

- squad list/detail
- create/update/archive/delete where gateway supports soft archive
- leader agent/member, members, roles, avatar, instructions
- member status and role update
- squad assignment in issues/autopilots when gateway supports it
- squad leader evaluation activity on issues

### Runtimes And Daemon

GUI must support:

- runtime list with provider logo, name, owner, status, runtime mode,
  visibility, last seen, CLI version, launched-by metadata, local/cloud marker
- runtime detail with bound agents, task activity, usage charts, hourly usage,
  model list, local skill list/import, update request/status/result
- local daemon card in desktop mode: status, logs/deep link when available,
  restart, auto-start, installed CLI detection
- runtime ping/diagnostic when gateway exposes it
- cloud runtime fleet panel for SaaS-compatible deployments: service health,
  nodes, create/delete/start/stop/reboot/status/exec
- clear offline/deleted state handling

### Skills

GUI must support:

- skill library list/search
- create/update/delete skill
- import from URL or local runtime skill list
- skill detail/editor with main content and attached files
- upsert/delete skill files
- bind/unbind skills to agents
- show provider-native injection targets in detail metadata when gateway
  exposes them, without writing files directly from GUI

### Autopilots

GUI must support:

- autopilot list/create/update/delete
- assignee can be agent or squad when gateway supports squads
- execution mode: create issue and run, or run-only where supported
- issue title/body templates, project binding, concurrency policy, timezone
- triggers: schedule/cron, webhook, API/manual
- trigger create/update/delete
- rotate webhook token and set signing secret
- manual trigger
- run history with statuses pending, issue_created, running, skipped,
  completed, failed
- delivery history, delivery detail, replay
- link runs to created issues/tasks
- surface skipped reasons and concurrency conflicts

### Chat

GUI must support:

- chat session list/create/rename/delete
- persistent user/agent messages
- choose agent/runtime where gateway allows
- send message with attachments
- pending task indicator
- active task transcript
- mark session read
- unread counts
- failure reasons and elapsed time
- context anchors linking chat to issue/project/task when present
- drafts per chat session

### Inbox And Notifications

GUI must support:

- inbox list with unread/read/archived states
- unread count badge
- mark item read/archive
- mark all read
- archive all
- archive all read
- archive completed
- notification type/severity display
- notification preferences page
- personal invitation notifications
- issue assignment/comment/mention/subscriber updates
- chat unread updates

### Dashboard And Usage

GUI must support:

- workspace usage daily chart
- usage by agent
- agent runtime summary
- runtime daily chart
- runtime detail charts: tasks, time, tokens, cost, hourly usage
- agent activity heatmap/trailing activity
- optional project filter where gateway supports it
- timezone-aware date buckets

### Search And Command Palette

GUI must support:

- global command palette
- search issues by title, number, description, comments
- search projects by title/description
- search workspaces
- navigation actions
- create issue/project actions
- recent issues
- filter chips in issues/projects/inbox
- no vector search assumption unless gateway adds it

### Auth And Onboarding

GUI must support:

- email code login
- Google OAuth
- logout
- PAT creation for CLI from settings
- CLI token issue flow
- signup restriction errors from gateway
- onboarding steps: welcome, workspace, runtime choice/connect, first agent,
  complete
- cloud waitlist and no-runtime path
- zero-workspace invite acceptance path
- starter content state when gateway exposes it
- user profile language/timezone handling
- public download/deep-link handoff for CLI setup

### Public Pages, Feedback, Analytics, And I18n

The authenticated workbench remains the primary product surface. If Tura keeps
Multica-compatible public pages, GUI must support them as thin routes:

- landing/home route when unauthenticated
- download page with platform-aware release links
- changelog page
- about page
- contact sales form
- feedback modal/form from authenticated UI
- privacy/consent links where configured
- pageview analytics hook that never blocks navigation
- client metadata headers: platform, version, OS
- i18n dictionaries for at least English and Chinese, including route labels,
  status labels, validation messages, onboarding, settings, issue fields,
  runtime states, and error messages

Analytics and public forms must call gateway endpoints. They must not introduce
a second telemetry or persistence client in the GUI.

### Settings

GUI settings must cover:

- My Account: profile name, avatar URL if gateway allows, profile description,
  language, timezone, theme
- API Tokens: list/create/revoke, one-time token reveal
- Appearance: light/dark/system
- Notifications: preferences
- Workspace General: name, slug, description, context, issue prefix
- Members and invitations
- Repositories/GitHub installation
- Agents, Runtimes, Skills, Autopilots management shortcuts
- Desktop-only daemon and update tabs when desktop shell exists

### Attachments And Editor

GUI must support:

- upload file through gateway
- associate attachments with issue descriptions, comments, and chat messages
- preview images and supported files
- attachment preview route
- delete attachment when allowed
- rich text/markdown editor with mentions, issue mentions, toolbar, file attach,
  readonly renderer, and safe link handling

### GitHub And Pull Requests

GUI must support:

- list workspace GitHub installations
- connect/disconnect for admins
- pull request links on issues
- PR status/stats/conflict/CI state when gateway exposes it
- realtime PR linked/updated/unlinked events

## Execution Transcript Integration

Agent tasks, issue runs, and chat runs must be able to open a Tura transcript:

- map task id to Tura session id when gateway provides `session_id`
- show task messages from `/api/tasks/{taskId}/messages` for Multica-style
  daemon output
- show Tura message parts from `/session/{sessionID}/message` for native
  session output
- show todos, diffs, files, PTY, permissions, and questions in side panels
- keep issue/comment/chat context visible while inspecting transcript

Transcript cells:

- `user`: prompt text, attachments, referenced files
- `assistant`: streamed/final assistant text
- `tool`: tool call status, command output, runtime metadata
- `diff`: VCS diff or patch summary
- `todo`: task updates
- `permission`: pending/approved/denied permission records
- `question`: user question and reply state
- `task`: queued/running/progress/completed/failed/cancelled
- `error`: gateway/runtime/provider errors

## Optimistic Updates

Allowed optimistic updates:

- board drag status/position
- pin reorder
- mark inbox read/archive
- local reaction toggle
- local draft updates
- comment composer append before server confirmation, marked pending

Optimistic updates must reconcile on realtime or query refresh. Failed writes
must roll back and show inline error near the affected control.

## Reconnect And Offline Behavior

The GUI must survive gateway restarts and network drops:

- show disconnected state when health/realtime fails
- retry with backoff
- rehydrate active workspace, issue, chat, and session after reconnect
- keep unsent drafts locally
- do not mark agent work failed until gateway emits failure or a query confirms
  the failed state
- dedupe events after reconnect by id when available, otherwise by object id and
  updated timestamp

## SDK Requirements

`sdk/gateway` must expose typed clients for:

- core Tura APIs
- auth/user/workspace/member/invitation/token APIs
- issues/comments/labels/attachments/reactions/subscribers/metadata/PRs
- projects/resources
- agents/templates/agent skills/tasks
- squads
- runtimes/updates/models/local skills/cloud runtime
- skills/files/import
- autopilots/triggers/runs/deliveries/webhooks
- chat sessions/messages
- inbox/preferences
- dashboard/usage
- daemon status where user-visible
- contact-sales, feedback, config, and analytics helpers
- realtime event source/WebSocket source

The SDK should keep one TypeScript shape per domain and should not add duplicate
session/task-management field names.

## Testing Strategy

Initial tests:

- gateway SDK unit tests with mocked fetch responses for every domain
- event reducer tests for all Tura and Multica-compatible event types
- route resolver tests for global/workspace paths
- issue filter/sort/group tests
- board drag/drop optimistic update tests
- editor mention serialization tests
- inbox reducer/action tests
- auth/onboarding branch tests

Component tests:

- issues board/list/gantt/detail
- create/edit issue modal
- comments, reactions, attachments
- project detail/resources
- agent create/detail/config
- runtime detail/usage/update/local skills
- skill editor/files/import
- autopilot detail/triggers/runs/deliveries
- chat composer/timeline
- inbox bulk actions
- settings tabs
- command palette

E2E tests against mocked gateway:

- login and onboarding
- workspace create/switch
- issue create, board drag, comment, mention agent, run transcript
- project create and issue assignment
- agent create from template and skill binding
- runtime list and update request
- skill import/edit
- autopilot create/trigger/run history
- chat send/read
- inbox read/archive
- reconnect and event replay

Live smoke tests can later start `crates/gateway` and exercise a minimal
session plus issue/task flow.

## Implementation Phases

1. Expand `sdk/gateway` with Multica-compatible DTOs, request helpers, and
   realtime normalization.
2. Introduce auth, workspace routing, layout shell, and command palette.
3. Implement workspace, members, invitations, settings, tokens, onboarding.
4. Implement issues board/list/gantt/detail, comments, labels, attachments,
   reactions, subscribers, pins, search, and task-run transcript linking.
5. Implement projects/resources and dashboard usage.
6. Implement agents, agent templates, agent skills, task controls, squads.
7. Implement runtimes, daemon/local runtime views, cloud runtime, model/local
   skill requests, usage/activity charts.
8. Implement skills library/editor/import/files.
9. Implement autopilots, triggers, webhook security, runs, deliveries, replay.
10. Implement chat sessions/messages/pending task/read state.
11. Implement inbox, notification preferences, realtime reconciliation, offline
    recovery, and full E2E coverage.

## Open Gateway Dependencies

The GUI can continue with current Tura APIs for the coding workbench. Full
Multica-compatible parity requires gateway contracts for:

- auth/user/workspace/member/invitation/token persistence
- issue/comment/project/agent/runtime/skill/autopilot/chat/inbox storage
- workspace-scoped realtime events
- task queue and daemon/runtime APIs
- file upload/attachment storage
- dashboard rollups and timezone-aware usage buckets
- GitHub installation/webhook/PR projection
- squads
- notification preferences
- event ids and replay/`Last-Event-ID`
- shared error envelope for all handlers

Until those contracts exist, GUI code should keep route shells behind feature
gates or mocked SDK adapters, but the architecture must not choose a path that
would conflict with the final gateway-owned implementation.
