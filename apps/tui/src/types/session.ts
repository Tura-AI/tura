import type { JsonObject } from "./common.js";

export type SessionStatusValue = "idle" | "busy" | "error";

export interface Session {
  id: string;
  name?: string | null;
  parent_id?: string | null;
  created_at?: number;
  updated_at?: number;
  directory?: string | null;
  model?: string | null;
  agent?: string | null;
  session_type?: string | null;
  lsp?: unknown;
  auto_session_name?: boolean;
  kill_processes_on_start?: boolean;
  validator_enabled?: boolean;
  model_variant?: string | null;
  model_acceleration_enabled?: boolean;
  status?: SessionStatusValue;
  message_count?: number;
  session_display_name?: string | null;
}

export interface MessagePart {
  id: string;
  sessionID?: string;
  session_id?: string;
  messageID?: string;
  message_id?: string;
  type: string;
  text?: string | null;
  content?: string | null;
  metadata?: unknown;
  callID?: string | null;
  call_id?: string | null;
  tool?: string | null;
  state?: unknown;
}

export interface Message {
  id: string;
  sessionID?: string;
  session_id?: string;
  parentID?: string | null;
  parent_id?: string | null;
  role: "user" | "assistant" | "system";
  parts: MessagePart[];
  created_at?: number;
  updated_at?: number;
  time?: { created?: number; updated?: number };
  cost?: number;
  providerID?: string;
  modelID?: string;
  tokens?: unknown;
}

export interface MessageEnvelope {
  info?: Message;
  parts?: MessagePart[];
  [key: string]: unknown;
}

export interface CreateSessionRequest {
  directory?: string;
  model?: string;
  agent?: string;
  session_type?: string;
  model_variant?: string;
  model_acceleration_enabled?: boolean;
  kill_processes_on_start?: boolean;
  validator_enabled?: boolean;
  auto_session_name?: boolean;
}

export interface PromptPayload {
  messageID: string;
  parts: Array<{ id: string; type: "text"; text: string }>;
  model?: string;
  agent?: string;
  source: "cli" | "tui";
  variant?: string;
  model_acceleration_enabled?: boolean;
  [key: string]: unknown;
}

export interface RunResult {
  sessionID: string;
  status: "completed" | "failed" | "timeout" | "permission_required";
  finalText: string;
  messages: Message[];
  usage: unknown | null;
}

export function sessionTitle(session: Session): string {
  return (session.session_display_name || session.name || session.id || "New Session").toString();
}

export function sessionUpdatedAt(session: Session): number {
  return session.updated_at ?? 0;
}

export function sessionStatusText(status: unknown): SessionStatusValue {
  if (typeof status === "string") {
    if (status === "busy" || status === "error") return status;
    return "idle";
  }
  if (status && typeof status === "object") {
    const object = status as JsonObject;
    const nested = object.status;
    if (nested) return sessionStatusText(nested);
    const type = object.type;
    if (type === "busy" || type === "error") return type;
  }
  return "idle";
}

export function sessionDirectory(session: Session): string {
  return session.directory ?? "";
}

export function normalizeMessage(value: Message | MessageEnvelope): Message {
  if ("info" in value && value.info) {
    return { ...value.info, parts: value.parts ?? value.info.parts ?? [] };
  }
  return value as Message;
}

export function messageSessionID(message: Message): string {
  return message.sessionID ?? message.session_id ?? "";
}

export function partMessageID(part: MessagePart): string {
  return part.messageID ?? part.message_id ?? "";
}

export function partSessionID(part: MessagePart): string {
  return part.sessionID ?? part.session_id ?? "";
}

export function messageText(message: Message): string {
  return (message.parts ?? [])
    .filter((part) => part.type === "text" || part.type === "message" || !part.type)
    .map((part) => part.text ?? part.content ?? "")
    .filter(Boolean)
    .join("");
}

export function messageSortValue(message: Message): number {
  return (
    message.created_at ?? message.time?.created ?? message.updated_at ?? message.time?.updated ?? 0
  );
}

export function lastAssistantText(messages: Message[]): string {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.role === "assistant") {
      const text = messageText(message).trim();
      if (isUserFacingAssistantText(text)) return text;
    }
  }
  return "";
}

export function hasUserFacingAssistantText(messages: Message[], startIndex = 0): boolean {
  return messages
    .slice(startIndex)
    .some(
      (message) => message.role === "assistant" && isUserFacingAssistantText(messageText(message)),
    );
}

function isUserFacingAssistantText(value: string): boolean {
  const text = value.trim();
  if (!text) return false;
  if (/completed without a user-facing message/i.test(text)) return false;
  return true;
}
