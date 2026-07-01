import type { Message, Session } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  blankSessionState,
  mergeSessions,
  mergeMessagePages,
  shouldFetchSessionMessages,
} from "../../app/src/app-state-utils";
import { initialAppState, sessionTitle } from "../../app/src/state/global-store";

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

describe("session cache merging", () => {
  test("does not let stale list refresh snapshots overwrite live display names", () => {
    const local: Session = {
      id: "s1",
      name: "实现 session 标题稳定",
      session_display_name: "实现 session 标题稳定",
      status: "busy",
      updated_at: 20,
    };
    const staleRemote: Session = {
      id: "s1",
      name: "用户输入生成的临时会话名",
      session_display_name: "用户输入生成的临时会话名",
      status: "busy",
      updated_at: 10,
    };

    const merged = mergeSessions([staleRemote], [local]);

    expect(sessionTitle(merged[0]!)).toBe("实现 session 标题稳定");
    expect(merged[0]?.updated_at).toBe(20);
  });

  test("accepts newer list refresh snapshots for the same session", () => {
    const local: Session = {
      id: "s1",
      name: "旧任务名",
      session_display_name: "旧任务名",
      status: "busy",
      updated_at: 20,
    };
    const remote: Session = {
      id: "s1",
      name: "新任务名",
      session_display_name: "新任务名",
      status: "idle",
      updated_at: 30,
    };

    const merged = mergeSessions([remote], [local]);

    expect(sessionTitle(merged[0]!)).toBe("新任务名");
    expect(merged[0]?.status).toBe("idle");
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
