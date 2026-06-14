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

test("reducer preserves busy streamed assistant text across polling hydrate", () => {
  const userMessage = {
    id: "msg-user-1",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-1", type: "text", text: "go" }],
    created_at: 1,
    updated_at: 1,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [userMessage],
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
          message_id: "msg-stream-runtime-1",
          part_id: "part-stream-runtime-1",
          field: "text",
          delta: "streaming reply",
        },
      },
    },
  });

  state = reducer(state, {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [userMessage],
    permissions: [],
  });

  assert.equal(state.messages.length, 1);
  assert.equal(
    displayMessages(state).find((message) => message.id === "msg-stream-runtime-1")?.parts[0].text,
    "streaming reply",
  );
});

test("reducer keeps streamed assistant text when final hydrate only has task_status text", () => {
  const userMessage = {
    id: "msg-user-task-status",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-task-status", type: "text", text: "suggest a meal" }],
    created_at: 1,
    updated_at: 1,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [userMessage],
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
          message_id: "msg-stream-runtime-1",
          part_id: "part-stream-runtime-1",
          field: "text",
          delta: "Try a bowl of noodles.",
        },
      },
    },
  });

  state = reducer(state, {
    type: "hydrate",
    session: { ...session, status: "idle" },
    messages: [
      userMessage,
      {
        id: "msg-stream-runtime-1",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-stream-runtime-1", type: "text", text: "done: {}" }],
        created_at: 2,
        updated_at: 3,
      },
    ],
    permissions: [],
  });

  const assistant = displayMessages(state).find((message) => message.id === "msg-stream-runtime-1");
  assert.equal(assistant?.parts[0].text, "Try a bowl of noodles.");
  assert.equal(messageText(assistant!), "Try a bowl of noodles.");
});

test("reducer keeps streamed runtime response when final hydrate omits the visible reply", () => {
  const input = "hello";
  const userMessage = {
    id: "msg-user-greeting",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-greeting", type: "text", text: input }],
    created_at: 10,
    updated_at: 10,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [userMessage],
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
          message_id: "msg-stream-runtime-greeting",
          part_id: "part-stream-runtime-greeting",
          field: "text",
          delta: "Hello, I am here.",
        },
      },
    },
  });

  state = reducer(state, {
    type: "hydrate",
    session: { ...session, status: "idle" },
    messages: [userMessage],
    permissions: [],
  });

  const visibleResponses = displayMessages(state)
    .filter((message) => message.role !== "user")
    .map((message) => messageText(message).trim())
    .filter(Boolean);
  assert.deepEqual(visibleResponses, ["Hello, I am here."]);
  assert.notEqual(visibleResponses[0], input);
});

test("reducer keeps current visible agent text even when the message id is already durable-shaped", () => {
  const input = "hello";
  const userMessage = {
    id: "msg-user-official-stream",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-official-stream", type: "text", text: input }],
    created_at: 30,
    updated_at: 30,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [userMessage],
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
          message_id: "msg-agent-official-id",
          part_id: "part-agent-official-id",
          field: "text",
          delta: "Hello, I saw your message.",
        },
      },
    },
  });

  state = reducer(state, {
    type: "hydrate",
    session: { ...session, status: "idle" },
    messages: [userMessage],
    permissions: [],
  });

  const visibleResponses = displayMessages(state)
    .filter((message) => message.role !== "user")
    .map((message) => messageText(message).trim())
    .filter(Boolean);
  assert.deepEqual(visibleResponses, ["Hello, I saw your message."]);
  assert.notEqual(visibleResponses[0], input);
});

test("reducer replaces temporary streamed text when final hydrate includes durable assistant text", () => {
  const userMessage = {
    id: "msg-user-durable",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-durable", type: "text", text: "hello" }],
    created_at: 20,
    updated_at: 20,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [userMessage],
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
          message_id: "msg-stream-runtime-durable",
          part_id: "part-stream-runtime-durable",
          field: "text",
          delta: "stream copy",
        },
      },
    },
  });

  state = reducer(state, {
    type: "hydrate",
    session: { ...session, status: "idle" },
    messages: [
      userMessage,
      {
        id: "msg-durable-assistant",
        sessionID: "sess-1",
        role: "assistant" as const,
        parts: [{ id: "part-durable-assistant", type: "text", text: "durable copy" }],
        created_at: 21,
        updated_at: 21,
      },
    ],
    permissions: [],
  });

  const visibleResponses = displayMessages(state)
    .filter((message) => message.role !== "user")
    .map((message) => messageText(message).trim())
    .filter(Boolean);
  assert.deepEqual(visibleResponses, ["durable copy"]);
});

test("reducer does not duplicate temporary streamed text across repeated polling hydrates", () => {
  const userMessage = {
    id: "msg-user-refresh",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-refresh", type: "text", text: "refresh ordering" }],
    created_at: 100,
    updated_at: 100,
  };
  const commandMessage = {
    id: "msg-command-refresh",
    sessionID: "sess-1",
    role: "assistant" as const,
    parts: [
      {
        id: "part-command-refresh",
        type: "tool",
        tool: "command_run",
        state: { status: "completed", input: { command_line: "node refresh-check.mjs" } },
      },
    ],
    created_at: 101,
    updated_at: 101,
  };
  const durableMessage = {
    id: "msg-durable-refresh",
    sessionID: "sess-1",
    role: "assistant" as const,
    parts: [{ id: "part-durable-refresh", type: "text", text: "DURABLE_REFRESH_FINAL" }],
    created_at: 102,
    updated_at: 102,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [userMessage, commandMessage],
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
          message_id: "msg-temp-refresh",
          part_id: "part-temp-refresh",
          field: "text",
          delta: "TEMP_REFRESH_STREAM",
        },
      },
    },
  });

  for (let index = 0; index < 3; index += 1) {
    state = reducer(state, {
      type: "hydrate",
      session: { ...session, status: "idle" },
      messages: [userMessage, commandMessage, durableMessage],
      permissions: [],
    });
  }

  assert.deepEqual(
    state.messages.map((message) => message.id),
    ["msg-user-refresh", "msg-command-refresh", "msg-durable-refresh"],
  );
  const text = displayMessages(state)
    .map((message) => messageText(message))
    .join("\n");
  assert.equal(text.includes("TEMP_REFRESH_STREAM"), false);
  assert.equal(text.match(/DURABLE_REFRESH_FINAL/gu)?.length, 1);
});
