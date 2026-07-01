import type { Agent, AgentConfig, AgentUpsertRequest, StoredAgent } from "@tura/gateway-sdk";
import { agentDisplayName } from "./utils/agent-display";

export function storedAgentFromRuntimeAgent(agent: Agent): StoredAgent {
  const capabilities = agentCapabilitiesFromOptions(agent.options);
  const displayName = agentDisplayName(agent);
  return {
    summary: {
      id: agent.name,
      name: displayName,
      description: agent.description,
      source: agent.native ? "static" : "dynamic",
      path: "",
      aliases: [],
      capabilities,
      provider: agentDefaultModelTierFromOptions(agent.options) ?? agent.model?.providerID ?? null,
      hidden: agent.hidden,
    },
    config: {
      agent_name: displayName,
      description: agent.description,
      aliases: [],
      provider: {
        default_model_tier: agentDefaultModelTierFromOptions(agent.options),
        tura_llm_name: agentDefaultModelTierFromOptions(agent.options),
      },
      agent_capabilities: capabilities.map((capability) => ({
        capability_name: capability,
        capability_directory: "crates/tools/src",
      })),
    },
    prompt: "",
  };
}

export function runtimeAgentFromUpsert(
  agentId: string | undefined,
  payload: AgentUpsertRequest,
): Agent {
  const name = payload.config?.agent_name || payload.id || agentId || "agent";
  return {
    name: agentId ?? payload.id ?? name,
    description: payload.config?.description ?? "",
    mode: "custom",
    native: false,
    hidden: false,
    model: null,
    options: {
      ...(payload.config?.avatar ? { avatar: payload.config.avatar } : {}),
      ...(payload.config?.provider ? { provider: payload.config.provider } : {}),
      capabilities: readCapabilityArray(payload.config?.agent_capabilities),
    },
    permission: { allow: [], deny: [] },
  };
}

export function storedAgentFromUpsert(agent: Agent, payload: AgentUpsertRequest): StoredAgent {
  const config: AgentConfig = payload.config ?? { agent_name: agent.name };
  const aliases = readStringArray(config.aliases);
  const capabilities = readCapabilityArray(config.agent_capabilities);
  return {
    summary: {
      id: agent.name,
      name: config.agent_name ?? agent.name,
      description: config.description ?? agent.description ?? "",
      source: "dynamic",
      path: "",
      aliases,
      capabilities,
      provider: agent.model?.providerID ?? null,
      hidden: agent.hidden,
    },
    config,
    prompt: payload.prompt ?? "",
  };
}

function readStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function readCapabilityArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value
        .map((item) => {
          if (typeof item === "string") {
            return item;
          }
          if (
            item &&
            typeof item === "object" &&
            "capability_name" in item &&
            typeof item.capability_name === "string"
          ) {
            return item.capability_name;
          }
          return undefined;
        })
        .filter((item): item is string => !!item)
    : [];
}

function agentDefaultModelTierFromOptions(options: Record<string, unknown>): string | undefined {
  const provider = options.provider;
  if (!provider || typeof provider !== "object" || Array.isArray(provider)) {
    return undefined;
  }
  const record = provider as Record<string, unknown>;
  const tier = record.default_model_tier ?? record.tura_llm_name;
  return typeof tier === "string" ? tier : undefined;
}

function agentCapabilitiesFromOptions(options: Record<string, unknown>): string[] {
  const capabilities = options.capabilities;
  return Array.isArray(capabilities)
    ? capabilities.filter((item): item is string => typeof item === "string")
    : [];
}
