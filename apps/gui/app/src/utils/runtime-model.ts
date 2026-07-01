import type { TuraConfigResponse } from "@tura/gateway-sdk";

export function workspaceModelPatch(model: string | undefined): Record<string, unknown> {
  if (!model) return {};
  const [provider, ...modelParts] = model.split("/");
  const modelID = modelParts.join("/");
  if (!provider || !modelID) return { model };
  return {
    model,
    active_provider: provider,
    active_model: modelID.startsWith(`${provider}/`) ? modelID.slice(provider.length + 1) : modelID,
  };
}

export function workspaceModelFromConfig(
  config: Record<string, unknown>,
  modelConfig?: TuraConfigResponse,
): string | undefined {
  const model = readConfigString(config, "model");
  const tierModel = model ? modelForTier(modelConfig, model) : undefined;
  if (tierModel) return tierModel;

  const provider = readConfigString(config, "active_provider");
  const activeModel = readConfigString(config, "active_model");
  if (provider && activeModel) {
    return `${provider}/${activeModel.startsWith(`${provider}/`) ? activeModel.slice(provider.length + 1) : activeModel}`;
  }
  if (model?.includes("/")) return model;
  if (provider && model && !isDefaultTierName(model)) return `${provider}/${model}`;
  return undefined;
}

function modelForTier(
  modelConfig: TuraConfigResponse | undefined,
  tier: string,
): string | undefined {
  const current = modelConfig?.tiers?.find((item) => item.tier === tier)?.current;
  if (!current?.provider || !current.model) return undefined;
  return `${current.provider}/${current.model.startsWith(`${current.provider}/`) ? current.model.slice(current.provider.length + 1) : current.model}`;
}

function readConfigString(config: Record<string, unknown>, key: string): string | undefined {
  const value = config[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function isDefaultTierName(value: string): boolean {
  return ["thinking", "fast"].includes(value.trim().toLowerCase());
}
