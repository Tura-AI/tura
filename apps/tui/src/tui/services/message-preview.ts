import type { Message } from "../../types/session.js";
import { messageText } from "../../types/session.js";

export function lastMessagePreview(
  messages: Message[],
  excludeMessageId?: string,
): string | undefined {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (!message || message.id === excludeMessageId) continue;
    const text = messageText(message).replace(/\s+/g, " ").trim();
    if (text) return text;
  }
  return undefined;
}
