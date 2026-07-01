import {
  GatewayClient,
  type GatewayEventEnvelope,
  defaultGatewayUrl,
  errorMessage,
  type AgentUpsertRequest,
  type Message,
  type PlanStatus,
  type ProductIssue,
  type Project,
  type Session,
  type StartCondition,
  type StoredAgent,
} from "@tura/gateway-sdk";
import { createEffect, createMemo, createSignal } from "solid-js";
import {
  runtimeAgentFromUpsert,
  storedAgentFromRuntimeAgent,
  storedAgentFromUpsert,
} from "./app-agent-config";
import {
  mergeMessagePages,
  mergeSessions,
  providerIssueIdFromError,
  shouldFetchSessionMessages,
  writeLastSessionOpened,
} from "./app-state-utils";
import { AppShell } from "./app/app-shell";
import { AppProviders } from "./context/app-providers";
import { isToolPart } from "./conversation/message-tools";
import { DEFAULT_AGENT_ID } from "./config/defaults";
import {
  appendTaskToSession,
  defaultPollInterval,
  materializeComposerContent,
  sessionTasks,
} from "./features/plan/tasks";
import { useAppGatewayLifecycle } from "./hooks/use-app-gateway-lifecycle";
import { useFileBrowserActions } from "./hooks/use-file-browser-actions";
import { useMainTabNavigation } from "./hooks/use-main-tab-navigation";
import { usePlanActions } from "./hooks/use-plan-actions";
import { useProviderSettingsActions } from "./hooks/use-provider-settings-actions";
import { t } from "./i18n";
import { applyGatewayEvent } from "./state/event-reducer";
import { fixtureAppState } from "./test/fixtures/app-fixtures";
import {
  activeSession,
  initialAppState,
  sessionDirectory,
  withSessionFallbackName,
  type AppState,
} from "./state/global-store";
import {
  readBooleanSearchParam,
  readMainTabSearchParam,
  readSearchParam,
  normalizePath,
  samePath,
  shortWorkspaceLabel,
  withInitialOverrides,
} from "./utils/app-format";
import { safe } from "./utils/safe";

const PROMPT_RESPONSE_TIMEOUT_MS = 30_000;
const PROMPT_RESPONSE_TIMEOUT_CODE = "GATEWAY_NO_RESPONSE_30S";
const ASSISTANT_REPLY_POLL_TIMEOUT_MS = 120_000;
const ASSISTANT_REPLY_POLL_INTERVAL_MS = 1_250;
const MESSAGE_PAGE_SIZE = 200;

declare global {
  interface Window {
    __turaGuiE2E?: {
      applyGatewayEvent: (envelope: GatewayEventEnvelope) => void;
      snapshot: () => AppState;
    };
  }
}

export function App() {
  const e2eFixture = readSearchParam("e2eFixture");
  const requestedGatewayUrl = readSearchParam("gatewayUrl") ?? defaultGatewayUrl();
  const requestedTab = readSearchParam("tab");
  const disableGatewayAutostart = readSearchParam("e2eNoGatewayStart") === "1";
  const initialTab = readMainTabSearchParam();
  const forceNewSession = readBooleanSearchParam("newSession") || requestedTab === "new";
  const disablePermissionRestrictions = readBooleanSearchParam("disablePermissionRestrictions");
  const initialSessionId = forceNewSession ? null : readSearchParam("sessionId");
  const initialModel = readSearchParam("model");
  const initialAgent = readSearchParam("agent");
  const [state, setState] = createSignal<AppState>(
    withInitialOverrides(
      e2eFixture
        ? fixtureAppState(requestedGatewayUrl, e2eFixture)
        : initialAppState(requestedGatewayUrl),
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
  const rootClient = createMemo(() => new GatewayClient({ baseUrl: gatewayUrl() }));
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
  const [expandedWorkspaces, setExpandedWorkspaces] = createSignal<Set<string>>(new Set());
  const [expandedRailGroup, setExpandedRailGroup] = createSignal<string>();
  const [workspaceTreeTouched, setWorkspaceTreeTouched] = createSignal(false);
  const e2eStoredAgents = new Map<string, StoredAgent>();

  if (e2eFixture && typeof window !== "undefined") {
    window.__turaGuiE2E = {
      applyGatewayEvent: (event) => setState((previous) => applyGatewayEvent(previous, event)),
      snapshot: () => state(),
    };
  }

  const {
    fileTree,
    setFileTree,
    fileLoadingPath,
    fileContentLoadingPath,
    expandedFileTreePaths,
    loadFiles,
    openFile,
    toggleFileTreeDirectory,
    openCurrentDirectory,
    openSelectedFile,
  } = useFileBrowserActions({
    state,
    setState,
    directoryClient,
    e2eFixture,
  });

  createEffect(() => {
    const directory = state().directory;
    if (!workspaceTreeTouched() && directory) {
      expandWorkspace(directory);
    }
  });

  function expandWorkspace(worktree: string) {
    const key = normalizePath(worktree);
    setExpandedWorkspaces((previous) => {
      if (previous.has(key)) {
        return previous;
      }
      return new Set([...previous, key]);
    });
  }

  function toggleExpandedWorkspace(worktree: string): boolean {
    const key = normalizePath(worktree);
    let opened = false;
    setExpandedWorkspaces((previous) => {
      const next = new Set(previous);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
        opened = true;
      }
      return next;
    });
    return opened;
  }

  createEffect(() => {
    document.documentElement.dataset.theme = state().themeMode;
  });

  async function openSession(sessionId: string, options: { forceRefreshMessages?: boolean } = {}) {
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
    if (!shouldFetchSessionMessages(existingMessages, options.forceRefreshMessages)) {
      setState((previous) => ({
        ...previous,
        messagePagingBySession: {
          ...previous.messagePagingBySession,
          [sessionId]: previous.messagePagingBySession[sessionId] ?? {
            hasEarlier: true,
            loadingEarlier: false,
          },
        },
      }));
      return;
    }
    const [messages] = await Promise.all([
      safe(() => client.messages(sessionId, { limit: MESSAGE_PAGE_SIZE }), existingMessages),
    ]);
    setState((previous) => ({
      ...previous,
      messagesBySession: {
        ...previous.messagesBySession,
        [sessionId]: mergeMessagePages(previous.messagesBySession[sessionId] ?? [], messages),
      },
      messagePagingBySession: {
        ...previous.messagePagingBySession,
        [sessionId]: {
          hasEarlier: !e2eFixture && messages.length >= MESSAGE_PAGE_SIZE,
          loadingEarlier: false,
        },
      },
    }));
  }

  async function loadEarlierMessages(sessionId: string): Promise<boolean> {
    if (e2eFixture) {
      return false;
    }
    const paging = state().messagePagingBySession[sessionId];
    if (paging?.loadingEarlier || paging?.hasEarlier === false) {
      return false;
    }
    const currentMessages = state().messagesBySession[sessionId] ?? [];
    const before = currentMessages[0]?.id;
    if (!before) {
      return false;
    }
    setState((previous) => ({
      ...previous,
      messagePagingBySession: {
        ...previous.messagePagingBySession,
        [sessionId]: { hasEarlier: paging?.hasEarlier ?? true, loadingEarlier: true },
      },
    }));
    try {
      const earlier = await directoryClient().messages(sessionId, {
        limit: MESSAGE_PAGE_SIZE,
        before,
      });
      setState((previous) => {
        const existing = previous.messagesBySession[sessionId] ?? [];
        return {
          ...previous,
          messagesBySession: {
            ...previous.messagesBySession,
            [sessionId]: mergeMessagePages(earlier, existing),
          },
          messagePagingBySession: {
            ...previous.messagePagingBySession,
            [sessionId]: {
              hasEarlier: earlier.length >= MESSAGE_PAGE_SIZE,
              loadingEarlier: false,
            },
          },
        };
      });
      return earlier.length > 0;
    } catch (error) {
      setState((previous) => ({
        ...previous,
        error: errorMessage(error),
        messagePagingBySession: {
          ...previous.messagePagingBySession,
          [sessionId]: { hasEarlier: paging?.hasEarlier ?? true, loadingEarlier: false },
        },
      }));
      return false;
    }
  }

  function openBlankSession() {
    const currentSessionId = state().selectedSessionId;
    if (currentSessionId) {
      writeLastSessionOpened(currentSessionId);
    }
    setState((previous) => ({
      ...previous,
      lastSessionOpenedId: previous.selectedSessionId ?? previous.lastSessionOpenedId,
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

  async function useWorkspaceDirectory(directory: string) {
    const workspaceDirectory = directory.trim();
    if (!workspaceDirectory) {
      return;
    }
    const project: Project = {
      id: workspaceDirectory,
      name: shortWorkspaceLabel(workspaceDirectory),
      worktree: workspaceDirectory,
    };
    setState((previous) => ({
      ...previous,
      directory: workspaceDirectory,
      projects: previous.projects.some((project) => samePath(project.worktree, workspaceDirectory))
        ? previous.projects
        : [project, ...previous.projects],
      activeTab: "conversation",
      previousMainTab: "conversation",
      selectedSessionId: undefined,
      sessions: previous.sessions,
      sessionsLoading: true,
      composerText: "",
    }));
    expandWorkspace(workspaceDirectory);
    if (e2eFixture) {
      setState((previous) => ({ ...previous, sessionsLoading: false }));
      return;
    }
    try {
      const scoped = rootClient().withDirectory(workspaceDirectory);
      const [currentProject, sessions] = await Promise.all([
        safe(() => scoped.currentProject(), { project }),
        scoped.sessions({ limit: 100 }),
      ]);
      setState((previous) => ({
        ...previous,
        currentProject,
        sessions: mergeSessions(sessions, previous.sessions),
        sessionsLoading: false,
        error: undefined,
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        sessionsLoading: false,
        error: errorMessage(error),
      }));
    }
  }

  function activateWorkspaceProject(project: Project) {
    setState((previous) => ({
      ...previous,
      directory: project.worktree,
      projects: previous.projects.some((item) => samePath(item.worktree, project.worktree))
        ? previous.projects.map((item) =>
            samePath(item.worktree, project.worktree) ? project : item,
          )
        : [project, ...previous.projects],
      activeTab: "conversation",
      previousMainTab: "conversation",
      selectedSessionId: undefined,
      sessions: previous.sessions,
      composerText: "",
    }));
    expandWorkspace(project.worktree);
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
          sessions: [session, ...previous.sessions.filter((item) => item.id !== session.id)],
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

  async function saveAgent(agentId: string | undefined, payload: AgentUpsertRequest) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      if (e2eFixture) {
        const nextAgent = runtimeAgentFromUpsert(agentId, payload);
        e2eStoredAgents.set(nextAgent.name, storedAgentFromUpsert(nextAgent, payload));
        setState((previous) => ({
          ...previous,
          agents: [nextAgent, ...previous.agents.filter((agent) => agent.name !== nextAgent.name)],
          settingsSaving: false,
          settingsNotice: t("saved"),
        }));
        return;
      }
      await (agentId
        ? directoryClient().updateAgent(agentId, payload)
        : directoryClient().createAgent(payload));
      const agents = await safe(() => directoryClient().agents(), state().agents);
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
          selectedAgent: previous.selectedAgent === agentId ? undefined : previous.selectedAgent,
          settingsSaving: false,
          settingsNotice: t("saved"),
        }));
        return;
      }
      await directoryClient().deleteAgent(agentId);
      const agents = await safe(() => directoryClient().agents(), state().agents);
      setState((previous) => ({
        ...previous,
        agents,
        selectedAgent: previous.selectedAgent === agentId ? undefined : previous.selectedAgent,
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

  useAppGatewayLifecycle({
    state,
    setState,
    gatewayUrl,
    rootClient,
    forceNewSession,
    disableGatewayAutostart,
    e2eFixture,
    openSession,
  });

  const { openSettings, closeSettings, changeMainTab } = useMainTabNavigation({
    state,
    setState,
    refreshProviderSurface,
    openBlankSession,
    openSession,
    loadFiles,
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
      const existingSession = state().sessions.find((session) => session.id === sessionId);
      const busySession = existingSession?.status === "busy";
      const optimisticMessage: Message = {
        id: optimisticId,
        sessionID: sessionId,
        role: "user",
        created_at: now,
        updated_at: now,
        time: { created: now, updated: now },
        parts: [
          {
            id: `${optimisticId}:text`,
            sessionID: sessionId,
            messageID: optimisticId,
            type: "text",
            text: content,
            metadata: busySession ? { userNewCommand: true } : { planRunPending: true },
          },
        ],
      };
      setState((previous) => ({
        ...previous,
        selectedSessionId: sessionId,
        sessions: createdSession
          ? [
              { ...createdSession, status: busySession ? createdSession.status : "busy" },
              ...previous.sessions.filter((session) => session.id !== sessionId),
            ]
          : busySession
            ? previous.sessions
            : previous.sessions.map((session) =>
                session.id === sessionId ? { ...session, status: "busy" } : session,
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
          messageID: optimisticId,
          parts: [{ id: `${optimisticId}:text`, type: "text", text: content }],
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
      await refreshSessions();
      if (!busySession) {
        void pollSessionMessagesUntilAssistantReply(sessionId);
      }
    } catch (error) {
      const timeout = error instanceof Error && error.message === PROMPT_RESPONSE_TIMEOUT_CODE;
      setState((previous) => ({
        ...previous,
        messagesBySession:
          optimisticSessionId && optimisticId
            ? {
                ...previous.messagesBySession,
                [optimisticSessionId]: (previous.messagesBySession[optimisticSessionId] ?? []).map(
                  (message) =>
                    message.id === optimisticId
                      ? {
                          ...message,
                          updated_at: Date.now(),
                          time: { ...message.time, updated: Date.now() },
                          parts: message.parts.map((part) => ({
                            ...part,
                            metadata: {
                              ...(typeof part.metadata === "object" && part.metadata !== null
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

  async function pollSessionMessagesUntilAssistantReply(sessionId: string): Promise<void> {
    if (e2eFixture) {
      return;
    }
    const deadline = Date.now() + ASSISTANT_REPLY_POLL_TIMEOUT_MS;
    while (Date.now() < deadline) {
      await delay(ASSISTANT_REPLY_POLL_INTERVAL_MS);
      if (state().selectedSessionId !== sessionId) {
        return;
      }
      let messages: Message[];
      try {
        messages = await directoryClient().messages(sessionId);
      } catch {
        continue;
      }
      if (messages.length === 0) {
        continue;
      }
      setState((previous) => ({
        ...previous,
        messagesBySession: {
          ...previous.messagesBySession,
          [sessionId]: mergeMessagePages(previous.messagesBySession[sessionId] ?? [], messages),
        },
      }));
      if (hasVisibleAssistantReply(messages)) {
        await refreshSessions();
        return;
      }
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

  async function submitQueuedPrompt(content: string, forcedStartCondition?: StartCondition) {
    const startCondition = forcedStartCondition ?? state().planDraftStartCondition;
    const timingPatch =
      startCondition === "session_idle" ? { start_condition: "session_idle" as const } : {};
    const [summaryLine = "", ...deliverableLines] = content.split(/\r?\n/u);
    const title = summaryLine.trim() || t("newTask");
    const currentSession = state().selectedSessionId
      ? state().sessions.find((session) => session.id === state().selectedSessionId)
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
    const optimisticSession: Session = currentSession
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
        optimisticSession,
        ...previous.sessions.filter((item) => item.id !== optimisticSession.id),
      ],
      selectedSessionId: optimisticSession.id,
      planPreviewSessionId:
        previous.activeTab === "plan" ? optimisticSession.id : previous.planPreviewSessionId,
      composerText: "",
      composerImages: [],
      planDraftStartCondition: "user_action",
      planDraftStartAt: "",
      planDraftPollInterval: defaultPollInterval(),
      planNotice: undefined,
      error: undefined,
    }));
    if (e2eFixture) {
      return;
    }
    const session = await Promise.race([
      currentSession
        ? directoryClient().updateSessionTaskManagement(currentSession.id, {
            tasks: [taskState],
          })
        : directoryClient()
            .createSession({
              ...createSessionPayload(),
              task_management: taskState,
            })
            .then((created) => withSessionFallbackName(created, title)),
      new Promise<Session>((resolve) =>
        window.setTimeout(() => resolve(optimisticSession), PROMPT_RESPONSE_TIMEOUT_MS),
      ),
    ]);
    setState((previous) => ({
      ...previous,
      sessions: [
        session,
        ...previous.sessions.filter(
          (item) => item.id !== session.id && item.id !== optimisticSession.id,
        ),
      ],
      selectedSessionId: session.id,
      planPreviewSessionId:
        previous.activeTab === "plan" ? session.id : previous.planPreviewSessionId,
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
    return {
      directory: state().directory,
      model: state().selectedModel,
      agent: state().selectedAgent ?? DEFAULT_AGENT_ID,
      model_variant: state().modelVariant,
      model_acceleration_enabled: state().accelerationEnabled,
      disable_permission_restrictions: disablePermissionRestrictions,
      auto_session_name: true,
      task_management:
        state().planDraftStartCondition === "session_idle"
          ? { start_condition: "session_idle" as const }
          : {},
    };
  }

  async function refreshSessions() {
    setState((previous) => ({ ...previous, sessionsLoading: true }));
    try {
      const sessions = await directoryClient().sessions({ limit: 100 });
      setState((previous) => ({
        ...previous,
        sessions: mergeSessions(sessions, previous.sessions),
        sessionsLoading: false,
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        sessionsLoading: false,
        error: errorMessage(error),
      }));
    }
  }

  async function switchWorkspace(project: Project, options: { selectSession?: boolean } = {}) {
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
        sessionsLoading: false,
        error: undefined,
      }));
      setFileTree({});
      expandWorkspace(directory);
      return;
    }
    const scoped = rootClient().withDirectory(directory);
    setState((previous) => ({
      ...previous,
      directory,
      selectedSessionId: undefined,
      messagesBySession: {},
      messagePagingBySession: {},
      todosBySession: {},
      files: [],
      filePath: "",
      selectedFile: undefined,
      fileContent: undefined,
      loading: true,
      sessionsLoading: true,
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
        sessions: mergeSessions(sessions, previous.sessions),
        files,
        selectedSessionId,
        loading: false,
        sessionsLoading: false,
      }));
      setFileTree({ "": files });
      if (selectedSessionId) {
        await openSession(selectedSessionId);
      }
    } catch (error) {
      setState((previous) => ({
        ...previous,
        loading: false,
        sessionsLoading: false,
        error: errorMessage(error),
      }));
    }
  }

  async function toggleWorkspace(project: Project) {
    setWorkspaceTreeTouched(true);
    const opened = toggleExpandedWorkspace(project.worktree);
    if (state().activeTab === "files") {
      setExpandedRailGroup(undefined);
      if (!opened && samePath(project.worktree, state().directory)) {
        return;
      }
      if (opened && !samePath(project.worktree, state().directory)) {
        await switchWorkspace(project, { selectSession: false });
        return;
      }
      if (opened) {
        await loadFiles("");
      }
      return;
    }
    if (!opened) {
      return;
    }
    setExpandedRailGroup(undefined);
    if (!samePath(project.worktree, state().directory)) {
      await switchWorkspace(project);
    }
  }

  function toggleRailGroup(id: string) {
    setExpandedRailGroup((previous) => (previous === id ? undefined : id));
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
          loadEarlierMessages,
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
          expandedWorkspaces,
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

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function hasVisibleAssistantReply(messages: Message[]): boolean {
  return messages.some((message) => {
    if (message.role !== "assistant") {
      return false;
    }
    return message.parts.some((part) => {
      if (isToolPart(part)) {
        return false;
      }
      const text = (part.text || part.content || "").trim();
      return text.length > 0 && !isTransientAssistantText(text);
    });
  });
}

function isTransientAssistantText(text: string): boolean {
  const normalized = text.trim().toLowerCase();
  return (
    normalized === "正在思考" ||
    normalized === "thinking" ||
    (normalized.includes("正在思考") && normalized.startsWith("已运行"))
  );
}
