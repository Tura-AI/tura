# Gateway Crate Architecture

`crates/gateway` is the middleware between the frontend and backend crates. It
provides the HTTP API surface consumed by the UI, translates
UI payloads into runtime/router/provider calls, persists UI-facing session data,
and streams backend events back to the frontend.

Gateway must not run the agent loop or own command routing. Runtime work goes
through `crates/runtime`; CLI forwarding and managed process lifecycle go
through `crates/router`.

## Layout

```text
crates/gateway/
  src/
    api/
      session/
      thread/
      turn/
      file/
      project/
      provider/
      permissions/
      pty/
      process/
      token_usage/
      config/

    session/
      manager/
      store/
      process_cleanup/
      process_snapshot/
      docker_snapshot/
      startup_context/
      event_replay/

    transport/
      sse/
      websocket/
      app_events/
      event_buffer/

    runtime_client/
      start_session.rs
      inject_turn.rs
      cancel_turn.rs
      read_session.rs

    web/
    mock/
```

## Owns

- Frontend-facing API routes.
- API payload validation, compatibility mapping, and response shaping.
- Session, thread, and turn API surfaces.
- UI-facing session/message/todo/event persistence.
- Event streaming and replay.
- Token usage projection and summaries.
- Permission request/response forwarding.
- File/project helper APIs for UI inspection.
- Provider/model config API projection and forwarding.
- PTY and process API adapters for UI.
- Workspace config load/merge/save.
- Session startup request assembly.
- Persisted session payloads.
- Mock stores for tests.
- Workspace process-cleanup coordination before session startup.
- Runtime client calls to `crates/runtime`.
- Router client calls to `crates/router`.

## Does Not Own

- Runtime loop.
- Agent activation.
- Prompt assembly.
- Provider request construction.
- Tool/command execution.
- Shell sandboxing.
- Patch application.
- Command registry.
- CLI forwarding rules.
- Router-managed service/process lifecycle.
- Router command internals.

Those belong to `crates/runtime`, `crates/provider`, `crates/tools`, and
`crates/router`.

## Runtime Client

`runtime_client/` is the gateway path into runtime execution.

It should support:

- `start_session`
- `inject_turn`
- `cancel_turn`
- `read_session`

Gateway treats runtime as a boundary even if the call is in-process.

## Router Client

Gateway may expose UI-facing process, PTY, command status, managed service
status, or project startup APIs, but CLI forwarding and lifecycle work are
delegated to `crates/router` or to a narrow session-cleanup helper.

The router client should support:

- resolve command metadata
- request managed process startup
- read command/process/service status
- forward stop/cancel requests for routed CLI processes
- proxy command events into gateway app events

Gateway shapes the API for the frontend; router owns CLI forwarding metadata and
managed lifecycle.

## Core Flows

Start session:

```text
apps/ui
  -> crates/gateway api/session
  -> gateway loads config and validates request
  -> gateway asks crates/router to resolve CLI commands and start managed services when needed
  -> runtime_client/start_session
  -> crates/runtime
  -> gateway transport events
```

Submit turn:

```text
apps/ui
  -> crates/gateway api/turn
  -> runtime_client/inject_turn
  -> crates/runtime turn loop
  -> crates/tools when tools are needed
  -> crates/router when CLI command forwarding is needed
  -> gateway app_events
  -> apps/ui stream
```

Permission:

```text
crates/tools or crates/router
  -> permission requested event
  -> gateway/api/permissions exposes pending request
  -> UI approves or denies
  -> gateway forwards decision
  -> caller continues or fails
```

Token usage replay:

```text
crates/provider usage records
  -> crates/runtime runtime records
  -> gateway event_replay
  -> api/token_usage
  -> UI usage display
```

## Session Cleanup Coordination

Before a new session starts, gateway may coordinate stale workspace cleanup
because cleanup is part of session startup policy. Router-managed services must
still be controlled through router.

Rules:

- Match by process cwd when possible.
- Protect gateway, router, current process, and parent process chain.
- Cleanup failures should be logged and should not block session creation.
- Router-managed services/processes must be stopped through router.

## Checks

Use:

```text
cargo fmt -p gateway
cargo check -p gateway
```
