import assert from "node:assert/strict";
import test from "node:test";
import { assertDictionaryParity, setLanguage, t } from "../../../src/i18n.js";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render, renderChatFrameParts, renderFrame } from "../../../src/tui/render.js";
import { transcriptLines, transcriptLiveLines } from "../../../src/tui/render/transcript.js";
import {
  ansiCapabilities,
  plainCapabilities,
  richCapabilities,
} from "../../../src/tui/capabilities.js";
import {
  stripAnsi,
  reset,
  textAgentRich,
  textAuxiliary,
  textBackground,
  textPrimary,
  textSecondary,
  truncate,
  truncateAnsi,
  visibleTextWidth,
  wrap,
  wrapAnsi,
} from "../../../src/tui/render-terminal.js";

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

function withNow<T>(now: number, fn: () => T): T {
  const original = Date.now;
  Date.now = () => now;
  try {
    return fn();
  } finally {
    Date.now = original;
  }
}

function assertFitsTerminal(output: string, cols: number, rows: number): void {
  const lines = output.split("\n");
  assert.ok(lines.length <= rows, `expected at most ${rows} rows, got ${lines.length}`);
  assertLineWidths(output, cols);
}

function assertLineWidths(output: string, cols: number): void {
  const lines = output.split("\n");
  for (const [index, line] of lines.entries()) {
    assert.ok(
      visibleTextWidth(line) <= cols,
      `line ${index + 1} overflows ${cols} cols: ${visibleTextWidth(line)} ${stripAnsi(line)}`,
    );
  }
}

function assertOpencodePalette(output: string): void {
  assert.doesNotMatch(output, /\x1b\[(?:3[1-6]|9[1-6])m/u);
  assert.doesNotMatch(
    output,
    /\x1b\[38;2;(?!64;224;208m|70;199;190m|75;174;172m|81;149;154m|86;124;136m|244;247;235m|217;222;205m|151;160;153m|103;116;111m|54;63;61m|61;70;68m)/u,
  );
  assert.doesNotMatch(output, /\x1b\[48;2;(?!16;19;20m|24;27;28m)/u);
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
  assert.equal(visibleTextWidth("𠀀𠀁𠀂"), 6);
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
  assert.deepEqual(wrap("𠀀".repeat(12), 22), ["𠀀".repeat(10), "𠀀".repeat(2)]);
});

test("terminal semantic text colors expose five intensity levels", () => {
  assert.equal(textPrimary, "\x1b[38;2;244;247;235m");
  assert.equal(textAgentRich, "\x1b[38;2;217;222;205m");
  assert.equal(textSecondary, "\x1b[38;2;151;160;153m");
  assert.equal(textAuxiliary, "\x1b[38;2;103;116;111m");
  assert.equal(textBackground, "\x1b[38;2;54;63;61m");
});

test("wrapAnsi preserves dimmed CJK color across wrapped lines", () => {
  const lines = wrapAnsi(`${textSecondary}${"中文滚动".repeat(8)}${textSecondary}尾部${reset}`, 20);
  assert.ok(lines.length > 2);
  for (const line of lines.slice(1, -1)) {
    assert.ok(
      line.startsWith(textSecondary),
      `wrapped Chinese line should keep secondary color: ${JSON.stringify(line)}`,
    );
  }
});

test("render wraps long CJK assistant lines without terminal-spawned blank rows", () => {
  const session = { id: "sess-cjk-width", title: "CJK Width", status: "idle" as const };
  const text = "𠀀𠀁𠀂𠀃𠀄𠀅𠀆𠀇𠀈𠀉".repeat(4);
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-cjk-width",
        sessionID: "sess-cjk-width",
        role: "assistant",
        parts: [{ id: "part-cjk-width", type: "text", text }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(48, 24, () => render(state, richCapabilities()));
  assertFitsTerminal(output, 48, 24);
  const contentLines = output
    .split("\n")
    .filter((line) => stripAnsi(line).includes("𠀀") || stripAnsi(line).includes("𠀁"));
  assert.ok(contentLines.length > 1);
  for (const line of contentLines) assert.ok(visibleTextWidth(line) < 48);
});

test("render keeps assistant message panel right margin tight", () => {
  const session = { id: "sess-tight-panel", title: "Tight Panel", status: "idle" as const };
  const text = "x".repeat(44);
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-tight-panel",
        sessionID: "sess-tight-panel",
        role: "assistant",
        parts: [{ id: "part-tight-panel", type: "text", text }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(48, 24, () => render(state, richCapabilities()));
  const contentLines = output.split("\n").filter((line) => stripAnsi(line).includes("xxxxx"));

  assert.equal(contentLines.length, 1);
  assert.match(stripAnsi(contentLines[0] ?? ""), new RegExp(text));
  assertLineWidths(output, 48);
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
    /\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;16;19;20m/,
  );
  assert.doesNotMatch(transcript, /(?:assistant|user|system)/);
  assert.doesNotMatch(transcript, /\[runtime:/);
  assert.match(transcript, /permission/);
  assert.match(transcript, /question/);
  assert.match(stripAnsi(transcript), /> Enter to send/);
  assert.doesNotMatch(transcript, /\x1b\[48;2;24;27;28m/);

  state = reducer(state, { type: "toggle-models" });
  assert.match(render(state, richCapabilities()), /openai\/gpt-5\.5/);

  state = reducer(state, { type: "toggle-models" });
  state = reducer(state, { type: "toggle-sessions" });
  const sessions = render(state, richCapabilities());
  assert.match(sessions, /Work/);
  assert.match(sessions, /New session/);
  assert.match(sessions, /> New session/);
  assert.match(sessions, /System ready/);
  assert.match(sessions, /Shift\+Enter copy context/);
  assert.match(sessions, /Delete remove/);
  assert.match(sessions, /─── .*Sessions.* ─────────/);
  assert.doesNotMatch(sessions, /> Work/);
  assert.doesNotMatch(sessions, /\/resume <id>/);
  assert.match(sessions, /\x1b\[48;2;16;19;20m/);
  assert.doesNotMatch(sessions, /Enter to send/);
  const sessionLine = sessions.split("\n").find((line) => stripAnsi(line).includes("System ready"));
  assert.ok(sessionLine);
  assertWideMenuGap(sessionLine, "Work", "System ready", 2);
});

test("sessions panel shows names, previews, and status diamonds", () => {
  const active = {
    id: "sess-active",
    session_display_name: "Active Chat",
    status: "idle" as const,
    message_count: 1,
  };
  const busy = {
    id: "sess-busy",
    session_display_name: "Running Chat",
    status: "busy" as const,
    message_count: 3,
  };
  const unread = {
    id: "sess-unread",
    session_display_name: "Finished Chat",
    status: "idle" as const,
    message_count: 4,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: active,
    messages: [
      {
        id: "msg-active",
        sessionID: "sess-active",
        role: "assistant",
        parts: [{ id: "part-active", type: "text", text: "Active preview" }],
      },
    ],
    permissions: [],
    sessions: [active, busy, unread],
  });
  state = {
    ...state,
    sessionsOpen: true,
    sessionPreviews: {
      ...state.sessionPreviews,
      "sess-busy": "Still working",
      "sess-unread": "Finished result with extra text that should not wrap",
    },
    seenSessionMessageCounts: {
      ...state.seenSessionMessageCounts,
      "sess-busy": 3,
      "sess-unread": 3,
    },
  };

  const output = render(state, richCapabilities());
  const plain = stripAnsi(output);
  assert.match(plain, /> New session\s+open a draft chat/);
  assert.match(plain, /Active Chat\s+Active preview/);
  assert.match(plain, /Running Chat ◇\s+Still working/);
  assert.match(plain, /Finished Chat ◆\s+Finished result with extra text that should not wrap/);
  assert.doesNotMatch(plain, /sess-active|sess-busy|sess-unread/);
  assert.doesNotMatch(plain, /Enter to send/);
});

test("sessions panel uses remaining terminal width for previews", () => {
  const active = {
    id: "sess-active-width",
    session_display_name: "Active Chat",
    status: "idle" as const,
    message_count: 1,
  };
  const preview =
    "This preview should stretch across the available right side before it finally truncates near the terminal edge";
  const state = {
    ...reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: active,
      messages: [],
      permissions: [],
      sessions: [active],
    }),
    sessionsOpen: true,
    sessionPreviews: { "sess-active-width": preview },
  };

  const output = withTerminalSize(140, 24, () => render(state, richCapabilities()));
  const line = output.split("\n").find((item) => stripAnsi(item).includes("This preview should"));

  assert.ok(line);
  assert.match(stripAnsi(line), /This preview should stretch across the available right side/);
  assertLineWidths(output, 140);
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
    /Gone\x1b\[0m\x1b\[48;2;16;19;20m\x1b\[38;2;244;247;235m \x1b\[48;5;236m\x1b\[38;2;217;222;205m src\/App\.tsx:12 \x1b\[0m\x1b\[48;2;16;19;20m/,
  );
  assert.doesNotMatch(transcript, /\x1b\[36msrc\/App\.tsx:12\x1b\[0m/);
  assert.match(transcript, /Example/);
  assert.match(transcript, /https:\/\/example\.com/);
  assert.match(transcript, /Example \x1b\[38;2;217;222;205m\(https:\/\/example\.com\)/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.doesNotMatch(transcript, /\[MEDIA:C:\/tmp\/shot\.png:MEDIA\]/);
  assert.match(transcript, /\x1b\[38;2;217;222;205mC:\/tmp\/shot\.png\x1b\[0m/);
  assert.match(transcript, /https:\/\/example\.com\/shot\.png/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\/shot\.png\x1b\\/);
  assert.match(transcript, /👍/u);
  assert.doesNotMatch(transcript, /\[EMOJI:/);
  assert.match(transcript, /\x1b\[48;5;235m\x1b\[38;2;217;222;205m│ quoted/);
  assert.match(transcript, /\x1b\[48;5;235m\x1b\[38;2;217;222;205m```python/);
  assert.match(transcript, /\x1b\[48;5;235m\x1b\[38;2;217;222;205mprint\('hello'\)/);
  assert.match(transcript, /\x1b\[48;5;235m\x1b\[38;2;217;222;205m```/);
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
  assert.match(ansi, /\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;16;19;20m/);

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
  assert.match(rich, /\x1b\[38;2;217;222;205mItem: Source/);
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
  assert.match(top, /Tab sessions/);
  assert.match(top, /\/stop stop agent/);
  assert.doesNotMatch(top, /↑\/↓ view sessions/);
  assert.doesNotMatch(top, /[┌┐└┘]/u);
  assert.match(top, /^\x1b\[48;2;16;19;20m\x1b\[38;2;244;247;235m▏\x1b\[0m/m);
  assert.match(top, /^\x1b\[48;2;16;19;20m\x1b\[38;2;244;247;235m▏\x1b\[0m.*Enter to send/m);
  assert.doesNotMatch(top, /\x1b\[38;2;64;224;208m█\x1b\[0m/);
  assert.match(top, /tokens -/);

  state = reducer(state, { type: "toggle-personas" });
  const panel = render(state, richCapabilities());
  assert.match(panel, /Personas/);
  assert.match(panel, /> tura/);
  assert.match(panel, /\x1b\[48;2;16;19;20m/);
  assert.match(panel, /tura/);
  assert.match(panel, /calm technical collaborator/);
  assert.match(stripAnsi(panel), /concise, direct, fri/u);
  assert.match(stripAnsi(panel), /fri…|fri\.\.\./u);
  const personaLine = panel.split("\n").find((line) => stripAnsi(line).includes("> tura"));
  assert.ok(personaLine);
  assertWideMenuGap(personaLine, "tura", "current");
});

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
            tool: "browser",
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
        id: "msg-command-user",
        sessionID: "sess-commands",
        role: "user",
        created_at: 1_000_000,
        parts: [{ id: "part-command-user", type: "text", text: "Run checks" }],
      },
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
            text: '[command_run: {"task_detail":"inline payload summary should be readable"}]\n[command_run: {"status":"done"}]',
          },
          {
            id: "tool-command-1",
            type: "tool",
            tool: "runtime",
            state: {
              status: "completed",
              input: {
                command_type: "shell_command",
                command_line: "npm test -- --runInBand",
              },
            },
          },
          {
            id: "tool-command-2",
            type: "tool",
            tool: "runtime",
            state: {
              status: "completed",
              input: {
                command_type: "shell_command",
                command_line: "node tools/snake_playwright.mjs",
              },
            },
          },
          {
            id: "tool-powershell-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                step: 7,
                command_type: "shell_command",
                command_line: "Get-ChildItem -Force | Select-Object FullName",
              },
            },
          },
          {
            id: "tool-running-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: { step: 9, command_type: "shell_command", command_line: "pnpm test --watch" },
            },
          },
          {
            id: "tool-task-summary",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output:
                '[command_run: {\\"task_detail\\":\\"provide concise final verification summary\\"}]',
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

  const collapsed = withNow(1_012_300, () => render(state, richCapabilities()));
  assert.match(collapsed, /Checking the app/);
  assert.match(collapsed, /Commands/);
  assert.doesNotMatch(collapsed, /Commands:\s*\d+/);
  assert.match(collapsed, /[◆◇].*Commands/);
  const collapsedCommandLine = collapsed
    .split("\n")
    .find((line) => stripAnsi(line).includes("Commands"));
  assert.ok(collapsedCommandLine);
  assert.match(stripAnsi(collapsedCommandLine), /^[◆◇] Commands$/u);
  assert.doesNotMatch(collapsedCommandLine, /\x1b\[48;2;16;19;20m/);
  assert.match(collapsed, /\x1b\[38;2;103;116;111m/);
  assert.doesNotMatch(collapsed, /last.*Get-ChildItem -Force/);
  assert.doesNotMatch(collapsed, /show commands/);
  assert.doesNotMatch(collapsed, /click \/ Ctrl\+O/);
  const collapsedText = stripAnsi(collapsed).replace(/\s*\n\s*/g, "");
  assert.doesNotMatch(collapsedText, /inline payload summary should be readable/);
  assert.doesNotMatch(collapsed, /\[command_run:/);
  assert.doesNotMatch(collapsed, /bash: npm test -- --runInBand/);
  assert.doesNotMatch(collapsed, /task_detail/);
  assert.doesNotMatch(collapsed, /\{"status"/);
  assert.match(stripAnsi(collapsed), /thinking\s+12s/);
  assert.match(stripAnsi(collapsed), /✦ thinking\s+12s/);
  assert.doesNotMatch(collapsed, /thinking.*Commands/);

  state = reducer(state, {
    type: "session-config",
    value: { show_command_instructions: true },
  });
  const expanded = withNow(1_012_300, () => render(state, richCapabilities()));
  assert.doesNotMatch(expanded, /hide commands/);
  const expandedCommandLine = expanded
    .split("\n")
    .find((line) => stripAnsi(line).includes("Commands"));
  assert.ok(expandedCommandLine);
  assert.match(stripAnsi(expandedCommandLine), /^[◆◇] Commands$/u);
  assert.doesNotMatch(expandedCommandLine, /\x1b\[48;2;16;19;20m/);
  assert.match(expanded, /#1 shell_command completed\s+\$ npm test -- --runInBand/);
  assert.match(expanded, /#1 shell_command completed\s+\$ node tools\/snake_playwright\.mjs/);
  assert.match(expanded, /#7 shell_command completed\s+\$ Get-ChildItem -Force/);
  assert.match(expanded, /#9 shell_command running\s+\$ pnpm test --watch/);
  assert.doesNotMatch(expanded, /provide concise final verification summary/);
  assert.doesNotMatch(expanded, /\$ done/);
  assert.match(expanded, /\x1b\[38;2;103;116;111m.*\$ pnpm test --watch/);
  const npmTestLine = expanded
    .split("\n")
    .find((line) => stripAnsi(line).includes("$ npm test -- --runInBand"));
  assert.ok(npmTestLine);
  assert.doesNotMatch(npmTestLine, /\x1b\[48;2;16;19;20m/);
  assert.doesNotMatch(expanded, /\{"command_line"/);
  assert.equal(
    expanded
      .split("\n")
      .filter((line) =>
        /\$ (?:npm test|node tools\/snake_playwright|Get-ChildItem|pnpm test)/.test(
          stripAnsi(line),
        ),
      ).length,
    4,
  );

  const solid = withNow(1_012_300, () =>
    stripAnsi(render({ ...state, thinkingFrame: 0 }, richCapabilities())),
  );
  const hollow = withNow(1_012_300, () =>
    stripAnsi(render({ ...state, thinkingFrame: 1 }, richCapabilities())),
  );
  const starburst = withNow(1_012_300, () =>
    stripAnsi(render({ ...state, thinkingFrame: 2 }, richCapabilities())),
  );
  assert.match(solid, /^◆ Commands$/mu);
  assert.match(hollow, /^◇ Commands$/mu);
  assert.match(solid, /✦ thinking\s+12s/);
  assert.match(hollow, /✧ thinking\s+12s/);
  assert.match(starburst, /✶ thinking\s+12s/);
  assert.match(solid, /^└─ ■ #9 shell_command running\s+\$ pnpm test --watch$/mu);
  assert.match(hollow, /^└─ □ #9 shell_command running\s+\$ pnpm test --watch$/mu);
  assert.doesNotMatch(solid, /task_status|provide concise final verification summary|\$ done/u);
});

test("render shows streamed command_run results before the whole command batch finishes", () => {
  const session = {
    id: "sess-streamed-commands",
    title: "Streamed Commands",
    status: "busy" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-streamed-command-user",
        sessionID: "sess-streamed-commands",
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-streamed-command-user", type: "text", text: "Run build and tests" }],
      },
      {
        id: "msg-streamed-command-tool",
        sessionID: "sess-streamed-commands",
        role: "assistant",
        created_at: 2_000,
        parts: [
          {
            id: "tool-streamed-command-run",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [
                  { step: 3, command_type: "shell_command", command_line: "npm run build" },
                  {
                    step: 10,
                    command_type: "shell_command",
                    command_line: "npm test -- --runInBand",
                  },
                ],
              },
              output: {
                streamed_command_run_result: {
                  results: [
                    {
                      status: "completed",
                      success: true,
                      step: 3,
                      command_type: "shell_command",
                      command_line: "npm run build",
                      output: { text: "built" },
                    },
                    {
                      status: "running",
                      success: null,
                      command_type: "shell_command",
                      command_line: "npm test -- --runInBand",
                      output: { stdout: "still testing" },
                    },
                  ],
                },
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const solid = withTerminalSize(100, 30, () =>
    stripAnsi(render({ ...state, thinkingFrame: 0 }, richCapabilities())),
  );
  const hollow = withTerminalSize(100, 30, () =>
    stripAnsi(render({ ...state, thinkingFrame: 1 }, richCapabilities())),
  );

  assert.match(solid, /^◆ Commands$/mu);
  assert.match(solid, /^├─ ✓ #3 shell_command completed\s+\$ npm run build$/mu);
  assert.match(solid, /^└─ ■ #10 shell_command running\s+\$ npm test -- --runInBand$/mu);
  assert.match(hollow, /^◇ Commands$/mu);
  assert.match(hollow, /^└─ □ #10 shell_command running\s+\$ npm test -- --runInBand$/mu);
});

test("render uses per-command results from dirty command_run error records", () => {
  const session = {
    id: "sess-dirty-command-error",
    title: "Dirty Command Error",
    status: "idle" as const,
  };
  const commands = [
    {
      step: 1,
      command_type: "shell_command",
      command_line: 'py -3 -c "print(\\"ok\\")"',
    },
    {
      step: 1,
      command_type: "shell_command",
      command_line: 'py -3 -c "raise SystemExit(1)"',
    },
    {
      step: 1,
      command_type: "shell_command",
      command_line: "Get-Content crates/runtime/src/turn_loop/provider_step.rs -TotalCount 120",
    },
    {
      step: 1,
      command_type: "shell_command",
      command_line: "Get-Content crates/session_log/src/store/mod.rs -TotalCount 260",
    },
  ];
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-dirty-command-error",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "tool-dirty-command-error",
            type: "tool",
            tool: "command_run",
            state: {
              status: "error",
              input: { commands },
              error: [
                "Exit code: 1",
                "Old diagnostic output included unrelated commands:",
                "npm run lint",
                "npm run format:check",
                "npm run typecheck",
              ].join("\n"),
            },
            metadata: {
              success: false,
              error: [
                "Exit code: 1",
                "Old diagnostic output included unrelated commands:",
                "npm run lint",
                "npm run format:check",
                "npm run typecheck",
              ].join("\n"),
              output: {
                commands,
                results: [
                  { success: true, output: "Exit code: 0\nOutput:\nok" },
                  { success: false, output: "Exit code: 1\nOutput:\nfailed" },
                  { success: true, output: "Exit code: 0\nOutput:\nfile contents" },
                  {
                    success: false,
                    output:
                      "Exit code: 1\nStderr:\nCannot find path 'crates/session_log/src/store/mod.rs'",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const transcript = withTerminalSize(140, 30, () => stripAnsi(render(state, richCapabilities())));

  assert.match(transcript, /^◇ Commands$/mu);
  assert.match(transcript, /^├─ ✓ #1 shell_command completed\s+\$ py -3 -c "print/mu);
  assert.match(transcript, /^├─ x #1 shell_command failed\s+\$ py -3 -c "raise SystemExit/mu);
  assert.match(transcript, /^├─ ✓ #1 shell_command completed\s+\$ Get-Content crates\/runtime/mu);
  assert.match(transcript, /^└─ x #1 shell_command failed\s+\$ Get-Content crates\/session_log/mu);
  assert.doesNotMatch(transcript, /\$ npm run (?:lint|format:check|typecheck)/u);
});

test("render uses real command_run step numbers from non-streamed command batches", () => {
  const session = { id: "sess-command-batch-steps", title: "Batch Steps", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-batch-steps",
        sessionID: "sess-command-batch-steps",
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "tool-command-batch-steps",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [
                  { step: 4, command_type: "shell_command", command_line: "npm run lint" },
                  { step: 12, command_type: "shell_command", command_line: "npm run typecheck" },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const transcript = stripAnsi(render(state, richCapabilities()));
  assert.match(transcript, /^├─ ✓ #4 shell_command completed\s+\$ npm run lint$/mu);
  assert.match(transcript, /^└─ ✓ #12 shell_command completed\s+\$ npm run typecheck$/mu);
  assert.equal(
    transcript.split("\n").filter((line) => /\$ npm run (?:lint|typecheck)/u.test(line)).length,
    2,
  );
});

test("render blinks only the running command block icon", () => {
  const session = { id: "sess-group-running", title: "Group Running", status: "busy" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-group-running",
        sessionID: session.id,
        role: "assistant",
        parts: [
          {
            id: "tool-group-done",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: "npm run build" },
            },
          },
          {
            id: "tool-group-progress",
            type: "tool",
            tool: "command_run",
            state: {
              status: "in_progress",
              input: { command_type: "shell_command", command_line: "npm test" },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: false },
  });

  const solid = stripAnsi(render({ ...state, thinkingFrame: 0 }, richCapabilities()));
  const hollow = stripAnsi(render({ ...state, thinkingFrame: 1 }, richCapabilities()));

  assert.match(solid, /^◇ Commands$/mu);
  assert.match(solid, /^◆ Commands$/mu);
  assert.equal(solid.match(/Commands/g)?.length, 2);
  assert.equal(hollow.match(/Commands/g)?.length, 2);
});

test("render keeps each command detail to one visible line", () => {
  const session = { id: "sess-long-command", title: "Long Command", status: "idle" as const };
  const tail = "TAIL_VISIBLE_AFTER_WRAP";
  const secondLine = "echo MULTILINE_COMMAND_SECOND_LINE_VISIBLE";
  const command = `node scripts/check.mjs --with-a-very-long-argument ${"参数".repeat(18)} ${tail}\n${secondLine}`;
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-long-command",
        sessionID: session.id,
        role: "assistant",
        parts: [
          {
            id: "tool-long-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: command },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const output = withTerminalSize(58, 30, () => render(state, richCapabilities()));
  const plainLines = stripAnsi(output).split("\n");
  const commandLines = plainLines.filter((line) =>
    /\$ |MULTILINE_COMMAND_SECOND_LINE_VISIBLE/u.test(line),
  );
  assert.equal(commandLines.length, 1, output);
  assert.match(commandLines[0] ?? "", /\$ node scripts\/check/u);
  assert.doesNotMatch(stripAnsi(output), /MULTILINE_COMMAND_SECOND_LINE_VISIBLE/);
  assertLineWidths(output, 58);
});

test("render filters internal task_status command updates from command sections", () => {
  const session = {
    id: "sess-task-status-hidden",
    title: "Task Status Hidden",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-agent",
        sessionID: session.id,
        role: "assistant",
        parts: [
          { id: "text", type: "text", text: "吃碗牛肉面吧。" },
          {
            id: "task-status-json",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: {
                results: [
                  {
                    command_type: "task_status",
                    success: true,
                    output: {
                      task_status: {
                        status: "done",
                        task_detail: "用户要求随机推荐食物",
                      },
                    },
                  },
                ],
              },
            },
          },
          {
            id: "task-status-summary",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output:
                '[command_run: {\\"task_detail\\":\\"用户要求随机推荐食物（中文：有点饿要推荐吃什么）\\"}]',
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const plain = stripAnsi(render(state, richCapabilities()));

  assert.match(plain, /吃碗牛肉面吧。/u);
  assert.doesNotMatch(plain, /Commands/u);
  assert.doesNotMatch(plain, /command_run completed/u);
  assert.doesNotMatch(plain, /task_status|用户要求随机推荐食物/u);
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
            state: {
              status: "running",
              input: {
                command_type: "shell_command",
                command_line: "npm run test:e2e -- --layout",
              },
            },
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
      assertLineWidths(output, cols);
      if (capabilities.level === "plain") assert.doesNotMatch(output, /\x1b/u);
      if (capabilities.level === "ansi") {
        assert.match(output, /[◆◇▏]/u);
        assert.doesNotMatch(stripAnsi(output), /^─{8,}$/mu);
        assertOpencodePalette(output);
      }
      if (capabilities.level === "rich") {
        assert.match(output, /\x1b\[38;2;64;224;208m/);
        assert.match(output, /\x1b\[48;2;16;19;20m/);
        assert.doesNotMatch(stripAnsi(output), /─{8,}/u);
        assert.doesNotMatch(output, /\x1b\[38;2;157;124;216m/);
        assert.doesNotMatch(output, /\x1b\[38;2;127;216;143m/);
        assertOpencodePalette(output);
      }
      assert.doesNotMatch(stripAnsi(output), /earlier output hidden|更早的内容已隐藏/u);
    }
  }
});

test("render uses opencode-style turn spacing and configured command disclosure in L1 L2 L3", () => {
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
            text: "Feedback first. Command details follow the session setting.",
          },
          {
            id: "tool-turn",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: "npm run test:e2e" },
            },
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
  assert.match(plain, /Commands/);
  const plainLines = plain.split("\n");
  const plainCommandIndex = plainLines.findIndex((line) => line.includes("Commands"));
  assert.ok(plainCommandIndex >= 0);
  assert.equal(plainLines[plainCommandIndex], "* Commands");
  assert.equal(plainLines[plainCommandIndex - 1], "");
  assert.match(plainLines[plainCommandIndex + 1] ?? "", /\$ npm run test:e2e/);
  assert.match(plain, /\|- \+ #1 shell_command completed\s+\$ npm run test:e2e/);
  assert.doesNotMatch(plain, /\x1b|▏|◆|◇/u);

  const ansi = render(state, ansiCapabilities());
  assert.doesNotMatch(ansi, /◇.*user|◆.*assistant/u);
  assert.match(ansi, /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m.*Feedback first/m);
  assert.match(ansi, /◇ Commands/u);
  const ansiLines = ansi.split("\n");
  const ansiCommandIndex = ansiLines.findIndex((line) => stripAnsi(line).includes("Commands"));
  assert.ok(ansiCommandIndex >= 0);
  assert.equal(stripAnsi(ansiLines[ansiCommandIndex]), "◇ Commands");
  assert.equal(stripAnsi(ansiLines[ansiCommandIndex - 1] ?? ""), "");
  assert.match(stripAnsi(ansiLines[ansiCommandIndex + 1] ?? ""), /\$ npm run test:e2e/);
  assert.notEqual(stripAnsi(ansiLines[ansiCommandIndex - 2] ?? ""), "");
  assert.doesNotMatch(ansiLines[ansiCommandIndex], /\x1b\[48;2;16;19;20m/);
  assert.match(ansi, /└─ ✓ #1 shell_command completed\s+\$ npm run test:e2e/u);
  assertOpencodePalette(ansi);

  const rich = render(state, richCapabilities());
  assert.match(rich, /^\x1b\[48;2;16;19;20m\x1b\[38;2;244;247;235m▏\x1b\[0m.*Summarize/m);
  assert.doesNotMatch(rich, /^\x1b\[38;2;(?:103;116;111|244;247;235)m▏\x1b\[0m +\x1b\[0m$/m);
  assert.match(rich, /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m.*Feedback first/m);
  assert.doesNotMatch(rich, /(?:user|assistant)/);
  assert.doesNotMatch(rich, /[┌├└].*(?:user|assistant)/u);
  assert.match(rich, /\x1b\[48;2;16;19;20m/);
  assert.doesNotMatch(rich, /\x1b\[38;2;157;124;216m/);
  assert.doesNotMatch(rich, /\x1b\[38;2;127;216;143m/);
  assert.match(rich, /◇ Commands/u);
  const richLines = rich.split("\n");
  const richCommandIndex = richLines.findIndex((line) => stripAnsi(line).includes("Commands"));
  assert.ok(richCommandIndex >= 0);
  assert.equal(stripAnsi(richLines[richCommandIndex]), "◇ Commands");
  assert.equal(stripAnsi(richLines[richCommandIndex - 1] ?? ""), "");
  assert.match(stripAnsi(richLines[richCommandIndex + 1] ?? ""), /\$ npm run test:e2e/);
  assert.notEqual(stripAnsi(richLines[richCommandIndex - 2] ?? ""), "");
  assert.doesNotMatch(richLines[richCommandIndex], /\x1b\[48;2;16;19;20m/);
  assert.match(rich, /└─ ✓ #1 shell_command completed\s+\$ npm run test:e2e/u);
  assertOpencodePalette(rich);
});

test("render ignores adjacent command_run summaries without command types", () => {
  const session = { id: "sess-command-group", title: "Command Group", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-group-user",
        sessionID: "sess-command-group",
        role: "user",
        parts: [{ id: "part-group-user", type: "text", text: "你好啊" }],
      },
      {
        id: "msg-group-command-1",
        sessionID: "sess-command-group",
        role: "assistant",
        parts: [
          {
            id: "tool-group-command-1",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: "Greeted the user" },
          },
        ],
      },
      {
        id: "msg-group-command-2",
        sessionID: "sess-command-group",
        role: "assistant",
        parts: [
          {
            id: "tool-group-command-2",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: "Greeted the user again" },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const rich = render(state, richCapabilities());
  const plain = stripAnsi(rich);
  assert.match(plain, /你好啊/);
  assert.doesNotMatch(plain, /◆\s+你好啊/u);
  assert.doesNotMatch(plain, /Commands/u);
  assert.doesNotMatch(plain, /#1 completed\s+\$ Greeted/u);
  assert.doesNotMatch(plain, /\[command_run:/u);
  const lines = plain.split("\n");
  const userIndex = lines.findIndex((line) => line.includes("你好啊"));
  assert.ok(userIndex >= 0);
});

test("render filters command_run summaries and type-only records from command sections", () => {
  const session = { id: "sess-command-filter", title: "Command Filter", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-filter",
        sessionID: session.id,
        role: "assistant",
        parts: [
          {
            id: "tool-filter-task-detail",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: '{"status":"done","task_detail":"large file scan"}',
            },
          },
          {
            id: "tool-filter-summary",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: "large file scan" },
          },
          {
            id: "tool-filter-valid-and-type-only",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: {
                results: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "$ErrorActionPreference='Stop'",
                    status: "completed",
                    success: true,
                  },
                  {
                    step: 1,
                    command_type: "shell_command",
                    status: "completed",
                    success: true,
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const plain = stripAnsi(render(state, richCapabilities()));

  assert.equal(plain.match(/Commands/g)?.length, 1);
  assert.match(plain, /\$ \$ErrorActionPreference='Stop'/u);
  assert.doesNotMatch(plain, /\$ large file scan/u);
  assert.doesNotMatch(plain, /\$ \{"status":"done","task_detail":"large file scan"\}/u);
  assert.doesNotMatch(plain, /\$ shell_command/u);
});

test("render keeps command-only updates at their exact message position", () => {
  const session = { id: "sess-command-order", title: "Command Order", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-order-user",
        sessionID: "sess-command-order",
        role: "user",
        parts: [{ id: "part-order-user", type: "text", text: "Fix it" }],
      },
      {
        id: "msg-order-first",
        sessionID: "sess-command-order",
        role: "assistant",
        parts: [{ id: "part-order-first", type: "text", text: "First visible reply." }],
      },
      {
        id: "msg-order-tool",
        sessionID: "sess-command-order",
        role: "assistant",
        parts: [
          {
            id: "tool-order-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: "npm test" },
            },
          },
        ],
      },
      {
        id: "msg-order-final",
        sessionID: "sess-command-order",
        role: "assistant",
        parts: [{ id: "part-order-final", type: "text", text: "Final visible reply." }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  for (const capabilities of [plainCapabilities(), richCapabilities()]) {
    const output = withTerminalSize(100, 30, () => stripAnsi(render(state, capabilities)));
    const firstIndex = output.indexOf("First visible reply.");
    const finalIndex = output.indexOf("Final visible reply.");
    const commandIndex = output.indexOf("$ npm test");
    assert.ok(firstIndex >= 0);
    assert.ok(commandIndex > firstIndex, output);
    assert.ok(finalIndex > commandIndex, output);
  }
});

test("render keeps assistant text above command parts even when tool part arrives first", () => {
  const session = { id: "sess-part-order", title: "Part Order", status: "idle" as const };
  const state = {
    ...initialState("C:/repo"),
    session,
    messages: [
      {
        id: "msg-part-order",
        sessionID: "sess-part-order",
        role: "assistant" as const,
        parts: [
          {
            id: "tool-part-order",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: '{"status":"done","task_detail":"Greeting answered"}',
            },
          },
          {
            id: "text-part-order",
            type: "text",
            text: "你好，问候已经回复。",
          },
        ],
      },
    ],
    sessionConfig: { show_command_instructions: true },
  };

  const output = withTerminalSize(100, 30, () => stripAnsi(render(state, plainCapabilities())));
  const textIndex = output.indexOf("你好，问候已经回复。");
  assert.ok(textIndex >= 0, output);
  assert.doesNotMatch(output, /Commands|command_run completed|\$ Greeting answered/u);
});

test("render normalizes command progress carriage returns into new lines", () => {
  const session = {
    id: "sess-command-progress",
    title: "Command Progress",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-progress-user",
        sessionID: "sess-command-progress",
        role: "user",
        parts: [{ id: "part-progress-user", type: "text", text: "run progress" }],
      },
      {
        id: "msg-progress-assistant",
        sessionID: "sess-command-progress",
        role: "assistant",
        parts: [
          {
            id: "part-progress-text",
            type: "text",
            text: "started\rstill running\x1b[2K\rfinished",
          },
          {
            id: "tool-progress",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: {
                results: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "Downloading 10%\rDownloading 90%\x1b[1Gdone",
                    status: "completed",
                    success: true,
                  },
                ],
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

  const plain = stripAnsi(render(state, richCapabilities()));
  assert.doesNotMatch(plain, /\r/u);
  assert.doesNotMatch(plain, /\x1b\[(?:2K|1G)/u);
  assert.match(plain, /started/);
  assert.match(plain, /still running/);
  assert.match(plain, /finished/);
  assert.match(plain, /Commands/);
  assert.match(plain, /#1 shell_command completed\s+\$ Downloading 10%/u);
});

test("render keeps composer and bottom meta visible after large command blocks", () => {
  const session = { id: "sess-command-footer", title: "Command Footer", status: "idle" as const };
  const commandParts = Array.from({ length: 8 }, (_, index) => ({
    id: `tool-footer-${index + 1}`,
    type: "tool",
    tool: "command_run",
    state: {
      status: "completed",
      input: {
        command_type: "shell_command",
        command_line: `Get-Content -Raw apps/tui/test-results/tui-snake-playwright/very-long-run-${index + 1}/summary.json`,
      },
    },
  }));
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-footer-user",
        sessionID: "sess-command-footer",
        role: "user",
        parts: [{ id: "part-footer-user", type: "text", text: "read summaries" }],
      },
      {
        id: "msg-footer-commands",
        sessionID: "sess-command-footer",
        role: "assistant",
        parts: commandParts,
      },
      {
        id: "msg-footer-reply",
        sessionID: "sess-command-footer",
        role: "assistant",
        parts: [
          {
            id: "part-footer-reply",
            type: "text",
            text: "这段新的回复必须在命令块下面的新行显示，不能覆盖命令。",
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  // Default view shows the most recent content and keeps the footer pinned.
  const output = withTerminalSize(106, 18, () => render(state, richCapabilities()));
  const plain = stripAnsi(output);
  const lines = plain.split("\n");
  assert.ok(lines.some((line) => line.includes("Enter to send")));
  assert.ok(lines.some((line) => line.includes("tokens")));
  assert.ok(lines.some((line) => line.includes("这段新的回复必须")));
  assertLineWidths(output, 106);
});

test("transcript cache keeps durable gateway text while live excludes chrome and thinking rows", () => {
  const session = { id: "sess-live-chrome", title: "Live Chrome", status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-user-live-chrome",
        sessionID: "sess-live-chrome",
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-user-live-chrome", type: "text", text: "hello" }],
      },
    ],
    permissions: [],
    sessions: [session],
  });
  state = reducer(state, { type: "composer", value: "draft input" });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: session.id,
          message_id: "msg-live-delta",
          part_id: "part-live-delta",
          field: "text",
          delta: "LIVE_DELTA_MARKER",
        },
      },
    },
  });

  const cachedHistory = stripAnsi(transcriptLines(state, 80).join("\n"));
  assert.match(cachedHistory, /hello/);
  assert.doesNotMatch(cachedHistory, /LIVE_DELTA_MARKER/);
  assert.doesNotMatch(cachedHistory, /thinking/i);
  assert.doesNotMatch(cachedHistory, /draft input/);
  assert.doesNotMatch(cachedHistory, /Enter to send|回车输入/);
  assert.doesNotMatch(cachedHistory, /tokens/);

  const liveRows = stripAnsi(transcriptLiveLines(state, 80).join("\n"));
  assert.doesNotMatch(liveRows, /hello/);
  assert.match(liveRows, /LIVE_DELTA_MARKER/);
  assert.doesNotMatch(liveRows, /thinking/i);

  const chromeRows = stripAnsi(renderChatFrameParts(state, richCapabilities()).chromeFrame);
  assert.match(chromeRows, /thinking/i);
});

test("completed user and assistant turn moves from live into transcript cache", () => {
  const session = {
    id: "sess-completed-turn-cache",
    title: "Completed Cache",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-cache-user",
        sessionID: session.id,
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-cache-user", type: "text", text: "CACHE_USER_TEXT" }],
      },
      {
        id: "msg-cache-agent",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_500,
        parts: [{ id: "part-cache-agent", type: "text", text: "CACHE_AGENT_TEXT" }],
      },
    ],
    permissions: [],
    sessions: [session],
  });

  const cachedHistory = stripAnsi(transcriptLines(state, 80).join("\n"));
  const liveRows = stripAnsi(transcriptLiveLines(state, 80).join("\n"));

  assert.match(cachedHistory, /CACHE_USER_TEXT[\s\S]*CACHE_AGENT_TEXT/);
  assert.doesNotMatch(liveRows, /CACHE_USER_TEXT|CACHE_AGENT_TEXT/);
});

test("live assistant text keeps event order before a later user message", () => {
  const session = { id: "sess-live-user-order", title: "Live Order", status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-user-before-live",
        sessionID: session.id,
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-user-before-live", type: "text", text: "USER_BEFORE_LIVE" }],
      },
    ],
    permissions: [],
    sessions: [session],
  });
  state = withNow(1_500, () =>
    reducer(state, {
      type: "event",
      event: {
        directory: "C:/repo",
        payload: {
          type: "message.part.delta",
          properties: {
            session_id: session.id,
            message_id: "msg-live-before-next-user",
            part_id: "part-live-before-next-user",
            field: "text",
            delta: "LIVE_BEFORE_NEXT_USER",
          },
        },
      },
    }),
  );
  state = reducer(state, {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-user-before-live",
        sessionID: session.id,
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-user-before-live", type: "text", text: "USER_BEFORE_LIVE" }],
      },
      {
        id: "msg-user-after-live",
        sessionID: session.id,
        role: "user",
        created_at: 2_000,
        parts: [{ id: "part-user-after-live", type: "text", text: "USER_AFTER_LIVE" }],
      },
    ],
    permissions: [],
    sessions: [session],
  });

  const output = stripAnsi(render(state, richCapabilities()));

  assert.match(output, /USER_BEFORE_LIVE[\s\S]*LIVE_BEFORE_NEXT_USER[\s\S]*USER_AFTER_LIVE/);
});

test("live transcript rows append below the complete history, independent of viewport height", () => {
  const session = {
    id: "sess-live-after-history",
    title: "Live After History",
    status: "busy" as const,
  };
  const messages = Array.from({ length: 24 }, (_, index) => ({
    id: `msg-live-history-${index}`,
    sessionID: session.id,
    role: index % 2 === 0 ? ("assistant" as const) : ("user" as const),
    parts: [
      {
        id: `part-live-history-${index}`,
        type: "text",
        text: `LIVE_HISTORY_MARKER_${String(index + 1).padStart(2, "0")}`,
      },
    ],
  }));
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages,
    permissions: [],
    sessions: [session],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: session.id,
          message_id: "msg-live-tail",
          part_id: "part-live-tail",
          field: "text",
          delta: "LIVE_STREAM_APPEND_MARKER",
        },
      },
    },
  });

  const output = withTerminalSize(80, 8, () => stripAnsi(render(state, richCapabilities())));

  assert.match(output, /LIVE_HISTORY_MARKER_01/);
  assert.match(output, /LIVE_HISTORY_MARKER_24/);
  assert.match(output, /LIVE_STREAM_APPEND_MARKER/);
  assert.match(output, /thinking/i);
  assert.ok(
    output.indexOf("LIVE_HISTORY_MARKER_24") < output.indexOf("LIVE_STREAM_APPEND_MARKER"),
    "live stream text must append after the full cached transcript",
  );
  assert.ok(
    output.indexOf("LIVE_STREAM_APPEND_MARKER") < output.search(/thinking/i),
    "live rows must be appended after the full cached transcript, not after a viewport slice",
  );
});

test("render prioritizes current content over overflow marker in very short terminals", () => {
  const session = { id: "sess-short-height", title: "Short Height", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-short-user",
        sessionID: "sess-short-height",
        role: "user",
        parts: [{ id: "part-short-user", type: "text", text: "summarize" }],
      },
      {
        id: "msg-short-assistant",
        sessionID: "sess-short-height",
        role: "assistant",
        parts: [
          {
            id: "part-short-assistant",
            type: "text",
            text: Array.from({ length: 14 }, (_item, index) => `old detail ${index + 1}`).join(
              "\n",
            ),
          },
        ],
      },
      {
        id: "msg-short-current",
        sessionID: "sess-short-height",
        role: "assistant",
        parts: [{ id: "part-short-current", type: "text", text: "CURRENT RESULT READY" }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(82, 10, () => render(state, richCapabilities()));
  const plain = stripAnsi(output);
  assertLineWidths(output, 82);
  assert.match(plain, /CURRENT RESULT READY/);
  assert.match(plain, /Enter to send/);
  assert.ok(plain.split("\n").some((line) => line.includes("tokens")));
  assert.doesNotMatch(plain, /earlier output hidden|更早的内容已隐藏/u);
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
  assertLineWidths(output, 52);
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
  assert.doesNotMatch(ansiLines[ansiTitleIndex - 1] ?? "", /\x1b\[48;2;16;19;20m/);
  assert.doesNotMatch(stripAnsi(ansi), /^─{8,}$/mu);
  assert.match(
    ansi,
    /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;16;19;20m \S/m,
  );
  assert.match(ansi, /Enter opens; Esc returns to chat/);
  assert.match(ansi, /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m.*> Model/m);
  assert.match(ansi, /Expand executed commands/);
  const ansiText = stripAnsi(ansi);
  assert.match(ansiText, /Language\s+en/);
  assert.match(ansiText, /Session type\s+coding/);
  assert.match(ansiText, /Validator\s+true/);
  assert.doesNotMatch(ansi, /Context messages/);
  assert.doesNotMatch(ansi, /\/config get|\/config set|\/model provider\/model/);
  assert.doesNotMatch(ansi, /\/model <provider\/model>|\/commands/);
  assert.doesNotMatch(ansi, /Enter to send/);
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
  assert.doesNotMatch(richLines[richTitleIndex - 1] ?? "", /\x1b\[48;2;16;19;20m/);
  assert.doesNotMatch(stripAnsi(rich), /^─{8,}$/mu);
  assert.match(rich, /\x1b\[48;2;16;19;20m/);
  assert.match(
    rich,
    /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;16;19;20m .*Session Settings/m,
  );
  assert.match(rich, /\x1b\[38;2;64;224;208m> Model\s+\x1b\[0m.*gpt-5\.5/);
  const richSettingInstructionLine = richLines.find((line) =>
    stripAnsi(line).includes("Expand executed commands"),
  );
  assert.ok(richSettingInstructionLine);
  assert.match(richSettingInstructionLine, /\x1b\[38;2;64;224;208m {2}Expand executed commands/);
  assert.match(richSettingInstructionLine, /false/);
  const richLanguageLine = richLines.find((line) => stripAnsi(line).includes("Language"));
  assert.ok(richLanguageLine);
  assert.match(stripAnsi(richLanguageLine), /Language\s+en/);
  const richSettingModelLine = richLines.find((line) => stripAnsi(line).includes("Model"));
  assert.ok(richSettingModelLine);
  assert.match(stripAnsi(richSettingModelLine), /Model/);
  assertWideMenuGap(richSettingModelLine, "Model", "gpt-5.5", 12);
  assert.doesNotMatch(rich, /\/config get|\/config set|\/model provider\/model/);
  assert.doesNotMatch(rich, /\/model <provider\/model>|\/commands|Enter to send/);
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
  assert.doesNotMatch(ansiLines[ansiTitleIndex - 1] ?? "", /\x1b\[48;2;16;19;20m/);
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
  assert.doesNotMatch(richLines[richTitleIndex - 1] ?? "", /\x1b\[48;2;16;19;20m/);
  assert.match(
    rich,
    /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;16;19;20m .*Help/m,
  );
  assert.match(rich, /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m.*\/chat/m);
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
  assert.doesNotMatch(expanded, /earlier output hidden|更早的内容已隐藏/u);
  assert.match(expanded, /◇ Commands/);
  assert.match(expanded, /\$ npm test/);
  const commandLine = expanded.split("\n").find((line) => stripAnsi(line).includes("Commands"));
  assert.ok(commandLine);
  assert.equal(stripAnsi(commandLine), "◇ Commands");
  assert.doesNotMatch(commandLine, /\x1b\[48;2;16;19;20m/);
  const npmTestLine = expanded.split("\n").find((line) => stripAnsi(line).includes("$ npm test"));
  assert.ok(npmTestLine);
  assert.match(stripAnsi(npmTestLine), /^└─ ✓ #1 shell_command completed\s+\$ npm test/u);
  assert.doesNotMatch(npmTestLine, /\x1b\[48;2;16;19;20m/);

  const collapsed = render(
    reducer(state, {
      type: "session-config",
      value: { show_command_instructions: false },
    }),
    richCapabilities(),
  );
  assert.match(collapsed, /◇ Commands/);
  assert.doesNotMatch(collapsed, /\$ npm test/);
});

test("render places composer at the bottom and reports its terminal cursor", () => {
  const session = { id: "sess-bottom-input", title: "Bottom Input", status: "idle" as const };
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session,
      messages: [
        {
          id: "msg-bottom-input",
          sessionID: "sess-bottom-input",
          role: "assistant",
          parts: [{ id: "part-bottom-input", type: "text", text: "Ready." }],
        },
      ],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
    }),
    { type: "composer", value: "hello" },
  );

  const rendered = withTerminalSize(80, 18, () => renderFrame(state, richCapabilities()));
  assertLineWidths(rendered.frame, 80);
  const lines = rendered.frame.split("\n");
  const composerIndex = lines.findIndex((line) => stripAnsi(line).includes("> hello"));
  const metaIndex = lines.findIndex((line) => stripAnsi(line).includes("tokens"));
  assert.ok(metaIndex >= 0);
  assert.ok(composerIndex > metaIndex, "composer should be below the meta/status line");
  assert.equal(composerIndex, lines.length - 2, "composer body should sit at the bottom edge");
  assert.deepEqual(rendered.cursor, { row: composerIndex + 1, column: 10 });
  assert.doesNotMatch(rendered.frame, /TURA_COMPOSER_CURSOR/);
});

test("render reports the composer cursor on the final visible input line", () => {
  const session = { id: "sess-full-composer", title: "Full Composer", status: "idle" as const };
  const composer = Array.from({ length: 9 }, (_item, index) => `输入行-${index + 1}`).join("\n");
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session,
      messages: [],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
    }),
    { type: "composer", value: composer },
  );

  const rendered = withTerminalSize(72, 10, () => renderFrame(state, richCapabilities()));
  const plain = stripAnsi(rendered.frame);
  assert.match(plain, /输入行-1/u);
  assert.match(plain, /输入行-9/u);
  assert.equal(rendered.cursor?.row, rendered.frame.split("\n").length - 1);
});

test("render keeps long multiline user text instead of truncating to the first line", () => {
  const session = { id: "sess-long-user", title: "Long User", status: "idle" as const };
  const tail = "用户尾部必须可见";
  const text = `${"第一段中文信息".repeat(12)}\n第二行继续说明 ${tail}`;
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-long-user",
        sessionID: session.id,
        role: "user",
        parts: [{ id: "part-long-user", type: "text", text }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(72, 30, () => render(state, richCapabilities()));
  assert.match(stripAnsi(output), new RegExp(tail, "u"));
  const secondLine = output.split("\n").find((line) => stripAnsi(line).includes("第二行继续说明"));
  assert.ok(secondLine);
  assert.ok(
    secondLine.includes(textSecondary),
    `second user line should keep secondary color: ${JSON.stringify(secondLine)}`,
  );
  assert.doesNotMatch(secondLine, /\x1b\[38;2;244;247;235m第二行/u);
});

// ─── Transcript viewport ────────────────────────────────────────────────────

test("transcript default view shows bottom content", () => {
  const session = { id: "sess-scroll-default", title: "Scroll Default", status: "idle" as const };
  const messages = Array.from({ length: 6 }, (_, i) => ({
    id: `msg-scroll-${i}`,
    sessionID: "sess-scroll-default",
    role: i % 2 === 0 ? ("user" as const) : ("assistant" as const),
    parts: [{ id: `part-scroll-${i}`, type: "text", text: `Message ${i + 1}` }],
  }));
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages,
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });
  const output = withTerminalSize(80, 18, () => stripAnsi(render(state, richCapabilities())));
  assert.match(output, /Message 6/, "latest message must be visible by default");
});

test("transcript render keeps full history and leaves viewport ownership to the terminal", () => {
  const session = { id: "sess-full-history", title: "Full History", status: "idle" as const };
  const messages = Array.from({ length: 130 }, (_, i) => ({
    id: `msg-full-${i}`,
    sessionID: "sess-full-history",
    role: i % 2 === 0 ? ("user" as const) : ("assistant" as const),
    parts: [
      {
        id: `part-full-${i}`,
        type: "text",
        text: `History marker ${String(i + 1).padStart(3, "0")}`,
      },
    ],
  }));
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages,
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });
  const output = withTerminalSize(80, 18, () => render(state, richCapabilities()));
  const plain = stripAnsi(output);
  assertLineWidths(output, 80);
  assert.match(plain, /History marker 001/);
  assert.match(plain, /History marker 130/);
  assert.ok(
    plain.indexOf("History marker 001") < plain.indexOf("History marker 130"),
    "history should remain in event order",
  );
  assert.ok(
    output.split("\n").length > 18,
    "render must not trim transcript history to the current terminal viewport",
  );
});

test("transcript render preserves terminal-owned history without app scroll state", () => {
  const session = { id: "sess-full-scroll", title: "Full Scroll", status: "idle" as const };
  const messages = Array.from({ length: 80 }, (_, index) => ({
    id: `msg-scroll-${index}`,
    sessionID: session.id,
    role: index % 2 === 0 ? ("user" as const) : ("assistant" as const),
    parts: [
      {
        id: `part-scroll-${index}`,
        type: "text",
        text: `Full scroll marker ${String(index + 1).padStart(2, "0")}`,
      },
    ],
  }));
  const state = {
    ...initialState("C:/repo"),
    session,
    sessions: [session],
    messages,
  };

  const output = withTerminalSize(80, 18, () => stripAnsi(render(state, richCapabilities())));

  assert.match(output, /Full scroll marker 01/);
  assert.match(output, /Full scroll marker 80/);
  assert.ok(
    output.indexOf("Full scroll marker 01") < output.indexOf("Full scroll marker 80"),
    "transcript history must stay in terminal-owned event order",
  );
});

// ─── Differential rendering (no full-screen clear in frame string) ─────────

test("renderFrame does not embed a screen-clear escape sequence", () => {
  const session = { id: "sess-no-clear", title: "No Clear", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-nc-1",
        sessionID: "sess-no-clear",
        role: "user" as const,
        parts: [{ id: "p-nc-1", type: "text", text: "test" }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });
  const { frame } = withTerminalSize(80, 20, () => renderFrame(state, richCapabilities()));
  // The frame must NOT start with or contain the full-screen-clear sequence
  // (\x1b[3J\x1b[2J\x1b[H). The draw() function handles the initial clear
  // separately; embedding it inside the frame string would cause every
  // differential repaint to flash.
  assert.doesNotMatch(
    frame,
    /\x1b\[2J/,
    "frame must not contain full-screen clear (causes flicker)",
  );
  assert.doesNotMatch(frame, /\x1b\[3J/, "frame must not contain scrollback-clear");
});
