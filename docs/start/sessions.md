# Sessions

## What a session is

A session is the durable conversation and work container for one workspace. It
keeps the user-visible thread, selected runtime settings, task-management state,
token usage, and enough internal state for the runtime to continue or report the
work correctly. If the app closes, the work should not suddenly become folklore.

A runtime is not the session. It is one model/provider execution inside a
session. A single session can contain many runtime calls across one prompt, tool
loop, retry, child delegation, or later follow-up prompt.

| Layer | Owns | Lifetime |
| --- | --- | --- |
| Session | Conversation identity, workspace, messages, task plan, status, context/usage, current objective | Survives across prompts and app restarts through session logs. |
| Runtime | One provider/model call, streaming state, provider config, tool-call records, usage/error for that call | Exists for one model call inside a session turn. |
| Agent | Prompt/capability/provider policy used by the runtime | Selected by the session but configured separately. |

## Storage

| Storage | Contents |
| --- | --- |
| `<workspace>/.tura/session_log.sqlite3` | Workspace session history, messages, snapshots, and runtime session-management state. |
| `<TURA_HOME>/db/session_log/index.sqlite3` | Per-home session index and queue used to find sessions across workspaces. |
| `<workspace>/.tura/config.conf` | Workspace defaults used when creating or running sessions, such as model, agent, language, reasoning effort, and priority routing. |
| Session log queue files under `<TURA_HOME>/db/session_log/message_queue/` | Pending/failed session-log writes when the session DB service is not immediately reachable. |

The session log service is the durable owner. Runtime and gateway code write
session snapshots or lifecycle commands to it; if direct IPC is not available,
writes are queued and replayed later.

## Session lifecycle

| Phase | What happens | User-visible status |
| --- | --- | --- |
| Create | The gateway creates a session id, binds it to a workspace directory, resolves model/agent/session defaults, and starts with no active runtime turn. | `idle` |
| Submit prompt | The user message is added, the session becomes busy, a planning/progress task may be shown, and a background runtime worker starts. | `busy` |
| Runtime turn | The runtime sends messages to the provider, streams text, receives tool calls, executes tools, updates task progress, records token usage, and checkpoints the session. | `busy` |
| More user input while busy | The message is stored as a user command and forwarded to the active runtime queue instead of starting a second simultaneous turn. | `busy` |
| Completion | The runtime reaches a final answer or terminal task state, session data is checkpointed, and the session becomes available for another user turn. | `idle` |
| Failure/interruption | A provider/runtime failure, abort, crash, or lost worker marks the session as failed/interrupted and clears active work markers. | `error` |
| Follow-up prompt | The same session can be reused. Its history remains, but the new turn is prepared from a fresh created state. | `busy` after prompt submission |

The important rule: the session represents the conversation lifetime, while the
internal session state usually represents the current or most recent runtime
turn. Completed work does not close the conversation.

## Session and runtime relationship

The session chooses the execution context: workspace directory, agent, model,
reasoning effort, priority flag, permission mode, task type, and active task
plan. The runtime uses that context for one provider call.

The runtime reports back to the session through these channels:

| Runtime output | Session effect |
| --- | --- |
| Streamed assistant text | Updates the live message overlay while the runtime is active. |
| Tool calls and tool results | Become message parts and task-progress evidence. |
| Runtime state sync | Tells the gateway whether to keep using the live overlay or refresh canonical session DB history. |
| Context token stats | Updates `context_tokens.input` and `context_tokens.limit`. |
| Usage report | Updates token/cost/latency usage shown on the session. |
| Final runtime state | Helps decide whether the session ends as completed or failed. |

The gateway intentionally exposes a simpler session status than the runtime
state. Frontends should normally use `idle`, `busy`, and `error`; the richer
runtime state is for execution synchronization.

## Public session status

| Public status | Meaning | Derived from internal session states |
| --- | --- | --- |
| `idle` | The session is not actively running a turn and can accept a new prompt. | `created`, `completed` |
| `busy` | A runtime turn is active or queued for this session. | `running`, `paused` |
| `error` | The last turn failed, was cancelled, or was interrupted. | `failed`, `cancelled`, `interrupted` |

The public status is a projection. It is not the source of truth for the runtime
state machine.

## Internal session state machine

| Internal state | Meaning |
| --- | --- |
| `created` | The session/turn exists but has not begun runtime work. |
| `running` | Runtime work is active. |
| `paused` | Runtime work is temporarily waiting but still considered in-flight. |
| `completed` | The turn finished successfully. The conversation can still continue later. |
| `failed` | The turn failed. This is terminal for that turn. |
| `cancelled` | The turn was cancelled. This is terminal for that turn. |
| `interrupted` | Runtime liveness was lost or the turn was forcibly interrupted. This is terminal for that turn. |

Allowed state movement is intentionally narrow:

| From | Can move to |
| --- | --- |
| `created` | `running`, `cancelled` |
| `running` | `paused`, `completed`, `failed`, `cancelled`, `interrupted` |
| `paused` | `running`, `failed`, `cancelled`, `interrupted` |
| `completed` | `created`, `running` for a later follow-up turn |
| `failed` | No normal outbound transition |
| `cancelled` | No normal outbound transition |
| `interrupted` | No normal outbound transition |

When a later user turn is prepared, the system can reuse the same conversation
session and reset the turn from `completed`, `failed`, `cancelled`, or
`interrupted` back to `created`. That reset is a new-turn preparation step, not a
normal state-machine edge. Subtle distinction, but it keeps the old failure from
pretending it never happened.

## Core session state properties

| Property | Role |
| --- | --- |
| `session_id` | Stable identifier for the conversation/work item. |
| `session_name` / `auto_session_name` | Display name and whether the name may follow the latest task summary automatically. |
| `session_directory` | Workspace directory the session operates in. |
| `session_current_turn` | Count of runtime turns consumed by the session tree. |
| `session_log` | Internal execution log used to rebuild context and persist history. |
| `session_log_retention` | Tracks omitted entries after context compaction so retained log indexes remain meaningful. |
| `session_created_at` | Creation timestamp. |
| `session_last_update_at` | Last state/log/task update timestamp. |
| `session_last_user_message_at` | Last user-authored message timestamp, used for ordering sessions. |
| `session_started_at` | Start time for the current or latest runtime turn. |
| `input` | Current user input and attached file metadata for the active turn. |
| `user_goal` | Original summarized goal. |
| `current_objective` | Current concrete objective used for completion auditing and planning reminders. |
| `task_type` | Operation-manual/task categories active for the session. Legacy broad session kinds are filtered out here. |
| `session_capabilities` | Tool capabilities already loaded into context, such as command or media tools. |
| `task_plan` | Plan summary plus detailed task records. |
| `state` | Canonical internal session lifecycle state. |
| `use_last_tool_call_response` | Whether context should include the previous tool response verbatim. |
| `is_child_session` | Marks delegated/sub-session work. |
| `disable_permission_restrictions` | Allows runtime command execution to bypass normal workspace permission restrictions. |
| `planning_enabled` | Whether the active agent/run includes planning behavior. |
| `reflection_enabled` | Whether reflective task-status prompt style is enabled. |
| `op_manual_enabled` / `no_op_manual` | Controls operation-manual injection. |
| `goal_mode` / `last_goal_user_input` | Keeps the session running toward an explicit goal until task status settles it. |
| `context_tokens` | Latest context input token count and configured limit. |
| `runtime_usage` | Latest terminal provider token/cost/latency report. |

## Task-management state

Task management is the model-visible work plan. A small single-task session is
shown as one object; larger plans are shown as a task list.

| Task field | Meaning |
| --- | --- |
| `task_id` | Stable task key used for updates. |
| `step` | Step number in the plan. |
| `sub_session_id` | Child session id when work is delegated. |
| `start_at` | Time when the task should start or next run. |
| `poll_interval` | Calendar-like polling interval with month/day/hour/second fields. |
| `start_condition` | Trigger: `session_idle`, `user_action`, `scheduled_task`, or `polling_task`. |
| `status` | Internal plan status. |
| `task_summary` | Compact summary shown in normal turns. |
| `step_task` | Full task description. |
| `step_turn` | Turn count consumed by this step, including child processes. |
| `step_tool` | Tool requirement description. |
| `step_context` | Context needed to execute the step. |
| `step_agent_name` | Agent responsible for the step. |
| `step_deliverable_description` | Expected deliverable. |
| `step_deliverable_path` | Output path for the deliverable. |

Plan statuses are canonical internal values:

| Plan status | Meaning |
| --- | --- |
| `todo` | Not started. |
| `waiting_user` | Waiting for user action or idle-session confirmation. |
| `doing` | Currently active. |
| `question` | Needs a user answer before it can continue. |
| `done` | Completed. |
| `archived` | Hidden from active work without deleting history. |

When the scheduler claims a due `scheduled_task` or `polling_task`, it only
runs tasks from idle sessions, marks the chosen task `doing`, changes the
session to busy, and starts the runtime prompt for that task.

## Runtime state machine

Runtime state is more detailed because it tracks a single provider call.

| Runtime state | Meaning |
| --- | --- |
| `created` | Runtime record exists but has not been sent to the provider. |
| `dispatching` | Request is being dispatched to the provider. |
| `waiting_first_token` | Provider request is accepted; no first token yet. |
| `streaming` | First token arrived and output/tool calls may stream. |
| `finished` | Runtime call succeeded. |
| `failed` | Runtime call failed, timed out, or was cancelled. |

| Runtime property | Role |
| --- | --- |
| `runtime_id` | Identifier for this provider call. |
| `session_id` | Parent session that owns this runtime call. |
| `agent_id` | Agent used for this runtime call. |
| `provider` | Captured provider/model config for reproducibility and diagnostics. |
| `called_at`, `first_token_at`, `call_finished_at` | Timing milestones. |
| `call_result_status` | `pending`, `streaming`, `succeeded`, `failed`, `timed_out`, or `cancelled`. |
| `fallback_from_id` | Links a fallback runtime to the failed runtime it replaced. |
| `input` / `output` | Exact provider request and response payloads when retained. |
| `text` | Assistant output text accumulated for this runtime call. |
| `tool_call` | Tool-call records produced by the model. |
| `context_tokens` | Input-token and limit data for this runtime call. |
| `usage` | Token/cost/latency report for this runtime call. |

The runtime also publishes a session sync status with `runtime_id`, runtime
state, call result status, `live`, and `session_db_refresh_required`. While the
runtime is active, the GUI/TUI can show live overlay updates. Once the runtime is
finished or failed, the frontend should refresh canonical session history from
the session DB.

## Interruption and liveness

Busy sessions are probed against the router. If the gateway sees a session as
busy but the router no longer reports it as active, queued, running, or backed
by a live worker, the gateway marks it interrupted.

Interruption does three things:

| Effect | Reason |
| --- | --- |
| Internal state becomes `interrupted` | The previous runtime turn is no longer trustworthy. |
| Public status becomes `error` | Frontends need to stop displaying it as actively running. |
| Active `doing` tasks become `waiting_user` | The user must decide whether to resume, retry, or abandon the work. |

## Child sessions

A child session is delegated work linked to a parent session. The parent keeps a
task record with `sub_session_id`; the child has its own messages, runtime calls,
task plan, context tokens, and usage. Child sessions let the runtime isolate
subtasks while the parent session remains the coordination thread.

## Context compaction

Long sessions can exceed the context token budget. When context is compacted,
the session records a compaction boundary, drops omitted log entries from the
in-memory state, and retains enough index information to keep later session-log
records meaningful.

The key properties are:

| Property | Meaning |
| --- | --- |
| `context_tokens.input` | Latest estimated or provider-reported context input tokens. |
| `context_tokens.limit` | Active context limit. Default is `260000`. |
| `session_log_retention.omitted_entries` | How many earlier log entries were removed from current state. |
| `session_log_retention.last_compaction` | Absolute compaction index, retained boundary, and compaction timestamp. |

Compaction changes the context history shape, not the session identity.

## Practical model

Think of a session as the durable work ledger and a runtime as one execution
attempt written into that ledger. The session decides what work exists, where it
runs, what settings apply, what status the user should see, and what history is
kept. The runtime performs one provider call and reports text, tool calls,
errors, usage, and synchronization state back into the session.
