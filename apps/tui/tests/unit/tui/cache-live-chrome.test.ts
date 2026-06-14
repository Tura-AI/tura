import assert from "node:assert/strict";
import test from "node:test";
import { setLanguage } from "../../../src/i18n.js";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { renderChatFrameParts, renderFrame } from "../../../src/tui/render.js";
import {
  transcriptLines,
  transcriptLiveLines,
  transcriptRenderLines,
} from "../../../src/tui/render/transcript.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { stripAnsi } from "../../../src/tui/render-terminal.js";

setLanguage("en");

test("rapid panel switching keeps composer and chrome out of transcript cache", () => {
  const session = { id: "sess-cache-switch", title: "Cache Switch", status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-cache-switch-user",
        sessionID: session.id,
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-cache-switch-user", type: "text", text: "USER_LIVE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [session],
  });
  state = reducer(state, { type: "composer", value: "DRAFT_INPUT_MARKER" });
  for (const action of [
    { type: "toggle-sessions" as const },
    { type: "toggle-sessions" as const },
    { type: "toggle-help" as const },
    { type: "toggle-help" as const },
    { type: "toggle-models" as const },
    { type: "toggle-models" as const },
  ]) {
    state = reducer(state, action);
    renderFrame(state, richCapabilities());
  }

  const cache = stripAnsi(transcriptLines(state, 100).join("\n"));
  const live = stripAnsi(transcriptLiveLines(state, 100).join("\n"));
  const chrome = stripAnsi(renderChatFrameParts(state, richCapabilities()).chromeFrame);

  assert.doesNotMatch(cache, /DRAFT_INPUT_MARKER|tokens|Enter to send|thinking/i);
  assert.match(cache, /USER_LIVE_MARKER/);
  assert.doesNotMatch(live, /USER_LIVE_MARKER/);
  assert.doesNotMatch(live, /thinking/i);
  assert.match(chrome, /thinking/i);
});

test("chat frame rows classify command and gap while keeping thinking in chrome", () => {
  const session = { id: "sess-command-kinds", title: "Command Kinds", status: "busy" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-kind-cache",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_000,
        parts: [{ id: "part-command-kind-cache", type: "text", text: "CACHE_BEFORE_COMMAND" }],
      },
      {
        id: "msg-command-kind-live",
        sessionID: session.id,
        role: "assistant",
        created_at: 2_000,
        parts: [
          {
            id: "part-command-kind-live",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run slow",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [session],
  });

  const frame = renderChatFrameParts(state, richCapabilities());
  const commandRows = frame.liveRows.filter((line) => line.kind === "command");

  assert.equal(frame.liveRows[0]?.kind, "gap");
  assert.ok(commandRows.length >= 2);
  assert.match(stripAnsi(commandRows.map((line) => line.text).join("\n")), /npm run slow/);
  assert.doesNotMatch(stripAnsi(frame.liveFrame), /thinking/i);
  assert.match(stripAnsi(frame.chromeFrame), /thinking/i);
});

test("completed command transcript rows are rendered as command rows", () => {
  const session = {
    id: "sess-command-complete",
    title: "Command Complete",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-complete",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "part-command-complete",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                step_summary: "Run grouped checks",
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm test",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [session],
  });

  const commandRows = transcriptRenderLines(state, 100).filter((line) => line.kind === "command");
  const output = stripAnsi(commandRows.map((line) => line.text).join("\n"));

  assert.ok(commandRows.length >= 2);
  assert.match(output, /npm test/);
});

test("completed command call can cache rows even when a background command is still running", () => {
  const session = {
    id: "sess-command-call-complete-background",
    title: "Command Call Complete",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-call-complete-background",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "part-command-call-complete-background",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run dev",
                  },
                ],
              },
              output: {
                streamed_command_run_result: {
                  results: [
                    {
                      step: 1,
                      status: "running",
                      command_type: "shell_command",
                      command_line: "npm run dev",
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
    sessions: [session],
  });

  const commandRows = transcriptRenderLines(state, 100).filter((line) => line.kind === "command");
  const output = stripAnsi(commandRows.map((line) => line.text).join("\n"));

  assert.match(output, /shell_command running\s+\$ npm run dev/);
});

test("adjacent command parts render as separate command blocks", () => {
  const session = {
    id: "sess-adjacent-command-group",
    title: "Adjacent Command Group",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-adjacent-command-group",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "part-adjacent-command-a",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run first",
                  },
                ],
              },
            },
          },
          {
            id: "part-adjacent-command-b",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [
                  {
                    step: 2,
                    command_type: "shell_command",
                    command_line: "npm run second",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [session],
  });

  const output = stripAnsi(
    transcriptRenderLines(state, 100)
      .map((line) => line.text)
      .join("\n"),
  );

  assert.equal(output.match(/Commands(?!:)/g)?.length ?? 0, 2);
  assert.match(output, /npm run first/);
  assert.match(output, /npm run second/);
  assert.doesNotMatch(output, /Commands:\s*\d+/);
});

test("concurrent streaming deltas from other sessions do not enter active live output", () => {
  const active = { id: "sess-active-concurrent", title: "Active", status: "busy" as const };
  const other = { id: "sess-other-concurrent", title: "Other", status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: active,
    messages: [
      {
        id: "msg-active-user",
        sessionID: active.id,
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-active-user", type: "text", text: "ACTIVE_USER_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [active, other],
  });
  for (const [sessionID, marker] of [
    [other.id, "OTHER_STREAM_MARKER"],
    [active.id, "ACTIVE_STREAM_MARKER"],
    [other.id, "OTHER_STREAM_MARKER_2"],
  ] as const) {
    state = reducer(state, {
      type: "event",
      event: {
        directory: "C:/repo",
        payload: {
          type: "message.part.delta",
          properties: {
            session_id: sessionID,
            message_id: `msg-${sessionID}`,
            part_id: `part-${sessionID}`,
            field: "text",
            delta: marker,
          },
        },
      },
    });
  }

  const output = stripAnsi(renderFrame(state, richCapabilities()).frame);

  assert.match(output, /ACTIVE_USER_MARKER[\s\S]*ACTIVE_STREAM_MARKER/);
  assert.doesNotMatch(output, /OTHER_STREAM_MARKER/);
});

test("high-frequency live updates keep history cached once and active turn live-only", () => {
  const session = { id: "sess-live-stress", title: "Live Stress", status: "busy" as const };
  const history = Array.from({ length: 16 }, (_item, index) => {
    const turn = index + 1;
    const marker = String(turn).padStart(2, "0");
    return [
      {
        id: `msg-history-user-${turn}`,
        sessionID: session.id,
        role: "user" as const,
        created_at: turn * 10,
        parts: [{ id: `part-history-user-${turn}`, type: "text", text: `HISTORY_USER_${marker}` }],
      },
      {
        id: `msg-history-agent-${turn}`,
        sessionID: session.id,
        role: "assistant" as const,
        created_at: turn * 10 + 1,
        parts: [
          { id: `part-history-agent-${turn}`, type: "text", text: `HISTORY_AGENT_${marker}` },
        ],
      },
    ];
  }).flat();
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      ...history,
      {
        id: "msg-live-stress-user",
        sessionID: session.id,
        role: "user",
        created_at: 10_000,
        parts: [{ id: "part-live-stress-user", type: "text", text: "LIVE_USER_STRESS" }],
      },
    ],
    permissions: [],
    sessions: [session],
  });
  state = reducer(state, { type: "composer", value: "LIVE_COMPOSER_STRESS" });
  for (let index = 0; index < 40; index += 1) {
    state = reducer(state, {
      type: "event",
      event: {
        directory: "C:/repo",
        payload: {
          type: "message.part.delta",
          properties: {
            session_id: session.id,
            message_id: "msg-live-stress-agent",
            part_id: "part-live-stress-agent",
            field: "text",
            delta: `LIVE_AGENT_CHUNK_${index};`,
          },
        },
      },
    });
    if (index % 5 === 0) {
      state = reducer(reducer(state, { type: "toggle-settings" }), { type: "toggle-settings" });
    }
    state = reducer(state, { type: "tick" });
    renderFrame(state, richCapabilities());
  }

  const frame = renderChatFrameParts(state, richCapabilities());
  const cache = stripAnsi(frame.cacheFrame);
  const live = stripAnsi(frame.liveFrame);
  const chrome = stripAnsi(frame.chromeFrame);

  assert.equal(markerCount(cache, "HISTORY_USER_01"), 1);
  assert.equal(markerCount(cache, "HISTORY_AGENT_16"), 1);
  assert.equal(markerCount(cache, "LIVE_USER_STRESS"), 1);
  assert.doesNotMatch(cache, /LIVE_AGENT_CHUNK_|LIVE_COMPOSER_STRESS|thinking/i);
  assert.doesNotMatch(live, /LIVE_USER_STRESS/);
  assert.match(live, /LIVE_AGENT_CHUNK_0;[\s\S]*LIVE_AGENT_CHUNK_39;/);
  assert.doesNotMatch(live, /thinking/i);
  assert.match(chrome, /thinking/i);
  assert.match(chrome, /LIVE_COMPOSER_STRESS/);
  assert.doesNotMatch(chrome, /HISTORY_USER_01|LIVE_AGENT_CHUNK_39/);
});

test("completed and error turns do not duplicate chrome or stale thinking in cache", () => {
  const session = { id: "sess-error-cache", title: "Error Cache", status: "error" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-error-user",
        sessionID: session.id,
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-error-user", type: "text", text: "ERROR_USER_MARKER" }],
      },
      {
        id: "msg-error-agent",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_001,
        parts: [{ id: "part-error-agent", type: "text", text: "ERROR_AGENT_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [session],
  });

  const frame = renderChatFrameParts(
    { ...state, composer: "ERROR_COMPOSER_MARKER" },
    richCapabilities(),
  );
  const cache = stripAnsi(frame.cacheFrame);
  const live = stripAnsi(frame.liveFrame);
  const chrome = stripAnsi(frame.chromeFrame);

  assert.equal(markerCount(cache, "ERROR_USER_MARKER"), 1);
  assert.equal(markerCount(cache, "ERROR_AGENT_MARKER"), 1);
  assert.doesNotMatch(cache, /ERROR_COMPOSER_MARKER|tokens|thinking/i);
  assert.doesNotMatch(live, /thinking|ERROR_USER_MARKER|ERROR_AGENT_MARKER/i);
  assert.match(chrome, /ERROR_COMPOSER_MARKER/);
});

function markerCount(value: string, marker: string): number {
  return value.split(marker).length - 1;
}
