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
  type JSX,
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
import Edit3 from "lucide-solid/icons/edit-3";
import FolderOpen from "lucide-solid/icons/folder-open";
import MoreHorizontal from "lucide-solid/icons/more-horizontal";
import Pin from "lucide-solid/icons/pin";
import Plus from "lucide-solid/icons/plus";
import Search from "lucide-solid/icons/search";
import Settings from "lucide-solid/icons/settings";
import Trash2 from "lucide-solid/icons/trash-2";
import {
  GatewayClient,
  connectGatewayEvents,
  defaultGatewayUrl,
  errorMessage,
  type Command,
  type FileInfo,
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
} from "./conversation/conversation-view";
import { applyGatewayEvent } from "./state/event-reducer";
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
} from "./state/global-store";
import { classNames, truncate } from "./state/format";
import { t, type TextKey } from "./i18n";

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
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
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
    setState((previous) => ({
      ...previous,
      directory,
      activeTab: "new",
      previousMainTab: "new",
      selectedSessionId: undefined,
      sessions: samePath(previous.directory, directory)
        ? previous.sessions
        : [],
      composerText: "",
    }));
    setExpandedWorkspace(directory);
  }

  function useDefaultWorkspace() {
    const home = state().paths?.home ?? "C:\\Users\\liuliu";
    useWorkspaceDirectory(`${home}\\tura workspace`);
  }

  function createNamedWorkspace(name: string) {
    const home = state().paths?.home ?? "C:\\Users\\liuliu";
    useWorkspaceDirectory(`${home}\\Documents\\${name.trim()}`);
  }

  async function openIssueConversation(issue: ProductIssue) {
    setState((previous) => ({
      ...previous,
      activeTab: "conversation",
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

  async function openPlanSession(session: Session) {
    acknowledgeSessionAttention(session.id);
    setState((previous) => ({
      ...previous,
      planPreviewSessionId: session.id,
      selectedSessionId: session.id,
      planDraftLane: undefined,
      planDraftSessionId: undefined,
      editingTask: undefined,
      error: undefined,
    }));
    await openSession(session.id);
  }

  async function selectDraftSession(planDraftSessionId: string | undefined) {
    setState((previous) => ({
      ...previous,
      planDraftSessionId,
      planPreviewSessionId: planDraftSessionId,
      selectedSessionId: planDraftSessionId ?? previous.selectedSessionId,
      editingTask: undefined,
      error: undefined,
    }));
    if (planDraftSessionId) {
      await openSession(planDraftSessionId);
    }
  }

  function acknowledgeSessionAttention(sessionId: string) {
    const session = state().sessions.find((item) => item.id === sessionId);
    const key = session ? sessionAttentionKey(session) : undefined;
    if (!key) {
      return;
    }
    setAcknowledgedAttentionSessions((previous) => {
      const next = new Set(previous);
      next.add(key);
      return next;
    });
  }

  function sessionAttentionAcknowledged(session: Session): boolean {
    const key = sessionAttentionKey(session);
    return key ? acknowledgedAttentionSessions().has(key) : false;
  }

  async function updatePlanTicketStatus(session: Session, status: PlanStatus) {
    const currentStatus = planSessionStatus(session);
    if (status === "question") {
      await openPlanSession(session);
      return;
    }
    if (status === "doing") {
      if (currentStatus !== "todo" || !firstRunnableTask(session)) {
        await openPlanSession(session);
        setState((previous) => ({
          ...previous,
          composerText: firstRunnableTask(session)
            ? taskDisplayText(firstRunnableTask(session)!)
            : sessionTitle(session),
        }));
        return;
      }
      await startPlanTicketDoing(session);
      return;
    }
    await updatePlanTicketTask(session, { status: status });
  }

  async function startPlanTicketDoing(session: Session) {
    const task = firstRunnableTask(session);
    if (!task) {
      await openPlanSession(session);
      setState((previous) => ({
        ...previous,
        composerText: sessionTitle(session),
      }));
      return;
    }
    await updatePlanTicketTask(session, { status: "doing" });
    if (e2eFixture) {
      return;
    }
    try {
      await directoryClient().promptAsync(session.id, {
        parts: [{ type: "text", text: taskDisplayText(task) }],
        model: state().selectedModel,
        variant: state().modelVariant,
        model_acceleration_enabled: state().accelerationEnabled,
      });
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  async function updatePlanTicketTask(
    session: Session,
    patch: Partial<
      TaskManagement & {
        status: PlanStatus;
        start_at: string;
        poll_interval: PollInterval;
      }
    >,
  ) {
    if (
      patch.status &&
      !["todo", "doing", "question", "done", "archived"].includes(
        patch.status,
      )
    ) {
      setState((previous) => ({
        ...previous,
        error: `Unsupported task status: ${patch.status}`,
      }));
      return;
    }
    setState((previous) => ({
      ...previous,
      sessions: previous.sessions.map((item) =>
        item.id === session.id ? applyTaskPatchToSession(item, patch) : item,
      ),
      error: undefined,
    }));
    if (e2eFixture) {
      return;
    }
    try {
      const updated = await directoryClient().updateSessionTaskManagement(
        session.id,
        patch,
      );
      setState((previous) => ({
        ...previous,
        sessions: previous.sessions.map((item) =>
          item.id === session.id ? { ...item, ...updated } : item,
        ),
      }));
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
      await refreshSessions();
    }
  }

  async function deletePlanTask(session: Session, task: TaskManagement) {
    await updatePlanTicketTask(session, {
      nonce_id: taskNonceId(task),
      status: "archived",
    });
  }

  async function createSessionFromPlanTask(
    session: Session,
    task: TaskManagement,
  ) {
    const title =
      taskDisplayText(task).split("\n")[0]?.trim() || sessionTitle(session);
    const patch = {
      ...task,
      nonce_id: `${session.id}:${Date.now()}`,
      status: "todo" as PlanStatus,
    };
    if (e2eFixture) {
      const next: Session = {
        ...session,
        id: `plan-task-session-${Date.now()}`,
        name: title,
        plan_summary: title,
        session_display_name: title,
        task_management: patch,
      };
      setState((previous) => ({
        ...previous,
        sessions: [next, ...previous.sessions],
        selectedSessionId: next.id,
        planPreviewSessionId: next.id,
        error: undefined,
      }));
      return;
    }
    const created = await directoryClient().createSession({
      ...createSessionPayload(),
      task_management: patch,
    });
    setState((previous) => ({
      ...previous,
      sessions: [created, ...previous.sessions],
      selectedSessionId: created.id,
      planPreviewSessionId: created.id,
      error: undefined,
    }));
  }

  async function updateEditingTaskFromComposer(): Promise<boolean> {
    const editing = state().editingTask;
    if (!editing) {
      return false;
    }
    const session = state().sessions.find(
      (item) => item.id === editing.sessionId,
    );
    if (!session) {
      return false;
    }
    const text = state().composerText.trim();
    if (!text) {
      return true;
    }
    const [summaryLine = "", ...deliveryLines] = text.split(/\r?\n/u);
    await updatePlanTicketTask(session, {
      nonce_id: editing.nonce_id,
      task_summary: summaryLine.trim(),
      delivery: deliveryLines.join("\n").trim(),
    });
    setState((previous) => ({
      ...previous,
      composerText: "",
      editingTask: undefined,
      error: undefined,
    }));
    return true;
  }

  async function createPlanTicket() {
    const title = state().composerText.trim();
    if (!title || !state().planDraftLane) {
      return;
    }
    const existingSession = state().planDraftSessionId
      ? state().sessions.find(
          (session) => session.id === state().planDraftSessionId,
        )
      : undefined;
    const startAt = localDateTimeToUtcIso(state().planDraftStartAt);
    const timingPatch = timedTaskPatch(
      state().planDraftStartCondition,
      startAt,
      state().planDraftPollInterval,
    );
    const taskState = {
      plan_summary: title,
      task_summary: title,
      ...(state().planDraftLane === "todo" || !state().planDraftLane
        ? {}
        : { status: state().planDraftLane }),
      ...timingPatch,
    };
    if (e2eFixture) {
      const session: Session = existingSession
        ? {
            ...existingSession,
            name: title,
            updated_at: Date.now(),
            plan_summary: title,
            session_display_name: title,
            task_management: {
              ...(existingSession.task_management ?? {}),
              ...taskState,
            },
          }
        : {
            id: `plan-local-${Date.now()}`,
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
        planPreviewSessionId: session.id,
        composerText: "",
        planDraftLane: undefined,
        planDraftSessionId: undefined,
        planDraftStartAt: "",
        planDraftStartCondition: "user_action",
        planDraftPollInterval: defaultPollInterval(),
        error: undefined,
      }));
      return;
    }
    try {
      let session: Session | undefined;
      if (existingSession) {
        session = await directoryClient().updateSession(existingSession.id, {
          task_management: taskState,
        } as Partial<Session>);
      } else {
        session = await directoryClient().createSession({
          ...createSessionPayload(),
          task_management: taskState,
        });
      }
      setState((previous) => ({
        ...previous,
        sessions: session
          ? [
              session,
              ...previous.sessions.filter((item) => item.id !== session!.id),
            ]
          : previous.sessions,
        selectedSessionId: session?.id ?? previous.selectedSessionId,
        planPreviewSessionId: session?.id ?? previous.planPreviewSessionId,
        composerText: "",
        planDraftLane: undefined,
        planDraftSessionId: undefined,
        planDraftStartAt: "",
        planDraftStartCondition: "user_action",
        planDraftPollInterval: defaultPollInterval(),
        error: undefined,
      }));
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

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
          previousMainTab: "conversation",
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
        previousMainTab: "conversation",
      }));
      await openSession(sessionId);
      await refreshSessions();
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
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

  async function loadFiles(path = "") {
    setFileLoadingPath(path);
    const files = e2eFixture
      ? fixtureFiles(e2eFixture, path)
      : await safe(() => directoryClient().files(path), []);
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
      await loadFiles(file.path);
      return;
    }
    const fileContent = await safe(
      () => directoryClient().fileContent(file.path),
      undefined,
    );
    setState((previous) => ({ ...previous, selectedFile: file, fileContent }));
  }

  async function openSelectedFile() {
    const file = state().selectedFile;
    if (!file) {
      return;
    }
    try {
      await directoryClient().openFile(file.path);
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  async function refreshProviderSurface(providerId?: string) {
    const client = rootClient();
    const [providers, providerAuthMethods] = await Promise.all([
      safe(() => directoryClient().providers(), state().providers),
      safe(() => client.providerAuthMethods(), state().providerAuthMethods),
    ]);
    const ids = providerId
      ? [providerId]
      : (providers?.all ?? state().providers?.all ?? []).map(
          (provider) => provider.id,
        );
    const statusEntries = await Promise.all(
      ids.map(async (id) => [
        id,
        await safe(() => client.providerAuthStatus(id), undefined),
      ]),
    );
    const providerAuthStatus = {
      ...state().providerAuthStatus,
      ...Object.fromEntries(
        statusEntries.filter(
          (entry): entry is [string, AppState["providerAuthStatus"][string]] =>
            !!entry[1],
        ),
      ),
    };
    setState((previous) => ({
      ...previous,
      providers,
      providerAuthMethods,
      providerAuthStatus,
    }));
  }

  async function saveRuntimeSettings() {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      modelValidation: undefined,
      error: undefined,
    }));
    try {
      const payload: Record<string, unknown> = {
        ...draftToRecord(state().workspaceConfigDraft),
        model: state().selectedModel,
        active_agent: state().selectedAgent,
        model_variant: state().modelVariant,
        model_acceleration_enabled: state().accelerationEnabled,
      };
      const configPayload = configDraftToPatch(
        state().configDraft,
        state().themeMode,
      );
      const [workspaceConfig, config] = await Promise.all([
        directoryClient().patchWorkspaceConfig(payload),
        rootClient().patchConfig(configPayload),
      ]);
      setState((previous) => ({
        ...previous,
        config,
        configDraft: configToDraft(config),
        workspaceConfig,
        workspaceConfigDraft: recordToDraft(workspaceConfig),
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

  async function validateSelectedModel() {
    const parsed = parseModelRef(state().selectedModel);
    if (!parsed) {
      return;
    }
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      modelValidation: undefined,
      error: undefined,
    }));
    try {
      const result = await rootClient().validateProviderModel({
        providerID: parsed.providerId,
        modelID: parsed.modelId,
      });
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        modelValidation: result.message,
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function saveProviderKey(
    providerId: string,
    method: ProviderAuthMethod,
  ) {
    const key = state().authDrafts[providerId]?.trim();
    if (!key) {
      return;
    }
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const ok = await rootClient().setProviderAuth(providerId, {
        type: method.type,
        key,
        metadata: { login: method.login },
      });
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: ok ? t("connected") : t("notConfigured"),
        authDrafts: { ...previous.authDrafts, [providerId]: "" },
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function startProviderLogin(providerId: string, methodIndex: number) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const result = await rootClient().providerOauthAuthorize(providerId, {
        method: methodIndex,
      });
      if (result.url) {
        window.open(result.url, "_blank", "noopener,noreferrer");
      }
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: result.instructions,
      }));
      if (result.method === "auto") {
        void completeProviderLogin(providerId, "", methodIndex);
      }
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function completeProviderLogin(
    providerId: string,
    code?: string,
    methodIndex = 0,
  ) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      error: undefined,
    }));
    try {
      const ok = await rootClient().providerOauthCallback(providerId, {
        method: methodIndex,
        code: code?.trim() || undefined,
      });
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: ok ? t("connected") : t("loginPending"),
        authCodeDrafts: { ...previous.authCodeDrafts, [providerId]: "" },
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function logoutProvider(providerId: string) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const result = await rootClient().providerAuthLogout(providerId);
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: result.message,
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
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
      previousMainTab: activeTab,
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
    <main
      class={classNames(
        "workbench",
        state().activeTab === "settings" && "settings-workbench",
      )}
    >
      <aside
        class={classNames(
          "rail",
          state().activeTab === "settings" && "settings-mode",
        )}
      >
        <Show
          when={state().activeTab === "settings"}
          fallback={
            <>
              <div class="brand">
                <div class="brand-mark" />
                <div>
                  <strong>Tura</strong>
                </div>
              </div>
              <MainTabs
                active={state().previousMainTab}
                onChange={(activeTab) => void changeMainTab(activeTab)}
              />
              <WorkspaceTree
                activeTab={state().activeTab}
                projects={state().projects}
                directory={state().directory}
                sessions={state().sessions}
                selectedSessionId={state().selectedSessionId}
                productIssues={state().productIssues}
                filePath={state().filePath}
                files={state().files}
                fileTree={fileTree()}
                fileLoadingPath={fileLoadingPath()}
                selectedFile={state().selectedFile}
                expandedWorkspace={expandedWorkspace()}
                expandedGroup={expandedRailGroup()}
                attentionAcknowledged={sessionAttentionAcknowledged}
                onWorkspace={toggleWorkspace}
                onBlankSession={openBlankSession}
                onGroup={toggleRailGroup}
                onIssue={openIssueConversation}
                onStatus={updatePlanTicketStatus}
                onSession={(sessionId) => {
                  const session = state().sessions.find(
                    (item) => item.id === sessionId,
                  );
                  if (state().activeTab === "plan" && session) {
                    void openPlanSession(session);
                    return;
                  }
                  void openSession(sessionId);
                }}
                onRenameSession={renameSession}
                onFile={openFile}
                onUp={() => loadFiles(parentPath(state().filePath))}
                onSettings={() => openSettings("workspace")}
              />
              <button
                class="settings-entry"
                type="button"
                onClick={() => openSettings("general")}
              >
                {t("settings")}
              </button>
            </>
          }
        >
          <SettingsRail
            active={state().settingsSection}
            onBack={closeSettings}
            onSection={(settingsSection) =>
              setState((previous) => ({ ...previous, settingsSection }))
            }
          />
        </Show>
      </aside>

      <section class="main-column">
        <Show when={state().error}>
          {(error) => (
            <div class="error-strip">
              <span>{error()}</span>
              <button
                onClick={() =>
                  setState((previous) => ({ ...previous, error: undefined }))
                }
              >
                ×
              </button>
            </div>
          )}
        </Show>
        <Switch>
          <Match when={state().activeTab === "new"}>
            <NewSessionView
              state={state()}
              slashCommands={slashCommands()}
              onWorkspace={useWorkspaceDirectory}
              onDefaultWorkspace={useDefaultWorkspace}
              onCreateWorkspace={createNamedWorkspace}
              onComposerText={(composerText) =>
                setState((previous) => ({ ...previous, composerText }))
              }
              onComposerImages={(composerImages) =>
                setState((previous) => ({ ...previous, composerImages }))
              }
              onDraftStartCondition={(planDraftStartCondition) =>
                setState((previous) => ({
                  ...previous,
                  planDraftStartCondition,
                }))
              }
              onDraftStartAt={(planDraftStartAt) =>
                setState((previous) => ({ ...previous, planDraftStartAt }))
              }
              onDraftPollInterval={(planDraftPollInterval) =>
                setState((previous) => ({
                  ...previous,
                  planDraftPollInterval,
                }))
              }
              onSubmit={submitPrompt}
            />
          </Match>
          <Match when={state().activeTab === "plan"}>
            <PlanView
              state={state()}
              previewSession={state().sessions.find(
                (session) => session.id === state().planPreviewSessionId,
              )}
              previewMessages={
                state().planPreviewSessionId
                  ? (state().messagesBySession[state().planPreviewSessionId!] ??
                    [])
                  : []
              }
              slashCommands={slashCommands()}
              onPlanMode={(planMode) =>
                setState((previous) => ({ ...previous, planMode }))
              }
              onClosePanel={() =>
                setState((previous) => ({
                  ...previous,
                  planPreviewSessionId: undefined,
                  planDraftLane: undefined,
                  planDraftSessionId: undefined,
                  editingTask: undefined,
                }))
              }
              onSearch={(issueSearch) =>
                setState((previous) => ({ ...previous, issueSearch }))
              }
              onDraftLane={(planDraftLane) =>
                setState((previous) => ({
                  ...previous,
                  planDraftLane,
                  editingTask: undefined,
                }))
              }
              onDraftStartCondition={(planDraftStartCondition) =>
                setState((previous) => ({
                  ...previous,
                  planDraftStartCondition,
                }))
              }
              onDraftStartAt={(planDraftStartAt) =>
                setState((previous) => ({ ...previous, planDraftStartAt }))
              }
              onDraftPollInterval={(planDraftPollInterval) =>
                setState((previous) => ({
                  ...previous,
                  planDraftPollInterval,
                }))
              }
              onDraftSession={(planDraftSessionId) =>
                void selectDraftSession(planDraftSessionId)
              }
              onCreateTicket={createPlanTicket}
              onStatus={updatePlanTicketStatus}
              attentionAcknowledged={sessionAttentionAcknowledged}
              onTask={updatePlanTicketTask}
              onEditTask={(session, task, composerText) =>
                setState((previous) => ({
                  ...previous,
                  composerText,
                  editingTask: {
                    sessionId: session.id,
                    nonce_id: taskNonceId(task),
                  },
                }))
              }
              onDeleteTask={deletePlanTask}
              onCreateSessionFromTask={createSessionFromPlanTask}
              onOpenSession={openPlanSession}
              onComposerText={(composerText) =>
                setState((previous) => ({ ...previous, composerText }))
              }
              onComposerImages={(composerImages) =>
                setState((previous) => ({ ...previous, composerImages }))
              }
              onSubmit={submitPrompt}
              onOpenFullConversation={() =>
                setState((previous) => ({
                  ...previous,
                  activeTab: "conversation",
                  previousMainTab: "conversation",
                }))
              }
            />
          </Match>
          <Match when={state().activeTab === "files"}>
            <FileBrowserView
              path={state().filePath}
              directory={state().directory}
              files={state().files}
              selectedFile={state().selectedFile}
              fileContent={state().fileContent}
              onFile={openFile}
              onUp={() => loadFiles(parentPath(state().filePath))}
              onList={() =>
                setState((previous) => ({
                  ...previous,
                  selectedFile: undefined,
                  fileContent: undefined,
                }))
              }
              onOpenExternal={openSelectedFile}
            />
          </Match>
          <Match when={state().activeTab === "conversation"}>
            <ConversationView
              state={state()}
              session={selectedSession()}
              messages={selectedMessages()}
              slashCommands={slashCommands()}
              onComposerText={(composerText) =>
                setState((previous) => ({ ...previous, composerText }))
              }
              onComposerImages={(composerImages) =>
                setState((previous) => ({ ...previous, composerImages }))
              }
              onSubmit={submitPrompt}
              composerToolbar={
                selectedSession() ? (
                  <PlanComposerControls
                    startCondition={
                      taskStartCondition(sessionTaskState(selectedSession()!))
                    }
                    startAt={utcIsoToLocalDateTime(
                      sessionTaskState(selectedSession()!).start_at,
                    )}
                    pollInterval={
                      sessionTaskState(selectedSession()!).poll_interval ??
                      defaultPollInterval()
                    }
                    onStartCondition={(start_condition) =>
                      updatePlanTicketTask(selectedSession()!, {
                        status: "todo",
                      })
                    }
                    onStartAt={(value) => {
                      const start_at = localDateTimeToUtcIso(value);
                      if (start_at) {
                        void updatePlanTicketTask(selectedSession()!, {
                          start_at,
                        });
                      }
                    }}
                    onPollInterval={(poll_interval) =>
                      updatePlanTicketTask(selectedSession()!, {
                        poll_interval,
                      })
                    }
                  />
                ) : undefined
              }
              composerTaskList={
                selectedSession() &&
                hasVisibleSessionTasks(selectedSession()!) ? (
                  <PlanComposerTaskList
                    session={selectedSession()!}
                    selected_nonce_id={state().editingTask?.nonce_id}
                    onEdit={(task, composerText) =>
                      setState((previous) => ({
                        ...previous,
                        composerText,
                        editingTask: {
                          sessionId: selectedSession()!.id,
                          nonce_id: taskNonceId(task),
                        },
                      }))
                    }
                    onDelete={(task) =>
                      deletePlanTask(selectedSession()!, task)
                    }
                    onCreateSession={(task) =>
                      createSessionFromPlanTask(selectedSession()!, task)
                    }
                  />
                ) : undefined
              }
            />
          </Match>
          <Match when={state().activeTab === "settings"}>
            <SettingsView
              state={state()}
              section={state().settingsSection}
              onProvider={(providerId) =>
                setState((previous) => ({
                  ...previous,
                  selectedProviderId: providerId,
                }))
              }
              onModel={(selectedModel) =>
                setState((previous) => ({
                  ...previous,
                  selectedModel,
                  selectedProviderId:
                    providerIdFromModel(selectedModel) ??
                    previous.selectedProviderId,
                }))
              }
              onAgent={(selectedAgent) =>
                setState((previous) => ({ ...previous, selectedAgent }))
              }
              onVariant={(modelVariant) =>
                setState((previous) => ({ ...previous, modelVariant }))
              }
              onAcceleration={(accelerationEnabled) =>
                setState((previous) => ({
                  ...previous,
                  accelerationEnabled,
                }))
              }
              onTheme={(themeMode) =>
                setState((previous) => ({
                  ...previous,
                  themeMode,
                  configDraft: {
                    ...previous.configDraft,
                    theme: themeMode,
                  },
                }))
              }
              onConfigDraft={(key, value) =>
                setState((previous) => ({
                  ...previous,
                  configDraft: {
                    ...previous.configDraft,
                    [key]: value,
                  },
                }))
              }
              onWorkspaceConfigDraft={(key, value) =>
                setState((previous) => ({
                  ...previous,
                  workspaceConfigDraft: {
                    ...previous.workspaceConfigDraft,
                    [key]: value,
                  },
                }))
              }
              onAuthDraft={(providerId, value) =>
                setState((previous) => ({
                  ...previous,
                  authDrafts: {
                    ...previous.authDrafts,
                    [providerId]: value,
                  },
                }))
              }
              onAuthCode={(providerId, value) =>
                setState((previous) => ({
                  ...previous,
                  authCodeDrafts: {
                    ...previous.authCodeDrafts,
                    [providerId]: value,
                  },
                }))
              }
              onSaveSettings={saveRuntimeSettings}
              onValidateModel={validateSelectedModel}
              onSaveKey={saveProviderKey}
              onStartLogin={startProviderLogin}
              onCompleteLogin={completeProviderLogin}
              onLogout={logoutProvider}
            />
          </Match>
        </Switch>
      </section>
    </main>
  );
}

function WorkspaceTree(props: {
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
      <RailSectionTitle
        expanded={workspaceSectionOpen()}
        onToggle={() => setWorkspaceSectionOpen((open) => !open)}
      >
        {t("workspace")}
      </RailSectionTitle>
      <Show when={workspaceSectionOpen()}>
        <For
          each={projects()}
          fallback={<div class="rail-empty">{t("noWorkspace")}</div>}
        >
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
                  selectedFile={props.selectedFile}
                  onIssue={props.onIssue}
                  onGroup={props.onGroup}
                  onStatus={props.onStatus}
                  onSession={openRailSession}
                  onRenameSession={props.onRenameSession}
                  onFile={props.onFile}
                  onUp={props.onUp}
                />
              </Show>
            </div>
          )}
        </For>
      </Show>
      <Show when={archivedWorkspaces().length > 0}>
        <RailSectionTitle
          className="archived-section-title"
          expanded={archivedSectionOpen()}
          onToggle={() => setArchivedSectionOpen((open) => !open)}
        >
          {t("archived")}会话
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

function RailSectionTitle(props: {
  className?: string;
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
      <span>{props.children}</span>
      <RailDisclosure expanded={props.expanded} />
    </button>
  );
}

function WorkspaceChildren(props: {
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
  selectedFile?: FileInfo;
  onIssue: (issue: ProductIssue) => void;
  onGroup: (id: string) => void;
  onStatus: (session: Session, status: PlanStatus) => void;
  onSession: (session: Session) => void;
  onRenameSession: (sessionId: string, title: string) => void;
  onFile: (file: FileInfo) => void;
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
  const statuses: Array<{ id: PlanStatus; label: string }> = [
    { id: "todo", label: t("todo") },
    { id: "doing", label: t("doing") },
    { id: "question", label: t("question") },
    { id: "done", label: t("done") },
  ];
  function dropStatus(event: DragEvent, status: PlanStatus) {
    event.preventDefault();
    const session = props.sessions.find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    if (session) {
      props.onStatus(session, status);
    }
  }
  return (
    <div class="workspace-children">
      <Switch>
        <Match when={props.activeTab === "plan"}>
          <For each={statuses}>
            {(status) => {
              const sessions = createMemo(() =>
                props.sessions.filter(
                  (session) => planSessionStatus(session) === status.id,
                ),
              );
              return (
                <div class="tree-group">
                  <button
                    class="child-row tree-toggle"
                    style={{ "--depth": 1 }}
                    onDragOver={(event) => event.preventDefault()}
                    onDrop={(event) => dropStatus(event, status.id)}
                    onClick={() => props.onGroup(`plan:${status.id}`)}
                  >
                    <span class="tree-row-label">
                      <RailDisclosure
                        expanded={props.expandedGroup === `plan:${status.id}`}
                      />
                      {status.label}
                    </span>
                  </button>
                  <Show when={props.expandedGroup === `plan:${status.id}`}>
                    <For each={sessions()}>
                      {(session) => (
                        <button
                          class="child-row"
                          style={{ "--depth": 2 }}
                          onClick={() => props.onSession(session)}
                          title={sessionHoverTitle(session)}
                        >
                          <span class="tree-row-label">
                            {truncate(sessionTitle(session), 26)}
                          </span>
                          <Show
                            when={shouldShowSessionAttention(
                              session,
                              props.attentionAcknowledged(session),
                            )}
                          >
                            <PlanStatusIndicator
                              status={planSessionStatus(session)}
                            />
                          </Show>
                        </button>
                      )}
                    </For>
                  </Show>
                </div>
              );
            }}
          </For>
        </Match>
        <Match when={props.activeTab === "conversation"}>
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
            selectedFile={props.selectedFile}
            depth={1}
            onFile={props.onFile}
          />
        </Match>
      </Switch>
    </div>
  );
}

function FileTreeRows(props: {
  files: FileInfo[];
  fileTree: Record<string, FileInfo[]>;
  activePath: string;
  loadingPath?: string;
  selectedFile?: FileInfo;
  depth: number;
  onFile: (file: FileInfo) => void;
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
          () =>
            file.type === "directory" &&
            (props.activePath === file.path ||
              props.activePath.startsWith(`${file.path}\\`) ||
              props.activePath.startsWith(`${file.path}/`) ||
              loadedChildren().length > 0),
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
              onClick={() => props.onFile(file)}
              title={file.path}
            >
              <FileTreeLabel file={file} expanded={expanded()} />
              <Show when={props.loadingPath === file.path}>
                <span class="file-tree-loading" />
              </Show>
            </button>
            <Show when={expanded()}>
              <FileTreeRows
                files={loadedChildren()}
                fileTree={props.fileTree}
                activePath={props.activePath}
                loadingPath={props.loadingPath}
                selectedFile={props.selectedFile}
                depth={props.depth + 1}
                onFile={props.onFile}
              />
            </Show>
          </>
        );
      }}
    </For>
  );
}

function WorkspaceMenu(props: {
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

function ArchiveIcon() {
  return <span class="tiny-icon">▣</span>;
}

function RailDisclosure(props: { expanded: boolean }) {
  return (
    <span
      class={classNames("rail-disclosure", props.expanded && "expanded")}
      aria-hidden="true"
    >
      <ChevronRight size={13} strokeWidth={1.8} />
    </span>
  );
}

function NewSessionView(props: {
  state: AppState;
  slashCommands: Command[];
  onWorkspace: (directory: string) => void;
  onDefaultWorkspace: () => void;
  onCreateWorkspace: (name: string) => void;
  onComposerText: (value: string) => void;
  onComposerImages: (images: ComposerImage[]) => void;
  onDraftStartCondition: (value: StartCondition) => void;
  onDraftStartAt: (value: string) => void;
  onDraftPollInterval: (value: PollInterval) => void;
  onSubmit: () => void;
}) {
  const [naming, setNaming] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const projects = createMemo(() => {
    const fallback = props.state.directory
      ? [
          {
            id: props.state.directory,
            name: shortWorkspaceLabel(props.state.directory),
            worktree: props.state.directory,
          } as Project,
        ]
      : [];
    const normalizedQuery = query().trim().toLowerCase();
    return (props.state.projects.length ? props.state.projects : fallback)
      .filter((project) => {
        if (!normalizedQuery) {
          return true;
        }
        return `${project.name} ${project.worktree}`
          .toLowerCase()
          .includes(normalizedQuery);
      })
      .slice(0, 10);
  });

  return (
    <section class="new-session-view">
      <div class="new-session-center">
        <h1>{t("todayQuestion")}</h1>
        <Composer
          text={props.state.composerText}
          images={props.state.composerImages}
          submitting={props.state.submitting}
          slashCommands={props.slashCommands}
          onText={props.onComposerText}
          onImages={props.onComposerImages}
          onSubmit={props.onSubmit}
          toolbar={
            <>
              <NewSessionWorkspacePicker
                projects={projects()}
                directory={props.state.directory}
                query={query()}
                onQuery={setQuery}
                onWorkspace={props.onWorkspace}
                onCreateWorkspace={() => setNaming(true)}
                onDefaultWorkspace={props.onDefaultWorkspace}
              />
              <PlanComposerControls
                startCondition={props.state.planDraftStartCondition}
                startAt={props.state.planDraftStartAt}
                pollInterval={props.state.planDraftPollInterval}
                onStartCondition={props.onDraftStartCondition}
                onStartAt={props.onDraftStartAt}
                onPollInterval={props.onDraftPollInterval}
              />
            </>
          }
        />
      </div>
      <Show when={naming()}>
        <NameDialog
          title={t("createWorkspace")}
          description={t("renameSessionHint")}
          initialValue="New project"
          onCancel={() => setNaming(false)}
          onSave={(value) => {
            props.onCreateWorkspace(value);
            setNaming(false);
          }}
        />
      </Show>
    </section>
  );
}

function NewSessionWorkspacePicker(props: {
  projects: Project[];
  directory?: string;
  query: string;
  onQuery: (value: string) => void;
  onWorkspace: (directory: string) => void;
  onCreateWorkspace: () => void;
  onDefaultWorkspace: () => void;
}) {
  let root: HTMLElement | undefined;
  let directoryInput: HTMLInputElement | undefined;
  const [open, setOpen] = createSignal(false);
  const selectedProject = createMemo(() =>
    props.projects.find((project) =>
      samePath(project.worktree, props.directory),
    ),
  );

  async function pickDirectory() {
    const picker = (
      window as unknown as {
        showDirectoryPicker?: () => Promise<{ name?: string }>;
      }
    ).showDirectoryPicker;
    if (picker) {
      try {
        const handle = await picker();
        if (handle.name) {
          props.onWorkspace(handle.name);
          setOpen(false);
        }
        return;
      } catch {
        return;
      }
    }
    directoryInput?.click();
  }

  createEffect(() => {
    if (!open()) {
      return;
    }
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });

  return (
    <section class="plan-session-picker" ref={root}>
      <button
        type="button"
        class="plan-session-button"
        onClick={() => setOpen(!open())}
        title={selectedProject()?.worktree ?? t("chooseWorkspace")}
      >
        <FolderOpen size={15} strokeWidth={1.8} />
        <span>
          {selectedProject()?.name ??
            (props.directory
              ? shortWorkspaceLabel(props.directory)
              : t("chooseWorkspace"))}
        </span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-session-menu">
          <label class="workspace-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={props.query}
              placeholder={`${t("workspaceSearch")}...`}
              onInput={(event) => props.onQuery(event.currentTarget.value)}
            />
          </label>
          <div class="workspace-picker-list plan-session-list">
            <For each={props.projects}>
              {(project) => (
                <button
                  type="button"
                  class={classNames(
                    "workspace-pick-row",
                    samePath(project.worktree, props.directory) && "selected",
                  )}
                  onClick={() => {
                    props.onWorkspace(project.worktree);
                    setOpen(false);
                  }}
                  title={project.worktree}
                >
                  <FolderOpen size={15} strokeWidth={1.6} />
                  <span>
                    {project.name || shortWorkspaceLabel(project.worktree)}
                  </span>
                  <Show when={samePath(project.worktree, props.directory)}>
                    <Check size={14} strokeWidth={1.8} />
                  </Show>
                </button>
              )}
            </For>
          </div>
          <div class="workspace-picker-actions">
            <button type="button" onClick={props.onCreateWorkspace}>
              <span>{t("createWorkspace")}</span>
            </button>
            <button type="button" onClick={pickDirectory}>
              <span>{t("existingDirectory")}</span>
            </button>
            <button
              type="button"
              onClick={() => {
                props.onDefaultWorkspace();
                setOpen(false);
              }}
            >
              <span>{t("defaultWorkspace")}</span>
            </button>
          </div>
          <input
            class="workspace-directory-input"
            type="file"
            ref={(element) => {
              directoryInput = element;
              element.setAttribute("webkitdirectory", "");
            }}
            onChange={(event) => {
              const file = event.currentTarget.files?.[0] as
                | (File & { webkitRelativePath?: string })
                | undefined;
              const root = file?.webkitRelativePath?.split(/[\\/]/u)[0];
              if (root) {
                props.onWorkspace(root);
                setOpen(false);
              }
            }}
          />
        </div>
      </Show>
    </section>
  );
}

function NameDialog(props: {
  title: string;
  description: string;
  initialValue: string;
  onCancel: () => void;
  onSave: (value: string) => void;
}) {
  const [value, setValue] = createSignal(props.initialValue);
  return (
    <div class="modal-scrim" onMouseDown={props.onCancel}>
      <div class="name-dialog" onMouseDown={(event) => event.stopPropagation()}>
        <header>
          <div>
            <h2>{props.title}</h2>
            <p>{props.description}</p>
          </div>
          <button type="button" onClick={props.onCancel}>
            ×
          </button>
        </header>
        <input
          value={value()}
          autofocus
          onInput={(event) => setValue(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              props.onSave(value());
            }
            if (event.key === "Escape") {
              props.onCancel();
            }
          }}
        />
        <footer>
          <button type="button" class="secondary" onClick={props.onCancel}>
            {t("cancel")}
          </button>
          <button
            type="button"
            class="primary"
            disabled={!value().trim()}
            onClick={() => props.onSave(value())}
          >
            {t("save")}
          </button>
        </footer>
      </div>
    </div>
  );
}

function FileTreeLabel(props: { file: FileInfo; expanded?: boolean }) {
  return (
    <Show
      when={props.file.type === "directory"}
      fallback={<span>{props.file.name}</span>}
    >
      <span>
        <RailDisclosure expanded={Boolean(props.expanded)} />
        {`${props.file.name}/`}
      </span>
    </Show>
  );
}

function MainTabs(props: {
  active: Exclude<MainTab, "settings">;
  onChange: (tab: Exclude<MainTab, "settings">) => void;
}) {
  const tabs: Array<{ id: Exclude<MainTab, "settings">; label: string }> = [
    { id: "new", label: t("newSession") },
    { id: "conversation", label: t("sessionHistory") },
    { id: "plan", label: t("plan") },
    { id: "files", label: t("explorer") },
  ];
  return (
    <nav class="main-tabs">
      <For each={tabs}>
        {(item) => (
          <button
            class={classNames(props.active === item.id && "selected")}
            onClick={() => props.onChange(item.id)}
          >
            {item.label}
          </button>
        )}
      </For>
    </nav>
  );
}

function SettingsRail(props: {
  active: SettingsSection;
  onBack: () => void;
  onSection: (section: SettingsSection) => void;
}) {
  return (
    <nav class="settings-rail">
      <button class="settings-back" type="button" onClick={props.onBack}>
        <ArrowLeft size={15} strokeWidth={1.8} aria-hidden="true" />
        {t("backToApp")}
      </button>
      <div class="section-title">{t("settings")}</div>
      <div class="settings-section-list">
        <For each={settingsSections()}>
          {(item) => (
            <button
              class={classNames(props.active === item.id && "selected")}
              type="button"
              onClick={() => props.onSection(item.id)}
            >
              {item.label}
            </button>
          )}
        </For>
      </div>
    </nav>
  );
}

function SettingsView(props: {
  state: AppState;
  section: SettingsSection;
  onProvider: (providerId: string) => void;
  onModel: (model: string) => void;
  onAgent: (agent?: string) => void;
  onVariant: (variant: string) => void;
  onAcceleration: (enabled: boolean) => void;
  onTheme: (theme: ThemeMode) => void;
  onConfigDraft: (key: string, value: string) => void;
  onWorkspaceConfigDraft: (key: string, value: string) => void;
  onAuthDraft: (providerId: string, value: string) => void;
  onAuthCode: (providerId: string, value: string) => void;
  onSaveSettings: () => void;
  onValidateModel: () => void;
  onSaveKey: (providerId: string, method: ProviderAuthMethod) => void;
  onStartLogin: (providerId: string, methodIndex: number) => void;
  onCompleteLogin: (
    providerId: string,
    code?: string,
    methodIndex?: number,
  ) => void;
  onLogout: (providerId: string) => void;
}) {
  const providers = createMemo(() => props.state.providers?.all ?? []);
  const selectedProvider = createMemo(
    () =>
      providers().find(
        (provider) => provider.id === props.state.selectedProviderId,
      ) ?? providers()[0],
  );
  const selectedProviderStatus = createMemo(() => {
    const provider = selectedProvider();
    return provider ? props.state.providerAuthStatus[provider.id] : undefined;
  });
  const selectedMethods = createMemo(() => {
    const provider = selectedProvider();
    return provider ? (props.state.providerAuthMethods[provider.id] ?? []) : [];
  });
  const selectedModels = createMemo(() =>
    Object.values(selectedProvider()?.models ?? {}).sort((left, right) =>
      left.name.localeCompare(right.name),
    ),
  );
  const title = createMemo(
    () =>
      settingsSections().find((item) => item.id === props.section)?.label ??
      t("settings"),
  );
  const configRows = createMemo(() => configFieldRows(props.state));
  const workspaceRows = createMemo(() =>
    Object.entries(props.state.workspaceConfigDraft).sort(([left], [right]) =>
      left.localeCompare(right),
    ),
  );

  function chooseProvider(provider: SdkProvider) {
    props.onProvider(provider.id);
    if (providerIdFromModel(props.state.selectedModel) !== provider.id) {
      const modelId =
        props.state.providers?.default[provider.id] ??
        Object.keys(provider.models)[0];
      if (modelId) {
        props.onModel(modelRef(provider.id, modelId));
      }
    }
  }

  return (
    <section class="settings-view">
      <header class="page-head">
        <div class="page-title">
          <span>{t("settings")}</span>
          <h1>{title()}</h1>
        </div>
        <div class="page-actions">
          <Show when={props.section === "models"}>
            <button
              class="secondary"
              disabled={
                props.state.settingsSaving || !props.state.selectedModel
              }
              onClick={props.onValidateModel}
            >
              {t("validate")}
            </button>
          </Show>
          <button
            class="primary"
            disabled={props.state.settingsSaving}
            onClick={props.onSaveSettings}
          >
            {t("save")}
          </button>
        </div>
      </header>

      <main class="settings-canvas">
        <section class="settings-stack">
          <Switch>
            <Match when={props.section === "general"}>
              <section class="settings-panel">
                <header>
                  <span>{t("overview")}</span>
                  <small>{props.state.connection}</small>
                </header>
                <div class="provider-detail">
                  <div class="provider-metrics">
                    <MetricCell
                      label={t("workspace")}
                      value={shortWorkspaceLabel(props.state.directory)}
                    />
                    <MetricCell
                      label={t("gateway")}
                      value={props.state.gatewayUrl}
                    />
                    <MetricCell
                      label={t("version")}
                      value={props.state.health?.version ?? "--"}
                    />
                    <MetricCell
                      label={t("user")}
                      value={props.state.me?.email ?? "--"}
                    />
                  </div>
                </div>
              </section>
              <section class="settings-panel">
                <header>
                  <span>{t("currentRuntime")}</span>
                  <small>{props.state.selectedModel ?? "--"}</small>
                </header>
                <div class="settings-fields">
                  <ReadonlyRow
                    label={t("provider")}
                    value={selectedProvider()?.name ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("model")}
                    value={props.state.selectedModel ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("agent")}
                    value={props.state.selectedAgent ?? "--"}
                  />
                </div>
              </section>
            </Match>

            <Match when={props.section === "appearance"}>
              <section class="settings-panel">
                <header>
                  <span>{t("theme")}</span>
                  <small>{props.state.themeMode}</small>
                </header>
                <div class="settings-fields">
                  <div class="field-row">
                    <span>{t("mode")}</span>
                    <div class="segmented two">
                      <For each={["light", "dark"] as ThemeMode[]}>
                        {(mode) => (
                          <button
                            class={classNames(
                              props.state.themeMode === mode && "selected",
                            )}
                            onClick={() => props.onTheme(mode)}
                          >
                            {mode === "dark" ? t("dark") : t("light")}
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                  <ReadonlyRow
                    label={t("surface")}
                    value="paper / line / ink"
                  />
                  <ReadonlyRow label={t("radius")} value="8 / 6" />
                </div>
              </section>
            </Match>

            <Match when={props.section === "providers"}>
              <section class="settings-panel">
                <header>
                  <span>{t("providers")}</span>
                  <small>{providers().length}</small>
                </header>
                <div class="settings-list">
                  <For
                    each={providers()}
                    fallback={
                      <div class="surface-list-empty">{t("empty")}</div>
                    }
                  >
                    {(provider) => (
                      <button
                        class={classNames(
                          "settings-provider-row",
                          selectedProvider()?.id === provider.id && "selected",
                        )}
                        onClick={() => chooseProvider(provider)}
                      >
                        <span>{provider.name}</span>
                        <small>
                          {providerStateLabel(
                            props.state,
                            provider.id,
                            provider.source,
                          )}
                        </small>
                      </button>
                    )}
                  </For>
                </div>
              </section>
              <section class="settings-panel">
                <header>
                  <span>{selectedProvider()?.name ?? t("provider")}</span>
                  <small>{authStatusText(selectedProviderStatus())}</small>
                </header>
                <Show when={selectedProvider()}>
                  {(provider) => (
                    <div class="provider-detail">
                      <div class="provider-metrics">
                        <MetricCell
                          label={t("state")}
                          value={authStatusText(selectedProviderStatus())}
                        />
                        <MetricCell
                          label={t("source")}
                          value={providerSourceLabel(provider().source)}
                        />
                        <MetricCell
                          label={t("env")}
                          value={provider().env.join(", ") || "--"}
                        />
                        <MetricCell
                          label={t("models")}
                          value={String(Object.keys(provider().models).length)}
                        />
                      </div>
                    </div>
                  )}
                </Show>
              </section>
            </Match>

            <Match when={props.section === "models"}>
              <section class="settings-panel">
                <header>
                  <span>{t("modelRuntime")}</span>
                  <small>{props.state.selectedModel ?? "--"}</small>
                </header>
                <div class="settings-fields">
                  <label class="field-row">
                    <span>{t("provider")}</span>
                    <select
                      value={selectedProvider()?.id ?? ""}
                      onChange={(event) => {
                        const provider = providers().find(
                          (item) => item.id === event.currentTarget.value,
                        );
                        if (provider) {
                          chooseProvider(provider);
                        }
                      }}
                    >
                      <For each={providers()}>
                        {(provider) => (
                          <option value={provider.id}>{provider.name}</option>
                        )}
                      </For>
                    </select>
                  </label>
                  <label class="field-row">
                    <span>{t("model")}</span>
                    <select
                      value={props.state.selectedModel ?? ""}
                      onChange={(event) =>
                        props.onModel(event.currentTarget.value)
                      }
                    >
                      <For each={selectedModels()}>
                        {(model) => (
                          <option
                            value={modelRef(selectedProvider()?.id, model.id)}
                          >
                            {model.name}
                          </option>
                        )}
                      </For>
                    </select>
                  </label>
                </div>
              </section>

              <section class="settings-panel">
                <header>
                  <span>{t("models")}</span>
                  <small>{selectedProvider()?.name ?? "--"}</small>
                </header>
                <Show
                  when={selectedProvider()}
                  fallback={<div class="surface-list-empty">{t("empty")}</div>}
                >
                  {(provider) => (
                    <div class="provider-detail">
                      <div class="model-list">
                        <For each={selectedModels()}>
                          {(model) => (
                            <button
                              class={classNames(
                                props.state.selectedModel ===
                                  modelRef(provider().id, model.id) &&
                                  "selected",
                              )}
                              onClick={() =>
                                props.onModel(modelRef(provider().id, model.id))
                              }
                            >
                              <span>{model.name}</span>
                              <small>
                                {formatModelLimit(model.limit.context)}
                              </small>
                            </button>
                          )}
                        </For>
                      </div>
                    </div>
                  )}
                </Show>
              </section>
            </Match>

            <Match when={props.section === "auth"}>
              <section class="settings-panel">
                <header>
                  <span>{t("login")}</span>
                  <small>{authStatusText(selectedProviderStatus())}</small>
                </header>
                <Show
                  when={selectedProvider()}
                  fallback={<div class="surface-list-empty">{t("empty")}</div>}
                >
                  {(provider) => (
                    <div class="settings-fields login-fields">
                      <For
                        each={selectedMethods()}
                        fallback={
                          <div class="surface-list-empty">{t("empty")}</div>
                        }
                      >
                        {(method, index) => (
                          <div
                            class={classNames(
                              "login-method",
                              method.type === "oauth" && "oauth",
                            )}
                          >
                            <div class="login-method-copy">
                              <span>{method.label}</span>
                              <small>
                                {method.token_env ??
                                  method.login_env ??
                                  method.kind}
                              </small>
                            </div>
                            <Show when={method.type === "api"}>
                              <div class="login-method-controls">
                                <input
                                  type="password"
                                  value={
                                    props.state.authDrafts[provider().id] ?? ""
                                  }
                                  placeholder={method.token_env ?? t("apiKey")}
                                  onInput={(event) =>
                                    props.onAuthDraft(
                                      provider().id,
                                      event.currentTarget.value,
                                    )
                                  }
                                />
                                <button
                                  class="secondary"
                                  disabled={
                                    props.state.settingsSaving ||
                                    !props.state.authDrafts[
                                      provider().id
                                    ]?.trim()
                                  }
                                  onClick={() =>
                                    props.onSaveKey(provider().id, method)
                                  }
                                >
                                  {t("save")}
                                </button>
                              </div>
                            </Show>
                            <Show when={method.type === "oauth"}>
                              <div class="login-method-controls oauth-controls">
                                <button
                                  class="secondary"
                                  disabled={props.state.settingsSaving}
                                  onClick={() =>
                                    props.onStartLogin(provider().id, index())
                                  }
                                >
                                  {t("openLogin")}
                                </button>
                                <input
                                  value={
                                    props.state.authCodeDrafts[provider().id] ??
                                    ""
                                  }
                                  placeholder={t("codeOrToken")}
                                  onInput={(event) =>
                                    props.onAuthCode(
                                      provider().id,
                                      event.currentTarget.value,
                                    )
                                  }
                                />
                                <button
                                  class="secondary"
                                  disabled={props.state.settingsSaving}
                                  onClick={() =>
                                    props.onCompleteLogin(
                                      provider().id,
                                      props.state.authCodeDrafts[provider().id],
                                      index(),
                                    )
                                  }
                                >
                                  {t("complete")}
                                </button>
                              </div>
                            </Show>
                          </div>
                        )}
                      </For>
                      <div class="settings-actions-row">
                        <button
                          class="text-button"
                          disabled={
                            props.state.settingsSaving ||
                            !selectedProviderStatus()?.configured
                          }
                          onClick={() => props.onLogout(provider().id)}
                        >
                          {t("logout")}
                        </button>
                      </div>
                    </div>
                  )}
                </Show>
              </section>
            </Match>

            <Match when={props.section === "runtime"}>
              <section class="settings-panel">
                <header>
                  <span>{t("runtime")}</span>
                  <small>{props.state.selectedAgent ?? "--"}</small>
                </header>
                <div class="settings-fields">
                  <label class="field-row">
                    <span>{t("agent")}</span>
                    <select
                      value={props.state.selectedAgent ?? ""}
                      onChange={(event) =>
                        props.onAgent(event.currentTarget.value || undefined)
                      }
                    >
                      <For
                        each={props.state.agents.filter(
                          (agent) => !agent.hidden,
                        )}
                      >
                        {(agent) => (
                          <option value={agent.name}>{agent.name}</option>
                        )}
                      </For>
                    </select>
                  </label>
                  <div class="field-row">
                    <span>{t("variant")}</span>
                    <div class="segmented">
                      <For each={["low", "medium", "high"]}>
                        {(variant) => (
                          <button
                            class={classNames(
                              props.state.modelVariant === variant &&
                                "selected",
                            )}
                            onClick={() => props.onVariant(variant)}
                          >
                            {variant}
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                  <label class="field-row compact-field">
                    <span>{t("acceleration")}</span>
                    <input
                      type="checkbox"
                      checked={props.state.accelerationEnabled}
                      onChange={(event) =>
                        props.onAcceleration(event.currentTarget.checked)
                      }
                    />
                  </label>
                  <ReadonlyRow
                    label={t("model")}
                    value={props.state.selectedModel ?? "--"}
                  />
                </div>
              </section>
            </Match>

            <Match when={props.section === "config"}>
              <section class="settings-panel">
                <header>
                  <span>{t("turaConfig")}</span>
                  <small>{t("global")}</small>
                </header>
                <div class="settings-fields">
                  <For each={configRows()}>
                    {(row) => (
                      <label class="field-row">
                        <span>{row.label}</span>
                        <input
                          value={props.state.configDraft[row.key] ?? ""}
                          onInput={(event) =>
                            props.onConfigDraft(
                              row.key,
                              event.currentTarget.value,
                            )
                          }
                        />
                      </label>
                    )}
                  </For>
                </div>
              </section>
            </Match>

            <Match when={props.section === "workspace"}>
              <section class="settings-panel">
                <header>
                  <span>{t("workspaceConfig")}</span>
                  <small>{workspaceRows().length}</small>
                </header>
                <div class="settings-fields">
                  <For
                    each={workspaceRows()}
                    fallback={
                      <div class="surface-list-empty">{t("empty")}</div>
                    }
                  >
                    {([key, value]) => (
                      <label class="field-row">
                        <span>{key}</span>
                        <input
                          value={value}
                          onInput={(event) =>
                            props.onWorkspaceConfigDraft(
                              key,
                              event.currentTarget.value,
                            )
                          }
                        />
                      </label>
                    )}
                  </For>
                </div>
              </section>
            </Match>

            <Match when={props.section === "environment"}>
              <section class="settings-panel">
                <header>
                  <span>{t("paths")}</span>
                  <small>{props.state.connection}</small>
                </header>
                <div class="settings-fields">
                  <ReadonlyRow
                    label={t("home")}
                    value={props.state.paths?.home ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("state")}
                    value={props.state.paths?.state ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("config")}
                    value={props.state.paths?.config ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("worktree")}
                    value={props.state.paths?.worktree ?? "--"}
                  />
                </div>
              </section>
              <section class="settings-panel">
                <header>
                  <span>{t("env")}</span>
                  <small>{selectedProvider()?.name ?? "--"}</small>
                </header>
                <div class="settings-fields">
                  <For each={providers().slice(0, 10)}>
                    {(provider) => (
                      <ReadonlyRow
                        label={provider.name}
                        value={provider.env.join(", ") || "--"}
                      />
                    )}
                  </For>
                </div>
              </section>
            </Match>
          </Switch>

          <Show
            when={props.state.settingsNotice || props.state.modelValidation}
          >
            <div class="settings-note">
              {props.state.settingsNotice ?? props.state.modelValidation}
            </div>
          </Show>
        </section>
      </main>
    </section>
  );
}

function ReadonlyRow(props: { label: string; value: string }) {
  return (
    <div class="field-row readonly-row">
      <span>{props.label}</span>
      <code>{props.value}</code>
    </div>
  );
}

function MetricCell(props: { label: string; value: string }) {
  return (
    <div class="metric-cell">
      <span>{props.value}</span>
      <small>{props.label}</small>
    </div>
  );
}

function PlanView(props: {
  state: AppState;
  previewSession?: Session;
  previewMessages: Message[];
  slashCommands: Command[];
  onPlanMode: (value: PlanMode) => void;
  onSearch: (value: string) => void;
  onDraftLane: (value: PlanStatus | undefined) => void;
  onDraftStartCondition: (value: StartCondition) => void;
  onDraftStartAt: (value: string) => void;
  onDraftPollInterval: (value: PollInterval) => void;
  onDraftSession: (value: string | undefined) => void;
  onCreateTicket: () => void;
  onStatus: (session: Session, status: PlanStatus) => void;
  attentionAcknowledged: (session: Session) => boolean;
  onTask: (
    session: Session,
    patch: Partial<
      TaskManagement & {
        status: PlanStatus;
        start_at: string;
        poll_interval: PollInterval;
      }
    >,
  ) => void;
  onEditTask: (
    session: Session,
    task: TaskManagement,
    composerText: string,
  ) => void;
  onDeleteTask: (session: Session, task: TaskManagement) => void;
  onCreateSessionFromTask: (session: Session, task: TaskManagement) => void;
  onOpenSession: (session: Session) => void;
  onComposerText: (text: string) => void;
  onComposerImages: (images: ComposerImage[]) => void;
  onSubmit: () => void;
  onOpenFullConversation: () => void;
  onClosePanel: () => void;
}) {
  const workspaceSessions = createMemo(() =>
    props.state.sessions.filter((session) =>
      samePath(sessionDirectory(session), props.state.directory),
    ),
  );
  const visibleSessions = createMemo(() => {
    const query = props.state.issueSearch.trim().toLowerCase();
    const sessions = workspaceSessions().filter(
      (session) => planSessionStatus(session) !== "archived",
    );
    if (!query) {
      return sessions;
    }
    return sessions.filter(
      (session) =>
        sessionTitle(session).toLowerCase().includes(query) ||
        session.id.toLowerCase().includes(query),
    );
  });
  const panelOpen = createMemo(() =>
    Boolean(props.previewSession || props.state.planDraftLane),
  );
  const [panelWidth, setPanelWidth] = createSignal(480);

  function beginPanelResize(event: PointerEvent) {
    event.preventDefault();
    const target = event.currentTarget as HTMLElement;
    const workbenchWidth =
      target.closest(".plan-workbench")?.getBoundingClientRect().width ??
      window.innerWidth;
    const startX = event.clientX;
    const startWidth = panelWidth();
    let closed = false;
    const onMove = (move: PointerEvent) => {
      const nextWidth = startWidth + startX - move.clientX;
      if (nextWidth < 300 || move.clientX > window.innerWidth - 12) {
        closed = true;
        onUp();
        closePlanPanel();
        return;
      }
      const maxWidth = Math.max(
        340,
        Math.min(window.innerWidth * 0.72, workbenchWidth - 360),
      );
      setPanelWidth(Math.max(340, Math.min(maxWidth, nextWidth)));
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      if (closed) {
        setPanelWidth(480);
      }
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  function closePlanPanel() {
    props.onClosePanel();
  }

  function openDraft(lane: PlanStatus | undefined) {
    props.onDraftLane(lane);
    props.onDraftSession(undefined);
    props.onDraftStartCondition("user_action");
    props.onDraftStartAt("");
    props.onDraftPollInterval(defaultPollInterval());
    props.onComposerText("");
  }

  function openDraftAt(startAt: string) {
    props.onDraftLane("todo");
    props.onDraftSession(undefined);
    props.onDraftStartCondition("scheduled_task");
    props.onDraftStartAt(utcIsoToLocalDateTime(startAt));
    props.onDraftPollInterval(defaultPollInterval());
    props.onComposerText("");
  }

  function toggleSplitPanel() {
    if (panelOpen()) {
      closePlanPanel();
      return;
    }
    const session =
      workspaceSessions().find(
        (item) => item.id === props.state.selectedSessionId,
      ) ?? visibleSessions()[0];
    if (session) {
      void props.onOpenSession(session);
    }
  }
  return (
    <section
      class={classNames(
        "product-workbench plan-workbench",
        panelOpen() && "plan-split-workbench",
      )}
    >
      <div class="plan-main">
        <header class="page-head plan-head">
          <div class="page-title">
            <span>{t("plan")}</span>
            <h1>{shortWorkspaceLabel(props.state.directory)}</h1>
          </div>
          <div class="page-actions">
            <label class="search-box">
              <input
                value={props.state.issueSearch}
                onInput={(event) => props.onSearch(event.currentTarget.value)}
                placeholder={t("search")}
              />
            </label>
            <PlanModeButtons
              mode={props.state.planMode}
              splitOpen={panelOpen()}
              onMode={props.onPlanMode}
              onSplit={toggleSplitPanel}
            />
          </div>
        </header>

        <main
          class={classNames(
            "plan-board",
            props.state.planMode === "calendar" && "calendar-mode",
          )}
        >
          <Switch>
            <Match when={props.state.planMode === "gantt"}>
              <PlanGanttView
                sessions={visibleSessions()}
                onOpenSession={props.onOpenSession}
                onSchedule={(session, startAt) =>
                  props.onTask(session, {
                    start_at: startAt,
                  })
                }
              />
            </Match>
            <Match when={props.state.planMode === "calendar"}>
              <PlanCalendarView
                sessions={visibleSessions()}
                onOpenSession={props.onOpenSession}
                onCreateAt={openDraftAt}
                onSchedule={(session, startAt) =>
                  props.onTask(session, {
                    start_at: startAt,
                  })
                }
              />
            </Match>
            <Match when={true}>
              <PlanBoard
                sessions={visibleSessions()}
                draftLane={props.state.planDraftLane}
                onDraftLane={openDraft}
                onStatus={props.onStatus}
                attentionAcknowledged={props.attentionAcknowledged}
                onOpenSession={props.onOpenSession}
              />
            </Match>
          </Switch>
        </main>
      </div>

      <Show when={panelOpen()}>
        <aside
          class="plan-conversation-panel"
          style={{ width: `${panelWidth()}px` }}
        >
          <div
            class="inspector-resize plan-panel-resize"
            role="separator"
            aria-orientation="vertical"
            onPointerDown={beginPanelResize}
          />
          <header class="plan-panel-topbar">
            <div class="plan-panel-title">
              <span>
                {props.state.planDraftLane ? t("newTicket") : t("conversation")}
              </span>
              <strong>
                {props.state.planDraftLane
                  ? props.previewSession
                    ? sessionTitle(props.previewSession)
                    : taskStateLabel(props.state.planDraftLane)
                  : props.previewSession
                    ? sessionTitle(props.previewSession)
                    : t("conversation")}
              </strong>
            </div>
            <button
              class="inspector-close"
              title={t("close")}
              onClick={closePlanPanel}
            >
              ×
            </button>
          </header>
          <ConversationView
            state={props.state}
            session={props.previewSession}
            messages={props.previewMessages}
            slashCommands={props.slashCommands}
            onComposerText={props.onComposerText}
            onComposerImages={props.onComposerImages}
            onSubmit={
              props.state.planDraftLane ? props.onCreateTicket : props.onSubmit
            }
            submitDisabled={
              Boolean(props.state.planDraftLane) &&
              props.state.composerText.trim().length === 0
            }
            composerToolbar={
              props.state.planDraftLane ? (
                <div class="plan-composer-tools">
                  <PlanDraftSessionPicker
                    sessions={workspaceSessions()}
                    selectedSessionId={props.state.planDraftSessionId}
                    onSession={props.onDraftSession}
                  />
                  <PlanComposerControls
                    startCondition={props.state.planDraftStartCondition}
                    startAt={props.state.planDraftStartAt}
                    pollInterval={props.state.planDraftPollInterval}
                    onStartCondition={props.onDraftStartCondition}
                    onStartAt={props.onDraftStartAt}
                    onPollInterval={props.onDraftPollInterval}
                  />
                </div>
              ) : props.previewSession ? (
                <PlanComposerControls
                  startCondition={
                    taskStartCondition(sessionTaskState(props.previewSession))
                  }
                  startAt={utcIsoToLocalDateTime(
                    sessionTaskState(props.previewSession).start_at,
                  )}
                  pollInterval={
                    sessionTaskState(props.previewSession).poll_interval ??
                    defaultPollInterval()
                  }
                  onStartCondition={(_start_condition) =>
                    props.onTask(props.previewSession!, { status: "todo" })
                  }
                  onStartAt={(value) => {
                    const start_at = localDateTimeToUtcIso(value);
                    if (start_at) {
                      props.onTask(props.previewSession!, { start_at });
                    }
                  }}
                  onPollInterval={(poll_interval) =>
                    props.onTask(props.previewSession!, { poll_interval })
                  }
                />
              ) : undefined
            }
            composerTaskList={
              props.previewSession &&
              !props.state.planDraftLane &&
              hasVisibleSessionTasks(props.previewSession) ? (
                <PlanComposerTaskList
                  session={props.previewSession}
                  selected_nonce_id={props.state.editingTask?.nonce_id}
                  onEdit={(task, composerText) =>
                    props.onEditTask(props.previewSession!, task, composerText)
                  }
                  onDelete={(task) =>
                    props.onDeleteTask(props.previewSession!, task)
                  }
                  onCreateSession={(task) =>
                    props.onCreateSessionFromTask(props.previewSession!, task)
                  }
                />
              ) : undefined
            }
            conversationNotice={
              props.previewSession &&
              shouldShowPlanFeedbackPrompt(
                props.previewSession,
                props.state.composerText,
              ) ? (
                <PlanConversationFeedbackNotice />
              ) : undefined
            }
            compact
            onToolOpen={props.onOpenFullConversation}
          />
        </aside>
      </Show>
    </section>
  );
}

function PlanBoard(props: {
  sessions: Session[];
  draftLane?: PlanStatus;
  onDraftLane: (value: PlanStatus | undefined) => void;
  onStatus: (session: Session, status: PlanStatus) => void;
  attentionAcknowledged: (session: Session) => boolean;
  onOpenSession: (session: Session) => void;
}) {
  const columns: Array<{ id: PlanStatus; label: string }> = [
    { id: "todo", label: t("todo") },
    { id: "doing", label: t("doing") },
    { id: "question", label: t("question") },
    { id: "done", label: t("done") },
  ];
  const [dragState, setDragState] = createSignal<PlanDragState>();
  function dragSession(event: DragEvent): Session | undefined {
    return props.sessions.find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
  }
  function dropOnStatus(event: DragEvent, status: PlanStatus) {
    event.preventDefault();
    const session = dragSession(event);
    if (session) {
      props.onStatus(session, status);
    }
  }
  function beginBoardDrag(event: PointerEvent | MouseEvent, session: Session) {
    beginPlanPointerDrag({
      event,
      session,
      setDragState,
      onOpen: () => props.onOpenSession(session),
      onSchedule: () => undefined,
      resolveSchedule: () => undefined,
      onDrop: (point) => {
        const element = document.elementFromPoint(point.x, point.y) as
          | HTMLElement
          | undefined;
        const archive = element?.closest<HTMLElement>(".board-archive-zone");
        if (archive) {
          props.onStatus(session, "archived");
          return true;
        }
        const column = element?.closest<HTMLElement>("[data-plan-status]");
        const status = column?.dataset.planStatus as PlanStatus | undefined;
        if (status && ["todo", "doing", "question", "done"].includes(status)) {
          props.onStatus(session, status);
          return true;
        }
        return false;
      },
    });
  }
  return (
    <section class="board-shell">
      <PlanDragGhost state={dragState()} />
      <section class="board-grid">
        <For each={columns}>
          {(column) => {
            const sessions = createMemo(() =>
              props.sessions.filter(
                (session) => planSessionStatus(session) === column.id,
              ),
            );
            return (
              <section
                class="board-column"
                data-plan-status={column.id}
                onDragOver={(event) => event.preventDefault()}
                onDrop={(event) => dropOnStatus(event, column.id)}
              >
                <header>
                  <span class="board-column-title">
                    <span>{column.label}</span>
                  </span>
                  <Show when={column.id === "todo"}>
                    <button
                      class="icon-action small"
                      title={t("create")}
                      onClick={() => props.onDraftLane(column.id)}
                    >
                      <Plus size={15} />
                    </button>
                  </Show>
                </header>
                <div
                  class={classNames(
                    "board-cards",
                    props.draftLane === column.id && "draft-target",
                  )}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) => dropOnStatus(event, column.id)}
                >
                  <For each={sessions()}>
                    {(session) => (
                      <article
                        class="board-card"
                        draggable="true"
                        onPointerDown={(event) =>
                          beginBoardDrag(event, session)
                        }
                        onMouseDown={(event) => beginBoardDrag(event, session)}
                        onDragStart={(event) => {
                          event.dataTransfer?.setData(
                            "text/session-id",
                            session.id,
                          );
                          event.currentTarget.classList.add(
                            "plan-source-dragging",
                          );
                        }}
                        onDragEnd={(event) =>
                          event.currentTarget.classList.remove(
                            "plan-source-dragging",
                          )
                        }
                        onClick={() => props.onOpenSession(session)}
                        title={sessionTitle(session)}
                      >
                        <small>{shortSessionId(session.id)}</small>
                        <span class="board-card-title">
                          <strong>{sessionTitle(session)}</strong>
                          <Show
                            when={shouldShowSessionAttention(
                              session,
                              props.attentionAcknowledged(session),
                            )}
                          >
                            <PlanStatusIndicator
                              status={planSessionStatus(session)}
                            />
                          </Show>
                        </span>
                        <PlanTicketMeta session={session} />
                      </article>
                    )}
                  </For>
                </div>
              </section>
            );
          }}
        </For>
      </section>
      <div
        class={classNames("board-archive-zone", dragState() && "active")}
        aria-hidden="true"
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          const session = dragSession(event);
          if (session) {
            props.onStatus(session, "archived");
          }
        }}
      />
    </section>
  );
}

let activePlanPointerDrag = false;

function PlanStatusIndicator(props: { status: PlanStatus }) {
  return (
    <Show
      when={
        props.status === "doing" ||
        props.status === "question" ||
        props.status === "done"
      }
    >
      <span
        class={classNames("plan-status-indicator", `status-${props.status}`)}
        aria-hidden="true"
      />
    </Show>
  );
}

function shouldShowSessionAttention(
  session: Session,
  acknowledged: boolean,
): boolean {
  const status = planSessionStatus(session);
  return (
    !acknowledged &&
    (status === "doing" || status === "question" || status === "done")
  );
}

function SessionRowMeta(props: {
  session: Session;
  attentionAcknowledged: boolean;
}) {
  const status = createMemo(() => planSessionStatus(props.session));
  return (
    <Show
      when={shouldShowSessionAttention(
        props.session,
        props.attentionAcknowledged,
      )}
      fallback={
        <small class="session-row-time">
          {relativeSessionTime(props.session)}
        </small>
      }
    >
      <span class="session-row-status">
        <PlanStatusIndicator status={status()} />
      </span>
    </Show>
  );
}

function PlanGanttView(props: {
  sessions: Session[];
  onOpenSession: (session: Session) => void;
  onSchedule: (session: Session, startAt: string) => void;
}) {
  const [dragState, setDragState] = createSignal<PlanDragState>();
  const [timelineMode, setTimelineMode] = createSignal<PlanGanttMode>("week");
  const timedSessions = createMemo(() => planTimedSessions(props.sessions));
  const [timelineCursor, setTimelineCursor] = createSignal(
    planTimelineStart(timedSessions()),
  );
  const [timelineWidth, setTimelineWidth] = createSignal(0);
  const dayHourCount = createMemo(() => {
    const width = timelineWidth();
    if (width <= 0) {
      return 6;
    }
    return Math.max(2, Math.min(12, Math.floor(width / 76)));
  });
  const timelineMarks = createMemo(() =>
    planTimelineMarks(timelineCursor(), timelineMode(), dayHourCount()),
  );
  const timelineTitle = createMemo(() =>
    timelineMode() === "day"
      ? formatGanttDayTitle(timelineMarks())
      : formatCalendarWeekTitle(timelineMarks()),
  );
  const timelineWindowMs = createMemo(() =>
    timelineMode() === "day" ? dayHourCount() * HOUR_MS : 7 * DAY_MS,
  );
  let timelineSessionsKey = "";
  createEffect(() => {
    const key = timedSessions()
      .map(
        (session) =>
          `${session.id}:${planSessionDate(session)?.toISOString() ?? ""}`,
      )
      .join("|");
    if (key !== timelineSessionsKey) {
      timelineSessionsKey = key;
      setTimelineCursor(planTimelineStart(timedSessions()));
    }
  });
  const todayPosition = createMemo(() => {
    const marks = timelineMarks();
    const start = marks[0]?.getTime();
    if (!start) {
      return undefined;
    }
    const ratio = (Date.now() - start) / timelineWindowMs();
    return ratio >= 0 && ratio <= 1 ? ratio : undefined;
  });
  let timelineEl: HTMLDivElement | undefined;
  let lastEdgeMoveAt = 0;
  let pixelMinuteRemainder = 0;
  let holdScrollTimer: number | undefined;
  const ganttRows = createMemo(() =>
    timedSessions()
      .map((session) => ({ session, tasks: timedSessionTasks(session) }))
      .filter((row) => row.tasks.length > 0),
  );
  createEffect(() => {
    if (!timelineEl) {
      return;
    }
    const updateWidth = () => setTimelineWidth(timelineTrackWidth());
    updateWidth();
    const observer = new ResizeObserver(updateWidth);
    observer.observe(timelineEl);
    window.addEventListener("resize", updateWidth);
    onCleanup(() => {
      observer.disconnect();
      window.removeEventListener("resize", updateWidth);
    });
  });
  function sessionTimelineStyle(session: Session): JSX.CSSProperties {
    const date = planSessionDate(session);
    const marks = timelineMarks();
    if (!date || marks.length === 0) {
      return { display: "none" };
    }
    const windowStart = marks[0]!.getTime();
    const windowEnd = windowStart + timelineWindowMs();
    const time = date.getTime();
    if (time < windowStart || time >= windowEnd) {
      return { display: "none" };
    }
    const position = ((time - windowStart) / (windowEnd - windowStart)) * 100;
    return {
      left: `${position}%`,
      "--plan-bar-width": "min(160px, calc(100% - 8px))",
    };
  }
  function timelinePointerDate(point: { x: number }): string | undefined {
    const marks = timelineMarks();
    if (!timelineEl || marks.length === 0) {
      return undefined;
    }
    const rect = timelineEl.getBoundingClientRect();
    const axis = timelineEl.querySelector<HTMLElement>(
      ".plan-timeline-left-head",
    );
    const start = axis?.getBoundingClientRect().right ?? rect.left;
    const width = rect.width - (start - rect.left);
    if (width <= 0) {
      return undefined;
    }
    const ratio = Math.max(0, Math.min(1, (point.x - start) / width));
    const windowStart = marks[0]!.getTime();
    const minutes = Math.round((ratio * timelineWindowMs()) / 60_000);
    const next = new Date(windowStart + minutes * 60_000);
    return Number.isNaN(next.getTime()) ? undefined : next.toISOString();
  }
  function dropOnDay(event: DragEvent, day: Date) {
    event.preventDefault();
    const session = props.sessions.find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    if (session) {
      const startAt =
        timelinePointerDate({ x: event.clientX }) ??
        dateWithPointerMinutes(day, event.currentTarget as HTMLElement, {
          axis: "x",
          x: event.clientX,
          y: event.clientY,
        }).toISOString();
      props.onSchedule(session, startAt);
    }
  }
  function beginGanttDrag(event: PointerEvent | MouseEvent, session: Session) {
    beginPlanPointerDrag({
      event,
      session,
      setDragState,
      onOpen: () => props.onOpenSession(session),
      onSchedule: (startAt) => props.onSchedule(session, startAt),
      onMove: (point) => scrollTimelineAtEdge(point),
      resolveSchedule: (point) =>
        timelinePointerDate(point) ?? pointerScheduleFromElement(point, "x"),
    });
  }
  function moveTimelineMinutes(minutesDelta: number) {
    setTimelineCursor(
      (cursor) => new Date(cursor.getTime() + minutesDelta * 60_000),
    );
  }
  function moveTimelineWindow(direction: number) {
    moveTimelineMinutes(
      direction * Math.round(timelineWindowMs() / 60_000 / 30),
    );
  }
  function stopTimelineHold() {
    if (holdScrollTimer !== undefined) {
      window.clearInterval(holdScrollTimer);
      holdScrollTimer = undefined;
    }
    window.removeEventListener("pointerup", stopTimelineHold);
    window.removeEventListener("pointercancel", stopTimelineHold);
    window.removeEventListener("mouseup", stopTimelineHold);
  }
  function beginTimelineHold(
    event: PointerEvent | MouseEvent,
    direction: number,
  ) {
    event.preventDefault();
    event.stopPropagation();
    stopTimelineHold();
    moveTimelineWindow(direction);
    holdScrollTimer = window.setInterval(
      () => moveTimelineWindow(direction),
      100,
    );
    window.addEventListener("pointerup", stopTimelineHold, { once: true });
    window.addEventListener("pointercancel", stopTimelineHold, { once: true });
    window.addEventListener("mouseup", stopTimelineHold, { once: true });
  }
  function timelineTrackWidth(): number {
    if (!timelineEl) {
      return 0;
    }
    const rect = timelineEl.getBoundingClientRect();
    const axis = timelineEl.querySelector<HTMLElement>(
      ".plan-timeline-left-head",
    );
    const leftWidth = axis?.getBoundingClientRect().width ?? 0;
    return Math.max(0, rect.width - leftWidth);
  }
  function moveTimelineByPixels(deltaX: number) {
    const width = timelineTrackWidth();
    if (width <= 0 || deltaX === 0) {
      return;
    }
    const rawMinutes =
      (-deltaX / width) * (timelineWindowMs() / 60_000) + pixelMinuteRemainder;
    const minutes =
      rawMinutes < 0 ? Math.ceil(rawMinutes) : Math.floor(rawMinutes);
    pixelMinuteRemainder = rawMinutes - minutes;
    if (minutes !== 0) {
      moveTimelineMinutes(minutes);
    }
  }
  function wheelTimeline(event: WheelEvent) {
    const delta =
      Math.abs(event.deltaX) > Math.abs(event.deltaY)
        ? event.deltaX
        : event.deltaY;
    if (delta === 0) {
      return;
    }
    event.preventDefault();
    moveTimelineByPixels(delta);
  }
  function scrollTimelineAtEdge(point: { x: number }) {
    if (!timelineEl) {
      return;
    }
    const rect = timelineEl.getBoundingClientRect();
    const edge = 56;
    const now = Date.now();
    if (now - lastEdgeMoveAt < 60) {
      return;
    }
    if (point.x < rect.left + edge) {
      moveTimelineByPixels(-(rect.left + edge - point.x));
      lastEdgeMoveAt = now;
    } else if (point.x > rect.right - edge) {
      moveTimelineByPixels(point.x - (rect.right - edge));
      lastEdgeMoveAt = now;
    }
  }
  function beginTimelinePan(event: PointerEvent | MouseEvent) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    let lastX = event.clientX;
    const onMove = (move: PointerEvent | MouseEvent) => {
      const delta = move.clientX - lastX;
      moveTimelineByPixels(delta);
      lastX = move.clientX;
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp, { once: true });
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp, { once: true });
  }
  return (
    <section class="plan-gantt">
      <PlanDragGhost state={dragState()} />
      <div
        ref={timelineEl}
        class="plan-timeline-grid"
        style={{ "--plan-days": String(timelineMarks().length) }}
        onWheel={wheelTimeline}
      >
        <div
          class="plan-timeline-scale"
          onPointerDown={beginTimelinePan}
          onMouseDown={beginTimelinePan}
        >
          <header class="plan-calendar-title plan-timeline-title">
            <div class="plan-calendar-nav">
              <button
                class="icon-action"
                type="button"
                title={t("previous")}
                onPointerDown={(event) => beginTimelineHold(event, -1)}
                onMouseDown={(event) => beginTimelineHold(event, -1)}
              >
                <ChevronLeft size={16} />
              </button>
              <strong>{timelineTitle()}</strong>
              <button
                class="icon-action"
                type="button"
                title={t("next")}
                onPointerDown={(event) => beginTimelineHold(event, 1)}
                onMouseDown={(event) => beginTimelineHold(event, 1)}
              >
                <ChevronRight size={16} />
              </button>
            </div>
            <div class="plan-calendar-view-toggle plan-gantt-view-toggle">
              <button
                type="button"
                class={classNames(timelineMode() === "week" && "selected")}
                onPointerDown={(event) => event.stopPropagation()}
                onMouseDown={(event) => event.stopPropagation()}
                onClick={() => setTimelineMode("week")}
              >
                {t("week")}
              </button>
              <button
                type="button"
                class={classNames(timelineMode() === "day" && "selected")}
                onPointerDown={(event) => event.stopPropagation()}
                onMouseDown={(event) => event.stopPropagation()}
                onClick={() => setTimelineMode("day")}
              >
                {t("day")}
              </button>
            </div>
          </header>
          <span class="plan-timeline-left-head" aria-hidden="true"></span>
          <For each={timelineMarks()}>
            {(mark, index) => (
              <span
                style={{
                  "grid-column": String(index() + 2),
                }}
                class="plan-timeline-day"
                data-plan-timeline-day={mark.toISOString()}
              >
                <small>{formatGanttMarkTop(mark, timelineMode())}</small>
                <strong>{formatGanttMarkBottom(mark, timelineMode())}</strong>
              </span>
            )}
          </For>
        </div>
        <Show when={todayPosition() !== undefined}>
          <i
            class="plan-today-line"
            style={{ "--today": String(todayPosition()) }}
          />
        </Show>
        <For each={ganttRows()}>
          {(row) => {
            const session = row.session;
            const topTask = () => row.tasks[0] ?? sessionTaskState(session);
            const barStyle = createMemo(() => sessionTimelineStyle(session));
            return (
              <div class="plan-timeline-row">
                <span>
                  <strong>{sessionTitle(session)}</strong>
                  <small>{shortSessionId(session.id)}</small>
                </span>
                <div class="plan-timeline-track">
                  <For each={row.tasks.slice(1, 4)}>
                    {(_, index) => (
                      <i
                        class="plan-timeline-stack-card"
                        style={{
                          ...barStyle(),
                          "--plan-stack-offset": `${(index() + 1) * 4}px`,
                        }}
                      />
                    )}
                  </For>
                  <button
                    class={classNames(
                      "plan-timeline-bar",
                      `status-${planSessionStatus(session)}`,
                      planTriggerClass(session),
                    )}
                    style={barStyle()}
                    onPointerDown={(event) => beginGanttDrag(event, session)}
                    onMouseDown={(event) => beginGanttDrag(event, session)}
                    onClick={(event) => event.preventDefault()}
                    title={sessionTitle(session)}
                  >
                    <strong>
                      {taskSummaryText(topTask()) || planTaskTitle(session)}
                    </strong>
                  </button>
                  <For each={timelineMarks()}>
                    {(day) => (
                      <button
                        class="plan-timeline-drop"
                        type="button"
                        title={day.toLocaleDateString()}
                        onDragOver={(event) => event.preventDefault()}
                        onDrop={(event) => dropOnDay(event, day)}
                        data-plan-timeline-day={day.toISOString()}
                      />
                    )}
                  </For>
                </div>
              </div>
            );
          }}
        </For>
      </div>
    </section>
  );
}

function PlanCalendarView(props: {
  sessions: Session[];
  onOpenSession: (session: Session) => void;
  onCreateAt: (startAt: string) => void;
  onSchedule: (session: Session, startAt: string) => void;
}) {
  const [dragState, setDragState] = createSignal<PlanDragState>();
  const timedSessions = createMemo(() => planTimedSessions(props.sessions));
  const [calendarView, setCalendarView] =
    createSignal<PlanCalendarMode>("month");
  const [calendarCursor, setCalendarCursor] = createSignal(
    planInitialCalendarDate(timedSessions()),
  );
  const monthStart = createMemo(() => {
    const cursor = calendarCursor();
    return new Date(cursor.getFullYear(), cursor.getMonth(), 1);
  });
  const days = createMemo(() => calendarGridDays(monthStart()));
  const weekDays = createMemo(() => calendarWeekDays(calendarCursor()));
  const activeHourDays = createMemo(() =>
    calendarView() === "day" ? [startOfDay(calendarCursor())] : weekDays(),
  );
  const weekHours = Array.from({ length: 24 }, (_, index) => index);
  const calendarTitle = createMemo(() =>
    calendarView() === "day"
      ? calendarCursor().toLocaleDateString(undefined, {
          month: "long",
          day: "numeric",
          year: "numeric",
        })
      : calendarView() === "week"
        ? formatCalendarWeekTitle(weekDays())
        : monthStart().toLocaleDateString(undefined, {
            month: "long",
            year: "numeric",
          }),
  );
  let hourGridEl: HTMLDivElement | undefined;
  function sessionsForDay(day: Date): Session[] {
    return timedSessions().filter((session) => {
      const date = planSessionDate(session);
      return date ? sameCalendarDay(date, day) : false;
    });
  }
  function sessionsForDayHour(day: Date, hour: number): Session[] {
    return sessionsForDay(day).filter((session) => {
      const date = planSessionDate(session);
      return date ? date.getHours() === hour : false;
    });
  }
  function dropOnDay(event: DragEvent, day: Date) {
    event.preventDefault();
    const session = timedSessions().find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    if (session) {
      props.onSchedule(
        session,
        dateWithPointerMinutes(day, event.currentTarget as HTMLElement, {
          axis: "y",
          x: event.clientX,
          y: event.clientY,
        }).toISOString(),
      );
    }
  }
  function dropOnDayHour(event: DragEvent, day: Date, hour: number) {
    event.preventDefault();
    const session = timedSessions().find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    if (session) {
      const next = new Date(day);
      const minuteRatio = pointerRatio(
        event.currentTarget as HTMLElement,
        event.clientY,
        "y",
      );
      next.setHours(hour, Math.round(minuteRatio * 59), 0, 0);
      props.onSchedule(session, next.toISOString());
    }
  }
  function beginCalendarDrag(
    event: PointerEvent | MouseEvent,
    session: Session,
  ) {
    beginPlanPointerDrag({
      event,
      session,
      setDragState,
      onOpen: () => props.onOpenSession(session),
      onSchedule: (startAt) => props.onSchedule(session, startAt),
      onMove: (point) => scrollCalendarAtEdge(point),
      resolveSchedule: (point) => pointerScheduleFromElement(point, "y"),
    });
  }
  function scrollCalendarAtEdge(point: { x: number; y: number }) {
    if (!hourGridEl) {
      return;
    }
    const rect = hourGridEl.getBoundingClientRect();
    const edge = 58;
    const topDistance = point.y - rect.top;
    const bottomDistance = rect.bottom - point.y;
    if (topDistance < edge) {
      hourGridEl.scrollTop -= Math.max(1, edge - topDistance) * 0.38;
    } else if (bottomDistance < edge) {
      hourGridEl.scrollTop += Math.max(1, edge - bottomDistance) * 0.38;
    }
  }
  function openWeekFromBlank(event: MouseEvent, day: Date) {
    if ((event.target as HTMLElement).closest(".plan-calendar-event")) {
      return;
    }
    setCalendarCursor(day);
    setCalendarView("week");
  }
  function createDraftFromWeek(event: MouseEvent, day: Date, hour: number) {
    if ((event.target as HTMLElement).closest(".plan-calendar-event")) {
      return;
    }
    const start = new Date(day);
    start.setHours(
      hour,
      Math.round(
        pointerRatio(event.currentTarget as HTMLElement, event.clientY, "y") *
          59,
      ),
      0,
      0,
    );
    props.onCreateAt(start.toISOString());
  }
  function moveCalendar(amount: number) {
    const cursor = calendarCursor();
    if (calendarView() === "day") {
      setCalendarCursor(new Date(cursor.getTime() + amount * DAY_MS));
      return;
    }
    if (calendarView() === "week") {
      setCalendarCursor(new Date(cursor.getTime() + amount * 7 * DAY_MS));
      return;
    }
    setCalendarCursor(
      new Date(cursor.getFullYear(), cursor.getMonth() + amount, 1),
    );
  }
  return (
    <section class="plan-calendar">
      <PlanDragGhost state={dragState()} />
      <header class="plan-calendar-title">
        <div class="plan-calendar-nav">
          <button
            class="icon-action"
            type="button"
            title={t("previous")}
            onClick={() => moveCalendar(-1)}
          >
            <ChevronLeft size={16} />
          </button>
          <strong>{calendarTitle()}</strong>
          <button
            class="icon-action"
            type="button"
            title={t("next")}
            onClick={() => moveCalendar(1)}
          >
            <ChevronRight size={16} />
          </button>
        </div>
        <div class="plan-calendar-view-toggle">
          <button
            type="button"
            class={classNames(calendarView() === "month" && "selected")}
            onClick={() => setCalendarView("month")}
          >
            月
          </button>
          <button
            type="button"
            class={classNames(calendarView() === "week" && "selected")}
            onClick={() => setCalendarView("week")}
          >
            周
          </button>
          <button
            type="button"
            class={classNames(calendarView() === "day" && "selected")}
            onClick={() => setCalendarView("day")}
          >
            日
          </button>
        </div>
      </header>
      <Show
        when={calendarView() !== "month"}
        fallback={
          <>
            <div class="plan-calendar-weekdays">
              <For each={["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]}>
                {(day) => <span>{day}</span>}
              </For>
            </div>
            <div class="plan-calendar-grid">
              <For each={days()}>
                {(day) => (
                  <section
                    class={classNames(
                      "plan-calendar-cell",
                      day.getMonth() !== monthStart().getMonth() && "muted",
                      sameCalendarDay(day, new Date()) && "today",
                    )}
                    onClick={(event) => openWeekFromBlank(event, day)}
                    onDragOver={(event) => event.preventDefault()}
                    onDrop={(event) => dropOnDay(event, day)}
                    data-plan-day={day.toISOString()}
                  >
                    <header>
                      <span>{day.getDate()}</span>
                    </header>
                    <For each={sessionsForDay(day)}>
                      {(session) => (
                        <PlanCalendarEvent
                          session={session}
                          onOpenSession={props.onOpenSession}
                          onPointerDragStart={beginCalendarDrag}
                        />
                      )}
                    </For>
                  </section>
                )}
              </For>
            </div>
          </>
        }
      >
        <div
          class={classNames(
            "plan-calendar-week",
            calendarView() === "day" && "day-mode",
          )}
          style={{ "--calendar-days": String(activeHourDays().length) }}
        >
          <div class="plan-calendar-week-head">
            <span />
            <For each={activeHourDays()}>
              {(day) => (
                <button
                  type="button"
                  class={classNames(
                    "plan-calendar-week-day",
                    sameCalendarDay(day, new Date()) && "today",
                    sameCalendarDay(day, calendarCursor()) && "selected",
                  )}
                  onClick={() => setCalendarCursor(day)}
                  onDblClick={() => setCalendarView("day")}
                >
                  <small>
                    {day.toLocaleDateString(undefined, { weekday: "short" })}
                  </small>
                  <strong>{day.getDate()}</strong>
                </button>
              )}
            </For>
          </div>
          <div class="plan-calendar-week-grid" ref={hourGridEl}>
            <For each={weekHours}>
              {(hour) => (
                <>
                  <span class="plan-calendar-hour-label">
                    {String(hour).padStart(2, "0")}:00
                  </span>
                  <For each={activeHourDays()}>
                    {(day) => (
                      <section
                        class="plan-calendar-hour-cell"
                        onClick={(event) =>
                          createDraftFromWeek(event, day, hour)
                        }
                        onDragOver={(event) => event.preventDefault()}
                        onDrop={(event) => dropOnDayHour(event, day, hour)}
                        data-plan-hour-start={hourStartIso(day, hour)}
                      >
                        <For each={sessionsForDayHour(day, hour)}>
                          {(session) => (
                            <PlanCalendarEvent
                              session={session}
                              onOpenSession={props.onOpenSession}
                              onPointerDragStart={beginCalendarDrag}
                            />
                          )}
                        </For>
                      </section>
                    )}
                  </For>
                </>
              )}
            </For>
          </div>
        </div>
      </Show>
    </section>
  );
}

function PlanCalendarEvent(props: {
  session: Session;
  onOpenSession: (session: Session) => void;
  onPointerDragStart: (
    event: PointerEvent | MouseEvent,
    session: Session,
  ) => void;
}) {
  return (
    <button
      class={classNames(
        "plan-calendar-event",
        `status-${planSessionStatus(props.session)}`,
        planTriggerClass(props.session),
      )}
      type="button"
      onPointerDown={(event) => props.onPointerDragStart(event, props.session)}
      onMouseDown={(event) => props.onPointerDragStart(event, props.session)}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
      }}
    >
      <span>{sessionTitle(props.session)}</span>
      <small>
        {formatCalendarEventTime(
          sessionTaskState(props.session).start_at,
        )}
      </small>
    </button>
  );
}

type PlanDragState = {
  sessionId: string;
  title: string;
  x: number;
  y: number;
  offsetX: number;
  offsetY: number;
  width: number;
  height: number;
  className: string;
  html: string;
};

function PlanDragGhost(props: { state?: PlanDragState }) {
  return (
    <Show when={props.state}>
      {(state) => (
        <div
          class={classNames("plan-drag-ghost", state().className)}
          style={{
            left: `${state().x}px`,
            top: `${state().y}px`,
            width: `${state().width}px`,
            height: `${state().height}px`,
          }}
          innerHTML={state().html}
          aria-label={`${shortSessionId(state().sessionId)} ${state().title}`}
        />
      )}
    </Show>
  );
}

function beginPlanPointerDrag(options: {
  event: PointerEvent | MouseEvent;
  session: Session;
  setDragState: (value?: PlanDragState) => void;
  onOpen: () => void;
  onSchedule: (startAt: string) => void;
  onMove?: (point: { x: number; y: number }) => void;
  onDrop?: (point: { x: number; y: number }) => boolean;
  resolveSchedule: (point: { x: number; y: number }) => string | undefined;
}) {
  if (options.event.button !== 0) {
    return;
  }
  if (activePlanPointerDrag) {
    options.event.preventDefault();
    options.event.stopPropagation();
    return;
  }
  activePlanPointerDrag = true;
  options.event.preventDefault();
  options.event.stopPropagation();
  const startX = options.event.clientX;
  const startY = options.event.clientY;
  const sourceElement = options.event.currentTarget as HTMLElement | null;
  const sourceRect = sourceElement?.getBoundingClientRect();
  const offsetX = sourceRect ? startX - sourceRect.left : 0;
  const offsetY = sourceRect ? startY - sourceRect.top : 0;
  const sourceClassName = sourceElement
    ? sourceElement.className.replace(/\bplan-source-dragging\b/g, "").trim()
    : "";
  const sourceHtml = sourceElement?.innerHTML ?? "";
  let moved = false;
  const dragThreshold = 8;
  const updateGhost = (x: number, y: number) =>
    options.setDragState({
      sessionId: options.session.id,
      title: sessionTitle(options.session),
      x,
      y,
      offsetX,
      offsetY,
      width: sourceRect?.width ?? 220,
      height: sourceRect?.height ?? 30,
      className: sourceClassName,
      html: sourceHtml,
    });
  const onMove = (move: PointerEvent | MouseEvent) => {
    if (
      !moved &&
      Math.hypot(move.clientX - startX, move.clientY - startY) >= dragThreshold
    ) {
      moved = true;
      sourceElement?.classList.add("plan-source-dragging");
    }
    if (moved) {
      move.preventDefault();
      updateGhost(move.clientX, move.clientY);
      options.onMove?.({ x: move.clientX, y: move.clientY });
    }
  };
  const onUp = (up: PointerEvent | MouseEvent) => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    activePlanPointerDrag = false;
    sourceElement?.classList.remove("plan-source-dragging");
    options.setDragState(undefined);
    if (!moved) {
      options.onOpen();
      return;
    }
    if (options.onDrop?.({ x: up.clientX, y: up.clientY })) {
      return;
    }
    const startAt = options.resolveSchedule({ x: up.clientX, y: up.clientY });
    if (startAt) {
      options.onSchedule(startAt);
    }
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp, { once: true });
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp, { once: true });
}

function pointerScheduleFromElement(
  point: { x: number; y: number },
  axis: "x" | "y",
): string | undefined {
  const element = document.elementFromPoint(point.x, point.y) as
    | HTMLElement
    | undefined;
  const hourCell = element?.closest<HTMLElement>("[data-plan-hour-start]");
  if (hourCell?.dataset.planHourStart) {
    const start = new Date(hourCell.dataset.planHourStart);
    if (Number.isNaN(start.getTime())) {
      return undefined;
    }
    start.setMinutes(
      Math.round(pointerRatio(hourCell, point.y, "y") * 59),
      0,
      0,
    );
    return start.toISOString();
  }
  const dayCell = element?.closest<HTMLElement>("[data-plan-day]");
  if (dayCell?.dataset.planDay) {
    return dateWithPointerMinutes(new Date(dayCell.dataset.planDay), dayCell, {
      ...point,
      axis,
    }).toISOString();
  }
  const timelineCell = element?.closest<HTMLElement>(
    "[data-plan-timeline-day]",
  );
  if (timelineCell?.dataset.planTimelineDay) {
    return dateWithPointerMinutes(
      new Date(timelineCell.dataset.planTimelineDay),
      timelineCell,
      { ...point, axis },
    ).toISOString();
  }
  return undefined;
}

function dateWithPointerMinutes(
  day: Date,
  element: HTMLElement,
  point: { x: number; y: number; axis: "x" | "y" },
): Date {
  const next = startOfDay(day);
  const ratio = pointerRatio(
    element,
    point.axis === "x" ? point.x : point.y,
    point.axis,
  );
  const minutes = Math.max(0, Math.min(1439, Math.round(ratio * 1439)));
  next.setHours(Math.floor(minutes / 60), minutes % 60, 0, 0);
  return next;
}

function pointerRatio(
  element: HTMLElement,
  coordinate: number,
  axis: "x" | "y",
): number {
  const rect = element.getBoundingClientRect();
  const size = axis === "x" ? rect.width : rect.height;
  const start = axis === "x" ? rect.left : rect.top;
  if (size <= 0) {
    return 0;
  }
  return Math.max(0, Math.min(1, (coordinate - start) / size));
}

const HOUR_MS = 3_600_000;
const DAY_MS = 86_400_000;

function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function planSessionDate(session: Session): Date | undefined {
  const raw = sessionTaskState(session).start_at;
  const fallback = sessionTasks(session)
    .filter((task) => isTimedStartCondition(taskStartCondition(task)))
    .map((task) => taskStartAt(task))
    .find(Boolean);
  const date = raw ? new Date(raw) : fallback ? new Date(fallback) : undefined;
  return date && !Number.isNaN(date.getTime()) ? date : undefined;
}

function planTimelineDays(sessions: Session[], count: number): Date[] {
  const first = sessions.map(planSessionDate).find(Boolean) ?? new Date();
  const start = startOfDay(new Date(first.getTime() - 2 * DAY_MS));
  return Array.from(
    { length: count },
    (_, index) => new Date(start.getTime() + index * DAY_MS),
  );
}

function planTimelineStart(sessions: Session[]): Date {
  return planTimelineDays(sessions, 1)[0] ?? startOfDay(new Date());
}

function planTimelineWindow(anchor: Date, count: number): Date[] {
  const start = new Date(anchor);
  return Array.from(
    { length: count },
    (_, index) => new Date(start.getTime() + index * DAY_MS),
  );
}

type PlanGanttMode = "week" | "day";

function planTimelineMarks(
  anchor: Date,
  mode: PlanGanttMode,
  dayHourCount = 6,
): Date[] {
  const start = new Date(new Date(anchor).setSeconds(0, 0));
  const count = mode === "day" ? dayHourCount : 7;
  const step = mode === "day" ? HOUR_MS : DAY_MS;
  return Array.from(
    { length: count },
    (_, index) => new Date(start.getTime() + index * step),
  );
}

function formatGanttDayTitle(days: Date[]): string {
  const first = days[0];
  const last = days[days.length - 1];
  if (!first || !last) {
    return "";
  }
  const end = new Date(last.getTime() + HOUR_MS);
  const date = first.toLocaleDateString(undefined, {
    month: "long",
    day: "numeric",
    year: "numeric",
  });
  const time = (value: Date) =>
    value.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  return `${date} ${time(first)} - ${time(end)}`;
}

function formatGanttMarkTop(date: Date, mode: PlanGanttMode): string {
  if (mode === "day") {
    return date.toLocaleDateString(undefined, { weekday: "short" });
  }
  return date.toLocaleDateString(undefined, { weekday: "short" });
}

function formatGanttMarkBottom(date: Date, mode: PlanGanttMode): string {
  if (mode === "day") {
    return date.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  }
  return date.toLocaleDateString(undefined, {
    month: "numeric",
    day: "numeric",
  });
}

function planTimelineWeeks(days: Date[]): Array<{
  label: string;
  start: number;
  span: number;
}> {
  const weeks: Array<{ label: string; start: number; span: number }> = [];
  for (const [index, day] of days.entries()) {
    const week = calendarWeekDays(day);
    const label = `${week[0]!.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    })} - ${week[6]!.toLocaleDateString(undefined, { day: "numeric" })}`;
    const last = weeks[weeks.length - 1];
    if (last?.label === label) {
      last.span += 1;
    } else {
      weeks.push({ label, start: index, span: 1 });
    }
  }
  return weeks;
}

function calendarGridDays(monthStart: Date): Date[] {
  const start = startOfDay(
    new Date(
      monthStart.getFullYear(),
      monthStart.getMonth(),
      1 - monthStart.getDay(),
    ),
  );
  return Array.from(
    { length: 42 },
    (_, index) => new Date(start.getTime() + index * DAY_MS),
  );
}

function calendarWeekDays(anchor: Date): Date[] {
  const start = startOfDay(
    new Date(
      anchor.getFullYear(),
      anchor.getMonth(),
      anchor.getDate() - anchor.getDay(),
    ),
  );
  return Array.from(
    { length: 7 },
    (_, index) => new Date(start.getTime() + index * DAY_MS),
  );
}

function hourStartIso(day: Date, hour: number): string {
  const start = new Date(day);
  start.setHours(hour, 0, 0, 0);
  return start.toISOString();
}

function formatCalendarWeekTitle(days: Date[]): string {
  const first = days[0];
  const last = days[days.length - 1];
  if (!first || !last) {
    return "";
  }
  const format = (date: Date) =>
    date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  return `${format(first)} - ${format(last)}`;
}

function sameCalendarDay(left: Date, right: Date): boolean {
  return startOfDay(left).getTime() === startOfDay(right).getTime();
}

function PlanModeButtons(props: {
  mode: PlanMode;
  splitOpen: boolean;
  onMode: (value: PlanMode) => void;
  onSplit: () => void;
}) {
  const modes: Array<{
    id: PlanMode | "split";
    label: string;
    icon: (props: { size?: number }) => JSX.Element;
  }> = [
    { id: "gantt", label: t("gantt"), icon: ChartGantt },
    { id: "calendar", label: t("calendar"), icon: CalendarDays },
    { id: "todo", label: t("todoList"), icon: LayoutList },
    { id: "split", label: t("splitCollaboration"), icon: Columns3 },
  ];
  return (
    <div class="plan-mode-actions">
      <For each={modes}>
        {(mode) => {
          const Icon = mode.icon;
          return (
            <button
              class={classNames(
                "icon-action",
                (mode.id === "split"
                  ? props.splitOpen
                  : props.mode === mode.id) && "selected",
              )}
              title={mode.label}
              onClick={() =>
                mode.id === "split" ? props.onSplit() : props.onMode(mode.id)
              }
            >
              <Icon size={17} />
            </button>
          );
        }}
      </For>
    </div>
  );
}

function PlanTicketMeta(props: { session: Session }) {
  const task = createMemo(() => sessionTaskState(props.session));
  const condition = createMemo(() => taskStartCondition(task()));
  return (
    <div class="ticket-meta">
      <span>{formatStartCondition(condition())}</span>
      <Show when={isTimedStartCondition(condition())}>
        <span>{formatTicketTime(task().start_at)}</span>
      </Show>
    </div>
  );
}

function PlanComposerTaskList(props: {
  session: Session;
  selected_nonce_id?: string;
  onEdit: (task: TaskManagement, value: string) => void;
  onDelete: (task: TaskManagement) => void;
  onCreateSession: (task: TaskManagement) => void;
}) {
  const [menuNonce, setMenuNonce] = createSignal<string>();
  const tasks = createMemo(() => sortedSessionTasks(props.session));
  const queuedTasks = createMemo(() =>
    tasks().filter((task) => !isTimedStartCondition(taskStartCondition(task))),
  );
  const timedTasks = createMemo(() =>
    tasks().filter((task) => isTimedStartCondition(taskStartCondition(task))),
  );
  return (
    <Show when={tasks().length > 0}>
      <section class="composer-task-list" aria-label={t("taskManagement")}>
        <For each={queuedTasks()}>
          {(task) => (
            <PlanTaskRow
              task={task}
              selected={props.selected_nonce_id === taskNonceId(task)}
              menuOpen={menuNonce() === taskNonceId(task)}
              onMenu={() =>
                setMenuNonce(
                  menuNonce() === taskNonceId(task)
                    ? undefined
                    : taskNonceId(task),
                )
              }
              onEdit={() => props.onEdit(task, taskDisplayText(task))}
              onDelete={() => props.onDelete(task)}
              onCreateSession={() => props.onCreateSession(task)}
            />
          )}
        </For>
        <Show when={queuedTasks().length > 0 && timedTasks().length > 0}>
          <div class="composer-task-divider" aria-hidden="true" />
        </Show>
        <For each={timedTasks()}>
          {(task) => (
            <PlanTaskRow
              task={task}
              selected={props.selected_nonce_id === taskNonceId(task)}
              menuOpen={menuNonce() === taskNonceId(task)}
              onMenu={() =>
                setMenuNonce(
                  menuNonce() === taskNonceId(task)
                    ? undefined
                    : taskNonceId(task),
                )
              }
              onEdit={() => props.onEdit(task, taskDisplayText(task))}
              onDelete={() => props.onDelete(task)}
              onCreateSession={() => props.onCreateSession(task)}
            />
          )}
        </For>
      </section>
    </Show>
  );
}

function PlanConversationFeedbackNotice() {
  return (
    <div class="plan-feedback-prompt">
      <span aria-hidden="true" />
      <p>请输入命令或者反馈</p>
    </div>
  );
}

function shouldShowPlanFeedbackPrompt(
  session: Session,
  composerText: string,
): boolean {
  const status = planSessionStatus(session);
  if (status === "question" || status === "done") {
    return true;
  }
  return (
    status === "todo" &&
    !firstRunnableTask(session) &&
    composerText.trim().length > 0
  );
}

function PlanTaskRow(props: {
  task: TaskManagement;
  selected: boolean;
  menuOpen: boolean;
  onMenu: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onCreateSession: () => void;
}) {
  let moreButton: HTMLButtonElement | undefined;
  const [menuRect, setMenuRect] = createSignal({ left: 0, top: 0 });
  const scheduleText = createMemo(() => {
    const startCondition = taskStartCondition(props.task);
    return formatStartCondition(startCondition);
  });
  const remainingText = createMemo(() => formatTaskRemaining(props.task));
  function updateMenuPosition() {
    const rect = moreButton?.getBoundingClientRect();
    if (!rect) {
      return;
    }
    const width = 146;
    const left = Math.max(
      8,
      Math.min(rect.right - width, window.innerWidth - width - 8),
    );
    setMenuRect({ left, top: rect.top - 6 });
  }
  createEffect(() => {
    if (!props.menuOpen) {
      return;
    }
    updateMenuPosition();
    window.addEventListener("resize", updateMenuPosition);
    window.addEventListener("scroll", updateMenuPosition, true);
    onCleanup(() => {
      window.removeEventListener("resize", updateMenuPosition);
      window.removeEventListener("scroll", updateMenuPosition, true);
    });
  });
  return (
    <div class="composer-task-row-wrap">
      <button
        type="button"
        class={classNames("composer-task-row", props.selected && "selected")}
        onClick={props.onEdit}
      >
        <span>{taskSummaryText(props.task)}</span>
        <small
          class={classNames(
            "composer-task-meta",
            remainingText() && "has-countdown",
          )}
        >
          <Show when={scheduleText()}>
            <span class="composer-task-condition">{scheduleText()}</span>
          </Show>
          <Show when={remainingText()}>
            <span class="composer-task-countdown">{remainingText()}</span>
          </Show>
        </small>
      </button>
      <button
        ref={moreButton}
        class="composer-task-more"
        type="button"
        title="更多"
        onClick={(event) => {
          event.stopPropagation();
          updateMenuPosition();
          props.onMenu();
        }}
      >
        <MoreHorizontal size={15} />
      </button>
      <Show when={props.menuOpen}>
        <Portal>
          <div
            class="composer-task-menu"
            style={{
              left: `${menuRect().left}px`,
              top: `${menuRect().top}px`,
            }}
          >
            <button type="button" onClick={props.onDelete}>
              删除
            </button>
            <button type="button" onClick={props.onCreateSession}>
              创建新会话
            </button>
          </div>
        </Portal>
      </Show>
    </div>
  );
}

function formatPollInterval(interval: PollInterval): string {
  const normalized = normalizePollInterval(interval);
  return (["d", "h", "m", "s"] as const)
    .map((part) => `${normalized[part] ?? 0}${part}`)
    .join(" ");
}

function PlanDraftSessionPicker(props: {
  sessions: Session[];
  selectedSessionId?: string;
  onSession: (value: string | undefined) => void;
}) {
  let root: HTMLElement | undefined;
  const [open, setOpen] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const selectedSession = createMemo(() =>
    props.selectedSessionId
      ? props.sessions.find((session) => session.id === props.selectedSessionId)
      : undefined,
  );
  const filteredSessions = createMemo(() => {
    const normalized = query().trim().toLowerCase();
    const sessions = props.sessions.filter(
      (session) => planSessionStatus(session) !== "archived",
    );
    if (!normalized) {
      return sessions.slice(0, 8);
    }
    return sessions
      .filter(
        (session) =>
          sessionTitle(session).toLowerCase().includes(normalized) ||
          session.id.toLowerCase().includes(normalized),
      )
      .slice(0, 8);
  });
  createEffect(() => {
    if (!open()) {
      return;
    }
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });
  return (
    <section class="plan-session-picker" ref={root}>
      <button
        type="button"
        class="plan-session-button"
        onClick={() => setOpen(!open())}
        title={
          selectedSession() ? sessionTitle(selectedSession()!) : t("newSession")
        }
      >
        <FolderOpen size={15} strokeWidth={1.8} />
        <span>
          {selectedSession()
            ? sessionTitle(selectedSession()!)
            : t("newSession")}
        </span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-session-menu">
          <label class="workspace-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={query()}
              placeholder={`${t("sessionHistory")}...`}
              onInput={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <button
            type="button"
            class={classNames(
              "workspace-pick-row",
              "session-pick-row",
              !props.selectedSessionId && "selected",
            )}
            onClick={() => {
              props.onSession(undefined);
              setOpen(false);
            }}
          >
            <Plus size={15} strokeWidth={1.7} />
            <span>{t("newSession")}</span>
            <Show when={!props.selectedSessionId}>
              <Check size={14} strokeWidth={1.8} />
            </Show>
          </button>
          <div class="workspace-picker-list plan-session-list">
            <For each={filteredSessions()}>
              {(session) => (
                <button
                  type="button"
                  class={classNames(
                    "workspace-pick-row",
                    "session-pick-row",
                    props.selectedSessionId === session.id && "selected",
                  )}
                  onClick={() => {
                    props.onSession(session.id);
                    setOpen(false);
                  }}
                  title={sessionTitle(session)}
                >
                  <FolderOpen size={15} strokeWidth={1.6} />
                  <span>{sessionTitle(session)}</span>
                  <Show when={props.selectedSessionId === session.id}>
                    <Check size={14} strokeWidth={1.8} />
                  </Show>
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </section>
  );
}

function PlanComposerControls(props: {
  startCondition: StartCondition;
  startAt: string;
  pollInterval: PollInterval;
  onStartCondition: (value: StartCondition) => void;
  onStartAt: (value: string) => void;
  onPollInterval: (value: PollInterval) => void;
}) {
  let root: HTMLElement | undefined;
  const [open, setOpen] = createSignal(false);
  const [scheduleOpen, setScheduleOpen] = createSignal(false);
  const startConditions: Array<{ id: StartCondition; label: string }> = [
    { id: "user_action", label: t("runNow") },
    { id: "session_idle", label: t("sessionIdle") },
    { id: "scheduled_task", label: t("scheduledTask") },
    { id: "polling_task", label: t("pollingTask") },
  ];
  const selectedLabel = createMemo(() => {
    return (
      startConditions.find((condition) => condition.id === props.startCondition)
        ?.label ?? t("userAction")
    );
  });
  const selectCondition = (condition: StartCondition) => {
    props.onStartCondition(condition);
    if (
      (condition === "scheduled_task" || condition === "polling_task") &&
      !props.startAt
    ) {
      props.onStartAt(defaultLocalStartAt());
    }
    if (condition === "polling_task") {
      props.onPollInterval(normalizePollInterval(props.pollInterval));
    }
    setOpen(false);
    if (condition === "scheduled_task" || condition === "polling_task") {
      setScheduleOpen(true);
    }
  };
  createEffect(() => {
    if (!open()) {
      return;
    }
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });
  return (
    <section class="plan-trigger-control" ref={root}>
      <button
        type="button"
        class="plan-trigger-button"
        onClick={() => setOpen(!open())}
      >
        <CalendarDays size={15} strokeWidth={1.8} />
        <span>{selectedLabel()}</span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-trigger-menu">
          <For each={startConditions}>
            {(condition) => (
              <button
                type="button"
                class={classNames(
                  "plan-trigger-option",
                  props.startCondition === condition.id && "selected",
                )}
                onClick={() => selectCondition(condition.id)}
              >
                <span>{condition.label}</span>
                <Show when={props.startCondition === condition.id}>
                  <Check size={14} strokeWidth={1.8} />
                </Show>
              </button>
            )}
          </For>
        </div>
      </Show>
      <Show when={scheduleOpen()}>
        <PlanScheduleDialog
          condition={props.startCondition}
          startAt={props.startAt || defaultLocalStartAt()}
          pollInterval={normalizePollInterval(props.pollInterval)}
          onCancel={() => setScheduleOpen(false)}
          onSave={(startAt, pollInterval) => {
            props.onStartAt(startAt);
            if (props.startCondition === "polling_task") {
              props.onPollInterval(normalizePollInterval(pollInterval));
            }
            setScheduleOpen(false);
          }}
        />
      </Show>
    </section>
  );
}

function PlanScheduleDialog(props: {
  condition: StartCondition;
  startAt: string;
  pollInterval: PollInterval;
  onCancel: () => void;
  onSave: (startAt: string, pollInterval: PollInterval) => void;
}) {
  const [startAt, setStartAt] = createSignal(props.startAt);
  const [interval, setInterval] = createSignal(
    normalizePollInterval(props.pollInterval),
  );
  const setIntervalPart = (part: keyof PollInterval, value: string) =>
    setInterval((previous) => ({
      ...previous,
      [part]: normalizeIntervalPart(value.replace(/\D/gu, "")),
    }));
  const blockNonNumericInput = (event: InputEvent) => {
    if (event.data && !/^\d+$/u.test(event.data)) {
      event.preventDefault();
    }
  };
  const intervalParts: Array<{
    id: keyof PollInterval;
    label: TextKey;
    maxLength: number;
  }> = [
    { id: "d", label: "intervalDay", maxLength: 3 },
    { id: "h", label: "intervalHour", maxLength: 2 },
    { id: "m", label: "intervalMinute", maxLength: 2 },
    { id: "s", label: "intervalSecond", maxLength: 2 },
  ];
  return (
    <div class="modal-scrim" onMouseDown={props.onCancel}>
      <div
        class="name-dialog plan-schedule-dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <h2>
              {props.condition === "polling_task"
                ? t("pollingTask")
                : t("scheduledTask")}
            </h2>
          </div>
          <button type="button" onClick={props.onCancel}>
            ×
          </button>
        </header>
        <label class="field-row">
          <span>{t("startTime")}</span>
          <input
            type="datetime-local"
            value={startAt()}
            onInput={(event) => setStartAt(event.currentTarget.value)}
          />
        </label>
        <Show when={props.condition === "polling_task"}>
          <div class="field-row plan-schedule-interval">
            <span>{t("pollInterval")}</span>
            <div class="plan-schedule-interval-grid">
              <For each={intervalParts}>
                {(part) => (
                  <label class={`interval-part-${part.id}`}>
                    <input
                      type="text"
                      inputmode="numeric"
                      pattern="[0-9]*"
                      maxlength={part.maxLength}
                      value={String(interval()[part.id] ?? 0)}
                      onBeforeInput={blockNonNumericInput}
                      onInput={(event) => {
                        const value = event.currentTarget.value
                          .replace(/\D/gu, "")
                          .slice(0, part.maxLength);
                        event.currentTarget.value = value;
                        setIntervalPart(part.id, value);
                      }}
                    />
                    <span>{t(part.label)}</span>
                  </label>
                )}
              </For>
            </div>
          </div>
        </Show>
        <footer>
          <button type="button" class="secondary" onClick={props.onCancel}>
            {t("cancel")}
          </button>
          <button
            type="button"
            class="primary"
            disabled={!startAt()}
            onClick={() => props.onSave(startAt(), interval())}
          >
            {t("save")}
          </button>
        </footer>
      </div>
    </div>
  );
}

function taskStateLabel(status: PlanStatus): string {
  switch (status) {
    case "doing":
      return t("doing");
    case "question":
      return t("question");
    case "done":
      return t("done");
    case "archived":
      return t("archived");
    case "todo":
    default:
      return t("todo");
  }
}

function sessionTaskState(session: Session) {
  return session.task_management ?? {};
}

function sessionTasks(session: Session): TaskManagement[] {
  const task = sessionTaskState(session);
  if (Array.isArray(task.tasks) && task.tasks.length > 0) {
    return task.tasks;
  }
  return [task];
}

function sortedSessionTasks(session: Session): TaskManagement[] {
  const visible = sessionTasks(session).filter(
    (task) =>
      taskPlanStatus(task) !== "archived" && taskHasVisibleContent(task),
  );
  const queued = visible
    .filter((task) => !isTimedStartCondition(taskStartCondition(task)))
    .sort(compareTaskStep);
  const timed = visible
    .filter((task) => isTimedStartCondition(taskStartCondition(task)))
    .sort((left, right) => {
      const leftTime = new Date(taskStartAt(left) ?? 0).getTime();
      const rightTime = new Date(taskStartAt(right) ?? 0).getTime();
      return leftTime - rightTime || compareTaskStep(left, right);
    });
  return [...queued, ...timed];
}

function hasVisibleSessionTasks(session: Session): boolean {
  return sessionTasks(session).some(
    (task) =>
      taskPlanStatus(task) !== "archived" && taskHasVisibleContent(task),
  );
}

function taskHasVisibleContent(task: TaskManagement): boolean {
  return taskDisplayText(task).trim().length > 0;
}

function compareTaskStep(left: TaskManagement, right: TaskManagement): number {
  const leftStep =
    typeof left.step === "number" ? left.step : Number.POSITIVE_INFINITY;
  const rightStep =
    typeof right.step === "number" ? right.step : Number.POSITIVE_INFINITY;
  return leftStep - rightStep;
}

function taskNonceId(task: TaskManagement): string | undefined {
  return task.nonce_id;
}

function taskPlanStatus(task: TaskManagement): PlanStatus | undefined {
  return task.status;
}

function taskStartCondition(task: TaskManagement): StartCondition {
  if (hasPollInterval(task.poll_interval)) {
    return "polling_task";
  }
  return task.start_at ? "scheduled_task" : "user_action";
}

function hasPollInterval(value: PollInterval | undefined): boolean {
  return Boolean(value && (value.m || value.d || value.h || value.s));
}

function taskStartAt(task: TaskManagement): string | number | undefined {
  return task.start_at;
}

function taskPollInterval(task: TaskManagement): PollInterval {
  return task.poll_interval ?? defaultPollInterval();
}

function taskDisplayText(task: TaskManagement): string {
  const summary = (task.task_summary ?? "").trim();
  const delivery = (task.delivery ?? "").trim();
  return [summary, delivery].filter(Boolean).join("\n\n");
}

function firstRunnableTask(session: Session): TaskManagement | undefined {
  return sortedSessionTasks(session).find((task) =>
    taskDisplayText(task).trim(),
  );
}

function taskSummaryText(task: TaskManagement): string {
  return (
    (
      task.task_summary ??
      task.delivery ??
      ""
    )
      .trim()
      .split(/\r?\n/u)[0]
      ?.trim() ?? ""
  );
}

function formatTaskRemaining(task: TaskManagement): string {
  const condition = taskStartCondition(task);
  if (!isTimedStartCondition(condition)) {
    return "";
  }
  const startAt =
    condition === "polling_task"
      ? nextPollingTime(taskStartAt(task), taskPollInterval(task))
      : taskStartAt(task);
  if (!startAt) {
    return "";
  }
  const target = new Date(startAt).getTime();
  if (Number.isNaN(target)) {
    return "";
  }
  const seconds = Math.max(0, Math.ceil((target - Date.now()) / 1000));
  if (seconds >= 86_400) {
    return `${Math.ceil(seconds / 86_400)}${t("intervalDay")}`;
  }
  if (seconds >= 3_600) {
    return `${Math.ceil(seconds / 3_600)}${t("intervalHour")}`;
  }
  if (seconds >= 60) {
    return `${Math.ceil(seconds / 60)}${t("intervalMinute")}`;
  }
  return `${seconds}${t("intervalSecond")}`;
}

function applyTaskPatchToSession(
  session: Session,
  patch: Partial<TaskManagement>,
): Session {
  const current = sessionTaskState(session);
  const nonce = patch.nonce_id;
  if (Array.isArray(current.tasks) || nonce) {
    const tasks = sessionTasks(session);
    const index = nonce
      ? tasks.findIndex((task) => taskNonceId(task) === nonce)
      : -1;
    const nextTasks =
      index >= 0
        ? tasks.map((task, itemIndex) =>
            itemIndex === index ? { ...task, ...patch } : task,
          )
        : [
            ...tasks,
            { ...patch, nonce_id: nonce ?? `${session.id}:${tasks.length}` },
          ];
    const nextManagement = { ...current, tasks: nextTasks };
    return {
      ...session,
      task_management: nextManagement,
    };
  }
  const nextManagement = { ...current, ...patch };
  return {
    ...session,
    task_management: nextManagement,
  };
}

function planSessionStatus(session: Session): PlanStatus {
  const task = sessionTaskState(session);
  const status = task.status;
  if (
    status === "archived" ||
    status === "done" ||
    status === "question" ||
    status === "doing"
  ) {
    return status;
  }
  if (session.status === "busy") {
    return "doing";
  }
  if (status === "todo") {
    return "todo";
  }
  return "todo";
}

function sessionAttentionKey(session: Session): string | undefined {
  const status = planSessionStatus(session);
  if (status !== "doing" && status !== "question" && status !== "done") {
    return undefined;
  }
  return `${session.id}:${status}:${normalizeTimeMs(sessionUpdatedAt(session) ?? 0)}`;
}

function planStoredPlanStatus(session: Session): PlanStatus | undefined {
  const task = sessionTaskState(session);
  const status = task.status;
  if (
    status === "todo" ||
    status === "doing" ||
    status === "question" ||
    status === "done" ||
    status === "archived"
  ) {
    return status;
  }
  return undefined;
}

type PlanCalendarMode = "month" | "week" | "day";

function planSessionStartCondition(
  session: Session,
): StartCondition | undefined {
  const task = sessionTaskState(session);
  return taskStartCondition(task);
}

function planTimedSessions(sessions: Session[]): Session[] {
  return sessions.filter((session) => {
    if (
      planStoredPlanStatus(session) !== "todo" &&
      timedSessionTasks(session).length === 0
    ) {
      return false;
    }
    const condition = planSessionStartCondition(session);
    return (
      (Boolean(planSessionDate(session)) &&
        (condition === "scheduled_task" || condition === "polling_task")) ||
      timedSessionTasks(session).length > 0
    );
  });
}

function timedSessionTasks(session: Session): TaskManagement[] {
  return sortedSessionTasks(session).filter(
    (task) =>
      (taskPlanStatus(task) ?? planStoredPlanStatus(session)) === "todo" &&
      isTimedStartCondition(taskStartCondition(task)) &&
      Boolean(taskStartAt(task)),
  );
}

function planTriggerClass(session: Session): string {
  const condition = planSessionStartCondition(session);
  return condition ? `trigger-${condition}` : "";
}

function planTaskTitle(session: Session): string {
  const task = sessionTaskState(session);
  const title = task.task_summary ?? sessionTitle(session);
  return title.replace(/^执行(?:状态|任务)：/u, "");
}

function planInitialCalendarDate(sessions: Session[]): Date {
  return sessions.map(planSessionDate).find(Boolean) ?? new Date();
}

function shortSessionId(sessionId: string): string {
  return sessionId.slice(0, 8);
}

function localDateTimeToUtcIso(value: string): string | undefined {
  if (!value) {
    return undefined;
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
}

function utcIsoToLocalDateTime(value: string | number | undefined): string {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

function defaultLocalStartAt(): string {
  const date = new Date(Date.now() + 60 * 60_000);
  date.setSeconds(0, 0);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

function defaultPollInterval(): PollInterval {
  return { m: 0, d: 0, h: 1, s: 0 };
}

function normalizeIntervalPart(value: string | number | undefined): number {
  const parsed = Number(value ?? 0);
  return Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) : 0;
}

function normalizePollInterval(value: PollInterval | undefined): PollInterval {
  const source = value ?? defaultPollInterval();
  const normalized = {
    m: normalizeIntervalPart(source.m),
    d: normalizeIntervalPart(source.d),
    h: normalizeIntervalPart(source.h),
    s: normalizeIntervalPart(source.s),
  };
  return normalized.m || normalized.d || normalized.h || normalized.s
    ? normalized
    : defaultPollInterval();
}

function timedTaskPatch(
  startCondition: StartCondition,
  startAt: string | undefined,
  pollInterval: PollInterval | undefined,
): {
  start_at?: string;
  poll_interval?: PollInterval;
} {
  return {
    ...(startCondition === "scheduled_task" || startCondition === "polling_task"
      ? startAt
        ? { start_at: startAt }
        : {}
      : {}),
    ...(startCondition === "polling_task"
      ? { poll_interval: normalizePollInterval(pollInterval) }
      : {}),
  };
}

function formatTicketTime(value: string | number | undefined): string {
  if (!value) {
    return t("notScheduled");
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return t("notScheduled");
  }
  return date.toLocaleString();
}

function formatCalendarEventTime(value: string | number | undefined): string {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return date.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatStartCondition(value: StartCondition | undefined): string {
  switch (value) {
    case "session_idle":
      return t("sessionIdle");
    case "scheduled_task":
      return t("scheduledTask");
    case "polling_task":
      return t("pollingTask");
    case "user_action":
    default:
      return t("userAction");
  }
}

function isTimedStartCondition(
  value: StartCondition | undefined,
): value is "scheduled_task" | "polling_task" {
  return value === "scheduled_task" || value === "polling_task";
}

function materializeComposerContent(
  text: string,
  images: ComposerImage[],
): string {
  const seen = new Set<string>();
  let index = 0;
  let content = text;
  for (const image of images) {
    const isImage = (image.kind ?? "image") === "image";
    const token = isImage
      ? composerImageToken(image.id)
      : composerFileToken(image.id);
    if (!content.includes(token)) {
      continue;
    }
    seen.add(image.id);
    index += 1;
    content = content.replaceAll(
      token,
      isImage
        ? `\n[Image ${index}: ${image.name}]\n[MEDIA:${image.dataUrl}:MEDIA]\n`
        : `\n[File ${index}: ${image.name}]\n`,
    );
  }
  const trailing = images.filter((image) => !seen.has(image.id));
  if (trailing.length > 0) {
    const appendix = trailing
      .map((image) => {
        const isImage = (image.kind ?? "image") === "image";
        index += 1;
        return isImage
          ? `[Image ${index}: ${image.name}]\n[MEDIA:${image.dataUrl}:MEDIA]`
          : `[File ${index}: ${image.name}]`;
      })
      .join("\n\n");
    content = `${content.trim()}\n\n${appendix}`;
  }
  return content.trim();
}

function FileBrowserView(props: {
  path: string;
  directory?: string;
  files: FileInfo[];
  selectedFile?: FileInfo;
  fileContent?: {
    type: string;
    content: string;
    encoding?: string | null;
    mimeType?: string | null;
  };
  onFile: (file: FileInfo) => void;
  onUp: () => void;
  onList: () => void;
  onOpenExternal: () => void;
}) {
  return (
    <section class="files-view">
      <header class="page-head">
        <div class="page-title">
          <span>{t("explorer")}</span>
          <h1>
            {shortPathLabel(props.path) ?? shortWorkspaceLabel(props.directory)}
          </h1>
        </div>
        <div class="page-actions">
          <button
            class={classNames("icon-action", !props.selectedFile && "selected")}
            title={t("list")}
            onClick={props.onList}
          >
            <LayoutList size={17} />
          </button>
          <button
            class="icon-action"
            title={t("open")}
            disabled={!props.selectedFile}
            onClick={props.onOpenExternal}
          >
            <ExternalLink size={17} />
          </button>
        </div>
      </header>
      <main class="file-canvas">
        <Show
          when={props.selectedFile}
          fallback={
            <FileListView
              files={props.files}
              path={props.path}
              selectedFile={props.selectedFile}
              onFile={props.onFile}
              onUp={props.onUp}
            />
          }
        >
          {(file) => <FilePreview file={file()} content={props.fileContent} />}
        </Show>
      </main>
    </section>
  );
}

function FileListView(props: {
  files: FileInfo[];
  path: string;
  selectedFile?: FileInfo;
  onFile: (file: FileInfo) => void;
  onUp: () => void;
}) {
  return (
    <section class="surface-list-panel">
      <div class="surface-list-head file-list-head">
        <span>{t("name")}</span>
        <span>{t("git")}</span>
        <span>{t("size")}</span>
        <span>{t("modifiedAt")}</span>
      </div>
      <Show when={props.path}>
        <button class="surface-list-row file-list-row" onClick={props.onUp}>
          <span>..</span>
          <small>{t("parent")}</small>
          <small>--</small>
          <small>{parentPath(props.path) || "/"}</small>
        </button>
      </Show>
      <For
        each={props.files}
        fallback={<div class="surface-list-empty">{t("empty")}</div>}
      >
        {(file) => (
          <button
            class={classNames(
              "surface-list-row file-list-row",
              props.selectedFile?.path === file.path && "selected",
            )}
            onClick={() => props.onFile(file)}
            title={file.path}
          >
            <span>
              {file.type === "directory" ? `${file.name}/` : file.name}
            </span>
            <small>{fileGitRemark(file)}</small>
            <small>{formatFileSize(file)}</small>
            <small>{formatModifiedTime(file.modified_at)}</small>
          </button>
        )}
      </For>
    </section>
  );
}

function FilePreview(props: {
  file?: FileInfo;
  content?: {
    type: string;
    content: string;
    encoding?: string | null;
    mimeType?: string | null;
  };
}) {
  const mediaSource = createMemo(() =>
    props.content?.encoding === "base64" && props.content.mimeType
      ? `data:${props.content.mimeType};base64,${props.content.content}`
      : undefined,
  );
  return (
    <section class="surface-preview-panel">
      <Show
        when={props.file}
        fallback={<div class="empty-type">{t("selectStep")}</div>}
      >
        {(file) => (
          <>
            <header>
              <span>{shortPathLabel(file().path)}</span>
              <small>
                {props.content?.mimeType ?? props.content?.type ?? file().type}
              </small>
            </header>
            <Switch fallback={<div class="binary-note">{t("empty")}</div>}>
              <Match when={props.content?.type === "text"}>
                <pre>{props.content?.content}</pre>
              </Match>
              <Match
                when={
                  props.content?.type === "media" &&
                  props.content?.mimeType?.startsWith("image/")
                }
              >
                <div class="media-preview">
                  <img src={mediaSource()} alt={file().name} />
                </div>
              </Match>
              <Match
                when={
                  props.content?.type === "media" &&
                  props.content?.mimeType?.startsWith("video/")
                }
              >
                <div class="media-preview">
                  <video src={mediaSource()} controls />
                </div>
              </Match>
              <Match
                when={
                  props.content?.type === "media" &&
                  props.content?.mimeType === "application/pdf"
                }
              >
                <iframe
                  class="pdf-preview"
                  src={mediaSource()}
                  title={file().name}
                />
              </Match>
              <Match
                when={
                  props.content?.type === "media" &&
                  props.content?.mimeType?.startsWith("audio/")
                }
              >
                <div class="media-preview">
                  <audio src={mediaSource()} controls />
                </div>
              </Match>
            </Switch>
          </>
        )}
      </Show>
    </section>
  );
}

async function safe<T>(run: () => Promise<T>, fallback: T): Promise<T> {
  try {
    return await run();
  } catch {
    return fallback;
  }
}

function defaultModel(providers: AppState["providers"]): string | undefined {
  if (!providers) {
    return "openai/gpt-5.5";
  }
  if (
    providers.all.some(
      (provider) => provider.id === "openai" && provider.models["gpt-5.5"],
    )
  ) {
    return "openai/gpt-5.5";
  }
  const firstConnected = providers.connected[0];
  if (firstConnected && providers.default[firstConnected]) {
    return `${firstConnected}/${providers.default[firstConnected]}`;
  }
  const firstProvider = providers.all[0];
  const firstModel = firstProvider
    ? Object.keys(firstProvider.models)[0]
    : undefined;
  return firstProvider && firstModel
    ? `${firstProvider.id}/${firstModel}`
    : undefined;
}

function settingsSections(): Array<{ id: SettingsSection; label: string }> {
  return [
    { id: "general", label: t("general") },
    { id: "appearance", label: t("appearance") },
    { id: "providers", label: t("providers") },
    { id: "models", label: t("models") },
    { id: "auth", label: t("login") },
    { id: "runtime", label: t("runtime") },
    { id: "config", label: t("turaConfig") },
    { id: "workspace", label: t("workspaceConfig") },
    { id: "environment", label: t("environment") },
  ];
}

function configFieldRows(
  state: AppState,
): Array<{ key: string; label: string }> {
  const keys = new Set([
    "language",
    "theme",
    "model",
    "agent",
    "skill_folders",
    ...Object.keys(state.configDraft),
  ]);
  return [...keys].map((key) => ({
    key,
    label: configFieldLabel(key),
  }));
}

function configFieldLabel(key: string): string {
  const labels: Record<string, TextKey> = {
    agent: "agent",
    language: "language",
    model: "model",
    skill_folders: "skillFolders",
    theme: "theme",
  };
  return labels[key] ? t(labels[key]) : key.replaceAll("_", " ");
}

function configToDraft(config: AppState["config"]): Record<string, string> {
  if (!config) {
    return {};
  }
  return {
    language: config.language ?? "",
    theme: config.theme ?? "",
    model: config.model ?? "",
    agent: config.agent ?? "",
    skill_folders: (config.skill_folders ?? []).join(", "),
  };
}

function configDraftToPatch(
  draft: Record<string, string>,
  themeMode: ThemeMode,
): Partial<NonNullable<AppState["config"]>> {
  return {
    language: draft.language || null,
    theme: themeMode,
    model: draft.model || null,
    agent: draft.agent || null,
    skill_folders: draft.skill_folders
      ? draft.skill_folders
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean)
      : [],
  };
}

function recordToDraft(
  record: Record<string, unknown>,
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(record).map(([key, value]) => [key, draftValue(value)]),
  );
}

function draftToRecord(draft: Record<string, string>): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(draft).map(([key, value]) => [key, parseDraftValue(value)]),
  );
}

function draftValue(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

function parseDraftValue(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  if (trimmed === "true") {
    return true;
  }
  if (trimmed === "false") {
    return false;
  }
  if (/^-?\d+(\.\d+)?$/u.test(trimmed)) {
    return Number(trimmed);
  }
  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      return JSON.parse(trimmed);
    } catch {
      return value;
    }
  }
  return value;
}

function parseModelRef(
  value?: string | null,
): { providerId: string; modelId: string } | undefined {
  if (!value) {
    return undefined;
  }
  const index = value.indexOf("/");
  if (index <= 0 || index >= value.length - 1) {
    return undefined;
  }
  return {
    providerId: value.slice(0, index),
    modelId: value.slice(index + 1),
  };
}

function providerIdFromModel(value?: string | null): string | undefined {
  return parseModelRef(value)?.providerId;
}

function modelRef(providerId?: string, modelId?: string): string {
  return providerId && modelId ? `${providerId}/${modelId}` : "";
}

function providerStateLabel(
  state: AppState,
  providerId: string,
  source: string,
): string {
  const status = state.providerAuthStatus[providerId];
  if (status?.authenticated) {
    return t("connected");
  }
  if (status?.configured) {
    return t("configured");
  }
  if (state.providers?.connected.includes(providerId)) {
    return t("connected");
  }
  return source ? providerSourceLabel(source) : t("notConfigured");
}

function providerSourceLabel(source?: string | null): string {
  const normalized = source?.toLowerCase();
  if (normalized === "config") {
    return t("config");
  }
  if (normalized === "env") {
    return t("env");
  }
  return source || t("notConfigured");
}

function authStatusText(
  status?: AppState["providerAuthStatus"][string],
): string {
  if (!status) {
    return "--";
  }
  if (status.authenticated) {
    return t("connected");
  }
  if (status.expired) {
    return t("expired");
  }
  if (status.configured) {
    return t("configured");
  }
  return t("notConfigured");
}

function formatModelLimit(value?: number): string {
  if (!value) {
    return "--";
  }
  if (value >= 1_000_000) {
    return `${Math.round(value / 1_000_000)}M`;
  }
  if (value >= 1_000) {
    return `${Math.round(value / 1_000)}K`;
  }
  return String(value);
}

function eventBelongsToState(
  state: AppState,
  directory?: string | null,
): boolean {
  if (!directory || directory === "global") {
    return true;
  }
  if (!state.directory) {
    return true;
  }
  return samePath(directory, state.directory);
}

function samePath(left?: string | null, right?: string | null): boolean {
  if (!left || !right) {
    return false;
  }
  return normalizePath(left) === normalizePath(right);
}

function normalizePath(value: string): string {
  const normalized = value.replaceAll("\\", "/").replace(/\/+$/, "");
  return /^[A-Za-z]:$/u.test(normalized)
    ? `${normalized}/`.toLowerCase()
    : normalized.toLowerCase();
}

function parentPath(path: string): string {
  const parts = path.replaceAll("\\", "/").split("/").filter(Boolean);
  parts.pop();
  return parts.join("/");
}

function shortPathLabel(path?: string | null): string | undefined {
  if (!path) {
    return undefined;
  }
  const parts = path.replaceAll("\\", "/").split("/").filter(Boolean);
  return parts.at(-1) ?? path;
}

function shortWorkspaceLabel(path?: string | null): string {
  return shortPathLabel(path) ?? t("noWorkspace");
}

function fixtureFiles(fixture: string | undefined, path = ""): FileInfo[] {
  if (fixture !== "plan-sessions") {
    return [];
  }
  const root = "C:\\Users\\liuliu\\Documents\\tura";
  const makeFile = (
    name: string,
    relativePath: string,
    type: "directory" | "file",
    size = type === "directory" ? null : 128,
  ): FileInfo => ({
    name,
    path: relativePath,
    type,
    absolute: `${root}\\${relativePath.replaceAll("/", "\\")}`,
    ignored: false,
    git_status: "clean",
    size_bytes: size,
    modified_at: Date.now() - 12_000,
  });
  const tree: Record<string, FileInfo[]> = {
    "": [
      makeFile("apps", "apps", "directory"),
      makeFile("crates", "crates", "directory"),
      makeFile("README.md", "README.md", "file"),
      makeFile("package.json", "package.json", "file"),
    ],
    apps: [
      makeFile("gui", "apps/gui", "directory"),
      makeFile("tui", "apps/tui", "directory"),
      makeFile("app.config.ts", "apps/app.config.ts", "file"),
    ],
    "apps/gui": [
      makeFile("app", "apps/gui/app", "directory"),
      makeFile("e2e", "apps/gui/e2e", "directory"),
      makeFile("package.json", "apps/gui/package.json", "file"),
    ],
    crates: [
      makeFile("gateway", "crates/gateway", "directory"),
      makeFile("runtime", "crates/runtime", "directory"),
      makeFile("Cargo.toml", "crates/Cargo.toml", "file"),
    ],
  };
  return tree[path] ?? [];
}

function shortSessionTitle(title: string): string {
  return title.length > 24 ? `${title.slice(0, 21)}...` : title;
}

function relativeSessionTime(session: Session): string {
  const updated = sessionUpdatedAt(session);
  if (!updated) {
    return "";
  }
  const delta = Math.max(0, Date.now() - normalizeTimeMs(updated));
  const minutes = Math.max(1, Math.floor(delta / 60_000));
  if (minutes < 60) {
    return `${minutes}分钟`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}小时`;
  }
  return `${Math.floor(hours / 24)}天`;
}

function sessionHoverTitle(session: Session): string {
  const schedule = sessionScheduleHoverText(session);
  return schedule
    ? `${sessionTitle(session)}\n${schedule}`
    : sessionTitle(session);
}

function sessionScheduleHoverText(session: Session): string | undefined {
  const task = sessionTaskState(session);
  const condition = taskStartCondition(task);
  if (condition === "scheduled_task") {
    return `${t("scheduledTask")}: ${formatTicketTime(taskStartAt(task))}`;
  }
  if (condition !== "polling_task") {
    return undefined;
  }
  const next = nextPollingTime(taskStartAt(task), taskPollInterval(task));
  return `${t("pollingTask")}: ${next ? formatTicketTime(next) : formatTicketTime(taskStartAt(task))}`;
}

function nextPollingTime(
  startAt: string | number | undefined,
  interval: PollInterval,
): string | undefined {
  if (!startAt) {
    return undefined;
  }
  const start = new Date(startAt).getTime();
  if (Number.isNaN(start)) {
    return undefined;
  }
  const step =
    normalizeIntervalPart(interval.d) * 86_400_000 +
    normalizeIntervalPart(interval.h) * 3_600_000 +
    normalizeIntervalPart(interval.m) * 60_000 +
    normalizeIntervalPart(interval.s) * 1_000;
  if (step <= 0) {
    return new Date(start).toISOString();
  }
  const now = Date.now();
  if (start > now) {
    return new Date(start).toISOString();
  }
  return new Date(start + Math.ceil((now - start) / step) * step).toISOString();
}

function normalizeTimeMs(value: number): number {
  return value > 10_000_000_000 ? value : value * 1000;
}

function readConfigString(
  config: Record<string, unknown>,
  key: string,
): string | undefined {
  const value = config[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function readConfigBoolean(
  config: Record<string, unknown>,
  key: string,
): boolean | undefined {
  const value = config[key];
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (["true", "1", "yes", "on"].includes(normalized)) {
      return true;
    }
    if (["false", "0", "no", "off"].includes(normalized)) {
      return false;
    }
  }
  return undefined;
}

function inputHeight(value: string): string {
  const lines = Math.min(
    12,
    Math.max(
      3,
      value.split(/\r\n|\r|\n/u).length + Math.floor(value.length / 72),
    ),
  );
  return `${lines * 24 + 36}px`;
}

function fileGitRemark(file: FileInfo): string {
  const status = file.git_status ?? (file.ignored ? "ignored" : "clean");
  switch (status) {
    case "added":
      return t("added");
    case "changed":
      return t("changed");
    case "copied":
      return t("copied");
    case "deleted":
      return t("deleted");
    case "ignored":
      return t("ignored");
    case "modified":
      return t("modified");
    case "renamed":
      return t("renamed");
    case "untracked":
      return t("untracked");
    default:
      return t("clean");
  }
}

function formatFileSize(file: FileInfo): string {
  if (file.type === "directory") {
    return "--";
  }
  const bytes = file.size_bytes;
  if (bytes === undefined || bytes === null) {
    return "--";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}

function formatModifiedTime(value?: number | null): string {
  if (!value) {
    return "--";
  }
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function readSearchParam(name: string): string | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  return new URLSearchParams(window.location.search).get(name) ?? undefined;
}

function readBooleanSearchParam(name: string): boolean {
  const value = readSearchParam(name);
  return value === "1" || value === "true" || value === "yes";
}

function readMainTabSearchParam(): MainTab | undefined {
  const tab = readSearchParam("tab");
  return tab === "plan" ||
    tab === "new" ||
    tab === "conversation" ||
    tab === "files" ||
    tab === "settings"
    ? tab
    : undefined;
}

function withInitialOverrides(
  state: AppState,
  overrides: {
    activeTab?: MainTab;
    selectedSessionId?: string;
    selectedModel?: string;
    selectedAgent?: string;
  },
): AppState {
  return {
    ...state,
    activeTab: overrides.activeTab ?? state.activeTab,
    selectedSessionId: overrides.selectedSessionId ?? state.selectedSessionId,
    selectedModel: overrides.selectedModel ?? state.selectedModel,
    selectedAgent: overrides.selectedAgent ?? state.selectedAgent,
  };
}

function fixtureAppState(gatewayUrl: string, fixture: string): AppState {
  const base = initialAppState(gatewayUrl);
  const now = Date.now();
  if (fixture === "plan-sessions") {
    const directory = "C:\\Users\\liuliu\\Documents\\tura";
    const otherDirectory = "C:\\Users\\liuliu\\Documents\\other";
    const makeSession = (
      id: string,
      title: string,
      status: PlanStatus,
      offset: number,
      startCondition: StartCondition = "user_action",
      sessionDirectory = directory,
    ): Session => ({
      id,
      name: title,
      directory: sessionDirectory,
      model: "openai/gpt-5.5",
      agent: "coding_agent",
      session_type: "coding",
      status: status === "doing" ? "busy" : "idle",
      created_at: now - offset - 12_000,
      updated_at: now - offset,
      model_variant: "low",
      model_acceleration_enabled: true,
      plan_summary: title,
      session_display_name: title,
      task_management: {
        nonce_id: `${id}:0`,
        step: 0,
        task_summary: title,
        delivery: "session ticket e2e",
        sub_session_id: "",
        start_at: new Date(now + offset).toISOString(),
        poll_interval: { m: 0, d: 0, h: 1, s: 0 },
        status: status,
      },
    });
    const sessions = [
      makeSession(
        "plan-todo-001",
        "整理发布检查清单",
        "todo",
        1_000,
        "scheduled_task",
      ),
      makeSession(
        "plan-doing-002",
        "实现拖拽状态切换",
        "doing",
        3_700_000,
        "polling_task",
      ),
      makeSession(
        "plan-question-003",
        "等待用户补充权限",
        "question",
        7_300_000,
        "scheduled_task",
      ),
      makeSession(
        "plan-done-004",
        "完成 gateway 字段回传",
        "done",
        11_200_000,
        "scheduled_task",
      ),
      makeSession(
        "plan-archived-005",
        "隐藏旧会话工单",
        "archived",
        5_000,
        "scheduled_task",
      ),
      makeSession(
        "plan-manual-007",
        "用户操作不显示在日历",
        "todo",
        9_200_000,
        "user_action",
      ),
      makeSession(
        "plan-polling-008",
        "轮询待办工单",
        "todo",
        13_200_000,
        "polling_task",
      ),
      makeSession(
        "plan-other-006",
        "其他目录里的待办",
        "todo",
        6_000,
        "user_action",
        otherDirectory,
      ),
    ];
    const fixtureMessagesBySession: Record<string, Message[]> =
      Object.fromEntries(
        sessions.map((session, index) => [
          session.id,
          [
            {
              id: `${session.id}-message-user`,
              session_id: session.id,
              role: "user" as const,
              created_at: now - 20_000 - index * 1_000,
              updated_at: now - 20_000 - index * 1_000,
              parts: [
                {
                  id: `${session.id}-message-user-part`,
                  type: "text",
                  text: `用户创建工单：${sessionTitle(session)}`,
                },
              ],
            },
            {
              id: `${session.id}-message-agent`,
              session_id: session.id,
              role: "assistant" as const,
              created_at: now - 16_000 - index * 1_000,
              updated_at: now - 16_000 - index * 1_000,
              parts: [
                {
                  id: `${session.id}-message-agent-part`,
                  type: "text",
                  text: `已载入 ${sessionTitle(session)} 的历史上下文。`,
                },
              ],
            },
          ],
        ]),
      );
    return {
      ...base,
      loading: false,
      bootstrapped: true,
      connection: "connected",
      activeTab: "plan",
      previousMainTab: "plan",
      directory,
      sessions,
      selectedSessionId: sessions[0]?.id,
      planPreviewSessionId: undefined,
      messagesBySession: fixtureMessagesBySession,
      selectedModel: "openai/gpt-5.5",
      projects: [
        { id: "fixture-project-a", name: "tura", worktree: directory },
        {
          id: "fixture-project-b",
          name: "other",
          worktree: otherDirectory,
        },
      ],
    };
  }
  const protocolFixture = fixture === "communication-protocol";
  const session: Session = {
    id: protocolFixture ? "fixture-protocol" : "fixture-snake",
    name: protocolFixture ? "Communication style protocol" : "Snake game page",
    directory: "C:\\Users\\liuliu\\Documents\\tura",
    model: "openai/gpt-5.5",
    agent: "coding_agent",
    session_type: "coding",
    status: fixture === "snake-pending" ? "busy" : "idle",
    created_at: now - 16_000,
    updated_at: now,
    model_variant: "low",
    model_acceleration_enabled: true,
  };
  const user: Message = {
    id: "fixture-user",
    session_id: session.id,
    role: "user",
    created_at: now - 16_000,
    updated_at: now - 16_000,
    parts: [
      {
        id: "fixture-user-part",
        type: "text",
        text: protocolFixture
          ? "解析 communication_style.md，并展示所有消息协议。"
          : "写一个贪吃蛇游戏页面，并检查 streaming 动画是否平滑。",
      },
    ],
  };
  const assistant: Message = {
    id: "fixture-assistant",
    session_id: session.id,
    role: "assistant",
    providerID: "openai",
    modelID: "gpt-5.5",
    cost: 0.0004,
    created_at: now - 15_000,
    updated_at: fixture === "snake-pending" ? now - 2_000 : now - 400,
    parts: [
      {
        id: "fixture-process-text",
        type: "text",
        content: protocolFixture
          ? "正在解析消息协议、工具记录和媒体排版。"
          : "正在检查棋盘布局、键盘交互和 streaming 输出稳定性。",
      },
      {
        id: "fixture-tool-shell",
        type: "tool",
        tool: "shell_command",
        callID: "call-shell",
        state: {
          status: "completed",
          title: "Create snake page scaffold",
          command: "bun create snake page",
          time: { start: now - 14_800, end: now - 11_300 },
          exit_code: 0,
          output: "created app/src/pages/snake.tsx\nExit code: 0",
        },
      },
      {
        id: "fixture-tool-patch",
        type: "tool",
        tool: "apply_patch",
        callID: "call-patch",
        state: {
          status: fixture === "snake-pending" ? "running" : "completed",
          title: "Patch game loop and controls",
          command: "apply_patch app/src/pages/snake.tsx",
          time: {
            start: now - 10_900,
            end: fixture === "snake-pending" ? undefined : now - 5_500,
          },
          output:
            "diff --git a/app/src/pages/snake.tsx b/app/src/pages/snake.tsx\n" +
            "-const speed = 120\n" +
            "+const speed = 96\n" +
            "-return <div>Snake</div>\n" +
            "+return <SnakeBoard cells={cells} score={score} />\n",
        },
      },
      {
        id: "fixture-process-check",
        type: "text",
        content: protocolFixture
          ? "正在校验格式、图片和命令展开范围。"
          : "正在运行截图检查，并继续观察控制台 streaming 输出。",
      },
      {
        id: "fixture-tool-test",
        type: "tool",
        tool: "browser",
        callID: "call-browser",
        state: {
          status: "completed",
          title: "Screenshot and motion check",
          command: "browser screenshot localhost snake page",
          time: { start: now - 5_200, end: now - 1_200 },
          exit_code: 0,
          output:
            "3 screenshots captured\nstreaming text remained stable\nno overlap detected",
        },
      },
      {
        id: "fixture-tool-format",
        type: "tool",
        tool: "format_check",
        callID: "call-format",
        state: {
          status: "error",
          title: "Format check guard",
          command: "bun run format:check",
          time: { start: now - 1_100, end: now - 700 },
          exit_code: 1,
          error: "prettier found a spacing issue in fixture only",
        },
      },
      {
        id: "fixture-tool-stream",
        type: "tool",
        tool: "command_run",
        callID: "call-stream",
        state: {
          status: "in_progress",
          title: "Streaming command output",
          command: "powershell -NoProfile -Command Write-Output streaming",
          time: { start: now - 600 },
          exit_code: undefined,
          output: "stream chunk 1\nstream chunk 2\nwaiting for final chunk",
        },
      },
      {
        id: "fixture-summary",
        type: "text",
        text:
          fixture === "snake-pending"
            ? ""
            : protocolFixture
              ? "<b>Bold</b>\n<i>Italic</i>\n<u>Underline</u>\n<s>Strike</s>\n<a href='https://example.com'>Search Link</a>\nInline <code>code_snippet</code>\n<span class='tg-spoiler'>Hidden Text</span>\n<blockquote>Cited text or summary</blockquote>\n<pre><code class='language-python'>print('hello')</code></pre>\n[MEDIA:/assets/conversation-avatar.png:MEDIA]\n[MEDIA:/assets/conversation-avatar.png:MEDIA]\n[MEDIA:/assets/conversation-avatar.png:MEDIA]\n[MEDIA:/assets/conversation-avatar.png:MEDIA]\n[EMOJI:sticker:😂:EMOJI]\n[EMOJI:react:👍:EMOJI]\nProtocol fixture complete."
              : "Snake 页面已经完成。棋盘、键盘控制、分数反馈和失败重开都在同一套极简布局里；streaming 输出保持稳定，没有挤压工具列表或输入框。",
      },
    ],
  };
  const reaction: Message = {
    id: "fixture-reaction",
    sessionID: session.id,
    role: "assistant",
    providerID: "openai",
    modelID: "gpt-5.5",
    created_at: now - 1_350,
    updated_at: now - 1_350,
    time: { created: now - 1_350, updated: now - 1_350 },
    parts: [
      {
        id: "fixture-reaction-part",
        type: "text",
        text: "[EMOJI:react:👍:EMOJI]",
      },
    ],
  };
  return {
    ...base,
    loading: false,
    bootstrapped: true,
    connection: "connected",
    activeTab: "conversation",
    directory: session.directory ?? undefined,
    selectedSessionId: session.id,
    sessions: [session],
    messagesBySession: {
      [session.id]: protocolFixture
        ? [user, reaction, assistant]
        : [user, assistant],
    },
    selectedModel: "openai/gpt-5.5",
    modelVariant: "low",
    accelerationEnabled: true,
    projects: [
      {
        id: "fixture-project",
        name: "tura",
        worktree: session.directory ?? "",
      },
    ],
  };
}
