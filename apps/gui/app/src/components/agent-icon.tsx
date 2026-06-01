import type { Agent } from "@tura/gateway-sdk";
import { classNames } from "../state/format";

export function AgentIcon(props: {
  agent?: Agent;
  agentId?: string;
  class?: string;
}) {
  return (
    <span class={classNames("agent-icon", props.class)} aria-hidden="true">
      {agentIconEmoji(props.agent, props.agentId)}
    </span>
  );
}

export function agentIconEmoji(agent?: Agent, agentId?: string): string {
  const configured = readOptionString(agent?.options, "icon_emoji");
  if (configured) {
    return configured;
  }
  const id = agent?.name ?? agentId;
  if (id === "coding_agent_planning") {
    return "🧭";
  }
  if (id === "coding_agent_fast") {
    return "🚀";
  }
  return "⚡";
}

function readOptionString(
  options: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = options?.[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
