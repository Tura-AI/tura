import type { Session } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  hiddenRootSessionCount,
  rootSessions,
  topmostSessionId,
  visibleSessionTreeRows,
} from "../../../app/src/state/session-tree";

function session(id: string, updated_at: number, parent_id?: string | null): Session {
  return {
    id,
    parent_id,
    status: "idle",
    updated_at,
  };
}

describe("session tree", () => {
  const sessions = [
    session("root-a", 10),
    session("root-b", 20),
    session("child-a-1", 30, "root-a"),
    session("child-a-2", 25, "root-a"),
    session("grandchild-a-1", 40, "child-a-1"),
  ];

  test("keeps sub sessions out of root plan lists", () => {
    expect(rootSessions(sessions).map((item) => item.id)).toEqual(["root-b", "root-a"]);
  });

  test("resolves a selected child to its top-level session", () => {
    expect(topmostSessionId(sessions, "grandchild-a-1")).toBe("root-a");
  });

  test("expands only the selected root session subtree", () => {
    expect(
      visibleSessionTreeRows(sessions, "root-a", { expandedRoots: true }).map((row) => [
        row.session.id,
        row.depth,
      ]),
    ).toEqual([
      ["root-b", 0],
      ["root-a", 0],
      ["child-a-1", 1],
      ["grandchild-a-1", 2],
      ["child-a-2", 1],
    ]);
  });

  test("does not count selected offscreen root as hidden", () => {
    const manyRoots = [
      session("root-1", 10),
      session("root-2", 9),
      session("root-3", 8),
      session("root-4", 7),
      session("root-5", 6),
      session("root-6", 5),
      session("root-7", 4),
    ];

    expect(hiddenRootSessionCount(manyRoots, "root-6")).toBe(1);
  });
});
