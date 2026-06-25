import type { Project } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  sidebarWorkspaceProjects,
  workspaceExpanded,
} from "../../../app/src/components/sidebar/workspace-projects";

function project(worktree: string, name: string, updated?: number): Project {
  return {
    id: worktree,
    name,
    worktree,
    time: updated === undefined ? undefined : { updated },
  };
}

describe("sidebar workspace projects", () => {
  test("keeps every known workspace instead of only the selected one", () => {
    const projects = [project("C:/repo/alpha", "Alpha"), project("C:/repo/beta", "Beta")];

    expect(
      sidebarWorkspaceProjects(projects, "C:/repo/alpha").map((item) => item.worktree),
    ).toEqual(["C:/repo/alpha", "C:/repo/beta"]);
  });

  test("sorts workspaces by last update instead of current selection", () => {
    const projects = [project("C:/repo/alpha", "Alpha", 200), project("C:/repo/beta", "Beta", 100)];

    expect(sidebarWorkspaceProjects(projects, "C:/repo/beta").map((item) => item.worktree)).toEqual(
      ["C:/repo/alpha", "C:/repo/beta"],
    );
  });

  test("adds a fallback current workspace without making selection a sort rule", () => {
    const projects = [project("C:/repo/alpha", "Alpha", 200)];

    expect(sidebarWorkspaceProjects(projects, "C:/repo/beta").map((item) => item.worktree)).toEqual(
      ["C:/repo/alpha", "C:/repo/beta"],
    );
  });

  test("allows multiple workspaces to be expanded at once", () => {
    const expanded = new Set(["c:/repo/alpha", "c:/repo/beta"]);

    expect(workspaceExpanded(expanded, "C:/repo/alpha")).toBe(true);
    expect(workspaceExpanded(expanded, "C:/repo/beta")).toBe(true);
    expect(workspaceExpanded(expanded, "C:/repo/gamma")).toBe(false);
  });
});
