export const AGENT_REASONING_LEVELS = ["low", "medium", "high", "xhigh", "max"] as const;

export type AgentReasoningLevel = (typeof AGENT_REASONING_LEVELS)[number];

export type AgentRuntimeModel = {
  provider: string;
  model: string;
};

export type AgentRuntimeConfig = {
  defaultModelTier: string;
  currentModel?: AgentRuntimeModel;
  reasoningLevel: AgentReasoningLevel;
  priorityEnabled: boolean;
};

export type AgentRuntimeFallback = {
  model?: string;
  modelConfig?: ModelTierConfigLike;
  reasoningLevel?: string;
  priorityEnabled?: boolean;
};

export type ModelTierConfigLike = {
  tiers?: Array<{
    tier: string;
    current?: { provider?: string; model?: string } | null;
  }>;
};

export type AgentRuntimeRequest = {
  model?: string;
  variant?: AgentReasoningLevel;
  model_acceleration_enabled: boolean;
};

type RuntimeAgentLike = {
  model?: { providerID?: string; modelID?: string } | null;
  options?: Record<string, unknown>;
};

type StoredAgentLike = {
  config?: { provider?: unknown };
};

export function agentRuntimeConfig(
  agent?: RuntimeAgentLike,
  stored?: StoredAgentLike,
): AgentRuntimeConfig {
  const providers = [stored?.config?.provider, agent?.options?.provider, agent?.options];
  return {
    defaultModelTier: agentDefaultModelTier(providers),
    currentModel: agentCurrentModel(providers, agent),
    reasoningLevel: normalizeAgentReasoningLevel(
      firstProviderString(providers, [
        "model_reasoning_effort",
        "reasoning_effort",
        "model_variant",
      ]),
    ),
    priorityEnabled: agentPriorityEnabled(providers),
  };
}

export function agentRuntimeRequest(
  agent: RuntimeAgentLike | undefined,
  fallback: AgentRuntimeFallback = {},
): AgentRuntimeRequest {
  if (!agent) {
    return {
      model: fallback.model,
      variant: normalizeAgentReasoningLevel(fallback.reasoningLevel),
      model_acceleration_enabled: fallback.priorityEnabled ?? false,
    };
  }
  const config = agentRuntimeConfig(agent);
  return {
    model:
      modelPairText(config.currentModel) ??
      modelForRuntimeTier(fallback.modelConfig, config.defaultModelTier) ??
      fallback.model,
    variant: config.reasoningLevel,
    model_acceleration_enabled: config.priorityEnabled,
  };
}

export function modelForRuntimeTier(
  modelConfig: ModelTierConfigLike | undefined,
  tier: string,
): string | undefined {
  const current = modelConfig?.tiers?.find((item) => item.tier === tier)?.current;
  if (!current?.provider || !current.model) return undefined;
  return `${current.provider}/${current.model.startsWith(`${current.provider}/`) ? current.model.slice(current.provider.length + 1) : current.model}`;
}

export function applyAgentRuntimeConfig<T extends { provider?: unknown }>(
  config: T,
  settings: {
    defaultModelTier: string;
    currentModel?: string;
    reasoningLevel: string;
    priorityEnabled: boolean;
  },
): T {
  const provider = providerRecord(config.provider);
  if (settings.currentModel?.trim()) {
    provider.current_model = settings.currentModel.trim();
  } else {
    delete provider.current_model;
  }
  provider.default_model_tier = settings.defaultModelTier;
  provider.tura_llm_name = settings.defaultModelTier;
  provider.model_reasoning_effort = normalizeAgentReasoningLevel(settings.reasoningLevel);
  if (settings.priorityEnabled) {
    provider.model_acceleration_enabled = true;
    provider.service_tier = "priority";
  } else {
    delete provider.model_acceleration_enabled;
    delete provider.service_tier;
  }
  return { ...config, provider };
}

export function normalizeAgentReasoningLevel(value: string | undefined): AgentReasoningLevel {
  const normalized = value?.trim().toLowerCase();
  if (normalized === "low" || normalized === "medium" || normalized === "high") {
    return normalized;
  }
  if (normalized === "xhigh" || normalized === "highest") {
    return "xhigh";
  }
  if (normalized === "max") {
    return "max";
  }
  return "high";
}

export function parseProviderModel(value: string | undefined): AgentRuntimeModel | undefined {
  const trimmed = value?.trim();
  if (!trimmed?.includes("/")) {
    return undefined;
  }
  const [provider, ...modelParts] = trimmed.split("/");
  const model = modelParts.join("/");
  return provider && model ? { provider, model } : undefined;
}

export function modelPairText(value: AgentRuntimeModel | undefined): string | undefined {
  return value ? `${value.provider}/${value.model}` : undefined;
}

export function formatAgentRuntimeModelText(
  model: string,
  runtime: Pick<AgentRuntimeConfig, "reasoningLevel" | "priorityEnabled">,
  priorityText: string,
): string {
  return [model, runtime.reasoningLevel, runtime.priorityEnabled ? priorityText : ""]
    .filter(Boolean)
    .join(" - ");
}

function agentDefaultModelTier(providers: unknown[]): string {
  for (const provider of providers) {
    const explicit = readProviderString(provider, ["default_model_tier"]);
    if (explicit) return explicit;
    const legacy = readProviderString(provider, ["tura_llm_name"]);
    if (legacy && !legacy.includes("/")) return legacy;
  }
  return "thinking";
}

function agentCurrentModel(
  providers: unknown[],
  agent?: RuntimeAgentLike,
): AgentRuntimeModel | undefined {
  for (const provider of providers) {
    const current = parseProviderModel(readProviderString(provider, ["current_model"]));
    if (current) return current;
  }
  for (const provider of providers) {
    const legacy = parseProviderModel(readProviderString(provider, ["tura_llm_name"]));
    if (legacy) return legacy;
  }
  return agent?.model?.providerID && agent.model.modelID
    ? { provider: agent.model.providerID, model: agent.model.modelID }
    : undefined;
}

function agentPriorityEnabled(providers: unknown[]): boolean {
  for (const provider of providers) {
    const enabled = readProviderBoolean(provider, ["model_acceleration_enabled", "accelerated"]);
    if (enabled !== undefined) return enabled;
  }
  for (const provider of providers) {
    const tier = readProviderString(provider, ["service_tier"]);
    if (tier) return tier === "priority";
  }
  return false;
}

function firstProviderString(providers: unknown[], keys: string[]): string | undefined {
  for (const provider of providers) {
    const value = readProviderString(provider, keys);
    if (value) return value;
  }
  return undefined;
}

function readProviderString(value: unknown, keys: string[]): string | undefined {
  const record = objectRecord(value);
  if (!record) return undefined;
  for (const key of keys) {
    const field = record[key];
    if (typeof field === "string" && field.trim() && field.trim().toLowerCase() !== "default") {
      return field.trim();
    }
  }
  return undefined;
}

function readProviderBoolean(value: unknown, keys: string[]): boolean | undefined {
  const record = objectRecord(value);
  if (!record) return undefined;
  for (const key of keys) {
    const field = record[key];
    if (typeof field === "boolean") return field;
    if (typeof field === "string") {
      const normalized = field.trim().toLowerCase();
      if (["1", "true", "yes", "on"].includes(normalized)) return true;
      if (["0", "false", "no", "off"].includes(normalized)) return false;
    }
  }
  return undefined;
}

function providerRecord(value: unknown): Record<string, unknown> {
  return objectRecord(value) ? { ...objectRecord(value) } : {};
}

function objectRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}
