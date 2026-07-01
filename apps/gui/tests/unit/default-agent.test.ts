import { describe, expect, test } from "bun:test";
import { DEFAULT_AGENT_ID } from "../../app/src/config/defaults";

describe("GUI default agent", () => {
  test("defaults new sessions to thoughtful", () => {
    expect(DEFAULT_AGENT_ID).toBe("thoughtful");
  });
});
