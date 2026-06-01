import type { Agent, StoredAgent } from "@tura/gateway-sdk";

export const CONFIGURABLE_AGENT_IDS = [
  "coding_agent_planning",
  "coding_agent_fast",
  "coding_agent_instant",
] as const;
const CONFIGURABLE_AGENT_ID_SET = new Set<string>(CONFIGURABLE_AGENT_IDS);

export function visibleConfigurableAgents(agents: Agent[]): Agent[] {
  const visibleAgents = agents.filter((agent) => !agent.hidden);
  const defaultAgents = visibleAgents.filter((agent) =>
    CONFIGURABLE_AGENT_ID_SET.has(agent.name),
  );
  return defaultAgents.length > 0 ? defaultAgents : visibleAgents;
}

export function agentDisplayName(agent?: Agent, stored?: StoredAgent): string {
  const agentId = agent?.name ?? stored?.summary.id ?? "";
  const configuredName =
    cleanDisplayName(stored?.summary.name, agentId) ??
    cleanDisplayName(
      readOptionString(agent?.options, "display_name"),
      agentId,
    ) ??
    cleanDisplayName(readOptionString(agent?.options, "name"), agentId);

  return configuredName ?? humanizeIdentifier(agentId);
}

function cleanDisplayName(
  value: string | null | undefined,
  agentId: string,
): string | undefined {
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
