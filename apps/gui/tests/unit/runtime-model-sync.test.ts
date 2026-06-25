import { describe, expect, test } from "bun:test";
import { workspaceModelFromConfig, workspaceModelPatch } from "../../app/src/utils/runtime-model";
import { sessionConfigPatchFromAssignments } from "../../../tui/src/commands/config-values";
import { plainCapabilities } from "../../../tui/src/tui/capabilities";
import { promptRuntimeSelection } from "../../../tui/src/tui/logic/selection";
import { initialState, reducer } from "../../../tui/src/tui/reducer";
import { render } from "../../../tui/src/tui/render";
import { stripAnsi } from "../../../tui/src/tui/render-terminal";

describe("GUI/TUI runtime model config sync", () => {
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
});

function tuiPromptModel(config: Record<string, unknown>): string | undefined {
  return promptRuntimeSelection(tuiState(config)).model;
}

function tuiBottomMeta(config: Record<string, unknown>): string {
  const frame = stripAnsi(render(tuiState(config), plainCapabilities()));
  return (
    frame
      .split("\n")
      .find((line) => line.includes("/") && !line.includes("sync-session"))
      ?.trim() ?? ""
  );
}

function tuiState(config: Record<string, unknown>) {
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
    },
  );
}
