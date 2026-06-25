import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render } from "../../../src/tui/render.js";
import {
  ansiCapabilities,
  plainCapabilities,
  richCapabilities,
} from "../../../src/tui/capabilities.js";
import { stripAnsi, visibleTextWidth } from "../../../src/tui/render-terminal.js";
import {
  providerEnums,
  withTerminalSize,
  assertFitsTerminal,
  assertLineWidths,
  assertOpencodePalette,
  assertWideMenuGap,
} from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

test("settings renders with help-style section rails and no command hint copy", () => {
  const session = { id: "sess-settings", title: "Settings Session", status: "idle" as const };
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session,
      messages: [],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
      sessionConfig: {
        model: "gpt-5.5",
        active_provider: "openai",
        active_agent: "build",
        language: "en",
        session_type: "coding",
        show_command_instructions: false,
        validator_enabled: true,
        context_message_limit: 64,
      },
    }),
    { type: "toggle-settings" },
  );

  const ansi = withTerminalSize(72, 20, () => render(state, ansiCapabilities()));
  assertFitsTerminal(ansi, 72, 20);
  assert.match(ansi, /─── .*Session Settings.* ─────────/);
  const ansiLines = ansi.split("\n");
  const ansiTitleIndex = ansiLines.findIndex((line) => {
    const text = stripAnsi(line);
    return text.includes("───") && text.includes("Session Settings");
  });
  assert.ok(ansiTitleIndex >= 0);
  assert.doesNotMatch(ansiLines[ansiTitleIndex - 1] ?? "", /\x1b\[48;2;20;23;24m/);
  assert.doesNotMatch(stripAnsi(ansi), /^─{8,}$/mu);
  assert.match(
    ansi,
    /^\x1b\[48;2;20;23;24m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;20;23;24m \S/m,
  );
  assert.match(ansi, /Enter opens; Esc returns to chat/);
  assert.match(ansi, /^\x1b\[48;2;20;23;24m\x1b\[38;2;103;116;111m▏\x1b\[0m.*> Model/m);
  assert.match(stripAnsi(ansi), /Show commands by default\s+false/);
  const ansiText = stripAnsi(ansi);
  assert.match(ansiText, /Language\s+en/);
  assert.doesNotMatch(ansiText, /Session type\s+coding/);
  assert.doesNotMatch(ansiText, /Validator\s+true/);
  assert.doesNotMatch(ansi, /Context messages/);
  assert.doesNotMatch(ansi, /\/config get|\/config set|\/model provider\/model/);
  assert.doesNotMatch(ansi, /\/model <provider\/model>|\/commands/);
  assert.doesNotMatch(ansi, /Enter: send/);
  assert.doesNotMatch(ansi, /system|assistant|user/);

  const rich = withTerminalSize(72, 20, () => render(state, richCapabilities()));
  assertFitsTerminal(rich, 72, 20);
  assert.match(rich, /─── .*Session Settings.* ─────────/);
  const richLines = rich.split("\n");
  const richTitleIndex = richLines.findIndex((line) => {
    const text = stripAnsi(line);
    return text.includes("───") && text.includes("Session Settings");
  });
  assert.ok(richTitleIndex >= 0);
  assert.doesNotMatch(richLines[richTitleIndex - 1] ?? "", /\x1b\[48;2;20;23;24m/);
  assert.doesNotMatch(stripAnsi(rich), /^─{8,}$/mu);
  assert.match(rich, /\x1b\[48;2;20;23;24m/);
  assert.match(
    rich,
    /^\x1b\[48;2;20;23;24m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;20;23;24m .*Session Settings/m,
  );
  assert.match(rich, /\x1b\[38;2;64;224;208m> Model\s+\x1b\[0m.*gpt-5\.5/);
  assert.match(stripAnsi(rich), /Show commands by default\s+false/);
  const richLanguageLine = richLines.find((line) => stripAnsi(line).includes("Language"));
  assert.ok(richLanguageLine);
  assert.match(stripAnsi(richLanguageLine), /Language\s+en/);
  const richSettingModelLine = richLines.find((line) => stripAnsi(line).includes("Model"));
  assert.ok(richSettingModelLine);
  assert.match(stripAnsi(richSettingModelLine), /Model/);
  assertWideMenuGap(richSettingModelLine, "Model", "gpt-5.5", 12);
  assert.doesNotMatch(rich, /\/config get|\/config set|\/model provider\/model/);
  assert.doesNotMatch(rich, /\/model <provider\/model>|\/commands|Enter: send/);
  assertOpencodePalette(rich);
});

test("settings provider menus show status and auth actions without replacing descriptions", () => {
  const session = {
    id: "sess-provider-settings",
    title: "Provider Settings",
    status: "idle" as const,
  };
  const base = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: {
      all: [
        {
          id: "openai",
          name: "OpenAI",
          source: "builtin",
          env: ["OPENAI_API_KEY"],
          options: { domains: ["llm"], capabilities: ["llm.chat"] },
          models: { "gpt-5": { id: "gpt-5", name: "GPT-5" } },
        },
        {
          id: "anthropic",
          name: "Anthropic",
          source: "builtin",
          options: { domains: ["llm"] },
          models: { claude: { id: "claude", name: "Claude" } },
        },
        {
          id: "airtable",
          name: "Airtable",
          source: "builtin",
          options: { domains: ["productivity"] },
          models: {},
        },
      ],
      default: {},
      connected: ["openai"],
      enums: providerEnums,
    },
    authMethods: {
      openai: [
        {
          type: "oauth",
          kind: "browser",
          login: "oauth",
          label: "Browser OAuth",
          available: true,
          supports_refresh: true,
        },
        {
          type: "api_key",
          kind: "key",
          login: "api-key",
          label: "API key",
          token_env: "OPENAI_API_KEY",
          docs_url: "https://docs.example.test/openai",
          available: true,
          supports_refresh: false,
        },
      ],
    },
    authStatuses: {
      openai: { configured: true, authenticated: true, auth_state: "authenticated" },
      anthropic: { configured: false, authenticated: false, auth_state: "missing" },
    },
    sessions: [session],
    sessionConfig: {
      model: "openai/gpt-5",
      active_provider: "openai",
      active_agent: "build",
    },
  });

  const root = reducer(base, { type: "toggle-settings" });
  const rootOutput = withTerminalSize(82, 24, () => render(root, richCapabilities()));
  assertLineWidths(rootOutput, 82);
  assert.match(stripAnsi(rootOutput), /Provider\s+\(1\/2\) configured/);
  assert.doesNotMatch(stripAnsi(rootOutput), /Provider\s+openai/);

  const providerList = reducer(root, { type: "open-setting-detail", detail: "provider" });
  const providerOutput = withTerminalSize(92, 24, () => render(providerList, richCapabilities()));
  assertLineWidths(providerOutput, 92);
  assert.match(stripAnsi(providerOutput), /openai \(authenticated\) ✓\s+OpenAI builtin/);
  assert.match(stripAnsi(providerOutput), /Auth:oauth\/api_key/);
  assert.match(stripAnsi(providerOutput), /anthropic \(missing\)\s+Anthropic\s+builtin/);
  assert.doesNotMatch(stripAnsi(providerOutput), /airtable/i);
  assert.doesNotMatch(stripAnsi(providerOutput), /openai \(authenticated\)\s+current/);

  const providerAuth = reducer(providerList, {
    type: "open-setting-detail",
    detail: "providerAuth",
    providerID: "openai",
  });
  const authOutput = withTerminalSize(92, 24, () => render(providerAuth, richCapabilities()));
  assertLineWidths(authOutput, 92);
  assert.match(stripAnsi(authOutput), /Session Settings \/ Provider \/ openai/);
  assert.match(stripAnsi(authOutput), /OAuth login: Browser OAuth/);
  assert.match(stripAnsi(authOutput), /API key\s+key\s+env:OPENAI_API_KEY/);
  assert.match(stripAnsi(authOutput), /Log out\s+authenticated/);
});

test("settings detail pagination names the active setting page", () => {
  const session = {
    id: "sess-setting-page-name",
    name: "Setting Page Name",
    directory: "C:/repo",
    created_at: 1,
    updated_at: 1,
    model: "openai/gpt-5",
    agent: "build",
    session_type: "coding",
    auto_session_name: false,
    kill_processes_on_start: false,
    validator_enabled: false,
    force_planning: false,
    model_acceleration_enabled: true,
    disable_permission_restrictions: false,
    status: "idle" as const,
    message_count: 0,
  };
  const base = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: {
      all: [
        {
          id: "openai",
          name: "OpenAI",
          source: "builtin",
          options: { domains: ["llm"] },
          models: {
            "gpt-5": { id: "gpt-5", name: "GPT-5" },
            "gpt-5-mini": { id: "gpt-5-mini", name: "GPT-5 mini" },
            "gpt-5-nano": { id: "gpt-5-nano", name: "GPT-5 nano" },
            "gpt-5-pro": { id: "gpt-5-pro", name: "GPT-5 pro" },
          },
        },
      ],
      default: {},
      connected: ["openai"],
      enums: providerEnums,
    },
    sessions: [session],
    sessionConfig: {
      model: "openai/gpt-5",
      active_provider: "openai",
      active_agent: "build",
    },
  });
  const state = reducer(reducer(base, { type: "toggle-settings" }), {
    type: "open-setting-detail",
    detail: "model",
  });

  const output = withTerminalSize(82, 8, () => render(state, richCapabilities()));

  assert.match(stripAnsi(output), /Model setting page 1\/4/);
});

test("help renders as a system dialogue instead of a separate command panel", () => {
  const session = { id: "sess-help", title: "Help Session", status: "idle" as const };
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session,
      messages: [],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
    }),
    { type: "toggle-help" },
  );

  const plain = withTerminalSize(52, 26, () => render(state, plainCapabilities()));
  assertLineWidths(plain, 52);
  assert.match(plain, /--- Help ---------/);
  assert.match(plain, /^\s{2}\/chat/m);
  assert.doesNotMatch(plain, /(?:^|\n)system(?:\n|$)/);
  assert.match(plain, /\/settings\s+show session config/);
  const wrappedWordPattern = new RegExp(
    ["ret\\n\\s+urn", "deta\\n\\s+ils", "lo\\n\\s+gin"].join("|"),
  );
  assert.doesNotMatch(plain, wrappedWordPattern);
  assert.doesNotMatch(plain, /[▏─┌┐└┘├┤┬┴┼]/u);

  const ansi = withTerminalSize(100, 30, () => render(state, ansiCapabilities()));
  assertFitsTerminal(ansi, 100, 30);
  assert.match(ansi, /─── .*Help.* ─────────/);
  const ansiLines = ansi.split("\n");
  const ansiTitleIndex = ansiLines.findIndex((line) => {
    const text = stripAnsi(line);
    return text.includes("───") && text.includes("Help");
  });
  assert.ok(ansiTitleIndex >= 0);
  assert.doesNotMatch(ansiLines[ansiTitleIndex - 1] ?? "", /\x1b\[48;2;20;23;24m/);
  assert.doesNotMatch(stripAnsi(ansi), /^─{8,}$/mu);
  assert.doesNotMatch(ansi, /◇.*system/u);
  assert.doesNotMatch(ansi, /system/);
  assert.match(ansi, /\/commands/);
  assertOpencodePalette(ansi);

  const rich = withTerminalSize(100, 30, () => render(state, richCapabilities()));
  assertFitsTerminal(rich, 100, 30);
  assert.match(rich, /Help/);
  const richLines = rich.split("\n");
  const richTitleIndex = richLines.findIndex((line) => {
    const text = stripAnsi(line);
    return text.includes("───") && text.includes("Help");
  });
  assert.ok(richTitleIndex >= 0);
  assert.doesNotMatch(richLines[richTitleIndex - 1] ?? "", /\x1b\[48;2;20;23;24m/);
  assert.match(
    rich,
    /^\x1b\[48;2;20;23;24m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;20;23;24m .*Help/m,
  );
  assert.match(rich, /^\x1b\[48;2;20;23;24m\x1b\[38;2;103;116;111m▏\x1b\[0m.*\/chat/m);
  assert.doesNotMatch(rich, /system/);
  assert.match(rich, /\x1b\[38;2;64;224;208m\/chat/);
  assert.match(rich, /\x1b\[38;2;103;116;111mclose panels/);
  assert.match(stripAnsi(rich), /\/config set KEY=VALUE/);
  assert.doesNotMatch(stripAnsi(rich), /\/config set KEY=VALUE\.\.\./);
  const richHelpModelLine = richLines.find((line) =>
    stripAnsi(line).includes("/model <provider/model>"),
  );
  assert.ok(richHelpModelLine);
  assertWideMenuGap(richHelpModelLine, "/model <provider/model>", "set current");
  assert.doesNotMatch(rich, /[┌├└].*system/u);
  assertOpencodePalette(rich);
});

test("rich opencode rails and composer size themselves to terminal columns", () => {
  const session = { id: "sess-width", title: "Width", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-width-user",
        sessionID: "sess-width",
        role: "user",
        parts: [{ id: "part-width-user", type: "text", text: "Keep borders aligned." }],
      },
      {
        id: "msg-width-assistant",
        sessionID: "sess-width",
        role: "assistant",
        parts: [
          {
            id: "part-width-assistant",
            type: "text",
            text: "The frame should use exactly the current terminal width.",
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  for (const cols of [40, 52, 80, 132]) {
    const output = withTerminalSize(cols, 24, () => render(state, richCapabilities()));
    assertLineWidths(output, cols);
    const railLines = output
      .split("\n")
      .filter((line) => /\x1b\[38;2;(?:244;247;235|64;224;208)m▏\x1b\[0m/.test(line));
    assert.ok(railLines.length >= 5, `expected rich rail and composer lines at ${cols} cols`);
    assert.doesNotMatch(output, /[┌┐└┘├┤]/u);
    for (const line of railLines) assert.ok(visibleTextWidth(line) <= cols);

    const lines = output.split("\n");
    const assistantIndex = lines.findIndex((line) =>
      stripAnsi(line).includes("The frame should use exactly"),
    );
    assert.ok(assistantIndex > 0, `missing assistant bubble line at ${cols} cols`);
    assert.match(
      lines[assistantIndex],
      /\x1b\[K/u,
      `assistant highlight should erase with background to the full terminal width at ${cols} cols`,
    );
    assert.match(
      lines[assistantIndex - 1],
      /\x1b\[K/u,
      `assistant top padding should erase with background to the full terminal width at ${cols} cols`,
    );
    assert.match(
      lines[assistantIndex + 1],
      /\x1b\[K/u,
      `assistant bottom padding should erase with background to the full terminal width at ${cols} cols`,
    );
  }
});

test("render keeps a full assistant list visible alongside command details", () => {
  const session = { id: "sess-compact", title: "Compact", status: "idle" as const };
  const text = Array.from(
    { length: 16 },
    (_item, index) => `- visible-policy-line-${index + 1}`,
  ).join("\n");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-compact",
        sessionID: "sess-compact",
        role: "assistant",
        parts: [
          { id: "part-compact", type: "text", text },
          {
            id: "tool-compact",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: "npm test" },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  // A tall terminal must show every list item (the bug capped assistant text at
  // 8 lines, so long lists were silently cut off) while the command section
  // remains visible beneath it.
  const expanded = withTerminalSize(100, 40, () => render(state, richCapabilities()));
  assert.match(expanded, /visible-policy-line-8/);
  assert.match(expanded, /visible-policy-line-9/);
  assert.match(expanded, /visible-policy-line-16/);
  assert.doesNotMatch(expanded, /earlier output hidden|earlier output hidden/u);
  assert.match(expanded, /◇ Commands/);
  assert.match(expanded, /\$ npm test/);
  const commandLine = expanded.split("\n").find((line) => stripAnsi(line).includes("Commands"));
  assert.ok(commandLine);
  assert.equal(stripAnsi(commandLine), "◇ Commands");
  assert.doesNotMatch(commandLine, /\x1b\[48;2;20;23;24m/);
  const npmTestLine = expanded.split("\n").find((line) => stripAnsi(line).includes("$ npm test"));
  assert.ok(npmTestLine);
  assert.match(stripAnsi(npmTestLine), /^└─ ✓ #1 shell_command completed\s+\$ npm test/u);
  assert.doesNotMatch(npmTestLine, /\x1b\[48;2;20;23;24m/);

  const collapsed = render(
    reducer(state, {
      type: "session-config",
      value: { show_command_instructions: false },
    }),
    richCapabilities(),
  );
  assert.doesNotMatch(collapsed, /◇ Commands/);
  assert.doesNotMatch(collapsed, /\$ npm test/);
});
