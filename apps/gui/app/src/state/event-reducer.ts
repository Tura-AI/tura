import type {
  GatewayEventEnvelope,
  CommandUpdate,
  Message,
  MessagePart,
  Session,
  TodoItem,
} from "@tura/gateway-sdk";
import type { AppState } from "./global-store";
import { messageSessionId, sessionHasDisplayName, sessionUpdatedAt } from "./global-store";

const STREAMED_DELTA_FIELDS_METADATA_KEY = "__turaStreamedDeltaFields";

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
          session.id === sessionId
            ? sessionWithStatusMetrics(session, status, properties.context_tokens, properties.usage)
            : session,
        ),
      };
    }
    case "message.updated": {
      const message = readMessage(properties);
      if (!message) {
        return next;
      }
      const sessionId = readId(properties, "sessionID", "session_id") || messageSessionId(message);
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
                        existing.id === part.id ? mergeMessagePart(existing, part) : existing,
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
    case "command.updated": {
      const update = properties as unknown as CommandUpdate;
      const sessionId = readId(properties, "sessionID", "session_id");
      if (!sessionId || !update.messageID || !update.partID || !update.commandID) {
        return next;
      }
      return {
        ...next,
        messagesBySession: {
          ...next.messagesBySession,
          [sessionId]: applyCommandUpdate(
            next.messagesBySession[sessionId] ?? [],
            sessionId,
            update,
          ),
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
  const existing = messages.find((item) => item.id === message.id);
  const nextMessage = existing ? mergeMessage(existing, message) : message;
  const without = messages.filter(
    (item) => item.id !== message.id && !isOptimisticDuplicateUserMessage(item, message),
  );
  return [...without, nextMessage].sort((left, right) => {
    const leftTime = left.time?.created ?? left.created_at ?? 0;
    const rightTime = right.time?.created ?? right.created_at ?? 0;
    return leftTime - rightTime;
  });
}

function mergeMessage(existing: Message, incoming: Message): Message {
  return {
    ...existing,
    ...incoming,
    parts: mergeMessageParts(existing.parts, incoming.parts),
  };
}

function mergeMessageParts(
  existingParts: MessagePart[],
  incomingParts: MessagePart[],
): MessagePart[] {
  const incomingIds = new Set(incomingParts.map((part) => part.id));
  return [
    ...incomingParts.map((incoming) => {
      const existing = existingParts.find((part) => part.id === incoming.id);
      return existing ? mergeMessagePart(existing, incoming) : incoming;
    }),
    ...existingParts.filter((part) => !incomingIds.has(part.id)),
  ];
}

function mergeMessagePart(existing: MessagePart, incoming: MessagePart): MessagePart {
  const streamedFields = streamedDeltaFields(existing);
  const merged = { ...existing, ...incoming } as MessagePart;
  if (streamedFields.text && existing.text !== undefined) {
    merged.text = existing.text;
  }
  if (streamedFields.content && existing.content !== undefined) {
    merged.content = existing.content;
  }
  if (!streamedFields.text && !streamedFields.content) {
    return merged;
  }
  merged.metadata = {
    ...recordValue(incoming.metadata),
    ...recordValue(existing.metadata),
    [STREAMED_DELTA_FIELDS_METADATA_KEY]: streamedFields,
  };
  return merged;
}

function streamedDeltaFields(part: MessagePart): Record<"text" | "content", boolean> {
  const fields = recordValue(recordValue(part.metadata)[STREAMED_DELTA_FIELDS_METADATA_KEY]);
  return {
    text: fields.text === true,
    content: fields.content === true,
  };
}

function appendPartDelta(part: MessagePart, field: "text" | "content", delta: string): MessagePart {
  const metadata = recordValue(part.metadata);
  const streamedFields = streamedDeltaFields(part);
  return {
    ...part,
    [field]: `${(part as Record<string, unknown>)[field] ?? ""}${delta}`,
    metadata: {
      ...metadata,
      [STREAMED_DELTA_FIELDS_METADATA_KEY]: {
        ...streamedFields,
        [field]: true,
      },
    },
  };
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
        return appendPartDelta(part, field, delta);
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
                metadata: {
                  [STREAMED_DELTA_FIELDS_METADATA_KEY]: { [field]: true },
                },
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
          metadata: {
            [STREAMED_DELTA_FIELDS_METADATA_KEY]: { [field]: true },
          },
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

function applyCommandUpdate(
  messages: Message[],
  sessionId: string,
  update: CommandUpdate,
): Message[] {
  const createdAt = update.createdAt ?? update.updatedAt ?? Date.now();
  const updatedAt = update.updatedAt ?? Date.now();
  const part = commandPartFromUpdate(update);
  let foundMessage = false;
  const next = messages.map((message) => {
    if (message.id !== update.messageID) {
      return message;
    }
    foundMessage = true;
    let foundPart = false;
    const parts = message.parts.map((existing) => {
      if (existing.id !== update.partID) {
        return existing;
      }
      foundPart = true;
      return mergeCommandPart(existing, update);
    });
    return {
      ...message,
      created_at: Math.min(message.time?.created ?? message.created_at ?? createdAt, createdAt),
      updated_at: updatedAt,
      time: {
        ...message.time,
        created: Math.min(message.time?.created ?? message.created_at ?? createdAt, createdAt),
        updated: updatedAt,
      },
      parts: foundPart ? parts : [...parts, part],
    };
  });
  if (!foundMessage) {
    next.push({
      id: update.messageID,
      sessionID: sessionId,
      role: "assistant",
      created_at: createdAt,
      updated_at: updatedAt,
      time: { created: createdAt, updated: updatedAt },
      parts: [part],
    });
  }
  return next.sort((left, right) => {
    const leftTime = left.time?.created ?? left.created_at ?? 0;
    const rightTime = right.time?.created ?? right.created_at ?? 0;
    return leftTime - rightTime;
  });
}

function commandPartFromUpdate(update: CommandUpdate): MessagePart {
  const createdAt = update.createdAt ?? update.updatedAt ?? Date.now();
  const updatedAt = update.updatedAt ?? createdAt;
  return mergeCommandPart(
    {
      id: update.partID,
      sessionID: update.sessionID,
      messageID: update.messageID,
      type: "tool",
      tool: "command_run",
      callID: update.commandRunID,
      state: {
        status: "running",
        created_at: createdAt,
        updated_at: updatedAt,
        time: { start: createdAt, updated: updatedAt },
        input: { commands: [] },
        streamed_command_run_result: { results: [] },
      },
    },
    update,
  );
}

function mergeCommandPart(part: MessagePart, update: CommandUpdate): MessagePart {
  const state = recordValue(part.state);
  const input = recordValue(state.input);
  const stream = recordValue(state.streamed_command_run_result);
  const time = recordValue(state.time);
  const previousCreatedAt = numberValue(state.created_at) ?? numberValue(state.createdAt);
  const updateCreatedAt = update.createdAt ?? update.updatedAt ?? Date.now();
  const createdAt = Math.min(previousCreatedAt ?? updateCreatedAt, updateCreatedAt);
  const updatedAt = update.updatedAt ?? numberValue(state.updated_at) ?? createdAt;
  const commands = upsertCommandRecord(arrayValue(input.commands), update.command, update);
  const results = update.result
    ? upsertCommandRecord(arrayValue(stream.results), update.result, update)
    : arrayValue(stream.results);
  return {
    ...part,
    id: update.partID,
    sessionID: update.sessionID,
    messageID: update.messageID,
    type: "tool",
    tool: "command_run",
    callID: part.callID ?? update.commandRunID,
    state: {
      ...state,
      status: commandRunStatus(commands, results, update.status),
      created_at: createdAt,
      updated_at: updatedAt,
      eventSeq: Math.max(numberValue(state.eventSeq) ?? 0, update.eventSeq ?? 0),
      time: { ...time, start: numberValue(time.start) ?? createdAt, updated: updatedAt },
      input: { ...input, commands },
      streamed_command_run_result: { ...stream, results },
    },
  };
}

function upsertCommandRecord(
  current: unknown[],
  incoming: unknown,
  update: CommandUpdate,
): unknown[] {
  if (!incoming || (typeof incoming === "object" && Object.keys(incoming).length === 0)) {
    return current;
  }
  const incomingRecord = {
    ...recordValue(incoming),
    command_id: update.commandID,
    command_run_id: update.commandRunID,
    provider_tool_call_id: update.providerToolCallID ?? undefined,
    command_index: update.commandIndex ?? undefined,
    event_seq: update.eventSeq ?? undefined,
    created_at: update.createdAt ?? undefined,
    updated_at: update.updatedAt ?? undefined,
    status: update.status,
  };
  const existingIndex = current.findIndex((item) => commandRecordID(item) === update.commandID);
  if (existingIndex < 0) {
    return sortCommandRecords([...current, incomingRecord]);
  }
  const existing = recordValue(current[existingIndex]);
  if ((numberValue(existing.event_seq) ?? -1) > (update.eventSeq ?? -1)) {
    return current;
  }
  const next = [...current];
  next[existingIndex] = { ...existing, ...incomingRecord };
  return sortCommandRecords(next);
}

function commandRunStatus(commands: unknown[], results: unknown[], fallback: string): string {
  const resultRecords = results.map(recordValue);
  if (resultRecords.some((result) => result.success === false || result.status === "failed")) {
    return "failed";
  }
  if (
    commands.length > 0 &&
    resultRecords.length >= commands.length &&
    resultRecords.every((result) => result.success === true || result.status === "completed")
  ) {
    return "completed";
  }
  return fallback === "ready" ? "running" : fallback;
}

function commandRecordID(value: unknown): string | undefined {
  const record = recordValue(value);
  return stringValue(record.command_id) ?? stringValue(record.commandID);
}

function sortCommandRecords(values: unknown[]): unknown[] {
  return [...values].sort((left, right) => {
    const leftRecord = recordValue(left);
    const rightRecord = recordValue(right);
    return (
      (numberValue(leftRecord.command_index) ?? Number.MAX_SAFE_INTEGER) -
        (numberValue(rightRecord.command_index) ?? Number.MAX_SAFE_INTEGER) ||
      (numberValue(leftRecord.step) ?? Number.MAX_SAFE_INTEGER) -
        (numberValue(rightRecord.step) ?? Number.MAX_SAFE_INTEGER)
    );
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

function sessionWithStatusMetrics(
  session: Session,
  status: "idle" | "busy" | "error",
  contextTokensValue: unknown,
  usageValue: unknown,
): Session {
  const usage = readSessionUsage(usageValue);
  const contextTokens = readContextTokens(
    recordValue(usageValue).context_tokens ?? contextTokensValue,
  );
  return {
    ...session,
    status,
    ...(contextTokens ? { context_tokens: contextTokens } : {}),
    ...(usage ? { usage } : {}),
  };
}

function readSessionUsage(value: unknown): Session["usage"] | undefined {
  const record = recordValue(value);
  if (!Object.keys(record).length) {
    return undefined;
  }
  const contextTokens = readContextTokens(record.context_tokens);
  return {
    context_tokens: contextTokens ?? { input: 0, limit: 0 },
    tokens: record.tokens ?? null,
    cost: typeof record.cost === "number" && Number.isFinite(record.cost) ? record.cost : null,
    currency: typeof record.currency === "string" ? record.currency : null,
  };
}

function readContextTokens(value: unknown): Session["context_tokens"] | undefined {
  const record = recordValue(value);
  const input = numberValue(record.input);
  const limit = numberValue(record.limit);
  if (input === undefined && limit === undefined) {
    return undefined;
  }
  return {
    input: input ?? 0,
    limit: limit ?? 0,
  };
}

function recordValue(value: unknown): Record<string, unknown> {
  return isObject(value) && !Array.isArray(value) ? value : {};
}

function arrayValue(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value ? value : undefined;
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
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
