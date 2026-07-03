import type { Agent, TuraConfigResponse } from "@tura/gateway-sdk";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import { For, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { AgentIcon } from "../components/agent-icon";
import { t } from "../i18n";
import { classNames } from "../state/format";
import type { SettingsSection } from "../state/global-store";
import { agentDisplayName, visibleConfigurableAgents } from "../utils/agent-display";
import {
  agentRuntimeConfig,
  formatAgentRuntimeModelText,
  modelPairText,
} from "../../../../tui/src/agent-runtime-config";

export function AgentComposerMenu(props: {
  agents: Agent[];
  modelConfig?: TuraConfigResponse;
  selectedAgent?: string;
  selectedModel?: string;
  onAgent: (agentId: string) => void;
  onSettings: (section: SettingsSection) => void;
}) {
  let root: HTMLElement | undefined;
  let menu: HTMLDivElement | undefined;
  const [open, setOpen] = createSignal(false);
  const [menuStyle, setMenuStyle] = createSignal<Record<string, string>>({});
  const visibleAgents = createMemo(() => visibleConfigurableAgents(props.agents));
  const selectedAgent = createMemo(
    () => visibleAgents().find((agent) => agent.name === props.selectedAgent) ?? visibleAgents()[0],
  );
  const selectedModelText = createMemo(() =>
    selectedAgent()
      ? agentModelText(selectedAgent()!, props.modelConfig, props.selectedModel)
      : props.selectedModel
        ? runtimeModelText(props.selectedModel, props.modelConfig)
        : "",
  );

  function updateMenuPosition() {
    if (!root || !menu) {
      return;
    }
    const edge = 12;
    const rootRect = root.getBoundingClientRect();
    const menuWidth = Math.min(380, Math.max(0, window.innerWidth - edge * 2));
    const preferredLeft = rootRect.left;
    const maxLeft = Math.max(edge, window.innerWidth - menuWidth - edge);
    const viewportLeft = Math.min(Math.max(preferredLeft, edge), maxLeft);
    setMenuStyle({
      left: `${viewportLeft - rootRect.left}px`,
      width: `${menuWidth}px`,
    });
  }

  createEffect(() => {
    if (!open()) {
      setMenuStyle({});
      return;
    }
    updateMenuPosition();
    const frame = window.requestAnimationFrame(updateMenuPosition);
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    window.addEventListener("resize", updateMenuPosition);
    window.addEventListener("scroll", updateMenuPosition, true);
    onCleanup(() => {
      window.cancelAnimationFrame(frame);
      document.removeEventListener("pointerdown", closeOutside);
      window.removeEventListener("resize", updateMenuPosition);
      window.removeEventListener("scroll", updateMenuPosition, true);
    });
  });

  function selectAgent(agent: Agent) {
    props.onAgent(agent.name);
    setOpen(false);
  }

  function openSettings(section: SettingsSection) {
    props.onSettings(section);
    setOpen(false);
  }

  return (
    <section class="plan-trigger-control agent-trigger-control" ref={root}>
      <button
        type="button"
        class="plan-trigger-button agent-trigger-button"
        onClick={() => setOpen(!open())}
        title={selectedModelText() || agentDisplayName(selectedAgent()) || t("agent")}
      >
        <Show when={selectedAgent()}>{(agent) => <AgentIcon agent={agent()} />}</Show>
        <span>{selectedModelText() || t("model")}</span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div
          class="plan-session-menu agent-trigger-menu"
          ref={(element) => {
            menu = element;
            window.requestAnimationFrame(updateMenuPosition);
          }}
          style={menuStyle()}
        >
          <div class="agent-trigger-list">
            <For each={visibleAgents()}>
              {(agent) => {
                const selected = createMemo(() => agent.name === selectedAgent()?.name);
                return (
                  <button
                    type="button"
                    class={classNames(
                      "workspace-pick-row",
                      "plan-trigger-option",
                      "agent-trigger-option",
                      selected() && "selected",
                    )}
                    onClick={() => selectAgent(agent)}
                  >
                    <AgentIcon agent={agent} />
                    <span>{agentDisplayName(agent)}</span>
                    <small>
                      {agentModelText(agent, props.modelConfig, props.selectedModel) || "--"}
                    </small>
                    <Show when={selected()}>
                      <Check size={14} strokeWidth={1.8} />
                    </Show>
                  </button>
                );
              }}
            </For>
          </div>
          <div class="agent-trigger-links">
            <button type="button" onClick={() => openSettings("models")}>
              <span>{t("models")}</span>
            </button>
            <button type="button" onClick={() => openSettings("agents")}>
              <span>{t("agentSettings")}</span>
            </button>
          </div>
        </div>
      </Show>
    </section>
  );
}

function agentModelText(
  agent: Agent,
  modelConfig: TuraConfigResponse | undefined,
  fallbackModel: string | undefined,
): string {
  const runtime = agentRuntimeConfig(agent);
  const model =
    modelPairText(runtime.currentModel) ??
    fallbackModel ??
    modelForTier(modelConfig, runtime.defaultModelTier);
  const displayModel = model ? runtimeModelText(model, modelConfig) : "";
  return displayModel ? formatAgentRuntimeModelText(displayModel, runtime, "p") : "";
}

function runtimeModelText(model: string, modelConfig: TuraConfigResponse | undefined): string {
  const [provider, ...modelParts] = model.split("/");
  const modelId = modelParts.join("/");
  return namedModelText(modelConfig, provider, modelId) ?? model;
}

function modelForTier(
  modelConfig: TuraConfigResponse | undefined,
  tier: string,
): string | undefined {
  const current = modelConfig?.tiers?.find((item) => item.tier === tier)?.current;
  return (
    namedModelText(modelConfig, current?.provider, current?.model) ??
    (current?.provider && current.model ? `${current.provider}/${current.model}` : undefined)
  );
}

function namedModelText(
  modelConfig: TuraConfigResponse | undefined,
  provider: string | undefined,
  model: string | undefined,
): string | undefined {
  if (!provider || !model) {
    return undefined;
  }
  const option = (modelConfig?.tiers ?? [])
    .flatMap((tier) => tier.options)
    .find((item) => item.provider === provider && item.model === model);
  const providerName = option?.provider_name || provider;
  const modelName = option?.model_name || model;
  return `${providerName}/${modelName}`;
}
