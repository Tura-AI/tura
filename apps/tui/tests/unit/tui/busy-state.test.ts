import assert from "node:assert/strict";
import test from "node:test";
import type { Session } from "../../../src/types/session.js";
import { hasActiveAnimation } from "../../../src/tui/busy-state.js";
import { initialState, reducer } from "../../../src/tui/reducer.js";

function sessionFixture(id: string, status: Session["status"] = "idle"): Session {
  return {
    id,
    name: id,
    directory: "C:/repo",
    status,
    created_at: 1,
    task_start_at: 1,
    updated_at: 1,
    message_count: 0,
  };
}

test("session picker keeps heartbeat active for visible busy sessions", () => {
  const active = sessionFixture("active", "idle");
  const backgroundBusy = sessionFixture("background-busy", "busy");
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: active,
      messages: [],
      permissions: [],
      sessions: [active, backgroundBusy],
    }),
    { type: "sessions", value: [active, backgroundBusy], open: true },
  );

  assert.equal(hasActiveAnimation(state), true);
});

test("closed session picker does not animate invisible background busy sessions", () => {
  const active = sessionFixture("active", "idle");
  const backgroundBusy = sessionFixture("background-busy", "busy");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: active,
    messages: [],
    permissions: [],
    sessions: [active, backgroundBusy],
  });

  assert.equal(hasActiveAnimation(state), false);
});
