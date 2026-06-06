import type {
  Agent,
  AgentConfig,
  AgentUpsertRequest,
  StoredAgent,
  TuraConfigResponse,
} from "@tura/gateway-sdk";
import Search from "lucide-solid/icons/search";
import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import { AgentIcon } from "../../components/agent-icon";
import { t, type TextKey } from "../../i18n";
import { classNames } from "../../state/format";
import { agentDisplayName, visibleConfigurableAgents } from "../../utils/agent-display";
import { AppearanceSelect } from "./appearance-select";
import { ReadonlyRow } from "./readonly-row";
import {
  AGENT_MODEL_TIERS,
  AGENT_REASONING_EFFORTS,
  modelOptionValue,
  modelTierLabel,
} from "./settings-options";

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
  const [selectedTier, setSelectedTier] =
    createSignal<(typeof AGENT_MODEL_TIERS)[number]>("thinking");
  const [selectedReasoningEffort, setSelectedReasoningEffort] =
    createSignal<(typeof AGENT_REASONING_EFFORTS)[number]>("medium");
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
      `${agentDisplayName(agent)} ${agent.description} ${agent.mode}`.toLowerCase().includes(query),
    );
  });
  const configuredAgentCount = createMemo(() => filteredAgents().length);
  const selectedCapabilities = createMemo(() =>
    capabilitiesForAgent(selectedAgent(), storedAgent()),
  );
  const modelTierOptions = createMemo(() =>
    AGENT_MODEL_TIERS.map((tier) => ({
      id: tier,
      label: modelTierLabel(tier),
      value: tier,
      detail: modelForTier(props.modelConfig, tier),
      preview: "inherit",
    })),
  );
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
    setSelectedTier(normalizeAgentModelTier(agentModelTier(agent, stored)));
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
        tier: selectedTier(),
        reasoningEffort: selectedReasoningEffort(),
        priority: priorityEnabled(),
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
                      <small>
                        {modelTierLabel(normalizeAgentModelTier(agentModelTier(agent)))}
                      </small>
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
            value={storedAgent()?.summary.description ?? selectedAgent()?.description ?? ""}
          />
          <div class="field-row">
            <label for="agent-settings-model">{t("model")}</label>
            <AppearanceSelect
              value={selectedTier()}
              options={modelTierOptions()}
              onSelect={(option) => setSelectedTier(normalizeAgentModelTier(option.value))}
            />
          </div>
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
          <div class="field-row">
            <span>{t("modelPriority")}</span>
            <div class="segmented two agent-priority-segmented">
              <button
                type="button"
                class={classNames(priorityEnabled() && "selected")}
                onClick={() => setPriorityEnabled(true)}
              >
                {t("enabled")}
              </button>
              <button
                type="button"
                class={classNames(!priorityEnabled() && "selected")}
                onClick={() => setPriorityEnabled(false)}
              >
                {t("disabled")}
              </button>
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

function agentModelTier(agent?: Agent, stored?: StoredAgent): string {
  return (
    readProviderTier(stored?.config.provider) ??
    readProviderTier(agent?.options?.provider) ??
    readProviderTier(agent?.options) ??
    "thinking"
  );
}

function normalizeAgentModelTier(value: string | undefined): (typeof AGENT_MODEL_TIERS)[number] {
  return AGENT_MODEL_TIERS.includes(value as (typeof AGENT_MODEL_TIERS)[number])
    ? (value as (typeof AGENT_MODEL_TIERS)[number])
    : "thinking";
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
    "medium"
  );
}

function normalizeReasoningEffort(
  value: string | undefined,
): (typeof AGENT_REASONING_EFFORTS)[number] {
  return value === "medium" || value === "high" || value === "xhigh" || value === "highest"
    ? value === "highest"
      ? "xhigh"
      : value
    : "medium";
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

function agentPriorityEnabled(agent?: Agent, stored?: StoredAgent): boolean {
  const configured =
    readProviderBool(stored?.config.provider, "model_acceleration_enabled") ??
    readProviderBool(agent?.options?.provider, "model_acceleration_enabled");
  if (configured !== undefined) {
    return configured;
  }
  return (
    readProviderString(stored?.config.provider, ["service_tier"]) === "priority" ||
    readProviderString(agent?.options?.provider, ["service_tier"]) === "priority"
  );
}

function readProviderTier(value: unknown): string | undefined {
  return readProviderString(value, ["tura_llm_name"]);
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

function readProviderBool(value: unknown, key: string): boolean | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const field = (value as Record<string, unknown>)[key];
  return typeof field === "boolean" ? field : undefined;
}

function agentConfigWithProviderSettings(
  config: AgentConfig,
  settings: {
    tier: (typeof AGENT_MODEL_TIERS)[number];
    reasoningEffort: (typeof AGENT_REASONING_EFFORTS)[number];
    priority: boolean;
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
      tura_llm_name: settings.tier,
      model_reasoning_effort: settings.reasoningEffort,
      model_acceleration_enabled: settings.priority,
      service_tier: settings.priority ? "priority" : "default",
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
