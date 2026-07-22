import type { GatewayEventEnvelope } from "../../types/event.js";
import type { Message, Session } from "../../types/session.js";
import type { PermissionRequest, QuestionRequest } from "../../types/permission.js";
import type {
  ProviderAuthMethodsResponse,
  ProviderAuthStatus,
  ProviderListResponse,
} from "../../types/provider.js";
import type { SessionConfig } from "../../types/config.js";
import type { StoredAgent } from "../../types/agent.js";
import type { AboutInfo, AboutUpdate } from "../../types/about.js";
import type { StoredPersona, TuraConfigResponse } from "../../types/gateway.js";
import type { SettingDetail } from "../settings-catalog.js";

export type { SettingDetail } from "../settings-catalog.js";

export type SettingInputKind = "api-key" | "oauth-callback";

export interface SettingInputState {
  kind: SettingInputKind;
  providerID: string;
  prompt: string;
  method?: number;
  oauthUrl?: string;
}

export interface SessionLoadingState {
  kind?: "loading" | "deleting";
  sessionID?: string;
  title?: string;
}

export interface AppState {
  cwd: string;
  session?: Session;
  sessions: Session[];
  messages: Message[];
  liveStreams: Record<string, LiveStream>;
  commandStatesBySession: Record<string, Record<string, CommandEventState>>;
  refreshState: Record<string, RefreshSessionState>;
  sessionPreviews: Record<string, string>;
  seenSessionMessageCounts: Record<string, number>;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  providers?: ProviderListResponse;
  agents: StoredAgent[];
  personas: StoredPersona[];
  authMethods?: ProviderAuthMethodsResponse;
  authStatuses: Record<string, ProviderAuthStatus>;
  sessionConfig?: SessionConfig;
  modelConfig?: TuraConfigResponse;
  status: "idle" | "busy" | "error";
  composer: string;
  composerCursor: number;
  selectedCompletionIndex: number;
  completionDismissed: boolean;
  notice?: string;
  noticeTransient?: boolean;
  help: boolean;
  sessionsOpen: boolean;
  modelsOpen: boolean;
  authOpen: boolean;
  settingsOpen: boolean;
  settingDetail?: SettingDetail;
  selectedProviderID?: string;
  settingInput?: SettingInputState;
  aboutInfo?: AboutInfo;
  aboutUpdate?: AboutUpdate;
  sessionLoading?: SessionLoadingState;
  personasOpen: boolean;
  selectedSessionIndex: number;
  selectedModelIndex: number;
  selectedPersonaIndex: number;
  selectedSettingsIndex: number;
  selectedSettingOptionIndex: number;
  thinkingFrame: number;
}

export interface LiveStream {
  sessionID: string;
  messageID: string;
  partID: string;
  field: "text" | "content";
  text: string;
  createdAt: number;
  updatedAt: number;
}

export interface CommandEventState {
  status: string;
  eventSeq?: number;
  updatedAt?: number;
}

export interface RefreshSessionState {
  lastFinalMessageID?: string;
  lastFinalMessageCount: number;
  updatedAt?: number;
  preview?: string;
}

export type AppAction =
  | {
      type: "hydrate";
      session: Session;
      messages: Message[];
      permissions: PermissionRequest[];
      providers?: ProviderListResponse;
      agents?: StoredAgent[];
      personas?: StoredPersona[];
      sessions?: Session[];
      authMethods?: ProviderAuthMethodsResponse;
      authStatuses?: Record<string, ProviderAuthStatus>;
      sessionConfig?: SessionConfig;
      modelConfig?: TuraConfigResponse;
      aboutInfo?: AboutInfo;
      closePanels?: boolean;
    }
  | { type: "event"; event: GatewayEventEnvelope }
  | { type: "messages-incremental"; sessionID: string; messages: Message[]; session?: Session }
  | { type: "composer"; value: string; cursor?: number }
  | { type: "select-completion"; delta: number; count: number }
  | { type: "dismiss-completion" }
  | { type: "notice"; value?: string; transient?: boolean }
  | { type: "status"; value: AppState["status"] }
  | { type: "permissions"; value: PermissionRequest[] }
  | { type: "questions"; value: QuestionRequest[] }
  | { type: "sessions"; value: Session[]; open?: boolean }
  | { type: "session-previews"; value: Record<string, string> }
  | { type: "session-loading"; value?: SessionLoadingState }
  | {
      type: "auth";
      methods?: ProviderAuthMethodsResponse;
      statuses?: Record<string, ProviderAuthStatus>;
      open?: boolean;
    }
  | { type: "agents"; value: StoredAgent[] }
  | {
      type: "session-config";
      value: SessionConfig;
      modelConfig?: TuraConfigResponse;
      open?: boolean;
    }
  | { type: "personas"; value: StoredPersona[]; open?: boolean }
  | { type: "select-session"; delta: number }
  | { type: "select-model"; delta: number }
  | { type: "select-persona"; delta: number }
  | { type: "select-settings"; delta: number }
  | { type: "open-setting-detail"; detail: SettingDetail; providerID?: string }
  | { type: "close-setting-detail" }
  | { type: "setting-input"; value?: SettingInputState }
  | { type: "about-info"; value: AboutInfo }
  | { type: "about-update"; value?: AboutUpdate }
  | { type: "select-setting-option"; delta: number }
  | { type: "tick" }
  | { type: "toggle-help" }
  | { type: "toggle-sessions" }
  | { type: "toggle-models" }
  | { type: "toggle-auth" }
  | { type: "toggle-settings" }
  | { type: "toggle-personas" }
  | { type: "close-panels" };

export function initialState(cwd: string): AppState {
  return {
    cwd,
    sessions: [],
    messages: [],
    liveStreams: {},
    commandStatesBySession: {},
    refreshState: {},
    sessionPreviews: {},
    seenSessionMessageCounts: {},
    permissions: [],
    questions: [],
    agents: [],
    personas: [],
    authStatuses: {},
    status: "idle",
    composer: "",
    composerCursor: 0,
    selectedCompletionIndex: 0,
    completionDismissed: false,
    help: false,
    sessionsOpen: false,
    modelsOpen: false,
    authOpen: false,
    settingsOpen: false,
    settingDetail: undefined,
    selectedProviderID: undefined,
    settingInput: undefined,
    aboutInfo: undefined,
    aboutUpdate: undefined,
    sessionLoading: undefined,
    personasOpen: false,
    selectedSessionIndex: 0,
    selectedModelIndex: 0,
    selectedPersonaIndex: 0,
    selectedSettingsIndex: 0,
    selectedSettingOptionIndex: 0,
    thinkingFrame: 0,
  };
}
