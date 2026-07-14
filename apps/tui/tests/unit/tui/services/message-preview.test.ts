import assert from "node:assert/strict";
import test from "node:test";
import type { Message } from "../../../../src/types/session.js";
import { setLanguage } from "../../../../src/i18n.js";
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

test("lastMessagePreview localizes structured runtime stopped messages", () => {
  setLanguage("zh-CN");
  const messages: Message[] = [
    {
      id: "m1",
      role: "assistant",
      parts: [
        {
          id: "m1-part",
          type: "text",
          text: "MANO failed while processing this prompt: one-shot worker cancelled",
          metadata: { kind: "runtime_status", code: "runtime_stopped" },
        },
      ],
    },
  ];
  assert.equal(lastMessagePreview(messages), "Runtime 已停止。");
  setLanguage(undefined);
});

function message(id: string, role: Message["role"], text: string): Message {
  return { id, role, parts: [{ id: `${id}-part`, type: "text", text }] };
}
