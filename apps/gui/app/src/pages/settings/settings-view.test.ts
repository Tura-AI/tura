import { describe, expect, test } from "bun:test";
import type { SdkProvider } from "@tura/gateway-sdk";
import { providerDomains } from "./provider-domain";

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
