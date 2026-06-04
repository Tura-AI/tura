import type { GatewayEventEnvelope } from "../types/event.js";
import type { Message, MessagePart, Session } from "../types/session.js";
import { normalizeEvent } from "../gateway/events.js";
import { sameDirectory } from "../gateway/directory.js";
import { messageSortValue, partMessageID, sessionStatusText, sessionUpdatedAt } from "../types/session.js";
import type { PermissionRequest, QuestionRequest } from "../types/permission.js";
import type { ProviderAuthMethodsResponse, ProviderAuthStatus, ProviderListResponse } from "../types/provider.js";
import type { SessionConfig } from "../types/config.js";
import type { StoredAgent } from "../types/agent.js";
import type { StoredPersona } from "../types/gateway.js";

export interface AppState {
  cwd: string;
  session?: Session;
  sessions: Session[];
  messages: Message[];
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
  personasOpen: boolean;
  selectedSessionIndex: number;
  selectedModelIndex: number;
  selectedPersonaIndex: number;
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
    }
  | { type: "event"; event: GatewayEventEnvelope }
  | { type: "composer"; value: string }
  | { type: "notice"; value?: string }
  | { type: "status"; value: AppState["status"] }
  | { type: "permissions"; value: PermissionRequest[] }
  | { type: "questions"; value: QuestionRequest[] }
  | { type: "sessions"; value: Session[]; open?: boolean }
  | { type: "auth"; methods?: ProviderAuthMethodsResponse; statuses?: Record<string, ProviderAuthStatus>; open?: boolean }
  | { type: "agents"; value: StoredAgent[] }
  | { type: "session-config"; value: SessionConfig; open?: boolean }
  | { type: "personas"; value: StoredPersona[]; open?: boolean }
  | { type: "select-session"; delta: number }
  | { type: "select-model"; delta: number }
  | { type: "select-persona"; delta: number }
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
    personasOpen: false,
    selectedSessionIndex: 0,
    selectedModelIndex: 0,
    selectedPersonaIndex: 0,
  };
}

export function reducer(state: AppState, action: AppAction): AppState {
  if (action.type === "hydrate") {
    return {
      ...state,
      session: action.session,
      sessions: action.sessions ?? state.sessions,
      messages: action.messages,
      permissions: action.permissions,
      questions: state.questions,
      providers: action.providers,
      agents: action.agents ?? state.agents,
      personas: action.personas ?? state.personas,
      authMethods: action.authMethods ?? state.authMethods,
      authStatuses: action.authStatuses ?? state.authStatuses,
      sessionConfig: action.sessionConfig ?? state.sessionConfig,
      status: action.session.status ?? "idle",
      selectedSessionIndex: selectedSessionIndex(action.sessions ?? state.sessions, action.session.id),
      selectedPersonaIndex: selectedPersonaIndex(action.personas ?? state.personas, action.agents ?? state.agents, action.session ?? state.session, action.sessionConfig ?? state.sessionConfig),
    };
  }
  if (action.type === "event") {
    const normalized = normalizeEvent(action.event);
    if (normalized.directory !== "global" && !sameDirectory(normalized.directory, state.cwd)) return state;
    if (state.session && normalized.sessionID && normalized.sessionID !== state.session.id) return state;
    if (action.event.payload?.type === "message.updated") {
      const message = (action.event.payload.properties as { info?: Message } | undefined)?.info;
      if (!message) return state;
      return { ...state, messages: upsertMessage(state.messages, message) };
    }
    if (action.event.payload?.type === "message.part.updated") {
      const part = (action.event.payload.properties as { part?: MessagePart } | undefined)?.part;
      if (!part) return state;
      return { ...state, messages: upsertPart(state.messages, part, normalized.sessionID) };
    }
    if (action.event.payload?.type === "message.part.delta") {
      const properties = action.event.payload.properties as Record<string, unknown> | undefined;
      return {
        ...state,
        messages: applyPartDelta(
          state.messages,
          readString(properties, "message_id") ?? readString(properties, "messageID"),
          readString(properties, "part_id") ?? readString(properties, "partID"),
          readString(properties, "field"),
          readString(properties, "delta"),
          normalized.sessionID,
        ),
      };
    }
    if (action.event.payload?.type === "message.removed") {
      const properties = action.event.payload.properties as { message_id?: string } | undefined;
      return { ...state, messages: state.messages.filter((message) => message.id !== properties?.message_id) };
    }
    if (action.event.payload?.type === "session.status") {
      const properties = action.event.payload.properties as Record<string, unknown> | undefined;
      const status = sessionStatusText(properties?.status);
      const sessionID = readString(properties, "sessionID") ?? readString(properties, "session_id");
      return {
        ...state,
        status: state.session?.id === sessionID || !sessionID ? status : state.status,
        sessions: sessionID
          ? state.sessions.map((session) => (session.id === sessionID ? { ...session, status } : session))
          : state.sessions,
        session: state.session && state.session.id === sessionID ? { ...state.session, status } : state.session,
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
      if (sessionID) return { ...state, sessions: state.sessions.filter((session) => session.id !== sessionID) };
    }
    if (action.event.payload?.type === "permission.asked" && normalized.permission) {
      return { ...state, permissions: upsertById(state.permissions, normalized.permission) };
    }
    if (action.event.payload?.type === "permission.replied" && normalized.permission) {
      return { ...state, permissions: state.permissions.filter((permission) => permission.id !== normalized.permission?.id) };
    }
    if (action.event.payload?.type === "question.asked" && normalized.question) {
      return { ...state, questions: upsertById(state.questions, normalized.question) };
    }
    if ((action.event.payload?.type === "question.replied" || action.event.payload?.type === "question.rejected") && normalized.question) {
      return { ...state, questions: state.questions.filter((question) => question.id !== normalized.question?.id) };
    }
    return state;
  }
  if (action.type === "composer") return { ...state, composer: action.value };
  if (action.type === "notice") return { ...state, notice: action.value };
  if (action.type === "status") return { ...state, status: action.value };
  if (action.type === "permissions") return { ...state, permissions: action.value };
  if (action.type === "questions") return { ...state, questions: action.value };
  if (action.type === "sessions") {
    return {
      ...state,
      sessions: action.value,
      sessionsOpen: action.open ?? state.sessionsOpen,
      selectedSessionIndex: selectedSessionIndex(action.value, state.session?.id),
    };
  }
  if (action.type === "auth") {
    return {
      ...state,
      authMethods: action.methods ?? state.authMethods,
      authStatuses: action.statuses ?? state.authStatuses,
      authOpen: action.open ?? state.authOpen,
      sessionsOpen: false,
      modelsOpen: false,
      settingsOpen: false,
      personasOpen: false,
    };
  }
  if (action.type === "agents") return { ...state, agents: action.value };
  if (action.type === "session-config") {
    return {
      ...state,
      sessionConfig: action.value,
      settingsOpen: action.open ?? state.settingsOpen,
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
      selectedPersonaIndex: selectedPersonaIndex(action.value, state.agents, state.session, state.sessionConfig),
    };
  }
  if (action.type === "select-session") {
    return { ...state, selectedSessionIndex: clampIndex(state.selectedSessionIndex + action.delta, state.sessions.length) };
  }
  if (action.type === "select-model") {
    return { ...state, selectedModelIndex: clampIndex(state.selectedModelIndex + action.delta, modelCount(state.providers)) };
  }
  if (action.type === "select-persona") {
    return { ...state, selectedPersonaIndex: clampIndex(state.selectedPersonaIndex + action.delta, state.personas.length) };
  }
  if (action.type === "toggle-help") return { ...state, help: !state.help };
  if (action.type === "toggle-sessions") return { ...state, sessionsOpen: !state.sessionsOpen, modelsOpen: false, authOpen: false, settingsOpen: false, personasOpen: false };
  if (action.type === "toggle-models") return { ...state, modelsOpen: !state.modelsOpen, sessionsOpen: false, authOpen: false, settingsOpen: false, personasOpen: false };
  if (action.type === "toggle-auth") return { ...state, authOpen: !state.authOpen, sessionsOpen: false, modelsOpen: false, settingsOpen: false, personasOpen: false };
  if (action.type === "toggle-settings") return { ...state, settingsOpen: !state.settingsOpen, sessionsOpen: false, modelsOpen: false, authOpen: false, personasOpen: false };
  if (action.type === "toggle-personas") return { ...state, personasOpen: !state.personasOpen, sessionsOpen: false, modelsOpen: false, authOpen: false, settingsOpen: false };
  if (action.type === "close-panels") return { ...state, sessionsOpen: false, modelsOpen: false, authOpen: false, settingsOpen: false, personasOpen: false, help: false };
  return state;
}

function upsertSession(sessions: Session[], session: Session): Session[] {
  const next = sessions.filter((item) => item.id !== session.id);
  next.push(session);
  next.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  return next;
}

function upsertMessage(messages: Message[], message: Message): Message[] {
  const next = messages.filter((item) => item.id !== message.id);
  next.push(message);
  next.sort((left, right) => messageSortValue(left) - messageSortValue(right));
  return next;
}

function upsertPart(messages: Message[], part: MessagePart, sessionID: string | undefined): Message[] {
  const messageID = partMessageID(part) || messages.at(-1)?.id || `message:${part.id}`;
  let found = false;
  const next = messages.map((message) => {
    if (message.id !== messageID) return message;
    found = true;
    return {
      ...message,
      parts: [...message.parts.filter((item) => item.id !== part.id), part],
      updated_at: Date.now(),
    };
  });
  if (!found) {
    next.push({
      id: messageID,
      sessionID,
      role: "assistant",
      parts: [part],
      created_at: Date.now(),
      updated_at: Date.now(),
    });
  }
  next.sort((left, right) => messageSortValue(left) - messageSortValue(right));
  return next;
}

function applyPartDelta(
  messages: Message[],
  messageID: string | undefined,
  partID: string | undefined,
  field: string | undefined,
  delta: string | undefined,
  sessionID: string | undefined,
): Message[] {
  if (!messageID || !partID || delta === undefined || !["text", "content"].includes(field ?? "")) return messages;
  let foundMessage = false;
  let foundPart = false;
  const next = messages.map((message) => {
    if (message.id !== messageID) return message;
    foundMessage = true;
    return {
      ...message,
      parts: message.parts.map((part) => {
        if (part.id !== partID) return part;
        foundPart = true;
        if (field === "text") return { ...part, text: `${part.text ?? ""}${delta}` };
        if (field === "content") return { ...part, content: `${part.content ?? ""}${delta}` };
        return part;
      }),
      updated_at: Date.now(),
    };
  });
  if (foundMessage && !foundPart) {
    return next.map((message) => {
      if (message.id !== messageID) return message;
      return {
        ...message,
        parts: [
          ...message.parts,
          { id: partID, sessionID, messageID, type: "text", [field as "text" | "content"]: delta },
        ],
        updated_at: Date.now(),
      };
    });
  }
  if (!foundMessage) {
    next.push({
      id: messageID,
      sessionID,
      role: "assistant",
      parts: [{ id: partID, sessionID, messageID, type: "text", [field as "text" | "content"]: delta }],
      created_at: Date.now(),
      updated_at: Date.now(),
    });
  }
  next.sort((left, right) => messageSortValue(left) - messageSortValue(right));
  return next;
}

function readString(properties: Record<string, unknown> | undefined, key: string): string | undefined {
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

function selectedPersonaIndex(personas: StoredPersona[], agents: StoredAgent[], session: Session | undefined, config: SessionConfig | undefined): number {
  const active = activePersonaID(agents, session, config);
  if (!active) return 0;
  const index = personas.findIndex((persona) => personaID(persona) === active);
  return index >= 0 ? index : 0;
}

function personaID(persona: StoredPersona): string | undefined {
  const configName = persona.config?.persona_name;
  return persona.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

function activePersonaID(agents: StoredAgent[], session: Session | undefined, config: SessionConfig | undefined): string | undefined {
  const agentID = session?.agent ?? config?.active_agent;
  const agent = agents.find((item) => storedAgentID(item) === agentID);
  const first = Array.isArray(agent?.config?.agent_persona) ? agent?.config?.agent_persona[0] : undefined;
  if (!first || typeof first !== "object" || Array.isArray(first)) return undefined;
  const name = (first as Record<string, unknown>).persona_name;
  if (typeof name === "string" && name.trim()) return name.trim();
  const runtimePersonas = (agent as unknown as { options?: { personas?: StoredPersona[] } }).options?.personas;
  return runtimePersonas?.[0] ? personaID(runtimePersonas[0]) : undefined;
}

function storedAgentID(agent: StoredAgent): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

function clampIndex(index: number, length: number): number {
  if (length <= 0) return 0;
  return Math.max(0, Math.min(length - 1, index));
}

function modelCount(providers: ProviderListResponse | undefined): number {
  return providers?.all.reduce((count, provider) => count + Object.keys(provider.models ?? {}).length, 0) ?? 0;
}
