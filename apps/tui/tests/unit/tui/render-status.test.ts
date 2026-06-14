import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render, renderFrame } from "../../../src/tui/render.js";
import {
  ansiCapabilities,
  plainCapabilities,
  richCapabilities,
} from "../../../src/tui/capabilities.js";
import { stripAnsi } from "../../../src/tui/render-terminal.js";
import { providerEnums, assertWideMenuGap } from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

test("render reports composer cursor without drawing an inline fake cursor", () => {
  const session = { id: "sess-cursor", title: "Cursor", status: "idle" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });
  const rendered = renderFrame(state, richCapabilities());
  for (let index = 0; index < 5; index += 1) state = reducer(state, { type: "tick" });
  const afterTicks = renderFrame(state, richCapabilities());

  assert.doesNotMatch(rendered.frame, /\x1b\[38;2;64;224;208m█\x1b\[0m/);
  assert.doesNotMatch(rendered.frame, /TURA_COMPOSER_CURSOR/);
  assert.match(stripAnsi(rendered.frame), /> ?Enter to send/u);
  assert.equal(rendered.frame, afterTicks.frame);
  assert.deepEqual(rendered.cursor, afterTicks.cursor);
});

test("render hides composer cursor outside input surfaces", () => {
  const session = { id: "sess-no-page-cursor", title: "No Page Cursor", status: "idle" as const };
  const base = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  for (const state of [
    reducer(base, { type: "toggle-help" }),
    reducer(base, { type: "toggle-sessions" }),
    reducer(base, { type: "toggle-auth" }),
    reducer(base, { type: "toggle-settings" }),
    reducer(base, { type: "toggle-personas" }),
    reducer(base, { type: "toggle-models" }),
  ]) {
    const rendered = renderFrame(state, richCapabilities());
    assert.equal(rendered.cursor, undefined);
    assert.doesNotMatch(stripAnsi(rendered.frame), /> ?Enter to send/u);
  }
});

test("render bottom meta sums current gateway token usage", () => {
  const session = {
    id: "sess-token-usage",
    title: "Token Usage",
    status: "idle" as const,
    model: "codex/gpt-5.5",
    model_variant: "low",
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-token-1",
        sessionID: "sess-token-usage",
        role: "assistant",
        tokens: { input: 11, output: 7, reasoning: 3, cache: { read: 5, write: 2 } },
        parts: [{ id: "part-token-1", type: "text", text: "Ready." }],
      },
      {
        id: "msg-token-2",
        sessionID: "sess-token-usage",
        role: "assistant",
        tokens: { prompt_tokens: 13, completion_tokens: 17, cached_input_tokens: 19 },
        parts: [{ id: "part-token-2", type: "text", text: "Done." }],
      },
      {
        id: "msg-token-3",
        sessionID: "sess-token-usage",
        role: "assistant",
        tokens: { total_tokens: 23 },
        parts: [{ id: "part-token-3", type: "text", text: "Final." }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const expected = /tokens 100/;
  assert.match(render(state, plainCapabilities()), expected);
  const ansi = render(state, ansiCapabilities());
  assert.match(ansi, expected);
  const rich = render(state, richCapabilities());
  assert.match(rich, expected);
  assert.match(rich, /\x1b\[38;2;103;116;111mtokens 100/);
  const ansiMeta = ansi.split("\n").find((line) => stripAnsi(line).includes("tokens 100")) ?? "";
  const richMeta = rich.split("\n").find((line) => stripAnsi(line).includes("tokens 100")) ?? "";
  assert.equal(stripAnsi(ansiMeta), "○ │ codex/gpt-5.5 low │ tokens 100");
  assert.equal(stripAnsi(richMeta), stripAnsi(ansiMeta));
  assert.match(ansiMeta, /\x1b\[38;2;103;116;111m/);
  assert.match(richMeta, /\x1b\[38;2;103;116;111m/);
  assert.doesNotMatch(ansiMeta, /\x1b\[48;2;24;27;28m/);
  assert.doesNotMatch(richMeta, /\x1b\[48;2;24;27;28m/);
});

test("render bottom meta displays provider/model from active runtime config", () => {
  const session = {
    id: "sess-active-model-meta",
    title: "Active Model Meta",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: {
      model: "flagship_thinking",
      active_provider: "openai",
      active_model: "gpt-5.5",
      model_variant: "high",
      model_acceleration_enabled: true,
    },
  });

  const output = render(state, richCapabilities());
  const meta = output.split("\n").find((line) => stripAnsi(line).includes("tokens -")) ?? "";
  assert.equal(stripAnsi(meta), "○ │ openai/gpt-5.5 high priority │ tokens -");
  assert.doesNotMatch(stripAnsi(meta), /flagship_thinking/);
});

test("render bottom meta uses one busy animation before the model", () => {
  const session = {
    id: "sess-busy-model-meta",
    title: "Busy Model Meta",
    status: "busy" as const,
  };
  const base = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: {
      model: "codex/gpt-5.5",
      model_variant: "medium",
    },
  });

  const frameOne = render({ ...base, thinkingFrame: 0 }, richCapabilities());
  const frameTwo = render({ ...base, thinkingFrame: 1 }, richCapabilities());
  const frameThree = render({ ...base, thinkingFrame: 2 }, richCapabilities());
  const frameFour = render({ ...base, thinkingFrame: 3 }, richCapabilities());

  const metaOne = stripAnsi(frameOne)
    .split("\n")
    .find((line) => line.includes("tokens -"));
  const metaTwo = stripAnsi(frameTwo)
    .split("\n")
    .find((line) => line.includes("tokens -"));
  const metaThree = stripAnsi(frameThree)
    .split("\n")
    .find((line) => line.includes("tokens -"));
  const metaFour = stripAnsi(frameFour)
    .split("\n")
    .find((line) => line.includes("tokens -"));

  assert.equal(metaOne, "◇ │ codex/gpt-5.5 medium │ tokens -");
  assert.equal(metaTwo, "◆ │ codex/gpt-5.5 medium │ tokens -");
  assert.equal(metaThree, "◈ │ codex/gpt-5.5 medium │ tokens -");
  assert.equal(metaFour, "◆ │ codex/gpt-5.5 medium │ tokens -");
});

test("render keeps thinking visible while the current session is busy", () => {
  const session = {
    id: "sess-session-busy-thinking",
    title: "Session Busy Thinking",
    status: "busy" as const,
  };
  const hydrated = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: {
      model: "codex/gpt-5.5",
      model_variant: "medium",
    },
  });
  const state = reducer(hydrated, { type: "status", value: "idle" });

  const output = stripAnsi(render({ ...state, thinkingFrame: 0 }, richCapabilities()));

  assert.match(output, /thinking\s+0s/);
  assert.match(output, /^◇ │ codex\/gpt-5\.5 medium │ tokens -$/mu);
});

test("render keeps thinking visible when the active session list entry is still busy", () => {
  const idleSession = {
    id: "sess-list-busy-thinking",
    title: "List Busy Thinking",
    status: "idle" as const,
  };
  const busySession = { ...idleSession, status: "busy" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: idleSession,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [busySession],
    sessionConfig: {
      model: "codex/gpt-5.5",
      model_variant: "medium",
    },
  });

  const output = stripAnsi(
    render({ ...state, status: "idle", thinkingFrame: 0 }, richCapabilities()),
  );

  assert.match(output, /thinking\s+0s/);
  assert.match(output, /^◇ │ codex\/gpt-5\.5 medium │ tokens -$/mu);
});

test("render keeps thinking visible across an idle hydrate while the user turn is pending", () => {
  const busySession = {
    id: "sess-pending-user-thinking",
    title: "Pending User Thinking",
    status: "busy" as const,
  };
  const idleSession = { ...busySession, status: "idle" as const };
  const userMessage = {
    id: "msg-pending-user-thinking",
    sessionID: busySession.id,
    role: "user" as const,
    created_at: 1_000,
    parts: [{ id: "part-pending-user-thinking", type: "text", text: "keep thinking" }],
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [userMessage],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [busySession],
    sessionConfig: {
      model: "codex/gpt-5.5",
      model_variant: "medium",
    },
  });

  state = reducer(state, {
    type: "hydrate",
    session: idleSession,
    messages: [userMessage],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [idleSession],
    sessionConfig: {
      model: "codex/gpt-5.5",
      model_variant: "medium",
    },
  });

  const pendingOutput = stripAnsi(render({ ...state, thinkingFrame: 0 }, richCapabilities()));

  assert.match(pendingOutput, /thinking\s+\d+s/);
  assert.match(pendingOutput, /^◇ │ codex\/gpt-5\.5 medium │ tokens -$/mu);

  const completed = reducer(state, {
    type: "hydrate",
    session: idleSession,
    messages: [
      userMessage,
      {
        id: "msg-pending-agent-thinking",
        sessionID: busySession.id,
        role: "assistant" as const,
        created_at: 1_500,
        parts: [{ id: "part-pending-agent-thinking", type: "text", text: "done" }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [idleSession],
    sessionConfig: {
      model: "codex/gpt-5.5",
      model_variant: "medium",
    },
  });

  const completedOutput = stripAnsi(render({ ...completed, thinkingFrame: 0 }, richCapabilities()));

  assert.doesNotMatch(completedOutput, /thinking\s+\d+s/);
  assert.match(completedOutput, /^○ │ codex\/gpt-5\.5 medium │ tokens -$/mu);
});

test("render keeps model and auth tables readable across display levels", () => {
  const session = { id: "sess-tables", title: "Tables", status: "idle" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: {
      all: [
        {
          id: "openai",
          name: "OpenAI",
          source: "system",
          models: {
            "gpt-5.5": { id: "gpt-5.5", name: "gpt-5.5" },
            "o5-mini": { id: "o5-mini", name: "o5-mini" },
          },
        },
      ],
      default: { openai: "gpt-5.5" },
      connected: ["openai"],
      enums: providerEnums,
    },
    authMethods: {
      openai: [
        {
          type: "oauth",
          login: "browser",
          label: "Browser login",
          available: true,
          supports_refresh: false,
        },
      ],
    },
    authStatuses: {
      openai: { authenticated: true, login: "browser", account_id: "acct-1" },
    },
    sessions: [session],
  });

  state = reducer(state, { type: "toggle-models" });
  for (const capabilities of [plainCapabilities(), ansiCapabilities(), richCapabilities()]) {
    const output = render(state, capabilities);
    assert.match(output, /openai\/gpt-5\.5/);
    assert.match(output, /openai\/o5-mini/);
    assert.match(output, /OpenAI/);
    if (capabilities.level === "rich") {
      const modelLine = output
        .split("\n")
        .find((line) => stripAnsi(line).includes("openai/gpt-5.5"));
      assert.ok(modelLine);
      assertWideMenuGap(modelLine, "openai/gpt-5.5", "OpenAI");
    }
    if (capabilities.level === "plain") assert.doesNotMatch(output, /\x1b|▏|─/u);
    if (capabilities.level === "ansi") {
      assert.doesNotMatch(output, /\x1b\]8/u);
      assert.doesNotMatch(stripAnsi(output), /^─{8,}$/mu);
    }
  }

  state = reducer(state, { type: "toggle-models" });
  state = reducer(state, { type: "toggle-auth" });
  for (const capabilities of [plainCapabilities(), ansiCapabilities(), richCapabilities()]) {
    const output = render(state, capabilities);
    assert.match(output, /openai/);
    assert.match(output, /OpenAI/);
    assert.match(output, /Browser login/);
    assert.match(output, /acct-1/);
    if (capabilities.level === "rich") {
      const authLine = output.split("\n").find((line) => stripAnsi(line).includes("openai"));
      assert.ok(authLine);
      assertWideMenuGap(authLine, "openai", "OpenAI");
    }
    if (capabilities.level === "plain") assert.doesNotMatch(output, /\x1b|▏|─/u);
    if (capabilities.level === "ansi") {
      assert.doesNotMatch(output, /\x1b\]8/u);
      assert.doesNotMatch(stripAnsi(output), /^─{8,}$/mu);
    }
  }
});
