import { describe, expect, test } from "bun:test";
import { DEFAULT_AGENT_ID } from "../../app/src/config/defaults";

describe("GUI default agent", () => {
  test("defaults new sessions to balanced", () => {
    expect(DEFAULT_AGENT_ID).toBe("balanced");
  });
});
