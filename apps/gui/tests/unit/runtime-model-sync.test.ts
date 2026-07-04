import { describe, expect, test } from "bun:test";
import { workspaceModelFromConfig, workspaceModelPatch } from "../../app/src/utils/runtime-model";
import { sessionConfigPatchFromAssignments } from "../../../tui/src/commands/config-values";
import { plainCapabilities } from "../../../tui/src/tui/capabilities";
import { promptRuntimeSelection } from "../../../tui/src/tui/logic/selection";
import { initialState, reducer } from "../../../tui/src/tui/reducer";
import { render } from "../../../tui/src/tui/render";
import { stripAnsi } from "../../../tui/src/tui/render-terminal";
import { agentRuntimeConfig, applyAgentRuntimeConfig } from "../../../tui/src/agent-runtime-config";

describe("GUI/TUI runtime model config sync", () => {
  test("tier names resolve to the configured model instead of rendering thinking", () => {
    const gatewayConfig = { model: "thinking" };
    const modelConfig = tierModelConfig("thinking", "codex", "gpt-5.5");

    expect(workspaceModelFromConfig(gatewayConfig, modelConfig)).toBe("codex/gpt-5.5");
    expect(tuiPromptModel(gatewayConfig, modelConfig)).toBe("codex/gpt-5.5");
    expect(tuiBottomMeta(gatewayConfig, modelConfig)).toContain("codex/gpt-5.5");
    expect(tuiBottomMeta(gatewayConfig, modelConfig)).not.toContain("thinking");
  });

  test("bare tier names are not displayed as runtime models when tier config is missing", () => {
    const gatewayConfig = { model: "thinking", active_provider: "codex" };

    expect(workspaceModelFromConfig(gatewayConfig)).toBeUndefined();
    expect(tuiPromptModel(gatewayConfig)).toBe("codex/gpt-5.5");
    expect(tuiBottomMeta(gatewayConfig)).toContain("codex/gpt-5.5");
    expect(tuiBottomMeta(gatewayConfig)).not.toContain("thinking");
  });

  test("GUI model changes are visible to TUI and use the same prompt model", () => {
    const guiSelectedModel = "openrouter/qwen/qwen3.7-max";
    const gatewayConfig = workspaceModelPatch(guiSelectedModel);

    expect(workspaceModelFromConfig(gatewayConfig)).toBe(guiSelectedModel);
    expect(tuiPromptModel(gatewayConfig)).toBe(guiSelectedModel);
    expect(tuiBottomMeta(gatewayConfig)).toContain(guiSelectedModel);
    expect(tuiBottomMeta(gatewayConfig)).not.toContain("codex/gpt-5.5");
  });

  test("active provider/model pair wins over stale model field everywhere", () => {
    const gatewayConfig = {
      model: "codex/gpt-5.5",
      active_provider: "openrouter",
      active_model: "qwen/qwen3.7-max",
    };

    expect(workspaceModelFromConfig(gatewayConfig)).toBe("openrouter/qwen/qwen3.7-max");
    expect(tuiPromptModel(gatewayConfig)).toBe("openrouter/qwen/qwen3.7-max");
    expect(tuiBottomMeta(gatewayConfig)).toContain("openrouter/qwen/qwen3.7-max");
    expect(tuiBottomMeta(gatewayConfig)).not.toContain("codex/gpt-5.5");
    expect(tuiBottomMeta(gatewayConfig)).not.toContain("openrouter/qwen/qwen3.7-max/gpt-5.5");
  });

  test("TUI model changes are visible to GUI and use the same prompt model", () => {
    const tuiSelectedModel = "anthropic/claude-opus-4.5";
    const gatewayConfig = sessionConfigPatchFromAssignments([`model=${tuiSelectedModel}`]);

    expect(workspaceModelFromConfig(gatewayConfig)).toBe(tuiSelectedModel);
    expect(tuiPromptModel(gatewayConfig)).toBe(tuiSelectedModel);
    expect(tuiBottomMeta(gatewayConfig)).toContain(tuiSelectedModel);
    expect(tuiBottomMeta(gatewayConfig)).not.toContain("codex/gpt-5.5");
  });

  test("GUI and TUI agent settings share one persisted provider contract", () => {
    const config = applyAgentRuntimeConfig(
      {
        agent_name: "balanced",
        provider: {
          current_model: "legacy/stale",
          default_model_tier: "direct",
          tura_llm_name: "direct",
          model_reasoning_effort: "low",
          model_acceleration_enabled: true,
          service_tier: "priority",
        },
      },
      {
        defaultModelTier: "thinking",
        currentModel: "codex/gpt-5.5",
        reasoningLevel: "medium",
        priorityEnabled: false,
      },
    );

    expect(config.provider).toEqual({
      current_model: "codex/gpt-5.5",
      default_model_tier: "thinking",
      tura_llm_name: "thinking",
      model_reasoning_effort: "medium",
    });
    expect(agentRuntimeConfig(undefined, { config })).toEqual({
      defaultModelTier: "thinking",
      currentModel: { provider: "codex", model: "gpt-5.5" },
      reasoningLevel: "medium",
      priorityEnabled: false,
    });
  });
});

function tuiPromptModel(
  config: Record<string, unknown>,
  modelConfig?: Parameters<typeof tuiState>[1],
): string | undefined {
  return promptRuntimeSelection(tuiState(config, modelConfig)).model;
}

function tuiBottomMeta(
  config: Record<string, unknown>,
  modelConfig?: Parameters<typeof tuiState>[1],
): string {
  const frame = stripAnsi(render(tuiState(config, modelConfig), plainCapabilities()));
  return (
    frame
      .split("\n")
      .find((line) => line.includes("/") && !line.includes("sync-session"))
      ?.trim() ?? ""
  );
}

function tuiState(
  config: Record<string, unknown>,
  modelConfig?: {
    path: string;
    tiers: Array<{
      tier: string;
      current?: { provider: string; model: string } | null;
      options: Array<{ provider: string; model: string }>;
    }>;
  },
) {
  return reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: {
        id: "sync-session",
        directory: "C:/repo",
        status: "idle",
        model: "codex/gpt-5.5",
        agent: "stale-agent",
        model_variant: "low",
        model_acceleration_enabled: false,
      },
      messages: [],
      permissions: [],
    }),
    {
      type: "session-config",
      value: config,
      modelConfig,
    },
  );
}

function tierModelConfig(tier: string, provider: string, model: string) {
  return {
    path: "C:/repo/.tura/config.conf",
    tiers: [
      {
        tier,
        current: { provider, model },
        options: [{ provider, model }],
      },
    ],
  };
}
