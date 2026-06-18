import type { Message, MessagePart, Session } from "./session.js";
import type { PermissionRequest, QuestionRequest } from "./permission.js";

export interface GatewayEventEnvelope {
  directory?: string;
  payload?: GatewayEventPayload;
}

export interface SessionEventProperties {
  sessionID: string;
  info: Session;
}

export interface SessionStatusEventProperties {
  sessionID: string;
  status: unknown;
  context_tokens: Session["context_tokens"];
  usage?: Session["usage"];
}

export interface MessageUpdatedEventProperties {
  sessionID: string;
  info: Message;
}

export interface MessageRemovedEventProperties {
  sessionID: string;
  messageID: string;
}

export interface MessagePartDeltaEventProperties {
  sessionID: string;
  messageID: string;
  partID: string;
  field: string;
  delta: string;
}

export interface MessagePartUpdatedEventProperties {
  sessionID: string;
  part: MessagePart;
}

export interface CommandUpdatedEventProperties {
  sessionID: string;
  messageID: string;
  partID: string;
  runtimeID: string;
  commandRunID: string;
  commandID: string;
  providerToolCallID?: string | null;
  commandIndex?: number | null;
  eventSeq?: number | null;
  status: string;
  command?: unknown;
  result?: unknown;
  updatedAt?: number | null;
}

export type GatewayEventPayload =
  | { type: "server.connected"; properties: Record<string, unknown> }
  | { type: "session.created"; properties: SessionEventProperties }
  | { type: "session.updated"; properties: SessionEventProperties }
  | { type: "session.deleted"; properties: SessionEventProperties }
  | { type: "session.status"; properties: SessionStatusEventProperties }
  | { type: "message.updated"; properties: MessageUpdatedEventProperties }
  | { type: "message.removed"; properties: MessageRemovedEventProperties }
  | { type: "message.part.delta"; properties: MessagePartDeltaEventProperties }
  | { type: "message.part.updated"; properties: MessagePartUpdatedEventProperties }
  | { type: "command.updated"; properties: CommandUpdatedEventProperties }
  | { type: "permission.asked"; properties: Record<string, unknown> }
  | { type: "permission.replied"; properties: Record<string, unknown> }
  | { type: "question.asked"; properties: Record<string, unknown> }
  | { type: "question.replied"; properties: Record<string, unknown> }
  | { type: "question.rejected"; properties: Record<string, unknown> }
  | { type: string; properties?: Record<string, unknown> };

export interface NormalizedEvent {
  type: string;
  directory: string;
  sessionID?: string;
  messageID?: string;
  partID?: string;
  status?: string;
  text?: string;
  tool?: string;
  commandID?: string;
  permission?: PermissionRequest;
  question?: QuestionRequest;
  raw: GatewayEventEnvelope;
}

export function eventSessionID(payload: GatewayEventPayload | undefined): string | undefined {
  const properties = payload?.properties as Record<string, unknown> | undefined;
  const direct = properties?.sessionID as string | undefined;
  if (direct) return direct;
  const info = properties?.info as Record<string, unknown> | undefined;
  const infoSession = info?.sessionID as string | undefined;
  if (infoSession) return infoSession;
  const part = properties?.part as Record<string, unknown> | undefined;
  const partSession = part?.sessionID as string | undefined;
  if (partSession) return partSession;
  if (
    payload?.type !== "permission.asked" &&
    payload?.type !== "permission.replied" &&
    payload?.type !== "question.asked" &&
    payload?.type !== "question.replied" &&
    payload?.type !== "question.rejected"
  ) {
    return undefined;
  }
  const request = (properties?.permission ??
    properties?.question ??
    properties?.request ??
    properties) as Record<string, unknown> | undefined;
  return request?.sessionID as string | undefined;
}
