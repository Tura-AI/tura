import type { Message, MessagePart, Session } from "../../types/session.js";
import {
  isInternalTaskStatusPart,
  messageSortValue,
  messageText,
  partMessageID,
} from "../../types/session.js";
import type { AppState, LiveStream, RefreshSessionState } from "../reducer.js";

const rawAnsiControlPattern = /\x1b\][^\x07]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[@-_]/g;

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

export function mergeStableMessages(current: Message[], incoming: Message[]): Message[] {
  let changed = false;
  const next = [...current];
  const indexes = new Map(next.map((message, index) => [message.id, index]));
  for (const message of incoming) {
    const index = indexes.get(message.id);
    if (index !== undefined) {
      const merged = mergeMessageForDisplay(next[index], message);
      if (merged !== next[index]) {
        next[index] = merged;
        changed = true;
      }
      continue;
    }
    indexes.set(message.id, next.length);
    next.push(message);
    changed = true;
  }
  return changed ? sortMessages(next) : current;
}

export function mergeStableMessagesIgnoringLive(
  current: Message[],
  incoming: Message[],
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
): { messages: Message[]; liveStreams: Record<string, LiveStream> } {
  const { messages, liveStreams } = commitLiveStreamsForMessages(
    current,
    streams,
    sessionID,
    incoming,
  );
  const filtered = incoming.filter(
    (message) => !messageMatchesLiveStream(message, streams, sessionID),
  );
  return {
    messages: mergeStableMessages(messages, filtered),
    liveStreams,
  };
}

export function appendNewStableMessages(current: Message[], incoming: Message[]): Message[] {
  const existingIDs = new Set(current.map((message) => message.id));
  const additions = incoming.filter((message) => !existingIDs.has(message.id));
  if (!additions.length) return current;
  return sortMessages([...current, ...additions]);
}

export function appendNewStableMessagesIgnoringLive(
  current: Message[],
  incoming: Message[],
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
): { messages: Message[]; liveStreams: Record<string, LiveStream> } {
  const { messages, liveStreams } = commitLiveStreamsForMessages(
    current,
    streams,
    sessionID,
    incoming,
  );
  const filtered = incoming.filter(
    (message) => !messageMatchesLiveStream(message, streams, sessionID),
  );
  return {
    messages: appendNewStableMessages(messages, filtered),
    liveStreams,
  };
}

export function updatePreviewForMessages(
  previews: Record<string, string>,
  sessionID: string,
  messages: Message[],
): Record<string, string> {
  const preview = lastMessagePreview(messages);
  return preview ? { ...previews, [sessionID]: preview } : previews;
}

export function refreshStateAfterBackgroundMessage(
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

export function refreshStateAfterMessages(
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

export function invalidateRefreshState(
  current: Record<string, RefreshSessionState>,
  sessionID?: string,
): Record<string, RefreshSessionState> {
  if (sessionID) {
    const { [sessionID]: _removed, ...rest } = current;
    return rest;
  }
  return {};
}

export function lastMessagePreview(messages: Message[]): string | undefined {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const preview = messagePreview(messages[index]);
    if (preview) return preview;
  }
  return undefined;
}

export function messagePreview(message: Message | undefined): string | undefined {
  const text = message ? messageText(message).replace(/\s+/g, " ").trim() : "";
  return text || undefined;
}

export function upsertMessage(messages: Message[], message: Message): Message[] {
  const existing = messages.find((item) => item.id === message.id);
  const merged = mergeMessageForDisplay(existing, message);
  const next = messages.filter((item) => item.id !== message.id);
  next.push(merged);
  next.sort((left, right) => messageSortValue(left) - messageSortValue(right));
  return next;
}

export function upsertMessageIgnoringLive(
  messages: Message[],
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  message: Message,
): { messages: Message[]; liveStreams: Record<string, LiveStream> } {
  if (!messageMatchesLiveStream(message, streams, sessionID)) {
    return { messages: upsertMessage(messages, message), liveStreams: streams };
  }
  const stableMessage = messageWithoutLiveStreamParts(message, streams, sessionID);
  if (!stableMessage.parts.length) return { messages, liveStreams: streams };
  return { messages: upsertMessage(messages, stableMessage), liveStreams: streams };
}

function sortMessages(messages: Message[]): Message[] {
  return [...messages].sort((left, right) => messageSortValue(left) - messageSortValue(right));
}

export function normalizeMessagesForDisplay(messages: Message[]): Message[] {
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
  return {
    ...existing,
    ...incoming,
    created_at: incomingCreated ?? existingCreated,
    time,
    parts: orderMessagePartsForDisplay(incomingParts),
  };
}

export function upsertPart(
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

export function upsertPartIgnoringLive(
  messages: Message[],
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  part: MessagePart,
): { messages: Message[]; liveStreams: Record<string, LiveStream> } {
  return { messages: upsertPart(messages, part, sessionID), liveStreams: streams };
}

export function applyPartDelta(
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

export function commitLiveStreams(
  messages: Message[],
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  shouldCommit: (stream: LiveStream) => boolean = () => true,
): { messages: Message[]; liveStreams: Record<string, LiveStream> } {
  let nextMessages = messages;
  let nextStreams = streams;
  for (const [key, stream] of Object.entries(streams)) {
    if (!streamMatchesSession(stream, sessionID) || !shouldCommit(stream)) continue;
    nextMessages = applyLiveStream(nextMessages, stream);
    const { [key]: _committed, ...rest } = nextStreams;
    nextStreams = rest;
  }
  return { messages: nextMessages, liveStreams: nextStreams };
}

export function clearLiveStreamsForMessageID(
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

function commitLiveStreamsForMessages(
  messages: Message[],
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  incoming: Message[],
): { messages: Message[]; liveStreams: Record<string, LiveStream> } {
  if (!incoming.some((message) => messageMatchesLiveStream(message, streams, sessionID))) {
    return { messages, liveStreams: streams };
  }
  let nextMessages = messages;
  let nextStreams = streams;
  for (const message of incoming) {
    const matching = Object.entries(nextStreams).filter(
      ([, stream]) =>
        streamMatchesSession(stream, sessionID) && liveStreamMatchesMessage(stream, message),
    );
    if (!matching.length) continue;
    nextMessages = upsertMessage(
      nextMessages,
      messageWithLiveStreams(
        message,
        matching.map(([, stream]) => stream),
      ),
    );
    nextStreams = Object.fromEntries(
      Object.entries(nextStreams).filter(([key]) => !matching.some(([matched]) => matched === key)),
    );
  }
  return { messages: nextMessages, liveStreams: nextStreams };
}

function messageWithLiveStreams(message: Message, streams: LiveStream[]): Message {
  let parts = message.parts ?? [];
  for (const stream of streams) {
    parts = partsWithLiveStream(parts, stream);
  }
  const updatedAt =
    message.updated_at ?? streams.reduce((latest, stream) => Math.max(latest, stream.updatedAt), 0);
  return {
    ...message,
    parts: orderMessagePartsForDisplay(parts),
    updated_at: updatedAt || message.updated_at,
  };
}

function partsWithLiveStream(parts: MessagePart[], stream: LiveStream): MessagePart[] {
  const exactPartIndex = parts.findIndex((part) => part.id === stream.partID);
  if (exactPartIndex >= 0) {
    return parts.map((part, index) =>
      index === exactPartIndex ? { ...part, [stream.field]: stream.text } : part,
    );
  }
  const messageTextPartIndex = parts.findIndex(
    (part) => part.type === "text" || part.type === "message" || !part.type,
  );
  if (messageTextPartIndex >= 0) {
    return parts.map((part, index) =>
      index === messageTextPartIndex ? { ...part, [stream.field]: stream.text } : part,
    );
  }
  return [...parts, liveStreamPart(stream)];
}

function messageShouldRemainLive(messages: Message[], incoming: Message): boolean {
  if (messageHasRunningPart(incoming)) return true;
  const existing = messages.find((message) => message.id === incoming.id);
  if (!existing) return false;
  const incomingPartsByID = new Map((incoming.parts ?? []).map((part) => [part.id, part]));
  return (existing.parts ?? []).some((part) => {
    if (!partIsRunning(part)) return false;
    const incomingPart = incomingPartsByID.get(part.id);
    return !incomingPart || partIsRunning(incomingPart);
  });
}

function messageWithoutLiveStreamParts(
  message: Message,
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
): Message {
  const liveStreamMessageIDs = new Set(
    Object.values(streams)
      .filter((stream) => streamMatchesSession(stream, sessionID))
      .map((stream) => stream.messageID),
  );
  const parts = (message.parts ?? []).filter(
    (part) => !partMatchesLiveStreamText(message, part, streams, sessionID, liveStreamMessageIDs),
  );
  return { ...message, parts };
}

function partMatchesLiveStreamText(
  message: Message,
  part: MessagePart,
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
  liveStreamMessageIDs: Set<string>,
): boolean {
  if (!partIsText(part)) return false;
  const messageID = partMessageID(part);
  if (liveStreamMessageIDs.has(message.id)) return true;
  return Object.values(streams).some(
    (stream) =>
      streamMatchesSession(stream, sessionID) &&
      (stream.partID === part.id || (Boolean(messageID) && stream.messageID === messageID)),
  );
}

function partIsText(part: MessagePart): boolean {
  return part.type === "text" || part.type === "message" || !part.type;
}

function messageMatchesLiveStream(
  message: Message,
  streams: Record<string, LiveStream>,
  sessionID: string | undefined,
): boolean {
  return Object.values(streams).some(
    (stream) =>
      streamMatchesSession(stream, sessionID) && liveStreamMatchesMessage(stream, message),
  );
}

function liveStreamMatchesMessage(stream: LiveStream, message: Message): boolean {
  if (stream.messageID === message.id) return true;
  return (message.parts ?? []).some((part) => {
    const messageID = partMessageID(part);
    return stream.partID === part.id || (Boolean(messageID) && stream.messageID === messageID);
  });
}

function streamMatchesSession(stream: LiveStream, sessionID: string | undefined): boolean {
  return !sessionID || !stream.sessionID || stream.sessionID === sessionID;
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
  return /run|progress|pending|busy|question|in[_ -]?progress|exec(?:ute|uting|uted|ution)?|start/i.test(
    status,
  );
}

function sanitizeStreamDelta(value: string): string {
  return value.replace(/\r\n/g, "\n").replace(/\r/g, "\n").replace(rawAnsiControlPattern, "");
}
