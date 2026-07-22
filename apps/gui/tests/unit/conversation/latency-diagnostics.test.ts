import { describe, expect, test } from "bun:test";
import { turnLatencyDiagnostics } from "../../../app/src/conversation/latency-diagnostics";

describe("turn latency diagnostics", () => {
  test("combines persisted provider, tool, and session timings", () => {
    const diagnostics = turnLatencyDiagnostics(
      [
        { id: "u", sessionID: "s", role: "user", created_at: 1_700_000_000_000, parts: [] },
        {
          id: "a",
          sessionID: "s",
          role: "assistant",
          created_at: 1_700_000_000_300,
          updated_at: 1_700_000_000_800,
          metadata: {
            usage: { latency_ms: 500, time_to_first_token_ms: 200, provider_queue_ms: 40 },
          },
          parts: [
            { id: "t", sessionID: "s", messageID: "a", type: "tool", state: { duration_ms: 75 } },
          ],
        },
      ],
      { id: "s", status: "idle", updated_at: 1_700_000_000_900 },
      12,
    );
    expect(diagnostics).toEqual({
      routingMs: 60,
      providerQueueMs: 40,
      firstTokenMs: 200,
      providerMs: 500,
      toolExecutionMs: 75,
      persistenceMs: 100,
      uiRenderMs: 12,
      totalMs: 900,
    });
  });

  test("leaves unavailable stages blank", () => {
    const diagnostics = turnLatencyDiagnostics([
      { id: "u", sessionID: "s", role: "user", parts: [] },
    ]);
    expect(diagnostics.routingMs).toBeUndefined();
    expect(diagnostics.providerQueueMs).toBeUndefined();
  });
});
