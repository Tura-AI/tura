import type { Message } from "@tura/gateway-sdk";
import { partText } from "../state/global-store";
import {
  reactionEmojiValues,
  stickerEmojiValues,
  stripReactionEmoji,
} from "./message-rich-protocol";
import { isToolPart } from "./message-tools";

export type ConversationReactionItem = {
  message: Message;
  reactions: string[];
};

export function latestSticker(messages: Message[]): string | undefined {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const stickers = messages[index]!.parts.filter((part) => !isToolPart(part)).flatMap((part) =>
      stickerEmojiValues(partText(part)),
    );
    const sticker = stickers.at(-1);
    if (sticker) {
      return sticker;
    }
  }
  return undefined;
}

export function conversationReactionItems(messages: Message[]): ConversationReactionItem[] {
  const items: ConversationReactionItem[] = [];
  for (const message of messages) {
    const reactions = messageReactionEmojis(message);
    if (message.role === "assistant" && reactions.length > 0) {
      const target = [...items].reverse().find((item) => item.message.role === "user");
      if (target) {
        target.reactions = [...target.reactions, ...reactions].slice(0, 4);
      }
      if (
        messageWithoutReactionsText(message).trim().length === 0 &&
        message.parts.every((part) => !isToolPart(part))
      ) {
        continue;
      }
    }
    items.push({
      message,
      reactions: [],
    });
  }
  return items;
}

export function isReactionOnlyMessage(message: Message): boolean {
  return (
    message.role === "assistant" &&
    messageReactionEmojis(message).length > 0 &&
    messageWithoutReactionsText(message).trim().length === 0 &&
    message.parts.every((part) => !isToolPart(part))
  );
}

function messageReactionEmojis(message: Message): string[] {
  return message.parts
    .filter((part) => !isToolPart(part))
    .flatMap((part) => reactionEmojiValues(partText(part)));
}

function messageWithoutReactionsText(message: Message): string {
  return message.parts
    .filter((part) => !isToolPart(part))
    .map((part) => stripReactionEmoji(partText(part)))
    .join("\n");
}
