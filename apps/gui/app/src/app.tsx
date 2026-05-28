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
  GatewayError,
  connectGatewayEvents,
  defaultGatewayUrl,
  errorMessage,
  type Message,
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
  systemThemeMode,
  type AppState,
  type SettingsSection,
  type ThemeMode,
} from "./state/global-store";
import { classNames } from "./state/format";
import { t } from "./i18n";
import { WorkspaceTree } from "./components/sidebar";
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
  appendTaskToSession,
  defaultLocalStartAt,
  defaultPollInterval,
  firstRunnableTask,
  formatTicketTime,
  hasVisibleSessionTasks,
  localDateTimeToUtcIso,
  materializeComposerContent,
  normalizePollInterval,
  sessionAttentionKey,
  sessionTaskState,
  sessionTasks,
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
  parseModelRef,
  providerConfigured,
  providerIdFromAuthError,
  providerIdFromModel,
  recordToDraft,
} from "./utils/settings";
import {
  eventBelongsToState,
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

const PROMPT_RESPONSE_TIMEOUT_MS = 30_000;
const PROMPT_RESPONSE_TIMEOUT_CODE = "GATEWAY_NO_RESPONSE_30S";
const GATEWAY_CONNECT_TIMEOUT_MS = 5_000;
const LAST_CESSION_OPENED_STORAGE_KEY = "last cession oppend";
let lastCessionOpenedMemory: string | undefined;

function readLastCessionOpened(): string | undefined {
  let stored: string | undefined;
  if (typeof window === "undefined") {
    return lastCessionOpenedMemory;
  }
  try {
    stored =
      window.localStorage
        .getItem(LAST_CESSION_OPENED_STORAGE_KEY)
        ?.trim() || undefined;
  } catch {
    stored = undefined;
  }
  return stored ?? lastCessionOpenedMemory;
}

function writeLastCessionOpened(sessionId: string) {
  lastCessionOpenedMemory = sessionId;
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(LAST_CESSION_OPENED_STORAGE_KEY, sessionId);
  } catch {
    // Memory fallback keeps tab navigation deterministic when storage is blocked.
  }
}

function clearLastCessionOpened() {
  lastCessionOpenedMemory = undefined;
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.removeItem(LAST_CESSION_OPENED_STORAGE_KEY);
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
        modelConfig,
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
          : clampNumber(config.main_font_size, 12, 15, 13),
        codeFontSize: previous.bootstrapped
          ? previous.codeFontSize
          : clampNumber(config.code_font_size, 10, 15, 12),
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

  async function openSession(sessionId: string) {
    writeLastCessionOpened(sessionId);
    acknowledgeSessionAttention(sessionId);
    setState((previous) => ({
      ...previous,
      lastCessionOpenedId: sessionId,
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
    const currentSessionId = state().selectedSessionId;
    if (currentSessionId) {
      writeLastCessionOpened(currentSessionId);
    }
    setState((previous) => ({
      ...previous,
      lastCessionOpenedId:
        previous.selectedSessionId ?? previous.lastCessionOpenedId,
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
    updateModelTier,
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
    handleProviderAuthError,
  });

  async function submitPrompt() {
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
      if (state().planDraftStartCondition !== "user_action") {
        await submitQueuedPrompt(content);
        return;
      }
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
          previousMainTab: "conversation",
        }));
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
          variant: state().modelVariant,
          model_acceleration_enabled: state().accelerationEnabled,
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

  async function submitQueuedPrompt(content: string) {
    const startCondition = state().planDraftStartCondition;
    const startAt =
      startCondition === "scheduled_task" || startCondition === "polling_task"
        ? (localDateTimeToUtcIso(
            state().planDraftStartAt || defaultLocalStartAt(),
          ) ?? localDateTimeToUtcIso(defaultLocalStartAt()))
        : undefined;
    const [summaryLine = "", ...deliveryLines] = content.split(/\r?\n/u);
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
      nonce_id: nonceId,
      step: currentSession ? sessionTasks(currentSession).length : 0,
      status: "todo" as PlanStatus,
      plan_summary: title,
      task_summary: title,
      delivery: deliveryLines.join("\n").trim(),
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
      : await directoryClient().createSession({
          ...createSessionPayload(),
          task_management: taskState,
        });
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
    const lastCessionId =
      state().lastCessionOpenedId ?? readLastCessionOpened();
    const lastCession = lastCessionId
      ? state().sessions.find((session) => session.id === lastCessionId)
      : undefined;

    if (activeTab === "conversation") {
      if (state().activeTab === "conversation") {
        openBlankSession();
        return;
      }
      if (!lastCessionId || !lastCession) {
        if (lastCessionId) {
          clearLastCessionOpened();
        }
        setState((previous) => ({
          ...previous,
          lastCessionOpenedId: undefined,
        }));
        openBlankSession();
        return;
      }
      setState((previous) => ({
        ...previous,
        activeTab: "conversation",
        previousMainTab: "conversation",
        selectedSessionId: lastCessionId,
        error: undefined,
      }));
      await openSession(lastCessionId);
      return;
    }

    if (activeTab === "plan") {
      if (lastCessionId && !lastCession) {
        clearLastCessionOpened();
        setState((previous) => ({
          ...previous,
          lastCessionOpenedId: undefined,
        }));
      }
      setState((previous) => ({
        ...previous,
        activeTab: "plan",
        previousMainTab: "plan",
        selectedSessionId: lastCession?.id ?? previous.selectedSessionId,
        planPreviewSessionId: lastCession?.id ?? previous.planPreviewSessionId,
        error: undefined,
      }));
      if (lastCession) {
        await openSession(lastCession.id);
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
        runPlanTaskNow,
        openPlanSession,
        selectDraftSession,
        createPlanTicket,
        createSessionFromPlanTask,
        updatePlanTicketTask,
        updateEditingTaskFromComposer,
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
        updateModelTier,
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

function normalizeThemeMode(value: string | null | undefined): ThemeMode {
  return value === "light" ||
    value === "dark" ||
    value === "caral" ||
    value === "uruk" ||
    value === "liangzhu"
    ? value
    : systemThemeMode();
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
