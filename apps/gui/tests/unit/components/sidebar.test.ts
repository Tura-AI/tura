import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { Project } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  sidebarWorkspaceProjects,
  workspaceExpanded,
} from "../../../app/src/components/sidebar/workspace-projects";

const sidebarCss = readFileSync(
  resolve(import.meta.dir, "../../../app/src/styles/components/sidebar.css"),
  "utf8",
);
const sidebarSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/components/sidebar.tsx"),
  "utf8",
);
const workspaceMenuSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/components/sidebar/workspace-menu.tsx"),
  "utf8",
);

function cssBlock(selector: string): string {
  const start = sidebarCss.indexOf(`\n${selector} {`);
  expect(start).toBeGreaterThanOrEqual(0);
  const bodyStart = sidebarCss.indexOf("{", start);
  const bodyEnd = sidebarCss.indexOf("}", bodyStart);
  return sidebarCss.slice(start, bodyEnd + 1);
}

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

describe("sidebar cursor affordances", () => {
  test("does not show the text cursor over empty session text or session rows", () => {
    expect(cssBlock(".workspace-children > .rail-empty")).toContain("cursor: default;");
    expect(cssBlock(".session-row")).toContain("cursor: pointer;");
  });
});

describe("sidebar workspace new session", () => {
  test("passes the clicked workspace into the new session action", () => {
    expect(sidebarSource).toContain("onBlankSession: (project: Project) => void;");
    expect(sidebarSource).toContain("props.onBlankSession(project);");
  });
});

describe("sidebar workspace action menu", () => {
  test("keeps only the delete workspace action in the three-dot menu", () => {
    expect(workspaceMenuSource).toContain("onDeleteWorkspace");
    expect(workspaceMenuSource).toContain('t("deleteWorkspace")');
    expect(workspaceMenuSource).not.toContain('t("pinWorkspace")');
    expect(workspaceMenuSource).not.toContain('t("openInExplorer")');
    expect(workspaceMenuSource).not.toContain('t("newSession")');
    expect(workspaceMenuSource).not.toContain('t("workspaceSettings")');
    expect(workspaceMenuSource).not.toContain('t("archiveSession")');
    expect(workspaceMenuSource).not.toContain('t("remove")');
  });
});
