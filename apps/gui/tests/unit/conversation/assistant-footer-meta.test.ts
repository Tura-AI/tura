import type { Message } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  assistantFooterMetaText,
  assistantFooterModelText,
} from "../../../app/src/conversation/assistant-footer-meta";

function assistantMessage(overrides: Partial<Message>): Message {
  return {
    id: "assistant-1",
    sessionID: "session-1",
    role: "assistant",
    parts: [],
    ...overrides,
  };
}

describe("assistant footer model text", () => {
  test("does not repeat the model when provider already contains the model suffix", () => {
    expect(
      assistantFooterModelText(
        assistantMessage({
          providerID: "codex/gpt-5.5",
          modelID: "gpt-5.5",
        }),
      ),
    ).toBe("codex/gpt-5.5");
  });

  test("keeps distinct provider and model names separated by one slash", () => {
    expect(
      assistantFooterModelText(
        assistantMessage({
          providerID: "openai",
          modelID: "gpt-5.5",
        }),
      ),
    ).toBe("openai/gpt-5.5");
  });

  test("keeps the footer metadata compact after model dedupe", () => {
    expect(
      assistantFooterMetaText(
        assistantMessage({
          providerID: "codex/gpt-5.5",
          modelID: "gpt-5.5",
          cost: 0.01234,
        }),
      ),
    ).toBe("codex/gpt-5.5 · $0.0123");
  });

  test("shows runtime reasoning and priority from assistant message metadata", () => {
    expect(
      assistantFooterMetaText(
        assistantMessage({
          providerID: "codex",
          modelID: "gpt-5.5",
          metadata: {
            runtime: {
              reasoning_level: "medium",
              model_acceleration_enabled: true,
            },
          },
        }),
      ),
    ).toBe("codex/gpt-5.5 - medium - priority");
  });

  test("omits priority when runtime metadata did not enable it", () => {
    expect(
      assistantFooterMetaText(
        assistantMessage({
          providerID: "codex",
          modelID: "gpt-5.5",
          metadata: {
            runtime: {
              reasoning_level: "high",
              model_acceleration_enabled: false,
            },
          },
        }),
      ),
    ).toBe("codex/gpt-5.5 - high");
  });
});
