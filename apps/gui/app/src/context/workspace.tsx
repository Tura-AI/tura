import { createContext, createMemo, useContext, type Accessor, type JSX } from "solid-js";
import type { AppState } from "../state/global-store";

export type WorkspaceContextValue = {
  workspaceId: Accessor<string | undefined>;
  issues: Accessor<AppState["productIssues"]>;
  projects: Accessor<AppState["productProjects"]>;
  workspaces: Accessor<AppState["workspaces"]>;
};

const WorkspaceContext = createContext<WorkspaceContextValue>();

export function WorkspaceProvider(props: { state: Accessor<AppState>; children: JSX.Element }) {
  const workspaceId = createMemo(() => props.state().workspaces[0]?.id);
  const issues = createMemo(() => props.state().productIssues);
  const projects = createMemo(() => props.state().productProjects);
  const workspaces = createMemo(() => props.state().workspaces);

  return (
    <WorkspaceContext.Provider value={{ workspaceId, issues, projects, workspaces }}>
      {props.children}
    </WorkspaceContext.Provider>
  );
}

export function useWorkspaceState() {
  const context = useContext(WorkspaceContext);
  if (!context) {
    throw new Error("useWorkspaceState must be used inside WorkspaceProvider");
  }
  return context;
}
