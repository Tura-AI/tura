import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";

const session = {
  id: "sess-1",
  title: "Work",
  directory: "C:/repo",
  status: "idle" as const,
};

test("reducer hydrates durable gateway state", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    sessions: [session],
  });

  assert.equal(state.session?.id, "sess-1");
  assert.equal(state.sessions.length, 1);
});

test("reducer wraps settings selection at panel edges", () => {
  let state = reducer(initialState("C:/repo"), { type: "select-settings", delta: -1 });
  assert.equal(state.selectedSettingsIndex, 7);

  state = reducer(state, { type: "select-settings", delta: 1 });
  assert.equal(state.selectedSettingsIndex, 0);

  state = reducer(state, { type: "select-settings", delta: 7 });
  assert.equal(state.selectedSettingsIndex, 7);

  state = reducer(state, { type: "select-settings", delta: 1 });
  assert.equal(state.selectedSettingsIndex, 0);
});

test("reducer includes create-new-session in session picker selection", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "sessions",
    value: [session],
    open: true,
  });
  assert.equal(state.selectedSessionIndex, 0);

  state = reducer(state, { type: "select-session", delta: 1 });
  assert.equal(state.selectedSessionIndex, 1);

  state = reducer(state, { type: "select-session", delta: 1 });
  assert.equal(state.selectedSessionIndex, 0);
});

test("reducer keeps session picker selection during active-session polling hydrate", () => {
  const other = {
    id: "sess-2",
    title: "Other",
    directory: "C:/repo",
    status: "idle" as const,
    updated_at: 2,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    sessions: [session, other],
  });

  state = reducer(state, { type: "sessions", value: [session, other], open: true });
  state = reducer(state, { type: "select-session", delta: 1 });
  assert.equal(state.selectedSessionIndex, 1);

  state = reducer(state, {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    sessions: [session, other],
  });

  assert.equal(state.sessionsOpen, true);
  assert.equal(state.selectedSessionIndex, 1);
});

test("reducer clears old transcript when hydrating a different session", () => {
  const nextSession = {
    id: "sess-2",
    title: "New Session",
    directory: "C:/repo",
    status: "idle" as const,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-old-user",
        sessionID: "sess-1",
        role: "user",
        created_at: 1,
        parts: [{ id: "part-old-user", type: "text", text: "old prompt" }],
      },
      {
        id: "msg-old-assistant",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 2,
        parts: [{ id: "part-old-assistant", type: "text", text: "old reply" }],
      },
    ],
    permissions: [],
    sessions: [session, nextSession],
  });

  state = reducer(state, {
    type: "hydrate",
    session: nextSession,
    messages: [],
    permissions: [],
    sessions: [nextSession, session],
  });

  assert.equal(state.session?.id, "sess-2");
  assert.deepEqual(state.messages, []);
  assert.equal(state.selectedSessionIndex, 0);
});

test("reducer can hydrate a selected session and close panels atomically", () => {
  const nextSession = {
    id: "sess-2",
    title: "New Session",
    directory: "C:/repo",
    status: "idle" as const,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    sessions: [session, nextSession],
  });
  state = reducer(state, { type: "sessions", value: [nextSession, session], open: true });
  assert.equal(state.sessionsOpen, true);

  state = reducer(state, {
    type: "hydrate",
    session: nextSession,
    messages: [],
    permissions: [],
    sessions: [nextSession, session],
    closePanels: true,
  });

  assert.equal(state.session?.id, "sess-2");
  assert.equal(state.sessionsOpen, false);
  assert.equal(state.modelsOpen, false);
  assert.equal(state.authOpen, false);
  assert.equal(state.settingsOpen, false);
  assert.equal(state.personasOpen, false);
  assert.equal(state.help, false);
});

test("reducer ignores events for another workspace", () => {
  const hydrated = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
  });

  const next = reducer(hydrated, {
    type: "event",
    event: {
      directory: "D:/other",
      payload: {
        type: "message.updated",
        properties: {
          sessionID: "sess-1",
          info: {
            id: "msg-1",
            sessionID: "sess-1",
            role: "assistant",
            parts: [{ id: "part-1", type: "text", text: "ignored" }],
          },
        },
      },
    },
  });

  assert.equal(next.messages.length, 0);
});
