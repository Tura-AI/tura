import type { Project } from "@tura/gateway-sdk";
import { normalizePath, samePath, shortWorkspaceLabel } from "../../utils/app-format";

function workspaceUpdatedAt(project: Project): number {
  return project.time?.updated ?? project.time?.created ?? 0;
}

export function workspaceExpanded(
  expandedWorkspaces: Set<string> | undefined,
  worktree: string,
): boolean {
  return expandedWorkspaces?.has(normalizePath(worktree)) ?? false;
}

export function sidebarWorkspaceProjects(projects: Project[], directory?: string): Project[] {
  const fallbackProject = directory
    ? {
        id: directory,
        name: shortWorkspaceLabel(directory),
        worktree: directory,
      }
    : undefined;
  const hydratedProjects = [...projects];

  if (
    fallbackProject &&
    !hydratedProjects.some((project) => samePath(project.worktree, directory))
  ) {
    hydratedProjects.push(fallbackProject);
  }
  return hydratedProjects.sort(
    (left, right) => workspaceUpdatedAt(right) - workspaceUpdatedAt(left),
  );
}
