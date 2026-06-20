import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render, renderChatFrameParts } from "../../../src/tui/render.js";
import { transcriptLines, transcriptLiveLines } from "../../../src/tui/render/transcript.js";
import { plainCapabilities, richCapabilities } from "../../../src/tui/capabilities.js";
import { stripAnsi } from "../../../src/tui/render-terminal.js";
import {
  providerEnums,
  withTerminalSize,
  withNow,
  assertLineWidths,
} from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

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
          sessionID: session.id,
          messageID: "msg-live-delta",
          partID: "part-live-delta",
          createdAt: 1_500,
          updatedAt: 1_500,
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
  assert.doesNotMatch(cachedHistory, /Enter: send|Enter: send/);
  assert.doesNotMatch(cachedHistory, /tokens/);

  const liveRows = stripAnsi(transcriptLiveLines(state, 80).join("\n"));
  assert.doesNotMatch(liveRows, /hello/);
  assert.match(liveRows, /LIVE_DELTA_MARKER/);
  assert.doesNotMatch(liveRows, /thinking/i);

  const chromeRows = stripAnsi(renderChatFrameParts(state, richCapabilities()).chromeFrame);
  assert.match(chromeRows, /thinking/i);
});

test("hidden command setting drops completed cache commands while keeping live commands visible", () => {
  const session = {
    id: "sess-hidden-cache-command",
    title: "Hidden Commands",
    status: "idle" as const,
  };
  const completed = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-hidden-command",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "part-hidden-text",
            type: "text",
            text: "Only this answer should remain in cache.",
          },
          {
            id: "part-hidden-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [{ command_type: "shell_command", command_line: "npm run hidden" }],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [session],
    sessionConfig: { show_command_instructions: false },
  });

  const cacheRows = stripAnsi(transcriptLines(completed, 100).join("\n"));
  assert.match(cacheRows, /Only this answer should remain in cache/);
  assert.doesNotMatch(cacheRows, /Commands|npm run hidden/);

  const runningSession = { ...session, status: "busy" as const };
  const running = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: runningSession,
    messages: [
      {
        id: "msg-live-command",
        sessionID: runningSession.id,
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "part-live-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [{ command_type: "shell_command", command_line: "npm run live" }],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [runningSession],
    sessionConfig: { show_command_instructions: false },
  });

  const liveRows = stripAnsi(transcriptLiveLines(running, 100).join("\n"));
  assert.match(liveRows, /Commands/);
  assert.match(liveRows, /npm run live/);
});

test("toggling command display rebuilds cache without hiding live command rows", () => {
  const session = {
    id: "sess-toggle-cache-command",
    title: "Toggle Commands",
    status: "busy" as const,
  };
  const base = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-toggle-cache-command",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "part-toggle-cache-text",
            type: "text",
            text: "Completed answer remains cached.",
          },
          {
            id: "part-toggle-cache-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [{ command_type: "shell_command", command_line: "npm run cached" }],
              },
            },
          },
        ],
      },
      {
        id: "msg-toggle-live-command",
        sessionID: session.id,
        role: "assistant",
        created_at: 2_000,
        parts: [
          {
            id: "part-toggle-live-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [{ command_type: "shell_command", command_line: "npm run live" }],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const first = renderChatFrameParts(base, richCapabilities());
  assert.match(stripAnsi(first.cacheFrame), /Completed answer remains cached/);
  assert.match(stripAnsi(first.cacheFrame), /npm run cached/);
  assert.match(stripAnsi(first.liveFrame), /npm run live/);

  const toggled = reducer(base, {
    type: "session-config",
    value: { show_command_instructions: false },
  });
  const next = renderChatFrameParts(toggled, richCapabilities(), { cache: first.cache });
  const cache = stripAnsi(next.cacheFrame);
  const live = stripAnsi(next.liveFrame);

  assert.notEqual(next.cache, first.cache);
  assert.match(cache, /Completed answer remains cached/);
  assert.doesNotMatch(cache, /Commands|npm run cached/);
  assert.match(live, /Commands/);
  assert.match(live, /npm run live/);
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
            sessionID: session.id,
            messageID: "msg-live-before-next-user",
            partID: "part-live-before-next-user",
            createdAt: 1_500,
            updatedAt: 1_500,
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

test("runtime message stays live while its command is still running", () => {
  const session = {
    id: "sess-runtime-live-command",
    title: "Runtime Live",
    status: "busy" as const,
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-runtime-live-user",
        sessionID: session.id,
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-runtime-live-user", type: "text", text: "run checks" }],
      },
    ],
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
          sessionID: session.id,
          messageID: "runtime-live.message",
          partID: "runtime-live.message",
          createdAt: 1_500,
          updatedAt: 1_500,
          field: "text",
          delta: "I will run the checks.",
        },
      },
    },
  });

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.updated",
        properties: {
          sessionID: session.id,
          createdAt: 1_500,
          updatedAt: 1_501,
          part: {
            id: "runtime-live.tool.command_run",
            sessionID: session.id,
            messageID: "runtime-live.message",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: { commands: [{ command_type: "shell_command", command_line: "npm test" }] },
            },
          },
        },
      },
    },
  });

  let cacheRows = stripAnsi(transcriptLines(state, 100).join("\n"));
  let liveRows = stripAnsi(transcriptLiveLines(state, 100).join("\n"));
  assert.doesNotMatch(cacheRows, /I will run the checks/);
  assert.match(liveRows, /I will run the checks/);
  assert.match(liveRows, /npm test/);

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.updated",
        properties: {
          sessionID: session.id,
          info: {
            id: "runtime-live.message",
            sessionID: session.id,
            role: "assistant",
            created_at: 1_500,
            updated_at: 1_600,
            parts: [
              {
                id: "runtime-live.message",
                sessionID: session.id,
                messageID: "runtime-live.message",
                type: "text",
                text: "I will run the checks.",
              },
            ],
          },
        },
      },
    },
  });

  cacheRows = stripAnsi(transcriptLines(state, 100).join("\n"));
  liveRows = stripAnsi(transcriptLiveLines(state, 100).join("\n"));
  assert.doesNotMatch(cacheRows, /I will run the checks/);
  assert.match(liveRows, /I will run the checks/);
  assert.match(liveRows, /npm test/);

  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.updated",
        properties: {
          sessionID: session.id,
          info: {
            id: "runtime-live.message",
            sessionID: session.id,
            role: "assistant",
            created_at: 1_500,
            updated_at: 1_700,
            parts: [
              {
                id: "runtime-live.message",
                sessionID: session.id,
                messageID: "runtime-live.message",
                type: "text",
                text: "I will run the checks.",
              },
              {
                id: "runtime-live.tool.command_run",
                sessionID: session.id,
                messageID: "runtime-live.message",
                type: "tool",
                tool: "command_run",
                state: {
                  status: "completed",
                  input: {
                    commands: [{ command_type: "shell_command", command_line: "npm test" }],
                  },
                },
              },
            ],
          },
        },
      },
    },
  });

  cacheRows = stripAnsi(transcriptLines(state, 100).join("\n"));
  liveRows = stripAnsi(transcriptLiveLines(state, 100).join("\n"));
  assert.match(cacheRows, /I will run the checks/);
  assert.match(cacheRows, /npm test/);
  assert.doesNotMatch(liveRows, /I will run the checks|npm test/);

  state = reducer(state, {
    type: "messages-incremental",
    sessionID: session.id,
    session: { ...session, status: "idle" },
    messages: [
      {
        id: "runtime-live.message",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_500,
        updated_at: 1_700,
        parts: [
          {
            id: "runtime-live.message",
            sessionID: session.id,
            messageID: "runtime-live.message",
            type: "text",
            text: "I will run the checks.",
          },
          {
            id: "runtime-live.tool.command_run",
            sessionID: session.id,
            messageID: "runtime-live.message",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [{ command_type: "shell_command", command_line: "npm test" }],
              },
            },
          },
        ],
      },
    ],
  });

  cacheRows = stripAnsi(transcriptLines(state, 100).join("\n"));
  liveRows = stripAnsi(transcriptLiveLines(state, 100).join("\n"));
  assert.match(cacheRows, /I will run the checks/);
  assert.match(cacheRows, /npm test/);
  assert.doesNotMatch(liveRows, /I will run the checks|npm test/);
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
          sessionID: session.id,
          messageID: "msg-live-tail",
          partID: "part-live-tail",
          createdAt: 2_500,
          updatedAt: 2_500,
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
  assert.match(plain, /Enter: send/);
  assert.ok(plain.split("\n").some((line) => line.trim() === "tura"));
  assert.doesNotMatch(plain, /earlier output hidden|earlier output hidden/u);
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
