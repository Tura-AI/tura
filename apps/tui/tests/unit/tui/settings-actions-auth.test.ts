import assert from "node:assert/strict";
import test from "node:test";
import { applySelectedSetting, submitSettingInput } from "../../../src/tui/settings-actions.js";
import { initialState, reducer, type AppState } from "../../../src/tui/reducer.js";
import { setExternalUrlOpenerForTests } from "../../../src/utils/external-url.js";
import type { TuiGatewayClient } from "../../../src/tui/runtime.js";

test("provider OAuth setting opens the browser and starts callback input", async () => {
  let state = providerAuthState();
  const opened: string[] = [];
  setExternalUrlOpenerForTests(async (url) => {
    opened.push(url);
    return { ok: true };
  });
  try {
    await applySelectedSetting(
      mockClient(),
      () => state,
      (action) => {
        state = reducer(state, action);
      },
    );
  } finally {
    setExternalUrlOpenerForTests();
  }

  assert.deepEqual(opened, ["https://example.test/oauth"]);
  assert.equal(state.settingInput?.kind, "oauth-callback");
  assert.equal(state.settingInput?.providerID, "mock");
  assert.equal(state.settingInput?.method, 0);
  assert.match(state.notice ?? "", /Open mock OAuth/u);
});

test("API key input validates before saving and preserves invalid input", async () => {
  let saved = false;
  let state: AppState = {
    ...providerAuthState(),
    settingInput: { kind: "api-key" as const, providerID: "mock", prompt: "API key" },
    composer: "bad-token",
  };
  const client = mockClient({
    providerAuthValidate: async () => ({
      ok: false,
      provider_id: "mock",
      code: "provider.validation.failed",
      message: "credential validation failed",
      level: "invalid",
      status: authStatus(false),
    }),
    setProviderAuth: async () => {
      saved = true;
      return true;
    },
  });

  await submitSettingInput(
    client,
    () => state,
    (action) => {
      state = reducer(state, action);
    },
  );

  assert.equal(saved, false);
  assert.equal(state.settingInput?.kind, "api-key");
  assert.equal(state.composer, "bad-token");
  assert.equal(state.notice, "credential validation failed");
});

test("OAuth callback input completes through the callback endpoint", async () => {
  let callbackPayload: unknown;
  let saved = false;
  let state: AppState = {
    ...providerAuthState(),
    settingInput: {
      kind: "oauth-callback" as const,
      providerID: "mock",
      method: 2,
      prompt: "Callback",
    },
    composer: "https://localhost/callback?code=abc&state=xyz",
  };
  const client = mockClient({
    providerOauthCallback: async (_providerID, payload) => {
      callbackPayload = payload;
      return {
        ok: true,
        provider_id: "mock",
        code: "provider.oauth.completed",
        message: "provider OAuth completed",
        level: "valid",
        status: authStatus(true),
      };
    },
    setProviderAuth: async () => {
      saved = true;
      return true;
    },
  });

  await submitSettingInput(
    client,
    () => state,
    (action) => {
      state = reducer(state, action);
    },
  );

  assert.equal(saved, false);
  assert.deepEqual(callbackPayload, {
    method: 2,
    code: "https://localhost/callback?code=abc&state=xyz",
  });
  assert.equal(state.settingInput, undefined);
  assert.equal(state.composer, "");
  assert.equal(state.authStatuses.mock.authenticated, true);
});

function providerAuthState(): AppState {
  return {
    ...initialState("C:/repo"),
    settingsOpen: true,
    settingDetail: "providerAuth",
    selectedProviderID: "mock",
    selectedSettingOptionIndex: 0,
    sessionConfig: {
      active_provider: "mock",
      active_agent: "balanced",
      active_model: "mock-fast",
      model: "mock/mock-fast",
      model_variant: "high",
      model_acceleration_enabled: false,
    },
    authMethods: {
      mock: [
        {
          type: "oauth",
          kind: "browser",
          login: "oauth",
          label: "Mock OAuth",
          available: true,
          supports_refresh: false,
        },
        {
          type: "api_key",
          kind: "api_key",
          login: "api",
          label: "Mock API key",
          token_env: "MOCK_API_KEY",
          available: true,
          supports_refresh: false,
        },
      ],
    },
    authStatuses: { mock: authStatus(false) },
  };
}

function authStatus(authenticated: boolean): AppState["authStatuses"][string] {
  return {
    provider_id: "mock",
    display_name: "Mock",
    configured: authenticated,
    authenticated,
    runtime_state: authenticated ? "connected" : "missing",
  };
}

function mockClient(overrides: Partial<TuiGatewayClient> = {}): TuiGatewayClient {
  return {
    providerOauthAuthorize: async () => ({
      url: "https://example.test/oauth",
      method: "code",
      instructions: "Open mock OAuth",
    }),
    providerOauthCallback: async () => ({
      ok: true,
      provider_id: "mock",
      code: "provider.oauth.completed",
      message: "provider OAuth completed",
      level: "valid",
      status: authStatus(true),
    }),
    providerAuthValidate: async () => ({
      ok: true,
      provider_id: "mock",
      code: "provider.validation.passed",
      message: "credential validation passed",
      level: "valid",
      status: authStatus(true),
    }),
    providerAuthStatus: async () => authStatus(false),
    setProviderAuth: async () => true,
    providerLogout: async () => true,
    ...overrides,
  } as TuiGatewayClient;
}
