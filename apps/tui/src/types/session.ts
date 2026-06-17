import type { JsonObject } from "./common.js";

export type SessionStatusValue = "idle" | "busy" | "error";

export interface SessionUsage {
  context_tokens?: {
    input?: number;
    limit?: number;
  } | null;
  tokens?: unknown;
  cost?: number | null;
  currency?: string | null;
}

export interface Session {
  id: string;
  draft?: boolean;
  name?: string | null;
  parent_id?: string | null;
  created_at?: number;
  updated_at?: number;
  directory?: string | null;
  model?: string | null;
  agent?: string | null;
  session_type?: string | null;
  auto_session_name?: boolean;
  kill_processes_on_start?: boolean;
  validator_enabled?: boolean;
  force_planning?: boolean;
  model_variant?: string | null;
  model_acceleration_enabled?: boolean;
  disable_permission_restrictions?: boolean;
  status?: SessionStatusValue;
  message_count?: number;
  task_management?: unknown;
  context_tokens?: {
    input?: number;
    limit?: number;
  } | null;
  usage?: SessionUsage | null;
  plan_summary?: string | null;
  session_display_name?: string | null;
}

export interface MessagePart {
  id: string;
  sessionID?: string;
  messageID?: string;
  type: string;
  text?: string | null;
  content?: string | null;
  metadata?: unknown;
  callID?: string | null;
  tool?: string | null;
  state?: unknown;
}

export interface Message {
  id: string;
  sessionID?: string;
  parentID?: string | null;
  role: "user" | "assistant" | "system";
  parts: MessagePart[];
  created_at?: number;
  updated_at?: number;
  time?: { created: number; updated: number };
  cost?: number;
  providerID?: string;
  modelID?: string;
  tokens?: unknown;
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
  force_planning?: boolean;
  auto_session_name?: boolean;
}

export interface ForkSessionRequest {
  directory?: string;
  model?: string;
  agent?: string;
  copy_context?: boolean;
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

export function isDraftSession(session: Session | undefined): boolean {
  return session?.draft === true;
}

export function sessionUpdatedAt(session: Session): number {
  return session.updated_at ?? session.created_at ?? 0;
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

export function messageSessionID(message: Message): string {
  return message.sessionID ?? "";
}

export function partMessageID(part: MessagePart): string {
  return part.messageID ?? "";
}

export function partSessionID(part: MessagePart): string {
  return part.sessionID ?? "";
}

export function messageText(message: Message): string {
  return (message.parts ?? [])
    .filter((part) => part.type === "text" || part.type === "message" || !part.type)
    .map((part) => part.text ?? part.content ?? "")
    .filter((text) => !isInternalTaskStatusText(text))
    .filter(Boolean)
    .join("");
}

export function messageSortValue(message: Message): number {
  return message.created_at ?? message.time?.created ?? 0;
}

export function lastAssistantText(messages: Message[]): string {
  const ordered = messages
    .map((message, index) => ({ message, index }))
    .sort(
      (left, right) =>
        messageSortValue(right.message) - messageSortValue(left.message) ||
        right.index - left.index,
    );
  for (const { message } of ordered) {
    if (message.role === "assistant") {
      const text = assistantResultText(message).trim();
      if (isUserFacingAssistantText(text)) return text;
    }
  }
  return "";
}

export function hasUserFacingAssistantText(messages: Message[], startIndex = 0): boolean {
  return messages
    .slice(startIndex)
    .some(
      (message) =>
        message.role === "assistant" && isUserFacingAssistantText(assistantResultText(message)),
    );
}

function assistantResultText(message: Message): string {
  const text = messageText(message).trim();
  if (text) return text;
  return (message.parts ?? []).map(partResultText).filter(Boolean).join("");
}

function partResultText(part: MessagePart): string {
  for (const value of [part.state, part.metadata]) {
    const output = userFacingOutputText(value);
    if (output) return output;
  }
  return "";
}

function userFacingOutputText(value: unknown): string {
  if (!value || isInternalTaskStatusPayload(value)) return "";
  if (typeof value === "string") return isInternalTaskStatusText(value) ? "" : value.trim();
  if (Array.isArray(value)) {
    return value.map(userFacingOutputText).filter(Boolean).join("");
  }
  if (typeof value !== "object") return "";
  const object = value as JsonObject;
  for (const key of ["output", "text", "content", "finalText", "final_text", "message"]) {
    const output = userFacingOutputText(object[key]);
    if (output) return output;
  }
  return "";
}

function isUserFacingAssistantText(value: string): boolean {
  const text = value.trim();
  if (!text) return false;
  if (isInternalTaskStatusText(text)) return false;
  if (/completed without a user-facing message/i.test(text)) return false;
  return true;
}

export function isInternalTaskStatusPart(part: MessagePart): boolean {
  if (part.type === "text" || part.type === "message" || !part.type) {
    return isInternalTaskStatusText(part.text ?? part.content ?? "");
  }
  if (part.tool === "command_run") {
    const state = part.state && typeof part.state === "object" ? (part.state as JsonObject) : {};
    return (
      isInternalTaskStatusPayload(state.input) ||
      isInternalTaskStatusPayload(state.output) ||
      isInternalTaskStatusPayload(part.metadata)
    );
  }
  return false;
}

export function isInternalTaskStatusText(value: unknown): boolean {
  if (typeof value !== "string") return false;
  const text = value.trim();
  if (!text) return false;
  if (/^(?:doing|done|question)\s*:\s*\{\s*\}$/iu.test(text)) return true;
  if (/^(?:done|question)\s*:\s+\S[\s\S]*$/iu.test(text)) return true;
  if (/^\[?command_run:\s*/iu.test(text)) {
    const payload = text.replace(/^\[?command_run:\s*/iu, "").replace(/\]\s*$/u, "");
    return isInternalTaskStatusText(payload);
  }
  const normalized = text.replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  try {
    return isInternalTaskStatusPayload(JSON.parse(normalized) as unknown);
  } catch {
    return false;
  }
}

export function isInternalTaskStatusPayload(value: unknown): boolean {
  if (!value) return false;
  if (typeof value === "string") return isInternalTaskStatusText(value);
  if (Array.isArray(value)) return value.length > 0 && value.every(isInternalTaskStatusPayload);
  if (typeof value !== "object") return false;
  const object = value as JsonObject;
  const commandType = stringField(object, "command_type") ?? stringField(object, "command");
  if (commandType?.trim().toLowerCase().replace(/-/g, "_") === "task_status") return true;
  if ("task_status" in object) return true;
  if (taskStatusOnlyObject(object)) return true;
  if ("output" in object && isInternalTaskStatusPayload(object.output)) return true;
  if ("input" in object && isInternalTaskStatusPayload(object.input)) return true;
  if ("results" in object && isInternalTaskStatusPayload(object.results)) return true;
  return false;
}

function taskStatusOnlyObject(object: JsonObject): boolean {
  const allowed = new Set(["status", "task_detail", "summary", "label"]);
  const keys = Object.keys(object);
  return (
    keys.length > 0 &&
    keys.every((key) => allowed.has(key)) &&
    ("task_detail" in object || "summary" in object || taskStatusValue(object.status))
  );
}

function taskStatusValue(value: unknown): boolean {
  return typeof value === "string" && /^(doing|done|question)$/iu.test(value.trim());
}

function stringField(object: JsonObject, key: string): string | undefined {
  const value = object[key];
  return typeof value === "string" ? value : undefined;
}
