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
});
