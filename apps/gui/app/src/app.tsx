import {
  Show,
  Switch,
  Match,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import { Portal } from "solid-js/web";
import {
  GatewayClient,
  connectGatewayEvents,
  defaultGatewayUrl,
  errorMessage,
  type FileContentResponse,
  type FileInfo,
  type ProductIssue,
  type Project,
  type PollInterval,
  type ProviderAuthMethod,
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
} from "./conversation/conversation-view";
import { applyGatewayEvent } from "./state/event-reducer";
import {
  activeSession,
  type ComposerImage,
  initialAppState,
  type MainTab,
  sessionDirectory,
  sessionTitle,
  sessionUpdatedAt,
  type AppState,
  type SettingsSection,
} from "./state/global-store";
import { classNames } from "./state/format";
import { t } from "./i18n";
import { WorkspaceTree } from "./components/sidebar";
import { NewSessionView } from "./pages/new-session";
import {
  MainTabs,
  SettingsRail,
  SettingsView,
} from "./pages/settings/settings-view";
import { ProviderAuthDialog } from "./pages/settings/provider-settings";
import { PlanView } from "./pages/plan/plan-view";
import { FileBrowserView } from "./pages/files/file-browser";
import {
  fixtureAppState,
  fixtureFileContent,
  fixtureFiles,
} from "./mock/fixtures";
import {
  applyTaskPatchToSession,
  defaultLocalStartAt,
  defaultPollInterval,
  firstRunnableTask,
  formatTicketTime,
  hasVisibleSessionTasks,
  localDateTimeToUtcIso,
  materializeComposerContent,
  normalizePollInterval,
  planSessionStatus,
  sessionAttentionKey,
  sessionTaskState,
  taskDisplayText,
  taskNonceId,
  taskPollInterval,
  taskStartAt,
  taskStartCondition,
  timedTaskPatch,
  utcIsoToLocalDateTime,
} from "./features/plan/tasks";
import {
  configDraftToPatch,
  configToDraft,
  defaultModel,
  draftToRecord,
  modelRef,
  parseModelRef,
  providerConfigured,
  providerIdFromAuthError,
  providerIdFromModel,
  recordToDraft,
} from "./utils/settings";
import {
  eventBelongsToState,
  normalizeTimeMs,
  parentPath,
  readBooleanSearchParam,
  readConfigBoolean,
  readConfigString,
  readMainTabSearchParam,
  readSearchParam,
  samePath,
  shortWorkspaceLabel,
  withInitialOverrides,
} from "./utils/app-format";
import { safe } from "./utils/safe";
import {
  PlanComposerControls,
  PlanComposerTaskList,
} from "./pages/plan/plan-composer";
import { useProviderSettingsActions } from "./hooks/use-provider-settings-actions";
import { usePlanActions } from "./hooks/use-plan-actions";
import { AppShell } from "./app/app-shell";
export function App() {
  const e2eFixture = readSearchParam("e2eFixture");
  const initialTab = readMainTabSearchParam();
  const forceNewSession = readBooleanSearchParam("newSession");
  const disablePermissionRestrictions = readBooleanSearchParam(
    "disablePermissionRestrictions",
  );
  const initialSessionId = forceNewSession
    ? undefined
    : readSearchParam("sessionId");
  const initialModel = readSearchParam("model");
  const initialAgent = readSearchParam("agent");
  const [state, setState] = createSignal<AppState>(
    withInitialOverrides(
      e2eFixture
        ? fixtureAppState(defaultGatewayUrl(), e2eFixture)
        : initialAppState(defaultGatewayUrl()),
      {
        activeTab: initialTab,
        selectedSessionId: initialSessionId,
        selectedModel: initialModel,
        selectedAgent: initialAgent,
      },
    ),
  );
  const gatewayUrl = createMemo(() => state().gatewayUrl);
  const directory = createMemo(() => state().directory);
  const selectedSession = createMemo(() => activeSession(state()));
  const directoryClient = createMemo(
    () => new GatewayClient({ baseUrl: gatewayUrl(), directory: directory() }),
  );
  const rootClient = createMemo(
    () => new GatewayClient({ baseUrl: gatewayUrl() }),
  );
  const selectedMessages = createMemo(() => {
    const sessionId = state().selectedSessionId;
    return sessionId ? (state().messagesBySession[sessionId] ?? []) : [];
  });
  const slashCommands = createMemo(() => {
    const text = state().composerText.trim();
    if (!text.startsWith("/")) {
      return [];
    }
    const query = text.slice(1).toLowerCase();
    return state()
      .commands.filter((command) => command.name.toLowerCase().includes(query))
      .slice(0, 6);
  });
  const [expandedWorkspace, setExpandedWorkspace] = createSignal<string>();
  const [expandedRailGroup, setExpandedRailGroup] = createSignal<string>();
  const [workspaceTreeTouched, setWorkspaceTreeTouched] = createSignal(false);
  const [fileTree, setFileTree] = createSignal<Record<string, FileInfo[]>>({});
  const [fileLoadingPath, setFileLoadingPath] = createSignal<string>();
  const [fileContentLoadingPath, setFileContentLoadingPath] =
    createSignal<string>();
  const [expandedFileTreePaths, setExpandedFileTreePaths] = createSignal(
    new Set<string>(),
  );
  let fileContentRequestId = 0;
  const [acknowledgedAttentionSessions, setAcknowledgedAttentionSessions] =
    createSignal(new Set<string>());

  createEffect(() => {
    if (!workspaceTreeTouched() && state().directory) {
      setExpandedWorkspace(state().directory);
    }
  });

  createEffect(() => {
    document.documentElement.dataset.theme = state().themeMode;
  });

  createEffect(() => {
    if (e2eFixture) {
      return;
    }
    const baseUrl = gatewayUrl();
    const stream = connectGatewayEvents({
      baseUrl,
      onEvent: (event) =>
        setState((previous) =>
          eventBelongsToState(previous, event.directory)
            ? applyGatewayEvent(previous, event)
            : previous,
        ),
      onError: () =>
        setState((previous) => ({ ...previous, connection: "disconnected" })),
    });
    onCleanup(() => stream.close());
  });

  onMount(() => {
    if (!e2eFixture) {
      void hydrate();
    }
  });

  async function hydrate() {
    setState((previous) => ({
      ...previous,
      loading: true,
      connection: "connecting",
      error: undefined,
    }));
    const client = rootClient();
    try {
      const [health, serviceStatus, paths, config, currentProject, projects] =
        await Promise.all([
          client.health(),
          safe(() => client.serviceStatus(), undefined),
          client.paths(),
          client.config(),
          client.currentProject(),
          safe(() => client.projects(), []),
        ]);
      const [productConfig, me, workspaces, productIssues, productProjects] =
        await Promise.all([
          safe(() => client.productConfig(), undefined),
          safe(() => client.me(), undefined),
          safe(() => client.workspaces(), []),
          safe(() => client.productIssues(), []),
          safe(() => client.productProjects(), []),
        ]);
      const directory =
        paths.directory || currentProject.project?.worktree || paths.worktree;
      const scoped = client.withDirectory(directory);
      const [sessions, providers, agents, commands, files, workspaceConfig] =
        await Promise.all([
          safe(() => scoped.sessions({ limit: 100 }), []),
          safe(() => scoped.providers(), undefined),
          safe(() => scoped.agents(), []),
          safe(() => scoped.commands(), []),
          safe(() => scoped.files(), []),
          safe(() => scoped.workspaceConfig(), {}),
        ]);
      const providerAuthMethods = await safe(
        () => client.providerAuthMethods(),
        {},
      );
      const providerAuthStatusEntries = await Promise.all(
        (providers?.all ?? []).map(async (provider) => [
          provider.id,
          await safe(() => client.providerAuthStatus(provider.id), undefined),
        ]),
      );
      const providerAuthStatus = Object.fromEntries(
        providerAuthStatusEntries.filter(
          (entry): entry is [string, AppState["providerAuthStatus"][string]] =>
            !!entry[1],
        ),
      );
      const selectedSessionId = forceNewSession
        ? undefined
        : (state().selectedSessionId ?? sessions[0]?.id);
      const configuredModel =
        readConfigString(workspaceConfig, "model") ?? config.model;
      const configuredAgent =
        readConfigString(workspaceConfig, "active_agent") ?? config.agent;
      const configuredVariant = readConfigString(
        workspaceConfig,
        "model_variant",
      );
      const configuredAcceleration = readConfigBoolean(
        workspaceConfig,
        "model_acceleration_enabled",
      );
      setState((previous) => ({
        ...previous,
        health,
        serviceStatus,
        productConfig,
        me,
        workspaces,
        productIssues,
        productProjects,
        paths,
        config,
        configDraft: configToDraft(config),
        workspaceConfig,
        workspaceConfigDraft: recordToDraft(workspaceConfig),
        currentProject,
        projects,
        directory,
        sessions,
        providers,
        providerAuthMethods,
        providerAuthStatus,
        agents,
        commands,
        files,
        selectedSessionId,
        selectedAgent:
          previous.selectedAgent ??
          configuredAgent ??
          agents.find((agent) => !agent.hidden)?.name,
        selectedModel:
          previous.selectedModel ??
          configuredModel ??
          defaultModel(providers) ??
          "openai/gpt-5.5",
        selectedProviderId:
          previous.selectedProviderId ??
          providerIdFromModel(configuredModel) ??
          providerIdFromModel(previous.selectedModel) ??
          providers?.connected[0] ??
          providers?.all[0]?.id,
        themeMode:
          previous.bootstrapped || previous.themeMode !== "light"
            ? previous.themeMode
            : config.theme === "dark"
              ? "dark"
              : "light",
        modelVariant: previous.bootstrapped
          ? previous.modelVariant
          : (configuredVariant ?? previous.modelVariant ?? "low"),
        accelerationEnabled: previous.bootstrapped
          ? previous.accelerationEnabled
          : (configuredAcceleration ?? previous.accelerationEnabled ?? true),
        loading: false,
        bootstrapped: true,
        connection: "connected",
      }));
      if (selectedSessionId) {
        await openSession(selectedSessionId);
      }
    } catch (error) {
      setState((previous) => ({
        ...previous,
        loading: false,
        bootstrapped: true,
        connection: "disconnected",
        error: errorMessage(error),
      }));
    }
  }

  async function openSession(sessionId: string) {
    acknowledgeSessionAttention(sessionId);
    setState((previous) => ({
      ...previous,
      selectedSessionId: sessionId,
      error: undefined,
    }));
    const client = directoryClient();
    const existingMessages = state().messagesBySession[sessionId] ?? [];
    const [messages] = await Promise.all([
      e2eFixture && existingMessages.length > 0
        ? Promise.resolve(existingMessages)
        : safe(() => client.messages(sessionId), existingMessages),
    ]);
    setState((previous) => ({
      ...previous,
      messagesBySession: {
        ...previous.messagesBySession,
        [sessionId]: messages,
      },
    }));
  }

  async function newSession() {
    setState((previous) => ({ ...previous, error: undefined }));
    try {
      const session = await directoryClient().createSession(
        createSessionPayload(),
      );
      setState((previous) => ({
        ...previous,
        sessions: [
          session,
          ...previous.sessions.filter((item) => item.id !== session.id),
        ],
        selectedSessionId: session.id,
      }));
      await openSession(session.id);
    } catch (error) {
      if (!handleProviderAuthError(error)) {
        setState((previous) => ({ ...previous, error: errorMessage(error) }));
      }
    }
  }

  function openBlankSession() {
    setState((previous) => ({
      ...previous,
      activeTab: "new",
      previousMainTab: "new",
      selectedSessionId: undefined,
      composerText: "",
      error: undefined,
    }));
  }

  async function renameSession(sessionId: string, title: string) {
    const cleanTitle = title.trim();
    if (!cleanTitle) {
      return;
    }
    try {
      const session = await directoryClient().updateSession(sessionId, {
        name: cleanTitle,
      });
      setState((previous) => ({
        ...previous,
        sessions: previous.sessions.map((item) =>
          item.id === sessionId ? { ...item, ...session } : item,
        ),
      }));
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  function useWorkspaceDirectory(directory: string) {
    const workspaceDirectory = directory.trim();
    if (!workspaceDirectory) {
      return;
    }
    setState((previous) => ({
      ...previous,
      directory: workspaceDirectory,
      projects: previous.projects.some((project) =>
        samePath(project.worktree, workspaceDirectory),
      )
        ? previous.projects
        : [
            {
              id: workspaceDirectory,
              name: shortWorkspaceLabel(workspaceDirectory),
              worktree: workspaceDirectory,
            },
            ...previous.projects,
          ],
      activeTab: "new",
      previousMainTab: "new",
      selectedSessionId: undefined,
      sessions: samePath(previous.directory, workspaceDirectory)
        ? previous.sessions
        : [],
      composerText: "",
    }));
    setExpandedWorkspace(workspaceDirectory);
  }

  function activateWorkspaceProject(project: Project) {
    setState((previous) => ({
      ...previous,
      directory: project.worktree,
      projects: previous.projects.some((item) =>
        samePath(item.worktree, project.worktree),
      )
        ? previous.projects.map((item) =>
            samePath(item.worktree, project.worktree) ? project : item,
          )
        : [project, ...previous.projects],
      activeTab: "new",
      previousMainTab: "new",
      selectedSessionId: undefined,
      sessions: samePath(previous.directory, project.worktree)
        ? previous.sessions
        : [],
      composerText: "",
    }));
    setExpandedWorkspace(project.worktree);
  }

  async function createNamedWorkspace(name: string) {
    try {
      activateWorkspaceProject(await rootClient().createWorkspace({ name }));
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  async function pickExistingWorkspaceDirectory(): Promise<void> {
    if (e2eFixture) {
      setState((previous) => ({
        ...previous,
        error: "Mock 页面不能打开系统目录选择器，请在真实 gateway 连接后使用。",
      }));
      return;
    }
    if (state().connection !== "connected") {
      setState((previous) => ({
        ...previous,
        error: "Gateway 未连接，无法打开系统目录选择器。",
      }));
      return;
    }
    try {
      const project = await rootClient().selectLocalWorkspace({
        title: t("chooseWorkspace"),
      });
      if (project) {
        activateWorkspaceProject(project);
      }
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  async function openIssueConversation(issue: ProductIssue) {
    setState((previous) => ({
      ...previous,
      activeTab: "conversation",
      previousMainTab: "new",
      error: undefined,
    }));
    let sessionId = issue.session_id ?? issue.active_task?.session_id;
    try {
      if (!sessionId) {
        const session = await directoryClient().createSession(
          createSessionPayload(),
        );
        sessionId = session.id;
        setState((previous) => ({
          ...previous,
          sessions: [
            session,
            ...previous.sessions.filter((item) => item.id !== session.id),
          ],
          selectedSessionId: session.id,
        }));
        const linked = await rootClient().updateProductIssue(issue.id, {
          session_id: session.id,
        });
        if (linked) {
          setState((previous) => ({
            ...previous,
            productIssues: previous.productIssues.map((item) =>
              item.id === issue.id ? linked : item,
            ),
          }));
        }
      }
      await openSession(sessionId);
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  const {
    refreshProviderSurface,
    handleProviderAuthError,
    saveRuntimeSettings,
    validateSelectedModel,
    saveProviderKey,
    startProviderLogin,
    completeProviderLogin,
    logoutProvider,
  } = useProviderSettingsActions({
    state,
    setState,
    rootClient,
    directoryClient,
  });

  const {
    openPlanSession,
    selectDraftSession,
    sessionAttentionAcknowledged,
    updatePlanTicketStatus,
    updatePlanTicketTask,
    deletePlanTask,
    createSessionFromPlanTask,
    acknowledgeSessionAttention,
    updateEditingTaskFromComposer,
    createPlanTicket,
  } = usePlanActions({
    state,
    setState,
    directoryClient,
    e2eFixture,
    openSession,
    createSessionPayload,
    refreshSessions,
    handleProviderAuthError,
  });

  async function submitPrompt() {
    const raw = state().composerText.trim();
    if ((!raw && state().composerImages.length === 0) || state().submitting) {
      return;
    }
    if (await updateEditingTaskFromComposer()) {
      return;
    }
    setState((previous) => ({
      ...previous,
      submitting: true,
      error: undefined,
    }));
    try {
      let sessionId = state().selectedSessionId;
      if (!sessionId) {
        const session = await directoryClient().createSession(
          createSessionPayload(),
        );
        sessionId = session.id;
        setState((previous) => ({
          ...previous,
          sessions: [
            session,
            ...previous.sessions.filter((item) => item.id !== session.id),
          ],
          selectedSessionId: session.id,
          activeTab: "conversation",
          previousMainTab: "new",
        }));
      }
      const content =
        state().composerImages.length === 0
          ? await expandCommand(raw)
          : materializeComposerContent(raw, state().composerImages);
      await directoryClient().promptAsync(sessionId, {
        parts: [{ type: "text", text: content }],
        model: state().selectedModel,
        variant: state().modelVariant,
        model_acceleration_enabled: state().accelerationEnabled,
      });
      setState((previous) => ({
        ...previous,
        composerText: "",
        composerImages: [],
        activeTab: "conversation",
        previousMainTab: "new",
      }));
      await openSession(sessionId);
      await refreshSessions();
    } catch (error) {
      if (!handleProviderAuthError(error)) {
        setState((previous) => ({ ...previous, error: errorMessage(error) }));
      }
    } finally {
      setState((previous) => ({ ...previous, submitting: false }));
    }
  }

  async function expandCommand(input: string): Promise<string> {
    if (!input.startsWith("/")) {
      return input;
    }
    const [name, ...args] = input.slice(1).split(/\s+/);
    const match = state().commands.find((command) => command.name === name);
    if (!match) {
      return input;
    }
    const response = await directoryClient().executeCommand(name, args);
    return response.output || input;
  }

  function createSessionPayload() {
    const startAt = localDateTimeToUtcIso(state().planDraftStartAt);
    return {
      directory: state().directory,
      model: state().selectedModel,
      agent: state().selectedAgent,
      model_variant: state().modelVariant,
      model_acceleration_enabled: state().accelerationEnabled,
      disable_permission_restrictions: disablePermissionRestrictions,
      task_management: timedTaskPatch(
        state().planDraftStartCondition,
        startAt,
        state().planDraftPollInterval,
      ),
    };
  }

  async function refreshSessions() {
    const sessions = await safe(
      () => directoryClient().sessions({ limit: 100 }),
      state().sessions,
    );
    setState((previous) => ({ ...previous, sessions }));
  }

  async function refreshProduct() {
    const client = rootClient();
    const [productIssues, productProjects] = await Promise.all([
      safe(
        () => client.productIssues({ search: state().issueSearch }),
        state().productIssues,
      ),
      safe(() => client.productProjects(), state().productProjects),
    ]);
    setState((previous) => ({
      ...previous,
      productIssues,
      productProjects,
    }));
  }

  async function createIssue() {
    const title = state().issueDraft.trim();
    if (!title) {
      return;
    }
    const optimistic: ProductIssue = {
      id: `local-${Date.now()}`,
      workspace_id: state().workspaces[0]?.id ?? "local",
      number:
        Math.max(0, ...state().productIssues.map((issue) => issue.number)) + 1,
      title,
      description: "",
      status: "todo",
      priority: "medium",
      position: state().productIssues.length + 1,
      assignee_type: state().selectedAgent ? "agent" : null,
      assignee_id: state().selectedAgent ?? null,
      project_id: state().productProjects[0]?.id ?? null,
      labels: [],
      session_id: null,
      active_task: null,
      created_at: Date.now(),
      updated_at: Date.now(),
    };
    setState((previous) => ({
      ...previous,
      issueDraft: "",
      productIssues: [optimistic, ...previous.productIssues],
      error: undefined,
    }));
    try {
      const issue = await rootClient().createProductIssue({
        title,
        priority: "medium",
        status: "todo",
        assignee_type: state().selectedAgent ? "agent" : undefined,
        assignee_id: state().selectedAgent,
        project_id: state().productProjects[0]?.id,
      });
      setState((previous) => ({
        ...previous,
        productIssues: [
          issue,
          ...previous.productIssues.filter(
            (item) => item.id !== optimistic.id && item.id !== issue.id,
          ),
        ],
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        productIssues: previous.productIssues.map((item) =>
          item.id === optimistic.id ? { ...item, labels: ["local"] } : item,
        ),
      }));
    }
  }

  async function switchWorkspace(
    project: Project,
    options: { selectSession?: boolean } = {},
  ) {
    const selectSession = options.selectSession ?? true;
    const directory = project.worktree;
    if (e2eFixture) {
      const sessions = state().sessions.filter((session) =>
        samePath(sessionDirectory(session), directory),
      );
      setState((previous) => ({
        ...previous,
        directory,
        currentProject: { project },
        selectedSessionId: selectSession ? sessions[0]?.id : undefined,
        planPreviewSessionId: undefined,
        filePath: "",
        files: [],
        selectedFile: undefined,
        fileContent: undefined,
        loading: false,
        error: undefined,
      }));
      setFileTree({});
      setExpandedWorkspace(directory);
      return;
    }
    const scoped = rootClient().withDirectory(directory);
    setState((previous) => ({
      ...previous,
      directory,
      selectedSessionId: undefined,
      messagesBySession: {},
      todosBySession: {},
      files: [],
      filePath: "",
      selectedFile: undefined,
      fileContent: undefined,
      loading: true,
      error: undefined,
    }));
    try {
      const [currentProject, sessions, files] = await Promise.all([
        scoped.currentProject(),
        scoped.sessions({ limit: 100 }),
        safe(() => scoped.files(), []),
      ]);
      const selectedSessionId = selectSession ? sessions[0]?.id : undefined;
      setState((previous) => ({
        ...previous,
        currentProject,
        sessions,
        files,
        selectedSessionId,
        loading: false,
      }));
      setFileTree({ "": files });
      if (selectedSessionId) {
        await openSession(selectedSessionId);
      }
    } catch (error) {
      setState((previous) => ({
        ...previous,
        loading: false,
        error: errorMessage(error),
      }));
    }
  }

  async function toggleWorkspace(project: Project) {
    setWorkspaceTreeTouched(true);
    if (state().activeTab === "files") {
      setExpandedWorkspace(project.worktree);
      setExpandedRailGroup(undefined);
      if (!samePath(project.worktree, state().directory)) {
        await switchWorkspace(project, { selectSession: false });
        return;
      }
      await loadFiles("");
      return;
    }
    if (expandedWorkspace() === project.worktree) {
      setExpandedWorkspace(undefined);
      return;
    }
    setExpandedWorkspace(project.worktree);
    setExpandedRailGroup(undefined);
    if (!samePath(project.worktree, state().directory)) {
      await switchWorkspace(project);
    }
  }

  function toggleRailGroup(id: string) {
    setExpandedRailGroup((previous) => (previous === id ? undefined : id));
  }

  async function readFiles(path = "") {
    return e2eFixture
      ? fixtureFiles(e2eFixture, path)
      : await safe(() => directoryClient().files(path), []);
  }

  async function loadFiles(path = "") {
    setFileLoadingPath(path);
    setFileContentLoadingPath(undefined);
    const files = await readFiles(path);
    setFileTree((previous) => ({ ...previous, [path]: files }));
    setState((previous) => ({
      ...previous,
      files,
      filePath: path,
      selectedFile: undefined,
      fileContent: undefined,
    }));
    setFileLoadingPath(undefined);
  }

  async function toggleFileTreeDirectory(file: FileInfo) {
    if (file.type !== "directory") {
      await openFile(file);
      return;
    }
    if (expandedFileTreePaths().has(file.path)) {
      setExpandedFileTreePaths((previous) => {
        const next = new Set(previous);
        next.delete(file.path);
        return next;
      });
      return;
    }
    setExpandedFileTreePaths((previous) => {
      const next = new Set(previous);
      next.add(file.path);
      return next;
    });
    setFileLoadingPath(file.path);
    const files = fileTree()[file.path] ?? (await readFiles(file.path));
    setFileTree((previous) => ({ ...previous, [file.path]: files }));
    setState((previous) => ({
      ...previous,
      files,
      filePath: file.path,
      selectedFile: undefined,
      fileContent: undefined,
    }));
    setFileContentLoadingPath(undefined);
    setFileLoadingPath(undefined);
  }

  createEffect(() => {
    if (
      state().activeTab === "files" &&
      state().files.length === 0 &&
      fileLoadingPath() === undefined &&
      fileTree()[""] === undefined
    ) {
      void loadFiles("");
    }
  });

  async function openFile(file: FileInfo) {
    if (file.type === "directory") {
      setExpandedFileTreePaths((previous) => {
        const next = new Set(previous);
        next.add(file.path);
        return next;
      });
      await loadFiles(file.path);
      return;
    }
    const requestId = ++fileContentRequestId;
    setFileContentLoadingPath(file.path);
    setState((previous) => ({
      ...previous,
      selectedFile: file,
      fileContent: undefined,
    }));
    const fileContent = e2eFixture
      ? fixtureFileContent(e2eFixture, file.path)
      : await safe(
          () => directoryClient().fileContent(file.path),
          undefined as FileContentResponse | undefined,
        );
    if (requestId !== fileContentRequestId) {
      return;
    }
    setFileContentLoadingPath(undefined);
    setState((previous) =>
      previous.selectedFile?.path === file.path
        ? { ...previous, fileContent }
        : previous,
    );
  }

  async function openSelectedFile() {
    const file = state().selectedFile;
    if (!file) {
      return;
    }
    if (e2eFixture) {
      return;
    }
    if (state().connection !== "connected") {
      setState((previous) => ({
        ...previous,
        error: "Gateway 未连接，无法调用系统默认应用打开文件。",
      }));
      return;
    }
    try {
      await directoryClient().openFile(file.path);
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  async function openCurrentDirectory() {
    if (e2eFixture) {
      return;
    }
    if (state().connection !== "connected") {
      setState((previous) => ({
        ...previous,
        error: "Gateway 未连接，无法在系统文件浏览器中打开。",
      }));
      return;
    }
    try {
      const selected = state().selectedFile;
      await directoryClient().openFileLocation(
        selected?.path ?? state().filePath,
      );
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  function openSettings(section: SettingsSection = state().settingsSection) {
    setState((previous) => ({
      ...previous,
      previousMainTab:
        previous.activeTab === "settings"
          ? previous.previousMainTab
          : previous.activeTab,
      activeTab: "settings",
      settingsSection: section,
      settingsNotice: undefined,
      modelValidation: undefined,
    }));
    void refreshProviderSurface();
  }

  function closeSettings() {
    setState((previous) => ({
      ...previous,
      activeTab: previous.previousMainTab,
      settingsNotice: undefined,
      modelValidation: undefined,
    }));
  }

  function latestConversationSessionId(): string | undefined {
    return [...state().sessions]
      .filter((session) => planSessionStatus(session) !== "archived")
      .sort(
        (left, right) =>
          normalizeTimeMs(sessionUpdatedAt(right) ?? 0) -
          normalizeTimeMs(sessionUpdatedAt(left) ?? 0),
      )[0]?.id;
  }

  async function changeMainTab(activeTab: Exclude<MainTab, "settings">) {
    const selectedSessionId =
      activeTab === "new"
        ? undefined
        : activeTab === "conversation"
          ? (state().selectedSessionId ?? latestConversationSessionId())
          : state().selectedSessionId;
    setState((previous) => ({
      ...previous,
      activeTab,
      previousMainTab: activeTab === "conversation" ? "new" : activeTab,
      selectedSessionId,
    }));
    if (activeTab === "conversation" && selectedSessionId) {
      await openSession(selectedSessionId);
    }
    if (activeTab === "files" && state().files.length === 0) {
      void loadFiles("");
    }
  }

  return (
    <AppShell
      view={{
        state,
        closeSettings,
        changeMainTab,
        expandedRailGroup,
        setExpandedRailGroup,
        toggleRailGroup,
        selectedSession,
        selectedMessages,
        slashCommands,
        openBlankSession,
        openSession,
        newSession,
        useWorkspaceDirectory,
        createNamedWorkspace,
        pickExistingWorkspaceDirectory,
        setState,
        submitPrompt,
        updatePlanTicketStatus,
        sessionAttentionAcknowledged,
        deletePlanTask,
        openPlanSession,
        selectDraftSession,
        createPlanTicket,
        createSessionFromPlanTask,
        updatePlanTicketTask,
        fileTree,
        fileLoadingPath,
        fileContentLoadingPath,
        expandedFileTreePaths,
        expandedWorkspace,
        setExpandedWorkspace,
        setWorkspaceTreeTouched,
        loadFiles,
        openFile,
        toggleFileTreeDirectory,
        renameSession,
        openSettings,
        openIssueConversation,
        refreshProduct,
        switchWorkspace,
        toggleWorkspace,
        directory,
        openCurrentDirectory,
        openSelectedFile,
        saveRuntimeSettings,
        validateSelectedModel,
        saveProviderKey,
        startProviderLogin,
        completeProviderLogin,
        logoutProvider,
        e2eFixture,
        providerAuthPanel: state().providerAuthPanel,
      }}
    />
  );
}
