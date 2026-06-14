import type { GatewayEventEnvelope } from "../types/event.js";
import type { Message, MessagePart, Session } from "../types/session.js";
import { normalizeEvent } from "../gateway/events.js";
import { sameDirectory } from "../gateway/directory.js";
import {
  isInternalTaskStatusPart,
  messageText,
  messageSortValue,
  partMessageID,
  partSessionID,
  sessionStatusText,
  sessionUpdatedAt,
} from "../types/session.js";
import type { PermissionRequest, QuestionRequest } from "../types/permission.js";
import type {
  ProviderAuthMethodsResponse,
  ProviderAuthStatus,
  ProviderListResponse,
} from "../types/provider.js";
import type { SessionConfig } from "../types/config.js";
import type { StoredAgent } from "../types/agent.js";
import type { StoredPersona } from "../types/gateway.js";

export type SettingDetail =
  | "model"
  | "provider"
  | "providerAuth"
  | "agent"
  | "persona"
  | "variant"
  | "priority"
  | "commands"
  | "stallGuard";

export type SettingInputKind = "api-key" | "oauth-callback";

export interface SettingInputState {
  kind: SettingInputKind;
  providerID: string;
  prompt: string;
}

const SETTINGS_ENTRY_COUNT = 8;
const SESSION_CREATE_ENTRY_COUNT = 1;
const rawAnsiControlPattern = /\x1b\][^\x07]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[@-_]/g;

export interface AppState {
  cwd: string;
  session?: Session;
  sessions: Session[];
  messages: Message[];
  liveStreams: Record<string, LiveStream>;
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
  status: "idle" | "busy" | "error";
  composer: string;
  notice?: string;
  help: boolean;
  sessionsOpen: boolean;
  modelsOpen: boolean;
  authOpen: boolean;
  settingsOpen: boolean;
  settingDetail?: SettingDetail;
  selectedProviderID?: string;
  settingInput?: SettingInputState;
  personasOpen: boolean;
  selectedSessionIndex: number;
  selectedModelIndex: number;
  selectedPersonaIndex: number;
  selectedSettingsIndex: number;
  selectedSettingOptionIndex: number;
  thinkingFrame: number;
}

export interface LiveStream {
  sessionID?: string;
  messageID: string;
  partID: string;
  field: "text" | "content";
  text: string;
  createdAt: number;
  updatedAt: number;
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
      closePanels?: boolean;
    }
  | { type: "event"; event: GatewayEventEnvelope }
  | { type: "messages-incremental"; sessionID: string; messages: Message[]; session?: Session }
  | { type: "composer"; value: string }
  | { type: "notice"; value?: string }
  | { type: "status"; value: AppState["status"] }
  | { type: "permissions"; value: PermissionRequest[] }
  | { type: "questions"; value: QuestionRequest[] }
  | { type: "sessions"; value: Session[]; open?: boolean }
  | { type: "session-previews"; value: Record<string, string> }
  | {
      type: "auth";
      methods?: ProviderAuthMethodsResponse;
      statuses?: Record<string, ProviderAuthStatus>;
      open?: boolean;
    }
  | { type: "agents"; value: StoredAgent[] }
  | { type: "session-config"; value: SessionConfig; open?: boolean }
  | { type: "personas"; value: StoredPersona[]; open?: boolean }
  | { type: "select-session"; delta: number }
  | { type: "select-model"; delta: number }
  | { type: "select-persona"; delta: number }
  | { type: "select-settings"; delta: number }
  | { type: "open-setting-detail"; detail: SettingDetail; providerID?: string }
  | { type: "close-setting-detail" }
  | { type: "setting-input"; value?: SettingInputState }
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
    help: false,
    sessionsOpen: false,
    modelsOpen: false,
    authOpen: false,
    settingsOpen: false,
    settingDetail: undefined,
    selectedProviderID: undefined,
    settingInput: undefined,
    personasOpen: false,
    selectedSessionIndex: 0,
    selectedModelIndex: 0,
    selectedPersonaIndex: 0,
    selectedSettingsIndex: 0,
    selectedSettingOptionIndex: 0,
    thinkingFrame: 0,
  };
}

export function reducer(state: AppState, action: AppAction): AppState {
  if (action.type === "hydrate") {
    const sessionID = action.session.id;
    const sessionChanged = Boolean(state.session && state.session.id !== sessionID);
    const nextSessions = action.sessions ?? state.sessions;
    const hydratedMessages = normalizeMessagesForDisplay(action.messages);
    const nextMessages = sessionChanged
      ? hydratedMessages
      : mergeStableMessages(state.messages, hydratedMessages);
    const currentPreview = lastMessagePreview(hydratedMessages);
    const panelState = action.closePanels ? closedPanelsState() : {};
    return {
      ...state,
      ...panelState,
      session: action.session,
      sessions: nextSessions,
      messages: nextMessages,
      liveStreams: sessionChanged
        ? {}
        : clearLiveStreamsForDurableMessages(state.liveStreams, sessionID, hydratedMessages),
      refreshState: refreshStateAfterMessages(
        state.refreshState,
        sessionID,
        nextMessages,
        action.session,
      ),
      sessionPreviews: currentPreview
        ? { ...state.sessionPreviews, [sessionID]: currentPreview }
        : state.sessionPreviews,
      seenSessionMessageCounts: {
        ...state.seenSessionMessageCounts,
        [sessionID]: action.messages.length,
      },
      permissions: action.permissions,
      questions: state.questions,
      providers: action.providers,
      agents: action.agents ?? state.agents,
      personas: action.personas ?? state.personas,
      authMethods: action.authMethods ?? state.authMethods,
      authStatuses: action.authStatuses ?? state.authStatuses,
      sessionConfig: action.sessionConfig ?? state.sessionConfig,
      status: action.session.status ?? "idle",
      selectedSessionIndex:
        state.sessionsOpen && !sessionChanged
          ? boundedSessionIndex(state.selectedSessionIndex, nextSessions)
          : selectedSessionIndex(nextSessions, action.session.id),
      selectedPersonaIndex: selectedPersonaIndex(
        action.personas ?? state.personas,
        action.agents ?? state.agents,
        action.session ?? state.session,
        action.sessionConfig ?? state.sessionConfig,
      ),
    };
  }
  if (action.type === "event") {
    const normalized = normalizeEvent(action.event);
    if (normalized.directory !== "global" && !sameDirectory(normalized.directory, state.cwd))
      return state;
    if (action.event.payload?.type === "server.connected") {
      return state;
    }
    if (action.event.payload?.type === "message.updated") {
      const message = (action.event.payload.properties as { info?: Message } | undefined)?.info;
      if (!message) return state;
      const sessionID = normalized.sessionID || message.sessionID || message.session_id;
      if (state.session && sessionID && sessionID !== state.session.id) {
        const preview = messagePreview(message) ?? state.sessionPreviews[sessionID] ?? "";
        return {
          ...state,
          sessionPreviews: {
            ...state.sessionPreviews,
            [sessionID]: preview,
          },
          refreshState: refreshStateAfterBackgroundMessage(state.refreshState, sessionID, message),
          sessions: state.sessions.map((session) =>
            session.id === sessionID
              ? {
                  ...session,
                  message_count: (session.message_count ?? 0) + 1,
                  updated_at: message.updated_at ?? message.created_at ?? session.updated_at,
                }
              : session,
          ),
        };
      }
      const messages = upsertMessage(state.messages, message);
      return {
        ...state,
        messages,
        liveStreams: clearLiveStreamsForDurableMessages(state.liveStreams, sessionID, [message]),
        refreshState: refreshStateAfterMessages(
          state.refreshState,
          sessionID,
          messages,
          state.session,
        ),
      };
    }
    if (action.event.payload?.type === "message.part.updated") {
      const part = (action.event.payload.properties as { part?: MessagePart } | undefined)?.part;
      if (!part) return state;
      const sessionID = normalized.sessionID ?? partSessionID(part);
      if (state.session && sessionID !== state.session.id)
        return state;
      const messages = upsertPart(state.messages, part, sessionID);
      return {
        ...state,
        messages,
        liveStreams: clearLiveStreamForPart(state.liveStreams, sessionID, part),
        refreshState: refreshStateAfterMessages(
          state.refreshState,
          sessionID,
          messages,
          state.session,
        ),
      };
    }
    if (action.event.payload?.type === "message.part.delta") {
      const properties = action.event.payload.properties as Record<string, unknown> | undefined;
      const sessionID = normalized.sessionID;
      if (state.session && sessionID !== state.session.id)
        return state;
      return {
        ...state,
        liveStreams: applyPartDelta(
          state.liveStreams,
          state.messages,
          readString(properties, "message_id") ?? readString(properties, "messageID"),
          readString(properties, "part_id") ?? readString(properties, "partID"),
          readString(properties, "field"),
          readString(properties, "delta"),
          sessionID,
        ),
      };
    }
    if (action.event.payload?.type === "message.removed") {
      const properties = action.event.payload.properties as { message_id?: string } | undefined;
      if (state.session && normalized.sessionID && normalized.sessionID !== state.session.id)
        return state;
      return {
        ...state,
        messages: state.messages.filter((message) => message.id !== properties?.message_id),
        liveStreams: clearLiveStreamsForMessageID(
          state.liveStreams,
          normalized.sessionID,
          properties?.message_id,
        ),
        refreshState: invalidateRefreshState(state.refreshState, normalized.sessionID),
      };
    }
    if (action.event.payload?.type === "session.status") {
      const properties = action.event.payload.properties as Record<string, unknown> | undefined;
      const status = sessionStatusText(properties?.status);
      const sessionID = readString(properties, "sessionID") ?? readString(properties, "session_id");
      return {
        ...state,
        status: state.session?.id === sessionID || !sessionID ? status : state.status,
        sessions: sessionID
          ? state.sessions.map((session) =>
              session.id === sessionID ? { ...session, status } : session,
            )
          : state.sessions,
        session:
          state.session && state.session.id === sessionID
            ? { ...state.session, status }
            : state.session,
      };
    }
    if (action.event.payload?.type === "session.updated") {
      const session = (action.event.payload.properties as { info?: Session } | undefined)?.info;
      if (session && session.id === state.session?.id) {
        return {
          ...state,
          session,
          sessions: upsertSession(state.sessions, session),
          status: session.status ?? state.status,
        };
      }
      if (session) return { ...state, sessions: upsertSession(state.sessions, session) };
    }
    if (action.event.payload?.type === "session.created") {
      const session = (action.event.payload.properties as { info?: Session } | undefined)?.info;
      if (session) return { ...state, sessions: upsertSession(state.sessions, session) };
    }
    if (action.event.payload?.type === "session.deleted") {
      const properties = action.event.payload.properties as Record<string, unknown> | undefined;
      const sessionID = readString(properties, "sessionID") ?? readString(properties, "session_id");
      if (sessionID)
        return { ...state, sessions: state.sessions.filter((session) => session.id !== sessionID) };
    }
    if (action.event.payload?.type === "permission.asked" && normalized.permission) {
      return { ...state, permissions: upsertById(state.permissions, normalized.permission) };
    }
    if (action.event.payload?.type === "permission.replied" && normalized.permission) {
      return {
        ...state,
        permissions: state.permissions.filter(
          (permission) => permission.id !== normalized.permission?.id,
        ),
      };
    }
    if (action.event.payload?.type === "question.asked" && normalized.question) {
      return { ...state, questions: upsertById(state.questions, normalized.question) };
    }
    if (
      (action.event.payload?.type === "question.replied" ||
        action.event.payload?.type === "question.rejected") &&
      normalized.question
    ) {
      return {
        ...state,
        questions: state.questions.filter((question) => question.id !== normalized.question?.id),
      };
    }
    return state;
  }
  if (action.type === "messages-incremental") {
    const sessionID = action.sessionID;
    const incoming = normalizeMessagesForDisplay(action.messages);
    if (state.session?.id !== sessionID) {
      return {
        ...state,
        sessionPreviews: updatePreviewForMessages(state.sessionPreviews, sessionID, incoming),
        refreshState: refreshStateAfterMessages(
          state.refreshState,
          sessionID,
          incoming,
          action.session,
        ),
      };
    }
    const messages = mergeStableMessages(state.messages, incoming);
    return {
      ...state,
      session: action.session ?? state.session,
      messages,
      liveStreams: clearLiveStreamsForDurableMessages(state.liveStreams, sessionID, incoming),
      sessionPreviews: updatePreviewForMessages(state.sessionPreviews, sessionID, messages),
      refreshState: refreshStateAfterMessages(
        state.refreshState,
        sessionID,
        messages,
        action.session ?? state.session,
      ),
    };
  }
  if (action.type === "composer") return { ...state, composer: action.value };
  if (action.type === "notice") return { ...state, notice: action.value };
  if (action.type === "status") return { ...state, status: action.value };
  if (action.type === "permissions") return { ...state, permissions: action.value };
  if (action.type === "questions") return { ...state, questions: action.value };
  if (action.type === "sessions") {
    const keepSelection = state.sessionsOpen && action.open;
    return {
      ...state,
      sessions: action.value,
      seenSessionMessageCounts: seedSeenSessionCounts(
        state.seenSessionMessageCounts,
        action.value,
        state.session?.id,
      ),
      sessionsOpen: action.open ?? state.sessionsOpen,
      selectedSessionIndex: keepSelection
        ? boundedSessionIndex(state.selectedSessionIndex, action.value)
        : selectedSessionIndex(action.value, state.session?.id),
    };
  }
  if (action.type === "session-previews") {
    return { ...state, sessionPreviews: { ...state.sessionPreviews, ...action.value } };
  }
  if (action.type === "auth") {
    return {
      ...state,
      authMethods: action.methods ?? state.authMethods,
      authStatuses: action.statuses ?? state.authStatuses,
      authOpen: action.open ?? state.authOpen,
      sessionsOpen: action.open ? false : state.sessionsOpen,
      modelsOpen: action.open ? false : state.modelsOpen,
      settingsOpen: action.open ? false : state.settingsOpen,
      settingDetail: action.open ? undefined : state.settingDetail,
      selectedProviderID: action.open ? undefined : state.selectedProviderID,
      personasOpen: action.open ? false : state.personasOpen,
    };
  }
  if (action.type === "agents") return { ...state, agents: action.value };
  if (action.type === "session-config") {
    return {
      ...state,
      sessionConfig: action.value,
      settingsOpen: action.open ?? state.settingsOpen,
      settingDetail: action.open ? undefined : state.settingDetail,
      selectedProviderID: action.open ? undefined : state.selectedProviderID,
      sessionsOpen: false,
      modelsOpen: false,
      authOpen: false,
      personasOpen: false,
    };
  }
  if (action.type === "personas") {
    return {
      ...state,
      personas: action.value,
      personasOpen: action.open ?? state.personasOpen,
      sessionsOpen: false,
      modelsOpen: false,
      authOpen: false,
      settingsOpen: false,
      settingDetail: undefined,
      selectedProviderID: undefined,
      settingInput: undefined,
      selectedPersonaIndex: selectedPersonaIndex(
        action.value,
        state.agents,
        state.session,
        state.sessionConfig,
      ),
    };
  }
  if (action.type === "select-session") {
    return {
      ...state,
      selectedSessionIndex: wrapIndex(
        state.selectedSessionIndex + action.delta,
        state.sessions.length + SESSION_CREATE_ENTRY_COUNT,
      ),
    };
  }
  if (action.type === "select-model") {
    return {
      ...state,
      selectedModelIndex: wrapIndex(
        state.selectedModelIndex + action.delta,
        modelCount(state.providers),
      ),
    };
  }
  if (action.type === "select-persona") {
    return {
      ...state,
      selectedPersonaIndex: wrapIndex(
        state.selectedPersonaIndex + action.delta,
        state.personas.length,
      ),
    };
  }
  if (action.type === "select-settings") {
    return {
      ...state,
      selectedSettingsIndex: wrapIndex(
        state.selectedSettingsIndex + action.delta,
        SETTINGS_ENTRY_COUNT,
      ),
    };
  }
  if (action.type === "open-setting-detail") {
    return {
      ...state,
      settingsOpen: true,
      settingDetail: action.detail,
      selectedProviderID: action.providerID ?? state.selectedProviderID,
      settingInput: undefined,
      selectedSettingOptionIndex: selectedSettingOptionIndex(state, action.detail),
      sessionsOpen: false,
      modelsOpen: false,
      authOpen: false,
      personasOpen: false,
    };
  }
  if (action.type === "close-setting-detail") {
    return {
      ...state,
      settingDetail: undefined,
      selectedProviderID: undefined,
      settingInput: undefined,
      selectedSettingOptionIndex: 0,
    };
  }
  if (action.type === "select-setting-option") {
    return {
      ...state,
      selectedSettingOptionIndex: wrapIndex(
        state.selectedSettingOptionIndex + action.delta,
        settingOptionCount(state),
      ),
    };
  }
  if (action.type === "setting-input") {
    return { ...state, settingInput: action.value, composer: action.value ? "" : state.composer };
  }
  if (action.type === "tick") return { ...state, thinkingFrame: state.thinkingFrame + 1 };
  if (action.type === "toggle-help") return { ...state, help: !state.help };
  if (action.type === "toggle-sessions")
    return {
      ...state,
      sessionsOpen: !state.sessionsOpen,
      modelsOpen: false,
      authOpen: false,
      settingsOpen: false,
      settingDetail: undefined,
      selectedProviderID: undefined,
      settingInput: undefined,
      personasOpen: false,
    };
  if (action.type === "toggle-models")
    return {
      ...state,
      modelsOpen: !state.modelsOpen,
      sessionsOpen: false,
      authOpen: false,
      settingsOpen: false,
      settingDetail: undefined,
      selectedProviderID: undefined,
      settingInput: undefined,
      personasOpen: false,
    };
  if (action.type === "toggle-auth")
    return {
      ...state,
      authOpen: !state.authOpen,
      sessionsOpen: false,
      modelsOpen: false,
      settingsOpen: false,
      settingDetail: undefined,
      selectedProviderID: undefined,
      settingInput: undefined,
      personasOpen: false,
    };
  if (action.type === "toggle-settings")
    return {
      ...state,
      settingsOpen: !state.settingsOpen,
      settingDetail: !state.settingsOpen ? undefined : state.settingDetail,
      selectedProviderID: !state.settingsOpen ? undefined : state.selectedProviderID,
      settingInput: !state.settingsOpen ? undefined : state.settingInput,
      sessionsOpen: false,
      modelsOpen: false,
      authOpen: false,
      personasOpen: false,
    };
  if (action.type === "toggle-personas")
    return {
      ...state,
      personasOpen: !state.personasOpen,
      sessionsOpen: false,
      modelsOpen: false,
      authOpen: false,
      settingsOpen: false,
      settingDetail: undefined,
      selectedProviderID: undefined,
      settingInput: undefined,
    };
  if (action.type === "close-panels")
    return {
      ...state,
      sessionsOpen: false,
      modelsOpen: false,
      authOpen: false,
      settingsOpen: false,
      settingDetail: undefined,
      selectedProviderID: undefined,
      settingInput: undefined,
      personasOpen: false,
      help: false,
    };
  return state;
}

function closedPanelsState(): Pick<
  AppState,
  | "sessionsOpen"
  | "modelsOpen"
  | "authOpen"
  | "settingsOpen"
  | "settingDetail"
  | "selectedProviderID"
  | "settingInput"
  | "personasOpen"
  | "help"
> {
  return {
    sessionsOpen: false,
    modelsOpen: false,
    authOpen: false,
    settingsOpen: false,
    settingDetail: undefined,
    selectedProviderID: undefined,
    settingInput: undefined,
    personasOpen: false,
    help: false,
  };
}

export function displayMessages(state: AppState): Message[] {
  const streams = Object.values(state.liveStreams).filter((stream) =>
    state.session?.id ? stream.sessionID === state.session.id : true,
  );
  if (!streams.length) return state.messages;
  let messages = state.messages;
  for (const stream of streams.sort((left, right) => left.updatedAt - right.updatedAt)) {
    messages = applyLiveStream(messages, stream);
  }
  return messages;
}

function mergeStableMessages(current: Message[], incoming: Message[]): Message[] {
  let changed = false;
  const next = [...current];
  const indexes = new Map(next.map((message, index) => [message.id, index]));
  for (const message of incoming) {
    const index = indexes.get(message.id);
    if (index === undefined) {
      indexes.set(message.id, next.length);
      next.push(message);
      changed = true;
    }
  }
  return changed ? next : current;
}

function updatePreviewForMessages(
  previews: Record<string, string>,
  sessionID: string,
  messages: Message[],
): Record<string, string> {
  const preview = lastMessagePreview(messages);
  return preview ? { ...previews, [sessionID]: preview } : previews;
}

function refreshStateAfterBackgroundMessage(
  current: Record<string, RefreshSessionState>,
  sessionID: string | undefined,
  message: Message,
): Record<string, RefreshSessionState> {
  if (!sessionID) return current;
  const existing = current[sessionID];
  return {
    ...current,
    [sessionID]: {
      lastFinalMessageID: message.id,
      lastFinalMessageCount:
        existing?.lastFinalMessageID === message.id
          ? existing.lastFinalMessageCount
          : (existing?.lastFinalMessageCount ?? 0) + 1,
      updatedAt: message.updated_at ?? message.created_at ?? Date.now(),
      preview: messagePreview(message) ?? existing?.preview,
    },
  };
}

function refreshStateAfterMessages(
  current: Record<string, RefreshSessionState>,
  sessionID: string | undefined,
  messages: Message[],
  session: Session | undefined,
): Record<string, RefreshSessionState> {
  if (!sessionID) return current;
  const last = messages.at(-1);
  const preview = lastMessagePreview(messages) ?? current[sessionID]?.preview;
  return {
    ...current,
    [sessionID]: {
      lastFinalMessageID: last?.id,
      lastFinalMessageCount: messages.length,
      updatedAt: session?.updated_at ?? last?.updated_at ?? last?.created_at ?? Date.now(),
      preview,
    },
  };
}

function invalidateRefreshState(
  current: Record<string, RefreshSessionState>,
  sessionID?: string,
): Record<string, RefreshSessionState> {
  if (sessionID) {
    const { [sessionID]: _removed, ...rest } = current;
    return rest;
  }
  return {};
}

function settingOptionCount(state: AppState): number {
  if (state.settingDetail === "model") return modelCount(state.providers);
  if (state.settingDetail === "provider") return settingProviderCount(state.providers);
  if (state.settingDetail === "providerAuth")
    return (state.authMethods?.[state.selectedProviderID ?? ""]?.length ?? 0) + 2;
  if (state.settingDetail === "agent") return state.agents.length;
  if (state.settingDetail === "persona") return state.personas.length;
  if (state.settingDetail === "variant") return 4;
  if (state.settingDetail === "priority") return 2;
  if (state.settingDetail === "commands") return 2;
  if (state.settingDetail === "stallGuard") return 4;
  return SETTINGS_ENTRY_COUNT;
}

function selectedSettingOptionIndex(state: AppState, detail: SettingDetail): number {
  const config = state.sessionConfig;
  if (detail === "model") return state.selectedModelIndex;
  if (detail === "provider") {
    const active = config?.active_provider;
    const index = settingProviders(state.providers).findIndex((provider) => provider.id === active);
    return index >= 0 ? index : 0;
  }
  if (detail === "providerAuth") return 0;
  if (detail === "agent") {
    const active = state.session?.agent ?? config?.active_agent;
    const index = state.agents.findIndex((agent) => storedAgentID(agent) === active);
    return index >= 0 ? index : 0;
  }
  if (detail === "persona") return state.selectedPersonaIndex;
  if (detail === "variant")
    return Math.max(0, ["low", "medium", "high", "xhigh"].indexOf(String(config?.model_variant)));
  if (detail === "priority") return config?.model_acceleration_enabled ? 0 : 1;
  if (detail === "commands") return config?.show_command_instructions !== false ? 0 : 1;
  if (detail === "stallGuard")
    return Math.max(
      0,
      ["default", "relaxed", "strict", "off"].indexOf(
        String(config?.command_run_stall_guard_profile),
      ),
    );
  return 0;
}

function upsertSession(sessions: Session[], session: Session): Session[] {
  const next = sessions.filter((item) => item.id !== session.id);
  next.push(session);
  next.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  return next;
}

function seedSeenSessionCounts(
  current: Record<string, number>,
  sessions: Session[],
  activeSessionID: string | undefined,
): Record<string, number> {
  const next = { ...current };
  for (const session of sessions) {
    if (next[session.id] !== undefined && session.id !== activeSessionID) continue;
    next[session.id] = session.message_count ?? next[session.id] ?? 0;
  }
  return next;
}

function boundedSessionIndex(index: number, sessions: Session[]): number {
  return wrapIndex(index, sessions.length + SESSION_CREATE_ENTRY_COUNT);
}

function lastMessagePreview(messages: Message[]): string | undefined {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const preview = messagePreview(messages[index]);
    if (preview) return preview;
  }
  return undefined;
}

function messagePreview(message: Message | undefined): string | undefined {
  const text = message ? messageText(message).replace(/\s+/g, " ").trim() : "";
  return text || undefined;
}

function upsertMessage(messages: Message[], message: Message): Message[] {
  const existing = messages.find((item) => item.id === message.id);
  const merged = mergeMessageForDisplay(existing, message);
  const next = messages.filter((item) => item.id !== message.id);
  next.push(merged);
  next.sort((left, right) => messageSortValue(left) - messageSortValue(right));
  return next;
}

function normalizeMessagesForDisplay(messages: Message[]): Message[] {
  return messages.map((message) => mergeMessageForDisplay(undefined, message));
}

function mergeMessageForDisplay(existing: Message | undefined, incoming: Message): Message {
  const existingCreated = existing?.created_at ?? existing?.time?.created;
  const incomingCreated = incoming.created_at ?? incoming.time?.created;
  const time =
    existing?.time || incoming.time ? { ...existing?.time, ...incoming.time } : undefined;
  if (time && time.created === undefined && existing?.time?.created !== undefined) {
    time.created = existing.time.created;
  }
  const incomingParts = incoming.parts ?? existing?.parts ?? [];
  const existingText = existing ? messageText(existing).trim() : "";
  const incomingText = messageText({ ...incoming, parts: incomingParts }).trim();
  const parts =
    existing && existingText && !incomingText
      ? mergePartsPreservingExistingText(existing.parts, incomingParts)
      : incomingParts;
  return {
    ...existing,
    ...incoming,
    created_at: incomingCreated ?? existingCreated,
    time,
    parts: orderMessagePartsForDisplay(parts),
  };
}

function mergePartsPreservingExistingText(
  existingParts: MessagePart[],
  incomingParts: MessagePart[],
): MessagePart[] {
  const existingTextParts = existingParts.filter(
    (part) =>
      (part.type === "text" || part.type === "message" || !part.type) &&
      !isInternalTaskStatusPart(part),
  );
  const incomingUsefulParts = incomingParts.filter((part) => !isInternalTaskStatusPart(part));
  const seen = new Set<string>();
  const merged: MessagePart[] = [];
  for (const part of [...existingTextParts, ...incomingUsefulParts]) {
    if (seen.has(part.id)) continue;
    seen.add(part.id);
    merged.push(part);
  }
  return merged.length ? merged : incomingParts;
}

function upsertPart(
  messages: Message[],
  part: MessagePart,
  sessionID: string | undefined,
): Message[] {
  const messageID = partMessageID(part) || messages.at(-1)?.id || `message:${part.id}`;
  let found = false;
  const next = messages.map((message) => {
    if (message.id !== messageID) return message;
    found = true;
    const hasPart = message.parts.some((item) => item.id === part.id);
    return {
      ...message,
      parts: orderMessagePartsForDisplay(
        hasPart
          ? message.parts.map((item) => (item.id === part.id ? part : item))
          : [...message.parts, part],
      ),
      updated_at: Date.now(),
    };
  });
  if (!found) {
    next.push({
      id: messageID,
      sessionID,
      role: "assistant",
      parts: orderMessagePartsForDisplay([part]),
      created_at: Date.now(),
      updated_at: Date.now(),
    });
  }
  next.sort((left, right) => messageSortValue(left) - messageSortValue(right));
  return next;
}

function applyPartDelta(
  streams: Record<string, LiveStream>,
  messages: Message[],
  messageID: string | undefined,
  partID: string | undefined,
  field: string | undefined,
  delta: string | undefined,
  sessionID: string | undefined,
): Record<string, LiveStream> {
  if (!messageID || !partID || delta === undefined || !["text", "content"].includes(field ?? ""))
    return streams;
  const textDelta = sanitizeStreamDelta(delta);
  if (!textDelta) return streams;
  const key = liveStreamKey(sessionID, messageID, partID);
  const existing = streams[key];
  return {
    ...streams,
    [key]: {
      sessionID,
      messageID,
      partID,
      field: field as "text" | "content",
      text: `${existing?.text ?? ""}${textDelta}`,
      createdAt: existing?.createdAt ?? streamedMessageCreatedAt(messages),
      updatedAt: Date.now(),
    },
  };
}

function applyLiveStream(messages: Message[], stream: LiveStream): Message[] {
  let foundMessage = false;
  let foundPart = false;
  const next = messages.map((message) => {
    if (message.id !== stream.messageID) return message;
    foundMessage = true;
    return {
      ...message,
      parts: message.parts.map((part) => {
        if (part.id !== stream.partID) return part;
        foundPart = true;
        const base = isInternalTaskStatusPart(part)
          ? ""
          : stream.field === "text"
            ? (part.text ?? "")
            : (part.content ?? "");
        if (stream.field === "text") return { ...part, text: `${base}${stream.text}` };
        return { ...part, content: `${base}${stream.text}` };
      }),
      updated_at: stream.updatedAt,
    };
  });
  if (foundMessage && !foundPart) {
    return next.map((message) => {
      if (message.id !== stream.messageID) return message;
      return {
        ...message,
        parts: orderMessagePartsForDisplay([...message.parts, liveStreamPart(stream)]),
        updated_at: stream.updatedAt,
      };
    });
  }
  if (!foundMessage) {
    next.push({
      id: stream.messageID,
      sessionID: stream.sessionID,
      role: "assistant",
      parts: [liveStreamPart(stream)],
      created_at: stream.createdAt,
      updated_at: stream.updatedAt,
    });
  }
  next.sort((left, right) => messageSortValue(left) - messageSortValue(right));
  return next;
}

function liveStreamPart(stream: LiveStream): MessagePart {
  return {
    id: stream.partID,
    sessionID: stream.sessionID,
    messageID: stream.messageID,
    type: "text",
    [stream.field]: stream.text,
  };
}

function liveStreamKey(sessionID: string | undefined, messageID: string, partID: string): string {
  return `${sessionID ?? ""}\u0000${messageID}\u0000${partID}`;
}

function clearLiveStreamsForDurableMessages(
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  messages: Message[],
): Record<string, LiveStream> {
  if (!messages.some((message) => message.role !== "user" && messageText(message).trim())) {
    return streams;
  }
  return filterLiveStreams(
    streams,
    (stream) => Boolean(sessionID) && Boolean(stream.sessionID) && stream.sessionID !== sessionID,
  );
}

function clearLiveStreamForPart(
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  part: MessagePart,
): Record<string, LiveStream> {
  const messageID = partMessageID(part);
  return filterLiveStreams(
    streams,
    (stream) =>
      stream.partID !== part.id ||
      (Boolean(messageID) && stream.messageID !== messageID) ||
      (Boolean(sessionID) && stream.sessionID !== sessionID),
  );
}

function clearLiveStreamsForMessageID(
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  messageID: string | undefined,
): Record<string, LiveStream> {
  if (!messageID) return streams;
  return filterLiveStreams(
    streams,
    (stream) =>
      stream.messageID !== messageID || (Boolean(sessionID) && stream.sessionID !== sessionID),
  );
}

function filterLiveStreams(
  streams: Record<string, LiveStream>,
  keep: (stream: LiveStream) => boolean,
): Record<string, LiveStream> {
  let changed = false;
  const entries = Object.entries(streams).filter(([, stream]) => {
    const include = keep(stream);
    if (!include) changed = true;
    return include;
  });
  return changed ? Object.fromEntries(entries) : streams;
}

function orderMessagePartsForDisplay(parts: MessagePart[]): MessagePart[] {
  return [...parts].sort(partDisplayComparator);
}

function partDisplayComparator(left: MessagePart, right: MessagePart): number {
  return partDisplayRank(left) - partDisplayRank(right);
}

function partDisplayRank(part: MessagePart): number {
  if (part.type === "text" || part.type === "message" || !part.type) return 0;
  if (part.tool || part.type === "tool") return 2;
  return 1;
}

function streamedMessageCreatedAt(messages: Message[]): number {
  const runningAssistant = latestRunningAssistantSort(messages);
  if (Number.isFinite(runningAssistant)) return runningAssistant + 0.5;
  let lastUser = Number.NEGATIVE_INFINITY;
  let latestAfterUser = Number.NEGATIVE_INFINITY;
  let visibleAssistantAfterUser = false;
  for (const message of messages) {
    const sort = messageSortValue(message);
    if (message.role === "user") lastUser = Math.max(lastUser, sort);
  }
  for (const message of messages) {
    const sort = messageSortValue(message);
    if (sort <= lastUser) continue;
    latestAfterUser = Math.max(latestAfterUser, sort);
    if (message.role === "assistant" && messageText(message).trim()) {
      visibleAssistantAfterUser = true;
    }
  }
  if (visibleAssistantAfterUser && Number.isFinite(latestAfterUser)) {
    return latestAfterUser + 0.5;
  }
  return Number.isFinite(lastUser) ? lastUser + 0.5 : Date.now();
}

function latestRunningAssistantSort(messages: Message[]): number {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message?.role === "assistant" && messageHasRunningPart(message)) {
      return messageSortValue(message);
    }
  }
  return Number.NEGATIVE_INFINITY;
}

function messageHasRunningPart(message: Message): boolean {
  return (message.parts ?? []).some((part) => partIsRunning(part));
}

function partIsRunning(part: MessagePart): boolean {
  if (part.tool !== "command_run" && part.type !== "tool") return false;
  const state =
    part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : "";
  return /run|progress|pending|busy|question|in[_ -]?progress|execut|start/i.test(status);
}

function sanitizeStreamDelta(value: string): string {
  return value.replace(/\r\n/g, "\n").replace(/\r/g, "\n").replace(rawAnsiControlPattern, "");
}

function readString(
  properties: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = properties?.[key];
  return typeof value === "string" ? value : undefined;
}

function upsertById<T extends { id: string }>(items: T[], item: T): T[] {
  return [...items.filter((existing) => existing.id !== item.id), item];
}

function selectedSessionIndex(sessions: Session[], sessionID: string | undefined): number {
  const index = sessions.findIndex((session) => session.id === sessionID);
  return index >= 0 ? index : 0;
}

function selectedPersonaIndex(
  personas: StoredPersona[],
  agents: StoredAgent[],
  session: Session | undefined,
  config: SessionConfig | undefined,
): number {
  const active = activePersonaID(agents, session, config);
  if (!active) return 0;
  const index = personas.findIndex((persona) => personaID(persona) === active);
  return index >= 0 ? index : 0;
}

function personaID(persona: StoredPersona): string | undefined {
  const configName = persona.config?.persona_name;
  return persona.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

function activePersonaID(
  agents: StoredAgent[],
  session: Session | undefined,
  config: SessionConfig | undefined,
): string | undefined {
  const agentID = session?.agent ?? config?.active_agent;
  const agent = agents.find((item) => storedAgentID(item) === agentID);
  const first = Array.isArray(agent?.config?.agent_persona)
    ? agent?.config?.agent_persona[0]
    : undefined;
  if (!first || typeof first !== "object" || Array.isArray(first)) return undefined;
  const name = (first as Record<string, unknown>).persona_name;
  if (typeof name === "string" && name.trim()) return name.trim();
  const runtimePersonas = (agent as unknown as { options?: { personas?: StoredPersona[] } }).options
    ?.personas;
  return runtimePersonas?.[0] ? personaID(runtimePersonas[0]) : undefined;
}

function storedAgentID(agent: StoredAgent): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

function wrapIndex(index: number, length: number): number {
  if (length <= 0) return 0;
  return ((index % length) + length) % length;
}

function modelCount(providers: ProviderListResponse | undefined): number {
  return (
    providers?.all.reduce(
      (count, provider) => count + Object.keys(provider.models ?? {}).length,
      0,
    ) ?? 0
  );
}

function settingProviderCount(providers: ProviderListResponse | undefined): number {
  return settingProviders(providers).length;
}

function settingProviders(
  providers: ProviderListResponse | undefined,
): ProviderListResponse["all"] {
  return (providers?.all ?? []).filter(isLlmProvider);
}

function isLlmProvider(provider: ProviderListResponse["all"][number]): boolean {
  const domains = stringArrayField(provider.options, "domains");
  if (domains.length) return domains.some((domain) => domain.toLowerCase() === "llm");
  const capabilities = stringArrayField(provider.options, "capabilities");
  if (capabilities.some((capability) => capability.toLowerCase().startsWith("llm."))) return true;
  return Object.keys(provider.models ?? {}).length > 0;
}

function stringArrayField(value: Record<string, unknown> | undefined, key: string): string[] {
  const item = value?.[key];
  return Array.isArray(item)
    ? item.filter((entry): entry is string => typeof entry === "string")
    : [];
}
