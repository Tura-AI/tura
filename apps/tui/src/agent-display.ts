import { t } from "./i18n.js";
import type { StoredAgent } from "./types/agent.js";

export function agentDescription(agent: StoredAgent): string {
  const id = agent.summary?.id ?? stringField(agent.config, "agent_name") ?? "";
  const translated = builtinAgentDescription(id);
  if (translated) return translated;
  return agent.summary?.description ?? stringField(agent.config, "description") ?? "";
}

function builtinAgentDescription(agentId: string): string | undefined {
  switch (agentId) {
    case "direct":
    case "direct-text-only":
    case "fast":
    case "fast-text-only":
      return t("agentDescriptionDirect");
    case "balanced":
    case "thinking":
      return t("agentDescriptionBalanced");
    case "thoughtful":
    case "thinking-planning":
      return t("agentDescriptionThoughtful");
    default:
      return undefined;
  }
}

function stringField(value: unknown, key: string): string | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const field = (value as Record<string, unknown>)[key];
  return typeof field === "string" && field.trim() ? field : undefined;
}
