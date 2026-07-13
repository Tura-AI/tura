import type { CommandUpdatedEventProperties } from "../types/event.js";
import type { Message, MessagePart, Session } from "../types/session.js";
import { normalizeEvent } from "../gateway/events.js";
import { sameDirectory } from "../gateway/directory.js";
import { partMessageID, sessionSortAt, sessionStatusText } from "../types/session.js";
import type { AppAction, AppState, LiveStream } from "./reducer/state.js";
import {
  boundedSessionIndex,
  modelCount,
  seedSeenSessionCounts,
  selectedPersonaIndex,
  selectedSettingOptionIndex,
  selectedSessionIndex,
  SESSION_CREATE_ENTRY_COUNT,
  settingOptionCount,
  settingsEntryCount,
  sortSessions,
  upsertById,
  upsertSession,
  wrapIndex,
} from "./reducer/navigation.js";
import {
  applyPartDelta,
  applyCommandUpdate,
  clearLiveStreamsForMessageID,
  invalidateRefreshState,
  lastMessagePreview,
  appendNewStableMessagesIgnoringLive,
  commitLiveStreams,
  messageHasRunningPart,
  messagePreview,
  prepareMessagesForDisplay,
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
  SessionLoadingState,
  SettingDetail,
  SettingInputKind,
  SettingInputState,
} from "./reducer/state.js";

export function reducer(state: AppState, action: AppAction): AppState {
  if (action.type === "hydrate") {
    const sessionID = action.session.id;
    const sessionChanged = Boolean(state.session && state.session.id !== sessionID);
    const nextSessions = sortSessions(action.sessions ?? state.sessions);
    const hydratedMessages = prepareMessagesForDisplay(action.messages);
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
    const nextState = {
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
      modelConfig: action.modelConfig ?? state.modelConfig,
      sessionLoading: undefined,
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
    return clearTransientNotice(nextState);
  }
  if (action.type === "session-loading") return { ...state, sessionLoading: action.value };
  if (state.sessionLoading && locksDuringSessionLoading(action)) return state;
  if (action.type === "event") {
    const normalized = normalizeEvent(action.event);
    if (normalized.directory !== "global" && !sameDirectory(normalized.directory, state.cwd))
      return state;
    if (action.event.payload?.type === "server.connected") {
      return clearTransientNotice(state);
    }
    if (action.event.payload?.type === "message.updated") {
      const message = (action.event.payload.properties as { info?: Message } | undefined)?.info;
      if (!message) return state;
      const sessionID = normalized.sessionID || message.sessionID;
      if (state.session && sessionID && sessionID !== state.session.id) {
        const preview = messagePreview(message) ?? state.sessionPreviews[sessionID] ?? "";
        return {
          ...state,
          sessionPreviews: {
            ...state.sessionPreviews,
            [sessionID]: preview,
          },
          refreshState: refreshStateAfterBackgroundMessage(state.refreshState, sessionID, message),
          sessions: updateSessionForMessage(state.sessions, sessionID, message),
        };
      }
      const updated = upsertMessageIgnoringLive(
        state.messages,
        state.liveStreams,
        sessionID,
        message,
      );
      const matchingLiveMessageIDs = liveStreamMessageIDsMatchingMessage(
        updated.liveStreams,
        sessionID,
        message,
      );
      const currentMessage = updated.messages.find((item) => item.id === message.id);
      const messageFinished =
        !messageHasRunningPart(message) &&
        !(currentMessage && messageHasRunningPart(currentMessage));
      const committed =
        messageFinished && matchingLiveMessageIDs.length
          ? commitLiveStreams(updated.messages, updated.liveStreams, sessionID, (stream) =>
              matchingLiveMessageIDs.includes(stream.messageID),
            )
          : updated;
      return {
        ...state,
        messages: committed.messages,
        liveStreams: committed.liveStreams,
        refreshState: refreshStateAfterMessages(
          state.refreshState,
          sessionID,
          committed.messages,
          state.session,
        ),
      };
    }
    if (action.event.payload?.type === "message.part.updated") {
      const properties = action.event.payload.properties as
        | { part?: MessagePart; createdAt?: number; updatedAt?: number }
        | undefined;
      const part = properties?.part;
      if (!part) return state;
      const sessionID = normalized.sessionID ?? part.sessionID;
      if (state.session && sessionID !== state.session.id) return state;
      const updated = upsertPartIgnoringLive(
        state.messages,
        state.liveStreams,
        sessionID,
        part,
        properties?.createdAt,
        properties?.updatedAt,
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
    if (action.event.payload?.type === "message.part.delta") {
      const properties = action.event.payload.properties as
        | {
            messageID?: string;
            partID?: string;
            createdAt?: number;
            updatedAt?: number;
            field?: string;
            delta?: string;
          }
        | undefined;
      const sessionID = normalized.sessionID;
      if (state.session && sessionID !== state.session.id) return state;
      const activeSession = Boolean(
        state.session && (!sessionID || state.session.id === sessionID),
      );
      const committed = commitLiveStreams(
        state.messages,
        state.liveStreams,
        sessionID,
        (stream) => stream.messageID !== properties?.messageID,
      );
      return {
        ...state,
        messages: committed.messages,
        status: activeSession ? "busy" : state.status,
        session:
          activeSession && state.session ? { ...state.session, status: "busy" } : state.session,
        sessions: sessionID
          ? state.sessions.map((session) =>
              session.id === sessionID ? { ...session, status: "busy" } : session,
            )
          : state.sessions,
        liveStreams: applyPartDelta(
          committed.liveStreams,
          properties?.messageID,
          properties?.partID,
          properties?.field,
          properties?.delta,
          sessionID,
          properties?.createdAt,
          properties?.updatedAt,
        ),
      };
    }
    if (action.event.payload?.type === "command.updated") {
      const properties = action.event.payload.properties as
        | CommandUpdatedEventProperties
        | undefined;
      const sessionID = normalized.sessionID;
      if (!properties) return state;
      if (!sessionID) return state;
      const commandStatesBySession = updateCommandEventState(
        state.commandStatesBySession,
        sessionID,
        properties,
      );
      if (state.session && sessionID !== state.session.id) {
        return { ...state, commandStatesBySession };
      }
      const messages = applyCommandUpdate(state.messages, sessionID, properties);
      return {
        ...state,
        commandStatesBySession,
        messages,
        refreshState: refreshStateAfterMessages(
          state.refreshState,
          sessionID,
          messages,
          state.session,
        ),
      };
    }
    if (action.event.payload?.type === "message.removed") {
      const properties = action.event.payload.properties as { messageID?: string } | undefined;
      if (state.session && normalized.sessionID && normalized.sessionID !== state.session.id)
        return state;
      return {
        ...state,
        messages: state.messages.filter((message) => message.id !== properties?.messageID),
        liveStreams: clearLiveStreamsForMessageID(
          state.liveStreams,
          normalized.sessionID,
          properties?.messageID,
        ),
        refreshState: invalidateRefreshState(state.refreshState, normalized.sessionID),
      };
    }
    if (action.event.payload?.type === "session.status") {
      const properties = action.event.payload.properties as
        | {
            sessionID?: string;
            updatedAt?: number;
            status?: unknown;
            context_tokens?: Session["context_tokens"];
            usage?: Session["usage"];
          }
        | undefined;
      if (properties?.updatedAt === undefined) return state;
      const status = sessionStatusText(properties?.status);
      const sessionID = properties?.sessionID;
      const activeSession = Boolean(
        state.session && (!sessionID || state.session.id === sessionID),
      );
      const committed =
        activeSession && status === "idle"
          ? commitLiveStreams(state.messages, state.liveStreams, state.session?.id)
          : { messages: state.messages, liveStreams: state.liveStreams };
      return {
        ...state,
        messages: committed.messages,
        liveStreams: committed.liveStreams,
        status: state.session?.id === sessionID || !sessionID ? status : state.status,
        sessions: sessionID
          ? state.sessions.map((session) =>
              session.id === sessionID
                ? sessionWithUsage(
                    { ...session, status, updated_at: properties.updatedAt },
                    properties?.usage,
                    properties?.context_tokens,
                  )
                : session,
            )
          : state.sessions,
        session:
          activeSession && state.session
            ? sessionWithUsage(
                { ...state.session, status, updated_at: properties.updatedAt },
                properties?.usage,
                properties?.context_tokens,
              )
            : state.session,
        refreshState: activeSession
          ? refreshStateAfterMessages(
              state.refreshState,
              state.session?.id,
              committed.messages,
              state.session,
            )
          : state.refreshState,
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
      const properties = action.event.payload.properties as { sessionID?: string } | undefined;
      const sessionID = properties?.sessionID;
      if (sessionID) {
        const { [sessionID]: _removed, ...commandStatesBySession } = state.commandStatesBySession;
        return {
          ...state,
          sessions: state.sessions.filter((session) => session.id !== sessionID),
          commandStatesBySession,
        };
      }
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
    const incoming = prepareMessagesForDisplay(action.messages);
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
    const nextState = {
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
    return nextState;
  }
  if (action.type === "composer") return { ...state, composer: action.value };
  if (action.type === "notice") {
    return {
      ...state,
      notice: action.value,
      noticeTransient: action.value ? Boolean(action.transient) : undefined,
    };
  }
  if (action.type === "status") return { ...state, status: action.value };
  if (action.type === "permissions") return { ...state, permissions: action.value };
  if (action.type === "questions") return { ...state, questions: action.value };
  if (action.type === "sessions") {
    const keepSelection = state.sessionsOpen && action.open;
    const sessions = sortSessions(action.value);
    return {
      ...state,
      sessions,
      seenSessionMessageCounts: seedSeenSessionCounts(
        state.seenSessionMessageCounts,
        sessions,
        state.session?.id,
      ),
      sessionsOpen: action.open ?? state.sessionsOpen,
      selectedSessionIndex: keepSelection
        ? boundedSessionIndex(state.selectedSessionIndex, sessions)
        : selectedSessionIndex(sessions, state.session?.id),
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
    const nextState = {
      ...state,
      sessionConfig: action.value,
      modelConfig: action.modelConfig ?? state.modelConfig,
      settingsOpen: action.open ?? state.settingsOpen,
      settingDetail: action.open ? undefined : state.settingDetail,
      selectedProviderID: action.open ? undefined : state.selectedProviderID,
      sessionsOpen: false,
      modelsOpen: false,
      authOpen: false,
      personasOpen: false,
    };
    return {
      ...nextState,
      selectedSettingOptionIndex: nextState.settingDetail
        ? selectedSettingOptionIndex(nextState, nextState.settingDetail)
        : state.selectedSettingOptionIndex,
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
    const nextState = {
      ...state,
      settingsOpen: true,
      settingDetail: action.detail,
      selectedProviderID: action.providerID ?? state.selectedProviderID,
      settingInput: undefined,
      sessionsOpen: false,
      modelsOpen: false,
      authOpen: false,
      personasOpen: false,
    };
    return {
      ...nextState,
      selectedSettingOptionIndex: selectedSettingOptionIndex(nextState, action.detail),
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

function updateSessionForMessage(
  sessions: Session[],
  sessionID: string,
  message: Message,
): Session[] {
  let changed = false;
  const next = sessions.map((session) => {
    if (session.id !== sessionID) return session;
    changed = true;
    const timestamp = messageTime(message);
    return {
      ...session,
      message_count: (session.message_count ?? 0) + 1,
      updated_at: timestamp ?? session.updated_at,
      ...(message.role === "user" && timestamp !== undefined
        ? { last_user_message_at: Math.max(session.last_user_message_at ?? 0, timestamp) }
        : {}),
    };
  });
  return changed && message.role === "user"
    ? next.sort((left, right) => sessionSortAt(right) - sessionSortAt(left))
    : next;
}

function updateCommandEventState(
  states: AppState["commandStatesBySession"],
  sessionID: string,
  update: CommandUpdatedEventProperties,
): AppState["commandStatesBySession"] {
  const sessionStates = states[sessionID] ?? {};
  const previous = sessionStates[update.commandID];
  const eventSeq = update.eventSeq ?? undefined;
  const updatedAt = update.updatedAt ?? undefined;
  if (
    (previous?.eventSeq !== undefined && eventSeq !== undefined && eventSeq < previous.eventSeq) ||
    (previous?.updatedAt !== undefined && updatedAt !== undefined && updatedAt < previous.updatedAt)
  ) {
    return states;
  }
  return {
    ...states,
    [sessionID]: {
      ...sessionStates,
      [update.commandID]: { status: update.status, eventSeq, updatedAt },
    },
  };
}

function messageTime(message: Message): number | undefined {
  return message.updated_at ?? message.created_at ?? message.time?.updated ?? message.time?.created;
}

function clearTransientNotice(state: AppState): AppState {
  return state.noticeTransient
    ? { ...state, notice: undefined, noticeTransient: undefined }
    : state;
}

function locksDuringSessionLoading(action: AppAction): boolean {
  return !["tick", "event", "notice", "session-previews", "messages-incremental"].includes(
    action.type,
  );
}

function sessionWithUsage(
  session: Session,
  usage: Session["usage"] | undefined,
  contextTokens: Session["context_tokens"] | undefined,
): Session {
  const usageContextTokens = usage?.context_tokens ?? undefined;
  const nextContextTokens = usageContextTokens ?? contextTokens;
  if (!usage && !nextContextTokens) return session;
  return {
    ...session,
    ...(usage ? { usage } : {}),
    ...(nextContextTokens ? { context_tokens: nextContextTokens } : {}),
  };
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

function liveStreamMessageIDsMatchingMessage(
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  message: Message,
): string[] {
  const partIDs = new Set((message.parts ?? []).map((part) => part.id));
  const partMessageIDs = new Set(
    (message.parts ?? []).map((part) => partMessageID(part)).filter(Boolean),
  );
  return unique(
    Object.values(streams)
      .filter(
        (stream) =>
          liveStreamMatchesSession(stream, sessionID) &&
          (stream.messageID === message.id ||
            partIDs.has(stream.partID) ||
            partMessageIDs.has(stream.messageID)),
      )
      .map((stream) => stream.messageID),
  );
}

function liveStreamMatchesSession(stream: LiveStream, sessionID: string | undefined): boolean {
  return !sessionID || !stream.sessionID || stream.sessionID === sessionID;
}

function unique(values: string[]): string[] {
  return [...new Set(values)];
}
