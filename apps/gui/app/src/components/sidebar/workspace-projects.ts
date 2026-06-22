import type { Project } from "@tura/gateway-sdk";
import { samePath, shortWorkspaceLabel } from "../../utils/app-format";

export function sidebarWorkspaceProjects(projects: Project[], directory?: string): Project[] {
  const fallbackProject = directory
    ? {
        id: directory,
        name: shortWorkspaceLabel(directory),
        worktree: directory,
      }
    : undefined;
  const sortedProjects = directory
    ? [...projects].sort((left, right) => {
        const leftSelected = samePath(left.worktree, directory);
        const rightSelected = samePath(right.worktree, directory);
        return Number(rightSelected) - Number(leftSelected);
      })
    : projects;

  if (sortedProjects.some((project) => samePath(project.worktree, directory))) {
    return sortedProjects;
  }
  return fallbackProject ? [fallbackProject, ...sortedProjects] : sortedProjects;
}
