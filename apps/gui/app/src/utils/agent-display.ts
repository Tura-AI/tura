import type { Agent, StoredAgent } from "@tura/gateway-sdk";

export const CONFIGURABLE_AGENT_IDS = [
  "thinking",
  "thinking-planning",
  "fast",
  "fast-text-only",
] as const;
const CONFIGURABLE_AGENT_ID_SET = new Set<string>(CONFIGURABLE_AGENT_IDS);
const CONFIGURABLE_AGENT_ORDER = new Map<string, number>(
  CONFIGURABLE_AGENT_IDS.map((id, index) => [id, index]),
);

export function visibleConfigurableAgents(agents: Agent[]): Agent[] {
  const visibleAgents = agents.filter((agent) => !agent.hidden);
  const defaultAgents = visibleAgents.filter((agent) =>
    CONFIGURABLE_AGENT_ID_SET.has(agent.name),
  );
  return defaultAgents.length > 0
    ? [...defaultAgents].sort(
        (left, right) =>
          (CONFIGURABLE_AGENT_ORDER.get(left.name) ?? Number.MAX_SAFE_INTEGER) -
          (CONFIGURABLE_AGENT_ORDER.get(right.name) ?? Number.MAX_SAFE_INTEGER),
      )
    : visibleAgents;
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
