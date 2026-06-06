import type { Message, Session } from "./session.js";
import type { PermissionRequest, QuestionRequest } from "./permission.js";

export interface GatewayEventEnvelope {
  directory?: string;
  payload?: GatewayEventPayload;
  [key: string]: unknown;
}

export type GatewayEventPayload =
  | { type: "server.connected"; properties: Record<string, unknown> }
  | {
      type: "session.created";
      properties: { sessionID?: string; session_id?: string; info: Session };
    }
  | {
      type: "session.updated";
      properties: { sessionID?: string; session_id?: string; info: Session };
    }
  | {
      type: "session.deleted";
      properties: { sessionID?: string; session_id?: string; info: Session };
    }
  | {
      type: "session.status";
      properties: { sessionID?: string; session_id?: string; status: unknown };
    }
  | {
      type: "message.updated";
      properties: { sessionID?: string; session_id?: string; info: Message };
    }
  | { type: "message.removed"; properties: { session_id?: string; message_id?: string } }
  | { type: "message.part.delta"; properties: Record<string, unknown> }
  | { type: "message.part.updated"; properties: Record<string, unknown> }
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
  permission?: PermissionRequest;
  question?: QuestionRequest;
  raw: GatewayEventEnvelope;
}

export function eventSessionID(payload: GatewayEventPayload | undefined): string | undefined {
  const properties = payload?.properties as Record<string, unknown> | undefined;
  const direct = (properties?.sessionID ?? properties?.session_id) as string | undefined;
  if (direct) return direct;
  const info = properties?.info as Record<string, unknown> | undefined;
  const infoSession = (info?.sessionID ?? info?.session_id) as string | undefined;
  if (infoSession) return infoSession;
  const part = properties?.part as Record<string, unknown> | undefined;
  const partSession = (part?.sessionID ?? part?.session_id) as string | undefined;
  if (partSession) return partSession;
  const request = (properties?.permission ??
    properties?.question ??
    properties?.request ??
    properties) as Record<string, unknown> | undefined;
  return (request?.sessionID ?? request?.session_id) as string | undefined;
}
