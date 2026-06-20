import assert from "node:assert/strict";
import test from "node:test";
import type { Session } from "../../../src/types/session.js";
import { messageText } from "../../../src/types/session.js";
import {
  createAndSelectSession,
  deleteSelectedSession,
  draw,
  forkSelectedSession,
  openSessionPicker,
  submitPrompt,
} from "../../../src/tui/app.js";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { clear as terminalClear } from "../../../src/tui/render-terminal.js";
import {
  activeSession,
  otherSession,
  captureDrawWritesAsync,
  regexCount,
  stateHarness,
} from "./helpers/app-harness.js";

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
