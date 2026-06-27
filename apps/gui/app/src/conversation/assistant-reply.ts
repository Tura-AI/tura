import type { Message } from "@tura/gateway-sdk";
import { isToolPart } from "./message-tools";

export type VisibleAssistantReplyCursor = ReadonlyMap<string, string>;

export function visibleAssistantReplyCursor(messages: Message[]): VisibleAssistantReplyCursor {
  const cursor = new Map<string, string>();
  for (const message of messages) {
    const signature = visibleAssistantReplySignature(message);
    if (signature) {
      cursor.set(message.id, signature);
    }
  }
  return cursor;
}

export function hasVisibleAssistantReply(messages: Message[]): boolean {
  return visibleAssistantReplyCursor(messages).size > 0;
}

export function hasVisibleNewAssistantReply(
  messages: Message[],
  knownReplies: VisibleAssistantReplyCursor,
): boolean {
  for (const [messageId, signature] of visibleAssistantReplyCursor(messages)) {
    if (knownReplies.get(messageId) !== signature) {
      return true;
    }
  }
  return false;
}

function visibleAssistantReplySignature(message: Message): string | undefined {
  if (message.role !== "assistant") {
    return undefined;
  }
  const texts = message.parts
    .filter((part) => !isToolPart(part))
    .map((part) => (part.text || part.content || "").trim())
    .filter((text) => text.length > 0 && !isTransientAssistantText(text));
  return texts.length > 0 ? texts.join("\u001f") : undefined;
}

function isTransientAssistantText(text: string): boolean {
  const normalized = text.trim().toLowerCase();
  return (
    normalized === "正在思考" ||
    normalized === "thinking" ||
    (normalized.includes("正在思考") && normalized.startsWith("已运行"))
  );
}
