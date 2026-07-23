import type { JsonObject } from "./common.js";
import { t } from "../i18n.js";

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
  last_user_message_at?: number | null;
  task_start_at?: number;
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
  metadata: RunResultMetadata;
}

export interface RunResultMetadata {
  input_token_usage: number;
  input_token_cache: number;
  provider_time_ms: number;
  total_time_ms: number;
  commands: number;
  failed_commands: number;
  tps: number;
  turns: number;
}

export function sessionTitle(session: Session): string {
  return (session.session_display_name || session.name || session.id || "New Session").toString();
}

export function isDraftSession(session: Session | undefined): boolean {
  return session?.draft === true;
}

export function sessionSortAt(session: Session): number {
  return session.last_user_message_at ?? 0;
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

export function sessionHasQuestionStatus(session: Session): boolean {
  const management = objectValue(session.task_management);
  if (management.status === "question") return true;
  const tasks = management.tasks;
  return Array.isArray(tasks) && tasks.some((task) => objectValue(task).status === "question");
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
    .map(messagePartText)
    .filter(Boolean)
    .join("");
}

export function messagePartText(part: MessagePart): string {
  if (isRuntimeStoppedPart(part)) return t("runtimeStopped");
  return part.text ?? part.content ?? "";
}

function isRuntimeStoppedPart(part: MessagePart): boolean {
  const metadata = objectValue(part.metadata);
  return metadata.code === "runtime_stopped";
}

function objectValue(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as JsonObject) : {};
}

export function messageSortValue(message: Message): number {
  return message.created_at as number;
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
  if (!value) return "";
  if (typeof value === "string") return value.trim();
  if (Array.isArray(value)) {
    return value.map(userFacingOutputText).filter(Boolean).join("");
  }
  if (typeof value !== "object") return "";
  const object = value as JsonObject;
  for (const key of [
    "task_status",
    "output",
    "text",
    "content",
    "finalText",
    "final_text",
    "message",
  ]) {
    const output = userFacingOutputText(object[key]);
    if (output) return output;
  }
  for (const key of ["task_group", "summary", "status", "label"]) {
    const output = userFacingOutputText(object[key]);
    if (output) return output;
  }
  return "";
}

function isUserFacingAssistantText(value: string): boolean {
  const text = value.trim();
  if (!text) return false;
  if (/completed without a user-facing message/i.test(text)) return false;
  return true;
}
