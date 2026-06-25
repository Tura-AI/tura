import { describe, expect, test } from "bun:test";
import {
  assertDictionaryParity,
  currentLanguage,
  parseLanguage,
  setLanguage,
  t,
} from "../../app/src/i18n";

describe("i18n", () => {
  test("keeps zh-CN and en dictionaries in sync", () => {
    expect(() => assertDictionaryParity()).not.toThrow();
  });

  test("formats startup failures through the dictionary", () => {
    expect(t("startupFailed", { message: "boom" })).toContain("boom");
  });

  test("switches GUI language at runtime", () => {
    setLanguage("zh-CN");
    expect(t("settings")).toBe("设置");
    setLanguage("en");
    expect(currentLanguage()).toBe("en");
    expect(t("settings")).toBe("Settings");
  });

  test("parses the same language aliases as the TUI", () => {
    expect(parseLanguage("cn")).toBe("zh-CN");
    expect(parseLanguage("en-US")).toBe("en");
    expect(parseLanguage("fr")).toBeUndefined();
  });

  test("resets to the GUI default when language is unset", () => {
    setLanguage("zh-CN");
    setLanguage(undefined);
    const browserLanguage =
      typeof navigator !== "undefined" ? parseLanguage(navigator.language) : undefined;
    expect(currentLanguage()).toBe(browserLanguage ?? "en");
  });
});
