import type { JsonObject } from "./common.js";

export type SessionStatusValue = "idle" | "busy" | "error";
export type PlanStatus =
  | "todo"
  | "doing"
  | "question"
  | "done"
  | "archived";
export type StartCondition =
  | "session_idle"
  | "user_action"
  | "scheduled_task"
  | "polling_task";

export interface PollInterval {
  m?: number;
  d?: number;
  h?: number;
  s?: number;
}

export interface TaskManagement {
  nonce_id?: string;
  step?: number;
  task_summary?: string;
  delivery?: string;
  sub_session_id?: string;
  start_at?: string | number;
  poll_interval?: PollInterval;
  status?: PlanStatus;
  plan_summary?: string;
  tasks?: TaskManagement[];
}

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
  kill_processes_on_start?: boolean;
  validator_enabled?: boolean;
  force_multiple_tasks?: boolean;
  model_variant?: string | null;
  model_acceleration_enabled?: boolean;
  status?: SessionStatusValue;
  message_count?: number;
  task_management?: TaskManagement;
  plan_summary?: string | null;
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
  task_management?: TaskManagement;
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
  return (
    session.session_display_name ||
    session.plan_summary ||
    session.name ||
    session.id ||
    "New Session"
  ).toString();
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

export function sessionTaskManagement(session: Session): TaskManagement {
  return session.task_management ?? {};
}

export function sessionPlanSummary(session: Session): string {
  const task = sessionTaskManagement(session);
  return (
    session.plan_summary ||
    task.plan_summary ||
    sessionTitle(session)
  ).toString();
}

export function sessionTaskSummary(session: Session): string {
  const task = sessionTaskManagement(session);
  return (
    task.task_summary ||
    sessionPlanSummary(session)
  ).toString();
}

export function sessionPlanStatus(session: Session): PlanStatus {
  const task = sessionTaskManagement(session);
  const status = task.status;
  return isLifecyclePlanStatus(status) ? status : "todo";
}

export const sessionTaskStatus = sessionPlanStatus;

export function sessionStartCondition(session: Session): StartCondition {
  const task = sessionTaskManagement(session);
  if (hasPollInterval(task.poll_interval)) return "polling_task";
  return task.start_at ? "scheduled_task" : "user_action";
}

export function sessionStartAt(session: Session): string | number | undefined {
  const task = sessionTaskManagement(session);
  return task.start_at;
}

export function sessionPollInterval(session: Session): PollInterval {
  const task = sessionTaskManagement(session);
  return task.poll_interval ?? {};
}

export function sessionDirectory(session: Session): string {
  return session.directory ?? "";
}

export function isPlanStatus(value: unknown): value is PlanStatus {
  return isLifecyclePlanStatus(value);
}

function isLifecyclePlanStatus(value: unknown): value is Exclude<PlanStatus, StartCondition> {
  return value === "todo" || value === "doing" || value === "question" || value === "done" || value === "archived";
}

function hasPollInterval(value: PollInterval | undefined): boolean {
  return Boolean(value && (value.m || value.d || value.h || value.s));
}

export function isStartCondition(value: unknown): value is StartCondition {
  return value === "session_idle" || value === "user_action" || value === "scheduled_task" || value === "polling_task";
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
