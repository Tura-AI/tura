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
          message_id: "runtime-stream.message",
          part_id: "runtime-stream.message",
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
          message_id: "runtime-stream.message",
          part_id: "runtime-stream.message",
          field: "text",
          delta: "lo",
        },
      },
    },
  });

  assert.equal(state.messages.length, 0);
  assert.equal(Object.values(state.liveStreams)[0]?.text, "hello");
  assert.equal(displayMessages(state)[0].id, "runtime-stream.message");
  assert.equal(displayMessages(state)[0].parts[0].text, "hello");
});

test("reducer keeps runtime text live while command parts update", () => {
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
          session_id: "sess-1",
          message_id: "runtime-command.message",
          part_id: "runtime-command.message",
          field: "text",
          delta: "checking files",
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
          session_id: "sess-1",
          part: {
            id: "runtime-command.tool.command_run",
            sessionID: "sess-1",
            messageID: "runtime-command.message",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: { commands: [{ command_type: "shell_command", command_line: "npm test" }] },
            },
          },
        },
      },
    },
  });

  const assistant = displayMessages(state).find(
    (message) => message.id === "runtime-command.message",
  );

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.equal(messageText(assistant!), "checking files");
  assert.equal(
    assistant?.parts.find((part) => part.id === "runtime-command.tool.command_run")?.tool,
    "command_run",
  );

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.updated",
        properties: {
          session_id: "sess-1",
          info: {
            id: "runtime-command.message",
            sessionID: "sess-1",
            role: "assistant",
            parts: [
              {
                id: "runtime-command.message",
                sessionID: "sess-1",
                messageID: "runtime-command.message",
                type: "text",
                text: "checking files",
              },
            ],
          },
        },
      },
    },
  });

  const stillLiveAssistant = displayMessages(state).find(
    (message) => message.id === "runtime-command.message",
  );

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.equal(messageText(stillLiveAssistant!), "checking files");
  assert.equal(
    stillLiveAssistant?.parts.find((part) => part.id === "runtime-command.tool.command_run")?.tool,
    "command_run",
  );
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

test("reducer keeps final live stream until cache snapshot confirms the message", () => {
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

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.equal(messageText(displayMessages(state)[0]), "duplicated live text");
  assert.equal(
    displayMessages(state)[0].parts.length,
    1,
    "event final text must not sit next to the frozen live text",
  );
  assert.equal(displayMessages(state)[0].parts[0].id, "part-sessionless-stream");

  state = reducer(state, {
    type: "messages-incremental",
    sessionID: "sess-1",
    messages: [
      {
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
    ],
  });

  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.equal(messageText(displayMessages(state)[0]), "duplicated live text");
  assert.equal(displayMessages(state)[0].parts.length, 1);
  assert.equal(displayMessages(state)[0].parts[0].id, "part-sessionless-stream");
});

test("reducer commits the last live command shape instead of the cache confirmation shape", () => {
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
          session_id: "sess-1",
          message_id: "runtime-live-order.message",
          part_id: "runtime-live-order.message",
          field: "text",
          delta: "checking",
        },
      },
    },
  });

  for (const [id, command_line] of [
    ["runtime-live-order.tool.command_run.1", "first-live-command"],
    ["runtime-live-order.tool.command_run.2", "second-live-command"],
  ]) {
    state = reducer(state, {
      type: "event",
      event: {
        directory: "C:/repo",
        payload: {
          type: "message.part.updated",
          properties: {
            session_id: "sess-1",
            part: {
              id,
              sessionID: "sess-1",
              messageID: "runtime-live-order.message",
              type: "tool",
              tool: "command_run",
              state: {
                status: "completed",
                input: { commands: [{ command_type: "shell_command", command_line }] },
              },
            },
          },
        },
      },
    });
  }

  state = reducer(state, {
    type: "messages-incremental",
    sessionID: "sess-1",
    messages: [
      {
        id: "runtime-live-order.message",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 2,
        updated_at: 3,
        parts: [
          {
            id: "runtime-live-order.message",
            sessionID: "sess-1",
            messageID: "runtime-live-order.message",
            type: "text",
            text: "checking",
          },
          {
            id: "runtime-live-order.tool.command_run.2",
            sessionID: "sess-1",
            messageID: "runtime-live-order.message",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [{ command_type: "shell_command", command_line: "second-db-command" }],
              },
            },
          },
          {
            id: "runtime-live-order.tool.command_run.1",
            sessionID: "sess-1",
            messageID: "runtime-live-order.message",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [{ command_type: "shell_command", command_line: "first-db-command" }],
              },
            },
          },
        ],
      },
    ],
  });

  const assistant = displayMessages(state)[0];
  const commands = assistant.parts
    .filter((part) => part.tool === "command_run")
    .map((part) =>
      (part.state as { input?: { commands?: Array<{ command_line?: string }> } }).input?.commands?.[0]
        ?.command_line,
    );

  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.deepEqual(commands, ["first-live-command", "second-live-command"]);
});

test("reducer buffers the next runtime live event until the previous runtime is cache-confirmed", () => {
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
          session_id: "sess-1",
          message_id: "runtime-a.message",
          part_id: "runtime-a.message",
          field: "text",
          delta: "A streamed",
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
            id: "runtime-a.message",
            sessionID: "sess-1",
            role: "assistant",
            created_at: 2,
            updated_at: 3,
            parts: [
              {
                id: "runtime-a.message",
                sessionID: "sess-1",
                messageID: "runtime-a.message",
                type: "text",
                text: "A durable",
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
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "runtime-b.message",
          part_id: "runtime-b.message",
          field: "text",
          delta: "B hidden",
        },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.equal(state.pendingLiveEvents.length, 1);
  assert.deepEqual(
    displayMessages(state).map((message) => message.id),
    ["runtime-a.message"],
  );
  assert.equal(messageText(displayMessages(state)[0]), "A streamed");

  state = reducer(state, {
    type: "messages-incremental",
    sessionID: "sess-1",
    messages: [
      {
        id: "runtime-a.message",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 2,
        updated_at: 3,
        parts: [
          {
            id: "runtime-a.message",
            sessionID: "sess-1",
            messageID: "runtime-a.message",
            type: "text",
            text: "A durable",
          },
        ],
      },
    ],
  });

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.equal(state.pendingLiveEvents.length, 0);
  assert.deepEqual(
    displayMessages(state).map((message) => message.id),
    ["runtime-a.message", "runtime-b.message"],
  );
  assert.equal(messageText(displayMessages(state)[0]), "A streamed");
  assert.equal(messageText(displayMessages(state)[1]), "B hidden");

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "runtime-b.message",
          part_id: "runtime-b.message",
          field: "text",
          delta: " and visible again",
        },
      },
    },
  });

  assert.equal(state.liveHandoffBarrier, undefined);
  assert.equal(state.pendingLiveEvents.length, 0);
  assert.equal(messageText(displayMessages(state)[1]), "B hidden and visible again");
});

test("reducer releases buffered runtime events when cache confirmation arrives as a replay event", () => {
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
          session_id: "sess-1",
          message_id: "runtime-a-confirmed-by-event.message",
          part_id: "runtime-a-confirmed-by-event.message",
          field: "text",
          delta: "A live",
        },
      },
    },
  });

  const runtimeAFinalEvent = {
    directory: "C:/repo",
    payload: {
      type: "message.updated",
      properties: {
        session_id: "sess-1",
        info: {
          id: "runtime-a-confirmed-by-event.message",
          sessionID: "sess-1",
          role: "assistant" as const,
          created_at: 2,
          updated_at: 3,
          parts: [
            {
              id: "runtime-a-confirmed-by-event.message",
              sessionID: "sess-1",
              messageID: "runtime-a-confirmed-by-event.message",
              type: "text",
              text: "A durable",
            },
          ],
        },
      },
    },
  };

  state = reducer(state, { type: "event", event: runtimeAFinalEvent });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "runtime-b-after-event-confirmation.message",
          part_id: "runtime-b-after-event-confirmation.message",
          field: "text",
          delta: "B waits",
        },
      },
    },
  });

  assert.equal(state.pendingLiveEvents.length, 1);
  assert.deepEqual(
    displayMessages(state).map((message) => message.id),
    ["runtime-a-confirmed-by-event.message"],
  );

  state = reducer(state, { type: "event", event: runtimeAFinalEvent });

  assert.equal(state.liveHandoffBarrier, undefined);
  assert.equal(state.pendingLiveEvents.length, 0);
  assert.deepEqual(
    displayMessages(state).map((message) => message.id),
    ["runtime-a-confirmed-by-event.message", "runtime-b-after-event-confirmation.message"],
  );
  assert.equal(messageText(displayMessages(state)[1]), "B waits");

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "sess-1",
          message_id: "runtime-b-after-event-confirmation.message",
          part_id: "runtime-b-after-event-confirmation.message",
          field: "text",
          delta: " and renders",
        },
      },
    },
  });

  assert.equal(messageText(displayMessages(state)[1]), "B waits and renders");
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
          message_id: "runtime-stream.message",
          part_id: "runtime-stream.message",
          field: "text",
          delta: "command complete\r\x1b[2Knew reply",
        },
      },
    },
  });

  assert.equal(state.messages.length, 0);
  assert.equal(displayMessages(state)[0].parts[0].text, "command complete\nnew reply");
});
