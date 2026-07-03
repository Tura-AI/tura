import type {
  Agent,
  AgentConfig,
  AgentUpsertRequest,
  StoredAgent,
  TuraConfigModelPair,
  TuraConfigResponse,
} from "@tura/gateway-sdk";
import Search from "lucide-solid/icons/search";
import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import { AgentIcon } from "../../components/agent-icon";
import { t, type TextKey } from "../../i18n";
import { classNames } from "../../state/format";
import {
  agentDescription,
  agentDisplayName,
  visibleConfigurableAgents,
} from "../../utils/agent-display";
import { AppearanceSelect } from "./appearance-select";
import { ReadonlyRow } from "./readonly-row";
import {
  AGENT_REASONING_EFFORTS,
  canonicalDefaultModelTier,
  modelOptionValue,
  modelTierLabel,
} from "./settings-options";
import type { DEFAULT_MODEL_TIERS } from "./settings-options";

export function AgentSettingsPanel(props: {
  agents: Agent[];
  saving: boolean;
  modelConfig?: TuraConfigResponse;
  onRefresh: () => Promise<void>;
  onGetAgent: (agentId: string) => Promise<StoredAgent | undefined>;
  onSaveAgent: (agentId: string | undefined, payload: AgentUpsertRequest) => Promise<void>;
}) {
  const [selectedAgentId, setSelectedAgentId] = createSignal<string>();
  const [storedAgent, setStoredAgent] = createSignal<StoredAgent>();
  const [selectedProvider, setSelectedProvider] = createSignal("");
  const [selectedModel, setSelectedModel] = createSignal("");
  const [selectedReasoningEffort, setSelectedReasoningEffort] =
    createSignal<(typeof AGENT_REASONING_EFFORTS)[number]>("high");
  const [priorityEnabled, setPriorityEnabled] = createSignal(false);
  const [loadingAgent, setLoadingAgent] = createSignal(false);
  const [agentQuery, setAgentQuery] = createSignal("");
  const visibleAgents = createMemo(() => visibleConfigurableAgents(props.agents));
  const selectedAgent = createMemo(() =>
    visibleAgents().find((agent) => agent.name === selectedAgentId()),
  );
  const filteredAgents = createMemo(() => {
    const query = agentQuery().trim().toLowerCase();
    if (!query) {
      return visibleAgents();
    }
    return visibleAgents().filter((agent) =>
      `${agentDisplayName(agent)} ${agentDescription(agent)} ${agent.mode}`
        .toLowerCase()
        .includes(query),
    );
  });
  const configuredAgentCount = createMemo(() => filteredAgents().length);
  const selectedCapabilities = createMemo(() =>
    capabilitiesForAgent(selectedAgent(), storedAgent()),
  );
  const selectedDefaultModelTier = createMemo(() =>
    normalizeDefaultModelTier(agentDefaultModelTier(selectedAgent(), storedAgent())),
  );
  const providerOptions = createMemo(() =>
    modelProviderOptions(
      props.modelConfig,
      currentAgentModelOption(selectedAgent(), storedAgent()),
    ),
  );
  const modelOptions = createMemo(() =>
    modelOptionsForProvider(
      props.modelConfig,
      selectedProvider(),
      currentAgentModelOption(selectedAgent(), storedAgent()),
    ),
  );
  const selectedModelValue = createMemo(() =>
    selectedProvider() && selectedModel() ? `${selectedProvider()}/${selectedModel()}` : "",
  );
  const currentModelLabel = createMemo(() => {
    const value = selectedModelValue();
    return value || modelForTier(props.modelConfig, selectedDefaultModelTier()) || "--";
  });
  const reasoningEffortOptions = createMemo(() =>
    AGENT_REASONING_EFFORTS.map((effort) => ({
      id: effort,
      label: reasoningEffortLabel(effort),
      value: effort,
      preview: "inherit",
    })),
  );

  createEffect(() => {
    if (!selectedAgentId() && filteredAgents().length > 0) {
      void selectAgent(filteredAgents()[0]!);
    }
  });

  async function selectAgent(agent: Agent) {
    setSelectedAgentId(agent.name);
    setLoadingAgent(true);
    const stored = await props.onGetAgent(agent.name);
    setStoredAgent(stored);
    const currentModel = agentCurrentModel(agent, stored);
    setSelectedProvider(currentModel?.provider ?? "");
    setSelectedModel(currentModel?.model ?? "");
    setSelectedReasoningEffort(normalizeReasoningEffort(agentReasoningEffort(agent, stored)));
    setPriorityEnabled(agentPriorityEnabled(agent, stored));
    setLoadingAgent(false);
  }

  async function saveAgentSettings() {
    const agent = selectedAgent();
    const stored = storedAgent();
    if (!agent || !stored) {
      return;
    }
    const payload: AgentUpsertRequest = {
      config: agentConfigWithProviderSettings(stored.config, {
        defaultModelTier: selectedDefaultModelTier(),
        currentModel: selectedModelValue(),
        reasoningEffort: selectedReasoningEffort(),
        priorityEnabled: priorityEnabled(),
      }),
      prompt: stored.prompt ?? undefined,
    };
    await props.onSaveAgent(agent.name, payload);
    await props.onRefresh();
    await selectAgent(agent);
  }

  return (
    <section class="settings-panel agent-settings-panel">
      <header>
        <span>{t("agentSettings")}</span>
        <small>{visibleAgents().length}</small>
      </header>
      <div class="agent-settings-layout">
        <div class="settings-list agent-list">
          <label class="workspace-search-row provider-search-row agent-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={agentQuery()}
              placeholder={`${t("search")}...`}
              onInput={(event) => setAgentQuery(event.currentTarget.value)}
            />
          </label>
          <div class="settings-list provider-config-list agent-config-list">
            <div class="provider-config-group agent-configured-group">
              <div class="provider-config-group-title">
                <span>默认智能体</span>
                <small>{configuredAgentCount()}</small>
              </div>
              <div class="workspace-picker-list agent-list-scroll">
                <For each={filteredAgents()}>
                  {(agent) => (
                    <button
                      type="button"
                      class={classNames(
                        "workspace-pick-row",
                        "agent-pick-row",
                        selectedAgentId() === agent.name && "selected",
                      )}
                      onClick={() => void selectAgent(agent)}
                    >
                      <AgentIcon agent={agent} />
                      <span>{agentDisplayName(agent)}</span>
                      <small>{agentModelDisplayText(agent, props.modelConfig)}</small>
                    </button>
                  )}
                </For>
              </div>
            </div>
          </div>
        </div>
        <div class="settings-fields agent-editor">
          <Show when={loadingAgent()}>
            <div class="settings-inline-loading" aria-label={t("loading")}>
              <div class="loading-bar wide" />
              <div class="loading-bar medium" />
            </div>
          </Show>
          <ReadonlyRow
            label={t("agentName")}
            value={agentDisplayName(selectedAgent(), storedAgent())}
          />
          <ReadonlyRow
            label={t("description")}
            value={agentDescription(selectedAgent(), storedAgent())}
          />
          <ReadonlyRow
            label={t("defaultModelTier")}
            value={[
              modelTierLabel(selectedDefaultModelTier()),
              modelForTier(props.modelConfig, selectedDefaultModelTier()),
            ]
              .filter((value) => value && value !== "--")
              .join(" · ")}
          />
          <div class="field-row">
            <label>{t("provider")}</label>
            <AppearanceSelect
              value={selectedProvider()}
              options={providerOptions()}
              placeholder={t("provider")}
              onSelect={(option) => {
                setSelectedProvider(option.value);
                const firstModel = modelOptionsForProvider(
                  props.modelConfig,
                  option.value,
                  currentAgentModelOption(selectedAgent(), storedAgent()),
                )[0];
                setSelectedModel(providerModelPair(firstModel?.value)?.model ?? "");
              }}
            />
          </div>
          <Show when={selectedProvider()}>
            <div class="field-row">
              <label>{t("currentModel")}</label>
              <AppearanceSelect
                value={selectedModelValue()}
                options={modelOptions()}
                placeholder={currentModelLabel()}
                onSelect={(option) => {
                  const model = providerModelPair(option.value)?.model ?? "";
                  setSelectedModel(model);
                }}
              />
            </div>
          </Show>
          <div class="field-row">
            <label for="agent-settings-reasoning">{t("modelReasoningEffort")}</label>
            <AppearanceSelect
              value={selectedReasoningEffort()}
              options={reasoningEffortOptions()}
              onSelect={(option) =>
                setSelectedReasoningEffort(normalizeReasoningEffort(option.value))
              }
            />
          </div>
          <div class="field-row agent-priority-row">
            <span>{t("modelPriority")}</span>
            <div class="settings-toggle-field">
              <label class="settings-toggle-row">
                <input
                  type="checkbox"
                  checked={priorityEnabled()}
                  onChange={(event) => setPriorityEnabled(event.currentTarget.checked)}
                />
                <span>{t("acceleration")}</span>
              </label>
              <small class="settings-note">{t("accelerationHint")}</small>
            </div>
          </div>
          <div class="field-row agent-capabilities-row">
            <span>{t("capabilities")}</span>
            <div class="agent-capability-list">
              <Show
                when={selectedCapabilities().length > 0}
                fallback={<span class="settings-note">暂无能力</span>}
              >
                <For each={selectedCapabilities()}>{(capability) => <code>{capability}</code>}</For>
              </Show>
            </div>
          </div>
          <div class="settings-actions-row agent-actions-row">
            <button
              type="button"
              class="primary"
              disabled={!selectedAgent() || !storedAgent() || props.saving}
              aria-busy={props.saving}
              onClick={() => void saveAgentSettings()}
            >
              <Show
                when={!props.saving}
                fallback={<span class="button-loading-bar loading-bar short" />}
              >
                {t("save")}
              </Show>
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}

function agentDefaultModelTier(agent?: Agent, stored?: StoredAgent): string {
  return (
    readProviderString(stored?.config.provider, ["default_model_tier"]) ??
    readLegacyDefaultTier(stored?.config.provider) ??
    readProviderString(agent?.options?.provider, ["default_model_tier"]) ??
    readLegacyDefaultTier(agent?.options?.provider) ??
    readProviderString(agent?.options, ["default_model_tier"]) ??
    readLegacyDefaultTier(agent?.options) ??
    "thinking"
  );
}

function normalizeDefaultModelTier(
  value: string | undefined,
): (typeof DEFAULT_MODEL_TIERS)[number] {
  return canonicalDefaultModelTier(value);
}

function readLegacyDefaultTier(value: unknown): string | undefined {
  const legacy = readProviderString(value, ["tura_llm_name"]);
  return legacy?.includes("/") ? undefined : legacy;
}

function agentCurrentModel(
  agent?: Agent,
  stored?: StoredAgent,
): Pick<TuraConfigModelPair, "provider" | "model"> | undefined {
  return (
    providerModelPair(readProviderString(stored?.config.provider, ["current_model"])) ??
    providerModelPair(readProviderString(agent?.options?.provider, ["current_model"])) ??
    providerModelPair(readProviderString(agent?.options, ["current_model"])) ??
    providerModelPair(readProviderString(stored?.config.provider, ["tura_llm_name"])) ??
    providerModelPair(readProviderString(agent?.options?.provider, ["tura_llm_name"])) ??
    (agent?.model?.providerID && agent.model.modelID
      ? { provider: agent.model.providerID, model: agent.model.modelID }
      : undefined)
  );
}

function currentAgentModelOption(
  agent?: Agent,
  stored?: StoredAgent,
): TuraConfigModelPair | undefined {
  const current = agentCurrentModel(agent, stored);
  return current
    ? {
        ...current,
        provider_name: current.provider,
        model_name: current.model,
      }
    : undefined;
}

function providerModelPair(
  value: string | undefined,
): Pick<TuraConfigModelPair, "provider" | "model"> | undefined {
  if (!value?.includes("/")) {
    return undefined;
  }
  const [provider, ...modelParts] = value.split("/");
  const model = modelParts.join("/");
  return provider && model ? { provider, model } : undefined;
}

function agentReasoningEffort(agent?: Agent, stored?: StoredAgent): string {
  return (
    readProviderString(stored?.config.provider, [
      "model_reasoning_effort",
      "reasoning_effort",
      "model_variant",
    ]) ??
    readProviderString(agent?.options?.provider, [
      "model_reasoning_effort",
      "reasoning_effort",
      "model_variant",
    ]) ??
    "high"
  );
}

function agentPriorityEnabled(agent?: Agent, stored?: StoredAgent): boolean {
  return (
    readProviderBoolean(stored?.config.provider, ["model_acceleration_enabled"]) ??
    readProviderBoolean(agent?.options?.provider, ["model_acceleration_enabled"]) ??
    readProviderBoolean(agent?.options, ["model_acceleration_enabled"]) ??
    readServiceTierPriority(stored?.config.provider) ??
    readServiceTierPriority(agent?.options?.provider) ??
    readServiceTierPriority(agent?.options) ??
    false
  );
}

function normalizeReasoningEffort(
  value: string | undefined,
): (typeof AGENT_REASONING_EFFORTS)[number] {
  return value === "medium" || value === "high" || value === "xhigh" || value === "highest"
    ? value === "highest"
      ? "xhigh"
      : value
    : "high";
}

function reasoningEffortLabel(value: string): string {
  const labels: Record<string, TextKey> = {
    high: "modelReasoningEffortHigh",
    low: "modelReasoningEffortLow",
    medium: "modelReasoningEffortMedium",
    xhigh: "modelReasoningEffortXHigh",
  };
  return labels[value] ? t(labels[value]) : value;
}

function readProviderString(value: unknown, keys: string[]): string | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const record = value as Record<string, unknown>;
  for (const key of keys) {
    const field = record[key];
    if (typeof field === "string" && field.trim()) {
      return field.trim();
    }
  }
  return undefined;
}

function readProviderBoolean(value: unknown, keys: string[]): boolean | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const record = value as Record<string, unknown>;
  for (const key of keys) {
    const field = record[key];
    if (typeof field === "boolean") {
      return field;
    }
  }
  return undefined;
}

function readServiceTierPriority(value: unknown): boolean | undefined {
  const serviceTier = readProviderString(value, ["service_tier"]);
  if (!serviceTier) {
    return undefined;
  }
  return serviceTier === "priority";
}

function agentConfigWithProviderSettings(
  config: AgentConfig,
  settings: {
    defaultModelTier: (typeof DEFAULT_MODEL_TIERS)[number];
    currentModel: string;
    reasoningEffort: (typeof AGENT_REASONING_EFFORTS)[number];
    priorityEnabled: boolean;
  },
): AgentConfig {
  const provider =
    config.provider && typeof config.provider === "object" && !Array.isArray(config.provider)
      ? { ...(config.provider as Record<string, unknown>) }
      : {};
  return {
    ...config,
    provider: {
      ...provider,
      default_model_tier: settings.defaultModelTier,
      tura_llm_name: settings.defaultModelTier,
      ...(settings.currentModel ? { current_model: settings.currentModel } : {}),
      model_reasoning_effort: settings.reasoningEffort,
      model_acceleration_enabled: settings.priorityEnabled,
      service_tier: settings.priorityEnabled ? "priority" : "auto",
    },
  };
}

function capabilitiesForAgent(agent?: Agent, stored?: StoredAgent): string[] {
  const values = [
    ...(stored?.summary.capabilities ?? []),
    ...readStringList(stored?.config.agent_capabilities),
    ...(Array.isArray(agent?.options?.capabilities)
      ? (agent!.options.capabilities as unknown[])
          .map((item) => (typeof item === "string" ? item : undefined))
          .filter((item): item is string => !!item)
      : []),
  ];
  return [...new Set(values)].sort();
}

function modelForTier(modelConfig: TuraConfigResponse | undefined, tier: string): string {
  const current = modelConfig?.tiers.find((item) => item.tier === tier)?.current;
  return current ? modelOptionValue(current) : "--";
}

function modelProviderOptions(
  modelConfig: TuraConfigResponse | undefined,
  current?: TuraConfigModelPair,
) {
  const providers = new Map<string, string>();
  for (const option of allModelOptions(modelConfig, current)) {
    providers.set(option.provider, option.provider_name || option.provider);
  }
  return [...providers.entries()].map(([provider, label]) => ({
    id: provider,
    label,
    value: provider,
    preview: "inherit",
  }));
}

function modelOptionsForProvider(
  modelConfig: TuraConfigResponse | undefined,
  provider: string,
  current?: TuraConfigModelPair,
) {
  if (!provider) {
    return [];
  }
  const seen = new Set<string>();
  return allModelOptions(modelConfig, current)
    .filter((option) => option.provider === provider)
    .filter((option) => {
      const value = modelOptionValue(option);
      if (seen.has(value)) {
        return false;
      }
      seen.add(value);
      return true;
    })
    .map((option) => ({
      id: modelOptionValue(option),
      label: option.model_name || option.model,
      value: modelOptionValue(option),
      model: option.model,
      detail: option.provider_name || option.provider,
      preview: "inherit",
    }));
}

function allModelOptions(
  modelConfig: TuraConfigResponse | undefined,
  current?: TuraConfigModelPair,
): TuraConfigModelPair[] {
  const options = modelConfig?.tiers.flatMap((tier) => tier.options) ?? [];
  return current ? [current, ...options] : options;
}

function agentModelDisplayText(agent: Agent, modelConfig: TuraConfigResponse | undefined): string {
  const current = agentCurrentModel(agent);
  if (current) {
    return modelOptionValue(current);
  }
  const tier = normalizeDefaultModelTier(agentDefaultModelTier(agent));
  return modelForTier(modelConfig, tier);
}

function readStringList(value: unknown): string[] {
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
