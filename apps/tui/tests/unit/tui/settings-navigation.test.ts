import assert from "node:assert/strict";
import test from "node:test";
import { draw } from "../../../src/tui/draw.js";
import { settingsLines } from "../../../src/tui/render/settings.js";
import { renderFrame } from "../../../src/tui/render.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { setActiveCapabilities, stripAnsi } from "../../../src/tui/render-terminal.js";
import { initialState, reducer, type AppState } from "../../../src/tui/reducer.js";
import { hydrate } from "../../../src/tui/runtime.js";
import type { TuiGatewayClient } from "../../../src/tui/runtime.js";
import type { Session } from "../../../src/types/session.js";

test("opening a setting detail starts selection at the active config option", () => {
  const root = {
    ...baseState(),
    settingsOpen: true,
    selectedSettingsIndex: 6,
    selectedSettingOptionIndex: 99,
  };

  const next = reducer(root, { type: "open-setting-detail", detail: "variant" });

  assert.equal(next.settingDetail, "variant");
  assert.equal(next.selectedSettingOptionIndex, 2);
});

test("initial hydrate opens provider settings when no LLM provider is configured", async () => {
  const next = await hydrate(
    initialState("C:/repo"),
    hydrateClient({ connected: [], configured: false }),
    session("sess-settings"),
  );

  assert.equal(next.settingsOpen, true);
  assert.equal(next.settingDetail, "provider");
  assert.equal(next.selectedProviderID, undefined);
});

test("initial hydrate keeps the chat surface when an LLM provider is configured", async () => {
  const next = await hydrate(
    initialState("C:/repo"),
    hydrateClient({ connected: ["mock"], configured: true }),
    session("sess-settings"),
  );

  assert.equal(next.settingsOpen, false);
  assert.equal(next.settingDetail, undefined);
});

test("initial hydrate opens provider settings when the provider list is empty", async () => {
  const next = await hydrate(
    initialState("C:/repo"),
    hydrateClient({ connected: [], configured: false, emptyProviders: true }),
    session("sess-settings"),
  );

  assert.equal(next.settingsOpen, true);
  assert.equal(next.settingDetail, "provider");
});

test("setting detail rendering pages when selection moves past visible rows", () => {
  setActiveCapabilities(richCapabilities());
  const state = {
    ...baseState(),
    settingsOpen: true,
    settingDetail: "variant" as const,
    selectedSettingOptionIndex: 2,
  };

  const rendered = settingsLines(state, 88, 6).join("\n");

  assert.doesNotMatch(rendered, /> low/u);
  assert.doesNotMatch(rendered, /> medium/u);
  assert.match(rendered, /> high/u);
});

test("agent setting marker follows session config instead of stale session agent", () => {
  setActiveCapabilities(richCapabilities());
  const state = reducer(
    {
      ...baseState(),
      session: { ...session("sess-settings"), agent: "stale-agent" },
      sessionConfig: {
        ...baseState().sessionConfig,
        active_agent: "thoughtful",
      },
      agents: [storedAgent("stale-agent"), storedAgent("thoughtful")],
    },
    { type: "open-setting-detail", detail: "agent" },
  );

  const rendered = stripAnsi(settingsLines(state, 96, 12).join("\n"));

  assert.match(rendered, /thoughtful\s+[✓*]/u);
  assert.doesNotMatch(rendered, /stale-agent\s+[✓*]/u);
  assert.equal(state.selectedSettingOptionIndex, 1);
});

test("settings root renders configured tier as real model, not thinking", () => {
  setActiveCapabilities(richCapabilities());
  const state = {
    ...baseState(),
    session: { ...session("sess-settings"), model: "thinking" },
    sessionConfig: {
      ...baseState().sessionConfig,
      model: "thinking",
      active_model: undefined,
    },
    modelConfig: {
      path: "C:/repo/.tura/config.conf",
      tiers: [
        {
          tier: "thinking",
          current: { provider: "mock", model: "mock-fast" },
          options: [{ provider: "mock", model: "mock-fast" }],
        },
      ],
    },
  };

  const rendered = stripAnsi(settingsLines(state, 96, 20).join("\n"));

  assert.match(rendered, /Model\s+mock\/mock-fast/u);
  assert.doesNotMatch(rendered, /Model\s+thinking/u);
});

test("persona setting marker follows session config over stale session persona", () => {
  setActiveCapabilities(richCapabilities());
  const state = reducer(
    {
      ...baseState(),
      sessionConfig: {
        ...baseState().sessionConfig,
        active_persona: "wonderful",
      },
      personas: [
        { summary: { id: "tura", source: "static" } },
        { summary: { id: "wonderful", source: "static" } },
      ],
    },
    { type: "open-setting-detail", detail: "persona" },
  );

  const rendered = stripAnsi(settingsLines(state, 96, 12).join("\n"));

  assert.match(rendered, /wonderful\s+[✓*]/u);
  assert.doesNotMatch(rendered, /tura\s+[✓*]/u);
  assert.equal(state.selectedSettingOptionIndex, 1);
});

test("settings root hides removed command validator stall guard and session type entries", () => {
  setActiveCapabilities(richCapabilities());
  const rendered = stripAnsi(settingsLines(baseState(), 96, 20).join("\n"));

  assert.match(rendered, /Priority mode/u);
  assert.doesNotMatch(rendered, /Fast mode/u);
  assert.doesNotMatch(rendered, /Acceleration mode/u);
  assert.match(rendered, /Persona\s+tura/u);
  assert.match(rendered, /Language\s+en/u);
  assert.doesNotMatch(rendered, /Show commands by default/u);
  assert.doesNotMatch(rendered, /Session type/u);
  assert.doesNotMatch(rendered, /Validator/u);
  assert.doesNotMatch(rendered, /Command stall guard/u);
});

test("settings and session panels render page count in bottom meta", () => {
  const settings = renderFrame(baseState(), richCapabilities()).frame;
  assert.match(settings, /1\/1/u);

  const sessions = renderFrame(
    {
      ...baseState(),
      settingsOpen: false,
      sessionsOpen: true,
      sessions: Array.from({ length: 9 }, (_item, index) => session(`sess-${index}`)),
      selectedSessionIndex: 8,
    },
    richCapabilities(),
  ).frame;
  assert.match(sessions, /\d+\/\d+/u);
});

test("draw keeps the terminal cursor hidden on settings pages even while collecting input", () => {
  const state = {
    ...baseState(),
    settingsOpen: true,
    settingDetail: "providerAuth" as const,
    selectedProviderID: "mock",
    settingInput: { kind: "api-key" as const, providerID: "mock", prompt: "API key" },
  };

  const writes = captureDrawWrites(() => draw(state, richCapabilities(), ""));
  const output = writes.join("");

  assert.match(output, /\x1b\[\?25l/u);
  assert.doesNotMatch(output, /\x1b\[\?25h/u);
});

function baseState(): AppState {
  return {
    ...initialState("C:/repo"),
    session: session("sess-settings"),
    sessions: [session("sess-settings")],
    settingsOpen: true,
    sessionConfig: {
      active_provider: "mock",
      active_agent: "thinking",
      model: "mock/mock-fast",
      active_model: "mock/mock-fast",
      model_variant: "high",
      language: undefined,
      session_type: "coding",
      model_acceleration_enabled: true,
    },
    providers: {
      all: [
        {
          id: "mock",
          name: "Mock",
          source: "mock",
          models: {
            "mock-fast": { id: "mock-fast", name: "Mock Fast" },
          },
        },
      ],
      default: { llm: "mock/mock-fast" },
      connected: ["mock"],
      enums: {
        domains: ["llm"],
        capabilities: ["chat"],
        api_styles: ["mock"],
        auth_methods: ["none"],
        statuses: ["connected"],
      },
    },
  };
}

function session(id: string): Session {
  return {
    id,
    name: null,
    parent_id: null,
    created_at: 1,
    updated_at: 1,
    directory: "C:/repo",
    model: "mock/mock-fast",
    agent: "thinking",
    session_type: "coding",
    auto_session_name: true,
    kill_processes_on_start: false,
    validator_enabled: false,
    force_planning: false,
    model_variant: "high",
    model_acceleration_enabled: true,
    disable_permission_restrictions: false,
    status: "idle",
    message_count: 0,
    task_management: {},
    plan_summary: null,
    session_display_name: null,
  };
}

function storedAgent(id: string): AppState["agents"][number] {
  return {
    summary: {
      id,
      name: id,
      description: id,
      source: "static",
      path: `agents/${id}.md`,
      aliases: [],
      capabilities: [],
      hidden: false,
    },
    config: { agent_name: id },
  };
}

function hydrateClient(options: {
  connected: string[];
  configured: boolean;
  emptyProviders?: boolean;
}): TuiGatewayClient {
  const providers: NonNullable<AppState["providers"]> = {
    all: options.emptyProviders
      ? []
      : [
          {
            id: "mock",
            name: "Mock",
            source: "mock",
            options: { domains: ["llm"] },
            models: {
              "mock-fast": { id: "mock-fast", name: "Mock Fast" },
            },
          },
        ],
    default: { mock: "mock-fast" },
    connected: options.connected,
    enums: {
      domains: ["llm"],
      capabilities: [],
      api_styles: [],
      auth_methods: [],
      statuses: [],
    },
  };
  return {
    listMessages: async () => [],
    listProviders: async () => providers,
    getSessionConfig: async () => baseState().sessionConfig!,
    modelConfig: async () => ({ path: "C:/repo/.tura/config.conf", tiers: [] }),
    listAgents: async () => [],
    listPersonas: async () => [],
    listProviderAuthMethods: async () => ({ mock: [] }),
    providerAuthStatus: async () => ({
      provider_id: "mock",
      display_name: "Mock",
      configured: options.configured,
      authenticated: options.configured,
      auth_state: options.configured ? "authenticated" : "missing",
      runtime_state: options.configured ? "ready" : "missing",
    }),
    listSessions: async () => [session("sess-settings")],
  } as unknown as TuiGatewayClient;
}

function captureDrawWrites(run: () => void): string[] {
  const writes: string[] = [];
  const stdout = process.stdout as typeof process.stdout & {
    isTTY?: boolean;
    columns?: number;
    rows?: number;
  };
  const originalIsTTY = Object.getOwnPropertyDescriptor(stdout, "isTTY");
  const originalColumns = Object.getOwnPropertyDescriptor(stdout, "columns");
  const originalRows = Object.getOwnPropertyDescriptor(stdout, "rows");
  const originalWrite = stdout.write;
  Object.defineProperty(stdout, "isTTY", { configurable: true, value: true });
  Object.defineProperty(stdout, "columns", { configurable: true, value: 96 });
  Object.defineProperty(stdout, "rows", { configurable: true, value: 12 });
  stdout.write = ((chunk: unknown) => {
    writes.push(String(chunk));
    return true;
  }) as typeof stdout.write;
  try {
    run();
  } finally {
    stdout.write = originalWrite;
    restore(stdout, "isTTY", originalIsTTY);
    restore(stdout, "columns", originalColumns);
    restore(stdout, "rows", originalRows);
  }
  return writes;
}

function restore(
  target: typeof process.stdout,
  key: "isTTY" | "columns" | "rows",
  descriptor: PropertyDescriptor | undefined,
): void {
  if (descriptor) Object.defineProperty(target, key, descriptor);
  else delete (target as unknown as Record<string, unknown>)[key];
}
