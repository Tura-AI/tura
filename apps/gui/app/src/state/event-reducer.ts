import type {
  GatewayEventEnvelope,
  Message,
  MessagePart,
  Session,
  TodoItem,
} from "@tura/gateway-sdk";
import type { AppState } from "./global-store";
import { messageSessionId, sessionHasDisplayName, sessionUpdatedAt } from "./global-store";

export function applyGatewayEvent(state: AppState, envelope: GatewayEventEnvelope): AppState {
  const event = envelope.payload;
  const properties = event.properties ?? {};
  const next: AppState = {
    ...state,
    connection: "connected",
    lastEvent: event.type,
  };

  switch (event.type) {
    case "server.connected":
      return next;
    case "server.instance.disposed":
      return {
        ...next,
        connection: "disconnected",
      };
    case "project.updated":
      return {
        ...next,
        currentProject: {
          project: properties as NonNullable<AppState["currentProject"]>["project"],
        },
      };
    case "session.created":
    case "session.updated": {
      const session = readSession(properties);
      if (!session) {
        return next;
      }
      return {
        ...next,
        sessions: upsertSession(next.sessions, session),
        selectedSessionId: next.selectedSessionId || session.id,
      };
    }
    case "session.deleted": {
      const sessionId = readId(properties, "sessionID", "session_id");
      if (!sessionId) {
        return next;
      }
      const sessions = next.sessions.filter((session) => session.id !== sessionId);
      const { [sessionId]: _messages, ...messagesBySession } = next.messagesBySession;
      const { [sessionId]: _paging, ...messagePagingBySession } = next.messagePagingBySession;
      const { [sessionId]: _todos, ...todosBySession } = next.todosBySession;
      return {
        ...next,
        sessions,
        messagesBySession,
        messagePagingBySession,
        todosBySession,
        selectedSessionId:
          next.selectedSessionId === sessionId ? sessions[0]?.id : next.selectedSessionId,
      };
    }
    case "session.status": {
      const sessionId = readId(properties, "sessionID", "session_id");
      const status = normalizeStatus(properties.status);
      if (!sessionId || !status) {
        return next;
      }
      return {
        ...next,
        sessions: next.sessions.map((session) =>
          session.id === sessionId ? { ...session, status } : session,
        ),
      };
    }
    case "message.updated": {
      const message = readMessage(properties);
      if (!message) {
        return next;
      }
      const sessionId =
        readId(properties, "sessionID", "session_id") || messageSessionId(message);
      if (!sessionId) {
        return next;
      }
      return {
        ...next,
        messagesBySession: {
          ...next.messagesBySession,
          [sessionId]: upsertMessage(next.messagesBySession[sessionId] ?? [], message),
        },
      };
    }
    case "message.removed": {
      const sessionId = readId(properties, "sessionID", "session_id");
      const messageId = readId(properties, "messageID", "message_id");
      if (!sessionId || !messageId) {
        return next;
      }
      return {
        ...next,
        messagesBySession: {
          ...next.messagesBySession,
          [sessionId]: (next.messagesBySession[sessionId] ?? []).filter(
            (message) => message.id !== messageId,
          ),
        },
      };
    }
    case "message.part.delta": {
      const sessionId = readId(properties, "sessionID", "session_id");
      const messageId = readId(properties, "messageID", "message_id");
      const partId = readId(properties, "partID", "part_id");
      const field = readString(properties, "field");
      const delta = readString(properties, "delta");
      if (!sessionId || !messageId || !partId || delta === undefined) {
        return next;
      }
      return {
        ...next,
        messagesBySession: {
          ...next.messagesBySession,
          [sessionId]: applyPartDelta(
            next.messagesBySession[sessionId] ?? [],
            messageId,
            partId,
            field,
            delta,
            sessionId,
          ),
        },
      };
    }
    case "message.part.updated": {
      const sessionId = readId(properties, "sessionID", "session_id");
      const part = properties.part as MessagePart | undefined;
      if (!sessionId || !part?.id) {
        return next;
      }
      const messageId = part.messageID;
      const messages = next.messagesBySession[sessionId] ?? [];
      const hasMessage = messageId
        ? messages.some((message) => message.id === messageId)
        : messages.length > 0;
      return {
        ...next,
        messagesBySession: {
          ...next.messagesBySession,
          [sessionId]: hasMessage
            ? messages.map((message) => {
                if (messageId && message.id !== messageId) {
                  return message;
                }
                const hasPart = message.parts.some((existing) => existing.id === part.id);
                return {
                  ...message,
                  parts: hasPart
                    ? message.parts.map((existing) =>
                        existing.id === part.id ? { ...existing, ...part } : existing,
                      )
                    : [...message.parts, part],
                };
              })
            : [
                ...messages,
                {
                  id: messageId,
                  sessionID: sessionId,
                  role: "assistant",
                  parts: [part],
                  created_at: Date.now(),
                  updated_at: Date.now(),
                  time: { created: Date.now(), updated: Date.now() },
                },
              ],
        },
      };
    }
    case "session.diff": {
      const files = Array.isArray(properties.diff)
        ? (properties.diff as AppState["diff"])
        : undefined;
      return files ? { ...next, diff: files } : next;
    }
    case "todo.updated": {
      const sessionId = readId(properties, "sessionID", "session_id") || next.selectedSessionId;
      const todos = Array.isArray(properties.todos) ? (properties.todos as TodoItem[]) : undefined;
      if (!sessionId || !todos) {
        return next;
      }
      return {
        ...next,
        todosBySession: {
          ...next.todosBySession,
          [sessionId]: todos,
        },
      };
    }
    case "permission.asked":
    case "permission.replied": {
      const request = readRequest<NonNullable<AppState["permissions"]>[number]>(properties, [
        "permission",
        "request",
      ]);
      if (!request?.id) {
        return next;
      }
      return {
        ...next,
        permissions:
          event.type === "permission.replied"
            ? next.permissions.filter((item) => item.id !== request.id)
            : upsertById(next.permissions, request),
      };
    }
    case "question.asked":
    case "question.replied":
    case "question.rejected": {
      const request = readRequest<NonNullable<AppState["questions"]>[number]>(properties, [
        "question",
        "request",
      ]);
      if (!request?.id) {
        return next;
      }
      return {
        ...next,
        questions:
          event.type === "question.asked"
            ? upsertById(next.questions, request)
            : next.questions.filter((item) => item.id !== request.id),
      };
    }
    case "vcs.branch.updated": {
      const branch = readString(properties, "branch");
      return branch
        ? {
            ...next,
            vcs: {
              branch,
              default_branch: next.vcs?.default_branch ?? "unknown",
            },
          }
        : next;
    }
    default:
      return next;
  }
}

export function upsertSession(sessions: Session[], session: Session): Session[] {
  const existing = sessions.find((item) => item.id === session.id);
  const nextSession =
    existing && !sessionHasDisplayName(session) && sessionHasDisplayName(existing)
      ? {
          ...session,
          name: existing.name,
          session_display_name: existing.session_display_name,
          plan_summary: existing.plan_summary,
        }
      : session;
  const without = sessions.filter((item) => item.id !== session.id);
  return [...without, nextSession].sort(
    (left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left),
  );
}

export function upsertMessage(messages: Message[], message: Message): Message[] {
  const without = messages.filter(
    (item) => item.id !== message.id && !isOptimisticDuplicateUserMessage(item, message),
  );
  return [...without, message].sort((left, right) => {
    const leftTime = left.time?.created ?? left.created_at ?? 0;
    const rightTime = right.time?.created ?? right.created_at ?? 0;
    return leftTime - rightTime;
  });
}

function isOptimisticDuplicateUserMessage(existing: Message, incoming: Message): boolean {
  if (existing.role !== "user" || incoming.role !== "user" || !existing.id.startsWith("prompt:")) {
    return false;
  }
  const existingText = messageText(existing).trim();
  const incomingText = messageText(incoming).trim();
  return existingText.length > 0 && existingText === incomingText;
}

function messageText(message: Message): string {
  return message.parts
    .map((part) => {
      const record = part as Record<string, unknown>;
      return typeof record.text === "string"
        ? record.text
        : typeof record.content === "string"
          ? record.content
          : "";
    })
    .join("\n");
}

function applyPartDelta(
  messages: Message[],
  messageId: string,
  partId: string,
  field: string | undefined,
  delta: string,
  sessionId: string,
): Message[] {
  if (field !== "text" && field !== "content") {
    return messages;
  }

  let foundMessage = false;
  let foundPart = false;
  const now = Date.now();
  const next = messages.map((message) => {
    if (message.id !== messageId) {
      return message;
    }
    foundMessage = true;
    return {
      ...message,
      updated_at: now,
      time: {
        ...message.time,
        updated: now,
      },
      parts: message.parts.map((part) => {
        if (part.id !== partId) {
          return part;
        }
        foundPart = true;
        return {
          ...part,
          [field]: `${(part as Record<string, unknown>)[field] ?? ""}${delta}`,
        };
      }),
    };
  });

  if (foundMessage && !foundPart) {
    return next.map((message) =>
      message.id === messageId
        ? {
            ...message,
            updated_at: now,
            time: {
              ...message.time,
              updated: now,
            },
            parts: [
              ...message.parts,
              {
                id: partId,
                sessionID: sessionId,
                messageID: messageId,
                type: "text",
                [field]: delta,
              } as MessagePart,
            ],
          }
        : message,
    );
  }

  if (!foundMessage) {
    next.push({
      id: messageId,
      sessionID: sessionId,
      role: "assistant",
      created_at: now,
      updated_at: now,
      time: { created: now, updated: now },
      parts: [
        {
          id: partId,
          sessionID: sessionId,
          messageID: messageId,
          type: "text",
          [field]: delta,
        } as MessagePart,
      ],
    });
  }

  return next.sort((left, right) => {
    const leftTime = left.time?.created ?? left.created_at ?? 0;
    const rightTime = right.time?.created ?? right.created_at ?? 0;
    return leftTime - rightTime;
  });
}

function readSession(properties: Record<string, unknown>): Session | undefined {
  const info = properties.info;
  return isObject(info) && typeof info.id === "string" ? (info as Session) : undefined;
}

function readMessage(properties: Record<string, unknown>): Message | undefined {
  const info = properties.info;
  return isObject(info) && typeof info.id === "string" ? (info as Message) : undefined;
}

function readString(properties: Record<string, unknown>, key: string): string | undefined {
  const value = properties[key];
  return typeof value === "string" ? value : undefined;
}

function readId(
  properties: Record<string, unknown>,
  camelKey: string,
  snakeKey: string,
): string | undefined {
  return readString(properties, camelKey) ?? readString(properties, snakeKey);
}

function normalizeStatus(value: unknown): "idle" | "busy" | "error" | undefined {
  if (typeof value === "string" && (value === "idle" || value === "busy" || value === "error")) {
    return value;
  }
  if (isObject(value)) {
    const nested = value.status ?? value.type;
    if (
      typeof nested === "string" &&
      (nested === "idle" || nested === "busy" || nested === "error")
    ) {
      return nested;
    }
  }
  return undefined;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object";
}

function readRequest<T extends { id: string }>(
  properties: Record<string, unknown>,
  keys: string[],
): T | undefined {
  for (const key of keys) {
    const value = properties[key];
    if (isObject(value) && typeof value.id === "string") {
      return value as T;
    }
  }
  if (typeof properties.id === "string") {
    return properties as T;
  }
  return undefined;
}

function upsertById<T extends { id: string }>(items: T[], item: T): T[] {
  return [...items.filter((existing) => existing.id !== item.id), item];
}
