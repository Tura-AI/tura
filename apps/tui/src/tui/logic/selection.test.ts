import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../reducer.js";
import { promptRuntimeSelection } from "./selection.js";

test("prompt runtime selection uses saved config before stale session runtime fields", () => {
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: {
        id: "sess-runtime",
        directory: "C:/repo",
        status: "idle",
        model: "openai/old-model",
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
    model: "codex/gpt-5.5",
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
