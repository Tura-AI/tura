import assert from "node:assert/strict";
import test from "node:test";
import { assertDictionaryParity, setLanguage, t } from "../i18n.js";
import { initialState, reducer } from "./reducer.js";
import { render } from "./render.js";
import { ansiCapabilities, plainCapabilities, richCapabilities } from "./capabilities.js";

process.env.TURA_LANG = "en";

const providerEnums = {
  domains: [],
  capabilities: [],
  api_styles: [],
  auth_methods: [],
  statuses: [],
};

test("TUI i18n dictionaries keep zh-CN and en keys in sync", () => {
  assert.doesNotThrow(() => assertDictionaryParity());
});

test("TUI language selection reads external locale files", () => {
  setLanguage("zh-CN");
  assert.equal(t("assistant"), "助手");
  setLanguage("en");
  assert.equal(t("assistant"), "assistant");
  setLanguage(undefined);
});

test("render includes core TUI panels without throwing", () => {
  const session = {
    id: "sess-1",
    name: "Work",
    directory: "C:/repo",
    status: "idle" as const,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-1",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          { id: "part-1", type: "text", text: "Ready" },
          { id: "tool-1", type: "tool", tool: "runtime", state: { status: "completed", output: { text: "checked" } } },
        ],
      },
      {
        id: "msg-2",
        sessionID: "sess-1",
        role: "user",
        parts: [{ id: "part-2", type: "text", text: "Please continue" }],
      },
      {
        id: "msg-3",
        sessionID: "sess-1",
        role: "system",
        parts: [{ id: "part-3", type: "text", text: "System ready" }],
      },
    ],
    permissions: [{ id: "perm-1", sessionID: "sess-1", permission: "shell" }],
    providers: {
      all: [{ id: "openai", name: "OpenAI", models: { "gpt-5.5": { id: "gpt-5.5", name: "gpt-5.5" } } }],
      default: { openai: "gpt-5.5" },
      connected: ["openai"],
      enums: providerEnums,
    },
    sessions: [session],
  });
  state = reducer(state, { type: "questions", value: [{ id: "q-1", sessionID: "sess-1", question: "Proceed?" }] });

  const transcript = render(state, richCapabilities());
  assert.match(transcript, /Tura/);
  assert.match(transcript, /assistant/);
  assert.match(transcript, /user/);
  assert.match(transcript, /system/);
  assert.match(transcript, /\[runtime: checked\]/);
  assert.match(transcript, /permission/);
  assert.match(transcript, /question/);

  state = reducer(state, { type: "toggle-models" });
  assert.match(render(state, richCapabilities()), /openai\/gpt-5\.5/);

  state = reducer(state, { type: "toggle-models" });
  state = reducer(state, { type: "toggle-sessions" });
  const sessions = render(state, richCapabilities());
  assert.match(sessions, /sess-1/);
  assert.match(sessions, /Work/);
});

test("render applies communication style rich text without leaking protocol markup", () => {
  const session = { id: "sess-rich", title: "Rich", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-rich",
        sessionID: "sess-rich",
        role: "assistant",
        parts: [
          {
            id: "part-rich",
            type: "text",
            text: "<b>Bold</b> <i>Italic</i> <u>Under</u> <s>Gone</s> <code>src/App.tsx:12</code>\n<a href='https://example.com'>Example</a>\n<span class='tg-spoiler'>secret</span>\n<blockquote>quoted</blockquote>\n<pre><code class='language-python'>print('hello')</code></pre>\n[MEDIA:C:/tmp/shot.png:MEDIA]\n[MEDIA:https://example.com/shot.png:MEDIA]\n[EMOJI:react:👍:EMOJI]",
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const transcript = render(state, richCapabilities());
  assert.match(transcript, /\x1b\[1mBold\x1b\[0m/);
  assert.match(transcript, /\x1b\[3mItalic\x1b\[0m/);
  assert.match(transcript, /\x1b\[4mUnder\x1b\[0m/);
  assert.match(transcript, /\x1b\[9mGone\x1b\[0m/);
  assert.match(transcript, /\x1b\[36msrc\/App\.tsx:12\x1b\[0m/);
  assert.match(transcript, /Example/);
  assert.match(transcript, /https:\/\/example\.com/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.match(transcript, /\[MEDIA:C:\/tmp\/shot\.png:MEDIA\]/);
  assert.match(transcript, /\[MEDIA:https:\/\/example\.com\/shot\.png:MEDIA\]/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\/shot\.png\x1b\\/);
  assert.match(transcript, /\[EMOJI:react:👍:EMOJI\]/u);
  assert.match(transcript, /│ quoted/);
  assert.match(transcript, /\[code: python\]/);
  assert.doesNotMatch(transcript, /<b>|<\/code>/);
});

test("render gracefully downgrades rich text across display levels", () => {
  const session = { id: "sess-rich-levels", title: "Rich Levels", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-rich-levels",
        sessionID: "sess-rich-levels",
        role: "assistant",
        parts: [
          {
            id: "part-rich-levels",
            type: "text",
            text: "<b>Bold</b> <code>src/App.tsx:12</code>\n<a href='https://example.com'>Example</a>\n<blockquote>quoted</blockquote>\n[MEDIA:https://example.com/shot.png:MEDIA]\n[EMOJI:react:👍:EMOJI]",
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = render(state, plainCapabilities());
  assert.match(plain, /Bold/);
  assert.match(plain, /src\/App\.tsx:12/);
  assert.match(plain, /Example \(https:\/\/example\.com\)/);
  assert.match(plain, /\[MEDIA:https:\/\/example\.com\/shot\.png:MEDIA\]/);
  assert.match(plain, /\[EMOJI:react:thumbs_up:EMOJI\]/);
  assert.doesNotMatch(plain, /👍/u);
  assert.doesNotMatch(plain, /\x1b|<b>|<\/code>|\x1b\]8|│/u);

  const ansi = render(state, ansiCapabilities());
  assert.match(ansi, /Bold/);
  assert.match(ansi, /Example/);
  assert.match(ansi, /https:\/\/example\.com/);
  assert.match(ansi, /\[MEDIA:https:\/\/example\.com\/shot\.png:MEDIA\]/);
  assert.match(ansi, /👍/u);
  assert.match(ansi, /\x1b\[[0-9;]*m/);
  assert.doesNotMatch(ansi, /<b>|<\/code>|\x1b\]8|│/u);

  const rich = render(state, richCapabilities());
  assert.match(rich, /\x1b\[1mBold\x1b\[0m/);
  assert.match(rich, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.match(rich, /│ quoted/);
  assert.match(rich, /\[EMOJI:react:👍:EMOJI\]/u);
  assert.doesNotMatch(rich, /<b>|<\/code>/);
});

test("render supports markdown tables, markdown links, and local path access by level", () => {
  const session = { id: "sess-md", title: "Markdown", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-md",
        sessionID: "sess-md",
        role: "assistant",
        parts: [
          {
            id: "part-md",
            type: "text",
            text: "| Item | Path |\n| --- | --- |\n| Source | C:/repo/apps/tui |\n| Docs | [README](https://example.com/readme) |",
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = render(state, plainCapabilities());
  assert.match(plain, /Item\s+Path/);
  assert.match(plain, /README \(https:\/\/example\.com\/readme\)/);
  assert.doesNotMatch(plain, /\x1b\]8/);

  const rich = render(state, richCapabilities());
  assert.match(rich, /Item\s+Path/);
  assert.match(rich, /\x1b\]8;;https:\/\/example\.com\/readme\x1b\\/);
  assert.match(rich, /\x1b\]8;;file:\/\/\/C:\/repo\/apps\/tui\x1b\\/);
});

test("render shows agent persona summary and persona panel", () => {
  const session = { id: "sess-persona", title: "Persona", status: "idle" as const, agent: "fast" };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    agents: [{
      summary: {
        id: "fast",
        name: "Fast",
        description: "fast agent",
        source: "static",
        path: "agents/src/fast",
        aliases: [],
        capabilities: ["chat"],
        hidden: false,
      },
      config: {
        agent_name: "fast",
        agent_persona: [{ persona_name: "tura", persona_directory: "personas/src/tura" }],
      },
      prompt: "Fast prompt",
    }],
    personas: [
      {
        summary: { id: "tura", source: "static", description: "calm technical collaborator", path: "personas/src/tura" },
        config: { persona_name: "tura" },
        communication_style: "concise, direct, friendly",
      },
      {
        summary: { id: "reviewer", source: "dynamic", description: "review-first mode", path: "personas/src/reviewer" },
        config: { persona_name: "reviewer" },
      },
    ],
    sessions: [session],
    sessionConfig: { active_agent: "fast" },
  });
  const top = render(state, richCapabilities());
  assert.match(top, /Agent:.*fast/);
  assert.match(top, /persona:.*tura/);

  state = reducer(state, { type: "toggle-personas" });
  const panel = render(state, richCapabilities());
  assert.match(panel, /Personas/);
  assert.match(panel, /tura/);
  assert.match(panel, /calm technical collaborator/);
  assert.match(panel, /concise, direct, friendly/);
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
      openai: [{ type: "oauth", login: "browser", label: "Browser login", available: true, supports_refresh: false }],
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
    if (capabilities.level === "plain") assert.doesNotMatch(output, /\x1b|│|─/u);
    if (capabilities.level === "ansi") assert.doesNotMatch(output, /\x1b\]8|│|─/u);
  }

  state = reducer(state, { type: "toggle-models" });
  state = reducer(state, { type: "toggle-auth" });
  for (const capabilities of [plainCapabilities(), ansiCapabilities(), richCapabilities()]) {
    const output = render(state, capabilities);
    assert.match(output, /openai/);
    assert.match(output, /OpenAI/);
    assert.match(output, /Browser login/);
    assert.match(output, /acct-1/);
    if (capabilities.level === "plain") assert.doesNotMatch(output, /\x1b|│|─/u);
    if (capabilities.level === "ansi") assert.doesNotMatch(output, /\x1b\]8|│|─/u);
  }
});

test("render applies rich text cleanup to tool summaries", () => {
  const session = { id: "sess-tool-rich", title: "Tool Rich", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-tool-rich",
        sessionID: "sess-tool-rich",
        role: "assistant",
        parts: [
          {
            id: "tool-rich",
            type: "tool",
            tool: "runtime",
            state: {
              status: "completed",
              output: {
                text: "完成：<b>Frontend</b> 验证 <code>npm run verify:all</code> [MEDIA:C:/tmp/a.png:MEDIA]",
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const transcript = render(state, richCapabilities());
  assert.match(transcript, /Frontend/);
  assert.match(transcript, /npm run verify:all/);
  assert.match(transcript, /\[MEDIA:C:\/tmp\/a\.png:MEDIA\]/);
  assert.doesNotMatch(transcript, /<b>|<\/b>|<code>|<\/code>/);
});
