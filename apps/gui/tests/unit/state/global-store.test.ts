import { describe, expect, test } from "bun:test";
import { initialAppState, sessionTitle } from "../../../app/src/state/global-store";

describe("initialAppState", () => {
  test("defaults GUI runs to medium thinking with priority routing", () => {
    const state = initialAppState("http://127.0.0.1:4126");

    expect(state.modelVariant).toBe("medium");
    expect(state.accelerationEnabled).toBe(true);
  });
});

describe("sessionTitle", () => {
  test("does not use plan task summaries as session names", () => {
    expect(
      sessionTitle({
        id: "s1",
        name: "",
        plan_summary: "user message task text",
        status: "idle",
      }),
    ).toBe("New Session");
  });

  test("uses runtime-managed display names", () => {
    expect(
      sessionTitle({
        id: "s1",
        name: "runtime name",
        session_display_name: "runtime display name",
        plan_summary: "task text",
        status: "idle",
      }),
    ).toBe("runtime display name");
  });
});
