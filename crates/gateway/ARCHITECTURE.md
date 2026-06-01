# Gateway Crate Architecture

`crates/gateway` is the middleware between frontend clients and Tura backend
crates. It provides the HTTP/SSE/WebSocket API surface consumed by the GUI,
translates UI payloads into runtime/router/provider calls, persists UI-facing
state, and streams backend events back to clients.

Gateway must not run the agent loop or own low-level command routing. Runtime
work goes through `crates/runtime`; provider calls go through `crates/provider`;
tool execution and shell effects go through their existing Tura owners; CLI
forwarding and managed process lifecycle go through `crates/router` or a narrow
gateway adapter around it.

This document also defines how gateway should grow to recreate Multica's full
product surface while remaining compatible with Tura's current session,
runtime, provider, PTY, file, command, and event architecture.

## Compatibility Goal

Gateway must support two overlapping domains:

1. Tura coding workbench APIs already present in this crate:
   sessions, messages, provider/model auth, files, find, VCS, PTY, MCP,
   commands, services, permissions, questions, todos, skills/plugins, config,
   path, formatter, logs, and SSE events.
2. Multica-compatible collaboration APIs:
   workspaces, users, members, invitations, issues, comments, labels,
   attachments, reactions, subscribers, projects, squads, agents, runtimes,
   daemon, skills, autopilots, chat, inbox, notification preferences, dashboard
   usage, GitHub integration, webhooks, PATs, onboarding, feedback, contact
   sales, analytics events, i18n/user locale state, public config, and CLI
   support.

The important architectural rule is that Multica-compatible product objects do
not replace Tura sessions. They orchestrate them.

```text
issue assignment / @agent comment / chat message / autopilot run
  -> gateway creates an agent task record
  -> gateway selects agent + runtime + workspace + repository/worktree
  -> gateway starts or resumes a Tura session/turn through runtime_client
  -> runtime/router/provider/tools execute work
  -> gateway stores task/message/activity/usage projections
  -> gateway emits realtime events for GUI, daemon, and CLI clients
```

## High-Level Layout

Current layout:

```text
crates/gateway/
  Cargo.toml
  ARCHITECTURE.md
  src/
    api/
      global.rs
      project.rs
      session.rs
      file.rs
      provider.rs
      pty.rs
      mcp.rs
      product.rs
      misc.rs
      types.rs

    session/
      manager.rs
      store.rs
      config.rs
      process_cleanup.rs
      process_snapshot.rs
      docker_snapshot.rs

    web/
    mock/
    bin/
      gateway.rs
      tura.rs

    channel.rs
    handler.rs
    media.rs
    runtime.rs
    simple_runtime.rs
    types.rs
```

Product domains such as collaboration issues, workspaces, daemon APIs, and
separate transport/domain/client directories are architectural growth areas,
not directories that currently exist in this crate.

## Owns

Gateway owns:

- frontend-facing HTTP API routes and DTOs
- request validation, auth, workspace scoping, role checks, and response shaping
- compatibility mapping between Multica-style APIs and Tura session/runtime APIs
- UI-facing persistence for workspaces, users, members, invitations, issues,
  comments, labels, attachments, reactions, subscribers, projects, squads,
  agents, skills, autopilots, chat, inbox, notification preferences, pins,
  tasks, task messages, usage rollups, GitHub projections, webhooks, PATs, and
  onboarding state
- Tura session/message/todo/event projections
- event streaming, event replay, and realtime room fanout
- token usage projection and summaries
- permission/question request and response forwarding
- file/project/helper APIs for UI inspection
- provider/model config projection and forwarding
- PTY/process/service adapters for UI
- workspace config load/merge/save
- session startup request assembly
- task queue and task lifecycle policy
- daemon/runtimes registration, heartbeat, task claim, task result ingest
- background schedulers for autopilot, runtime sweeps, orphan recovery, usage
  rollups, webhook delivery retry, and stale notification maintenance
- mock stores for tests
- runtime client calls to `crates/runtime`
- router client calls to `crates/router`

## Does Not Own

Gateway does not own:

- the runtime agent loop
- prompt/turn execution internals
- provider request construction
- LLM API calls
- low-level tool execution
- shell sandboxing
- patch application internals
- command registry internals
- CLI forwarding rules
- router-managed lifecycle internals
- GUI rendering

Those belong to `crates/runtime`, `crates/provider`, `crates/tools`,
`crates/router`, and `apps/gui`.

## Persistence Model

To recreate Multica's functionality, gateway needs durable product state.
The persistence backend may be SQLite for local/single-user Tura deployments
and PostgreSQL for hosted/multi-user deployments, but the domain model must not
depend on the storage engine.

Required domains and fields:

```text
user
  id, email, name, avatar_url, profile_description, language, timezone,
  onboarded_at, onboarding_state, starter_content_state, created_at, updated_at

verification_code
  email, code_hash, expires_at, attempts, consumed_at

personal_access_token
  id, user_id, name, token_hash, token_prefix, expires_at, last_used_at, revoked_at

workspace
  id, name, slug, description, context, settings, repos, issue_prefix,
  issue_counter, avatar, created_at, updated_at

member
  id, workspace_id, user_id, role(owner/admin/member), created_at, updated_at

workspace_invitation
  id, workspace_id, inviter_id, invitee_email, role, status,
  expires_at, accepted_at, declined_at, revoked_at

agent
  id, workspace_id, creator_id, name, description, avatar_url, provider,
  model, runtime_id, instructions, custom_env, custom_args, mcp_config,
  max_concurrent_tasks, visibility(workspace/private), status,
  thinking_level, archived_at, created_at, updated_at

agent_runtime
  id, workspace_id, owner_id, daemon_id, provider, name, runtime_mode(local/cloud),
  visibility, status(online/offline/error), metadata, timezone, last_seen_at,
  created_at, updated_at

daemon_token
  id, workspace_id, daemon_id, token_hash, token_prefix, revoked_at, created_at

skill
  id, workspace_id, name, description, content, source_url, created_by,
  created_at, updated_at

skill_file
  id, skill_id, path, content, created_at, updated_at

agent_skill
  agent_id, skill_id

issue
  id, workspace_id, number, title, description, status, priority, position,
  assignee_type(member/agent/squad/null), assignee_id, creator_type,
  creator_id, parent_issue_id, project_id, start_date, due_date,
  acceptance_criteria, origin_type, origin_id, metadata, first_executed_at,
  created_at, updated_at, deleted_at

issue_label / issue_to_label
  labels and issue-label relation

issue_dependency
  issue_id, related_issue_id, relation(blocks/blocked_by/related)

issue_subscriber
  issue_id, subscriber_type(member/agent), subscriber_id, reason

comment
  id, workspace_id, issue_id, author_type(member/agent/system), author_id,
  parent_id, body, type(comment/status_change/progress_update/system),
  resolved_at, created_at, updated_at, deleted_at

issue_reaction / comment_reaction
  target id, actor_type, actor_id, emoji, created_at

attachment
  id, workspace_id, uploader_type, uploader_id, issue_id, comment_id,
  chat_session_id, chat_message_id, filename, content_type, size, storage_key,
  url, created_at

project
  id, workspace_id, title, description, icon, status, priority,
  lead_type(member/agent/squad/null), lead_id, created_at, updated_at

project_resource
  id, project_id, label, url, kind, metadata

pinned_item
  id, workspace_id, user_id, item_type(issue/project), item_id, position

squad
  id, workspace_id, name, description, avatar_url, leader_id, instructions,
  archived_at, created_by, created_at, updated_at

squad_member
  squad_id, member_type(member/agent), member_id, role, created_at

autopilot
  id, workspace_id, name, description, assignee_type(agent/squad),
  assignee_id, project_id, execution_mode, issue_title_template,
  issue_body_template, concurrency_policy, enabled, created_at, updated_at

autopilot_trigger
  id, autopilot_id, kind(schedule/webhook/api), cron_expression, timezone,
  next_run_at, webhook_token_hash, signing_secret_hash, enabled, created_at

autopilot_run
  id, autopilot_id, trigger_id, status(pending/issue_created/running/skipped/completed/failed),
  issue_id, task_id, skipped_reason, error, started_at, completed_at

webhook_delivery
  id, autopilot_id, trigger_id, request_headers, request_body, status,
  response, replay_of, created_at

agent_task_queue
  id, workspace_id, issue_id, chat_session_id, autopilot_run_id,
  trigger_comment_id, assignee_type(agent/squad), assignee_id, runtime_id,
  status(queued/dispatched/running/completed/failed/cancelled), lease_until,
  retry_count, context, result, failure_reason, session_id, work_dir,
  force_fresh_session, is_leader, created_at, started_at, completed_at

task_message
  id, task_id, seq, role/type/tool, input, output, metadata, created_at

task_usage
  id, task_id, runtime_id, agent_id, issue_id, chat_session_id, model,
  input_tokens, output_tokens, reasoning_tokens, cache_read_tokens,
  cache_write_tokens, cost, elapsed_ms, created_at, updated_at

chat_session
  id, workspace_id, user_id, agent_id, runtime_id, title, unread_since,
  session_id, work_dir, created_at, updated_at, deleted_at

chat_message
  id, chat_session_id, role(user/assistant/system), body, failure_reason,
  elapsed_ms, created_at

inbox_item
  id, workspace_id, recipient_type(member/agent), recipient_id, type, severity,
  actor_type, actor_id, issue_id, comment_id, chat_session_id, details,
  read_at, archived_at, created_at

notification_preference
  user_id, workspace_id, channel, event_type, enabled

activity_log
  id, workspace_id, actor_type(member/agent/system), actor_id, action,
  issue_id, project_id, agent_id, task_id, details, created_at

github_installation / pull_request
  workspace installation state and PR links/status projection
```

## API Surface

Gateway must keep current Tura routes stable and add Multica-compatible routes.
The GUI architecture document contains the full route map. Gateway route
modules should group them by domain and share common middleware:

- auth extraction
- PAT cache and invalidation
- daemon token auth
- workspace membership/role checks
- request id/client metadata
- CORS/CSP
- rate limits for auth, webhook, contact sales, upload, and expensive search
- JSON error envelope

All handlers should return:

```json
{
  "error": {
    "code": "invalid_request",
    "message": "human readable message",
    "details": {}
  }
}
```

for failures. Existing routes can keep legacy behavior until migration, but new
Multica-compatible routes should use the shared envelope.

## Session Plan And Task Management

Gateway owns session scanning, hydration, persistence, and UI-facing
task-management response shaping for the plan surface. GUI and TUI clients must
not scan session directories directly.

Benchmark-specific fixtures and evaluator contracts belong under e2e test
directories only. Gateway session types, task-management patching, API
responses, and persistence must stay benchmark-agnostic.

Current routes:

```text
GET    /session?directory=<workspace>&includeChildren=true
POST   /session
GET    /session/{sessionID}
PATCH  /session/{sessionID}
PATCH  /session/{sessionID}/task-management
GET    /session/status
GET    /session/{sessionID}/todo
GET    /session/{sessionID}/message
POST   /session/{sessionID}/message
POST   /session/{sessionID}/prompt_async
```

Every session response should use one session field set:

```text
task_management
plan_summary
session_display_name
```

Do not add duplicate names for the same session state. Clients that integrate
an external protocol should translate at that protocol boundary.

Display name resolution is:

1. non-empty `plan_summary`
2. first non-empty task `task_summary`
3. session `name`
4. `New Session`

The runtime/session status remains `idle | busy | error`. Task status is
separate and lives in `task_management`:

```text
todo
doing
question
done
archived
```

Start conditions are:

```text
session_idle
user_action
scheduled_task
polling_task
```

Single-task `task_management` is an object. Object patches apply to the active
single task and may set the task `nonce_id`. Multi-task updates that need
nonce-specific matching use `task_management.tasks[]`; array entries match by
`nonce_id` and create missing tasks using supplied fields plus defaults.

Current patch validation behavior is intentionally compatibility-preserving:
invalid task-management patches are logged and ignored, prior state is kept,
and the session response is returned. GUI/TUI behavior must reconcile by
refreshing gateway state.

User messages appended through gateway message APIs are also appended to the
session-management log so runtime context and hydration can keep follow-up
constraints.

Pending follow-up controls are currently projected from `task_management` and
`/session/{sessionID}/todo`. The current source of truth is the enriched
session response plus todo projection.

The file-backed scheduler for `scheduled_task`, `polling_task`, and
`session_idle` belongs in gateway when execution ownership is implemented. It
must not start archived/done tasks and must not start an already busy session.

## Auth And Identity

Gateway must support:

- email verification code login
- Google OAuth login
- logout
- current user read/update
- profile description, language, and timezone
- onboarding state patches and completion
- cloud waitlist/runtime-choice fields when enabled
- PAT issue/list/revoke
- CLI token issue
- signup restrictions through `ALLOW_SIGNUP`, `ALLOWED_EMAILS`, and
  `ALLOWED_EMAIL_DOMAINS`
- workspace invitations for users who may not yet have a workspace
- user language and timezone persistence for i18n and usage bucketing
- public config values needed by unauthenticated clients

Identity must support polymorphic actors:

```text
actor_type = member | agent | squad | system
actor_id   = nullable for system
```

This is required so agents can create issues, comment, be assigned, lead
projects/squads, subscribe, appear in inbox/activity, and trigger tasks.

## Public Config, Feedback, Contact Sales, Analytics, And I18n

Gateway must support small public/product-support endpoints without weakening
the authenticated workspace boundary:

- `GET /api/config` for public client configuration such as signup availability,
  OAuth enablement, deployment mode, and public URLs
- `POST /api/contact-sales` with rate limiting, validation, storage, and email
  or log fallback
- `POST /api/feedback` for authenticated feedback, including client metadata
  and optional workspace context
- pageview/product analytics adapter with a no-op implementation for local and
  self-hosted deployments
- client metadata extraction from headers: `X-Client-Platform`,
  `X-Client-Version`, and `X-Client-OS`
- user language/timezone fields and validation
- localized error codes/messages where the API intentionally returns user-facing
  text; otherwise return stable error codes for GUI localization

These endpoints must never leak private workspace data to unauthenticated
callers.

## Workspace And Roles

Workspace is the product isolation boundary.

Gateway must enforce:

- member access for reading workspace-scoped resources
- admin/owner access for workspace update, member invite, member role changes,
  repository/GitHub management, and destructive configuration
- owner-only access for workspace deletion
- self-leave for non-owner members
- workspace slug uniqueness and reserved slug checks
- issue prefix/counter atomicity
- workspace repository allowlist for daemon/runtime checkout
- workspace context injection into agent task startup

Every workspace-scoped response must be filtered by workspace id. The GUI must
never be trusted to filter cross-workspace data client-side.

## Issues And Collaboration

Gateway owns issue business rules:

- list with filters: status, statuses, priority/priorities, assignee ids,
  assignee actor filters, include no assignee, creator filters, project ids,
  include no project, label ids, metadata containment, involved user, open-only,
  scheduled-only, pagination, sorting
- grouped issues for board/list by assignee/status/project where needed
- search over issue title, description, and comments with snippets
- quick create and rich create
- update status, priority, assignee, project, parent, dates, position,
  title/description, metadata, attachments
- batch update/delete
- child progress
- timeline combining activity and comments
- subscribers auto-add for creator, assignee, commenter, mentioned actors, and
  manual subscriptions
- comments with one-level replies, resolve/unresolve, edit/delete
- issue and comment reactions
- label attach/detach
- attachments list/content/delete
- active task, task runs, task messages, usage
- rerun issue
- cancel task
- pull request list
- metadata key set/delete

When an issue is assigned to an agent or squad, gateway must enqueue the
appropriate task unless the operation is explicitly metadata-only or disabled by
policy. `@agent` mentions in comments must enqueue comment-triggered tasks.

## Task Lifecycle

Task states:

```text
queued -> dispatched -> running -> completed
queued -> dispatched -> running -> failed
queued|dispatched|running -> cancelled
queued|dispatched -> failed on lease/orphan timeout
```

Rules:

- one task row represents one agent run
- task context contains issue/chat/autopilot/comment trigger data
- task assignment may target an agent or a squad; squad tasks can spawn member
  tasks and leader evaluation where supported
- task queue enforces per-agent max concurrent tasks
- daemon/runtime claims tasks by runtime id
- leases protect against abandoned claims
- orphan recovery marks stale dispatched/running tasks failed or requeues when
  policy allows
- task messages append with monotonic sequence
- task usage is reported incrementally and rolled up
- cancellation is user-visible and forwarded to runtime/session cancellation
- task completion stores result, session id, and work dir for resumption

## Session Resumption

Gateway maps Multica-style tasks to Tura sessions.

For the same `(workspace, agent, issue)` or `(workspace, agent, chat_session)`,
gateway should reuse the latest successful `session_id` and `work_dir` unless:

- user requests force fresh session
- agent/runtime/provider changed incompatibly
- work dir is unavailable
- previous session is invalid
- task policy forbids reuse

When resuming, gateway injects current issue/chat context as a new turn rather
than rebuilding history in the GUI.

## Runtime Client

`runtime_client/` is the gateway path into runtime execution. It should support:

- `start_session`
- `inject_turn`
- `cancel_turn`
- `read_session`
- `read_messages`
- `read_todos`
- `read_diff`

Gateway treats runtime as a boundary even if the call is in-process. Product
handlers enqueue or update task/session records first, then call runtime client
through lifecycle services.

## Router Client

Gateway may expose UI-facing process, PTY, command status, managed service
status, or project startup APIs, but lifecycle work is delegated to
`crates/router` or narrow helpers.

The router client should support:

- resolve command metadata
- request managed process startup
- read command/process/service status
- forward stop/cancel requests for routed CLI processes
- proxy command events into gateway app events
- PTY create/list/update/delete/connect

The gateway↔router HTTP route (`POST /run_agent`) is the **external** boundary
(frontend → gateway → router). For **internal** runtime-initiated child
sub-session dispatch, the runtime worker invokes `tura_router run-agent`
directly as a subprocess (CLI stdin/stdout JSON); it does not go back through
the gateway or over HTTP. Router + gateway are a single process; all runtimes
are subprocesses spawned by that process.

## Daemon And Runtime APIs

Multica-compatible daemon behavior:

- daemon registers one or more runtimes for detected coding CLIs
- daemon heartbeats every interval with runtime status, metadata, version,
  timezone, and capabilities
- daemon can keep a WebSocket open for task wakeups
- daemon claims pending tasks by runtime id
- daemon starts task, reports progress/messages/usage, completes/fails task
- daemon pins session id and work dir after successful execution
- daemon supports runtime update request/result
- daemon supports model list request/result
- daemon supports local skill list/import request/result
- daemon can recover orphans for a runtime
- daemon can query workspace repository allowlist and GC checks

Gateway must validate daemon tokens, PAT fallback, runtime ownership, workspace
scope, and task ownership on every daemon route.

## Agents

Gateway agent domain must support:

- list/create/get/update
- create from template with skill import/binding in one transaction
- archive/restore
- cancel active tasks
- list agent tasks
- list/set agent skills
- status derived from runtime heartbeat and active task state
- visibility: workspace/private
- provider/model/runtime/instructions/custom env/custom args/MCP config
- max concurrent tasks
- thinking level and model options where supported
- activity and run-count summary endpoints

Agent custom env must be filtered so it cannot override gateway/daemon auth
variables or other protected runtime variables.

## Squads

Gateway must support squads as a first-class assignee type:

- list/create/get/update/delete or archive
- leader, members, member roles, member status
- instructions/avatar
- issue assignment and autopilot assignment when enabled
- leader evaluation activity on issues
- task expansion and coordination policy owned by gateway/task service

## Runtimes And Cloud Runtime

Runtime domain must support:

- list/update/delete
- owner filter
- runtime mode local/cloud
- visibility
- status and last seen
- usage by day, by agent, by hour
- task activity
- update initiation and polling
- model list initiation and polling
- local skill list/import initiation and polling
- local daemon metadata projection
- cloud runtime fleet proxy: health, readiness, service info, nodes,
  create/delete/start/stop/reboot/status/exec

Long-offline runtimes can be garbage-collected only when no active agents/tasks
depend on them.

## Skills

Gateway owns structured skill storage:

- skill main markdown content
- extra skill files by path
- import from URL/GitHub/marketplace/local runtime
- list/get/create/update/delete
- upsert/delete files
- bind to agents
- inject into provider-native locations during task startup

Gateway, not GUI, writes skill files into task work dirs.

## Autopilots

Gateway owns automation:

- autopilot list/create/get/update/delete
- enabled/disabled state
- assignee agent or squad
- project binding
- execution mode
- issue title/body templates
- concurrency policy
- triggers: schedule/cron, webhook, API/manual
- timezone-aware next run calculation
- webhook token and signing secret
- manual trigger
- run rows and statuses
- delivery rows and replay
- skipped run reasons

Scheduler:

- wakes at a fixed interval
- finds due schedule triggers
- creates run rows transactionally
- applies concurrency policy
- creates issue and/or task
- emits autopilot run events
- updates next run

Webhook ingress:

- validates trigger token
- validates signing secret when configured
- rate limits by token and IP
- stores delivery
- creates/reuses run
- supports replay from stored delivery

## Chat

Gateway chat domain must support:

- create/list/get/update/delete sessions
- send/list messages
- pending task lookup
- mark read
- pending chat tasks list
- attachments
- failure reason and elapsed time
- session id/work dir resumption
- chat task queue integration
- unread count events

Chat is not tied to an issue, but may still create agent tasks and Tura
sessions.

## Inbox And Notifications

Gateway owns:

- inbox item creation from issue/comment/task/member/invitation/chat/autopilot
  events
- unread count
- list with filters/pagination
- mark read/archive
- mark all read
- archive all
- archive all read
- archive completed
- notification preferences
- directed realtime events for recipient users/members

Inbox writes should happen in the same logical transaction as the domain event
when possible.

## Dashboard And Usage

Gateway must provide:

- workspace usage daily
- usage by agent
- agent runtime summary
- runtime daily
- runtime usage by day/by agent/by hour
- runtime task activity
- issue usage
- agent 30-day activity
- agent run counts
- project-scoped usage where requested
- timezone-aware buckets

Usage rollups should be derived from task usage and updated incrementally or by
background workers. Raw usage records remain available for audit.

## Attachments And Storage

Gateway owns upload and serving:

- `POST /api/upload-file`
- attachment records for issue/comment/chat
- local storage fallback
- S3/CloudFront or equivalent signed URL support
- content type sniffing and safe headers
- delete authorization
- preview/content routes

The GUI never writes files directly to storage.

## GitHub Integration

Gateway must support:

- workspace GitHub installation list
- connect callback
- disconnect
- GitHub webhook HMAC verification
- issue/PR linking
- PR update projection: state, CI, conflict, stats when available
- PR events: linked, updated, unlinked
- member-visible read endpoints and admin-only management endpoints

Repository allowlists used by daemon/runtime must remain workspace-owned.

## CLI Compatibility

Gateway must expose enough API for a Multica-compatible CLI without adding a
separate business layer:

- auth status, login token issue, logout, config show/set through gateway-owned
  config endpoints
- workspace list/get/watch/unwatch where watch maps to repository/workspace
  runtime visibility policy
- issue list/get/create/update/assign/status, comments list/add/delete,
  task runs, and task run messages
- agent list/get/create/update/archive
- skill list/get/create/update/delete/import/files upsert
- autopilot list/get/create/update/trigger and trigger management
- project list/get/create/update
- repo list/add/update/delete through workspace repository APIs
- runtime list/usage/activity/update
- daemon start/stop/status/logs are local CLI responsibilities, but daemon
  register/heartbeat/claim/report APIs are gateway-owned

The CLI must use the same auth, workspace scoping, validation, task lifecycle,
and event contracts as the GUI.

## Realtime Transport

Gateway currently has SSE-style events for Tura. Multica-compatible features
need workspace rooms and directed user events. Gateway may support both SSE and
WebSocket with a shared event bus.

Event envelope:

```json
{
  "id": "optional-event-id",
  "workspace_id": "optional-workspace-id",
  "workspace_slug": "optional-slug",
  "directory": "optional-directory",
  "recipient_user_id": "optional-directed-user",
  "type": "issue:updated",
  "payload": {},
  "created_at": "2026-05-24T00:00:00Z"
}
```

Realtime requirements:

- workspace-scoped room fanout
- personal directed events for inbox/invitations
- daemon wakeup events
- heartbeat/ping/pong
- slow-client eviction
- event replay buffer and `Last-Event-ID`/cursor support
- event idempotency
- metrics: active connections, send failures, slow evictions, per-event counts

## Event Types

Gateway must emit both current Tura and Multica-compatible events.

Tura events:

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
permission.asked
permission.replied
question.asked
question.replied
question.rejected
session.diff
vcs.branch.updated
```

Multica-compatible events:

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

## Core Flows

### Start Tura Session

```text
apps/gui
  -> gateway /session
  -> gateway loads workspace/session config
  -> gateway asks router for command/service setup when needed
  -> runtime_client/start_session
  -> crates/runtime
  -> gateway emits session/message events
  -> apps/gui updates transcript
```

### Create Issue Assigned To Agent

```text
apps/gui
  -> POST /api/issues
  -> gateway validates workspace/member and issue payload
  -> gateway creates issue, subscribers, activity, inbox rows
  -> if assignee is agent/squad, gateway enqueues task
  -> gateway emits issue:created and task:queued
  -> daemon/runtime claims task or scheduler starts runtime path
```

### Comment Mentions Agent

```text
apps/gui
  -> POST /api/issues/{id}/comments
  -> gateway stores comment and attachments
  -> gateway parses mentions
  -> gateway subscribes mentioned actors
  -> gateway enqueues one task per mentioned agent/squad policy
  -> gateway emits comment:created, subscriber:added, task:queued, inbox:new
```

### Daemon Executes Task

```text
daemon
  -> POST /api/daemon/runtimes/{runtimeId}/tasks/claim
  -> gateway leases queued task
  -> daemon prepares work dir and injected context/skills
  -> daemon starts CLI/provider-native agent process
  -> daemon reports start/progress/messages/usage
  -> gateway stores task messages and usage
  -> daemon reports complete/fail and session/workdir
  -> gateway updates issue/chat/autopilot state and emits events
```

### Chat Message

```text
apps/gui
  -> POST /api/chat/sessions/{sessionId}/messages
  -> gateway stores user message
  -> gateway enqueues chat task for selected agent/runtime
  -> gateway emits chat:message and task:queued
  -> task execution writes assistant messages and chat:done
```

### Autopilot Schedule

```text
scheduler
  -> finds due trigger in autopilot_trigger
  -> creates autopilot_run
  -> applies concurrency policy
  -> creates issue and/or queues task
  -> stores delivery/run details
  -> emits autopilot:run_start, issue:created, task:queued
  -> updates run done/skipped/failed
```

### Permission

```text
crates/tools/router/runtime
  -> permission requested event
  -> gateway stores pending request
  -> UI reads /permission or receives permission.asked
  -> UI replies through gateway
  -> gateway forwards decision
  -> caller continues or fails
```

## Session Startup Context

When gateway starts an agent task, it assembles context from:

- workspace context
- agent instructions
- issue title/description/properties/comments/attachments/acceptance criteria
- project details and resources
- triggering comment or chat message
- relevant skill content and files
- repository/worktree allowlist
- custom env and args after protected-variable filtering
- MCP config
- resumption session id/work dir if allowed

Gateway should keep this assembly auditable and testable. The runtime sees an
execution-ready request; the GUI never assembles prompts for agent work.

## Background Workers

Gateway needs these workers:

- runtime sweeper: mark offline runtimes based on heartbeat timeout
- orphan task sweeper: recover stale dispatched/running tasks
- autopilot scheduler: process due schedule triggers
- webhook retry/replay maintenance
- usage rollup worker: daily/hourly aggregation and invalidation
- notification cleanup/archive maintenance if policy requires it
- long-offline runtime GC
- stale verification/invitation cleanup

Workers must be idempotent and safe in multi-node deployments. Use DB locks,
leases, or equivalent coordination for hosted mode.

## Security

Requirements:

- all workspace-scoped APIs enforce membership
- role checks for admin/owner operations
- PATs and daemon tokens stored only as hashes
- one-time token reveal for newly-created PATs
- rate limits for auth, verification, webhook, contact sales, and expensive
  public endpoints
- webhook HMAC verification where signing secret exists
- attachment content type and safe serving headers
- path traversal protection for local storage and file APIs
- protected env var filtering for agent custom env
- repository allowlist enforcement before daemon checkout
- CORS configured from explicit origins
- no secrets in realtime payloads
- redaction helper for logs and errors

## Testing Strategy

Unit tests:

- DTO serialization compatibility
- workspace role middleware
- auth/PAT/daemon token validation
- issue filters/grouping/search params
- task lifecycle transitions and lease recovery
- subscriber/inbox/activity side effects
- comment mention parsing and task enqueue
- agent env filtering
- skill import and file path validation
- autopilot cron/timezone calculation and concurrency policy
- webhook signature/rate limit/replay
- chat message/task integration
- usage rollups and timezone buckets
- realtime event normalization and replay

Integration tests:

- create workspace -> invite -> accept
- create issue assigned to agent -> task queued -> daemon claim -> complete
- issue comment @agent -> task queued
- chat send -> task queued -> assistant message
- autopilot trigger -> issue/task/run history
- runtime heartbeat -> status update -> offline sweep
- skill bind -> task startup context contains skill
- attachment upload -> bind -> preview/content
- GitHub webhook -> PR linked/updated event

Checks:

```text
cargo fmt -p gateway
cargo check -p gateway
cargo test -p gateway
```

## Implementation Phases

1. Stabilize shared error envelope, auth middleware, workspace scoping, and
   persistence abstraction.
2. Add user/workspace/member/invitation/PAT/onboarding APIs.
3. Add issue/comment/label/attachment/reaction/subscriber/activity/pin APIs.
4. Add task queue, daemon/runtime claim/report APIs, and Tura session
   resumption mapping.
5. Add agent/template/skill/squad APIs.
6. Add project/resource APIs and dashboard usage rollups.
7. Add runtime detail/update/model/local-skill/cloud runtime APIs.
8. Add autopilot triggers/runs/deliveries/webhook scheduler.
9. Add chat and inbox/notification preference APIs.
10. Add GitHub integration and PR projection.
11. Add realtime replay, event ids, WebSocket rooms, directed user events, and
    full mocked/live integration tests.

## Current Compatibility Notes

The existing gateway already has a strong base for:

- session creation and prompt submission
- SSE event streaming
- provider/model projection and auth routes
- file and VCS inspection
- PTY and service status
- permissions/questions
- MCP
- commands
- skills/plugins projection

The missing Multica-compatible areas are durable collaboration state, auth,
workspace membership, task queue/daemon APIs, dashboards, chat, inbox,
autopilot, GitHub, and full workspace realtime. These must be added behind
gateway-owned contracts before GUI code treats them as production features.
