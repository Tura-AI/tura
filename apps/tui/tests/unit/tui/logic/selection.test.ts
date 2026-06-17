import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../../src/tui/reducer.js";
import {
  promptRuntimeSelection,
  selectedSettingDetail,
  settingPatch,
} from "../../../../src/tui/logic/selection.js";

test("prompt runtime selection uses the active session model before saved config", () => {
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: {
        id: "sess-runtime",
        directory: "C:/repo",
        status: "idle",
        model: "codex/gpt-5.3-codex-spark",
        agent: "fast",
        model_variant: "low",
        model_acceleration_enabled: false,
      },
      messages: [],
      permissions: [],
    }),
    {
      type: "session-config",
      value: {
        model: "flagship_thinking",
        active_provider: "codex",
        active_model: "gpt-5.5",
        active_agent: "thinking",
        model_variant: "high",
        model_acceleration_enabled: true,
      },
    },
  );

  assert.deepEqual(promptRuntimeSelection(state), {
    model: "codex/gpt-5.3-codex-spark",
    agent: "thinking",
    modelVariant: "high",
    modelAccelerationEnabled: true,
  });
});

test("prompt runtime selection enables priority by default", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: {
      id: "draft-runtime",
      draft: true,
      directory: "C:/repo",
      status: "idle",
    },
    messages: [],
    permissions: [],
  });

  assert.equal(promptRuntimeSelection(state).modelAccelerationEnabled, true);
});

test("prompt runtime selection preserves explicit priority off", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: {
      id: "sess-runtime-off",
      directory: "C:/repo",
      status: "idle",
      model_acceleration_enabled: false,
    },
    messages: [],
    permissions: [],
  });

  assert.equal(promptRuntimeSelection(state).modelAccelerationEnabled, false);
});

test("settings selection follows the rendered settings order", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "session-config",
    value: {
      model: "openai/gpt-5",
      active_provider: "openai",
      active_agent: "thinking",
      language: "en",
      session_type: "coding",
      model_variant: "high",
      model_acceleration_enabled: true,
      show_command_instructions: true,
      validator_enabled: false,
      command_run_stall_guard_profile: "balanced_20s",
    },
  });

  const details = Array.from({ length: 8 }, (_item, index) =>
    selectedSettingDetail({ ...state, selectedSettingsIndex: index }),
  );

  assert.deepEqual(details, [
    "model",
    "provider",
    "agent",
    "persona",
    "language",
    "variant",
    "priority",
    "commands",
  ]);
});

test("settings patches cover language, session type, validator, and stall guard", () => {
  assert.deepEqual(settingPatch("language", "zh-CN"), { language: "zh-CN" });
  assert.deepEqual(settingPatch("commands", false), { show_command_instructions: false });
  assert.deepEqual(settingPatch("session", "business"), { session_type: "business" });
  assert.deepEqual(settingPatch("validator", true), { validator_enabled: true });
  assert.deepEqual(settingPatch("stallGuard", "long_io_60s"), {
    command_run_stall_guard_profile: "long_io_60s",
  });
});
