import { describe, expect, test } from "bun:test";
import { GatewayClient } from "./client";

describe("GatewayClient", () => {
  test("normalizes wrapped gateway message list items", async () => {
    const client = new GatewayClient({
      baseUrl: "http://gateway.test",
      fetch: mockFetch([
        {
          info: {
            id: "m1",
            sessionID: "s1",
            role: "assistant",
            parts: [],
          },
          parts: [{ id: "p1", type: "text", text: "hello" }],
        },
      ]),
    });

    const messages = await client.messages("s1");

    expect(messages[0]?.id).toBe("m1");
    expect(messages[0]?.parts[0]?.text).toBe("hello");
  });

  test("scopes directory query and header for workspace requests", async () => {
    let observedUrl = "";
    let observedHeader = "";
    const client = new GatewayClient({
      baseUrl: "http://gateway.test",
      directory: "C:\\repo",
      fetch: async (input, init) => {
        observedUrl = String(input);
        observedHeader =
          new Headers(init?.headers).get("x-opencode-directory") ?? "";
        return jsonResponse([]);
      },
    });

    await client.files("src");

    expect(observedUrl).toContain("directory=C%3A%5Crepo");
    expect(observedUrl).toContain("path=src");
    expect(observedHeader).toBe("C%3A%5Crepo");
  });

  test("updates model config with a tier provider model selection", async () => {
    let observedBody = "";
    const client = new GatewayClient({
      baseUrl: "http://gateway.test",
      fetch: async (_input, init) => {
        observedBody = String(init?.body ?? "");
        return jsonResponse({ path: "config/provider_config.json", tiers: [] });
      },
    });

    await client.putModelConfig({
      tier: "fast",
      provider: "codex",
      model: "gpt-5.1-codex-mini",
    });

    expect(JSON.parse(observedBody)).toEqual({
      tier: "fast",
      provider: "codex",
      model: "gpt-5.1-codex-mini",
    });
  });

  test("aborts unanswered requests after the configured timeout", async () => {
    let aborted = false;
    const client = new GatewayClient({
      baseUrl: "http://gateway.test",
      timeoutMs: 5,
      fetch: ((_input, init) =>
        new Promise((_resolve, reject) => {
          init?.signal?.addEventListener(
            "abort",
            () => {
              aborted = true;
              reject(
                init.signal?.reason ??
                  new DOMException(
                    "Gateway request timed out.",
                    "TimeoutError",
                  ),
              );
            },
            { once: true },
          );
        })) as typeof fetch,
    });

    await expect(client.health()).rejects.toThrow("Gateway request timed out.");
    expect(aborted).toBe(true);
  });
});

function mockFetch(payload: unknown): typeof fetch {
  return async () => jsonResponse(payload);
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}
