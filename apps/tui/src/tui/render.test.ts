import assert from "node:assert/strict";
import test from "node:test";
import { assertDictionaryParity } from "../i18n.js";
import { initialState, reducer } from "./reducer.js";
import { render } from "./render.js";

process.env.TURA_LANG = "en";

test("TUI i18n dictionaries keep zh-CN and en keys in sync", () => {
  assert.doesNotThrow(() => assertDictionaryParity());
});

test("render includes core TUI panels without throwing", () => {
  const session = {
    id: "sess-1",
    title: "Work",
    directory: "C:/repo",
    status: "idle" as const,
    plan_summary: "Plan Work",
    task_management: {
      plan_summary: "Plan Work",
      task_summary: "Do Work",
      status: "question" as const,      start_at: "2026-05-25T08:30:00Z",
    },
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
    ],
    todos: [{ id: "todo-1", content: "Verify", status: "in_progress" }],
    permissions: [{ id: "perm-1", sessionID: "sess-1", permission: "shell" }],
    providers: {
      all: [{ id: "openai", name: "OpenAI", models: { "gpt-5.5": { id: "gpt-5.5", name: "gpt-5.5" } } }],
      default: { openai: "gpt-5.5" },
      connected: ["openai"],
    },
    sessions: [session],
  });
  state = reducer(state, { type: "questions", value: [{ id: "q-1", sessionID: "sess-1", question: "Proceed?" }] });

  const transcript = render(state);
  assert.match(transcript, /Tura/);
  assert.match(transcript, /assistant/);
  assert.match(transcript, /\[runtime: checked\]/);
  assert.match(transcript, /permission/);
  assert.match(transcript, /question/);

  state = reducer(state, { type: "toggle-models" });
  assert.match(render(state), /openai\/gpt-5\.5/);

  state = reducer(state, { type: "toggle-models" });
  state = reducer(state, { type: "toggle-plan" });
  const plan = render(state);
  assert.match(plan, /Plan/);
  assert.match(plan, /question/);
  assert.match(plan, /Plan Work/);
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
    todos: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [] },
    sessions: [session],
  });

  const transcript = render(state);
  assert.match(transcript, /\x1b\[1mBold\x1b\[0m/);
  assert.match(transcript, /\x1b\[3mItalic\x1b\[0m/);
  assert.match(transcript, /\x1b\[4mUnder\x1b\[0m/);
  assert.match(transcript, /\x1b\[9mGone\x1b\[0m/);
  assert.match(transcript, /\x1b\[36msrc\/App\.tsx:12\x1b\[0m/);
  assert.match(transcript, /Example/);
  assert.match(transcript, /https:\/\/example\.com/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.match(transcript, /\[media: C:\/tmp\/shot\.png\]/);
  assert.match(transcript, /\[media: https:\/\/example\.com\/shot\.png\]/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\/shot\.png\x1b\\/);
  assert.match(transcript, /\[react: 👍\]/);
  assert.match(transcript, /│ quoted/);
  assert.match(transcript, /\[code: python\]/);
  assert.doesNotMatch(transcript, /<b>|<\/code>|MEDIA:C:\/tmp\/shot\.png:MEDIA|EMOJI:react/);
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
    todos: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [] },
    sessions: [session],
  });

  const transcript = render(state);
  assert.match(transcript, /Frontend/);
  assert.match(transcript, /npm run verify:all/);
  assert.match(transcript, /\[media: C:\/tmp\/a\.png\]/);
  assert.doesNotMatch(transcript, /<b>|<\/b>|<code>|<\/code>|MEDIA:C:\/tmp\/a\.png:MEDIA/);
});
