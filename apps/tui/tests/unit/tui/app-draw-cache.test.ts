import assert from "node:assert/strict";
import test from "node:test";
import { draw } from "../../../src/tui/app.js";
import { drawChatChromeOverlay } from "../../../src/tui/draw.js";
import { renderChatFrameParts } from "../../../src/tui/render.js";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { clear as terminalClear, stripAnsi } from "../../../src/tui/render-terminal.js";
import {
  activeSession,
  otherSession,
  assertMutableRegionRepaintedWithoutClearBefore,
  captureDrawWrites,
  lastAbsoluteCursorBefore,
  restoreProperty,
} from "./helpers/app-harness.js";

test("draw clears terminal when opening the session picker over chat scrollback", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-old",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-old", type: "text", text: "old transcript" }],
      },
    ],
    permissions: [],
    sessions: [activeSession, otherSession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    const picker = reducer(state, {
      type: "sessions",
      value: [otherSession, activeSession],
      open: true,
    });
    draw(picker, richCapabilities(), previous);
  });

  assert.ok(writes.join("").includes(terminalClear));
});

test("draw clears terminal when the active session changes", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-active",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-active", type: "text", text: "active transcript" }],
      },
    ],
    permissions: [],
    sessions: [activeSession, otherSession],
  });

  const writes = captureDrawWrites((writes) => {
    const first = draw(state, richCapabilities(), "");
    const picker = reducer(state, {
      type: "sessions",
      value: [otherSession, activeSession],
      open: true,
    });
    const pickerFrame = draw(picker, richCapabilities(), first);
    writes.length = 0;
    const selected = reducer(picker, {
      type: "hydrate",
      session: otherSession,
      messages: [],
      permissions: [],
      sessions: [otherSession, activeSession],
    });
    draw(selected, richCapabilities(), pickerFrame);
  });

  assert.ok(writes.join("").includes(terminalClear));
});

test("draw keeps cursor hidden on pages without an input box", () => {
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [],
      permissions: [],
      sessions: [activeSession, otherSession],
    }),
    {
      type: "sessions",
      value: [otherSession, activeSession],
      open: true,
    },
  );

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");

  assert.match(output, /\x1b\[\?25l/);
  assert.doesNotMatch(output, /\x1b\[\?25h/);
});

test("draw can enter a selected session without rendering the previous session", () => {
  const active = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-active",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-active", type: "text", text: "ACTIVE_STALE_TRANSCRIPT" }],
      },
    ],
    permissions: [],
    sessions: [activeSession, otherSession],
  });
  const picker = reducer(active, {
    type: "sessions",
    value: [otherSession, activeSession],
    open: true,
  });

  const writes = captureDrawWrites((writes) => {
    const pickerFrame = draw(picker, richCapabilities(), "");
    writes.length = 0;
    const selected = reducer(picker, {
      type: "hydrate",
      session: otherSession,
      messages: [
        {
          id: "msg-other",
          sessionID: "sess-2",
          role: "assistant",
          parts: [{ id: "part-other", type: "text", text: "OTHER_SELECTED_TRANSCRIPT" }],
        },
      ],
      permissions: [],
      sessions: [otherSession, activeSession],
      closePanels: true,
    });
    draw(selected, richCapabilities(), pickerFrame);
  });
  const output = writes.join("");

  assert.match(output, /OTHER_SELECTED_TRANSCRIPT/);
  assert.doesNotMatch(output, /ACTIVE_STALE_TRANSCRIPT/);
  assert.doesNotMatch(output, /New session/);
});

test("draw writes chat as fixed history plus live output without screen-window repaint", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-chat",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-chat", type: "text", text: "chat transcript" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");

  assert.ok(output.includes(terminalClear));
  assert.match(output, /chat transcript/);
  assert.match(output, /chat transcript[\s\S]*Active[\s\S]*Enter: send/);
  assert.doesNotMatch(output, /\x1b\[999;1H/);
  assert.doesNotMatch(output, /\x1b7|\x1b8/);
  assert.match(output, /\x1b\[\d+(?:;\d+)?[HG]\x1b\[\?25h$/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
});

test("draw appends new cache lines once before live and chrome", () => {
  const initial = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-cache-base",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-cache-base", type: "text", text: "CACHE_BASE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });
  const appended = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      ...initial.messages,
      {
        id: "msg-cache-never-live",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-cache-never-live", type: "text", text: "CACHE_NEVER_LIVE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(initial, richCapabilities(), "");
    const previousLiveRegion = renderChatFrameParts(initial, richCapabilities()).liveRegionCursor;
    writes.length = 0;
    draw(appended, richCapabilities(), previous);
    assert.ok(previousLiveRegion);
    assertMutableRegionRepaintedWithoutClearBefore(writes.join(""), "CACHE_NEVER_LIVE_MARKER");
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /CACHE_BASE_MARKER/);
  assert.match(output, /CACHE_NEVER_LIVE_MARKER/);
  assert.match(output, /CACHE_NEVER_LIVE_MARKER[\s\S]*Active[\s\S]*Enter: send/);
  assert.doesNotMatch(
    output,
    /\x1b\[K\x1b\[0m\x1b\[K/u,
    "rich message rows must not clear the line tail again after resetting panel background",
  );
});

test("draw redraws cache live and chrome on terminal width resize", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-resize-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-resize-cache", type: "text", text: "RESIZE_CACHE_MARKER" }],
      },
      {
        id: "msg-resize-live-user",
        sessionID: "sess-1",
        role: "user",
        parts: [{ id: "part-resize-live-user", type: "text", text: "RESIZE_LIVE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-resize-live-assistant",
          partID: "part-resize-live-assistant",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: "RESIZE_STREAM_MARKER",
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    Object.defineProperty(process.stdout, "columns", { configurable: true, value: 100 });
    draw(state, richCapabilities(), previous);
  });
  const output = writes.join("");

  assert.ok(output.includes(terminalClear));
  assert.match(output, /RESIZE_CACHE_MARKER/);
  assert.match(output, /RESIZE_LIVE_MARKER/);
  assert.match(output, /RESIZE_STREAM_MARKER/);
  assert.match(output, /Active[\s\S]*Enter: send/);
});

test("draw ignores terminal height resize without rewriting", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-height-resize-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-height-resize-cache", type: "text", text: "HEIGHT_CACHE_MARKER" }],
      },
      {
        id: "msg-height-resize-user",
        sessionID: "sess-1",
        role: "user",
        parts: [{ id: "part-height-resize-user", type: "text", text: "HEIGHT_USER_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-height-resize-live",
          partID: "part-height-resize-live",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: "HEIGHT_STREAM_MARKER",
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    Object.defineProperty(process.stdout, "rows", { configurable: true, value: 40 });
    draw(state, richCapabilities(), previous);
  });
  const output = writes.join("");

  assert.equal(output, "");
});

test("draw force reset redraws cache live and chrome for resize snapshots", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-force-resize-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-force-resize-cache", type: "text", text: "FORCE_RESIZE_CACHE" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-force-resize-live",
          partID: "part-force-resize-live",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: "FORCE_RESIZE_LIVE",
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    Object.defineProperty(process.stdout, "rows", { configurable: true, value: 40 });
    draw(state, richCapabilities(), previous, { forceReset: true });
  });
  const output = writes.join("");

  assert.ok(output.includes(terminalClear));
  assert.match(output, /FORCE_RESIZE_CACHE/);
  assert.match(output, /FORCE_RESIZE_LIVE/);
  assert.match(output, /Active[\s\S]*Enter: send/);
});

test("draw appends completed live rows before painting chrome in the reservation tail", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-anchor-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-anchor-cache", type: "text", text: "CACHE_ANCHOR_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-anchor-live",
          partID: "part-anchor-live",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: "LIVE_ANCHOR_MARKER",
        },
      },
    },
  });

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");
  const cacheIndex = output.indexOf("CACHE_ANCHOR_MARKER");
  const liveIndex = output.indexOf("LIVE_ANCHOR_MARKER");
  const chromeIndex = output.indexOf("Active", liveIndex);
  const chromeAnchor = lastAbsoluteCursorBefore(output, chromeIndex);

  assert.ok(cacheIndex >= 0, "expected cache marker to be written");
  assert.ok(liveIndex > cacheIndex, "completed live rows must render after the cache is written");
  assert.ok(chromeIndex > liveIndex, "chrome must render after live");
  assert.ok(chromeAnchor, "chrome must start in the visible reservation tail");
  assert.ok(
    chromeAnchor.row > 1 && chromeAnchor.row <= 20,
    "chrome must start after live inside reservation tail",
  );
  assert.equal(
    output.indexOf("\x1b[1;1H\x1b[J", cacheIndex),
    -1,
    "full chat redraw must not clear cache with the mutable-region clear",
  );
});

test("draw renders chrome directly below cache when there is no live", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-no-live-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-no-live-cache", type: "text", text: "NO_LIVE_CACHE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });

  const parts = renderChatFrameParts(state, richCapabilities());
  const cacheLineCount = parts.cacheFrame ? parts.cacheFrame.split("\n").length : 0;
  const expectedChromeStartRow = Math.min(20, cacheLineCount + 1);
  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");
  const cacheIndex = output.indexOf("NO_LIVE_CACHE_MARKER");
  const chromeIndex = output.indexOf("Active", cacheIndex);
  const chromeAnchorIndex = output.lastIndexOf(`\x1b[${expectedChromeStartRow};1H`, chromeIndex);

  assert.equal(parts.liveFrame, "");
  assert.ok(cacheIndex >= 0, "expected cache marker to be written");
  assert.ok(chromeIndex > cacheIndex, "chrome must render after cache when live is empty");
  assert.ok(chromeAnchorIndex >= 0, "chrome must start immediately after cache when live is empty");
});

test("draw spills overflowing live rows into scrollback and keeps the tail mutable", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 12 });
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-live-overflow-cache",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-live-overflow-cache", type: "text", text: "LIVE_OVERFLOW_CACHE" }],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  state = reducer(state, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-live-overflow-stream",
          partID: "part-live-overflow-stream",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: Array.from({ length: 30 }, (_, index) => `LIVE_OVERFLOW_${index}`).join("\n"),
        },
      },
    },
  });

  try {
    const writes = captureDrawWrites(() => {
      draw(state, richCapabilities(), "");
    });
    const output = writes.join("");
    const saveCursorIndex = output.indexOf("\x1b[s");
    const spilledIndex = output.indexOf("LIVE_OVERFLOW_0");
    const tailIndex = output.indexOf("LIVE_OVERFLOW_29");
    const tailAnchor = lastAbsoluteCursorBefore(output, tailIndex);

    assert.ok(saveCursorIndex >= 0, "chat draw must save the scrollback cursor");
    assert.ok(spilledIndex >= 0, "overflowing live prefix must be written to scrollback");
    assert.ok(
      spilledIndex < saveCursorIndex,
      "overflowing live prefix must be appended before overlay painting",
    );
    assert.ok(tailIndex > saveCursorIndex, "live tail must remain mutable after the saved cursor");
    assert.ok(tailAnchor, "live tail must be painted through the mutable overlay");
    assert.match(output, /Active[\s\S]*Enter: send/);
  } finally {
    restoreProperty(process.stdout, "rows", rows);
  }
});

test("draw promotes spilled live through cache handoff without rewriting visible live rows", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const idleSession = { ...activeSession, status: "idle" as const };
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 12 });
  let liveState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [],
    permissions: [],
    sessions: [busySession],
  });
  const liveText = Array.from({ length: 30 }, (_, index) => `HANDOFF_OVERFLOW_${index}`).join("\n");
  liveState = reducer(liveState, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-handoff-overflow",
          partID: "part-handoff-overflow",
          createdAt: 1,
          updatedAt: 2,
          field: "text",
          delta: liveText,
        },
      },
    },
  });
  const cacheState = reducer(liveState, {
    type: "hydrate",
    session: idleSession,
    messages: [
      {
        id: "msg-handoff-overflow",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-handoff-overflow", type: "text", text: liveText }],
      },
    ],
    permissions: [],
    sessions: [idleSession],
  });

  try {
    const writes = captureDrawWrites((writes) => {
      const previous = draw(liveState, richCapabilities(), "");
      writes.length = 0;
      draw(cacheState, richCapabilities(), previous);
    });
    const output = writes.join("");

    assert.equal(output.includes(terminalClear), false);
    assert.match(output, /Active[\s\S]*Enter: send/);
  } finally {
    restoreProperty(process.stdout, "rows", rows);
  }
});

test("draw renders chrome in reserved scrollback tail when cache fills the viewport", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: Array.from({ length: 5 }, (_, index) => ({
      id: `msg-cache-almost-full-${index}`,
      sessionID: "sess-1",
      role: "assistant" as const,
      parts: [
        {
          id: `part-cache-almost-full-${index}`,
          type: "text" as const,
          text: `CACHE_ALMOST_FULL_MARKER_${index}`,
        },
      ],
    })),
    permissions: [],
    sessions: [activeSession],
  });

  const parts = renderChatFrameParts(state, richCapabilities());
  const cacheLineCount = parts.cacheFrame ? parts.cacheFrame.split("\n").length : 0;
  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");

  assert.equal(cacheLineCount, 19);
  assert.match(output, /CACHE_ALMOST_FULL_MARKER_4/);
  assert.match(output, /Active[\s\S]*Enter: send/);
  assert.match(output, /\r\n/, "chrome reservation must add blank scrollback rows");
});

test("draw appends reservation rows so live and chrome remain visible after full cache", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: Array.from({ length: 30 }, (_, index) => ({
      id: `msg-cache-fill-${index}`,
      sessionID: "sess-1",
      role: "assistant" as const,
      parts: [
        {
          id: `part-cache-fill-${index}`,
          type: "text" as const,
          text: `CACHE_FILL_MARKER_${index}`,
        },
      ],
    })),
    permissions: [],
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    draw(
      reducer(state, {
        type: "event",
        event: {
          directory: "C:/repo",
          payload: {
            type: "message.part.delta",
            properties: {
              sessionID: "sess-1",
              messageID: "msg-cache-fill-live",
              partID: "part-cache-fill-live",
              createdAt: 1,
              updatedAt: 2,
              field: "text",
              delta: "FULL_CACHE_LIVE_MARKER",
            },
          },
        },
      }),
      richCapabilities(),
      previous,
    );
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /CACHE_FILL_MARKER_0/);
  assert.match(output, /\r\n/, "live/chrome reservation must extend scrollback");
  assert.match(output, /FULL_CACHE_LIVE_MARKER/);
  assert.match(output, /Active[\s\S]*Enter: send/);
});

test("chat chrome keeps the composer row stable when thinking stops", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const idleSession = { ...activeSession, status: "idle" as const };
  const busyState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [],
    permissions: [],
    sessions: [busySession],
  });
  const idleState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: idleSession,
    messages: [],
    permissions: [],
    sessions: [idleSession],
  });

  const busyChrome = renderChatFrameParts(busyState, richCapabilities());
  const idleChrome = renderChatFrameParts(idleState, richCapabilities());
  const busyPlain = stripAnsi(busyChrome.chromeFrame);
  const idlePlain = stripAnsi(idleChrome.chromeFrame);
  const busyLines = busyChrome.chromeFrame.split("\n");
  const idleLines = idleChrome.chromeFrame.split("\n");
  const busyTitleRow = busyLines.findIndex((line) => line.includes("Active"));
  const idleTitleRow = idleLines.findIndex((line) => line.includes("Active"));

  assert.match(busyPlain, /thinking/);
  assert.doesNotMatch(idlePlain, /thinking/);
  assert.equal(idleLines.length, busyLines.length);
  assert.equal(idleTitleRow, busyTitleRow);
  assert.equal(idleChrome.chromeCursor?.row, busyChrome.chromeCursor?.row);
});

test("idle chrome overlay repaints stale busy chrome without clearing first", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const idleSession = { ...activeSession, status: "idle" as const };
  const cacheMessages = Array.from({ length: 30 }, (_item, index) => ({
    id: `msg-idle-overlay-cache-${index}`,
    sessionID: "sess-1",
    role: "assistant" as const,
    parts: [
      {
        id: `part-idle-overlay-cache-${index}`,
        type: "text" as const,
        text: `IDLE_OVERLAY_CACHE_${index}`,
      },
    ],
  }));
  const commandMessage = {
    id: "msg-idle-overlay-command",
    sessionID: "sess-1",
    role: "assistant" as const,
    parts: [
      {
        id: "part-idle-overlay-command",
        type: "tool" as const,
        tool: "command_run",
        state: {
          status: "completed",
          input: {
            commands: [
              {
                step: 1,
                command_type: "shell_command",
                command_line: "IDLE_OVERLAY_COMMAND",
              },
            ],
          },
        },
      },
    ],
  };
  const busyState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: cacheMessages,
    permissions: [],
    sessions: [busySession],
  });
  const commandState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [...cacheMessages, commandMessage],
    permissions: [],
    sessions: [busySession],
  });
  const idleState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: idleSession,
    messages: [...cacheMessages, commandMessage],
    permissions: [],
    sessions: [idleSession],
  });

  const writes = captureDrawWrites((writes) => {
    let previous = draw(busyState, richCapabilities(), "");
    writes.length = 0;
    previous = draw(commandState, richCapabilities(), previous);
    assert.match(writes.join(""), /IDLE_OVERLAY_COMMAND/);
    writes.length = 0;
    drawChatChromeOverlay(idleState, richCapabilities(), previous);
  });
  const output = writes.join("");
  const titleIndex = output.indexOf("Active");

  assert.doesNotMatch(output, /\x1b\[\d+;1H\x1b\[J/u);
  assert.ok(titleIndex >= 0, "idle chrome must be written");
  assert.doesNotMatch(output, /thinking/);
});
