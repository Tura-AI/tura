import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "./reducer.js";

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

test("reducer applies message and part replay events idempotently", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.updated",
        properties: {
          sessionID: "sess-1",
          info: {
            id: "msg-1",
            sessionID: "sess-1",
            role: "assistant",
            parts: [{ id: "part-1", type: "text", text: "hello" }],
          },
        },
      },
    },
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.updated",
        properties: {
          sessionID: "sess-1",
          part: {
            id: "tool-1",
            sessionID: "sess-1",
            messageID: "msg-1",
            type: "tool",
            tool: "runtime",
            state: { status: "completed", output: { text: "done" } },
          },
        },
      },
    },
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.updated",
        properties: {
          sessionID: "sess-1",
          part: {
            id: "tool-1",
            sessionID: "sess-1",
            messageID: "msg-1",
            type: "tool",
            tool: "runtime",
            state: { status: "completed", output: { text: "done again" } },
          },
        },
      },
    },
  });

  assert.equal(state.messages.length, 1);
  assert.equal(state.messages[0].parts.length, 2);
  assert.equal(state.messages[0].parts.find((part) => part.id === "tool-1")?.tool, "runtime");
});

test("reducer keeps streaming deltas that arrive before full message hydration", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-stream",
          part_id: "part-stream",
          field: "text",
          delta: "hel",
        },
      },
    },
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "msg-stream",
          part_id: "part-stream",
          field: "text",
          delta: "lo",
        },
      },
    },
  });

  assert.equal(state.messages[0].id, "msg-stream");
  assert.equal(state.messages[0].parts[0].text, "hello");
});

test("reducer updates permission and question requests from events", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "permission.asked",
        properties: { permission: { id: "perm-1", sessionID: "sess-1", permission: "shell" } },
      },
    },
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "question.asked",
        properties: { question: { id: "q-1", sessionID: "sess-1", question: "Proceed?" } },
      },
    },
  });

  assert.equal(state.permissions.length, 1);
  assert.equal(state.questions.length, 1);

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "permission.replied",
        properties: { permission: { id: "perm-1", sessionID: "sess-1", permission: "shell" } },
      },
    },
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "question.rejected",
        properties: { question: { id: "q-1", sessionID: "sess-1", question: "Proceed?" } },
      },
    },
  });

  assert.equal(state.permissions.length, 0);
  assert.equal(state.questions.length, 0);
});

test("reducer keeps session metadata from gateway events", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    sessions: [session],
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "session.updated",
        properties: {
          sessionID: "sess-1",
          info: {
            ...session,
            session_display_name: "Updated Session",
            agent: "fast",
          },
        },
      },
    },
  });

  assert.equal(state.session?.session_display_name, "Updated Session");
  assert.equal(state.session?.agent, "fast");
});
