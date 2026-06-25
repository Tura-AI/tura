export type PermissionRequest = {
  id: string;
  session_id: string;
  permission: string;
  args: Record<string, unknown>;
};

export type QuestionRequest = {
  id: string;
  session_id: string;
  question: string;
  metadata: Record<string, unknown>;
};

export type TodoItem = {
  id: string;
  content?: string;
  status?: string;
  priority?: string;
  [key: string]: unknown;
};

export type CommandUpdate = {
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
  createdAt?: number | null;
  updatedAt?: number | null;
};

export type GatewayEventPayload = {
  type: string;
  properties?: Record<string, unknown>;
};

export type GatewayEventEnvelope = {
  directory?: string | null;
  payload: GatewayEventPayload;
};
