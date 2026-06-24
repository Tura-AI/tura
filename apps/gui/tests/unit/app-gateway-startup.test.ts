import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import type { Setter } from "solid-js";
import {
  isGatewayTimeoutError,
  tryStartGateway,
  waitForGatewayHealth,
} from "../../app/src/app-gateway-startup";
import type { AppState } from "../../app/src/state/global-store";

let invokeCalls: Array<{ command: string; args: unknown }> = [];
let invokeResult: unknown = { status: "starting" };
let invokeError: unknown;

mock.module("@tauri-apps/api/core", () => ({
  invoke: async (command: string, args: unknown) => {
    invokeCalls.push({ command, args });
    if (invokeError) throw invokeError;
    return invokeResult;
  },
}));

describe("gateway startup wrapper", () => {
  const originalFetch = globalThis.fetch;
  const originalWindow = globalThis.window;

  beforeEach(() => {
    invokeCalls = [];
    invokeResult = { status: "starting" };
    invokeError = undefined;
    globalThis.window = {
      setTimeout: (_callback: TimerHandler, _timeout?: number) => 0,
      clearTimeout: (_handle?: number) => undefined,
    } as Window & typeof globalThis;
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    globalThis.window = originalWindow;
  });

  test("starts through the dev server endpoint first", async () => {
    const fetchCalls: Array<{ url: string; init?: RequestInit }> = [];
    globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
      fetchCalls.push({ url: String(url), init });
      return Response.json({ ok: true, status: "building" });
    }) as typeof fetch;
    const { state, setState } = stateHarness();

    await expect(tryStartGateway("http://127.0.0.1:4126", setState)).resolves.toBe(true);

    expect(fetchCalls).toHaveLength(1);
    expect(fetchCalls[0]?.url).toBe("/__tura/start-gateway");
    expect(JSON.parse(String(fetchCalls[0]?.init?.body))).toEqual({
      gatewayUrl: "http://127.0.0.1:4126",
    });
    expect(state().connection).toBe("connecting");
    expect(state().gatewayStartupNotice).toBeTruthy();
    expect(invokeCalls).toHaveLength(0);
  });

  test("falls back to the Tauri command when the dev endpoint is unavailable", async () => {
    globalThis.fetch = (async () => {
      throw new TypeError("Failed to fetch");
    }) as typeof fetch;
    const { setState } = stateHarness();

    await expect(tryStartGateway("http://localhost:4100", setState)).resolves.toBe(true);

    expect(invokeCalls).toEqual([
      {
        command: "start_gateway",
        args: { gatewayUrl: "http://localhost:4100" },
      },
    ]);
  });

  test("uses the Tauri command before the dev endpoint inside the Tauri runtime", async () => {
    globalThis.window = {
      setTimeout: (_callback: TimerHandler, _timeout?: number) => 0,
      clearTimeout: (_handle?: number) => undefined,
      __TAURI_INTERNALS__: {},
    } as Window & typeof globalThis;
    const fetchCalls: string[] = [];
    globalThis.fetch = (async (url: string | URL | Request) => {
      fetchCalls.push(String(url));
      return new Response("missing", { status: 404 });
    }) as typeof fetch;
    const { setState } = stateHarness();

    await expect(tryStartGateway("http://localhost:4100", setState)).resolves.toBe(true);

    expect(invokeCalls).toEqual([
      {
        command: "start_gateway",
        args: { gatewayUrl: "http://localhost:4100" },
      },
    ]);
    expect(fetchCalls).toEqual([]);
  });

  test("uses the gateway url returned by the Tauri command", async () => {
    globalThis.window = {
      setTimeout: (_callback: TimerHandler, _timeout?: number) => 0,
      clearTimeout: (_handle?: number) => undefined,
      __TAURI_INTERNALS__: {},
    } as Window & typeof globalThis;
    invokeResult = { status: "starting", gatewayUrl: "http://127.0.0.1:49231" };
    const { state, setState } = stateHarness();

    await expect(tryStartGateway("http://127.0.0.1:4126", setState)).resolves.toBe(true);

    expect(state().gatewayUrl).toBe("http://127.0.0.1:49231");
  });

  test("returns false when neither dev server nor Tauri can start gateway", async () => {
    globalThis.fetch = (async () => new Response("missing", { status: 404 })) as typeof fetch;
    invokeError = new Error("command denied");
    const { setState } = stateHarness();

    await expect(tryStartGateway("http://localhost:4126", setState)).resolves.toBe(false);
  });

  test("recognizes abort, timeout, and fetch failures as gateway timeouts", () => {
    expect(isGatewayTimeoutError(new DOMException("aborted", "AbortError"))).toBe(true);
    expect(isGatewayTimeoutError(new DOMException("timed out", "TimeoutError"))).toBe(true);
    expect(isGatewayTimeoutError(new TypeError("Failed to fetch"))).toBe(true);
    expect(isGatewayTimeoutError(new TypeError("fetch failed"))).toBe(true);
    expect(
      isGatewayTimeoutError(new TypeError("NetworkError when attempting to fetch resource.")),
    ).toBe(true);
    expect(isGatewayTimeoutError(new TypeError("Load failed"))).toBe(true);
    expect(isGatewayTimeoutError(new Error("other"))).toBe(false);
  });

  test("health polling strips trailing slashes before probing", async () => {
    const urls: string[] = [];
    globalThis.fetch = (async (url: string | URL | Request) => {
      urls.push(String(url));
      return new Response("ok", { status: 200 });
    }) as typeof fetch;
    const { state, setState } = stateHarness();

    await waitForGatewayHealth("http://127.0.0.1:4126///", 1000, setState);

    expect(urls).toEqual(["http://127.0.0.1:4126/global/health"]);
    expect(state().settingsNotice).toBeUndefined();
    expect(state().gatewayStartupNotice).toBeUndefined();
  });
});

function stateHarness() {
  let current = {
    loading: false,
    connection: "offline",
    gatewayUrl: "http://127.0.0.1:4126",
    error: "previous",
    settingsNotice: "Waiting for Gateway health...",
    gatewayStartupNotice: "Waiting for Gateway health...",
  } as unknown as AppState;
  const setState = ((updater: (previous: AppState) => AppState) => {
    current = updater(current);
    return current;
  }) as Setter<AppState>;
  return {
    state: () => current,
    setState,
  };
}
