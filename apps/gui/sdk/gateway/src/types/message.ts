export type MessageRole = "user" | "assistant" | "system";

export type MessagePart = {
  id: string;
  sessionID: string;
  messageID: string;
  type: string;
  content?: string | null;
  text?: string | null;
  metadata?: unknown;
  callID?: string | null;
  tool?: string | null;
  state?: unknown;
};

export type Message = {
  id: string;
  sessionID: string;
  parentID?: string | null;
  role: MessageRole;
  parts: MessagePart[];
  time?: {
    created?: number;
    updated?: number;
  };
  created_at?: number;
  updated_at?: number;
  cost?: number;
  providerID?: string;
  modelID?: string;
  metadata?: unknown;
  tokens?: unknown;
};

export type MessageListInput = {
  limit?: number;
  before?: string;
  after?: string;
};

export type SendMessageResponse = {
  message: Message;
};
