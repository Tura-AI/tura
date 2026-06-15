import type { Message, MessagePart, Session } from "../types/session.js";
import { normalizeEvent } from "../gateway/events.js";
import { sameDirectory } from "../gateway/directory.js";
import { partSessionID, sessionStatusText } from "../types/session.js";
import type { AppAction, AppState } from "./reducer/state.js";
import {
  boundedSessionIndex,
  modelCount,
  readString,
  seedSeenSessionCounts,
  selectedPersonaIndex,
  selectedSessionIndex,
  selectedSettingOptionIndex,
  SESSION_CREATE_ENTRY_COUNT,
  settingOptionCount,
  settingsEntryCount,
  upsertById,
  upsertSession,
  wrapIndex,
} from "./reducer/navigation.js";
import {
  applyPartDelta,
  clearLiveStreamsForMessageID,
  invalidateRefreshState,
  lastMessagePreview,
  appendNewStableMessagesIgnoringLive,
  mergeStableMessagesIgnoringLive,
  messagePreview,
  normalizeMessagesForDisplay,
  refreshStateAfterBackgroundMessage,
  refreshStateAfterMessages,
  updatePreviewForMessages,
  upsertMessageIgnoringLive,
  upsertPartIgnoringLive,
} from "./reducer/messages.js";

export { displayMessages } from "./reducer/messages.js";
export { initialState } from "./reducer/state.js";
export type {
  AppAction,
  AppState,
  LiveStream,
  RefreshSessionState,
  SettingDetail,
  SettingInputKind,
  SettingInputState,
} from "./reducer/state.js";

export function reducer(state: AppState, action: AppAction): AppState {
  if (action.type === "hydrate") {
    const sessionID = action.session.id;
    const sessionChanged = Boolean(state.session && state.session.id !== sessionID);
    const nextSessions = action.sessions ?? state.sessions;
    const hydratedMessages = normalizeMessagesForDisplay(action.messages);
    const merged = sessionChanged
      ? { messages: hydratedMessages, liveStreams: {} }
      : appendNewStableMessagesIgnoringLive(
          state.messages,
          hydratedMessages,
          state.liveStreams,
          sessionID,
        );
    const currentPreview = lastMessagePreview(hydratedMessages);
    const panelState = action.closePanels ? closedPanelsState() : {};
    return {
      ...state,
      ...panelState,
      session: action.session,
      sessions: nextSessions,
      messages: merged.messages,
      liveStreams: merged.liveStreams,
      refreshState: refreshStateAfterMessages(
        state.refreshState,
        sessionID,
        merged.messages,
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
      const updated = upsertMessageIgnoringLive(
        state.messages,
        state.liveStreams,
        sessionID,
        message,
      );
      return {
        ...state,
        messages: updated.messages,
        liveStreams: updated.liveStreams,
        refreshState: refreshStateAfterMessages(
          state.refreshState,
          sessionID,
          updated.messages,
          state.session,
        ),
      };
    }
    if (action.event.payload?.type === "message.part.updated") {
      const part = (action.event.payload.properties as { part?: MessagePart } | undefined)?.part;
      if (!part) return state;
      const sessionID = normalized.sessionID ?? partSessionID(part);
      if (state.session && sessionID !== state.session.id) return state;
      const updated = upsertPartIgnoringLive(state.messages, state.liveStreams, sessionID, part);
      return {
        ...state,
        messages: updated.messages,
        liveStreams: updated.liveStreams,
        refreshState: refreshStateAfterMessages(
          state.refreshState,
          sessionID,
          updated.messages,
          state.session,
        ),
      };
    }
    if (action.event.payload?.type === "message.part.delta") {
      const properties = action.event.payload.properties as Record<string, unknown> | undefined;
      const sessionID = normalized.sessionID;
      if (state.session && sessionID !== state.session.id) return state;
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
      const activeSession = Boolean(
        state.session && (!sessionID || state.session.id === sessionID),
      );
      return {
        ...state,
        status: state.session?.id === sessionID || !sessionID ? status : state.status,
        sessions: sessionID
          ? state.sessions.map((session) =>
              session.id === sessionID ? { ...session, status } : session,
            )
          : state.sessions,
        session: activeSession && state.session ? { ...state.session, status } : state.session,
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
    const updated = appendNewStableMessagesIgnoringLive(
      state.messages,
      incoming,
      state.liveStreams,
      sessionID,
    );
    return {
      ...state,
      session: action.session ?? state.session,
      messages: updated.messages,
      liveStreams: updated.liveStreams,
      sessionPreviews: updatePreviewForMessages(state.sessionPreviews, sessionID, updated.messages),
      refreshState: refreshStateAfterMessages(
        state.refreshState,
        sessionID,
        updated.messages,
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
        settingsEntryCount(state),
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
