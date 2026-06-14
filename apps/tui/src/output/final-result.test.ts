import assert from "node:assert/strict";
import test from "node:test";
import { buildRunResult } from "./final-result.js";
import { hasUserFacingAssistantText, type Message } from "../types/session.js";

test("run result ignores internal assistant completion placeholders", () => {
  const messages: Message[] = [
    {
      id: "msg-user",
      role: "user",
      parts: [{ id: "part-user", type: "text", text: "hello" }],
    },
    {
      id: "msg-internal",
      role: "assistant",
      parts: [
        {
          id: "part-internal",
          type: "text",
          text: "MANO completed without a user-facing message.",
        },
      ],
    },
  ];

  assert.equal(hasUserFacingAssistantText(messages, 1), false);
  assert.equal(buildRunResult("sess-1", messages).finalText, "");

  messages.push({
    id: "msg-final",
    role: "assistant",
    parts: [{ id: "part-final", type: "text", text: "TUI_BUSINESS_OK" }],
  });

  assert.equal(hasUserFacingAssistantText(messages, 1), true);
  assert.equal(buildRunResult("sess-1", messages).finalText, "TUI_BUSINESS_OK");
});

test("run result uses the newest assistant text when gateway messages are unordered", () => {
  const messages: Message[] = [
    {
      id: "msg-final",
      role: "assistant",
      created_at: 30,
      parts: [{ id: "part-final", type: "text", text: "FINAL_MARKER" }],
    },
    {
      id: "msg-user",
      role: "user",
      created_at: 10,
      parts: [{ id: "part-user", type: "text", text: "hello" }],
    },
    {
      id: "msg-progress",
      role: "assistant",
      created_at: 20,
      parts: [{ id: "part-progress", type: "text", text: "working" }],
    },
  ];

  assert.equal(buildRunResult("sess-1", messages).finalText, "FINAL_MARKER");
});
