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
        parts: [{ id: "part-agent", type: "text", text: "Hello, working on it." }],
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
        parts: [{ id: "part-final", type: "text", text: "Done." }],
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
    displayMessages(state).map((message) => message.id),
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
    ["msg-user", "msg-agent", "msg-command"],
  );
  assert.deepEqual(
    displayMessages(state).map((message) => message.id),
    ["msg-user", "msg-agent", "msg-command", "msg-final"],
  );
  assert.equal(
    messageText(displayMessages(state).at(-1)!).trim(),
    "Acceptance passed. Final marker is visible.",
  );
});

test("reducer appends incremental polling pages without rewriting displayed history", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: { ...session, message_count: 1 },
    messages: [
      {
        id: "msg-1",
        sessionID: "sess-1",
        role: "user",
        created_at: 1,
        parts: [{ id: "part-1", type: "text", text: "one" }],
      },
    ],
    permissions: [],
  });

  state = reducer(state, {
    type: "messages-incremental",
    sessionID: "sess-1",
    session: { ...session, message_count: 2 },
    messages: [
      {
        id: "msg-2",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 2,
        parts: [{ id: "part-2", type: "text", text: "two" }],
      },
    ],
  });
  state = reducer(state, {
    type: "messages-incremental",
    sessionID: "sess-1",
    session: { ...session, message_count: 2 },
    messages: [
      {
        id: "msg-2",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 2,
        updated_at: 3,
        parts: [{ id: "part-2", type: "text", text: "two updated" }],
      },
    ],
  });

  assert.deepEqual(
    state.messages.map((message) => message.id),
    ["msg-1", "msg-2"],
  );
  assert.equal(messageText(state.messages[1]), "two");
  assert.equal(state.refreshState["sess-1"]?.lastFinalMessageID, "msg-2");
  assert.equal(state.refreshState["sess-1"]?.lastFinalMessageCount, 2);
});

test("reducer keeps refresh cursor stable on event reconnect", () => {
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-1",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 1,
        parts: [{ id: "part-1", type: "text", text: "one" }],
      },
    ],
    permissions: [],
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "global",
      payload: { type: "server.connected", properties: {} },
    },
  });

  assert.equal(state.refreshState["sess-1"]?.lastFinalMessageID, "msg-1");
  assert.equal(state.refreshState["sess-1"]?.lastFinalMessageCount, 1);
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
          { id: "part-text", type: "text", text: "Question answered." },
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
