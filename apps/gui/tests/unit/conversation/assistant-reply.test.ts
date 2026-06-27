import type { Message, MessagePart } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  hasVisibleAssistantReply,
  hasVisibleNewAssistantReply,
  visibleAssistantReplyCursor,
} from "../../../app/src/conversation/assistant-reply";

function assistantMessage(id: string, parts: MessagePart[]): Message {
  return {
    id,
    sessionID: "s1",
    role: "assistant",
    parts,
  };
}

function textPart(messageId: string, text: string): MessagePart {
  return {
    id: `${messageId}:text`,
    sessionID: "s1",
    messageID: messageId,
    type: "text",
    text,
  };
}

function toolPart(messageId: string): MessagePart {
  return {
    id: `${messageId}:tool`,
    sessionID: "s1",
    messageID: messageId,
    type: "tool",
    tool: "runtime",
    state: { status: "running" },
  };
}

describe("assistant reply visibility", () => {
  test("ignores transient and tool-only assistant messages", () => {
    expect(
      hasVisibleAssistantReply([assistantMessage("thinking", [textPart("thinking", "thinking")])]),
    ).toBe(false);
    expect(hasVisibleAssistantReply([assistantMessage("tool", [toolPart("tool")])])).toBe(false);
    expect(hasVisibleAssistantReply([assistantMessage("done", [textPart("done", "done")])])).toBe(
      true,
    );
  });

  test("detects only assistant replies that are new or visibly changed", () => {
    const known = visibleAssistantReplyCursor([
      assistantMessage("old", [textPart("old", "already visible")]),
    ]);

    expect(
      hasVisibleNewAssistantReply(
        [assistantMessage("old", [textPart("old", "already visible")])],
        known,
      ),
    ).toBe(false);
    expect(
      hasVisibleNewAssistantReply(
        [assistantMessage("old", [textPart("old", "updated visible")])],
        known,
      ),
    ).toBe(true);
    expect(
      hasVisibleNewAssistantReply(
        [assistantMessage("new", [textPart("new", "fresh reply")])],
        known,
      ),
    ).toBe(true);
  });
});
