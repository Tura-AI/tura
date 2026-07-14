import type { SessionConfig } from "../types/config.js";
import type { TuraConfigResponse } from "../types/gateway.js";

export function runtimeModelFromConfig(
  config: SessionConfig | undefined,
  modelConfig?: TuraConfigResponse,
): string | undefined {
  const model = stringValue(config?.model);
  const tierModel = model ? modelForTier(modelConfig, model) : undefined;
  if (tierModel) return tierModel;

  const provider = stringValue(config?.active_provider);
  const activeModel = stringValue(config?.active_model);
  if (provider && activeModel) {
    return `${provider}/${stripProviderPrefix(provider, activeModel)}`;
  }

  if (model?.includes("/")) return model;
  if (provider && model && !isDefaultTierName(model)) {
    return `${provider}/${stripProviderPrefix(provider, model)}`;
  }
  return activeModel?.includes("/") ? activeModel : undefined;
}

export function modelForTier(
  modelConfig: TuraConfigResponse | undefined,
  tier: string,
): string | undefined {
  const current = modelConfig?.tiers?.find((item) => item.tier === tier)?.current;
  if (!current?.provider || !current.model) return undefined;
  return `${current.provider}/${stripProviderPrefix(current.provider, current.model)}`;
}

function stripProviderPrefix(provider: string, model: string): string {
  return model.startsWith(`${provider}/`) ? model.slice(provider.length + 1) : model;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function isDefaultTierName(value: string): boolean {
  return ["thinking", "fast"].includes(value.trim().toLowerCase());
}
