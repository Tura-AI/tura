import { describe, expect, test } from "bun:test";
import { GatewayClient, defaultGatewayUrl } from "../../../../sdk/gateway/src/client";

describe("GatewayClient", () => {
  test("defaults to the release gateway port inside Tauri", () => {
    const previousWindow = globalThis.window;
    globalThis.window = {
      __TAURI_INTERNALS__: {},
      location: { search: "" },
    } as Window & typeof globalThis;
    try {
      expect(defaultGatewayUrl()).toBe("http://127.0.0.1:4126");
    } finally {
      globalThis.window = previousWindow;
    }
  });

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
        observedHeader = new Headers(init?.headers).get("x-opencode-directory") ?? "";
        return jsonResponse([]);
      },
    });

    await client.files("src");

    expect(observedUrl).toContain("directory=C%3A%5Crepo");
    expect(observedUrl).toContain("path=src");
    expect(observedHeader).toBe("C%3A%5Crepo");
  });

  test("scopes non-English directory paths without mojibake", async () => {
    let observedUrl = "";
    let observedHeader = "";
    const directory = "C:\\Users\\测试\\项目";
    const encodedDirectory = encodeURIComponent(directory);
    const client = new GatewayClient({
      baseUrl: "http://gateway.test",
      directory,
      fetch: async (input, init) => {
        observedUrl = String(input);
        observedHeader = new Headers(init?.headers).get("x-opencode-directory") ?? "";
        return jsonResponse([]);
      },
    });

    await client.files("src");

    expect(observedUrl).toContain(`directory=${encodedDirectory}`);
    expect(observedHeader).toBe(encodedDirectory);
    expect(decodeURIComponent(observedHeader)).toBe(directory);
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

  test("validates provider auth through the provider auth endpoint", async () => {
    let observedUrl = "";
    let observedMethod = "";
    let observedBody = "";
    const client = new GatewayClient({
      baseUrl: "http://gateway.test",
      fetch: async (input, init) => {
        observedUrl = String(input);
        observedMethod = init?.method ?? "";
        observedBody = String(init?.body ?? "");
        return jsonResponse({
          ok: true,
          provider_id: "custom-openai",
          code: "provider.validation.passed",
          message: "credential validation passed",
          level: "valid",
        });
      },
    });

    const result = await client.providerAuthValidate("custom-openai", {
      type: "api",
      kind: "api_key",
      login: "api",
      token_env: "CUSTOM_OPENAI_API_KEY",
      key: "sk-test",
    });

    expect(observedUrl).toBe("http://gateway.test/provider/custom-openai/auth/validate");
    expect(observedMethod).toBe("POST");
    expect(JSON.parse(observedBody)).toEqual({
      type: "api",
      kind: "api_key",
      login: "api",
      token_env: "CUSTOM_OPENAI_API_KEY",
      key: "sk-test",
    });
    expect(result.ok).toBe(true);
  });

  test("falls back to live session list when session log is empty", async () => {
    const observed: string[] = [];
    const client = new GatewayClient({
      baseUrl: "http://gateway.test",
      directory: "C:\\repo",
      fetch: async (input) => {
        const url = String(input);
        observed.push(url);
        if (url.includes("/session-log/sessions")) {
          return jsonResponse({ page: 0, page_size: 100, total: 0, sessions: [] });
        }
        return jsonResponse([{ id: "s-live", status: "idle", name: "live session" }]);
      },
    });

    const sessions = await client.sessions();

    expect(sessions[0]?.id).toBe("s-live");
    expect(observed.some((url) => url.includes("/session-log/sessions"))).toBe(true);
    expect(observed.some((url) => url.includes("/session?"))).toBe(true);
  });

  test("retries transient network changed failures", async () => {
    let calls = 0;
    const client = new GatewayClient({
      baseUrl: "http://gateway.test",
      fetch: async () => {
        calls += 1;
        if (calls === 1) {
          throw new TypeError("Failed to fetch: net::ERR_NETWORK_CHANGED");
        }
        return jsonResponse({ healthy: true, version: "test" });
      },
    });

    await expect(client.health()).resolves.toEqual({ healthy: true, version: "test" });
    expect(calls).toBe(2);
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
                  new DOMException("Gateway request timed out.", "TimeoutError"),
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
