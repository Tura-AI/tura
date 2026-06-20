import type { AgentAvatarConfig, Message, PersonaMediaConfig, Session } from "@tura/gateway-sdk";
import {
  AVATAR_WORKSPACE_CONFIG_KEY,
  avatarSettingsFromConfigValue,
  normalizeAvatarSettings,
} from "../components/avatar/agent-avatar-canvas";
import { type AppState, messageCreatedAt, partText } from "../state/global-store";
import { isReactionOnlyMessage } from "./conversation-protocol";
import { isToolPart } from "./message-tools";
export {
  messagesWithSessionThinking,
  sessionIsWorking,
  sessionShowsBusyAnimation,
} from "./session-animation";
export {
  conversationReactionItems,
  latestSticker,
  type ConversationReactionItem,
} from "./conversation-protocol";

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

