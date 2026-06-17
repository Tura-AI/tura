import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render } from "../../../src/tui/render.js";
import {
  ansiCapabilities,
  plainCapabilities,
  richCapabilities,
} from "../../../src/tui/capabilities.js";
import { stripAnsi } from "../../../src/tui/render-terminal.js";
import {
  providerEnums,
  withTerminalSize,
  assertFitsTerminal,
  assertWideMenuGap,
} from "./helpers/render-harness.js";
import type {
  Message,
  MessagePart,
  Session,
  SessionStatusValue,
} from "../../../src/types/session.js";
import { visibleTextWidth } from "../../../src/tui/render-terminal.js";

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

function textMessage(id: string, sessionID: string, text: string): Message {
  return {
    id,
    sessionID,
    role: "assistant",
    created_at: 1_000,
    updated_at: 1_000,
    time: { created: 1_000, updated: 1_000 },
    parts: [textPart(sessionID, id, `${id}-part`, text)],
  };
}

test("render applies communication style rich text without leaking protocol markup", () => {
  const session = sessionFixture("sess-rich", "Rich");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-rich",
        "sess-rich",
        "<b>Bold</b> <i>Italic</i> <u>Under</u> <s>Gone</s> <code>src/App.tsx:12</code>\n<a href='https://example.com'>Example</a>\n<span class='tg-spoiler'>secret</span>\n<blockquote>quoted</blockquote>\n<pre><code class='language-python'>print('hello')</code></pre>\n[MEDIA:C:/tmp/shot.png:MEDIA]\n[MEDIA:https://example.com/shot.png:MEDIA]\n[EMOJI:react:👍:EMOJI]",
      ),
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
    /Gone\x1b\[0m\x1b\[48;2;20;23;24m\x1b\[38;2;244;247;235m \x1b\[48;5;236m\x1b\[38;2;217;222;205m src\/App\.tsx:12 \x1b\[0m\x1b\[48;2;20;23;24m/,
  );
  assert.doesNotMatch(transcript, /\x1b\[36msrc\/App\.tsx:12\x1b\[0m/);
  assert.match(transcript, /Example/);
  assert.match(transcript, /https:\/\/example\.com/);
  assert.doesNotMatch(stripAnsi(transcript), /Example \(https:\/\/example\.com\)/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.doesNotMatch(transcript, /\[MEDIA:C:\/tmp\/shot\.png:MEDIA\]/);
  assert.match(transcript, /\x1b\[38;2;217;222;205mC:\/tmp\/shot\.png\x1b\[0m/);
  assert.match(transcript, /https:\/\/example\.com\/shot\.png/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\/shot\.png\x1b\\/);
  assert.match(transcript, /👍/u);
  assert.doesNotMatch(transcript, /\[EMOJI:/);
  assert.match(transcript, /\x1b\[48;5;234m\x1b\[38;2;217;222;205mquoted/);
  assert.doesNotMatch(stripAnsi(transcript), /│ quoted/);
  assert.match(transcript, /\x1b\[48;5;234m\x1b\[38;2;217;222;205mprint\('hello'\)/);
  assert.doesNotMatch(stripAnsi(transcript), /```/);
  const htmlCodeLines = transcript.split("\n");
  const htmlCodeLineIndex = htmlCodeLines.findIndex((line) =>
    stripAnsi(line).includes("print('hello')"),
  );
  assert.ok(htmlCodeLineIndex > 0);
  const htmlCodeTop = htmlCodeLines[htmlCodeLineIndex - 1] ?? "";
  const htmlCodeBottom = htmlCodeLines[htmlCodeLineIndex + 1] ?? "";
  assert.match(stripAnsi(htmlCodeTop), /^▏\s*$/u);
  assert.match(stripAnsi(htmlCodeBottom), /^▏\s*$/u);
  assert.ok(htmlCodeTop.includes("\x1b[48;5;234m"));
  assert.ok(htmlCodeBottom.includes("\x1b[48;5;234m"));
  assert.doesNotMatch(transcript, /<b>|<\/code>/);
});

test("render gracefully downgrades rich text across display levels", () => {
  const session = sessionFixture("sess-rich-levels", "Rich Levels");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-rich-levels",
        "sess-rich-levels",
        "<b>Bold</b> <code>src/App.tsx:12</code>\n<a href='https://example.com'>Example</a>\n<blockquote>quoted</blockquote>\n[MEDIA:https://example.com/shot.png:MEDIA]\n[EMOJI:react:👍:EMOJI]",
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = render(state, plainCapabilities());
  assert.match(plain, /Bold/);
  assert.match(plain, /src\/App\.tsx:12/);
  assert.match(plain, /Example/);
  assert.doesNotMatch(plain, /Example \(https:\/\/example\.com\)/);
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
  assert.match(ansi, /\x1b\[48;2;20;23;24m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;20;23;24m/);

  const rich = render(state, richCapabilities());
  assert.match(rich, /\x1b\[1mBold\x1b\[0m/);
  assert.match(rich, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.doesNotMatch(stripAnsi(rich), /Example \(https:\/\/example\.com\)/);
  assert.match(rich, /quoted/);
  assert.doesNotMatch(stripAnsi(rich), /│ quoted/);
  assert.match(rich, /👍/u);
  assert.doesNotMatch(rich, /\[EMOJI:/);
  assert.doesNotMatch(rich, /<b>|<\/code>/);
});

test("render supports markdown tables, markdown links, and local path access by level", () => {
  const session = sessionFixture("sess-md", "Markdown");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-md",
        "sess-md",
        "*Italic* _Em_ ~~Gone~~ ==Mark== [Site](https://example.com) [Local Doc](C:/repo/docs/readme.md)\n| Item | Path |\n| --- | --- |\n| Source | C:/repo/apps/tui |\n| Docs | [README](https://example.com/readme) |",
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = render(state, plainCapabilities());
  assert.match(plain, /Item\s+Path/);
  assert.match(plain, /README/);
  assert.doesNotMatch(plain, /README \(https:\/\/example\.com\/readme\)/);
  assert.doesNotMatch(plain, /Site \(https:\/\/example\.com\)/);
  assert.doesNotMatch(plain, /Local Doc \(C:\/repo\/docs\/readme\.md\)/);
  assert.doesNotMatch(plain, /\x1b\]8/);

  const rich = render(state, richCapabilities());
  const richText = stripAnsi(rich);
  assert.doesNotMatch(rich, /[┬┼┴]/u);
  assert.match(rich, /\x1b\[3mItalic\x1b\[0m/);
  assert.match(rich, /\x1b\[3mEm\x1b\[0m/);
  assert.match(rich, /\x1b\[9mGone\x1b\[0m/);
  assert.match(rich, /\x1b\[7mMark\x1b\[0m/);
  assert.match(richText, /Site/);
  assert.match(richText, /Local Doc/);
  assert.doesNotMatch(richText, /Site \(https:\/\/example\.com\)/);
  assert.doesNotMatch(richText, /Local Doc \(C:\/repo\/docs\/readme\.md\)/);
  assert.match(richText, /Item\s+│\s+Path/);
  assert.match(richText, /Source\s+│\s+C:\/repo\/apps\/tui/);
  assert.match(richText, /Item/);
  assert.match(richText, /Path/);
  assert.match(rich, /\x1b\[38;2;103;116;111m│\x1b\[38;2;217;222;205m/);
  assert.match(richText, /Source\s+│\s+C:\/repo\/apps\/tui/);
  assert.match(rich, /C:\/repo\/apps\/tui/);
  assert.match(rich, /\x1b\]8;;https:\/\/example\.com\/readme\x1b\\/);
  assert.match(rich, /\x1b\]8;;file:\/\/\/C:\/repo\/docs\/readme\.md\x1b\\/);
  assert.match(rich, /\x1b\]8;;file:\/\/\/C:\/repo\/apps\/tui\x1b\\/);

  const narrowRich = withTerminalSize(42, 24, () => render(state, richCapabilities()));
  assertFitsTerminal(narrowRich, 42, 24);
  assert.match(stripAnsi(narrowRich), /Source\s+│\s+C:\/repo\/apps\/tui/);
  assert.doesNotMatch(narrowRich, /[┬┼┴]/u);
  assert.match(narrowRich, /\x1b\]8/u);
  assert.doesNotMatch(narrowRich, /\x1b\[4m/u);
});

test("render preserves rich text blank paragraphs and full-width code block backgrounds", () => {
  const session = sessionFixture("sess-rich-blocks", "Rich Blocks");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-rich-blocks",
        session.id,
        "Paragraph before blank.\n\nParagraph after blank.\n> Quoted without a rail\n```ts\nconst width = 'full message text area';\n```\n| Kind | Result |\n| --- | --- |\n| Table | compact row |",
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const rich = withTerminalSize(96, 28, () => render(state, richCapabilities()));
  const stripped = stripAnsi(rich);
  const lines = stripped.split("\n");
  const before = lines.findIndex((line) => line.includes("Paragraph before blank."));
  const after = lines.findIndex((line) => line.includes("Paragraph after blank."));
  assert.ok(before >= 0 && after > before + 1, "blank paragraph line should remain in-message");
  assert.ok(lines.slice(before + 1, after).some((line) => /^▏\s*$/u.test(line)));
  assert.match(stripped, /Quoted without a rail/);
  assert.doesNotMatch(stripped, /│ Quoted without a rail/);
  assert.match(stripped, /Kind\s+│\s+Result/);
  assert.match(stripped, /Table\s+│\s+compact row/);
  const quoteIndex = lines.findIndex((line) => line.includes("Quoted without a rail"));
  const codeLineIndex = lines.findIndex((line) => line.includes("const width"));
  const tableHeaderIndex = lines.findIndex((line) => /Kind\s+│\s+Result/u.test(line));
  const tableRowIndex = lines.findIndex((line) => /Table\s+│\s+compact row/u.test(line));
  assert.ok(quoteIndex >= 0 && codeLineIndex > quoteIndex);
  assert.ok(tableHeaderIndex > codeLineIndex);
  assert.ok(tableRowIndex > tableHeaderIndex);
  assert.ok(lines.slice(quoteIndex + 1, codeLineIndex).some((line) => /^▏\s*$/u.test(line)));
  assert.ok(lines.slice(codeLineIndex + 1, tableHeaderIndex).some((line) => /^▏\s*$/u.test(line)));
  assert.doesNotMatch(stripped, /```(?:ts)?/);
  assert.match(lines[tableRowIndex + 1] ?? "", /^▏\s*$/u);

  const rawCodeLines = rich.split("\n");
  const codeLine = rawCodeLines.find((line) => stripAnsi(line).includes("const width"));
  assert.ok(codeLine);
  assert.ok(codeLine.includes("\x1b[48;5;234m"));
  assert.ok(
    visibleTextWidth(codeLine) >= 94,
    `code block background should fill the rich message text area: ${visibleTextWidth(codeLine)}`,
  );
  const rawCodeLineIndex = rawCodeLines.findIndex((line) => stripAnsi(line).includes("const width"));
  const codeTopBlank = rawCodeLines[rawCodeLineIndex - 1] ?? "";
  const codeBottomBlank = rawCodeLines[rawCodeLineIndex + 1] ?? "";
  assert.match(stripAnsi(codeTopBlank), /^▏\s*$/u);
  assert.match(stripAnsi(codeBottomBlank), /^▏\s*$/u);
  assert.ok(codeTopBlank.includes("\x1b[48;5;234m"));
  assert.ok(codeBottomBlank.includes("\x1b[48;5;234m"));
  assert.ok(visibleTextWidth(codeTopBlank) >= 94);
  assert.ok(visibleTextWidth(codeBottomBlank) >= 94);
});

test("render shows agent persona summary and persona panel", () => {
  const session = sessionFixture("sess-persona", "Persona", "idle", { agent: "fast" });
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
  assert.match(top, /Tab: sessions/);
  assert.match(top, /\/stop: cancel/);
  assert.doesNotMatch(top, /↑\/↓ view sessions/);
  assert.doesNotMatch(top, /[┌┐└┘]/u);
  assert.match(top, /^\x1b\[48;2;20;23;24m\x1b\[38;2;244;247;235m▏\x1b\[0m/m);
  assert.match(top, /^\x1b\[48;2;20;23;24m\x1b\[38;2;244;247;235m▏\x1b\[0m.*Enter: send/m);
  assert.doesNotMatch(top, /\x1b\[38;2;64;224;208m█\x1b\[0m/);
  assert.match(stripAnsi(top), /fast\s+│\s+tura/);

  state = reducer(state, { type: "toggle-personas" });
  const panel = render(state, richCapabilities());
  assert.match(panel, /Personas/);
  assert.match(panel, /> tura/);
  assert.match(panel, /\x1b\[48;2;20;23;24m/);
  assert.match(panel, /tura/);
  assert.match(panel, /calm technical collaborator/);
  assert.match(stripAnsi(panel), /concise, direct, fri/u);
  assert.match(stripAnsi(panel), /fri…|fri\.\.\./u);
  const personaLine = panel.split("\n").find((line) => stripAnsi(line).includes("> tura"));
  assert.ok(personaLine);
  assertWideMenuGap(personaLine, "tura", "current");
});
