import type { Message } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import { assistantFooterMetaText } from "../../../app/src/conversation/assistant-footer-meta";

function assistantMessage(overrides: Partial<Message>): Message {
  return {
    id: "assistant-1",
    sessionID: "session-1",
    role: "assistant",
    parts: [],
    ...overrides,
  };
}

describe("assistant footer metadata text", () => {
  test("does not show provider and model names under assistant messages", () => {
    expect(
      assistantFooterMetaText(
        assistantMessage({
          providerID: "codex/gpt-5.5",
          modelID: "gpt-5.5",
        }),
      ),
    ).toBe("");
  });

  test("does not show distinct provider and model names", () => {
    expect(
      assistantFooterMetaText(
        assistantMessage({
          providerID: "openai",
          modelID: "gpt-5.5",
        }),
      ),
    ).toBe("");
  });

  test("keeps cost metadata without model text", () => {
    expect(
      assistantFooterMetaText(
        assistantMessage({
          providerID: "codex/gpt-5.5",
          modelID: "gpt-5.5",
          cost: 0.01234,
        }),
      ),
    ).toBe("$0.0123");
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
    ).toBe("medium - priority");
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
    ).toBe("high");
  });
});
