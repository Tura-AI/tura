import assert from "node:assert/strict";
import test from "node:test";
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
          sessionID: "sess-1",
          messageID: "runtime-1.message",
          partID: "runtime-1.message",
          createdAt: 1,
          updatedAt: 2,
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
          sessionID: "sess-1",
          messageID: "runtime-new-turn.message",
          partID: "runtime-new-turn.message",
          createdAt: 1,
          updatedAt: 2,
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
          sessionID: "sess-1",
          messageID: "runtime-side-event.message",
          partID: "runtime-side-event.message",
          createdAt: 1,
          updatedAt: 2,
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
          sessionID: "sess-1",
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

test("reducer commits active live stream when session becomes idle", () => {
  const userMessage = {
    id: "msg-user-idle-commit",
    sessionID: "sess-1",
    role: "user" as const,
    parts: [{ id: "part-user-idle-commit", type: "text", text: "finish this" }],
    created_at: 40,
    updated_at: 40,
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
          sessionID: "sess-1",
          messageID: "runtime-idle-commit.message",
          partID: "runtime-idle-commit.message",
          createdAt: 41,
          updatedAt: 41,
          field: "text",
          delta: "idle commit response",
        },
      },
    },
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "session.status",
        properties: { sessionID: "sess-1", updatedAt: 42, status: "idle" },
      },
    },
  });

  const assistant = state.messages.find((message) => message.id === "runtime-idle-commit.message");
  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.equal(assistant?.parts[0]?.text, "idle commit response");
});

test("reducer does not duplicate a live delta when its full part snapshot arrives", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
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
          sessionID: "sess-1",
          messageID: "runtime-snapshot.message",
          partID: "runtime-snapshot.message",
          createdAt: 50,
          updatedAt: 51,
          field: "text",
          delta: "streamed once",
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
          sessionID: "sess-1",
          info: {
            id: "runtime-snapshot.message",
            sessionID: "sess-1",
            role: "assistant",
            created_at: 50,
            updated_at: 52,
            parts: [
              {
                id: "runtime-snapshot.message",
                sessionID: "sess-1",
                messageID: "runtime-snapshot.message",
                type: "text",
                text: "streamed once",
              },
              {
                id: "runtime-snapshot.tool.command_run",
                sessionID: "sess-1",
                messageID: "runtime-snapshot.message",
                type: "tool",
                tool: "command_run",
                state: { status: "running" },
              },
            ],
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
          createdAt: 50,
          updatedAt: 52,
          part: {
            id: "runtime-snapshot.message",
            sessionID: "sess-1",
            messageID: "runtime-snapshot.message",
            type: "text",
            text: "streamed once",
          },
        },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.equal(
    displayMessages(state).find((message) => message.id === "runtime-snapshot.message")?.parts[0]
      .text,
    "streamed once",
  );

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "session.status",
        properties: { sessionID: "sess-1", updatedAt: 53, status: "idle" },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.equal(
    state.messages.find((message) => message.id === "runtime-snapshot.message")?.parts[0].text,
    "streamed once",
  );
});

test("reducer preserves a durable content prefix when a live part snapshot arrives", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [
      {
        id: "runtime-prefix.message",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 60,
        updated_at: 60,
        parts: [
          {
            id: "runtime-prefix.message",
            sessionID: "sess-1",
            messageID: "runtime-prefix.message",
            type: "text",
            content: "durable prefix ",
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
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "runtime-prefix.message",
          partID: "runtime-prefix.message",
          createdAt: 60,
          updatedAt: 61,
          field: "content",
          delta: "live suffix",
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
          createdAt: 60,
          updatedAt: 62,
          part: {
            id: "runtime-prefix.message",
            sessionID: "sess-1",
            messageID: "runtime-prefix.message",
            type: "text",
            content: "durable prefix live suffix",
            metadata: { phase: "complete" },
          },
        },
      },
    },
  });

  const durablePart = state.messages[0]?.parts[0];
  const visiblePart = displayMessages(state)[0]?.parts[0];
  assert.equal(durablePart?.content, "durable prefix ");
  assert.deepEqual(durablePart?.metadata, { phase: "complete" });
  assert.equal(visiblePart?.content, "durable prefix live suffix");
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
            sessionID: "sess-1",
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
