import assert from "node:assert/strict";
import test from "node:test";
import { assertDictionaryParity, setLanguage, t } from "../i18n.js";
import { initialState, reducer } from "./reducer.js";
import { render } from "./render.js";
import { ansiCapabilities, plainCapabilities, richCapabilities } from "./capabilities.js";
import { stripAnsi, truncate, truncateAnsi, visibleTextWidth, wrap } from "./render-terminal.js";

process.env.TURA_LANG = "en";

const providerEnums = {
  domains: [],
  capabilities: [],
  api_styles: [],
  auth_methods: [],
  statuses: [],
};

function withTerminalSize<T>(cols: number, rows: number, fn: () => T): T {
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const stdoutRows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: cols });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: rows });
  try {
    return fn();
  } finally {
    if (columns) Object.defineProperty(process.stdout, "columns", columns);
    else Reflect.deleteProperty(process.stdout, "columns");
    if (stdoutRows) Object.defineProperty(process.stdout, "rows", stdoutRows);
    else Reflect.deleteProperty(process.stdout, "rows");
  }
}

function assertFitsTerminal(output: string, cols: number, rows: number): void {
  const lines = output.split("\n");
  assert.ok(lines.length <= rows, `expected at most ${rows} rows, got ${lines.length}`);
  for (const [index, line] of lines.entries()) {
    assert.ok(
      visibleTextWidth(line) <= cols,
      `line ${index + 1} overflows ${cols} cols: ${visibleTextWidth(line)} ${stripAnsi(line)}`,
    );
  }
}

function assertOpencodePalette(output: string): void {
  assert.doesNotMatch(output, /\x1b\[(?:3[1-6]|9[1-6])m/u);
  assert.doesNotMatch(output, /\x1b\[38;2;(?!250;178;131m|238;238;238m|128;128;128m|58;58;58m)/u);
  assert.doesNotMatch(output, /\x1b\[48;2;(?!32;32;34m|38;38;40m)/u);
}

function assertWideMenuGap(
  line: string,
  label: string,
  description: string,
  minimumGap = 20,
): void {
  const text = stripAnsi(line);
  const labelIndex = text.indexOf(label);
  const descriptionIndex = text.indexOf(description);
  assert.ok(labelIndex >= 0, `missing label ${label}: ${text}`);
  assert.ok(descriptionIndex >= 0, `missing description ${description}: ${text}`);
  const gap = descriptionIndex - labelIndex - label.length;
  assert.ok(gap >= minimumGap, `expected wide menu label gap, got ${gap}: ${text}`);
}

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

test("terminal width helpers count CJK and emoji as double-width", () => {
  assert.equal(visibleTextWidth("空闲"), 4);
  assert.equal(visibleTextWidth("ok👍"), 4);
  assert.equal(visibleTextWidth("🇨🇳"), 2);
  assert.equal(visibleTextWidth("👨‍💻"), 2);
  assert.equal(visibleTextWidth("a\u0301"), 1);
  assert.equal(truncate("空闲 ready", 8), "空闲 ...");
  assert.equal(stripAnsi(truncateAnsi("\x1b[90m空闲 ready\x1b[0m", 8)), "空闲 ...");
  assert.deepEqual(wrap("空闲".repeat(12), 22), [
    "空闲".repeat(5),
    "空闲".repeat(5),
    "空闲".repeat(2),
  ]);
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
          {
            id: "tool-1",
            type: "tool",
            tool: "runtime",
            state: { status: "completed", output: { text: "checked" } },
          },
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
      all: [
        { id: "openai", name: "OpenAI", models: { "gpt-5.5": { id: "gpt-5.5", name: "gpt-5.5" } } },
      ],
      default: { openai: "gpt-5.5" },
      connected: ["openai"],
      enums: providerEnums,
    },
    sessions: [session],
  });
  state = reducer(state, {
    type: "questions",
    value: [{ id: "q-1", sessionID: "sess-1", question: "Proceed?" }],
  });

  const transcript = render(state, richCapabilities());
  assert.match(transcript, /Work/);
  assert.match(
    transcript,
    /\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m\x1b\[48;2;32;32;34m/,
  );
  assert.doesNotMatch(transcript, /(?:assistant|user|system)/);
  assert.match(transcript, /\[runtime: checked\]/);
  assert.match(transcript, /permission/);
  assert.match(transcript, /question/);
  assert.match(
    transcript,
    /\x1b\[48;2;38;38;40m\x1b\[38;2;238;238;238m▏\x1b\[0m\x1b\[48;2;38;38;40m \x1b\[38;2;250;178;131m>\x1b\[0m\x1b\[48;2;38;38;40m/,
  );

  state = reducer(state, { type: "toggle-models" });
  assert.match(render(state, richCapabilities()), /openai\/gpt-5\.5/);

  state = reducer(state, { type: "toggle-models" });
  state = reducer(state, { type: "toggle-sessions" });
  const sessions = render(state, richCapabilities());
  assert.match(sessions, /sess-1/);
  assert.match(sessions, /Work/);
  assert.match(sessions, /─── .*Sessions.* ─────────/);
  assert.match(sessions, /> sess-1/);
  assert.match(sessions, /\x1b\[48;2;32;32;34m/);
  const sessionLine = sessions.split("\n").find((line) => stripAnsi(line).includes("sess-1"));
  assert.ok(sessionLine);
  assertWideMenuGap(sessionLine, "sess-1", "current");
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
  assert.match(
    transcript,
    /Gone\x1b\[0m\x1b\[48;2;32;32;34m \x1b\[48;5;236m\x1b\[38;2;128;128;128m src\/App\.tsx:12 \x1b\[0m\x1b\[48;2;32;32;34m/,
  );
  assert.doesNotMatch(transcript, /\x1b\[36msrc\/App\.tsx:12\x1b\[0m/);
  assert.match(transcript, /Example/);
  assert.match(transcript, /https:\/\/example\.com/);
  assert.match(transcript, /Example \x1b\[38;2;128;128;128m\(https:\/\/example\.com\)/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.doesNotMatch(transcript, /\[MEDIA:C:\/tmp\/shot\.png:MEDIA\]/);
  assert.match(transcript, /\x1b\[38;2;128;128;128mC:\/tmp\/shot\.png\x1b\[0m/);
  assert.match(transcript, /https:\/\/example\.com\/shot\.png/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\/shot\.png\x1b\\/);
  assert.match(transcript, /👍/u);
  assert.doesNotMatch(transcript, /\[EMOJI:/);
  assert.match(transcript, /\x1b\[48;5;235m│ quoted/);
  assert.match(transcript, /\x1b\[48;5;235m\x1b\[38;2;128;128;128m\[code: python\]/);
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
  assert.match(plain, /👍/u);
  assert.doesNotMatch(plain, /\[EMOJI:/);
  assert.doesNotMatch(plain, /\x1b|<b>|<\/code>|\x1b\]8|▏/u);

  const ansi = render(state, ansiCapabilities());
  assert.match(ansi, /Bold/);
  assert.match(ansi, /Example/);
  assert.match(ansi, /https:\/\/example\.com/);
  assert.match(ansi, /https:\/\/example\.com\/shot\.png/);
  assert.doesNotMatch(ansi, /\[MEDIA:https:\/\/example\.com\/shot\.png:MEDIA\]/);
  assert.match(ansi, /👍/u);
  assert.match(ansi, /\x1b\[[0-9;]*m/);
  assert.doesNotMatch(ansi, /<b>|<\/code>/u);
  assert.match(ansi, /\x1b\]8;;https:\/\/example\.com\/shot\.png\x1b\\/);
  assert.match(ansi, /\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m\x1b\[48;2;32;32;34m/);

  const rich = render(state, richCapabilities());
  assert.match(rich, /\x1b\[1mBold\x1b\[0m/);
  assert.match(rich, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.match(rich, /Example .*https:\/\/example\.com/);
  assert.match(rich, /│ quoted/);
  assert.match(rich, /👍/u);
  assert.doesNotMatch(rich, /\[EMOJI:/);
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
  const richText = stripAnsi(rich);
  assert.doesNotMatch(rich, /[┬┼┴]/u);
  assert.match(richText, /Item: Source/);
  assert.match(richText, /Path: C:\/repo\/apps\/tui/);
  assert.match(richText, /Item/);
  assert.match(richText, /Path/);
  assert.match(rich, /\x1b\[38;2;128;128;128mItem: Source/);
  assert.match(rich, /C:\/repo\/apps\/tui/);
  assert.match(rich, /\x1b\]8;;https:\/\/example\.com\/readme\x1b\\/);
  assert.match(rich, /\x1b\]8;;file:\/\/\/C:\/repo\/apps\/tui\x1b\\/);

  const narrowRich = withTerminalSize(42, 24, () => render(state, richCapabilities()));
  assertFitsTerminal(narrowRich, 42, 24);
  assert.match(narrowRich, /Item: Source/);
  assert.doesNotMatch(narrowRich, /[┬┼┴]/u);
  assert.match(narrowRich, /\x1b\]8/u);
  assert.doesNotMatch(narrowRich, /\x1b\[4m/u);
});

test("render shows agent persona summary and persona panel", () => {
  const session = { id: "sess-persona", title: "Persona", status: "idle" as const, agent: "fast" };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    agents: [
      {
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
      },
    ],
    personas: [
      {
        summary: {
          id: "tura",
          source: "static",
          description: "calm technical collaborator",
          path: "personas/src/tura",
        },
        config: { persona_name: "tura" },
        communication_style: "concise, direct, friendly",
      },
      {
        summary: {
          id: "reviewer",
          source: "dynamic",
          description: "review-first mode",
          path: "personas/src/reviewer",
        },
        config: { persona_name: "reviewer" },
      },
    ],
    sessions: [session],
    sessionConfig: { active_agent: "fast" },
  });
  const top = render(state, richCapabilities());
  assert.doesNotMatch(top, /Agent:.*fast/);
  assert.doesNotMatch(top, /persona:.*tura/);
  assert.match(top, /Enter to send, \/help commands \/settings settings/);
  assert.doesNotMatch(top, /[┌┐└┘]/u);
  assert.match(
    top,
    /^\x1b\[48;2;38;38;40m\x1b\[38;2;238;238;238m▏\x1b\[0m\x1b\[48;2;38;38;40m +…?\x1b\[0m$/m,
  );
  assert.match(top, /^\x1b\[48;2;38;38;40m\x1b\[38;2;238;238;238m▏\x1b\[0m.*Enter to send/m);
  assert.match(top, /tokens -/);

  state = reducer(state, { type: "toggle-personas" });
  const panel = render(state, richCapabilities());
  assert.match(panel, /Personas/);
  assert.match(panel, /> tura/);
  assert.match(panel, /\x1b\[48;2;32;32;34m/);
  assert.match(panel, /tura/);
  assert.match(panel, /calm technical collaborator/);
  assert.match(panel, /concise, direct, friendly/);
  const personaLine = panel.split("\n").find((line) => stripAnsi(line).includes("> tura"));
  assert.ok(personaLine);
  assertWideMenuGap(personaLine, "tura", "current");
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
  assert.match(rich, /\x1b\[38;2;128;128;128mtokens 100/);
  const ansiMeta = ansi.split("\n").at(-1) ?? "";
  const richMeta = rich.split("\n").at(-1) ?? "";
  assert.equal(stripAnsi(ansiMeta), "◇ │ codex/gpt-5.5 low │ tokens 100");
  assert.equal(stripAnsi(richMeta), stripAnsi(ansiMeta));
  assert.match(ansiMeta, /\x1b\[38;2;128;128;128m/);
  assert.match(richMeta, /\x1b\[38;2;128;128;128m/);
  assert.doesNotMatch(ansiMeta, /\x1b\[48;2;38;38;40m/);
  assert.doesNotMatch(richMeta, /\x1b\[48;2;38;38;40m/);
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
  assert.match(transcript, /C:\/tmp\/a\.png/);
  assert.doesNotMatch(transcript, /\[MEDIA:C:\/tmp\/a\.png:MEDIA\]/);
  assert.doesNotMatch(transcript, /<b>|<\/b>|<code>|<\/code>/);
});

test("render shows assistant command summaries, command details setting, and thinking state", () => {
  const session = { id: "sess-commands", title: "Commands", status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-summary",
        sessionID: "sess-commands",
        role: "assistant",
        parts: [
          {
            id: "part-command-text",
            type: "text",
            text: "Checking the app before the final answer.",
          },
          {
            id: "part-inline-payload",
            type: "text",
            text: '[command_run: {"task_summary":"inline payload summary should be readable"}]\n[command_run: {"status":"done"}]',
          },
          {
            id: "tool-command-1",
            type: "tool",
            tool: "runtime",
            state: { status: "completed", input: { command_line: "npm test -- --runInBand" } },
          },
          {
            id: "tool-command-2",
            type: "tool",
            tool: "runtime",
            state: {
              status: "completed",
              input: { command_line: "node tools/snake_playwright.mjs" },
            },
          },
          {
            id: "tool-powershell-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command: "Get-ChildItem -Force | Select-Object FullName" },
            },
          },
          {
            id: "tool-running-command",
            type: "tool",
            tool: "command_run",
            state: { status: "running", input: { command_line: "pnpm test --watch" } },
          },
          {
            id: "tool-task-summary",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output:
                '[command_run: {\\"task_summary\\":\\"provide concise final verification summary\\"}]',
            },
          },
          {
            id: "tool-status",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: '[command_run: {\\"status\\":\\"done\\"}]' },
          },
          {
            id: "tool-input-status",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", input: { status: "done" } },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: false },
  });

  const collapsed = render(state, richCapabilities());
  assert.match(collapsed, /Checking the app/);
  assert.match(collapsed, /Commands: 4/);
  assert.match(collapsed, /◇.*Commands: 4/);
  const collapsedCommandLine = collapsed
    .split("\n")
    .find((line) => stripAnsi(line).includes("Commands: 4"));
  assert.ok(collapsedCommandLine);
  assert.equal(stripAnsi(collapsedCommandLine), "◇ Commands: 4");
  assert.doesNotMatch(collapsedCommandLine, /\x1b\[48;2;32;32;34m/);
  assert.match(collapsed, /\x1b\[90m/);
  assert.doesNotMatch(collapsed, /last.*Get-ChildItem -Force/);
  assert.doesNotMatch(collapsed, /show commands/);
  assert.doesNotMatch(collapsed, /click \/ Ctrl\+O/);
  const collapsedText = stripAnsi(collapsed).replace(/\s*\n\s*/g, "");
  assert.match(collapsedText, /inline payload summary should be r/);
  assert.match(collapsedText, /eadable/);
  assert.match(collapsed, /\[command_run: done\]/);
  assert.doesNotMatch(collapsed, /bash: npm test -- --runInBand/);
  assert.doesNotMatch(collapsed, /task_summary/);
  assert.doesNotMatch(collapsed, /\{"status"/);
  assert.match(collapsed, /thinking/);

  state = reducer(state, { type: "toggle-command-details" });
  const expanded = render(state, richCapabilities());
  assert.doesNotMatch(expanded, /hide commands/);
  const expandedCommandLine = expanded
    .split("\n")
    .find((line) => stripAnsi(line).includes("Commands: 4"));
  assert.ok(expandedCommandLine);
  assert.equal(stripAnsi(expandedCommandLine), "◇ Commands: 4");
  assert.doesNotMatch(expandedCommandLine, /\x1b\[48;2;32;32;34m/);
  assert.match(expanded, /\$.*npm test -- --runInBand/);
  assert.match(expanded, /\$.*node tools\/snake_playwright\.mjs/);
  assert.match(expanded, /\$.*Get-ChildItem -Force/);
  assert.match(expanded, /\$.*pnpm test --watch/);
  assert.match(expanded, /\x1b\[90m\$ pnpm test --watch/);
  const npmTestLine = expanded
    .split("\n")
    .find((line) => stripAnsi(line).includes("$ npm test -- --runInBand"));
  assert.ok(npmTestLine);
  assert.doesNotMatch(npmTestLine, /\x1b\[48;2;32;32;34m/);
  assert.doesNotMatch(expanded, /\{"command_line"/);
});

test("render keeps L1 L2 L3 readable without overflow across terminal sizes", () => {
  const session = { id: "sess-layout", title: "Layout", status: "idle" as const };
  const longPath = "C:/Users/liuliu/Documents/tura/apps/tui/src/tui/render-terminal.ts:123";
  const state = reducer(initialState("C:/Users/liuliu/Documents/tura"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-layout-user",
        sessionID: "sess-layout",
        role: "user",
        parts: [
          {
            id: "part-layout-user",
            type: "text",
            text: `Please inspect ${longPath} and keep the answer compact even on a narrow terminal.`,
          },
        ],
      },
      {
        id: "msg-layout-assistant",
        sessionID: "sess-layout",
        role: "assistant",
        parts: [
          {
            id: "part-layout-assistant",
            type: "text",
            text:
              "**Layout evidence**\n" +
              "Short status first, details hidden by default.\n" +
              `Local path ${longPath}\n` +
              "| Phase | Evidence |\n" +
              "| --- | --- |\n" +
              "| L1 | plain safe text |\n" +
              "| L2 | geometric feedback |\n" +
              "| L3 | Primer-style rich UI |\n" +
              "```text\nnpm run test:e2e\n```\n" +
              "Extra line one\nExtra line two\nExtra line three\nExtra line four",
          },
          {
            id: "tool-layout",
            type: "tool",
            tool: "command_run",
            state: { status: "running", input: { command: "npm run test:e2e -- --layout" } },
          },
        ],
      },
    ],
    permissions: [{ id: "perm-layout", sessionID: "sess-layout", permission: "shell" }],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  for (const { cols, rows } of [
    { cols: 52, rows: 18 },
    { cols: 80, rows: 24 },
    { cols: 132, rows: 36 },
  ]) {
    for (const capabilities of [plainCapabilities(), ansiCapabilities(), richCapabilities()]) {
      const output = withTerminalSize(cols, rows, () => render(state, capabilities));
      assertFitsTerminal(output, cols, rows);
      if (capabilities.level === "plain") assert.doesNotMatch(output, /\x1b/u);
      if (capabilities.level === "ansi") {
        assert.match(output, /[◆◇▏]/u);
        assert.match(output, /\x1b\]8/u);
        assert.doesNotMatch(stripAnsi(output), /^─{8,}$/mu);
        assertOpencodePalette(output);
      }
      if (capabilities.level === "rich") {
        assert.match(output, /\x1b\[38;2;250;178;131m/);
        assert.match(output, /\x1b\[48;2;32;32;34m/);
        assert.doesNotMatch(stripAnsi(output), /─{8,}/u);
        assert.doesNotMatch(output, /\x1b\[38;2;157;124;216m/);
        assert.doesNotMatch(output, /\x1b\[38;2;127;216;143m/);
        assertOpencodePalette(output);
      }
    }
  }
});

test("render uses opencode-style turn spacing and compact command disclosure in L1 L2 L3", () => {
  const session = { id: "sess-turns", title: "Turn Layout", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-turn-user",
        sessionID: "sess-turns",
        role: "user",
        parts: [{ id: "part-turn-user", type: "text", text: "Summarize the terminal layout." }],
      },
      {
        id: "msg-turn-assistant",
        sessionID: "sess-turns",
        role: "assistant",
        parts: [
          {
            id: "part-turn-assistant",
            type: "text",
            text: "Feedback first. Details stay folded unless commands are expanded.",
          },
          {
            id: "tool-turn",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", input: { command: "npm run test:e2e" } },
          },
        ],
      },
      {
        id: "msg-turn-assistant-followup",
        sessionID: "sess-turns",
        role: "assistant",
        parts: [{ id: "part-turn-assistant-followup", type: "text", text: "Follow-up block." }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = render(state, plainCapabilities());
  assert.match(plain, /^\s{2}Summarize/m);
  assert.match(plain, /^\s{2}Feedback first/m);
  assert.doesNotMatch(plain, /(?:^|\n)(?:user|assistant)(?:\n|$)/);
  assert.match(plain, /Commands: 1/);
  const plainLines = plain.split("\n");
  const plainCommandIndex = plainLines.findIndex((line) => line.includes("Commands: 1"));
  assert.ok(plainCommandIndex >= 0);
  assert.equal(plainLines[plainCommandIndex], "* Commands: 1");
  assert.equal(plainLines[plainCommandIndex - 1], "");
  assert.equal(plainLines[plainCommandIndex + 1], "");
  assert.doesNotMatch(plain, /\$ npm run test:e2e/);
  assert.doesNotMatch(plain, /\x1b|▏|◆|◇/u);

  const ansi = render(state, ansiCapabilities());
  assert.doesNotMatch(ansi, /◇.*user|◆.*assistant/u);
  assert.match(ansi, /^\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m.*Feedback first/m);
  assert.match(ansi, /◇ Commands: 1/u);
  const ansiLines = ansi.split("\n");
  const ansiCommandIndex = ansiLines.findIndex((line) => stripAnsi(line).includes("Commands: 1"));
  assert.ok(ansiCommandIndex >= 0);
  assert.equal(stripAnsi(ansiLines[ansiCommandIndex]), "◇ Commands: 1");
  assert.equal(stripAnsi(ansiLines[ansiCommandIndex - 1] ?? ""), "");
  assert.equal(stripAnsi(ansiLines[ansiCommandIndex + 1] ?? ""), "");
  assert.notEqual(stripAnsi(ansiLines[ansiCommandIndex - 2] ?? ""), "");
  assert.notEqual(stripAnsi(ansiLines[ansiCommandIndex + 2] ?? ""), "");
  assert.doesNotMatch(ansiLines[ansiCommandIndex], /\x1b\[48;2;32;32;34m/);
  assert.doesNotMatch(ansi, /\$ npm run test:e2e/);
  assertOpencodePalette(ansi);

  const rich = render(state, richCapabilities());
  assert.match(rich, /^\x1b\[48;2;32;32;34m\x1b\[38;2;238;238;238m▏\x1b\[0m.*Summarize/m);
  assert.doesNotMatch(rich, /^\x1b\[38;2;(?:128;128;128|238;238;238)m▏\x1b\[0m +\x1b\[0m$/m);
  assert.match(rich, /^\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m.*Feedback first/m);
  assert.doesNotMatch(rich, /(?:user|assistant)/);
  assert.doesNotMatch(rich, /[┌├└].*(?:user|assistant)/u);
  assert.match(rich, /\x1b\[48;2;32;32;34m/);
  assert.doesNotMatch(rich, /\x1b\[38;2;157;124;216m/);
  assert.doesNotMatch(rich, /\x1b\[38;2;127;216;143m/);
  assert.match(rich, /◇ Commands: 1/u);
  const richLines = rich.split("\n");
  const richCommandIndex = richLines.findIndex((line) => stripAnsi(line).includes("Commands: 1"));
  assert.ok(richCommandIndex >= 0);
  assert.equal(stripAnsi(richLines[richCommandIndex]), "◇ Commands: 1");
  assert.equal(stripAnsi(richLines[richCommandIndex - 1] ?? ""), "");
  assert.equal(stripAnsi(richLines[richCommandIndex + 1] ?? ""), "");
  assert.notEqual(stripAnsi(richLines[richCommandIndex - 2] ?? ""), "");
  assert.notEqual(stripAnsi(richLines[richCommandIndex + 2] ?? ""), "");
  assert.doesNotMatch(richLines[richCommandIndex], /\x1b\[48;2;32;32;34m/);
  assert.doesNotMatch(rich, /\$ npm run test:e2e/);
  assertOpencodePalette(rich);
});

test("plain L1 uses whitespace instead of decorative lines", () => {
  const session = { id: "sess-plain-lines", title: "Plain Lines", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-plain-user",
        sessionID: "sess-plain-lines",
        role: "user",
        parts: [{ id: "part-plain-user", type: "text", text: "No line art here." }],
      },
      {
        id: "msg-plain-assistant",
        sessionID: "sess-plain-lines",
        role: "assistant",
        parts: [{ id: "part-plain-assistant", type: "text", text: "Only text and spacing." }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(52, 18, () => render(state, plainCapabilities()));
  assertFitsTerminal(output, 52, 18);
  assert.doesNotMatch(output, /[▏─┌┐└┘├┤┬┴┼]/u);
  for (const line of output.split("\n")) {
    assert.doesNotMatch(line, /^-{8,}$/);
  }
});

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
        show_command_instructions: false,
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
  assert.doesNotMatch(ansiLines[ansiTitleIndex - 1] ?? "", /\x1b\[48;2;32;32;34m/);
  assert.doesNotMatch(stripAnsi(ansi), /^─{8,}$/mu);
  assert.match(
    ansi,
    /^\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m\x1b\[48;2;32;32;34m +…?\x1b\[0m$/m,
  );
  assert.match(ansi, /^\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m.*> \/model/m);
  assert.match(ansi, /\/model <provider\/model>/);
  assert.match(ansi, /\/commands/);
  assert.doesNotMatch(ansi, /\/config get|\/config set|\/model provider\/model/);
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
  assert.doesNotMatch(richLines[richTitleIndex - 1] ?? "", /\x1b\[48;2;32;32;34m/);
  assert.doesNotMatch(stripAnsi(rich), /^─{8,}$/mu);
  assert.match(rich, /\x1b\[48;2;32;32;34m/);
  assert.match(
    rich,
    /^\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m\x1b\[48;2;32;32;34m .*Session Settings/m,
  );
  assert.match(
    rich,
    /\x1b\[38;2;250;178;131m> \/model <provider\/model>\s+\x1b\[0m.*\x1b\[38;2;128;128;128mgpt-5\.5/,
  );
  const richSettingInstructionLine = richLines.find((line) =>
    stripAnsi(line).includes("/commands"),
  );
  assert.ok(richSettingInstructionLine);
  assert.match(richSettingInstructionLine, /\x1b\[38;2;250;178;131m {2}\/commands/);
  assert.match(richSettingInstructionLine, /\x1b\[38;2;128;128;128mfalse/);
  const richSettingModelLine = richLines.find((line) => stripAnsi(line).includes("/model"));
  assert.ok(richSettingModelLine);
  assert.match(stripAnsi(richSettingModelLine), /\/model <provider\/model>/);
  assertWideMenuGap(richSettingModelLine, "/model <provider/model>", "gpt-5.5", 12);
  assert.doesNotMatch(richSettingInstructionLine, /\x1b\[90m/);
  assert.doesNotMatch(rich, /\x1b\[90m(?:gpt-5\.5|false)/);
  assert.doesNotMatch(rich, /\/config get|\/config set|\/model provider\/model/);
  assertOpencodePalette(rich);
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
  assertFitsTerminal(plain, 52, 26);
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
  assert.doesNotMatch(ansiLines[ansiTitleIndex - 1] ?? "", /\x1b\[48;2;32;32;34m/);
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
  assert.doesNotMatch(richLines[richTitleIndex - 1] ?? "", /\x1b\[48;2;32;32;34m/);
  assert.match(
    rich,
    /^\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m\x1b\[48;2;32;32;34m .*Help/m,
  );
  assert.match(rich, /^\x1b\[48;2;32;32;34m\x1b\[38;2;128;128;128m▏\x1b\[0m.*\/chat/m);
  assert.doesNotMatch(rich, /system/);
  assert.match(rich, /\x1b\[38;2;250;178;131m\/chat/);
  assert.match(rich, /\x1b\[38;2;128;128;128mclose panels/);
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
    assertFitsTerminal(output, cols, 24);
    const railLines = output
      .split("\n")
      .filter((line) => /\x1b\[38;2;(?:238;238;238|250;178;131)m▏\x1b\[0m/.test(line));
    assert.ok(railLines.length >= 5, `expected rich rail and composer lines at ${cols} cols`);
    assert.doesNotMatch(output, /[┌┐└┘├┤]/u);
    for (const line of railLines) assert.ok(visibleTextWidth(line) <= cols);
  }
});

test("render defaults to compact feedback and keeps extra assistant text hidden", () => {
  const session = { id: "sess-compact", title: "Compact", status: "idle" as const };
  const text = Array.from(
    { length: 16 },
    (_item, index) => `visible-policy-line-${index + 1}`,
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
            state: { status: "completed", input: { command: "npm test" } },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const collapsed = render(state, richCapabilities());
  assert.match(collapsed, /visible-policy-line-8/);
  assert.doesNotMatch(collapsed, /visible-policy-line-9/);
  assert.match(collapsed, /◇ Commands: 1/);
  assert.doesNotMatch(collapsed, /\$ npm test/);

  const expanded = render(reducer(state, { type: "toggle-command-details" }), richCapabilities());
  assert.match(expanded, /◇ Commands: 1/);
  assert.match(expanded, /\$ npm test/);
  const commandLine = expanded.split("\n").find((line) => stripAnsi(line).includes("Commands: 1"));
  assert.ok(commandLine);
  assert.equal(stripAnsi(commandLine), "◇ Commands: 1");
  assert.doesNotMatch(commandLine, /\x1b\[48;2;32;32;34m/);
  const npmTestLine = expanded.split("\n").find((line) => stripAnsi(line).includes("$ npm test"));
  assert.ok(npmTestLine);
  assert.match(stripAnsi(npmTestLine), /^[└├│ ]/u);
  assert.doesNotMatch(npmTestLine, /\x1b\[48;2;32;32;34m/);
});
