import type { GatewayEventEnvelope } from "../types/event.js";
import type { Message, MessagePart, Session, TodoItem } from "../types/session.js";
import { normalizeEvent } from "../gateway/events.js";
import { sameDirectory } from "../gateway/directory.js";
import { messageSortValue, partMessageID, sessionStatusText, sessionUpdatedAt } from "../types/session.js";
import type { PermissionRequest, QuestionRequest } from "../types/permission.js";
import type { ProviderListResponse } from "../types/provider.js";

export interface AppState {
  cwd: string;
  session?: Session;
  sessions: Session[];
  messages: Message[];
  todos: TodoItem[];
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  providers?: ProviderListResponse;
  status: "idle" | "busy" | "error";
  composer: string;
  notice?: string;
  help: boolean;
  sessionsOpen: boolean;
  modelsOpen: boolean;
  diffOpen: boolean;
  diffText: string;
  selectedSessionIndex: number;
  selectedModelIndex: number;
}

export type AppAction =
  | { type: "hydrate"; session: Session; messages: Message[]; todos: TodoItem[]; permissions: PermissionRequest[]; providers?: ProviderListResponse; sessions?: Session[] }
  | { type: "event"; event: GatewayEventEnvelope }
  | { type: "composer"; value: string }
  | { type: "notice"; value?: string }
  | { type: "status"; value: AppState["status"] }
  | { type: "permissions"; value: PermissionRequest[] }
  | { type: "questions"; value: QuestionRequest[] }
  | { type: "todos"; value: TodoItem[] }
  | { type: "sessions"; value: Session[]; open?: boolean }
  | { type: "select-session"; delta: number }
  | { type: "select-model"; delta: number }
  | { type: "toggle-help" }
  | { type: "toggle-sessions" }
  | { type: "toggle-models" }
  | { type: "diff"; open: boolean; text?: string };

export function initialState(cwd: string): AppState {
  return {
    cwd,
    sessions: [],
    messages: [],
    todos: [],
    permissions: [],
    questions: [],
    status: "idle",
    composer: "",
    help: false,
    sessionsOpen: false,
    modelsOpen: false,
    diffOpen: false,
    diffText: "",
    selectedSessionIndex: 0,
    selectedModelIndex: 0,
  };
}

export function reducer(state: AppState, action: AppAction): AppState {
  if (action.type === "hydrate") {
    return {
      ...state,
      session: action.session,
      sessions: action.sessions ?? state.sessions,
      messages: action.messages,
      todos: action.todos,
      permissions: action.permissions,
      questions: state.questions,
      providers: action.providers,
      status: action.session.status ?? "idle",
      selectedSessionIndex: selectedSessionIndex(action.sessions ?? state.sessions, action.session.id),
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
    if (action.event.payload?.type === "todo.updated") {
      const todos = normalized.todos as TodoItem[] | undefined;
      if (todos) return { ...state, todos };
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
  if (action.type === "todos") return { ...state, todos: action.value };
  if (action.type === "sessions") {
    return {
      ...state,
      sessions: action.value,
      sessionsOpen: action.open ?? state.sessionsOpen,
      selectedSessionIndex: selectedSessionIndex(action.value, state.session?.id),
    };
  }
  if (action.type === "select-session") {
    return { ...state, selectedSessionIndex: clampIndex(state.selectedSessionIndex + action.delta, state.sessions.length) };
  }
  if (action.type === "select-model") {
    return { ...state, selectedModelIndex: clampIndex(state.selectedModelIndex + action.delta, modelCount(state.providers)) };
  }
  if (action.type === "toggle-help") return { ...state, help: !state.help };
  if (action.type === "toggle-sessions") return { ...state, sessionsOpen: !state.sessionsOpen, modelsOpen: false };
  if (action.type === "toggle-models") return { ...state, modelsOpen: !state.modelsOpen, sessionsOpen: false };
  if (action.type === "diff") return { ...state, diffOpen: action.open, diffText: action.text ?? state.diffText };
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

function clampIndex(index: number, length: number): number {
  if (length <= 0) return 0;
  return Math.max(0, Math.min(length - 1, index));
}

function modelCount(providers: ProviderListResponse | undefined): number {
  return providers?.all.reduce((count, provider) => count + Object.keys(provider.models ?? {}).length, 0) ?? 0;
}
