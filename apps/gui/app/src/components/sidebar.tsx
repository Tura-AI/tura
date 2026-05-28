import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Accessor,
  type JSX,
  type Setter,
} from "solid-js";
import { Portal } from "solid-js/web";
import ExternalLink from "lucide-solid/icons/external-link";
import LayoutList from "lucide-solid/icons/layout-list";
import ArrowLeft from "lucide-solid/icons/arrow-left";
import CalendarDays from "lucide-solid/icons/calendar-days";
import ChartGantt from "lucide-solid/icons/chart-gantt";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import Columns3 from "lucide-solid/icons/columns-3";
import Copy from "lucide-solid/icons/copy";
import Edit3 from "lucide-solid/icons/pencil";
import FolderOpen from "lucide-solid/icons/folder-open";
import KeyRound from "lucide-solid/icons/key-round";
import MoreHorizontal from "lucide-solid/icons/ellipsis";
import Pin from "lucide-solid/icons/pin";
import Plus from "lucide-solid/icons/plus";
import Search from "lucide-solid/icons/search";
import Settings from "lucide-solid/icons/settings";
import Trash2 from "lucide-solid/icons/trash-2";
import {
  GatewayClient,
  GatewayError,
  connectGatewayEvents,
  defaultGatewayUrl,
  errorMessage,
  type Agent,
  type Command,
  type FileContentResponse,
  type FileInfo,
  type GatewayConfig,
  type Message,
  type ProviderAuthMethod,
  type ProductIssue,
  type Project,
  type PollInterval,
  type SdkProvider,
  type Session,
  type StartCondition,
  type TaskManagement,
  type PlanStatus,
} from "@tura/gateway-sdk";
import {
  Composer,
  ConversationView,
  composerFileToken,
  composerImageToken,
} from "../conversation/conversation-view";
import { applyGatewayEvent } from "../state/event-reducer";
import {
  activeSession,
  type ComposerImage,
  initialAppState,
  type MainTab,
  type PlanMode,
  sessionDirectory,
  sessionUpdatedAt,
  sessionTitle,
  type AppState,
  type SettingsSection,
  type ThemeMode,
} from "../state/global-store";
import { classNames } from "../state/format";
import { t, type TextKey } from "../i18n";

import {
  applyTaskPatchToSession,
  planSessionStatus,
} from "../features/plan/tasks";
import { PlanStatusIndicator, SessionRowMeta } from "../pages/plan/plan-view";
import { FileTreeLabel, NameDialog } from "../pages/new-session";
import {
  normalizePath,
  normalizeTimeMs,
  relativeSessionTime,
  samePath,
  sessionHoverTitle,
  shortPathLabel,
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

export function RailSectionTitle(props: {
  className?: string;
  icon?: JSX.Element;
  expanded: boolean;
  children: JSX.Element;
  onToggle: () => void;
}) {
  return (
    <button
      class={classNames("section-title", props.className)}
      type="button"
      onClick={props.onToggle}
    >
      {props.icon}
      <span>{props.children}</span>
      <RailDisclosure expanded={props.expanded} />
    </button>
  );
}

export function WorkspaceChildren(props: {
  activeTab: MainTab;
  expandedGroup?: string;
  sessions: Session[];
  attentionAcknowledged: (session: Session) => boolean;
  selectedSessionId?: string;
  productIssues: ProductIssue[];
  filePath: string;
  files: FileInfo[];
  fileTree: Record<string, FileInfo[]>;
  fileLoadingPath?: string;
  expandedFileTreePaths: Set<string>;
  selectedFile?: FileInfo;
  onIssue: (issue: ProductIssue) => void;
  onGroup: (id: string) => void;
  onStatus: (session: Session, status: PlanStatus) => void;
  onSession: (session: Session) => void;
  onRenameSession: (sessionId: string, title: string) => void;
  onFile: (file: FileInfo) => void;
  onFileTreeDirectory: (file: FileInfo) => void;
  onUp: () => void;
}) {
  const [expandedSessions, setExpandedSessions] = createSignal(false);
  const [renaming, setRenaming] = createSignal<Session>();
  const visibleSessions = createMemo(() =>
    expandedSessions() ? props.sessions : props.sessions.slice(0, 5),
  );
  const hiddenSessionCount = createMemo(() =>
    Math.max(0, props.sessions.length - 5),
  );
  const rootFiles = createMemo(() => props.fileTree[""] ?? props.files);
  const sortedPlanSessions = createMemo(() =>
    [...props.sessions].sort(
      (left, right) =>
        normalizeTimeMs(sessionUpdatedAt(right) ?? 0) -
        normalizeTimeMs(sessionUpdatedAt(left) ?? 0),
    ),
  );
  return (
    <div class="workspace-children">
      <Switch>
        <Match when={props.activeTab === "plan"}>
          <For
            each={sortedPlanSessions()}
            fallback={<div class="rail-empty">{t("noSessions")}</div>}
          >
            {(session) => (
              <button
                class={classNames(
                  "child-row",
                  "session-row",
                  props.selectedSessionId === session.id && "selected",
                )}
                style={{ "--depth": 1 }}
                onClick={() => props.onSession(session)}
                title={sessionHoverTitle(session)}
              >
                <span>{shortSessionTitle(sessionTitle(session))}</span>
                <SessionRowMeta
                  session={session}
                  attentionAcknowledged={props.attentionAcknowledged(session)}
                />
                <Edit3
                  class="session-rename-icon"
                  size={13}
                  strokeWidth={1.7}
                  onClick={(event) => {
                    event.stopPropagation();
                    setRenaming(session);
                  }}
                />
              </button>
            )}
          </For>
        </Match>
        <Match
          when={props.activeTab === "conversation"}
        >
          <For
            each={visibleSessions()}
            fallback={<div class="rail-empty">{t("noSessions")}</div>}
          >
            {(session) => (
              <button
                class={classNames(
                  "child-row",
                  "session-row",
                  props.selectedSessionId === session.id && "selected",
                )}
                style={{ "--depth": 1 }}
                onClick={() => props.onSession(session)}
                title={sessionHoverTitle(session)}
              >
                <span>{shortSessionTitle(sessionTitle(session))}</span>
                <SessionRowMeta
                  session={session}
                  attentionAcknowledged={props.attentionAcknowledged(session)}
                />
                <Edit3
                  class="session-rename-icon"
                  size={13}
                  strokeWidth={1.7}
                  onClick={(event) => {
                    event.stopPropagation();
                    setRenaming(session);
                  }}
                />
              </button>
            )}
          </For>
          <Show when={hiddenSessionCount() > 0}>
            <button
              type="button"
              class="child-row rail-more"
              style={{ "--depth": 1 }}
              onClick={() => setExpandedSessions((value) => !value)}
            >
              {expandedSessions()
                ? t("collapse")
                : t("showMore", { count: hiddenSessionCount() })}
            </button>
          </Show>
          <Show when={renaming()}>
            {(session) => (
              <NameDialog
                title={t("renameSession")}
                description={t("renameSessionHint")}
                initialValue={sessionTitle(session())}
                onCancel={() => setRenaming(undefined)}
                onSave={(value) => {
                  props.onRenameSession(session().id, value);
                  setRenaming(undefined);
                }}
              />
            )}
          </Show>
        </Match>
        <Match when={props.activeTab === "files"}>
          <FileTreeRows
            files={rootFiles()}
            fileTree={props.fileTree}
            activePath={props.filePath}
            loadingPath={props.fileLoadingPath}
            expandedPaths={props.expandedFileTreePaths}
            selectedFile={props.selectedFile}
            depth={2}
            onFile={props.onFile}
            onDirectory={props.onFileTreeDirectory}
          />
        </Match>
      </Switch>
    </div>
  );
}

export function FileTreeRows(props: {
  files: FileInfo[];
  fileTree: Record<string, FileInfo[]>;
  activePath: string;
  loadingPath?: string;
  expandedPaths: Set<string>;
  selectedFile?: FileInfo;
  depth: number;
  onFile: (file: FileInfo) => void;
  onDirectory: (file: FileInfo) => void;
}) {
  return (
    <For
      each={props.files}
      fallback={
        props.depth === 1 ? <div class="rail-empty">{t("empty")}</div> : null
      }
    >
      {(file) => {
        const loadedChildren = createMemo(
          () => props.fileTree[file.path] ?? [],
        );
        const expanded = createMemo(
          () => file.type === "directory" && props.expandedPaths.has(file.path),
        );
        return (
          <>
            <button
              class={classNames(
                "child-row",
                file.type === "directory" && "tree-folder",
                props.selectedFile?.path === file.path && "selected",
              )}
              style={{ "--depth": props.depth }}
              onClick={() =>
                file.type === "directory"
                  ? props.onDirectory(file)
                  : props.onFile(file)
              }
              title={file.path}
            >
              <FileTreeLabel file={file} expanded={expanded()} />
              <Show when={props.loadingPath === file.path}>
                <span class="file-tree-loading loading-bar" />
              </Show>
            </button>
            <Show when={expanded()}>
              <FileTreeRows
                files={loadedChildren()}
                fileTree={props.fileTree}
                activePath={props.activePath}
                loadingPath={props.loadingPath}
                expandedPaths={props.expandedPaths}
                selectedFile={props.selectedFile}
                depth={props.depth + 1}
                onFile={props.onFile}
                onDirectory={props.onDirectory}
              />
            </Show>
          </>
        );
      }}
    </For>
  );
}

export function WorkspaceMenu(props: {
  onSettings: () => void;
  onNewSession: () => void;
}) {
  const [open, setOpen] = createSignal(false);
  return (
    <div class="workspace-menu">
      <button
        type="button"
        title={t("settings")}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((value) => !value);
        }}
      >
        <MoreHorizontal size={15} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="rail-menu" onClick={(event) => event.stopPropagation()}>
          <button type="button">
            <Pin size={14} strokeWidth={1.7} />
            <span>{t("pinWorkspace")}</span>
          </button>
          <button type="button">
            <FolderOpen size={14} strokeWidth={1.7} />
            <span>{t("openInExplorer")}</span>
          </button>
          <button type="button" onClick={props.onNewSession}>
            <Plus size={14} strokeWidth={1.7} />
            <span>{t("newSession")}</span>
          </button>
          <button type="button" onClick={props.onSettings}>
            <Settings size={14} strokeWidth={1.7} />
            <span>{t("workspaceSettings")}</span>
          </button>
          <button type="button">
            <ArchiveIcon />
            <span>{t("archiveSession")}</span>
          </button>
          <button type="button">
            <Trash2 size={14} strokeWidth={1.7} />
            <span>{t("remove")}</span>
          </button>
        </div>
      </Show>
    </div>
  );
}

export function ArchiveIcon() {
  return <span class="tiny-icon">▣</span>;
}

export function RailDisclosure(props: { expanded: boolean }) {
  return (
    <span
      class={classNames("rail-disclosure", props.expanded && "expanded")}
      aria-hidden="true"
    >
      <ChevronRight size={13} strokeWidth={1.8} />
    </span>
  );
}
