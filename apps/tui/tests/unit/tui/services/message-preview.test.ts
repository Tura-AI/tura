import assert from "node:assert/strict";
import test from "node:test";
import type { Message } from "../../../../src/types/session.js";
import { lastMessagePreview } from "../../../../src/tui/services/message-preview.js";

test("lastMessagePreview returns the latest non-empty user-facing text", () => {
  const messages: Message[] = [
    message("m1", "user", "  hello\nworld  "),
    message("m2", "assistant", ""),
    message("m3", "assistant", "done: {}"),
    message("m4", "assistant", " final answer "),
  ];
  assert.equal(lastMessagePreview(messages), "final answer");
});

test("lastMessagePreview can exclude the active message id", () => {
  const messages: Message[] = [
    message("m1", "assistant", "older"),
    message("m2", "assistant", "streaming"),
  ];
  assert.equal(lastMessagePreview(messages, "m2"), "older");
});

test("lastMessagePreview returns undefined when every candidate is empty or internal", () => {
  const messages: Message[] = [
    message("m1", "assistant", ""),
    message("m2", "assistant", "done: {}"),
  ];
  assert.equal(lastMessagePreview(messages), undefined);
});

function message(id: string, role: Message["role"], text: string): Message {
  return { id, role, parts: [{ id: `${id}-part`, type: "text", text }] };
}
