import assert from "node:assert/strict";
import test from "node:test";
import { messageText } from "../../../src/types/session.js";
import { displayMessages, initialState, reducer } from "../../../src/tui/reducer.js";

const session = {
  id: "sess-1",
  title: "Work",
  directory: "C:/repo",
  status: "idle" as const,
};

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

  assert.equal(state.messages.length, 0);
  assert.equal(Object.values(state.liveStreams)[0]?.text, "hello");
  assert.equal(displayMessages(state)[0].id, "msg-stream");
  assert.equal(displayMessages(state)[0].parts[0].text, "hello");
});

test("reducer ignores message deltas without a session for an active session", () => {
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
          message_id: "msg-unknown-session",
          part_id: "part-unknown-session",
          field: "text",
          delta: "must not leak into the active chat",
        },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.deepEqual(displayMessages(state), []);
});

test("reducer ignores part updates without a session for an active session", () => {
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
        type: "message.part.updated",
        properties: {
          part: {
            id: "part-unknown-session",
            messageID: "msg-unknown-session",
            type: "text",
            text: "must not create an active-session message",
          },
        },
      },
    },
  });

  assert.deepEqual(state.messages, []);
});

test("reducer clears sessionless live stream when its durable message arrives", () => {
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
      sessionID: "sess-1",
      payload: {
        type: "message.part.delta",
        properties: {
          message_id: "msg-sessionless-stream",
          part_id: "part-sessionless-stream",
          field: "text",
          delta: "duplicated live text",
        },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 1);

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.updated",
        properties: {
          session_id: "sess-1",
          info: {
            id: "msg-sessionless-stream",
            sessionID: "sess-1",
            role: "assistant",
            created_at: 2,
            parts: [
              {
                id: "part-final-durable",
                type: "text",
                text: "duplicated live text",
              },
            ],
          },
        },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.equal(messageText(displayMessages(state)[0]), "duplicated live text");
  assert.equal(
    displayMessages(state)[0].parts.length,
    1,
    "durable final text must replace, not sit next to, sessionless live text",
  );
});

test("reducer overlays later deltas on durable part text without mutating history", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-durable-part",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 1,
        parts: [{ id: "part-durable", type: "text", text: "hello " }],
      },
    ],
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
          message_id: "msg-durable-part",
          part_id: "part-durable",
          field: "text",
          delta: "world",
        },
      },
    },
  });

  assert.equal(messageText(state.messages[0]), "hello ");
  assert.equal(messageText(displayMessages(state)[0]), "hello world");
});

test("reducer normalizes streamed agent terminal controls into plain text", () => {
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
          delta: "command complete\r\x1b[2Knew reply",
        },
      },
    },
  });

  assert.equal(state.messages.length, 0);
  assert.equal(displayMessages(state)[0].parts[0].text, "command complete\nnew reply");
});
