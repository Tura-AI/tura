import type { AgentAvatarConfig, PersonaMediaConfig } from "@tura/gateway-sdk";
import {
  AVATAR_WORKSPACE_CONFIG_KEY,
  avatarSettingsFromConfigValue,
  normalizeAvatarSettings,
} from "../components/avatar/agent-avatar-canvas";
import { type AppState } from "../state/global-store";
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

export { groupConversationTurns } from "./conversation-turns";

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
