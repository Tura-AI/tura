import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import type { Setter } from "solid-js";
import {
  isGatewayTimeoutError,
  tryStartGateway,
  waitForGatewayHealth,
} from "../../app/src/app-gateway-startup";
import type { AppState } from "../../app/src/state/global-store";

let invokeCalls: Array<{ command: string; args: unknown }> = [];
let invokeResult: unknown = { status: "connected" };
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
    invokeResult = { status: "connected" };
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

  test("connects by probing existing gateway health instead of dev-server start endpoint", async () => {
    const fetchCalls: Array<{ url: string; init?: RequestInit }> = [];
    globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
      fetchCalls.push({ url: String(url), init });
      return Response.json({ healthy: true });
    }) as typeof fetch;
    const { state, setState } = stateHarness();

    await expect(tryStartGateway("http://127.0.0.1:4126", false, setState)).resolves.toBe(true);

    expect(fetchCalls).toHaveLength(1);
    expect(fetchCalls[0]?.url).toBe("http://127.0.0.1:4126/global/health");
    expect(fetchCalls[0]?.init?.method).toBeUndefined();
    expect(state().connection).toBe("connecting");
    expect(state().gatewayStartupNotice).toBeTruthy();
    expect(invokeCalls).toHaveLength(0);
  });

  test("returns false when browser health probe cannot connect", async () => {
    globalThis.fetch = (async () => {
      throw new TypeError("Failed to fetch");
    }) as typeof fetch;
    const { setState } = stateHarness();

    await expect(tryStartGateway("http://localhost:4100", false, setState)).resolves.toBe(false);
    expect(invokeCalls).toHaveLength(0);
  });

  test("uses the Tauri command inside the Tauri runtime", async () => {
    globalThis.window = {
      setTimeout: (_callback: TimerHandler, _timeout?: number) => 0,
      clearTimeout: (_handle?: number) => undefined,
      __TAURI_INTERNALS__: {},
    } as Window & typeof globalThis;
    const { setState } = stateHarness();

    await expect(tryStartGateway("http://localhost:4100", false, setState)).resolves.toBe(true);

    expect(invokeCalls).toEqual([
      {
        command: "start_gateway",
        args: { gatewayUrl: "http://localhost:4100", gatewayUrlExplicit: false },
      },
    ]);
  });

  test("uses the gateway url returned by the Tauri connect command", async () => {
    globalThis.window = {
      setTimeout: (_callback: TimerHandler, _timeout?: number) => 0,
      clearTimeout: (_handle?: number) => undefined,
      __TAURI_INTERNALS__: {},
    } as Window & typeof globalThis;
    invokeResult = { status: "connected", gatewayUrl: "http://127.0.0.1:49231" };
    const { state, setState } = stateHarness();

    await expect(tryStartGateway("http://127.0.0.1:4126", false, setState)).resolves.toBe(true);

    expect(state().gatewayUrl).toBe("http://127.0.0.1:49231");
  });

  test("returns false when the Tauri connect command cannot reach gateway", async () => {
    globalThis.window = {
      setTimeout: (_callback: TimerHandler, _timeout?: number) => 0,
      clearTimeout: (_handle?: number) => undefined,
      __TAURI_INTERNALS__: {},
    } as Window & typeof globalThis;
    invokeError = new Error("gateway unavailable");
    const { setState } = stateHarness();

    await expect(tryStartGateway("http://localhost:4126", false, setState)).resolves.toBe(false);
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
      return Response.json({ healthy: true });
    }) as typeof fetch;
    const { state, setState } = stateHarness();

    await waitForGatewayHealth("http://127.0.0.1:4126///", 1000, setState);

    expect(urls).toEqual(["http://127.0.0.1:4126/global/health"]);
    expect(state().settingsNotice).toBeUndefined();
    expect(state().gatewayStartupNotice).toBeUndefined();
  });

  test("health polling rejects non-gateway 200 responses", async () => {
    globalThis.fetch = (async () => new Response("ok", { status: 200 })) as typeof fetch;
    globalThis.window = {
      setTimeout: (callback: TimerHandler, _timeout?: number) => {
        if (typeof callback === "function") callback();
        return 0;
      },
      clearTimeout: (_handle?: number) => undefined,
    } as Window & typeof globalThis;
    const { setState } = stateHarness();

    await expect(waitForGatewayHealth("http://127.0.0.1:4126", 1, setState)).rejects.toThrow(
      DOMException,
    );
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
