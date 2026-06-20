import assert from "node:assert/strict";
import test from "node:test";
import { messageText } from "../../../src/types/session.js";
import { displayMessages, initialState, reducer } from "../../../src/tui/reducer.js";
import { withNow } from "./helpers/render-harness.js";

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
            created_at: 10,
            updated_at: 10,
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
          createdAt: 10,
          updatedAt: 11,
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
          createdAt: 10,
          updatedAt: 12,
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
          sessionID: "sess-1",
          messageID: "runtime-stream.message",
          partID: "runtime-stream.message",
          createdAt: 1,
          updatedAt: 2,
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
          sessionID: "sess-1",
          messageID: "runtime-stream.message",
          partID: "runtime-stream.message",
          createdAt: 100,
          updatedAt: 101,
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
          sessionID: "sess-1",
          messageID: "runtime-command.message",
          partID: "runtime-command.message",
          createdAt: 100,
          updatedAt: 100,
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
          sessionID: "sess-1",
          createdAt: 100,
          updatedAt: 101,
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
          sessionID: "sess-1",
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
          messageID: "msg-unknown-session",
          partID: "part-unknown-session",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: "must not leak into the active chat",
        },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.deepEqual(displayMessages(state), []);
});

test("reducer merges command updates by command id and ignores stale event seq", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [],
    permissions: [],
  });

  const event = (status: string, eventSeq: number, result: unknown = null) => ({
    directory: "C:/repo",
    payload: {
      type: "command.updated" as const,
      properties: {
        sessionID: "sess-1",
        messageID: "runtime-command-id.message",
        partID: "runtime-command-id.tool.command_run",
        createdAt: 1,
        updatedAt: eventSeq,
        runtimeID: "runtime-command-id",
        commandRunID: "runtime-command-id.tool.command_run",
        commandID: "runtime-command-id.tool.command_run:call_1:0",
        providerToolCallID: "call_1",
        commandIndex: 0,
        eventSeq,
        status,
        command: {
          command_id: "runtime-command-id.tool.command_run:call_1:0",
          command_type: "shell_command",
          command_line: "npm test",
        },
        result,
      },
    },
  });

  state = reducer(state, { type: "event", event: event("running", 30) });
  state = reducer(state, {
    type: "event",
    event: event("completed", 40, {
      command_id: "runtime-command-id.tool.command_run:call_1:0",
      command_type: "shell_command",
      command_line: "npm test",
      success: true,
    }),
  });
  state = reducer(state, { type: "event", event: event("running", 30) });

  const assistant = displayMessages(state).find(
    (message) => message.id === "runtime-command-id.message",
  );
  const commandPart = assistant?.parts.find((part) => part.tool === "command_run");
  const commandState = commandPart?.state as
    | {
        status?: string;
        input?: { commands?: Array<{ command_id?: string }> };
        streamed_command_run_result?: { results?: Array<{ status?: string; success?: boolean }> };
      }
    | undefined;

  assert.equal(commandState?.status, "completed");
  assert.equal(commandState?.input?.commands?.length, 1);
  assert.equal(commandState?.streamed_command_run_result?.results?.length, 1);
  assert.equal(commandState?.streamed_command_run_result?.results?.[0]?.success, true);
});

test("reducer keeps command updates when final runtime message has only text", () => {
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
        type: "command.updated",
        properties: {
          sessionID: "sess-1",
          messageID: "runtime-final-text.message",
          partID: "runtime-final-text.tool.command_run",
          createdAt: 1,
          updatedAt: 40,
          runtimeID: "runtime-final-text",
          commandRunID: "runtime-final-text.tool.command_run",
          commandID: "runtime-final-text.tool.command_run:call_1:0",
          providerToolCallID: "call_1",
          commandIndex: 0,
          eventSeq: 40,
          status: "completed",
          command: {
            command_id: "runtime-final-text.tool.command_run:call_1:0",
            command_type: "shell_command",
            command_line: "npm test",
          },
          result: {
            command_id: "runtime-final-text.tool.command_run:call_1:0",
            command_type: "shell_command",
            command_line: "npm test",
            success: true,
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
        type: "message.updated",
        properties: {
          sessionID: "sess-1",
          info: {
            id: "runtime-final-text.message",
            sessionID: "sess-1",
            role: "assistant",
            created_at: 1,
            updated_at: 50,
            parts: [
              {
                id: "runtime-final-text.message",
                sessionID: "sess-1",
                messageID: "runtime-final-text.message",
                type: "text",
                text: "Final answer",
              },
            ],
          },
        },
      },
    },
  });

  const assistant = displayMessages(state).find(
    (message) => message.id === "runtime-final-text.message",
  );
  const commandPart = assistant?.parts.find((part) => part.tool === "command_run");
  const commandState = commandPart?.state as
    | {
        input?: { commands?: Array<{ command_line?: string }> };
        streamed_command_run_result?: { results?: Array<{ success?: boolean }> };
      }
    | undefined;

  assert.equal(messageText(assistant!), "Final answer");
  assert.equal(commandState?.input?.commands?.[0]?.command_line, "npm test");
  assert.equal(commandState?.streamed_command_run_result?.results?.[0]?.success, true);
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
          createdAt: 100,
          updatedAt: 101,
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

test("reducer commits final live stream on the ending message event", () => {
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
          sessionID: "sess-1",
          messageID: "msg-sessionless-stream",
          partID: "part-sessionless-stream",
          createdAt: 2,
          updatedAt: 2,
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
          sessionID: "sess-1",
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
    "event final text must not sit next to the committed live text",
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

test("reducer replaces live command snapshots instead of appending command parts", () => {
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
          messageID: "runtime-live-order.message",
          partID: "runtime-live-order.message",
          createdAt: 100,
          updatedAt: 100,
          field: "text",
          delta: "checking",
        },
      },
    },
  });

  for (const [id, command_line, updatedAt] of [
    ["runtime-live-order.tool.command_run.1", "first-live-command", 101],
    ["runtime-live-order.tool.command_run.2", "second-live-command", 102],
  ] as const) {
    state = reducer(state, {
      type: "event",
      event: {
        directory: "C:/repo",
        payload: {
          type: "message.part.updated",
          properties: {
            sessionID: "sess-1",
            createdAt: 100,
            updatedAt,
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

  let assistant = displayMessages(state).find(
    (message) => message.id === "runtime-live-order.message",
  );
  let commands = (assistant?.parts ?? [])
    .filter((part) => part.tool === "command_run")
    .map(
      (part) =>
        (part.state as { input?: { commands?: Array<{ command_line?: string }> } }).input
          ?.commands?.[0]?.command_line,
    );

  assert.deepEqual(commands, ["second-live-command"]);

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

  assistant = displayMessages(state)[0];
  commands = assistant.parts
    .filter((part) => part.tool === "command_run")
    .map(
      (part) =>
        (part.state as { input?: { commands?: Array<{ command_line?: string }> } }).input
          ?.commands?.[0]?.command_line,
    );

  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.deepEqual(commands, ["second-live-command"]);
});

test("reducer commits a finished live stream and renders the next runtime event immediately", () => {
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
          messageID: "runtime-a-idle-commit.message",
          partID: "runtime-a-idle-commit.message",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: "A live",
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
            id: "runtime-a-idle-commit.message",
            sessionID: "sess-1",
            role: "assistant",
            created_at: 2,
            updated_at: 3,
            parts: [
              {
                id: "runtime-a-idle-commit.message",
                sessionID: "sess-1",
                messageID: "runtime-a-idle-commit.message",
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
          sessionID: "sess-1",
          messageID: "runtime-b-after-idle-commit.message",
          partID: "runtime-b-after-idle-commit.message",
          createdAt: 4,
          updatedAt: 4,
          field: "text",
          delta: "B should appear",
        },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 1);
  assert.deepEqual(
    displayMessages(state).map((message) => message.id),
    ["runtime-a-idle-commit.message", "runtime-b-after-idle-commit.message"],
  );
  assert.equal(messageText(displayMessages(state)[0]), "A live");
  assert.equal(messageText(displayMessages(state)[1]), "B should appear");

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "session.status",
        properties: { sessionID: "sess-1", updatedAt: 5, status: "idle" },
      },
    },
  });

  assert.equal(Object.values(state.liveStreams).length, 0);
  assert.deepEqual(
    displayMessages(state).map((message) => message.id),
    ["runtime-a-idle-commit.message", "runtime-b-after-idle-commit.message"],
  );
  assert.equal(messageText(displayMessages(state)[0]), "A live");
  assert.equal(messageText(displayMessages(state)[1]), "B should appear");
  assert.equal(state.session?.updated_at, 5);
});

test("reducer ignores session status events without gateway update time", () => {
  const baseSession = { ...session, status: "idle" as const, updated_at: 100 };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: baseSession,
    messages: [],
    permissions: [],
    sessions: [baseSession],
  });

  const updated = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "session.status",
        properties: { sessionID: "sess-1", status: "busy" },
      },
    },
  });

  assert.equal(updated.status, "idle");
  assert.equal(updated.session?.status, "idle");
  assert.equal(updated.session?.updated_at, 100);
  assert.equal(updated.sessions[0]?.updated_at, 100);
});

test("reducer orders interleaved live streams by creation time instead of update time", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, status: "busy" },
    messages: [
      {
        id: "msg-user-live-order",
        sessionID: "sess-1",
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-user-live-order", type: "text", text: "start" }],
      },
    ],
    permissions: [],
  });

  const applyDelta = (now: number, messageID: string, delta: string) => {
    state = withNow(now, () =>
      reducer(state, {
        type: "event",
        event: {
          directory: "C:/repo",
          payload: {
            type: "message.part.delta",
            properties: {
              sessionID: "sess-1",
              messageID: messageID,
              partID: messageID,
              createdAt: now,
              updatedAt: now,
              field: "text",
              delta,
            },
          },
        },
      }),
    );
  };

  applyDelta(1_500, "runtime-live-a.message", "A1 ");
  applyDelta(1_600, "runtime-live-b.message", "B1 ");
  applyDelta(1_700, "runtime-live-a.message", "A2 ");
  applyDelta(1_800, "runtime-live-b.message", "B2 ");

  const visible = displayMessages(state);
  assert.deepEqual(
    visible.map((message) => message.id),
    ["msg-user-live-order", "runtime-live-a.message", "runtime-live-b.message"],
  );
  assert.equal(messageText(visible[1]), "A1 A2 ");
  assert.equal(messageText(visible[2]), "B1 B2 ");
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
          sessionID: "sess-1",
          messageID: "msg-durable-part",
          partID: "part-durable",
          createdAt: 1,
          updatedAt: 2,
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
          sessionID: "sess-1",
          messageID: "runtime-stream.message",
          partID: "runtime-stream.message",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: "command complete\r\x1b[2Knew reply",
        },
      },
    },
  });

  assert.equal(state.messages.length, 0);
  assert.equal(displayMessages(state)[0].parts[0].text, "command complete\nnew reply");
});
