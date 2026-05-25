import type { JsonObject } from "./common.js";

export type SessionStatusValue = "idle" | "busy" | "error";

export interface Session {
  id: string;
  slug?: string;
  name?: string | null;
  title?: string | null;
  parentID?: string | null;
  parent_id?: string | null;
  created_at?: number;
  updated_at?: number;
  time?: { created?: number; updated?: number };
  directory?: string | null;
  model?: string | null;
  agent?: string | null;
  session_type?: string | null;
  sessionType?: string | null;
  lsp?: unknown;
  kill_processes_on_start?: boolean;
  validator_enabled?: boolean;
  force_multiple_tasks?: boolean;
  model_variant?: string | null;
  model_acceleration_enabled?: boolean;
  status?: SessionStatusValue;
  message_count?: number;
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

export interface TodoItem {
  id?: string;
  content?: string;
  title?: string;
  status?: "pending" | "in_progress" | "completed" | "cancelled" | string;
  priority?: string;
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
  force_multiple_tasks?: boolean;
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
  return (session.title || session.name || session.id || "New Session").toString();
}

export function sessionUpdatedAt(session: Session): number {
  return session.updated_at ?? session.time?.updated ?? 0;
}

export function sessionStatusText(status: unknown): SessionStatusValue {
  if (typeof status === "string") {
    if (status === "busy" || status === "error") return status;
    return "idle";
  }
  if (status && typeof status === "object") {
    const type = (status as JsonObject).type;
    if (type === "busy" || type === "error") return type;
  }
  return "idle";
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
  return message.created_at ?? message.time?.created ?? message.updated_at ?? message.time?.updated ?? 0;
}

export function lastAssistantText(messages: Message[]): string {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.role === "assistant") {
      const text = messageText(message).trim();
      if (text) return text;
    }
  }
  return "";
}
