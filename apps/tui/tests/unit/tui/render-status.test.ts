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
import type {
  Message,
  MessagePart,
  Session,
  SessionStatusValue,
} from "../../../src/types/session.js";

process.env.TURA_LANG = "en";

type TestSession = Session & { title: string };

function sessionFixture(
  id: string,
  title: string,
  status: SessionStatusValue = "idle",
  overrides: Partial<Session> = {},
): TestSession {
  return {
    id,
    title,
    name: title,
    parent_id: null,
    created_at: 1_000,
    updated_at: 1_000,
    directory: "C:/repo",
    model: null,
    agent: null,
    session_type: null,
    auto_session_name: true,
    kill_processes_on_start: false,
    validator_enabled: false,
    force_planning: false,
    model_variant: null,
    model_acceleration_enabled: false,
    disable_permission_restrictions: false,
    status,
    message_count: 0,
    task_management: null,
    context_tokens: null,
    plan_summary: null,
    session_display_name: title,
    ...overrides,
  };
}

function textPart(sessionID: string, messageID: string, id: string, text: string): MessagePart {
  return { id, sessionID, messageID, type: "text", text };
}

function textMessage(
  id: string,
  sessionID: string,
  role: Message["role"],
  createdAt: number,
  text: string,
  tokens?: unknown,
): Message {
  return {
    id,
    sessionID,
    role,
    created_at: createdAt,
    updated_at: createdAt,
    time: { created: createdAt, updated: createdAt },
    tokens,
    parts: [textPart(sessionID, id, `${id}-part`, text)],
  };
}

test("render reports composer cursor without drawing an inline fake cursor", () => {
  const session = sessionFixture("sess-cursor", "Cursor");
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
  assert.match(stripAnsi(rendered.frame), /> ?Enter: send/u);
  assert.equal(rendered.frame, afterTicks.frame);
  assert.deepEqual(rendered.cursor, afterTicks.cursor);
});

test("render hides composer cursor outside input surfaces", () => {
  const session = sessionFixture("sess-no-page-cursor", "No Page Cursor");
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
    assert.doesNotMatch(stripAnsi(rendered.frame), /> ?Enter: send/u);
  }
});

test("render bottom meta displays current gateway context usage", () => {
  const session = sessionFixture("sess-token-usage", "Token Usage", "idle", {
    model: "codex/gpt-5.5",
    model_variant: "low",
    context_tokens: { input: 90_000, limit: 200_000 },
  });
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage("msg-token-1", "sess-token-usage", "assistant", 1_000, "Ready.", {
        input: 11,
        output: 7,
        reasoning: 3,
        cache: { read: 5, write: 2 },
      }),
      textMessage("msg-token-2", "sess-token-usage", "assistant", 1_100, "Done.", {
        prompt_tokens: 13,
        completion_tokens: 17,
        cached_input_tokens: 19,
      }),
      textMessage("msg-token-3", "sess-token-usage", "assistant", 1_200, "Final.", {
        total_tokens: 23,
      }),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const expected = /context 90k\/200k ██▓░░░/u;
  assert.match(render(state, plainCapabilities()), expected);
  const ansi = render(state, ansiCapabilities());
  assert.match(ansi, expected);
  const rich = render(state, richCapabilities());
  assert.match(rich, expected);
  assert.doesNotMatch(rich, /tokens 100/);
  assert.match(rich, /\x1b\[38;2;103;116;111mcontext 90k\/200k/);
  const ansiMeta =
    ansi.split("\n").find((line) => stripAnsi(line).includes("context 90k/200k")) ?? "";
  const richMeta =
    rich.split("\n").find((line) => stripAnsi(line).includes("context 90k/200k")) ?? "";
  assert.equal(stripAnsi(ansiMeta), "codex/gpt-5.5 │ low │ tura │ context 90k/200k ██▓░░░");
  assert.equal(stripAnsi(richMeta), stripAnsi(ansiMeta));
  assert.match(ansiMeta, /\x1b\[38;2;103;116;111m/);
  assert.match(richMeta, /\x1b\[38;2;103;116;111m/);
  assert.doesNotMatch(ansiMeta, /\x1b\[48;2;24;27;28m/);
  assert.doesNotMatch(richMeta, /\x1b\[48;2;24;27;28m/);
});

test("render bottom meta displays provider/model from active runtime config", () => {
  const session = sessionFixture("sess-active-model-meta", "Active Model Meta", "idle", {
    model_acceleration_enabled: true,
  });
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
  const meta = output.split("\n").find((line) => stripAnsi(line).includes("openai/gpt-5.5")) ?? "";
  assert.equal(stripAnsi(meta), "openai/gpt-5.5 │ high │ priority │ tura");
  assert.doesNotMatch(stripAnsi(meta), /flagship_thinking/);
});

test("render bottom meta context bar uses partial block thresholds", () => {
  const cases = [
    { input: 0, limit: 200_000, bar: "░░░░░░" },
    { input: 10_000, limit: 200_000, bar: "▒░░░░░" },
    { input: 90_000, limit: 200_000, bar: "██▓░░░" },
    { input: 100_000, limit: 200_000, bar: "███░░░" },
    { input: 200_000, limit: 200_000, bar: "██████" },
  ];

  for (const item of cases) {
    const session = sessionFixture(`sess-context-${item.input}`, "Context Bar", "idle", {
      model: "codex/gpt-5.5",
      context_tokens: { input: item.input, limit: item.limit },
    });
    const state = reducer(initialState("C:/repo"), {
      type: "hydrate",
      session,
      messages: [],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
    });

    assert.match(
      stripAnsi(render(state, richCapabilities())),
      new RegExp(`context ${item.input ? `${item.input / 1000}k` : "0"}/200k ${item.bar}`, "u"),
    );
  }
});

test("session status event updates bottom meta context without rehydrating", () => {
  const session = sessionFixture("sess-context-event", "Context Event", "busy", {
    model: "codex/gpt-5.5",
    context_tokens: { input: 0, limit: 200_000 },
    usage: {
      context_tokens: { input: 0, limit: 200_000 },
      tokens: null,
      cost: null,
      currency: null,
    },
  });
  const hydrated = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  assert.match(stripAnsi(render(hydrated, richCapabilities())), /context 0\/200k ░░░░░░/u);

  const updated = reducer(hydrated, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "session.status",
        properties: {
          sessionID: session.id,
          status: { type: "busy" },
          context_tokens: { input: 1, limit: 1 },
          usage: {
            context_tokens: { input: 90_000, limit: 200_000 },
            tokens: { total_tokens: 123, input_tokens: 100, output_tokens: 23 },
            cost: 0.045,
            currency: "USD",
          },
        },
      },
    },
  });

  assert.match(stripAnsi(render(updated, richCapabilities())), /context 90k\/200k ██▓░░░/u);
  assert.equal(updated.session?.context_tokens?.input, 90_000);
  assert.equal(updated.sessions[0]?.context_tokens?.input, 90_000);
  assert.equal(updated.session?.usage?.cost, 0.045);
  assert.equal((updated.session?.usage?.tokens as { total_tokens?: number })?.total_tokens, 123);
  assert.equal(updated.sessions[0]?.usage?.currency, "USD");
});

test("hydrate restores session usage for reselected sessions", () => {
  const session = sessionFixture("sess-context-restore", "Context Restore", "idle", {
    context_tokens: { input: 60_000, limit: 180_000 },
    usage: {
      context_tokens: { input: 60_000, limit: 180_000 },
      tokens: { total_tokens: 321 },
      cost: 0.067,
      currency: "USD",
    },
  });

  const restored = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  assert.match(stripAnsi(render(restored, richCapabilities())), /context 60k\/180k ██░░░░/u);
  assert.equal(restored.session?.usage?.cost, 0.067);
  assert.equal(
    (restored.sessions[0]?.usage?.tokens as { total_tokens?: number })?.total_tokens,
    321,
  );
});

test("render bottom meta uses one busy animation before the model", () => {
  const session = sessionFixture("sess-busy-model-meta", "Busy Model Meta", "busy");
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
    .find((line) => line.includes("codex/gpt-5.5"));
  const metaTwo = stripAnsi(frameTwo)
    .split("\n")
    .find((line) => line.includes("codex/gpt-5.5"));
  const metaThree = stripAnsi(frameThree)
    .split("\n")
    .find((line) => line.includes("codex/gpt-5.5"));
  const metaFour = stripAnsi(frameFour)
    .split("\n")
    .find((line) => line.includes("codex/gpt-5.5"));

  assert.equal(metaOne, "codex/gpt-5.5 │ medium │ tura");
  assert.equal(metaTwo, "codex/gpt-5.5 │ medium │ tura");
  assert.equal(metaThree, "codex/gpt-5.5 │ medium │ tura");
  assert.equal(metaFour, "codex/gpt-5.5 │ medium │ tura");
});

test("render keeps thinking visible while the current session is busy", () => {
  const session = sessionFixture("sess-session-busy-thinking", "Session Busy Thinking", "busy");
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
  assert.match(output, /^codex\/gpt-5\.5 │ medium │ tura$/mu);
});

test("render keeps thinking visible when the active session list entry is still busy", () => {
  const idleSession = sessionFixture("sess-list-busy-thinking", "List Busy Thinking");
  const busySession = sessionFixture("sess-list-busy-thinking", "List Busy Thinking", "busy");
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
  assert.match(output, /^codex\/gpt-5\.5 │ medium │ tura$/mu);
});

test("render keeps thinking visible across an idle hydrate while the user turn is pending", () => {
  const busySession = sessionFixture("sess-pending-user-thinking", "Pending User Thinking", "busy");
  const idleSession = sessionFixture("sess-pending-user-thinking", "Pending User Thinking");
  const userMessage = textMessage(
    "msg-pending-user-thinking",
    busySession.id,
    "user",
    1_000,
    "keep thinking",
  );
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
  assert.match(pendingOutput, /^codex\/gpt-5\.5 │ medium │ tura$/mu);

  const completed = reducer(state, {
    type: "hydrate",
    session: idleSession,
    messages: [
      userMessage,
      textMessage("msg-pending-agent-thinking", busySession.id, "assistant", 1_500, "done"),
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
  assert.match(completedOutput, /^codex\/gpt-5\.5 │ medium │ tura$/mu);
});

test("render keeps model and auth tables readable across display levels", () => {
  const session = sessionFixture("sess-tables", "Tables");
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
