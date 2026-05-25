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
  QuestionRequest,
  ServiceStatusResponse,
  Session,
  TodoItem,
  VcsInfo,
  Workspace,
} from "@tura/gateway-sdk";

export type ConnectionState = "connecting" | "connected" | "disconnected";
export type MainTab = "new" | "conversation" | "plan" | "files" | "settings";
export type SettingsSection =
  | "general"
  | "appearance"
  | "providers"
  | "models"
  | "auth"
  | "runtime"
  | "config"
  | "workspace"
  | "environment";
export type ThemeMode = "light" | "dark";

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
  activeTab: MainTab;
  previousMainTab: Exclude<MainTab, "settings">;
  settingsSection: SettingsSection;
  themeMode: ThemeMode;
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
  selectedModel?: string;
  selectedAgent?: string;
  selectedProviderId?: string;
  modelVariant?: string;
  accelerationEnabled: boolean;
  authDrafts: Record<string, string>;
  authCodeDrafts: Record<string, string>;
  settingsSaving: boolean;
  settingsNotice?: string;
  modelValidation?: string;
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
    activeTab: "new",
    previousMainTab: "new",
    settingsSection: "general",
    themeMode: "light",
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
    selectedProviderId: undefined,
    modelVariant: "low",
    accelerationEnabled: true,
    authDrafts: {},
    authCodeDrafts: {},
    settingsSaving: false,
    submitting: false,
  };
}

export function sessionTitle(session: Session): string {
  return session.title || session.name || "New Session";
}

export function sessionUpdatedAt(session: Session): number {
  return session.time?.updated ?? session.updated_at ?? 0;
}

export function sessionDirectory(session: Session): string {
  return session.directory || session.projectID || "";
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
