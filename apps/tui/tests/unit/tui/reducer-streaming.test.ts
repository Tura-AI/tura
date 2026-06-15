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
          message_id: "runtime-1.message",
          part_id: "runtime-1.message",
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
    displayMessages(state).find((message) => message.id === "runtime-1.message")?.parts[0].text,
    "streaming reply",
  );
});

test("reducer preserves live stream when polling hydrate includes unrelated assistant history", () => {
  const previousAssistant = {
    id: "msg-previous-assistant",
    sessionID: "sess-1",
    role: "assistant" as const,
    parts: [{ id: "part-previous-assistant", type: "text", text: "previous durable answer" }],
    created_at: 1,
    updated_at: 1,
  };
  const userMessage = {
    id: "msg-user-new-turn",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-new-turn", type: "text", text: "continue" }],
    created_at: 2,
    updated_at: 2,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [previousAssistant, userMessage],
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
          message_id: "runtime-new-turn.message",
          part_id: "runtime-new-turn.message",
          field: "text",
          delta: "new streamed answer",
        },
      },
    },
  });

  state = reducer(state, {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [previousAssistant, userMessage],
    permissions: [],
  });

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.equal(
    displayMessages(state).find((message) => message.id === "runtime-new-turn.message")?.parts[0]
      .text,
    "new streamed answer",
  );
});

test("reducer preserves live stream when an unrelated durable assistant event arrives", () => {
  const userMessage = {
    id: "msg-user-side-event",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-side-event", type: "text", text: "work" }],
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
          message_id: "runtime-side-event.message",
          part_id: "runtime-side-event.message",
          field: "text",
          delta: "stream still visible",
        },
      },
    },
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.updated",
        properties: {
          session_id: "sess-1",
          info: {
            id: "msg-unrelated-assistant",
            sessionID: "sess-1",
            role: "assistant",
            parts: [{ id: "part-unrelated-assistant", type: "text", text: "side durable text" }],
            created_at: 2,
            updated_at: 2,
          },
        },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.equal(
    displayMessages(state).find((message) => message.id === "runtime-side-event.message")?.parts[0]
      .text,
    "stream still visible",
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
          message_id: "runtime-1.message",
          part_id: "runtime-1.message",
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
        id: "runtime-1.message",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "runtime-1.message", type: "text", text: "done: {}" }],
        created_at: 2,
        updated_at: 3,
      },
    ],
    permissions: [],
  });

  const assistant = displayMessages(state).find((message) => message.id === "runtime-1.message");
  assert.equal(assistant?.parts[0].text, "Try a bowl of noodles.");
  assert.equal(messageText(assistant!), "Try a bowl of noodles.");
  assert.equal(Object.values(state.liveStreams).length, 0);
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
          message_id: "runtime-greeting.message",
          part_id: "runtime-greeting.message",
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
  assert.equal(Object.values(state.liveStreams).length, 1);
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
  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.notEqual(visibleResponses[0], input);
});

test("reducer commits streamed text when final hydrate includes durable assistant text", () => {
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
          message_id: "runtime-durable.message",
          part_id: "runtime-durable.message",
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
        id: "runtime-durable.message",
        sessionID: "sess-1",
        role: "assistant" as const,
        parts: [{ id: "runtime-durable.message", type: "text", text: "durable copy" }],
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
  assert.deepEqual(visibleResponses, ["stream copy"]);
  assert.equal(Object.values(state.liveStreams).length, 0);
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
          message_id: "msg-durable-refresh",
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
    ["msg-user-refresh", "msg-durable-refresh", "msg-command-refresh"],
  );
  const text = displayMessages(state)
    .map((message) => messageText(message))
    .join("\n");
  assert.equal(text.includes("DURABLE_REFRESH_FINAL"), false);
  assert.equal(text.match(/TEMP_REFRESH_STREAM/gu)?.length, 1);
  assert.equal(Object.values(state.liveStreams).length, 0);
});

test("reducer keeps repeated runtime message and command callbacks in one message", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [],
    permissions: [],
  });
  const runtimeMessage = {
    id: "runtime-repeat.message",
    sessionID: "sess-1",
    role: "assistant" as const,
    parts: [
      { id: "runtime-repeat.message", type: "text", text: "checking" },
      {
        id: "runtime-repeat.tool.command_run",
        type: "tool",
        tool: "command_run",
        state: {
          status: "running",
          input: { commands: [{ command_type: "shell_command", command_line: "npm test" }] },
        },
      },
    ],
    created_at: 10,
    updated_at: 11,
  };

  for (const status of ["running", "completed"]) {
    state = reducer(state, {
      type: "event",
      event: {
        directory: "C:/repo",
        payload: {
          type: "message.updated",
          properties: {
            session_id: "sess-1",
            info: {
              ...runtimeMessage,
              updated_at: status === "completed" ? 12 : 11,
              parts: [
                runtimeMessage.parts[0],
                {
                  ...runtimeMessage.parts[1],
                  state: { ...runtimeMessage.parts[1].state, status },
                },
              ],
            },
          },
        },
      },
    });
  }

  const messages = displayMessages(state).filter(
    (message) => message.id === "runtime-repeat.message",
  );
  assert.equal(messages.length, 1);
  assert.equal(messages[0].parts.filter((part) => part.id === "runtime-repeat.message").length, 1);
  assert.equal(
    messages[0].parts.filter((part) => part.id === "runtime-repeat.tool.command_run").length,
    1,
  );
  assert.equal(
    (
      messages[0].parts.find((part) => part.id === "runtime-repeat.tool.command_run")?.state as
        | { status?: string }
        | undefined
    )?.status,
    "completed",
  );
});
