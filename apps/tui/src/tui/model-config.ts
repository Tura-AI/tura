import type { SessionConfig } from "../types/config.js";

export function runtimeModelFromConfig(config: SessionConfig | undefined): string | undefined {
  const provider = stringValue(config?.active_provider);
  const activeModel = stringValue(config?.active_model);
  if (provider && activeModel) {
    return `${provider}/${stripProviderPrefix(provider, activeModel)}`;
  }

  const model = stringValue(config?.model);
  if (model?.includes("/")) return model;
  if (provider && model) return `${provider}/${stripProviderPrefix(provider, model)}`;
  return model ?? activeModel;
}

function stripProviderPrefix(provider: string, model: string): string {
  return model.startsWith(`${provider}/`) ? model.slice(provider.length + 1) : model;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
