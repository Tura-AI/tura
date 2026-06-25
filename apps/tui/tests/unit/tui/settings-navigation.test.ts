import assert from "node:assert/strict";
import test from "node:test";
import { draw } from "../../../src/tui/draw.js";
import { settingsLines } from "../../../src/tui/render/settings.js";
import { renderFrame } from "../../../src/tui/render.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { setActiveCapabilities, stripAnsi } from "../../../src/tui/render-terminal.js";
import { initialState, reducer, type AppState } from "../../../src/tui/reducer.js";
import type { Session } from "../../../src/types/session.js";

test("opening a setting detail starts selection at the first option", () => {
  const root = {
    ...baseState(),
    settingsOpen: true,
    selectedSettingsIndex: 6,
    selectedSettingOptionIndex: 99,
  };

  const next = reducer(root, { type: "open-setting-detail", detail: "variant" });

  assert.equal(next.settingDetail, "variant");
  assert.equal(next.selectedSettingOptionIndex, 0);
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

test("settings root shows command default while hiding removed validator stall guard and session type entries", () => {
  setActiveCapabilities(richCapabilities());
  const rendered = stripAnsi(settingsLines(baseState(), 96, 20).join("\n"));

  assert.match(rendered, /Acceleration mode/u);
  assert.match(rendered, /Show commands by default\s+true/u);
  assert.match(rendered, /Persona\s+tura/u);
  assert.match(rendered, /Language\s+en/u);
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
