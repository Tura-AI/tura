# TUI Refresh-Only Streaming Repair Plan

## Scope

The Tura TUI can feel laggy during assistant streaming because each
`message.part.delta` currently mutates `state.messages` and schedules a render
that can rebuild the full transcript frame. In chat mode, the frame can also be
written as a clear-and-repaint operation, which makes terminal scrollback and
the scrollbar appear to jump or flicker.

This plan is intentionally scoped to the refresh/state-update path for the next
implementation pass. Do not change displayed text, transcript layout, colors,
message ordering, panel content, scroll behavior, or the current full-history
display contract in this pass. The first pass should make refresh cheaper by
changing where streaming and polling data is stored and reconciled.

Rendering and paint-path ideas from Codex are included as reference only. They
must not be implemented in this refresh-only pass unless a separate task
explicitly authorizes display behavior changes.

## Problem

The desired behavior is closer to Codex:

- live deltas update only transient, non-historical stream state;
- durable transcript history changes only when a finalized message or part
  update arrives;
- polling fetches and reconciles only the new durable messages when possible;
- full durable history hydration is reserved for startup, session switch, event
  reconnect, mismatch recovery, or explicit low-frequency reconciliation.

## Codex Reference Shape

Reference repository root:

```text
C:\Users\liuliu\Documents\Codex
```

Relevant Codex files and directories to inspect while implementing:

```text
codex-rs\tui\src\app\resize_reflow.rs
codex-rs\tui\src\chatwidget\transcript.rs
codex-rs\tui\src\app\replay_filter.rs
codex-rs\tui\src\app\pending_interactive_replay.rs
codex-rs\tui\src\app\loaded_threads.rs
codex-rs\app-server-protocol\src\protocol\v2\thread.rs
codex-rs\app-server-protocol\src\protocol\thread_history.rs
codex-rs\app-server-protocol\src\protocol\event_mapping.rs
codex-rs\app-server\src\request_processors\thread_processor.rs
codex-rs\app-server\src\request_processors\thread_lifecycle.rs
```

Codex separates three concepts that Tura currently blends together:

- **Source history**: durable conversation items or transcript cells. These are
  the truth used for replay, copy, resize reflow, and resume.
- **Live stream cells**: transient cells for active agent output, plan deltas,
  and running tool/status displays. They are replaced or consolidated when the
  final source-backed item arrives.
- **Rendered rows**: terminal-width-specific display lines. Codex renders from
  the transcript tail with a row cap and rebuilds from source only when needed,
  such as resize reflow.

Tura should adopt the same ownership split without porting Codex internals
directly.

For this refresh-only pass, use the Codex split only to guide state ownership:
streaming deltas are live state, finalized messages are durable history, and
refresh/replay should reconcile those two streams without changing presentation.

## Target Model

Add an explicit split to TUI state:

```text
AppState
  session
  sessions
  messages                 durable finalized messages only
  liveStreams              transient per-message/part streaming text
  refreshState             per-session cursors and reconciliation metadata
```

`liveStreams` is keyed by `sessionID/messageID/partID` and contains only the
currently streamed text plus small metadata needed for display:

```text
LiveStream {
  sessionID
  messageID
  partID
  field
  text
  updatedAt
}
```

Streaming deltas must not append text to `state.messages`. The durable message
array is updated only by:

- `hydrate`
- `message.updated`
- `message.part.updated`
- `message.removed`

When a final `message.updated` or `message.part.updated` arrives for the same
`messageID/partID`, the reducer removes the matching `liveStreams` entry and
lets the source-backed message render from `messages`.

## Phase 1: Delta-Only Live State, Same Display

Reducer changes:

- Add `liveStreams` to `AppState`.
- Change `message.part.delta` handling so it patches `liveStreams`, not
  `messages`.
- Preserve out-of-order behavior by creating a live stream entry if the matching
  message or part is not present yet.
- On `message.updated`, upsert the finalized message into `messages` and clear
  live streams for its parts.
- On `message.part.updated`, upsert the finalized part and clear that live
  stream.
- On `hydrate`, clear live streams only when the active session changes; keep
  active live streams during same-session polling so a stale hydrate cannot hide
  visible output.

Acceptance:

- Streaming output remains visible before the final persisted message arrives.
- Replayed or delayed final messages replace live output without duplicate text.
- `state.messages` length and message text do not grow on every delta.
- Existing render output stays visually equivalent for the same event sequence.

Required verification before moving on:

```text
cd apps/tui
npm test
npm run test:stream
npm run stress:memory
```

If a phase changes browser-visible terminal behavior unexpectedly, stop and
repair before continuing.

## Phase 2: Incremental Message Refresh

Polling changes:

- Track `lastFinalMessageID` and `lastFinalMessageCount` per session.
- In `pollingLoop`, prefer
  `GET /session/{sessionID}/message?after=<lastFinalMessageID>`.
- Merge returned messages into durable `messages`.
- Fall back to a full message hydrate only on session switch, event reconnect,
  detected count mismatch, message removal, or periodic low-frequency
  reconciliation.
- During same-session polling, never clear active `liveStreams` from an older
  durable snapshot.

Important detail: this phase changes how fresh data is fetched and merged. It
must not change the renderer, message text, ordering, panel content, or current
scroll/display behavior.

Acceptance:

- Polling cannot replace the active live stream with stale durable history.
- Long sessions do not allocate full message snapshots every 1.5 seconds.
- Session picker previews reuse cached message previews where possible.
- Existing TUI screenshots/snapshots remain equivalent except for timing.

Required verification before moving on:

```text
cd apps/tui
npm test
npm run test:stream
npm run test:business:refresh-replay
npm run stress:memory
```

## Phase 3: Refresh Cache And Reconciliation

Refresh cache changes:

- Add a small TUI-local refresh cache for per-session durable message cursors,
  message counts, updated timestamps, and previews.
- Keep gateway as source of truth. The cache exists only to avoid redundant
  fetches and to merge event/polling data deterministically.
- Reconcile event-stream updates and polling updates through the same helper so
  `message.updated`, `message.part.updated`, and incremental pages cannot create
  duplicates.
- Invalidate the cache on session switch, event stream reconnect, explicit
  resume, message removal, and full hydrate fallback.

Important detail: do not add a broad render cache in this pass. This phase is a
data refresh cache, not a display cache.

Acceptance:

- Session picker message counts and previews stay correct.
- Active session transcript stays identical to the previous implementation.
- Reconnect and stale polling snapshots do not duplicate or erase live output.

Required verification before moving on:

```text
cd apps/tui
npm test
npm run test:stream
npm run test:business:multi-session
npm run test:business:refresh-replay
npm run stress:memory
```

## Deferred Display Work

The following Codex-inspired display changes are intentionally deferred because
the current task is refresh-only:

- bounded visible-tail rendering;
- line-level chat repaint;
- terminal scrollback ownership changes;
- finalized-message render cache;
- resize reflow changes;
- full history pager/export changes.

These can be planned separately after the refresh-only pass proves that delta
and polling churn are no longer the main memory/latency source.

## Test Plan

Unit tests:

- Reducer: deltas update `liveStreams` and leave `messages` unchanged.
- Reducer: final `message.updated` clears matching live stream.
- Reducer: stale same-session hydrate does not erase live stream output.
- Reducer: incremental polling pages merge without duplicating messages.
- Reducer: cache invalidates on session switch and reconnect.
- Renderer regression: live stream appears in the same place as before.
- Renderer regression: display text and ordering remain unchanged.

Performance tests:

- Keep `npm run stress:memory` as the regression harness.
- Add a focused case where thousands of deltas target one live stream and assert
  retained heap remains bounded after GC.
- Add a large-history polling case and compare full hydrate versus incremental
  refresh.

E2E tests:

- Mock gateway streaming verifies no duplicate final assistant text.
- Refresh replay verifies stale polling does not hide active live output.
- Multi-session verifies background session events update counts/previews
  without changing the active transcript.

Every implementation phase must run the existing TUI tests that cover normal
display and behavior before continuing:

```text
cd apps/tui
npm test
npm run test:stream
```

Run the focused business tests when the phase touches that surface:

```text
npm run test:business:refresh-replay
npm run test:business:multi-session
```

Run `npm run stress:memory` after each phase to ensure the refresh changes are
moving memory and elapsed time in the right direction.

## Rollout Order

1. Implement `liveStreams` and delta-only reducer behavior while preserving
   current rendered output.
2. Convert polling to incremental `after` refresh with full-hydrate fallback.
3. Add the refresh cache and shared reconciliation helpers.

Do not start with a general cache layer. The first measurable win should come
from preventing deltas from touching durable history and preventing polling from
replacing the active session with redundant full snapshots.
