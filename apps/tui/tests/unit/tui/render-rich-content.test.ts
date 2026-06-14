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

process.env.TURA_LANG = "en";

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
