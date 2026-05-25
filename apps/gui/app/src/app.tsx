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
} from "solid-js";
import ExternalLink from "lucide-solid/icons/external-link";
import LayoutList from "lucide-solid/icons/layout-list";
import ArrowLeft from "lucide-solid/icons/arrow-left";
import ArrowUp from "lucide-solid/icons/arrow-up";
import Check from "lucide-solid/icons/check";
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
  type FileInfo,
  type Message,
  type ProviderAuthMethod,
  type ProductIssue,
  type ProductIssueStatus,
  type Project,
  type SdkProvider,
  type Session,
} from "@tura/gateway-sdk";
import { ConversationView } from "./conversation/conversation-view";
import { applyGatewayEvent } from "./state/event-reducer";
import {
  activeSession,
  initialAppState,
  type MainTab,
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
  const [state, setState] = createSignal<AppState>(
    withInitialOverrides(
      e2eFixture
        ? fixtureAppState(defaultGatewayUrl(), e2eFixture)
        : initialAppState(defaultGatewayUrl()),
      {
        activeTab: initialTab,
        selectedSessionId: initialSessionId,
        selectedModel: initialModel,
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
    setState((previous) => ({
      ...previous,
      selectedSessionId: sessionId,
      error: undefined,
    }));
    const client = directoryClient();
    const [messages] = await Promise.all([
      safe(() => client.messages(sessionId), []),
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
        title: cleanTitle,
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

  async function submitPrompt() {
    const raw = state().composerText.trim();
    if (!raw || state().submitting) {
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
      const content = await expandCommand(raw);
      await directoryClient().promptAsync(sessionId, {
        parts: [{ type: "text", text: content }],
        model: state().selectedModel,
        variant: state().modelVariant,
        model_acceleration_enabled: state().accelerationEnabled,
      });
      setState((previous) => ({
        ...previous,
        composerText: "",
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
    return {
      directory: state().directory,
      model: state().selectedModel,
      agent: state().selectedAgent,
      model_variant: state().modelVariant,
      model_acceleration_enabled: state().accelerationEnabled,
      disable_permission_restrictions: disablePermissionRestrictions,
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
    const files = await safe(() => directoryClient().files(path), []);
    setState((previous) => ({
      ...previous,
      files,
      filePath: path,
      selectedFile: undefined,
      fileContent: undefined,
    }));
  }

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
                onChange={(activeTab) => {
                  setState((previous) => ({
                    ...previous,
                    activeTab,
                    previousMainTab: activeTab,
                    selectedSessionId:
                      activeTab === "new"
                        ? undefined
                        : previous.selectedSessionId,
                  }));
                  if (activeTab === "files" && state().files.length === 0) {
                    void loadFiles("");
                  }
                }}
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
                selectedFile={state().selectedFile}
                expandedWorkspace={expandedWorkspace()}
                expandedGroup={expandedRailGroup()}
                onWorkspace={toggleWorkspace}
                onBlankSession={openBlankSession}
                onGroup={toggleRailGroup}
                onIssue={openIssueConversation}
                onSession={openSession}
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
              onWorkspace={useWorkspaceDirectory}
              onDefaultWorkspace={useDefaultWorkspace}
              onCreateWorkspace={createNamedWorkspace}
              onComposerText={(composerText) =>
                setState((previous) => ({ ...previous, composerText }))
              }
              onSubmit={submitPrompt}
            />
          </Match>
          <Match when={state().activeTab === "plan"}>
            <PlanView
              state={state()}
              onIssueDraft={(issueDraft) =>
                setState((previous) => ({ ...previous, issueDraft }))
              }
              onIssueSearch={(issueSearch) =>
                setState((previous) => ({ ...previous, issueSearch }))
              }
              onCreateIssue={createIssue}
              onOpenConversation={openIssueConversation}
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
              onSubmit={submitPrompt}
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
  selectedFile?: FileInfo;
  expandedWorkspace?: string;
  expandedGroup?: string;
  onWorkspace: (project: Project) => void;
  onBlankSession: () => void;
  onGroup: (id: string) => void;
  onIssue: (issue: ProductIssue) => void;
  onSession: (sessionId: string) => void;
  onRenameSession: (sessionId: string, title: string) => void;
  onFile: (file: FileInfo) => void;
  onUp: () => void;
  onSettings: () => void;
}) {
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
    props.projects.length > 0
      ? props.projects.slice(0, 6)
      : fallbackProject()
        ? [fallbackProject()!]
        : [],
  );

  return (
    <div class="workspace-tree">
      <div class="section-title">{t("workspace")}</div>
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
                <span>
                  {project.name || shortWorkspaceLabel(project.worktree)}
                </span>
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
                sessions={props.sessions}
                selectedSessionId={props.selectedSessionId}
                productIssues={props.productIssues}
                filePath={props.filePath}
                files={props.files}
                selectedFile={props.selectedFile}
                onIssue={props.onIssue}
                onGroup={props.onGroup}
                onSession={props.onSession}
                onRenameSession={props.onRenameSession}
                onFile={props.onFile}
                onUp={props.onUp}
              />
            </Show>
          </div>
        )}
      </For>
    </div>
  );
}

function WorkspaceChildren(props: {
  activeTab: MainTab;
  expandedGroup?: string;
  sessions: Session[];
  selectedSessionId?: string;
  productIssues: ProductIssue[];
  filePath: string;
  files: FileInfo[];
  selectedFile?: FileInfo;
  onIssue: (issue: ProductIssue) => void;
  onGroup: (id: string) => void;
  onSession: (sessionId: string) => void;
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
  const fileDirectories = createMemo(() =>
    props.files.filter((file) => file.type === "directory"),
  );
  const statuses: Array<{ id: ProductIssueStatus; label: string }> = [
    { id: "todo", label: t("todo") },
    { id: "in_progress", label: t("doing") },
    { id: "review", label: t("review") },
    { id: "done", label: t("done") },
  ];
  return (
    <div class="workspace-children">
      <Switch>
        <Match when={props.activeTab === "plan"}>
          <For each={statuses}>
            {(status) => {
              const issues = createMemo(() =>
                props.productIssues.filter(
                  (issue) => issue.status === status.id,
                ),
              );
              return (
                <div class="tree-group">
                  <button
                    class="child-row tree-toggle"
                    style={{ "--depth": 1 }}
                    onClick={() => props.onGroup(`plan:${status.id}`)}
                  >
                    {status.label}
                  </button>
                  <Show when={props.expandedGroup === `plan:${status.id}`}>
                    <For each={issues()}>
                      {(issue) => (
                        <button
                          class="child-row"
                          style={{ "--depth": 2 }}
                          onClick={() => props.onIssue(issue)}
                          title={issue.title}
                        >
                          {truncate(issue.title, 26)}
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
                onClick={() => props.onSession(session.id)}
                title={sessionTitle(session)}
              >
                <span>{shortSessionTitle(sessionTitle(session))}</span>
                <small>{relativeSessionTime(session)}</small>
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
          <Show when={props.filePath}>
            <button
              class="child-row"
              style={{ "--depth": 1 }}
              onClick={props.onUp}
            >
              ..
            </button>
          </Show>
          <For
            each={fileDirectories()}
            fallback={<div class="rail-empty">{t("empty")}</div>}
          >
            {(file) => (
              <button
                class={classNames(
                  "child-row",
                  file.type === "directory" && "tree-folder",
                  props.selectedFile?.path === file.path && "selected",
                )}
                style={{ "--depth": 1 }}
                onClick={() => props.onFile(file)}
                title={file.path}
              >
                <FileTreeLabel file={file} />
              </button>
            )}
          </For>
        </Match>
      </Switch>
    </div>
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

function NewSessionView(props: {
  state: AppState;
  onWorkspace: (directory: string) => void;
  onDefaultWorkspace: () => void;
  onCreateWorkspace: (name: string) => void;
  onComposerText: (value: string) => void;
  onSubmit: () => void;
}) {
  const [naming, setNaming] = createSignal(false);
  const [query, setQuery] = createSignal("");
  let directoryInput: HTMLInputElement | undefined;
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
        }
        return;
      } catch {
        return;
      }
    }
    directoryInput?.click();
  }

  return (
    <section class="new-session-view">
      <div class="new-session-center">
        <h1>{t("todayQuestion")}</h1>
        <div class="workspace-picker">
          <div class="workspace-picker-label">{t("chooseWorkspace")}</div>
          <label class="workspace-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={query()}
              placeholder={`${t("projectSearch")}...`}
              onInput={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <div class="workspace-picker-list">
            <For each={projects()}>
              {(project) => (
                <button
                  type="button"
                  class="workspace-pick-row"
                  onClick={() => props.onWorkspace(project.worktree)}
                  title={project.worktree}
                >
                  <FolderOpen size={15} strokeWidth={1.6} />
                  <span>
                    {project.name || shortWorkspaceLabel(project.worktree)}
                  </span>
                  <Show
                    when={samePath(project.worktree, props.state.directory)}
                  >
                    <Check size={14} strokeWidth={1.8} />
                  </Show>
                </button>
              )}
            </For>
          </div>
          <div class="workspace-picker-actions">
            <button type="button" onClick={() => setNaming(true)}>
              <span>{t("createWorkspace")}</span>
            </button>
            <button type="button" onClick={pickDirectory}>
              <span>{t("existingDirectory")}</span>
            </button>
            <button type="button" onClick={props.onDefaultWorkspace}>
              <span>{t("defaultWorkspace")}</span>
            </button>
          </div>
          <input
            class="composer-file-input"
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
              }
            }}
          />
        </div>
        <div class="bottom-composer composer new-session-composer">
          <div class="composer-input">
            <textarea
              value={props.state.composerText}
              placeholder={t("writeMessage")}
              onInput={(event) =>
                props.onComposerText(event.currentTarget.value)
              }
              onKeyDown={(event) => {
                if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                  event.preventDefault();
                  props.onSubmit();
                }
              }}
            />
            <div class="composer-toolbar">
              <button class="composer-attach" type="button">
                <Plus size={20} strokeWidth={1.6} />
              </button>
              <span />
              <button
                class="composer-send"
                type="button"
                disabled={!props.state.composerText.trim()}
                onClick={props.onSubmit}
              >
                <ArrowUp size={16} strokeWidth={1.8} />
              </button>
            </div>
          </div>
        </div>
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

function FileTreeLabel(props: { file: FileInfo }) {
  return (
    <Show
      when={props.file.type === "directory"}
      fallback={<span>{props.file.name}</span>}
    >
      <span>{`${props.file.name}/`}</span>
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
  onIssueDraft: (value: string) => void;
  onIssueSearch: (value: string) => void;
  onCreateIssue: () => void;
  onOpenConversation: (issue: ProductIssue) => void;
}) {
  const visibleIssues = createMemo(() => {
    const query = props.state.issueSearch.trim().toLowerCase();
    if (!query) {
      return props.state.productIssues;
    }
    return props.state.productIssues.filter(
      (issue) =>
        issue.title.toLowerCase().includes(query) ||
        issue.description.toLowerCase().includes(query),
    );
  });
  return (
    <section class="product-workbench">
      <header class="page-head plan-head">
        <div class="page-title">
          <span>{t("plan")}</span>
          <h1>{shortWorkspaceLabel(props.state.directory)}</h1>
        </div>
        <div class="page-actions">
          <label class="search-box">
            <input
              value={props.state.issueSearch}
              onInput={(event) =>
                props.onIssueSearch(event.currentTarget.value)
              }
              placeholder={t("search")}
            />
          </label>
        </div>
      </header>

      <main class="plan-board">
        <PlanBoard
          issues={visibleIssues()}
          onOpenConversation={props.onOpenConversation}
        />
      </main>

      <div class="bottom-composer quick-create">
        <textarea
          value={props.state.issueDraft}
          style={{ height: inputHeight(props.state.issueDraft) }}
          onInput={(event) => props.onIssueDraft(event.currentTarget.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault();
              void props.onCreateIssue();
            }
          }}
          placeholder={t("newIssue")}
        />
        <div class="composer-actions">
          <button class="primary" onClick={props.onCreateIssue}>
            {t("create")}
          </button>
        </div>
      </div>
    </section>
  );
}

function PlanBoard(props: {
  issues: ProductIssue[];
  onOpenConversation: (issue: ProductIssue) => void;
}) {
  const columns: Array<{ id: ProductIssueStatus; label: string }> = [
    { id: "todo", label: t("todo") },
    { id: "in_progress", label: t("doing") },
    { id: "review", label: t("review") },
    { id: "done", label: t("done") },
  ];
  return (
    <section class="board-grid">
      <For each={columns}>
        {(column) => {
          const issues = createMemo(() =>
            props.issues.filter((issue) => issue.status === column.id),
          );
          return (
            <section class="board-column">
              <header>
                <span>{column.label}</span>
                <small>{issues().length}</small>
              </header>
              <div class="board-cards">
                <For each={issues()}>
                  {(issue) => (
                    <article
                      class="board-card"
                      onClick={() => props.onOpenConversation(issue)}
                      title={issue.title}
                    >
                      <small>#{issue.number}</small>
                      <strong>{issue.title}</strong>
                      <button
                        class="ticket-session"
                        onClick={(event) => {
                          event.stopPropagation();
                          props.onOpenConversation(issue);
                        }}
                      >
                        {t("conversation")}
                      </button>
                    </article>
                  )}
                </For>
              </div>
            </section>
          );
        }}
      </For>
    </section>
  );
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
  },
): AppState {
  return {
    ...state,
    activeTab: overrides.activeTab ?? state.activeTab,
    selectedSessionId: overrides.selectedSessionId ?? state.selectedSessionId,
    selectedModel: overrides.selectedModel ?? state.selectedModel,
  };
}

function fixtureAppState(gatewayUrl: string, fixture: string): AppState {
  const base = initialAppState(gatewayUrl);
  const now = Date.now();
  const protocolFixture = fixture === "communication-protocol";
  const session: Session = {
    id: protocolFixture ? "fixture-protocol" : "fixture-snake",
    title: protocolFixture ? "Communication style protocol" : "Snake game page",
    name: protocolFixture ? "Communication style protocol" : "Snake game page",
    directory: "C:\\Users\\liuliu\\Documents\\tura",
    model: "openai/gpt-5.5",
    agent: "coding_agent",
    session_type: "coding",
    status: fixture === "snake-pending" ? "busy" : "idle",
    created_at: now - 16_000,
    updated_at: now,
    modelVariant: "low",
    modelAccelerationEnabled: true,
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
  return {
    ...base,
    loading: false,
    bootstrapped: true,
    connection: "connected",
    activeTab: "conversation",
    directory: session.directory ?? undefined,
    selectedSessionId: session.id,
    sessions: [session],
    messagesBySession: { [session.id]: [user, assistant] },
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
