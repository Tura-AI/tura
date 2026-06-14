import assert from "node:assert/strict";
import test from "node:test";
import type { Session } from "../../../src/types/session.js";
import { messageText } from "../../../src/types/session.js";
import {
  createResizeDrawGate,
  createTerminalResizeHandler,
  createAndSelectSession,
  deleteSelectedSession,
  draw,
  forkSelectedSession,
  openSessionPicker,
  resetDrawState,
  submitPrompt,
} from "../../../src/tui/app.js";
import { renderChatFrameParts } from "../../../src/tui/render.js";
import { initialState, reducer, type AppAction, type AppState } from "../../../src/tui/reducer.js";
import { plainCapabilities, richCapabilities } from "../../../src/tui/capabilities.js";
import { clear as terminalClear } from "../../../src/tui/render-terminal.js";

const activeSession: Session = {
  id: "sess-1",
  name: "Active",
  directory: "C:/repo",
  status: "idle",
  updated_at: 1,
  message_count: 2,
};

const otherSession: Session = {
  id: "sess-2",
  name: "Other",
  directory: "C:/repo",
  status: "idle",
  updated_at: 2,
  message_count: 3,
};

test("openSessionPicker returns immediately when remote session refresh is wedged", async () => {
  const client = {
    listSessions: () => new Promise<Session[]>(() => {}),
    listMessages: () => new Promise(() => {}),
  };
  const harness = stateHarness(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [],
      permissions: [],
      sessions: [activeSession, otherSession],
    }),
  );

  await openSessionPicker(client as never, harness.getState, harness.dispatch);

  assert.equal(harness.getState().sessionsOpen, true);
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-2", "sess-1"],
  );
});

test("openSessionPicker clears the terminal before rendering the session picker", async () => {
  const client = {
    listSessions: () => new Promise<Session[]>(() => {}),
    listMessages: () => new Promise(() => {}),
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-live",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-live", type: "text", text: "OLD_CHAT_LIVE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [activeSession, otherSession],
  });

  const writes = await captureDrawWritesAsync(async (capturedWrites) => {
    let lastFrame = draw(state, richCapabilities(), "");
    capturedWrites.length = 0;
    await openSessionPicker(
      client as never,
      () => state,
      (action) => {
        state = reducer(state, action);
        lastFrame = draw(state, richCapabilities(), lastFrame);
      },
    );
  });

  const firstWrite = writes[0] ?? "";
  const output = writes.join("");
  const firstClearIndex = output.indexOf(terminalClear);
  const sessionPickerIndex = output.indexOf("Other");

  assert.ok(
    firstWrite.includes(terminalClear),
    "session picker should hard-clear before state paint",
  );
  assert.equal(
    regexCount(firstWrite, /\x1b\[2K\r\n/gu),
    0,
    "session picker clear must not fill scrollback with blank rows",
  );
  assert.ok(firstClearIndex >= 0, "session picker transition must emit a terminal clear");
  assert.ok(sessionPickerIndex > firstClearIndex, "session picker content must render after clear");
});

test("createAndSelectSession creates and selects a real session", async () => {
  const createdSession: Session = {
    id: "sess-created",
    name: "Created",
    directory: "C:/repo",
    status: "idle",
    updated_at: 4,
    message_count: 0,
  };
  const calls: unknown[] = [];
  const client = {
    createSession: async (payload: unknown) => {
      calls.push(payload);
      return createdSession;
    },
    listMessages: () => new Promise(() => {}),
    listProviders: () => new Promise(() => {}),
    getSessionConfig: () => new Promise(() => {}),
    listAgents: () => new Promise(() => {}),
    listPersonas: () => new Promise(() => {}),
    listSessions: () => new Promise(() => {}),
  };
  const harness = stateHarness(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [
        {
          id: "msg-old",
          sessionID: "sess-1",
          role: "assistant",
          parts: [{ id: "part-old", type: "text", text: "old transcript" }],
        },
      ],
      permissions: [],
      sessions: [activeSession],
    }),
  );

  await createAndSelectSession(client as never, harness.getState, harness.dispatch);

  assert.equal(calls.length, 1);
  assert.equal(harness.getState().session?.id, "sess-created");
  assert.equal(harness.getState().session?.draft, undefined);
  assert.deepEqual(harness.getState().messages, []);
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-1", "sess-created"],
  );
});

test("createAndSelectSession falls back to a draft when session creation fails", async () => {
  const client = {
    createSession: async () => {
      throw new Error("offline");
    },
  };
  const harness = stateHarness(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [
        {
          id: "msg-old",
          sessionID: "sess-1",
          role: "assistant",
          parts: [{ id: "part-old", type: "text", text: "old transcript" }],
        },
      ],
      permissions: [],
      sessions: [activeSession],
    }),
  );

  await createAndSelectSession(client as never, harness.getState, harness.dispatch);

  assert.equal(harness.getState().session?.draft, true);
  assert.match(harness.getState().session?.id ?? "", /^draft-session-/);
  assert.deepEqual(harness.getState().messages, []);
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-1"],
  );
});

test("submitPrompt promotes a draft session to a real session", async () => {
  const realSession: Session = {
    id: "sess-3",
    name: "Real",
    directory: "C:/repo",
    status: "idle",
    updated_at: 3,
  };
  const calls: Array<{ method: string; payload?: unknown; sessionID?: string }> = [];
  const client = {
    createSession: async (payload: unknown) => {
      calls.push({ method: "createSession", payload });
      return realSession;
    },
    sendPromptAsync: async (sessionID: string) => {
      calls.push({ method: "sendPromptAsync", sessionID });
    },
  };
  const draftClient = {
    createSession: async () => {
      throw new Error("offline");
    },
  };
  const harness = stateHarness(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [
        {
          id: "msg-old",
          sessionID: "sess-1",
          role: "assistant",
          parts: [{ id: "part-old", type: "text", text: "old transcript" }],
        },
      ],
      permissions: [],
      sessions: [activeSession],
      sessionConfig: {
        active_agent: "thinking",
        model: "codex/gpt-5.5",
        model_variant: "high",
        model_acceleration_enabled: true,
      },
    }),
  );

  await createAndSelectSession(draftClient as never, harness.getState, harness.dispatch);
  await submitPrompt(client as never, harness.getState, harness.dispatch, "hello");

  assert.deepEqual(
    calls.map((call) => call.method),
    ["createSession", "sendPromptAsync"],
  );
  assert.equal(calls[1].sessionID, "sess-3");
  assert.equal(harness.getState().session?.id, "sess-3");
  assert.equal(harness.getState().session?.draft, undefined);
  assert.deepEqual(harness.getState().messages, []);
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-1", "sess-3"],
  );
  assert.equal(harness.getState().status, "busy");
});

test("forkSelectedSession copies selected session context and selects the copy", async () => {
  const copiedSession: Session = {
    id: "sess-copy",
    name: "Copied",
    parent_id: "sess-2",
    directory: "C:/repo",
    status: "idle",
    updated_at: 5,
    message_count: 1,
  };
  const calls: Array<{ method: string; sessionID?: string; payload?: unknown }> = [];
  const client = {
    forkSession: async (sessionID: string, payload: unknown) => {
      calls.push({ method: "forkSession", sessionID, payload });
      return copiedSession;
    },
    listMessages: async (sessionID: string) => [
      {
        id: "msg-copy",
        sessionID,
        role: "assistant" as const,
        parts: [{ id: "part-copy", type: "text", text: "copied context" }],
      },
    ],
    listProviders: async () => undefined,
    getSessionConfig: async () => undefined,
    listAgents: async () => [],
    listPersonas: async () => [],
    listProviderAuthMethods: async () => ({}),
    providerAuthStatus: async () => undefined,
    listSessions: async () => [copiedSession, activeSession, otherSession],
  };
  const harness = stateHarness({
    ...reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [],
      permissions: [],
      sessions: [otherSession, activeSession],
    }),
    sessionsOpen: true,
    selectedSessionIndex: 1,
  });

  await forkSelectedSession(client as never, harness.getState, harness.dispatch);

  assert.deepEqual(calls, [
    { method: "forkSession", sessionID: "sess-2", payload: { copy_context: true } },
  ]);
  assert.equal(harness.getState().session?.id, "sess-copy");
  assert.equal(harness.getState().sessionsOpen, false);
  assert.equal(messageText(harness.getState().messages[0]), "copied context");
});

test("deleteSelectedSession removes selected session and refreshes the picker", async () => {
  const calls: string[] = [];
  const client = {
    deleteSession: async (sessionID: string) => {
      calls.push(`delete:${sessionID}`);
      return true;
    },
    listSessions: async () => [activeSession],
  };
  const harness = stateHarness({
    ...reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [],
      permissions: [],
      sessions: [otherSession, activeSession],
    }),
    sessionsOpen: true,
    selectedSessionIndex: 1,
  });

  await deleteSelectedSession(client as never, harness.getState, harness.dispatch);

  assert.deepEqual(calls, ["delete:sess-2"]);
  assert.equal(harness.getState().sessionsOpen, true);
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-1"],
  );
  assert.equal(harness.getState().session?.id, "sess-1");
});

test("draw clears terminal when opening the session picker over chat scrollback", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-old",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-old", type: "text", text: "old transcript" }],
      },
    ],
    permissions: [],
    sessions: [activeSession, otherSession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    const picker = reducer(state, {
      type: "sessions",
      value: [otherSession, activeSession],
      open: true,
    });
    draw(picker, richCapabilities(), previous);
  });

  assert.ok(writes.join("").includes(terminalClear));
});

test("draw clears terminal when the active session changes", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-active",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-active", type: "text", text: "active transcript" }],
      },
    ],
    permissions: [],
    sessions: [activeSession, otherSession],
  });

  const writes = captureDrawWrites((writes) => {
    const first = draw(state, richCapabilities(), "");
    const picker = reducer(state, {
      type: "sessions",
      value: [otherSession, activeSession],
      open: true,
    });
    const pickerFrame = draw(picker, richCapabilities(), first);
    writes.length = 0;
    const selected = reducer(picker, {
      type: "hydrate",
      session: otherSession,
      messages: [],
      permissions: [],
      sessions: [otherSession, activeSession],
    });
    draw(selected, richCapabilities(), pickerFrame);
  });

  assert.ok(writes.join("").includes(terminalClear));
});

test("draw keeps cursor hidden on pages without an input box", () => {
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [],
      permissions: [],
      sessions: [activeSession, otherSession],
    }),
    {
      type: "sessions",
      value: [otherSession, activeSession],
      open: true,
    },
  );

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");

  assert.match(output, /^\x1b\[\?25l/);
  assert.doesNotMatch(output, /\x1b\[\?25h/);
});

test("draw can enter a selected session without rendering the previous session", () => {
  const active = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-active",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-active", type: "text", text: "ACTIVE_STALE_TRANSCRIPT" }],
      },
    ],
    permissions: [],
    sessions: [activeSession, otherSession],
  });
  const picker = reducer(active, {
    type: "sessions",
    value: [otherSession, activeSession],
    open: true,
  });

  const writes = captureDrawWrites((writes) => {
    const pickerFrame = draw(picker, richCapabilities(), "");
    writes.length = 0;
    const selected = reducer(picker, {
      type: "hydrate",
      session: otherSession,
      messages: [
        {
          id: "msg-other",
          sessionID: "sess-2",
          role: "assistant",
          parts: [{ id: "part-other", type: "text", text: "OTHER_SELECTED_TRANSCRIPT" }],
        },
      ],
      permissions: [],
      sessions: [otherSession, activeSession],
      closePanels: true,
    });
    draw(selected, richCapabilities(), pickerFrame);
  });
  const output = writes.join("");

  assert.match(output, /OTHER_SELECTED_TRANSCRIPT/);
  assert.doesNotMatch(output, /ACTIVE_STALE_TRANSCRIPT/);
  assert.doesNotMatch(output, /New session/);
});

test("draw writes chat as fixed history plus live output without screen-window repaint", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-chat",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-chat", type: "text", text: "chat transcript" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");

  assert.ok(output.includes(terminalClear));
  assert.match(output, /chat transcript/);
  assert.match(output, /chat transcript[\s\S]*Active[\s\S]*回车输入/);
  assert.doesNotMatch(output, /\x1b\[999;1H/);
  assert.doesNotMatch(output, /\x1b7|\x1b8/);
  assert.match(output, /\x1b\[\d+(?:;\d+)?[HG]\x1b\[\?25h$/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
});

test("draw appends new cache lines once before live and chrome", () => {
  const initial = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-cache-base",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-cache-base", type: "text", text: "CACHE_BASE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });
  const appended = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      ...initial.messages,
      {
        id: "msg-cache-never-live",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-cache-never-live", type: "text", text: "CACHE_NEVER_LIVE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(initial, richCapabilities(), "");
    const previousLiveRegion = renderChatFrameParts(initial, richCapabilities()).liveRegionCursor;
    writes.length = 0;
    draw(appended, richCapabilities(), previous);
    assert.ok(previousLiveRegion);
    assertMutableRegionClearedBefore(
      writes.join(""),
      previousLiveRegion.row,
      "CACHE_NEVER_LIVE_MARKER",
    );
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /CACHE_BASE_MARKER/);
  assert.match(output, /CACHE_NEVER_LIVE_MARKER/);
  assert.match(output, /CACHE_NEVER_LIVE_MARKER[\s\S]*Active[\s\S]*回车输入/);
});

test("draw redraws cache live and chrome on terminal width resize", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-resize-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-resize-cache", type: "text", text: "RESIZE_CACHE_MARKER" }],
      },
      {
        id: "msg-resize-live-user",
        sessionID: "sess-1",
        role: "user",
        parts: [{ id: "part-resize-live-user", type: "text", text: "RESIZE_LIVE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-resize-live-assistant",
          part_id: "part-resize-live-assistant",
          field: "text",
          delta: "RESIZE_STREAM_MARKER",
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    Object.defineProperty(process.stdout, "columns", { configurable: true, value: 100 });
    draw(state, richCapabilities(), previous);
  });
  const output = writes.join("");

  assert.ok(output.includes(terminalClear));
  assert.match(output, /RESIZE_CACHE_MARKER/);
  assert.match(output, /RESIZE_LIVE_MARKER/);
  assert.match(output, /RESIZE_STREAM_MARKER/);
  assert.match(output, /Active[\s\S]*回车输入/);
});

test("draw ignores terminal height resize without rewriting", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-height-resize-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-height-resize-cache", type: "text", text: "HEIGHT_CACHE_MARKER" }],
      },
      {
        id: "msg-height-resize-user",
        sessionID: "sess-1",
        role: "user",
        parts: [{ id: "part-height-resize-user", type: "text", text: "HEIGHT_USER_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-height-resize-live",
          part_id: "part-height-resize-live",
          field: "text",
          delta: "HEIGHT_STREAM_MARKER",
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    Object.defineProperty(process.stdout, "rows", { configurable: true, value: 40 });
    draw(state, richCapabilities(), previous);
  });
  const output = writes.join("");

  assert.equal(output, "");
});

test("draw force reset redraws cache live and chrome for resize snapshots", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-force-resize-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-force-resize-cache", type: "text", text: "FORCE_RESIZE_CACHE" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-force-resize-live",
          part_id: "part-force-resize-live",
          field: "text",
          delta: "FORCE_RESIZE_LIVE",
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    Object.defineProperty(process.stdout, "rows", { configurable: true, value: 40 });
    draw(state, richCapabilities(), previous, { forceReset: true });
  });
  const output = writes.join("");

  assert.ok(output.includes(terminalClear));
  assert.match(output, /FORCE_RESIZE_CACHE/);
  assert.match(output, /FORCE_RESIZE_LIVE/);
  assert.match(output, /Active[\s\S]*回车输入/);
});

test("draw appends completed live rows before painting chrome in the reservation tail", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-anchor-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-anchor-cache", type: "text", text: "CACHE_ANCHOR_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-anchor-live",
          part_id: "part-anchor-live",
          field: "text",
          delta: "LIVE_ANCHOR_MARKER",
        },
      },
    },
  });

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");
  const cacheIndex = output.indexOf("CACHE_ANCHOR_MARKER");
  const liveIndex = output.indexOf("LIVE_ANCHOR_MARKER");
  const chromeIndex = output.indexOf("Active", liveIndex);
  const chromeAnchor = lastAbsoluteCursorBefore(output, chromeIndex);

  assert.ok(cacheIndex >= 0, "expected cache marker to be written");
  assert.ok(liveIndex > cacheIndex, "completed live rows must render after the cache is written");
  assert.ok(chromeIndex > liveIndex, "chrome must render after live");
  assert.ok(chromeAnchor, "chrome must start in the visible reservation tail");
  assert.ok(
    chromeAnchor.row > 1 && chromeAnchor.row <= 20,
    "chrome must start after live inside reservation tail",
  );
  assert.equal(
    output.indexOf("\x1b[1;1H\x1b[J", cacheIndex),
    -1,
    "full chat redraw must not clear cache with the mutable-region clear",
  );
});

test("draw renders chrome directly below cache when there is no live", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-no-live-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-no-live-cache", type: "text", text: "NO_LIVE_CACHE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });

  const parts = renderChatFrameParts(state, richCapabilities());
  const cacheLineCount = parts.cacheFrame ? parts.cacheFrame.split("\n").length : 0;
  const expectedChromeStartRow = Math.min(20, cacheLineCount + 1);
  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");
  const cacheIndex = output.indexOf("NO_LIVE_CACHE_MARKER");
  const chromeIndex = output.indexOf("Active", cacheIndex);
  const chromeAnchorIndex = output.lastIndexOf(`\x1b[${expectedChromeStartRow};1H`, chromeIndex);

  assert.equal(parts.liveFrame, "");
  assert.ok(cacheIndex >= 0, "expected cache marker to be written");
  assert.ok(chromeIndex > cacheIndex, "chrome must render after cache when live is empty");
  assert.ok(chromeAnchorIndex >= 0, "chrome must start immediately after cache when live is empty");
});

test("draw keeps overflowing live rows mutable and chrome visible at the reservation tail", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-live-overflow-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-live-overflow-cache", type: "text", text: "LIVE_OVERFLOW_CACHE" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-live-overflow-stream",
          part_id: "part-live-overflow-stream",
          field: "text",
          delta: Array.from({ length: 30 }, (_, index) => `LIVE_OVERFLOW_${index}`).join("\n"),
        },
      },
    },
  });

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");

  assert.match(output, /LIVE_OVERFLOW_29/);
  assert.match(output, /Active[\s\S]*回车输入/);
  assert.match(output, /LIVE_OVERFLOW_28[\s\S]*LIVE_OVERFLOW_29[\s\S]*Active/);
});

test("draw renders chrome in reserved scrollback tail when cache fills the viewport", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: Array.from({ length: 5 }, (_, index) => ({
      id: `msg-cache-almost-full-${index}`,
      sessionID: "sess-1",
      role: "assistant" as const,
      parts: [
        {
          id: `part-cache-almost-full-${index}`,
          type: "text" as const,
          text: `CACHE_ALMOST_FULL_MARKER_${index}`,
        },
      ],
    })),
    permissions: [],
    sessions: [activeSession],
  });

  const parts = renderChatFrameParts(state, richCapabilities());
  const cacheLineCount = parts.cacheFrame ? parts.cacheFrame.split("\n").length : 0;
  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");

  assert.equal(cacheLineCount, 19);
  assert.match(output, /CACHE_ALMOST_FULL_MARKER_4/);
  assert.match(output, /Active[\s\S]*回车输入/);
  assert.match(output, /\r\n/, "chrome reservation must add blank scrollback rows");
});

test("draw appends reservation rows so live and chrome remain visible after full cache", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: Array.from({ length: 30 }, (_, index) => ({
      id: `msg-cache-fill-${index}`,
      sessionID: "sess-1",
      role: "assistant" as const,
      parts: [
        {
          id: `part-cache-fill-${index}`,
          type: "text" as const,
          text: `CACHE_FILL_MARKER_${index}`,
        },
      ],
    })),
    permissions: [],
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    draw(
      reducer(state, {
        type: "event",
        event: {
          directory: "C:/repo",
          payload: {
            type: "message.part.delta",
            properties: {
              session_id: "sess-1",
              message_id: "msg-cache-fill-live",
              part_id: "part-cache-fill-live",
              field: "text",
              delta: "FULL_CACHE_LIVE_MARKER",
            },
          },
        },
      }),
      richCapabilities(),
      previous,
    );
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /CACHE_FILL_MARKER_0/);
  assert.match(output, /\r\n/, "live/chrome reservation must extend scrollback");
  assert.match(output, /FULL_CACHE_LIVE_MARKER/);
  assert.match(output, /Active[\s\S]*回车输入/);
});

test("resize draw gate paints the entry snapshot and freezes until resize settles", () => {
  let drawCount = 0;
  let clearPendingCount = 0;
  let clearTimerCount = 0;
  let timeoutCallback: (() => void) | undefined;
  const gate = createResizeDrawGate({
    drawNow: () => {
      drawCount += 1;
    },
    clearPendingDraw: () => {
      clearPendingCount += 1;
    },
    resizePauseMs: 50,
    setTimeoutFn: (callback) => {
      timeoutCallback = callback;
      return 1 as unknown as ReturnType<typeof setTimeout>;
    },
    clearTimeoutFn: () => {
      clearTimerCount += 1;
    },
  });

  gate.enterResize();
  assert.equal(drawCount, 1, "entering resize must draw the current snapshot once");
  assert.equal(clearPendingCount, 1, "entering resize must clear pending stream redraws");
  assert.equal(gate.isFrozen(), true);

  gate.enterResize();
  assert.equal(drawCount, 1, "ongoing resize must not redraw cache/live/chrome");
  assert.equal(clearPendingCount, 2, "ongoing resize keeps pending stream redraws suppressed");
  assert.equal(clearTimerCount, 1, "ongoing resize extends the resize-settled timer");

  timeoutCallback?.();
  assert.equal(gate.isFrozen(), false);
  assert.equal(drawCount, 2, "settled resize must draw the latest state once");

  gate.enterResize();
  assert.equal(drawCount, 3, "a later resize starts a new frozen window");
  gate.dispose();
  assert.equal(gate.isFrozen(), false);
});

test("terminal resize handler enters resize freeze on every size change", () => {
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: 80 });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 20 });
  try {
    const actions: AppAction[] = [];
    let resizeCount = 0;
    let heightResizeCount = 0;
    const state = { ...initialState("C:/repo"), notice: "resize notice" };
    const onResize = createTerminalResizeHandler(
      () => state,
      (action) => actions.push(action),
      {
        onResize: () => (resizeCount += 1),
        onHeightResize: () => (heightResizeCount += 1),
      },
    );

    Object.defineProperty(process.stdout, "rows", { configurable: true, value: 40 });
    onResize();
    assert.deepEqual(actions, []);
    assert.equal(resizeCount, 1);
    assert.equal(heightResizeCount, 1);

    Object.defineProperty(process.stdout, "columns", { configurable: true, value: 100 });
    onResize();
    assert.deepEqual(actions, [{ type: "notice", value: "resize notice" }]);
    assert.equal(resizeCount, 2);
    assert.equal(heightResizeCount, 1);

    Object.defineProperty(process.stdout, "rows", { configurable: true, value: 24 });
    onResize();
    assert.deepEqual(actions, [{ type: "notice", value: "resize notice" }]);
    assert.equal(resizeCount, 3);
    assert.equal(heightResizeCount, 2);

    onResize();
    assert.equal(resizeCount, 3);
    assert.equal(heightResizeCount, 2);
  } finally {
    restoreProperty(process.stdout, "columns", columns);
    restoreProperty(process.stdout, "rows", rows);
  }
});

test("draw rewrites streaming chat updates without clearing terminal scrollback", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-plain-chat",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-plain-chat", type: "text", text: "plain transcript" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, plainCapabilities(), "");
    const previousLiveRegion = renderChatFrameParts(state, plainCapabilities()).liveRegionCursor;
    writes.length = 0;
    draw(
      reducer(state, {
        type: "event",
        event: {
          directory: "C:/repo",
          payload: {
            type: "message.part.delta",
            properties: {
              session_id: "sess-1",
              message_id: "msg-plain-stream",
              part_id: "part-plain-stream",
              field: "text",
              delta: "streaming",
            },
          },
        },
      }),
      plainCapabilities(),
      previous,
    );
    assert.ok(previousLiveRegion);
    assertMutableRegionClearedBefore(writes.join(""), previousLiveRegion.row, "streaming");
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.doesNotMatch(output, /plain transcript/);
  assert.match(output, /^\x1b\[\?25l/);
  assert.match(output, /streaming[\s\S]*Active[\s\S]*回车输入/);
  assert.match(output, /\r\n/, "streaming redraw may append reservation rows");
  assert.doesNotMatch(output, /\x1b\[999;1H/);
  assert.doesNotMatch(output, /\x1b7|\x1b8/);
  assert.match(output, /\x1b\[\d+(?:;\d+)?[HG]\x1b\[\?25h$/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.match(output, /streaming/);
});

test("draw commits completed rendered live rows while appending only blank reservation rows", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-line-commit",
          part_id: "part-line-commit",
          field: "text",
          delta: `LIVE_WRAP_START ${"a".repeat(55)}`,
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, plainCapabilities(), "");
    writes.length = 0;
    state = reducer(state, {
      type: "event",
      event: {
        directory: "C:/repo",
        payload: {
          type: "message.part.delta",
          properties: {
            session_id: "sess-1",
            message_id: "msg-line-commit",
            part_id: "part-line-commit",
            field: "text",
            delta: " LIVE_WRAP_SECOND",
          },
        },
      },
    });
    draw(state, plainCapabilities(), previous);
  });
  const output = writes.join("");
  const clearIndex = output.search(liveRegionClearPattern(0));
  const blankAppendIndex = output.indexOf("\x1b[u\r\n");
  const committedIndex = output.indexOf("LIVE_WRAP_START");
  const pendingIndex = output.indexOf("LIVE_WRAP_SECOND");
  const chromeIndex = output.indexOf("Active", pendingIndex);
  const pendingAnchor = lastAbsoluteCursorBefore(output, pendingIndex);

  assert.equal(output.includes(terminalClear), false);
  assert.ok(clearIndex >= 0, "old live/chrome reservation must be cleared first");
  assert.ok(
    committedIndex > clearIndex,
    "the completed rendered live row must be materialized after clearing",
  );
  assert.ok(
    blankAppendIndex > committedIndex,
    "growing live/chrome must append blank reservation rows after materializing completed rows",
  );
  assert.doesNotMatch(
    output.slice(clearIndex, blankAppendIndex),
    /LIVE_WRAP_SECOND/,
    "materialized scrollback rows must not contain the active live row",
  );
  assert.ok(pendingAnchor, "active live text must be painted with an absolute overlay cursor");
  assert.ok(pendingIndex > blankAppendIndex, "active live row must be overlaid after blank rows");
  assert.ok(chromeIndex > pendingIndex, "chrome must still render after pending live");
});

test("draw keeps a single active live content row mutable until the next content row starts", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-partial-live-row",
          part_id: "part-partial-live-row",
          field: "text",
          delta: "PARTIAL_LIVE_ROW",
        },
      },
    },
  });

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");
  const markerIndex = output.indexOf("PARTIAL_LIVE_ROW");
  const saveCursorIndex = output.indexOf("\x1b[s");

  assert.ok(markerIndex >= 0, "partial live row must be rendered");
  assert.ok(saveCursorIndex >= 0, "chat draw must save the scrollback cursor before overlays");
  assert.ok(
    markerIndex > saveCursorIndex,
    "partial live row must stay in the mutable overlay until another content row exists",
  );
});

test("draw keeps only the active rendered command row mutable", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-running-command-live",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-running-command-live",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run slow",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    draw({ ...state, thinkingFrame: state.thinkingFrame + 1 }, richCapabilities(), previous);
  });
  const output = writes.join("");
  const commandIndex = output.indexOf("npm run slow");
  const clearIndex = output.search(liveRegionClearPattern(0));

  assert.equal(output.includes(terminalClear), false);
  assert.ok(clearIndex >= 0, "running command redraw must clear only the mutable region");
  assert.ok(commandIndex > clearIndex, "running command must be redrawn as mutable live output");
  assert.equal(
    output.includes("\x1b[u\r\n"),
    false,
    "unchanged active rows must not be appended again",
  );
});

test("draw materializes message gap before a running command starts", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let liveTextState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [],
    permissions: [],
    sessions: [busySession],
  });
  liveTextState = reducer(liveTextState, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-command-gap",
          part_id: "part-command-gap-text",
          field: "text",
          delta: "COMMAND_GAP_PREFACE",
        },
      },
    },
  });
  const commandState = reducer(liveTextState, {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-command-gap",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-command-gap-text",
            type: "text",
            text: "COMMAND_GAP_PREFACE",
          },
          {
            id: "part-command-gap-run",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run command-gap",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(liveTextState, plainCapabilities(), "");
    writes.length = 0;
    draw(commandState, plainCapabilities(), previous);
  });
  const output = writes.join("");
  const markerIndex = output.indexOf("COMMAND_GAP_PREFACE");
  const commandIndex = output.indexOf("npm run command-gap");
  const materializedGapIndex = output.indexOf(
    "\x1b[2K",
    markerIndex + "COMMAND_GAP_PREFACE".length,
  );

  assert.ok(markerIndex >= 0, "message text must be materialized before the command");
  assert.ok(
    materializedGapIndex > markerIndex,
    "the message/command separator gap must be materialized into cache",
  );
  assert.ok(
    commandIndex > materializedGapIndex,
    "running command must render after the cached gap",
  );
  assert.equal(output.includes(terminalClear), false);
});

test("draw materializes a running command row once a following live row starts", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const commandMessage = {
    id: "msg-command-before-followup",
    sessionID: "sess-1",
    role: "assistant" as const,
    created_at: 1_000,
    parts: [
      {
        id: "part-command-before-followup",
        type: "tool",
        tool: "command_run",
        state: {
          status: "running",
          input: {
            commands: [
              {
                step: 1,
                command_type: "shell_command",
                command_line: "npm run long-lived",
              },
            ],
          },
        },
      },
    ],
  };
  const runningState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [commandMessage],
    permissions: [],
    sessions: [busySession],
  });
  const followedState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      commandMessage,
      {
        id: "msg-following-command",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 1_001,
        parts: [
          {
            id: "part-following-command",
            type: "text",
            text: "FOLLOWING_ASSISTANT_CONTENT",
          },
        ],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(runningState, richCapabilities(), "");
    writes.length = 0;
    draw(followedState, richCapabilities(), previous);
  });
  const output = writes.join("");
  const clearIndex = output.search(liveRegionClearPattern(0));
  const commandIndex = output.indexOf("npm run long-lived");
  const followingIndex = output.indexOf("FOLLOWING_ASSISTANT_CONTENT");

  assert.equal(output.includes(terminalClear), false);
  assert.ok(clearIndex >= 0, "old live/chrome reservation must be cleared first");
  assert.ok(commandIndex > clearIndex, "running command must render after the mutable clear");
  assert.ok(followingIndex > commandIndex, "following content must render after the command block");
  assert.match(
    output,
    /\x1b\[\d+;1H\x1b\[2K(?:(?!\x1b\[\d+;1H)[\s\S])*npm run long-lived/u,
    "running command rows may be materialized once a later rendered row exists",
  );
  assert.doesNotMatch(
    output,
    /\x1b\[\d+;1H\x1b\[2K(?:(?!\x1b\[\d+;1H)[\s\S])*FOLLOWING_ASSISTANT_CONTENT/u,
    "following content must stay mutable until another content row starts",
  );
  assert.equal(regexCount(output, /npm run long-lived/gu), 1);
  assert.equal(regexCount(output, /FOLLOWING_ASSISTANT_CONTENT/gu), 1);
});

test("draw materializes completed command calls even when a background command still runs", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const runningState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-background-command",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-background-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run dev",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  const completedState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-background-command",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-background-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run dev",
                  },
                ],
              },
              output: {
                streamed_command_run_result: {
                  results: [
                    {
                      step: 1,
                      status: "running",
                      command_type: "shell_command",
                      command_line: "npm run dev",
                    },
                  ],
                },
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(runningState, richCapabilities(), "");
    writes.length = 0;
    draw(completedState, richCapabilities(), previous);
  });
  const output = writes.join("");
  const commandIndex = output.indexOf("npm run dev");
  const chromeIndex = output.indexOf("Active", commandIndex);

  assert.equal(output.includes(terminalClear), false);
  assert.ok(commandIndex >= 0, "completed command call must be materialized");
  assert.ok(chromeIndex > commandIndex, "chrome must render after the materialized command block");
  assert.match(output, /shell_command running[\s\S]*npm run dev/);
});

test("draw materializes finalized live as new cache without repainting fixed cache", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const idleSession = { ...activeSession, status: "idle" as const };
  const baseMessages = [
    {
      id: "msg-fixed-before-live",
      sessionID: "sess-1",
      role: "assistant" as const,
      parts: [{ id: "part-fixed-before-live", type: "text", text: "FIXED_CACHE_MARKER" }],
    },
    {
      id: "msg-live-user",
      sessionID: "sess-1",
      role: "user" as const,
      parts: [{ id: "part-live-user", type: "text", text: "LIVE_USER_MARKER" }],
    },
  ];
  let liveState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: baseMessages,
    permissions: [],
    sessions: [busySession],
  });
  liveState = reducer(liveState, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-live-assistant",
          part_id: "part-live-assistant",
          field: "text",
          delta: "LIVE_STREAM_MARKER",
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(liveState, richCapabilities(), "");
    const previousParts = renderChatFrameParts(liveState, richCapabilities());
    const previousLiveRegion = previousParts.liveRegionCursor;
    writes.length = 0;
    const finalizedState = reducer(liveState, {
      type: "hydrate",
      session: idleSession,
      messages: [
        ...baseMessages,
        {
          id: "msg-live-assistant",
          sessionID: "sess-1",
          role: "assistant",
          parts: [
            {
              id: "part-live-assistant",
              type: "text",
              text: "LIVE_STREAM_MARKER",
            },
          ],
        },
      ],
      permissions: [],
      sessions: [idleSession],
    });
    draw(finalizedState, richCapabilities(), previous);
    assert.ok(previousLiveRegion);
    assertMutableRegionClearedBefore(writes.join(""), previousLiveRegion.row, "LIVE_STREAM_MARKER");
  });
  const output = writes.join("");
  const finalizedIndex = output.indexOf("LIVE_STREAM_MARKER");
  const chromeIndex = output.indexOf("Active", finalizedIndex);

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /FIXED_CACHE_MARKER/);
  assert.doesNotMatch(output, /LIVE_USER_MARKER/);
  assert.ok(finalizedIndex >= 0, "finalized live text must be written as new cache");
  assert.ok(chromeIndex > finalizedIndex, "chrome must render after finalized cache text");
  assert.match(output, /Active[\s\S]*回车输入/);
});

function liveRegionClearPattern(cursorRow: number): RegExp {
  void cursorRow;
  return /\x1b\[\d+;1H\x1b\[J/u;
}

function lastAbsoluteCursorBefore(
  output: string,
  index: number,
): { row: number; column: number } | undefined {
  if (index < 0) return undefined;
  let cursor: { row: number; column: number } | undefined;
  for (const match of output.slice(0, index).matchAll(/\x1b\[(\d+);(\d+)H/gu)) {
    cursor = { row: Number(match[1]), column: Number(match[2]) };
  }
  return cursor;
}

function assertMutableRegionClearedBefore(output: string, cursorRow: number, marker: string): void {
  const clearMatch = output.match(liveRegionClearPattern(cursorRow));
  assert.ok(clearMatch?.index !== undefined, "live/chrome rewrite must clear mutable region first");
  const markerIndex = output.indexOf(marker);
  assert.ok(markerIndex >= 0, `expected rewritten output to include ${marker}`);
  assert.ok(
    clearMatch.index < markerIndex,
    "live/chrome content must be written only after clearing the old mutable region",
  );
}

function captureDrawWrites(fn: (writes: string[]) => void): string[] {
  resetDrawState();
  const writes: string[] = [];
  const isTTY = Object.getOwnPropertyDescriptor(process.stdout, "isTTY");
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  const write = Object.getOwnPropertyDescriptor(process.stdout, "write");
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: 80 });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 20 });
  Object.defineProperty(process.stdout, "write", {
    configurable: true,
    value: (chunk: string | Uint8Array): boolean => {
      writes.push(typeof chunk === "string" ? chunk : chunk.toString());
      return true;
    },
  });
  try {
    fn(writes);
    return writes;
  } finally {
    resetDrawState();
    restoreProperty(process.stdout, "isTTY", isTTY);
    restoreProperty(process.stdout, "columns", columns);
    restoreProperty(process.stdout, "rows", rows);
    restoreProperty(process.stdout, "write", write);
  }
}

async function captureDrawWritesAsync(fn: (writes: string[]) => Promise<void>): Promise<string[]> {
  resetDrawState();
  const writes: string[] = [];
  const isTTY = Object.getOwnPropertyDescriptor(process.stdout, "isTTY");
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  const write = Object.getOwnPropertyDescriptor(process.stdout, "write");
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: 80 });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 20 });
  Object.defineProperty(process.stdout, "write", {
    configurable: true,
    value: (chunk: string | Uint8Array): boolean => {
      writes.push(typeof chunk === "string" ? chunk : chunk.toString());
      return true;
    },
  });
  try {
    await fn(writes);
    return writes;
  } finally {
    resetDrawState();
    restoreProperty(process.stdout, "isTTY", isTTY);
    restoreProperty(process.stdout, "columns", columns);
    restoreProperty(process.stdout, "rows", rows);
    restoreProperty(process.stdout, "write", write);
  }
}

function restoreProperty<T extends object>(
  target: T,
  key: keyof T,
  descriptor: PropertyDescriptor | undefined,
): void {
  if (descriptor) Object.defineProperty(target, key, descriptor);
  else Reflect.deleteProperty(target, key);
}

function regexCount(text: string, pattern: RegExp): number {
  return Array.from(text.matchAll(pattern)).length;
}

function stateHarness(initial: AppState): {
  getState: () => AppState;
  dispatch: (action: AppAction) => void;
} {
  let state = initial;
  return {
    getState: () => state,
    dispatch: (action) => {
      state = reducer(state, action);
    },
  };
}
