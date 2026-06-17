import type { AgentAvatarConfig, Message, PersonaMediaConfig, Session } from "@tura/gateway-sdk";
import {
  AVATAR_WORKSPACE_CONFIG_KEY,
  avatarSettingsFromConfigValue,
  normalizeAvatarSettings,
} from "../components/avatar/agent-avatar-canvas";
import { type AppState, messageCreatedAt, partText, sessionUpdatedAt } from "../state/global-store";
import { reactionEmojiValues, stickerEmojiValues, stripReactionEmoji } from "./message-rich-text";
import { isToolPart } from "./message-tools";

export type ConversationReactionItem = {
  message: Message;
  reactions: string[];
};

export function groupConversationTurns(messages: Message[]): Message[] {
  const grouped: Message[] = [];
  let assistantGroup: Message[] = [];

  function flushAssistantGroup() {
    if (assistantGroup.length === 0) {
      return;
    }
    grouped.push(mergeAssistantMessages(assistantGroup));
    assistantGroup = [];
  }

  for (const message of messages) {
    if (message.role === "assistant") {
      if (isReactionOnlyMessage(message)) {
        flushAssistantGroup();
        grouped.push(message);
        continue;
      }
      assistantGroup.push(message);
      continue;
    }
    flushAssistantGroup();
    grouped.push(message);
  }
  flushAssistantGroup();
  return grouped;
}

export function avatarConfigForAgent(
  agents: AppState["agents"],
  selectedAgentId: string | undefined,
  workspaceConfig: AppState["workspaceConfig"],
): AgentAvatarConfig {
  if (workspaceConfig[AVATAR_WORKSPACE_CONFIG_KEY]) {
    return avatarSettingsFromConfigValue(workspaceConfig[AVATAR_WORKSPACE_CONFIG_KEY]);
  }
  const selected =
    agents.find((agent) => agent.name === selectedAgentId) ?? agents.find((agent) => !agent.hidden);
  return normalizeAvatarSettings(
    selected?.options?.avatar as Partial<AgentAvatarConfig> | undefined,
  );
}

export function personaMediaForAvatar(
  personas: AppState["personas"],
  avatar: AgentAvatarConfig,
): PersonaMediaConfig | undefined {
  const personaId = avatar.persona_id ?? avatar.role;
  return (
    personas.find((persona) => persona.summary.id === personaId)?.summary.media ??
    personas.find((persona) => persona.summary.id === personaId)?.config.media ??
    undefined
  );
}

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
    if (
      message.role === "assistant" &&
      reactions.length > 0 &&
      messageWithoutReactionsText(message).trim().length === 0
    ) {
      const target = [...items].reverse().find((item) => item.message.role === "user");
      if (target) {
        target.reactions = [...target.reactions, ...reactions].slice(0, 4);
        continue;
      }
    }
    items.push({
      message,
      reactions: message.role === "user" ? reactions : [],
    });
  }
  return items;
}

export function messagesWithSessionThinking(
  messages: Message[],
  session: Session | undefined,
): Message[] {
  if (!session || !sessionIsWorking(session.status)) {
    return messages;
  }
  if (messages.at(-1)?.role === "assistant") {
    return messages;
  }
  return [...messages, sessionThinkingMessage(session)];
}

function mergeAssistantMessages(messages: Message[]): Message {
  const first = messages[0]!;
  const last = messages.at(-1)!;
  const withText = [...messages]
    .reverse()
    .find((message) => message.parts.some((part) => !isToolPart(part) && partText(part).trim()));
  const providerMessage = withText ?? last;
  return {
    ...providerMessage,
    id: messages.map((message) => message.id).join("+"),
    created_at: first.created_at ?? first.time?.created,
    updated_at: last.updated_at ?? last.time?.updated,
    time: {
      created: messageCreatedAt(first),
      updated: last.time?.updated ?? last.updated_at ?? messageCreatedAt(last),
    },
    parts: messages.flatMap((message) => message.parts),
  };
}

function sessionThinkingMessage(session: Session): Message {
  const updatedAt = sessionUpdatedAt(session) ?? Date.now();
  return {
    id: `session-thinking:${session.id}`,
    sessionID: session.id,
    role: "assistant",
    created_at: updatedAt,
    updated_at: updatedAt,
    time: { created: updatedAt, updated: updatedAt },
    parts: [],
  };
}

export function sessionIsWorking(status: Session["status"] | undefined): boolean {
  return status !== undefined && status !== "idle";
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

function isReactionOnlyMessage(message: Message): boolean {
  return (
    message.role === "assistant" &&
    messageReactionEmojis(message).length > 0 &&
    messageWithoutReactionsText(message).trim().length === 0 &&
    message.parts.every((part) => !isToolPart(part))
  );
}
