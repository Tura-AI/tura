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
  PlanStatus,
  PollInterval,
  ProductConfig,
  ProductIssue,
  ProductProject,
  ProductUser,
  Project,
  ProviderAuthActionResponse,
  ProviderAuthMethod,
  ProviderAuthStatusResponse,
  ProviderListResponse,
  QuestionRequest,
  ServiceStatusResponse,
  Session,
  StartCondition,
  StoredPersona,
  TodoItem,
  TuraConfigResponse,
  VcsInfo,
  Workspace,
} from "@tura/gateway-sdk";
import { draftStateDefaults } from "./drafts";

export type ConnectionState = "connecting" | "connected" | "disconnected";
export type MainTab = "conversation" | "plan" | "files" | "settings";
export type SettingsSection =
  | "application"
  | "appearance"
  | "providers"
  | "models"
  | "agents"
  | "personalization";
export type ThemeMode = "light" | "dark" | "caral" | "uruk" | "liangzhu";
export type PlanMode = "todo" | "gantt";
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
  gatewayStartupNotice?: string;
  loading: boolean;
  sessionsLoading: boolean;
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
  editingTask?: { sessionId: string; task_id?: string };
  taskPulse?: { sessionId: string; task_id?: string; token: number };
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
  lastSessionOpenedId?: string;
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
  messagePagingBySession: Record<string, { hasEarlier: boolean; loadingEarlier: boolean }>;
  transcriptScrollBySession: Record<string, number>;
  todosBySession: Record<string, TodoItem[]>;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  providers?: ProviderListResponse;
  modelConfig?: TuraConfigResponse;
  providerAuthMethods: Record<string, ProviderAuthMethod[]>;
  providerAuthStatus: Record<string, ProviderAuthStatusResponse>;
  providerValidationReceipts: Record<string, ProviderAuthActionResponse>;
  agents: Agent[];
  personas: StoredPersona[];
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
  const drafts = draftStateDefaults();
  return {
    gatewayUrl,
    connection: "connecting",
    gatewayStartupNotice: undefined,
    loading: true,
    sessionsLoading: true,
    bootstrapped: false,
    sessions: [],
    workspaces: [],
    productIssues: [],
    productProjects: [],
    issueDraft: drafts.issueDraft,
    issueSearch: drafts.issueSearch,
    planMode: "todo",
    planDraftStartCondition: drafts.planDraftStartCondition,
    planDraftStartAt: drafts.planDraftStartAt,
    planDraftPollInterval: drafts.planDraftPollInterval,
    editingTask: undefined,
    activeTab: "conversation",
    previousMainTab: "conversation",
    settingsSection: drafts.settingsSection,
    lastSessionOpenedId: undefined,
    themeMode: systemThemeMode(),
    mainFont: "",
    codeFont: "",
    mainFontSize: 12,
    codeFontSize: 12,
    messagesBySession: {},
    messagePagingBySession: {},
    transcriptScrollBySession: {},
    todosBySession: {},
    permissions: [],
    questions: [],
    configDraft: drafts.configDraft,
    workspaceConfig: {},
    workspaceConfigDraft: drafts.workspaceConfigDraft,
    providerAuthMethods: {},
    providerAuthStatus: {},
    providerValidationReceipts: {},
    agents: [],
    personas: [],
    commands: [],
    projects: [],
    diff: [],
    files: [],
    filePath: "",
    composerText: drafts.composerText,
    composerImages: drafts.composerImages,
    selectedProviderId: undefined,
    providerSearch: drafts.providerSearch,
    providerAuthPanel: undefined,
    modelVariant: "medium",
    accelerationEnabled: true,
    authDrafts: drafts.authDrafts,
    authCodeDrafts: drafts.authCodeDrafts,
    settingsSaving: false,
    submitting: false,
  };
}

export function systemThemeMode(): Extract<ThemeMode, "light" | "dark"> {
  return typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export const SESSION_FALLBACK_NAME_MAX_LENGTH = 48;

export function sessionFallbackNameFromInput(
  input: string,
  maxLength = SESSION_FALLBACK_NAME_MAX_LENGTH,
): string {
  const normalized = input.replace(/\s+/gu, " ").trim();
  if (!normalized) {
    return "";
  }
  return Array.from(normalized).slice(0, maxLength).join("");
}

export function sessionHasDisplayName(session: Session): boolean {
  return Boolean(
    session.session_display_name?.trim() || session.plan_summary?.trim() || session.name?.trim(),
  );
}

export function withSessionFallbackName(session: Session, input: string): Session {
  if (sessionHasDisplayName(session)) {
    return session;
  }
  const fallbackName = sessionFallbackNameFromInput(input);
  if (!fallbackName) {
    return session;
  }
  return {
    ...session,
    name: fallbackName,
    session_display_name: fallbackName,
  };
}

export function sessionTitle(session: Session): string {
  return session.session_display_name || session.plan_summary || session.name || "New Session";
}

export function sessionUpdatedAt(session: Session): number {
  return session.updated_at ?? 0;
}

export function sessionDirectory(session: Session): string {
  return session.directory || "";
}

export function messageSessionId(message: Message): string {
  return message.sessionID;
}

export function messageCreatedAt(message: Message): number {
  return message.time?.created ?? message.created_at ?? 0;
}

export function partText(part: { text?: string | null; content?: string | null }): string {
  return part.text || part.content || "";
}

export function activeSession(state: AppState): Session | undefined {
  return state.sessions.find((session) => session.id === state.selectedSessionId);
}
