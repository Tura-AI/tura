import providerConfig from "../../../../../../crates/provider/config/provider_config.json";
import type { SdkProvider } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import { initialAppState } from "../../../../app/src/state/global-store";
import { providerStartupSettingsRedirect } from "../../../../app/src/app-state-utils";
import { mainTabEntries } from "../../../../app/src/pages/settings/main-tabs";
import { providerDomains } from "../../../../app/src/pages/settings/provider-domain";
import { settingsRoutes } from "../../../../app/src/pages/settings/settings-router";
import {
  configDraftToPatch,
  providerAuthDisplayState,
  providerConfigured,
} from "../../../../app/src/utils/settings";

const MEDIA_GENERATION_PROVIDERS = [
  "alibaba_cloud",
  "azure_cloud",
  "azure_speech",
  "codex",
  "elevenlabs",
  "google",
  "huggingface",
  "openai",
  "qwen",
  "replicate",
  "together",
  "volcengine",
  "xai",
] as const;

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

  test("classifies media generation providers by capability when no domain is listed", () => {
    expect(
      providerDomains(
        provider({
          id: "image-provider",
          options: { capabilities: ["media.generation", "image.generation"] },
        }),
      ),
    ).toEqual(["media_generation"]);
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

  test("provider catalog exposes paid media generation providers to GUI filtering", () => {
    const catalog = providerConfig.model_catalog.providers as Record<
      string,
      { capabilities?: string[]; domains?: string[] }
    >;

    expect(providerConfig.provider_enums.capabilities).toContain("media.generation");
    expect(providerConfig.provider_enums.domains).toContain("media_generation");

    for (const id of MEDIA_GENERATION_PROVIDERS) {
      expect(catalog[id], `${id} provider is registered`).toBeTruthy();
      expect(catalog[id]?.capabilities ?? [], `${id} capabilities`).toContain("media.generation");
      expect(catalog[id]?.domains ?? [], `${id} domains`).toContain("media_generation");
    }
  });
});

describe("MainTabs", () => {
  test("shows the session entry instead of the plan entry", () => {
    const entries = mainTabEntries("Session");

    expect(entries).toEqual([{ id: "conversation", label: "Session" }]);
    expect(entries.some((entry) => entry.id === "plan")).toBe(false);
  });

  test("keeps About as the final settings destination", () => {
    expect(settingsRoutes().at(-1)?.id).toBe("about");
  });
});

describe("settings config patches", () => {
  test("keeps runtime settings out of global config patches", () => {
    expect(
      configDraftToPatch(
        { language: "en", model: "openai/gpt-5.5", agent: "thinking", theme: "dark" },
        "dark",
        "8px",
      ),
    ).toEqual({
      theme: "dark",
      corner_radius: "8px",
      main_font: null,
      code_font: null,
      main_font_size: null,
      code_font_size: null,
      skill_folders: [],
    });
  });
});

describe("provider auth display state", () => {
  test("does not treat catalog connected providers as authenticated credentials", () => {
    const state = initialAppState();
    state.providers = {
      all: [],
      default: {},
      connected: ["openai"],
      enums: { domains: [], capabilities: [], api_styles: [], auth_methods: [], statuses: [] },
    };

    expect(providerAuthDisplayState(state, "openai")).toMatchObject({
      label: "Not configured",
      configured: false,
    });
    expect(providerConfigured(state, "openai")).toBe(false);
  });

  test("uses auth status as the credential source of truth", () => {
    const state = initialAppState();
    state.providerAuthStatus.openai = {
      provider_id: "openai",
      display_name: "OpenAI",
      configured: true,
      authenticated: true,
      auth_state: "authenticated",
      runtime_state: "ready",
    };

    expect(providerAuthDisplayState(state, "openai")).toMatchObject({
      label: "Connected",
      configured: true,
    });
    expect(providerConfigured(state, "openai")).toBe(true);
  });
});

describe("provider startup routing", () => {
  test("opens the provider settings page when no LLM provider is configured", () => {
    const state = initialAppState();
    state.providers = {
      all: [provider({ id: "openai", name: "OpenAI", models: llmModels() })],
      default: {},
      connected: [],
      enums: { domains: ["llm"], capabilities: [], api_styles: [], auth_methods: [], statuses: [] },
    };

    expect(providerStartupSettingsRedirect(state, false)).toEqual({
      activeTab: "settings",
      settingsSection: "providers",
      previousMainTab: "conversation",
    });
  });

  test("does not override an explicit startup tab", () => {
    const state = initialAppState();
    state.providers = {
      all: [provider({ id: "openai", name: "OpenAI", models: llmModels() })],
      default: {},
      connected: [],
      enums: { domains: ["llm"], capabilities: [], api_styles: [], auth_methods: [], statuses: [] },
    };

    expect(providerStartupSettingsRedirect(state, true)).toBeUndefined();
  });

  test("opens the provider settings page when the provider list is empty", () => {
    const state = initialAppState();
    state.providers = {
      all: [],
      default: {},
      connected: [],
      enums: { domains: [], capabilities: [], api_styles: [], auth_methods: [], statuses: [] },
    };

    expect(providerStartupSettingsRedirect(state, false)).toEqual({
      activeTab: "settings",
      settingsSection: "providers",
      previousMainTab: "conversation",
    });
  });

  test("keeps the current tab when an LLM provider is configured", () => {
    const state = initialAppState();
    state.providers = {
      all: [provider({ id: "openai", name: "OpenAI", models: llmModels() })],
      default: {},
      connected: [],
      enums: { domains: ["llm"], capabilities: [], api_styles: [], auth_methods: [], statuses: [] },
    };
    state.providerAuthStatus.openai = {
      provider_id: "openai",
      display_name: "OpenAI",
      configured: true,
      authenticated: true,
      auth_state: "authenticated",
      runtime_state: "ready",
    };

    expect(providerStartupSettingsRedirect(state, false)).toBeUndefined();
  });
});

function llmModels(): SdkProvider["models"] {
  return {
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
  };
}
