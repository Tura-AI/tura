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
    expect(t("home")).toBe("首页");
    expect(t("modelPriority")).toBe("优先");
    expect(t("writeMessage")).toBe("输入消息...");
    expect(t("runtimeStopped")).toBe("Runtime 已停止。");
    setLanguage("en");
    expect(currentLanguage()).toBe("en");
    expect(t("settings")).toBe("Settings");
    expect(t("runtimeStopped")).toBe("Runtime stopped.");
  });

  test("parses the same language aliases as the TUI", () => {
    expect(parseLanguage("cn")).toBe("zh-CN");
    expect(parseLanguage("en-US")).toBe("en");
    expect(parseLanguage("fr")).toBeUndefined();
  });

  test("uses English as the GUI default even in a Chinese browser locale", async () => {
    const navigatorDescriptor = Object.getOwnPropertyDescriptor(globalThis, "navigator");
    try {
      Object.defineProperty(globalThis, "navigator", {
        value: { language: "zh-CN", languages: ["zh-CN"] },
        configurable: true,
      });
      const module = await import(`../../app/src/i18n.ts?default-locale=${Date.now()}`);
      expect(module.currentLanguage()).toBe("en");
      expect(module.t("settings")).toBe("Settings");
    } finally {
      if (navigatorDescriptor) {
        Object.defineProperty(globalThis, "navigator", navigatorDescriptor);
      } else {
        delete (globalThis as { navigator?: unknown }).navigator;
      }
    }
  });

  test("resets to the GUI default when language is unset", () => {
    setLanguage("zh-CN");
    setLanguage(undefined);
    expect(currentLanguage()).toBe("en");
  });
});
