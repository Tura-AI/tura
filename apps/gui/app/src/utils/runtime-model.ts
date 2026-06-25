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

export function workspaceModelFromConfig(config: Record<string, unknown>): string | undefined {
  const provider = readConfigString(config, "active_provider");
  const activeModel = readConfigString(config, "active_model");
  if (provider && activeModel) {
    return `${provider}/${activeModel.startsWith(`${provider}/`) ? activeModel.slice(provider.length + 1) : activeModel}`;
  }
  const model = readConfigString(config, "model");
  if (model?.includes("/")) return model;
  if (provider && model) return `${provider}/${model}`;
  return model;
}

function readConfigString(config: Record<string, unknown>, key: string): string | undefined {
  const value = config[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
