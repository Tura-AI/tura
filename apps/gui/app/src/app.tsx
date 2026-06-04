import {
  GatewayClient,
  GatewayError,
  connectGatewayEvents,
  defaultGatewayUrl,
  errorMessage,
  type Agent,
  type AgentConfig,
  type AgentUpsertRequest,
  type FileContentResponse,
  type FileInfo,
  type Message,
  type PlanStatus,
  type ProductIssue,
  type Project,
  type Session,
  type StartCondition,
  type StoredAgent,
} from "@tura/gateway-sdk";
import {
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import { AppShell } from "./app/app-shell";
import { AppProviders } from "./context/app-providers";
import { DEFAULT_MODEL_ID } from "./config/defaults";
import {
  appendTaskToSession,
  defaultLocalStartAt,
  defaultPollInterval,
  localDateTimeToUtcIso,
  materializeComposerContent,
  sessionTasks,
  timedTaskPatch,
} from "./features/plan/tasks";
import { usePlanActions } from "./hooks/use-plan-actions";
import { useProviderSettingsActions } from "./hooks/use-provider-settings-actions";
import { t } from "./i18n";
import { agentDisplayName } from "./utils/agent-display";
import {
  fixtureAppState,
  fixtureFileContent,
  fixtureFiles,
} from "./test/fixtures/app-fixtures";
import { applyGatewayEvent } from "./state/event-reducer";
import {
  activeSession,
  initialAppState,
  sessionDirectory,
  sessionHasDisplayName,
  systemThemeMode,
  withSessionFallbackName,
  type AppState,
  type MainTab,
  type SettingsSection,
  type ThemeMode,
} from "./state/global-store";
import {
  eventBelongsToState,
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
  configToDraft,
  defaultModel,
  providerIdFromAuthError,
  providerIdFromModel,
  recordToDraft,
} from "./utils/settings";

const PROMPT_RESPONSE_TIMEOUT_MS = 30_000;
const PROMPT_RESPONSE_TIMEOUT_CODE = "GATEWAY_NO_RESPONSE_30S";
const GATEWAY_CONNECT_TIMEOUT_MS = 5_000;
const LAST_SESSION_OPENED_STORAGE_KEY = "last_session_opened";
const LEGACY_LAST_SESSION_OPENED_STORAGE_KEY = "last cession oppend";
let lastSessionOpenedMemory: string | undefined;

function readLastSessionOpened(): string | undefined {
  let stored: string | undefined;
  if (typeof window === "undefined") {
    return lastSessionOpenedMemory;
  }
  try {
    stored =
      window.localStorage.getItem(LAST_SESSION_OPENED_STORAGE_KEY)?.trim() ||
      window.localStorage
        .getItem(LEGACY_LAST_SESSION_OPENED_STORAGE_KEY)
        ?.trim() ||
      undefined;
    if (stored) {
      window.localStorage.setItem(LAST_SESSION_OPENED_STORAGE_KEY, stored);
      window.localStorage.removeItem(LEGACY_LAST_SESSION_OPENED_STORAGE_KEY);
    }
  } catch {
    stored = undefined;
  }
  return stored ?? lastSessionOpenedMemory;
}

function writeLastSessionOpened(sessionId: string) {
  lastSessionOpenedMemory = sessionId;
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(LAST_SESSION_OPENED_STORAGE_KEY, sessionId);
    window.localStorage.removeItem(LEGACY_LAST_SESSION_OPENED_STORAGE_KEY);
  } catch {
    // Memory fallback keeps tab navigation deterministic when storage is blocked.
  }
}

function clearLastSessionOpened() {
  lastSessionOpenedMemory = undefined;
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.removeItem(LAST_SESSION_OPENED_STORAGE_KEY);
    window.localStorage.removeItem(LEGACY_LAST_SESSION_OPENED_STORAGE_KEY);
  } catch {
    // Nothing else to clear when storage is blocked.
  }
}

function providerIssueIdFromError(
  error: unknown,
  state: AppState,
): string | undefined {
  const authProvider = providerIdFromAuthError(error, state);
  if (authProvider) {
    return authProvider;
  }
  if (!(error instanceof GatewayError)) {
    return undefined;
  }
  const bodyText = JSON.stringify(error.body ?? {}).toLowerCase();
  const messageText = error.message.toLowerCase();
  const billingLike =
    error.status === 402 ||
    /\b(billing|payment|quota|credit|balance|insufficient|subscription|rate_limit|rate limit|limit exceeded)\b/u.test(
      `${bodyText} ${messageText}`,
    );
  return billingLike ? providerIdFromModel(state.selectedModel) : undefined;
}

function mergeSessions(remoteSessions: Session[], localSessions: Session[]) {
  const byId = new Map<string, Session>();
  for (const session of remoteSessions) {
    byId.set(session.id, session);
  }
  for (const session of localSessions) {
    const remote = byId.get(session.id);
    if (!remote) {
      byId.set(session.id, session);
    } else if (
      !sessionHasDisplayName(remote) &&
      sessionHasDisplayName(session)
    ) {
      byId.set(session.id, {
        ...remote,
        name: session.name,
        session_display_name: session.session_display_name,
        plan_summary: session.plan_summary,
      });
    }
  }
  return [...byId.values()].sort(
    (a, b) => (b.updated_at ?? 0) - (a.updated_at ?? 0),
  );
}

function isGatewayTimeoutError(error: unknown): boolean {
  if (
    error instanceof DOMException &&
    (error.name === "AbortError" || error.name === "TimeoutError")
  ) {
    return true;
  }
  return (
    error instanceof TypeError &&
    error.message.toLowerCase() === "failed to fetch"
  );
}

export function App() {
  const e2eFixture = readSearchParam("e2eFixture");
  const requestedTab = readSearchParam("tab");
  const initialTab = readMainTabSearchParam();
  const forceNewSession =
    readBooleanSearchParam("newSession") || requestedTab === "new";
  const disablePermissionRestrictions = readBooleanSearchParam(
    "disablePermissionRestrictions",
  );
  const initialSessionId = forceNewSession
    ? null
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
  const e2eStoredAgents = new Map<string, StoredAgent>();
  let fileContentRequestId = 0;

  createEffect(() => {
    if (!workspaceTreeTouched() && state().directory) {
      setExpandedWorkspace(state().directory);
    }
  });

  createEffect(() => {
    if (e2eFixture || state().connection === "connected" || state().error) {
      return;
    }
    const timer = window.setTimeout(() => {
      setState((previous) =>
        previous.connection === "connected" || previous.error
          ? previous
          : {
              ...previous,
              loading: false,
              bootstrapped: true,
              connection: "disconnected",
              error: t("gatewayResponseTimeout"),
            },
      );
    }, GATEWAY_CONNECT_TIMEOUT_MS);
    onCleanup(() => window.clearTimeout(timer));
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
      const [
        health,
        serviceStatus,
        paths,
        config,
        modelConfig,
        currentProject,
        projects,
      ] = await Promise.all([
        client.health(),
        safe(() => client.serviceStatus(), undefined),
        client.paths(),
        client.config(),
        safe(() => client.modelConfig(), undefined),
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
      const [
        sessions,
        providers,
        agents,
        personas,
        commands,
        files,
        workspaceConfig,
      ] = await Promise.all([
        safe(() => scoped.sessions({ limit: 100 }), []),
        safe(() => scoped.providers(), undefined),
        safe(() => scoped.agents(), []),
        safe(() => scoped.personas(), []),
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
        modelConfig,
        configDraft: configToDraft(config),
        workspaceConfig,
        workspaceConfigDraft: recordToDraft(workspaceConfig),
        currentProject,
        projects,
        directory,
        sessions: mergeSessions(sessions, previous.sessions),
        providers,
        providerAuthMethods,
        providerAuthStatus,
        agents,
        personas,
        commands,
        files,
        selectedSessionId: previous.selectedSessionId ?? selectedSessionId,
        selectedAgent:
          previous.selectedAgent ??
          configuredAgent ??
          agents.find((agent) => !agent.hidden)?.name,
        selectedModel:
          previous.selectedModel ??
          configuredModel ??
          defaultModel(providers) ??
          DEFAULT_MODEL_ID,
        selectedProviderId:
          previous.selectedProviderId ??
          providerIdFromModel(configuredModel) ??
          providerIdFromModel(previous.selectedModel) ??
          providers?.connected[0] ??
          providers?.all[0]?.id,
        themeMode: previous.bootstrapped
          ? previous.themeMode
          : normalizeThemeMode(config.theme),
        mainFont: previous.bootstrapped
          ? previous.mainFont
          : (config.main_font ?? previous.mainFont),
        codeFont: previous.bootstrapped
          ? previous.codeFont
          : (config.code_font ?? previous.codeFont),
        mainFontSize: previous.bootstrapped
          ? previous.mainFontSize
          : clampNumber(config.main_font_size, 11, 15, 12),
        codeFontSize: previous.bootstrapped
          ? previous.codeFontSize
          : clampNumber(config.code_font_size, 9, 15, 11),
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
        error: isGatewayTimeoutError(error)
          ? t("gatewayResponseTimeout")
          : errorMessage(error),
      }));
    }
  }

  async function openSession(
    sessionId: string,
    options: { forceRefreshMessages?: boolean } = {},
  ) {
    writeLastSessionOpened(sessionId);
    acknowledgeSessionAttention(sessionId);
    setState((previous) => ({
      ...previous,
      lastSessionOpenedId: sessionId,
      selectedSessionId: sessionId,
      error: undefined,
    }));
    const client = directoryClient();
    const existingMessages = state().messagesBySession[sessionId] ?? [];
    const [messages] = await Promise.all([
      e2eFixture && existingMessages.length > 0 && !options.forceRefreshMessages
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

  function openBlankSession() {
    const currentSessionId = state().selectedSessionId;
    if (currentSessionId) {
      writeLastSessionOpened(currentSessionId);
    }
    setState((previous) => ({
      ...previous,
      lastSessionOpenedId:
        previous.selectedSessionId ?? previous.lastSessionOpenedId,
      activeTab: "conversation",
      previousMainTab: "conversation",
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
        auto_session_name: false,
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
      activeTab: "conversation",
      previousMainTab: "conversation",
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
      activeTab: "conversation",
      previousMainTab: "conversation",
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
      previousMainTab: "conversation",
      error: undefined,
    }));
    let sessionId = issue.session_id ?? issue.active_task?.session_id;
    try {
      if (!sessionId) {
        const session = withSessionFallbackName(
          await directoryClient().createSession(createSessionPayload()),
          issue.title,
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
    saveRuntimeSettings,
    updateModelTier,
    saveProviderKey,
    validateProvider,
    startProviderLogin,
    completeProviderLogin,
    logoutProvider,
  } = useProviderSettingsActions({
    state,
    setState,
    rootClient,
    directoryClient,
  });

  async function refreshAgents() {
    if (e2eFixture) {
      return;
    }
    const [agents, personas] = await Promise.all([
      safe(() => directoryClient().agents(), state().agents),
      safe(() => directoryClient().personas(), state().personas),
    ]);
    setState((previous) => ({ ...previous, agents, personas }));
  }

  async function getAgent(agentId: string): Promise<StoredAgent | undefined> {
    if (e2eFixture) {
      const storedAgent = e2eStoredAgents.get(agentId);
      if (storedAgent) {
        return storedAgent;
      }
      const agent = state().agents.find((item) => item.name === agentId);
      return agent ? storedAgentFromRuntimeAgent(agent) : undefined;
    }
    try {
      return await directoryClient().agent(agentId);
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
      return undefined;
    }
  }

  async function saveAgent(
    agentId: string | undefined,
    payload: AgentUpsertRequest,
  ) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      if (e2eFixture) {
        const nextAgent = runtimeAgentFromUpsert(agentId, payload);
        e2eStoredAgents.set(
          nextAgent.name,
          storedAgentFromUpsert(nextAgent, payload),
        );
        setState((previous) => ({
          ...previous,
          agents: [
            nextAgent,
            ...previous.agents.filter((agent) => agent.name !== nextAgent.name),
          ],
          settingsSaving: false,
          settingsNotice: t("saved"),
        }));
        return;
      }
      await (agentId
        ? directoryClient().updateAgent(agentId, payload)
        : directoryClient().createAgent(payload));
      const agents = await safe(
        () => directoryClient().agents(),
        state().agents,
      );
      setState((previous) => ({
        ...previous,
        agents,
        settingsSaving: false,
        settingsNotice: t("saved"),
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function deleteAgent(agentId: string) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      if (e2eFixture) {
        e2eStoredAgents.delete(agentId);
        setState((previous) => ({
          ...previous,
          agents: previous.agents.filter((agent) => agent.name !== agentId),
          selectedAgent:
            previous.selectedAgent === agentId
              ? undefined
              : previous.selectedAgent,
          settingsSaving: false,
          settingsNotice: t("saved"),
        }));
        return;
      }
      await directoryClient().deleteAgent(agentId);
      const agents = await safe(
        () => directoryClient().agents(),
        state().agents,
      );
      setState((previous) => ({
        ...previous,
        agents,
        selectedAgent:
          previous.selectedAgent === agentId
            ? undefined
            : previous.selectedAgent,
        settingsSaving: false,
        settingsNotice: t("saved"),
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  const {
    openPlanSession,
    selectDraftSession,
    sessionAttentionAcknowledged,
    updatePlanTicketStatus,
    updatePlanTicketTask,
    reorderPlanTasks,
    deletePlanTask,
    runPlanTaskNow,
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
  });

  async function submitPrompt(options: { queued?: boolean } = {}) {
    if (await updateEditingTaskFromComposer()) {
      return;
    }
    const raw = state().composerText.trim();
    if ((!raw && state().composerImages.length === 0) || state().submitting) {
      return;
    }
    setState((previous) => ({
      ...previous,
      submitting: true,
      error: undefined,
      planNotice: undefined,
    }));
    let optimisticSessionId: string | undefined;
    let optimisticId: string | undefined;
    try {
      const content =
        state().composerImages.length === 0
          ? await expandCommand(raw)
          : materializeComposerContent(raw, state().composerImages);
      if (options.queued) {
        await submitQueuedPrompt(content, "session_idle");
        return;
      }
      if (state().planDraftStartCondition !== "user_action") {
        await submitQueuedPrompt(content);
        return;
      }
      let sessionId = state().selectedSessionId;
      let createdSession: Session | undefined;
      if (!sessionId) {
        const session = withSessionFallbackName(
          await directoryClient().createSession(createSessionPayload()),
          content,
        );
        sessionId = session.id;
        createdSession = session;
      }
      optimisticSessionId = sessionId;
      optimisticId = `prompt:${sessionId}:${Date.now()}`;
      const now = Date.now();
      const optimisticMessage: Message = {
        id: optimisticId,
        sessionID: sessionId,
        session_id: sessionId,
        role: "user",
        created_at: now,
        updated_at: now,
        time: { created: now, updated: now },
        parts: [
          {
            id: `${optimisticId}:text`,
            type: "text",
            text: content,
            metadata: { planRunPending: true },
          },
        ],
      };
      setState((previous) => ({
        ...previous,
        selectedSessionId: sessionId,
        sessions: createdSession
          ? [
              { ...createdSession, status: "busy" },
              ...previous.sessions.filter(
                (session) => session.id !== sessionId,
              ),
            ]
          : previous.sessions.map((session) =>
              session.id === sessionId
                ? { ...session, status: "busy" }
                : session,
            ),
        messagesBySession: {
          ...previous.messagesBySession,
          [sessionId]: [
            ...(previous.messagesBySession[sessionId] ?? []).filter(
              (message) => message.id !== optimisticId,
            ),
            optimisticMessage,
          ],
        },
      }));
      await Promise.race([
        directoryClient().promptAsync(sessionId, {
          parts: [{ type: "text", text: content }],
          model: state().selectedModel,
          agent: state().selectedAgent,
        }),
        new Promise<never>((_, reject) =>
          window.setTimeout(
            () => reject(new Error(PROMPT_RESPONSE_TIMEOUT_CODE)),
            PROMPT_RESPONSE_TIMEOUT_MS,
          ),
        ),
      ]);
      setState((previous) => ({
        ...previous,
        selectedSessionId: sessionId,
        composerText: "",
        composerImages: [],
        activeTab: "conversation",
        previousMainTab: "conversation",
        planNotice: undefined,
      }));
      await openSession(sessionId, { forceRefreshMessages: true });
      setState((previous) => ({
        ...previous,
        selectedSessionId: sessionId,
        composerText: "",
        composerImages: [],
        activeTab: "conversation",
        previousMainTab: "conversation",
        planNotice: undefined,
      }));
      await refreshSessions();
    } catch (error) {
      const timeout =
        error instanceof Error &&
        error.message === PROMPT_RESPONSE_TIMEOUT_CODE;
      setState((previous) => ({
        ...previous,
        messagesBySession:
          optimisticSessionId && optimisticId
            ? {
                ...previous.messagesBySession,
                [optimisticSessionId]: (
                  previous.messagesBySession[optimisticSessionId] ?? []
                ).map((message) =>
                  message.id === optimisticId
                    ? {
                        ...message,
                        updated_at: Date.now(),
                        time: { ...message.time, updated: Date.now() },
                        parts: message.parts.map((part) => ({
                          ...part,
                          metadata: {
                            ...(typeof part.metadata === "object" &&
                            part.metadata !== null
                              ? part.metadata
                              : {}),
                            planRunPending: false,
                            planRunError: true,
                          },
                        })),
                      }
                    : message,
                ),
              }
            : previous.messagesBySession,
        planNotice: timeout
          ? {
              message: "Gateway 30 秒内没有响应请求。",
              code: PROMPT_RESPONSE_TIMEOUT_CODE,
            }
          : {
              message: errorMessage(error),
              code: "GATEWAY_PROMPT_FAILED",
              providerId: providerIssueIdFromError(error, previous),
            },
        error: undefined,
      }));
    } finally {
      setState((previous) => ({ ...previous, submitting: false }));
    }
  }

  async function abortSession(sessionId: string) {
    setState((previous) => ({
      ...previous,
      submitting: false,
      sessions: previous.sessions.map((session) =>
        session.id === sessionId ? { ...session, status: "idle" } : session,
      ),
      error: undefined,
    }));
    if (e2eFixture) {
      return;
    }
    try {
      await directoryClient().abort(sessionId);
      await refreshSessions();
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
      await refreshSessions();
    }
  }

  async function submitQueuedPrompt(
    content: string,
    forcedStartCondition?: StartCondition,
  ) {
    const startCondition =
      forcedStartCondition ?? state().planDraftStartCondition;
    const startAt =
      startCondition === "scheduled_task" || startCondition === "polling_task"
        ? (localDateTimeToUtcIso(
            state().planDraftStartAt || defaultLocalStartAt(),
          ) ?? localDateTimeToUtcIso(defaultLocalStartAt()))
        : undefined;
    const [summaryLine = "", ...deliverableLines] = content.split(/\r?\n/u);
    const title = summaryLine.trim() || t("newTask");
    const timingPatch = timedTaskPatch(
      startCondition,
      startAt,
      state().planDraftPollInterval,
    );
    const currentSession = state().selectedSessionId
      ? state().sessions.find(
          (session) => session.id === state().selectedSessionId,
        )
      : undefined;
    const nonceId = currentSession
      ? `${currentSession.id}:${Date.now()}`
      : `queued-task:${Date.now()}`;
    const taskState = {
      task_id: nonceId,
      step: currentSession ? sessionTasks(currentSession).length + 1 : 1,
      status: "todo" as PlanStatus,
      plan_summary: title,
      task_summary: title,
      deliverable: deliverableLines.join("\n").trim(),
      ...timingPatch,
    };
    if (e2eFixture) {
      const session: Session = currentSession
        ? {
            ...appendTaskToSession(currentSession, taskState),
            updated_at: Date.now(),
          }
        : {
            id: `queued-local-${Date.now()}`,
            name: title,
            directory: state().directory,
            status: "idle",
            created_at: Date.now(),
            updated_at: Date.now(),
            plan_summary: title,
            session_display_name: title,
            task_management: taskState,
          };
      setState((previous) => ({
        ...previous,
        sessions: [
          session,
          ...previous.sessions.filter((item) => item.id !== session.id),
        ],
        selectedSessionId: session.id,
        planPreviewSessionId:
          previous.activeTab === "plan"
            ? session.id
            : previous.planPreviewSessionId,
        activeTab: previous.activeTab,
        previousMainTab: previous.previousMainTab,
        composerText: "",
        composerImages: [],
        planDraftStartCondition: "user_action",
        planDraftStartAt: "",
        planDraftPollInterval: defaultPollInterval(),
        planNotice: undefined,
        error: undefined,
      }));
      return;
    }
    const session = currentSession
      ? await directoryClient().updateSessionTaskManagement(currentSession.id, {
          tasks: [taskState],
        })
      : withSessionFallbackName(
          await directoryClient().createSession({
            ...createSessionPayload(),
            task_management: taskState,
          }),
          title,
        );
    setState((previous) => ({
      ...previous,
      sessions: [
        session,
        ...previous.sessions.filter((item) => item.id !== session.id),
      ],
      selectedSessionId: session.id,
      planPreviewSessionId:
        previous.activeTab === "plan"
          ? session.id
          : previous.planPreviewSessionId,
      activeTab: previous.activeTab,
      previousMainTab: previous.previousMainTab,
      composerText: "",
      composerImages: [],
      planDraftStartCondition: "user_action",
      planDraftStartAt: "",
      planDraftPollInterval: defaultPollInterval(),
      planNotice: undefined,
      error: undefined,
    }));
    await refreshSessions();
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
      auto_session_name: true,
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
    setState((previous) => ({
      ...previous,
      sessions: mergeSessions(sessions, previous.sessions),
    }));
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
      setExpandedRailGroup(undefined);
      if (
        expandedWorkspace() === project.worktree &&
        samePath(project.worktree, state().directory)
      ) {
        setExpandedWorkspace(undefined);
        return;
      }
      setExpandedWorkspace(project.worktree);
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
    }));
    void refreshProviderSurface();
  }

  function closeSettings() {
    setState((previous) => ({
      ...previous,
      activeTab: previous.previousMainTab,
      settingsNotice: undefined,
    }));
  }

  async function changeMainTab(activeTab: Exclude<MainTab, "settings">) {
    const lastSessionId =
      state().lastSessionOpenedId ??
      readLastSessionOpened() ??
      (state().activeTab === "conversation"
        ? undefined
        : (state().selectedSessionId ?? state().planPreviewSessionId));
    const lastSession = lastSessionId
      ? state().sessions.find((session) => session.id === lastSessionId)
      : undefined;

    if (activeTab === "conversation") {
      if (state().activeTab === "conversation") {
        openBlankSession();
        return;
      }
      if (!lastSessionId || !lastSession) {
        if (lastSessionId) {
          clearLastSessionOpened();
        }
        setState((previous) => ({
          ...previous,
          lastSessionOpenedId: undefined,
        }));
        openBlankSession();
        return;
      }
      setState((previous) => ({
        ...previous,
        activeTab: "conversation",
        previousMainTab: "conversation",
        selectedSessionId: lastSessionId,
        error: undefined,
      }));
      await openSession(lastSessionId);
      return;
    }

    if (activeTab === "plan") {
      if (lastSessionId && !lastSession) {
        clearLastSessionOpened();
        setState((previous) => ({
          ...previous,
          lastSessionOpenedId: undefined,
        }));
      }
      setState((previous) => ({
        ...previous,
        activeTab: "plan",
        previousMainTab: "plan",
        selectedSessionId: lastSession?.id ?? previous.selectedSessionId,
        planPreviewSessionId: lastSession?.id ?? previous.planPreviewSessionId,
        error: undefined,
      }));
      if (lastSession) {
        await openSession(lastSession.id);
      }
      return;
    }

    const selectedSessionId = state().selectedSessionId;
    setState((previous) => ({
      ...previous,
      activeTab,
      previousMainTab: activeTab,
      selectedSessionId,
    }));
    if (activeTab === "files" && state().files.length === 0) {
      void loadFiles("");
    }
  }

  return (
    <AppProviders state={state} setState={setState} gatewayUrl={gatewayUrl}>
      <AppShell
        view={{
          state,
          closeSettings,
          changeMainTab,
          expandedRailGroup,
          toggleRailGroup,
          selectedSession,
          selectedMessages,
          slashCommands,
          openBlankSession,
          openSession,
          useWorkspaceDirectory,
          createNamedWorkspace,
          pickExistingWorkspaceDirectory,
          setState,
          submitPrompt,
          abortSession,
          updatePlanTicketStatus,
          sessionAttentionAcknowledged,
          deletePlanTask,
          runPlanTaskNow,
          openPlanSession,
          selectDraftSession,
          createPlanTicket,
          createSessionFromPlanTask,
          updatePlanTicketTask,
          reorderPlanTasks,
          updateEditingTaskFromComposer,
          fileTree,
          fileLoadingPath,
          fileContentLoadingPath,
          expandedFileTreePaths,
          expandedWorkspace,
          loadFiles,
          openFile,
          toggleFileTreeDirectory,
          renameSession,
          openSettings,
          openIssueConversation,
          toggleWorkspace,
          openCurrentDirectory,
          openSelectedFile,
          saveRuntimeSettings,
          updateModelTier,
          refreshAgents,
          getAgent,
          saveAgent,
          deleteAgent,
          saveProviderKey,
          validateProvider,
          startProviderLogin,
          completeProviderLogin,
          logoutProvider,
        }}
      />
    </AppProviders>
  );
}

function normalizeThemeMode(value: string | null | undefined): ThemeMode {
  return value === "light" ||
    value === "dark" ||
    value === "caral" ||
    value === "uruk" ||
    value === "liangzhu"
    ? value
    : systemThemeMode();
}

function storedAgentFromRuntimeAgent(agent: Agent): StoredAgent {
  const capabilities = agentCapabilitiesFromOptions(agent.options);
  const displayName = agentDisplayName(agent);
  return {
    summary: {
      id: agent.name,
      name: displayName,
      description: agent.description,
      source: agent.native ? "static" : "dynamic",
      path: "",
      aliases: [],
      capabilities,
      provider:
        agentProviderTierFromOptions(agent.options) ??
        agent.model?.providerID ??
        null,
      hidden: agent.hidden,
    },
    config: {
      agent_name: displayName,
      description: agent.description,
      aliases: [],
      provider: {
        tura_llm_name:
          agentProviderTierFromOptions(agent.options) ?? "thinking",
      },
      agent_capabilities: capabilities.map((capability) => ({
        capability_name: capability,
        capability_directory: "crates/tools/src",
      })),
    },
    prompt: "",
  };
}

function runtimeAgentFromUpsert(
  agentId: string | undefined,
  payload: AgentUpsertRequest,
): Agent {
  const name = payload.config?.agent_name || payload.id || agentId || "agent";
  return {
    name: agentId ?? payload.id ?? name,
    description: payload.config?.description ?? "",
    mode: "custom",
    native: false,
    hidden: false,
    model: null,
    options: {
      ...(payload.config?.avatar ? { avatar: payload.config.avatar } : {}),
      ...(payload.config?.provider
        ? { provider: payload.config.provider }
        : {}),
      capabilities: readCapabilityArray(payload.config?.agent_capabilities),
    },
    permission: { allow: [], deny: [] },
  };
}

function storedAgentFromUpsert(
  agent: Agent,
  payload: AgentUpsertRequest,
): StoredAgent {
  const config: AgentConfig = payload.config ?? { agent_name: agent.name };
  const aliases = readStringArray(config.aliases);
  const capabilities = readCapabilityArray(config.agent_capabilities);
  return {
    summary: {
      id: agent.name,
      name: config.agent_name ?? agent.name,
      description: config.description ?? agent.description ?? "",
      source: "dynamic",
      path: "",
      aliases,
      capabilities,
      provider: agent.model?.providerID ?? null,
      hidden: agent.hidden,
    },
    config,
    prompt: payload.prompt ?? "",
  };
}

function readStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function readCapabilityArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value
        .map((item) => {
          if (typeof item === "string") {
            return item;
          }
          if (
            item &&
            typeof item === "object" &&
            "capability_name" in item &&
            typeof item.capability_name === "string"
          ) {
            return item.capability_name;
          }
          return undefined;
        })
        .filter((item): item is string => !!item)
    : [];
}

function agentProviderTierFromOptions(
  options: Record<string, unknown>,
): string | undefined {
  const provider = options.provider;
  if (!provider || typeof provider !== "object" || Array.isArray(provider)) {
    return undefined;
  }
  const tier = (provider as Record<string, unknown>).tura_llm_name;
  return typeof tier === "string" ? tier : undefined;
}

function agentCapabilitiesFromOptions(
  options: Record<string, unknown>,
): string[] {
  const capabilities = options.capabilities;
  return Array.isArray(capabilities)
    ? capabilities.filter((item): item is string => typeof item === "string")
    : [];
}

function clampNumber(
  value: number | null | undefined,
  min: number,
  max: number,
  fallback: number,
): number {
  return Math.min(
    max,
    Math.max(min, Number.isFinite(value) ? value! : fallback),
  );
}
