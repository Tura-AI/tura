import type { Message } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import { mergeMessagePages, shouldFetchSessionMessages } from "../../app/src/app-state-utils";

function assistantMessage(id: string, parts: Message["parts"]): Message {
  return {
    id,
    sessionID: "s1",
    role: "assistant",
    parts,
  };
}

describe("message cache merging", () => {
  test("reuses cached session messages unless a refresh is explicit", () => {
    const cached = [assistantMessage("m1", [])];

    expect(shouldFetchSessionMessages(cached)).toBe(false);
    expect(shouldFetchSessionMessages(cached, true)).toBe(true);
    expect(shouldFetchSessionMessages([])).toBe(true);
  });

  test("keeps existing snapshot order while merging later static session snapshots", () => {
    const intro = { id: "intro", sessionID: "s1", messageID: "m1", type: "text", text: "start" };
    const command = {
      id: "tool",
      sessionID: "s1",
      messageID: "m1",
      type: "tool",
      tool: "command_run",
      state: { status: "running" },
    };
    const existing = [assistantMessage("m1", [intro, command])];

    const merged = mergeMessagePages(existing, [
      assistantMessage("m1", [
        { ...command, state: { status: "completed" } },
        intro,
        { id: "final", sessionID: "s1", messageID: "m1", type: "text", text: "done" },
      ]),
    ]);

    expect(merged.map((message) => message.id)).toEqual(["m1"]);
    expect(merged[0]?.parts.map((part) => part.id)).toEqual(["intro", "tool", "final"]);
    expect((merged[0]?.parts[1]?.state as { status?: string } | undefined)?.status).toBe(
      "completed",
    );
  });

  test("does not replace cached messages when a repeated static snapshot has no changes", () => {
    const message = assistantMessage("m1", [
      { id: "intro", sessionID: "s1", messageID: "m1", type: "text", text: "stable" },
    ]);
    const existing = [message];

    const merged = mergeMessagePages(existing, [
      assistantMessage("m1", [
        { id: "intro", sessionID: "s1", messageID: "m1", type: "text", text: "stable" },
      ]),
    ]);

    expect(merged).toBe(existing);
    expect(merged[0]).toBe(message);
  });

  test("appends new snapshot messages without reordering rendered history", () => {
    const existing = [assistantMessage("m2", [])];
    const merged = mergeMessagePages(existing, [
      assistantMessage("m1", []),
      assistantMessage("m2", []),
    ]);

    expect(merged.map((message) => message.id)).toEqual(["m2", "m1"]);
  });
});
