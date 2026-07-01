import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { SdkProvider } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import { mainTabEntries } from "../../../../app/src/pages/settings/main-tabs";
import { providerDomains } from "../../../../app/src/pages/settings/provider-domain";
import { configDraftToPatch } from "../../../../app/src/utils/settings";

const settingsViewSource = readFileSync(
  resolve(import.meta.dir, "../../../../app/src/pages/settings/settings-view.tsx"),
  "utf8",
);
const navigationCss = readFileSync(
  resolve(import.meta.dir, "../../../../app/src/styles/parts/base/navigation.css"),
  "utf8",
);

function provider(overrides: Partial<SdkProvider>): SdkProvider {
  return {
    id: "test",
    name: "Test",
    source: "test",
    env: [],
    options: {},
    models: {},
    ...overrides,
  };
}

describe("providerDomains", () => {
  test("reads non-LLM catalog domains from provider options", () => {
    expect(
      providerDomains(
        provider({
          id: "feishu",
          options: { domains: ["communication", "productivity"] },
        }),
      ),
    ).toEqual(["communication", "productivity"]);
  });

  test("keeps legacy model providers visible under LLM", () => {
    expect(
      providerDomains(
        provider({
          id: "legacy-openai",
          models: {
            "gpt-5.5": {
              id: "gpt-5.5",
              name: "GPT-5.5",
              family: "gpt",
              release_date: "2026-05-01",
              attachment: true,
              reasoning: true,
              temperature: true,
              tool_call: true,
              limit: { context: 1, input: 1, output: 1 },
              modalities: { input: ["text"], output: ["text"] },
              options: {},
            },
          },
        }),
      ),
    ).toEqual(["llm"]);
  });

  test("keeps service providers without models visible", () => {
    expect(
      providerDomains(
        provider({
          id: "service-only",
          options: { capabilities: ["calendar.events"] },
        }),
      ),
    ).toEqual(["other"]);
  });
});

describe("MainTabs", () => {
  test("shows the session entry instead of the plan entry", () => {
    const entries = mainTabEntries("Session");

    expect(entries).toEqual([{ id: "conversation", label: "Session" }]);
    expect(entries.some((entry) => entry.id === "plan")).toBe(false);
  });

  test("uses the no-icon grid so the session label is not clipped into the icon column", () => {
    expect(navigationCss).toContain(".main-tabs button.no-icon");
    expect(settingsViewSource).toContain(
      'classNames("no-icon", props.active === item.id && "selected")',
    );
  });
});

describe("settings config patches", () => {
  test("keeps runtime settings out of global config patches", () => {
    expect(
      configDraftToPatch(
        { language: "en", model: "openai/gpt-5.5", agent: "thinking", theme: "dark" },
        "dark",
      ),
    ).toEqual({
      theme: "dark",
      main_font: null,
      code_font: null,
      main_font_size: null,
      code_font_size: null,
      skill_folders: [],
    });
  });
});
