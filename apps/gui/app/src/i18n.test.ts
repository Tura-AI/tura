import { describe, expect, test } from "bun:test";
import { assertDictionaryParity, t } from "./i18n";

describe("i18n", () => {
  test("keeps zh-CN and en dictionaries in sync", () => {
    expect(() => assertDictionaryParity()).not.toThrow();
  });

  test("formats startup failures through the dictionary", () => {
    expect(t("startupFailed", { message: "boom" })).toContain("boom");
  });
});
