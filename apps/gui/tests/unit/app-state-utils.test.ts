import type { Message } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  blankSessionState,
  mergeMessagePages,
  shouldFetchSessionMessages,
} from "../../app/src/app-state-utils";
import { initialAppState } from "../../app/src/state/global-store";

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

describe("blank session workspace state", () => {
  test("uses the workspace clicked in the rail as the new session workspace", () => {
    const state = {
      ...initialAppState("http://127.0.0.1:4126"),
      activeTab: "plan" as const,
      previousMainTab: "plan" as const,
      directory: "C:/repo/alpha",
      selectedSessionId: "alpha-session",
      lastSessionOpenedId: "older-session",
      composerText: "draft that should be cleared",
      projects: [
        { id: "alpha", name: "Alpha", worktree: "C:/repo/alpha" },
        { id: "beta", name: "Beta", worktree: "C:/repo/beta" },
      ],
    };

    expect(
      blankSessionState(state, { id: "beta", name: "Beta", worktree: "C:/repo/beta" }),
    ).toMatchObject({
      activeTab: "conversation",
      previousMainTab: "conversation",
      directory: "C:/repo/beta",
      selectedSessionId: undefined,
      lastSessionOpenedId: "alpha-session",
      composerText: "",
      error: undefined,
    });
  });
});
