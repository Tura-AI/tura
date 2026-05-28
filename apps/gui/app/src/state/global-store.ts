import type {
  Agent,
  Command,
  CurrentProjectResponse,
  FileContentResponse,
  FileDiff,
  FileInfo,
  GatewayConfig,
  HealthResponse,
  Message,
  PathResponse,
  PermissionRequest,
  ProductConfig,
  ProductIssue,
  ProductProject,
  ProductUser,
  Project,
  ProviderAuthMethod,
  ProviderAuthStatusResponse,
  ProviderListResponse,
  PollInterval,
  QuestionRequest,
  ServiceStatusResponse,
  Session,
  StartCondition,
  PlanStatus,
  TodoItem,
  TuraConfigResponse,
  VcsInfo,
  Workspace,
} from "@tura/gateway-sdk";

export type ConnectionState = "connecting" | "connected" | "disconnected";
export type MainTab = "new" | "conversation" | "plan" | "files" | "settings";
export type SettingsSection =
  | "appearance"
  | "providers"
  | "models";
export type ThemeMode = "light" | "dark" | "caral" | "uruk" | "liangzhu";
export type PlanMode = "todo" | "gantt" | "calendar";
export type ProviderAuthPanel = {
  providerId: string;
  reason?: string;
};
export type ComposerImage = {
  id: string;
  name: string;
  dataUrl: string;
  objectUrl?: string;
  mimeType?: string;
  kind?: "image" | "file";
};

export type AppState = {
  gatewayUrl: string;
  connection: ConnectionState;
  loading: boolean;
  bootstrapped: boolean;
  productConfig?: ProductConfig;
  me?: ProductUser;
  workspaces: Workspace[];
  productIssues: ProductIssue[];
  productProjects: ProductProject[];
  issueDraft: string;
  issueSearch: string;
  planMode: PlanMode;
  planDraftLane?: PlanStatus;
  planDraftStartCondition: StartCondition;
  planDraftStartAt: string;
  planDraftPollInterval: PollInterval;
  planDraftSessionId?: string;
  planPreviewSessionId?: string;
  editingTask?: { sessionId: string; nonce_id?: string };
  taskPulse?: { sessionId: string; nonce_id?: string; token: number };
  planNotice?: { message: string; code?: string; providerId?: string };
  activeTab: MainTab;
  previousMainTab: Exclude<MainTab, "settings">;
  settingsSection: SettingsSection;
  themeMode: ThemeMode;
  mainFont: string;
  codeFont: string;
  mainFontSize: number;
  codeFontSize: number;
  directory?: string;
  selectedSessionId?: string;
  health?: HealthResponse;
  serviceStatus?: ServiceStatusResponse;
  config?: GatewayConfig;
  configDraft: Record<string, string>;
  workspaceConfig: Record<string, unknown>;
  workspaceConfigDraft: Record<string, string>;
  paths?: PathResponse;
  currentProject?: CurrentProjectResponse;
  projects: Project[];
  sessions: Session[];
  messagesBySession: Record<string, Message[]>;
  todosBySession: Record<string, TodoItem[]>;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  providers?: ProviderListResponse;
  modelConfig?: TuraConfigResponse;
  providerAuthMethods: Record<string, ProviderAuthMethod[]>;
  providerAuthStatus: Record<string, ProviderAuthStatusResponse>;
  agents: Agent[];
  commands: Command[];
  vcs?: VcsInfo;
  diff: FileDiff[];
  files: FileInfo[];
  filePath: string;
  selectedFile?: FileInfo;
  fileContent?: FileContentResponse;
  composerText: string;
  composerImages: ComposerImage[];
  selectedModel?: string;
  selectedAgent?: string;
  selectedProviderId?: string;
  providerSearch: string;
  providerAuthPanel?: ProviderAuthPanel;
  modelVariant?: string;
  accelerationEnabled: boolean;
  authDrafts: Record<string, string>;
  authCodeDrafts: Record<string, string>;
  settingsSaving: boolean;
  settingsNotice?: string;
  submitting: boolean;
  error?: string;
  lastEvent?: string;
};

export function initialAppState(gatewayUrl: string): AppState {
  return {
    gatewayUrl,
    connection: "connecting",
    loading: true,
    bootstrapped: false,
    sessions: [],
    workspaces: [],
    productIssues: [],
    productProjects: [],
    issueDraft: "",
    issueSearch: "",
    planMode: "todo",
    planDraftStartCondition: "user_action",
    planDraftStartAt: "",
    planDraftPollInterval: { m: 0, d: 0, h: 1, s: 0 },
    editingTask: undefined,
    activeTab: "new",
    previousMainTab: "new",
    settingsSection: "appearance",
    themeMode: systemThemeMode(),
    mainFont: "",
    codeFont: "",
    mainFontSize: 13,
    codeFontSize: 12,
    messagesBySession: {},
    todosBySession: {},
    permissions: [],
    questions: [],
    configDraft: {},
    workspaceConfig: {},
    workspaceConfigDraft: {},
    providerAuthMethods: {},
    providerAuthStatus: {},
    agents: [],
    commands: [],
    projects: [],
    diff: [],
    files: [],
    filePath: "",
    composerText: "",
    composerImages: [],
    selectedProviderId: undefined,
    providerSearch: "",
    providerAuthPanel: undefined,
    modelVariant: "low",
    accelerationEnabled: true,
    authDrafts: {},
    authCodeDrafts: {},
    settingsSaving: false,
    submitting: false,
  };
}

export function systemThemeMode(): Extract<ThemeMode, "light" | "dark"> {
  return typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function sessionTitle(session: Session): string {
  return (
    session.session_display_name ||
    session.plan_summary ||
    session.name ||
    "New Session"
  );
}

export function sessionUpdatedAt(session: Session): number {
  return session.updated_at ?? 0;
}

export function sessionDirectory(session: Session): string {
  return session.directory || "";
}

export function messageSessionId(message: Message): string {
  return message.sessionID || message.session_id || "";
}

export function messageCreatedAt(message: Message): number {
  return message.time?.created ?? message.created_at ?? 0;
}

export function partText(part: {
  text?: string | null;
  content?: string | null;
}): string {
  return part.text || part.content || "";
}

export function activeSession(state: AppState): Session | undefined {
  return state.sessions.find(
    (session) => session.id === state.selectedSessionId,
  );
}
