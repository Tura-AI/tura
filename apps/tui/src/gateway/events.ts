import type { GatewayEventEnvelope, NormalizedEvent } from "../types/event.js";
import { eventSessionID } from "../types/event.js";
import type { PermissionRequest, QuestionRequest } from "../types/permission.js";
import type { Message, MessagePart } from "../types/session.js";
import { messageText, partMessageID } from "../types/session.js";

export async function* parseSse(response: Response): AsyncGenerator<GatewayEventEnvelope> {
  if (!response.body) return;
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let boundary = eventBoundary(buffer);
      while (boundary >= 0) {
        const raw = buffer.slice(0, boundary);
        buffer = buffer.slice(skipBoundary(buffer, boundary));
        const event = parseSseBlock(raw);
        if (event) yield event;
        boundary = eventBoundary(buffer);
      }
    }
    const trailing = parseSseBlock(buffer);
    if (trailing) yield trailing;
  } finally {
    reader.releaseLock();
  }
}

function eventBoundary(buffer: string): number {
  const windows = buffer.indexOf("\r\n\r\n");
  const unix = buffer.indexOf("\n\n");
  if (windows < 0) return unix;
  if (unix < 0) return windows;
  return Math.min(windows, unix);
}

function skipBoundary(buffer: string, boundary: number): number {
  return buffer.startsWith("\r\n\r\n", boundary) ? boundary + 4 : boundary + 2;
}

export function parseSseBlock(block: string): GatewayEventEnvelope | undefined {
  const data = block
    .split(/\r?\n/)
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice(5).trimStart())
    .join("\n")
    .trim();
  if (!data || data === "[DONE]") return undefined;
  try {
    return JSON.parse(data) as GatewayEventEnvelope;
  } catch {
    return { directory: "global", payload: { type: "parse.error", properties: { data } } };
  }
}

export function normalizeEvent(raw: GatewayEventEnvelope): NormalizedEvent {
  const payload = raw.payload;
  const type = payload?.type ?? "unknown";
  const sessionID = eventSessionID(payload);
  let messageID: string | undefined;
  let partID: string | undefined;
  let text: string | undefined;
  let status: string | undefined;
  let tool: string | undefined;
  let todos: unknown[] | undefined;
  let permission: PermissionRequest | undefined;
  let question: QuestionRequest | undefined;

  if (payload?.type === "message.updated") {
    const info = (payload.properties as { info?: Message } | undefined)?.info;
    messageID = info?.id;
    text = info ? messageText(info) : undefined;
  }
  if (payload?.type === "message.part.updated") {
    const part = (payload.properties as { part?: MessagePart } | undefined)?.part;
    partID = part?.id;
    messageID = part ? partMessageID(part) : undefined;
    text = part?.text ?? part?.content ?? undefined;
    tool = part?.tool ?? undefined;
    status = partStatus(part);
  }
  if (payload?.type === "message.part.delta") {
    const properties = payload.properties as Record<string, unknown> | undefined;
    partID = readString(properties, "part_id") ?? readString(properties, "partID");
    messageID = readString(properties, "message_id") ?? readString(properties, "messageID");
    text = readString(properties, "delta");
  }
  if (payload?.type === "session.status") {
    const statusValue = (payload.properties as { status?: unknown } | undefined)?.status;
    status =
      typeof statusValue === "string"
        ? statusValue
        : statusValue && typeof statusValue === "object" && "type" in statusValue
          ? String((statusValue as { type?: unknown }).type)
          : undefined;
  }
  if (payload?.type === "todo.updated") {
    const properties = payload.properties as { todos?: unknown[] } | undefined;
    todos = Array.isArray(properties?.todos) ? properties.todos : undefined;
  }
  if (payload?.type === "permission.asked" || payload?.type === "permission.replied") {
    permission = readRequest<PermissionRequest>(payload.properties, ["permission", "request"]);
  }
  if (payload?.type === "question.asked" || payload?.type === "question.replied" || payload?.type === "question.rejected") {
    question = readRequest<QuestionRequest>(payload.properties, ["question", "request"]);
  }
  return {
    type,
    directory: raw.directory ?? "global",
    sessionID,
    messageID,
    partID,
    status,
    text,
    tool,
    todos,
    permission,
    question,
    raw,
  };
}

function partStatus(part: MessagePart | undefined): string | undefined {
  const state = part?.state;
  if (!state || typeof state !== "object") return undefined;
  const status = (state as Record<string, unknown>).status;
  return typeof status === "string" ? status : undefined;
}

function readString(properties: Record<string, unknown> | undefined, key: string): string | undefined {
  const value = properties?.[key];
  return typeof value === "string" ? value : undefined;
}

function readRequest<T extends { id: string }>(
  properties: Record<string, unknown> | undefined,
  keys: string[],
): T | undefined {
  if (!properties) return undefined;
  for (const key of keys) {
    const value = properties[key];
    if (value && typeof value === "object" && typeof (value as { id?: unknown }).id === "string") {
      return value as T;
    }
  }
  return typeof properties.id === "string" ? (properties as T) : undefined;
}
