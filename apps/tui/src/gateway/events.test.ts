import assert from "node:assert/strict";
import test from "node:test";
import { normalizeEvent, parseSseBlock } from "./events.js";

test("parseSseBlock joins multiline data and ignores non-data fields", () => {
  const event = parseSseBlock(
    'event: message\ndata: {"directory":"C:/repo",\ndata: "payload":{"type":"server.connected","properties":{}}}\n',
  );

  assert.equal(event?.directory, "C:/repo");
  assert.equal(event?.payload?.type, "server.connected");
});

test("normalizeEvent extracts message and session status fields", () => {
  const message = normalizeEvent({
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
  });

  assert.equal(message.sessionID, "sess-1");
  assert.equal(message.messageID, "msg-1");
  assert.equal(message.text, "hello");

  const status = normalizeEvent({
    payload: {
      type: "session.status",
      properties: { sessionID: "sess-1", status: { type: "busy" } },
    },
  });

  assert.equal(status.status, "busy");
});

test("normalizeEvent extracts part updates", () => {
  const part = normalizeEvent({
    payload: {
      type: "message.part.updated",
      properties: {
        sessionID: "sess-1",
        part: {
          id: "part-1",
          sessionID: "sess-1",
          messageID: "msg-1",
          type: "tool",
          tool: "runtime",
          state: { status: "completed" },
        },
      },
    },
  });

  assert.equal(part.sessionID, "sess-1");
  assert.equal(part.messageID, "msg-1");
  assert.equal(part.partID, "part-1");
  assert.equal(part.tool, "runtime");
  assert.equal(part.status, "completed");
});

test("normalizeEvent accepts camel and snake fields for streaming deltas", () => {
  const delta = normalizeEvent({
    directory: "C:/repo",
    payload: {
      type: "message.part.delta",
      properties: {
        session_id: "sess-1",
        messageID: "msg-1",
        partID: "part-1",
        field: "text",
        delta: "hel",
      },
    },
  });

  assert.equal(delta.sessionID, "sess-1");
  assert.equal(delta.messageID, "msg-1");
  assert.equal(delta.partID, "part-1");
  assert.equal(delta.text, "hel");
});

test("normalizeEvent uses envelope session fields when delta properties omit them", () => {
  const delta = normalizeEvent({
    directory: "C:/repo",
    sessionID: "sess-envelope",
    payload: {
      type: "message.part.delta",
      properties: {
        message_id: "msg-1",
        part_id: "part-1",
        field: "text",
        delta: "hel",
      },
    },
  });

  assert.equal(delta.sessionID, "sess-envelope");
  assert.equal(delta.messageID, "msg-1");
  assert.equal(delta.partID, "part-1");
  assert.equal(delta.text, "hel");
});

test("normalizeEvent extracts permission and question requests", () => {
  const permission = normalizeEvent({
    payload: {
      type: "permission.asked",
      properties: {
        permission: { id: "perm-1", session_id: "sess-1", permission: "shell" },
      },
    },
  });
  assert.equal(permission.sessionID, "sess-1");
  assert.equal(permission.permission?.id, "perm-1");

  const question = normalizeEvent({
    payload: {
      type: "question.asked",
      properties: {
        request: { id: "question-1", sessionID: "sess-1", question: "Continue?" },
      },
    },
  });
  assert.equal(question.sessionID, "sess-1");
  assert.equal(question.question?.question, "Continue?");
});
