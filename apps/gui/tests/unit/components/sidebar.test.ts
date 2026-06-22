import type { Project } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import { sidebarWorkspaceProjects } from "../../../app/src/components/sidebar/workspace-projects";

function project(worktree: string, name: string): Project {
  return {
    id: worktree,
    name,
    worktree,
  };
}

describe("sidebar workspace projects", () => {
  test("keeps every known workspace instead of only the selected one", () => {
    const projects = [project("C:/repo/alpha", "Alpha"), project("C:/repo/beta", "Beta")];

    expect(sidebarWorkspaceProjects(projects, "C:/repo/alpha").map((item) => item.worktree)).toEqual([
      "C:/repo/alpha",
      "C:/repo/beta",
    ]);
  });

  test("moves the current workspace first without dropping the others", () => {
    const projects = [project("C:/repo/alpha", "Alpha"), project("C:/repo/beta", "Beta")];

    expect(sidebarWorkspaceProjects(projects, "C:/repo/beta").map((item) => item.worktree)).toEqual([
      "C:/repo/beta",
      "C:/repo/alpha",
    ]);
  });

  test("adds a fallback current workspace when the project list has not hydrated it", () => {
    const projects = [project("C:/repo/alpha", "Alpha")];

    expect(sidebarWorkspaceProjects(projects, "C:/repo/beta").map((item) => item.worktree)).toEqual([
      "C:/repo/beta",
      "C:/repo/alpha",
    ]);
  });
});
