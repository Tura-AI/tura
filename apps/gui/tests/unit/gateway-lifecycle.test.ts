import { afterEach, expect, mock, test } from "bun:test";
import * as solid from "../../app/node_modules/solid-js/dist/solid.js";

mock.module("solid-js", () => solid);

const { createRoot, createSignal } = solid;

const originalFetch = globalThis.fetch;
const originalWindow = globalThis.window;

afterEach(() => {
  globalThis.fetch = originalFetch;
  globalThis.window = originalWindow;
});

test("gateway startup failure exits the loading state", async () => {
  const [{ useAppGatewayLifecycle }, { initialAppState }] = await Promise.all([
    import("../../app/src/hooks/use-app-gateway-lifecycle"),
    import("../../app/src/state/global-store"),
  ]);
  const [state, setState] = createSignal(initialAppState("http://127.0.0.1:65530"));
  globalThis.window = globalThis as Window & typeof globalThis;
  globalThis.fetch = (async () => {
    throw new TypeError("Failed to fetch");
  }) as typeof fetch;
  let dispose = () => {};

  createRoot((rootDispose) => {
    dispose = rootDispose;
    useAppGatewayLifecycle({
      state,
      setState,
      gatewayUrl: () => state().gatewayUrl,
      gatewayUrlExplicit: false,
      rootClient: () => ({}) as never,
      forceNewSession: false,
      openSession: async () => {},
    });
  });
  await new Promise((resolve) => setTimeout(resolve, 0));

  expect(state().loading).toBe(false);
  expect(state().bootstrapped).toBe(true);
  expect(state().connection).toBe("disconnected");
  expect(state().error).toBeTruthy();
  dispose();
});
