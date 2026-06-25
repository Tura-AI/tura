import type { Agent, StoredAgent } from "@tura/gateway-sdk";
import { t } from "../i18n";

export function visibleConfigurableAgents(agents: Agent[]): Agent[] {
  return agents.filter((agent) => !agent.hidden);
}

export function agentDisplayName(agent?: Agent, stored?: StoredAgent): string {
  const agentId = agent?.name ?? stored?.summary.id ?? "";
  const configuredName =
    cleanDisplayName(stored?.summary.name, agentId) ??
    cleanDisplayName(readOptionString(agent?.options, "display_name"), agentId) ??
    cleanDisplayName(readOptionString(agent?.options, "name"), agentId);

  return configuredName ?? humanizeIdentifier(agentId);
}

export function agentDescription(agent?: Agent, stored?: StoredAgent): string {
  const agentId = agent?.name ?? stored?.summary.id ?? "";
  const translated = builtinAgentDescription(agentId);
  if (translated) {
    return translated;
  }
  return stored?.summary.description ?? agent?.description ?? "";
}

function cleanDisplayName(value: string | null | undefined, agentId: string): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed || trimmed === agentId) {
    return undefined;
  }
  return trimmed;
}

function readOptionString(
  options: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = options?.[key];
  return typeof value === "string" ? value : undefined;
}

function humanizeIdentifier(value: string): string {
  return value
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
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
