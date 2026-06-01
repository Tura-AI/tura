import {
  type FileInfo,
  type PlanStatus,
  type ProductIssue,
  type Project,
  type Session,
} from "@tura/gateway-sdk";
import FolderOpen from "lucide-solid/icons/folder-open";
import Plus from "lucide-solid/icons/plus";
import { For, Show, createMemo, createSignal } from "solid-js";
import { RailSectionTitle } from "./sidebar/rail-section";
import { WorkspaceChildren } from "./sidebar/workspace-children";
import { WorkspaceMenu } from "./sidebar/workspace-menu";
import { t } from "../i18n";
import { classNames } from "../state/format";
import {
  sessionDirectory,
  sessionTitle,
  sessionUpdatedAt,
  type MainTab,
} from "../state/global-store";

import { planSessionStatus } from "../features/plan/tasks";
import { PlanStatusIndicator } from "../pages/plan/plan-view";
import {
  normalizePath,
  normalizeTimeMs,
  relativeSessionTime,
  samePath,
  sessionHoverTitle,
  shortSessionTitle,
  shortWorkspaceLabel,
} from "../utils/app-format";
export function WorkspaceTree(props: {
  activeTab: MainTab;
  projects: Project[];
  directory?: string;
  sessions: Session[];
  selectedSessionId?: string;
  productIssues: ProductIssue[];
  filePath: string;
  files: FileInfo[];
  fileTree: Record<string, FileInfo[]>;
  fileLoadingPath?: string;
  expandedFileTreePaths: Set<string>;
  selectedFile?: FileInfo;
  expandedWorkspace?: string;
  expandedGroup?: string;
  attentionAcknowledged: (session: Session) => boolean;
  onWorkspace: (project: Project) => void;
  onBlankSession: () => void;
  onGroup: (id: string) => void;
  onIssue: (issue: ProductIssue) => void;
  onStatus: (session: Session, status: PlanStatus) => void;
  onSession: (sessionId: string) => void;
  onRenameSession: (sessionId: string, title: string) => void;
  onFile: (file: FileInfo) => void;
  onFileTreeDirectory: (file: FileInfo) => void;
  onUp: () => void;
  onSettings: () => void;
}) {
  const [workspaceSectionOpen, setWorkspaceSectionOpen] = createSignal(true);
  const [archivedSectionOpen, setArchivedSectionOpen] = createSignal(true);
  const fallbackProject = createMemo<Project | undefined>(() =>
    props.directory
      ? {
          id: props.directory,
          name: shortWorkspaceLabel(props.directory),
          worktree: props.directory,
        }
      : undefined,
  );
  const projects = createMemo(() =>
    props.projects
      .filter((project) => samePath(project.worktree, props.directory))
      .slice(0, 1)
      .concat(
        props.projects.some((project) =>
          samePath(project.worktree, props.directory),
        )
          ? []
          : fallbackProject()
            ? [fallbackProject()!]
            : [],
      ),
  );
  const activeWorkspaceSessions = (worktree: string) =>
    props.sessions.filter(
      (session) =>
        samePath(sessionDirectory(session), worktree) &&
        planSessionStatus(session) !== "archived",
    );
  function openRailSession(session: Session) {
    props.onSession(session.id);
  }
  function workspaceAttentionStatus(worktree: string): PlanStatus | undefined {
    const sessions = activeWorkspaceSessions(worktree)
      .filter((session) => {
        const status = planSessionStatus(session);
        return status === "doing" || status === "question" || status === "done";
      })
      .filter((session) => !props.attentionAcknowledged(session))
      .sort(
        (left, right) =>
          normalizeTimeMs(sessionUpdatedAt(right) ?? 0) -
          normalizeTimeMs(sessionUpdatedAt(left) ?? 0),
      );
    return sessions[0] ? planSessionStatus(sessions[0]) : undefined;
  }
  const archivedWorkspaces = createMemo(() => {
    const groups = new Map<string, { project: Project; sessions: Session[] }>();
    for (const session of props.sessions) {
      if (planSessionStatus(session) !== "archived") {
        continue;
      }
      const directory = sessionDirectory(session);
      if (!directory) {
        continue;
      }
      const project = props.projects.find((item) =>
        samePath(item.worktree, directory),
      ) ?? {
        id: directory,
        name: shortWorkspaceLabel(directory),
        worktree: directory,
      };
      const key = normalizePath(directory);
      const existing = groups.get(key);
      if (existing) {
        existing.sessions.push(session);
      } else {
        groups.set(key, { project, sessions: [session] });
      }
    }
    return Array.from(groups.values()).sort((left, right) =>
      (left.project.name || left.project.worktree).localeCompare(
        right.project.name || right.project.worktree,
      ),
    );
  });
  function dropArchivedSession(event: DragEvent) {
    event.preventDefault();
    const session = props.sessions.find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    if (session) {
      props.onStatus(session, "archived");
    }
  }

  return (
    <div class="workspace-tree">
      <Show when={projects().length > 0}>
        <RailSectionTitle
          expanded={workspaceSectionOpen()}
          onToggle={() => setWorkspaceSectionOpen((open) => !open)}
        >
          {t("workspace")}
        </RailSectionTitle>
        <Show when={workspaceSectionOpen()}>
          <For each={projects()}>
            {(project) => (
              <div class="workspace-node">
                <div class="workspace-row-wrap">
                  <button
                    class={classNames(
                      "workspace-row",
                      samePath(project.worktree, props.directory) && "selected",
                    )}
                    onClick={() => props.onWorkspace(project)}
                    title={project.worktree}
                  >
                    <FolderOpen size={15} strokeWidth={1.6} />
                    <span class="workspace-row-label">
                      {project.name || shortWorkspaceLabel(project.worktree)}
                    </span>
                    <Show
                      when={
                        props.activeTab !== "plan" &&
                        props.expandedWorkspace !== project.worktree &&
                        workspaceAttentionStatus(project.worktree)
                      }
                    >
                      {(status) => <PlanStatusIndicator status={status()} />}
                    </Show>
                  </button>
                  <div class="workspace-actions">
                    <button
                      type="button"
                      title={t("newSession")}
                      onClick={(event) => {
                        event.stopPropagation();
                        props.onBlankSession();
                      }}
                    >
                      <Plus size={14} strokeWidth={1.8} />
                    </button>
                    <WorkspaceMenu
                      onSettings={props.onSettings}
                      onNewSession={props.onBlankSession}
                    />
                  </div>
                </div>
                <Show
                  when={
                    samePath(project.worktree, props.directory) &&
                    props.expandedWorkspace === project.worktree
                  }
                >
                  <WorkspaceChildren
                    activeTab={props.activeTab}
                    expandedGroup={props.expandedGroup}
                    sessions={activeWorkspaceSessions(project.worktree)}
                    attentionAcknowledged={props.attentionAcknowledged}
                    selectedSessionId={props.selectedSessionId}
                    productIssues={props.productIssues}
                    filePath={props.filePath}
                    files={props.files}
                    fileTree={props.fileTree}
                    fileLoadingPath={props.fileLoadingPath}
                    expandedFileTreePaths={props.expandedFileTreePaths}
                    selectedFile={props.selectedFile}
                    onIssue={props.onIssue}
                    onGroup={props.onGroup}
                    onStatus={props.onStatus}
                    onSession={openRailSession}
                    onRenameSession={props.onRenameSession}
                    onFile={props.onFile}
                    onFileTreeDirectory={props.onFileTreeDirectory}
                    onUp={props.onUp}
                  />
                </Show>
              </div>
            )}
          </For>
        </Show>
      </Show>
      <Show
        when={props.activeTab !== "files" && archivedWorkspaces().length > 0}
      >
        <RailSectionTitle
          className="archived-section-title"
          expanded={archivedSectionOpen()}
          onToggle={() => setArchivedSectionOpen((open) => !open)}
        >
          {t("archived")}
        </RailSectionTitle>
        <Show when={archivedSectionOpen()}>
          <For each={archivedWorkspaces()}>
            {(group) => (
              <div class="workspace-node archived-workspace-node">
                <button
                  class={classNames(
                    "workspace-row",
                    props.expandedGroup ===
                      `archived:${group.project.worktree}` && "selected",
                  )}
                  onClick={() =>
                    props.onGroup(`archived:${group.project.worktree}`)
                  }
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={dropArchivedSession}
                  title={group.project.worktree}
                >
                  <FolderOpen size={15} strokeWidth={1.6} />
                  <span class="workspace-row-label">
                    {group.project.name ||
                      shortWorkspaceLabel(group.project.worktree)}
                  </span>
                </button>
                <Show
                  when={
                    props.expandedGroup === `archived:${group.project.worktree}`
                  }
                >
                  <div class="workspace-children archived-group">
                    <For each={group.sessions}>
                      {(session) => (
                        <button
                          class="child-row session-row"
                          style={{ "--depth": 1 }}
                          onClick={() => openRailSession(session)}
                          title={sessionHoverTitle(session)}
                        >
                          <span>
                            {shortSessionTitle(sessionTitle(session))}
                          </span>
                          <small>{relativeSessionTime(session)}</small>
                        </button>
                      )}
                    </For>
                  </div>
                </Show>
              </div>
            )}
          </For>
        </Show>
      </Show>
    </div>
  );
}
