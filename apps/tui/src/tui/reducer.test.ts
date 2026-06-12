import assert from "node:assert/strict";
import test from "node:test";
import { messageText } from "../types/session.js";
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
          delta: "命令完成\r\x1b[2K新的回复",
        },
      },
    },
  });

  assert.equal(state.messages[0].parts[0].text, "命令完成\n新的回复");
});

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

  assert.equal(state.messages.length, 2);
  assert.equal(
    state.messages.find((message) => message.id === "msg-stream-runtime-1")?.parts[0].text,
    "streaming reply",
  );
});

test("reducer keeps streamed assistant text when final hydrate only has task_status text", () => {
  const userMessage = {
    id: "msg-user-task-status",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-task-status", type: "text", text: "推荐吃什么" }],
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
          delta: "可以吃牛肉面。",
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

  const assistant = state.messages.find((message) => message.id === "msg-stream-runtime-1");
  assert.equal(assistant?.parts[0].text, "可以吃牛肉面。");
  assert.equal(messageText(assistant!), "可以吃牛肉面。");
});

test("reducer keeps streamed runtime response when final hydrate omits the visible reply", () => {
  const input = "你好";
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
          delta: "你好，我在。",
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

  const visibleResponses = state.messages
    .filter((message) => message.role !== "user")
    .map((message) => messageText(message).trim())
    .filter(Boolean);
  assert.deepEqual(visibleResponses, ["你好，我在。"]);
  assert.notEqual(visibleResponses[0], input);
});

test("reducer keeps current visible agent text even when the message id is already durable-shaped", () => {
  const input = "你好";
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
          delta: "你好，已经看到你的消息。",
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

  const visibleResponses = state.messages
    .filter((message) => message.role !== "user")
    .map((message) => messageText(message).trim())
    .filter(Boolean);
  assert.deepEqual(visibleResponses, ["你好，已经看到你的消息。"]);
  assert.notEqual(visibleResponses[0], input);
});

test("reducer keeps current visible text when final hydrate also includes durable assistant text", () => {
  const userMessage = {
    id: "msg-user-durable",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-durable", type: "text", text: "你好" }],
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

  const visibleResponses = state.messages
    .filter((message) => message.role !== "user")
    .map((message) => messageText(message).trim())
    .filter(Boolean);
  assert.deepEqual(visibleResponses, ["stream copy", "durable copy"]);
});

test("reducer keeps command updates under the original assistant reply when completion lacks created_at", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-user",
        sessionID: "sess-1",
        role: "user",
        created_at: 1,
        parts: [{ id: "part-user", type: "text", text: "Say hello" }],
      },
      {
        id: "msg-agent",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 2,
        parts: [{ id: "part-agent", type: "text", text: "你好，马上处理。" }],
      },
      {
        id: "msg-command",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 3,
        parts: [
          {
            id: "part-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: { command_line: '{"status":"done","task_detail":"Greeting answered"}' },
            },
          },
        ],
      },
      {
        id: "msg-final",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 4,
        parts: [{ id: "part-final", type: "text", text: "处理完了。" }],
      },
    ],
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
            id: "msg-command",
            sessionID: "sess-1",
            role: "assistant",
            updated_at: 100,
            parts: [
              {
                id: "part-command",
                type: "tool",
                tool: "command_run",
                state: {
                  status: "completed",
                  output: '{"status":"done","task_detail":"Greeting answered"}',
                },
              },
            ],
          },
        },
      },
    },
  });

  assert.deepEqual(
    state.messages.map((message) => message.id),
    ["msg-user", "msg-agent", "msg-command", "msg-final"],
  );
  assert.equal(state.messages[2].created_at, 3);
});

test("reducer appends later streamed assistant replies after command results", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-user",
        sessionID: "sess-1",
        role: "user",
        created_at: 1,
        parts: [{ id: "part-user", type: "text", text: "Run acceptance" }],
      },
      {
        id: "msg-agent",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 1.5,
        parts: [{ id: "part-agent", type: "text", text: "I will run it now." }],
      },
      {
        id: "msg-command",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 3,
        parts: [
          {
            id: "part-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: '{"status":"done","task_detail":"Acceptance"}',
            },
          },
        ],
      },
    ],
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
          message_id: "msg-final",
          part_id: "part-final",
          field: "text",
          delta: "Acceptance passed. Final marker is visible.",
        },
      },
    },
  });

  assert.deepEqual(
    state.messages.map((message) => message.id),
    ["msg-user", "msg-agent", "msg-command", "msg-final"],
  );
  assert.equal(messageText(state.messages.at(-1)!).trim(), "Acceptance passed. Final marker is visible.");
});

test("reducer orders assistant text before command parts even when tool arrives first", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-agent",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-command",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: '{"status":"done"}' },
          },
          { id: "part-text", type: "text", text: "已经问好了。" },
        ],
      },
    ],
    permissions: [],
  });

  assert.equal(state.messages[0].parts[0].id, "part-text");
  assert.equal(state.messages[0].parts[1].id, "part-command");
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
