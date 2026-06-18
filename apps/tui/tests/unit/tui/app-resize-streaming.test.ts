import assert from "node:assert/strict";
import test from "node:test";
import { createResizeDrawGate, createTerminalResizeHandler, draw } from "../../../src/tui/app.js";
import { renderChatFrameParts } from "../../../src/tui/render.js";
import { initialState, reducer, type AppAction } from "../../../src/tui/reducer.js";
import { plainCapabilities, richCapabilities } from "../../../src/tui/capabilities.js";
import { clear as terminalClear } from "../../../src/tui/render-terminal.js";
import {
  activeSession,
  assertMutableRegionRepaintedWithoutClearBefore,
  captureDrawWrites,
  lastAbsoluteCursorBefore,
  regexCount,
  restoreProperty,
} from "./helpers/app-harness.js";

test("resize draw gate paints the entry snapshot and freezes until resize settles", () => {
  let drawCount = 0;
  let clearPendingCount = 0;
  let clearTimerCount = 0;
  let timeoutCallback: (() => void) | undefined;
  const gate = createResizeDrawGate({
    drawNow: () => {
      drawCount += 1;
    },
    clearPendingDraw: () => {
      clearPendingCount += 1;
    },
    resizePauseMs: 50,
    setTimeoutFn: (callback) => {
      timeoutCallback = callback;
      return 1 as unknown as ReturnType<typeof setTimeout>;
    },
    clearTimeoutFn: () => {
      clearTimerCount += 1;
    },
  });

  gate.enterResize();
  assert.equal(drawCount, 1, "entering resize must draw the current snapshot once");
  assert.equal(clearPendingCount, 1, "entering resize must clear pending stream redraws");
  assert.equal(gate.isFrozen(), true);

  gate.enterResize();
  assert.equal(drawCount, 1, "ongoing resize must not redraw cache/live/chrome");
  assert.equal(clearPendingCount, 2, "ongoing resize keeps pending stream redraws suppressed");
  assert.equal(clearTimerCount, 1, "ongoing resize extends the resize-settled timer");

  timeoutCallback?.();
  assert.equal(gate.isFrozen(), false);
  assert.equal(drawCount, 2, "settled resize must draw the latest state once");

  gate.enterResize();
  assert.equal(drawCount, 3, "a later resize starts a new frozen window");
  gate.dispose();
  assert.equal(gate.isFrozen(), false);
});

test("terminal resize handler enters resize freeze on every size change", () => {
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: 80 });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 20 });
  try {
    const actions: AppAction[] = [];
    let resizeCount = 0;
    let heightResizeCount = 0;
    const state = { ...initialState("C:/repo"), notice: "resize notice" };
    const onResize = createTerminalResizeHandler(
      () => state,
      (action) => actions.push(action),
      {
        onResize: () => (resizeCount += 1),
        onHeightResize: () => (heightResizeCount += 1),
      },
    );

    Object.defineProperty(process.stdout, "rows", { configurable: true, value: 40 });
    onResize();
    assert.deepEqual(actions, []);
    assert.equal(resizeCount, 1);
    assert.equal(heightResizeCount, 1);

    Object.defineProperty(process.stdout, "columns", { configurable: true, value: 100 });
    onResize();
    assert.deepEqual(actions, [{ type: "notice", value: "resize notice" }]);
    assert.equal(resizeCount, 2);
    assert.equal(heightResizeCount, 1);

    Object.defineProperty(process.stdout, "rows", { configurable: true, value: 24 });
    onResize();
    assert.deepEqual(actions, [{ type: "notice", value: "resize notice" }]);
    assert.equal(resizeCount, 3);
    assert.equal(heightResizeCount, 2);

    onResize();
    assert.equal(resizeCount, 3);
    assert.equal(heightResizeCount, 2);
  } finally {
    restoreProperty(process.stdout, "columns", columns);
    restoreProperty(process.stdout, "rows", rows);
  }
});

test("draw rewrites streaming chat updates without clearing terminal scrollback", () => {
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-plain-chat",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-plain-chat", type: "text", text: "plain transcript" }],
      },
    ],
    permissions: [],
    sessions: [activeSession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, plainCapabilities(), "");
    const previousLiveRegion = renderChatFrameParts(state, plainCapabilities()).liveRegionCursor;
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
              messageID: "msg-plain-stream",
              partID: "part-plain-stream",
              field: "text",
              delta: "streaming",
            },
          },
        },
      }),
      plainCapabilities(),
      previous,
    );
    assert.ok(previousLiveRegion);
    assertMutableRegionRepaintedWithoutClearBefore(writes.join(""), "streaming");
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.doesNotMatch(output, /plain transcript/);
  assert.match(output, /^\x1b\[\?25l/);
  assert.match(output, /streaming[\s\S]*Active[\s\S]*Enter: send/);
  assert.match(output, /\r\n/, "streaming redraw may append reservation rows");
  assert.doesNotMatch(output, /\x1b\[999;1H/);
  assert.doesNotMatch(output, /\x1b7|\x1b8/);
  assert.match(output, /\x1b\[\d+(?:;\d+)?[HG]\x1b\[\?25h$/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.match(output, /streaming/);
});

test("draw keeps every rendered live row mutable while appending blank reservation rows", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [],
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
          messageID: "msg-line-commit",
          partID: "part-line-commit",
          field: "text",
          delta: `LIVE_WRAP_START ${"a".repeat(55)}`,
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, plainCapabilities(), "");
    writes.length = 0;
    state = reducer(state, {
      type: "event",
      event: {
        directory: "C:/repo",
        payload: {
          type: "message.part.delta",
          properties: {
            sessionID: "sess-1",
            messageID: "msg-line-commit",
            partID: "part-line-commit",
            field: "text",
            delta: " LIVE_WRAP_SECOND",
          },
        },
      },
    });
    draw(state, plainCapabilities(), previous);
  });
  const output = writes.join("");
  const blankAppendIndex = output.indexOf("\x1b[u\r\n");
  const firstLiveIndex = output.indexOf("LIVE_WRAP_START");
  const pendingIndex = output.indexOf("LIVE_WRAP_SECOND");
  const chromeIndex = output.indexOf("Active", pendingIndex);
  const firstLiveAnchor = lastAbsoluteCursorBefore(output, firstLiveIndex);
  const pendingAnchor = lastAbsoluteCursorBefore(output, pendingIndex);

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /\x1b\[\d+;1H\x1b\[J/u);
  assert.ok(blankAppendIndex >= 0, "growing live/chrome must append blank rows");
  assert.ok(
    blankAppendIndex < firstLiveIndex,
    "blank reservation rows must be appended before repainting live overlay",
  );
  assert.ok(pendingIndex > firstLiveIndex, "the pending live row must render after earlier live");
  assert.ok(firstLiveAnchor, "earlier live text must be painted with an absolute overlay cursor");
  assert.ok(pendingAnchor, "pending live text must be painted with an absolute overlay cursor");
  assert.ok(
    firstLiveAnchor.row <= pendingAnchor.row,
    "all live rows must remain inside the mutable overlay",
  );
  assert.ok(chromeIndex > pendingIndex, "chrome must still render after pending live");
});

test("draw keeps a single active live content row mutable until the next content row starts", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [],
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
          messageID: "msg-partial-live-row",
          partID: "part-partial-live-row",
          field: "text",
          delta: "PARTIAL_LIVE_ROW",
        },
      },
    },
  });

  const writes = captureDrawWrites(() => {
    draw(state, richCapabilities(), "");
  });
  const output = writes.join("");
  const markerIndex = output.indexOf("PARTIAL_LIVE_ROW");
  const saveCursorIndex = output.indexOf("\x1b[s");

  assert.ok(markerIndex >= 0, "partial live row must be rendered");
  assert.ok(saveCursorIndex >= 0, "chat draw must save the scrollback cursor before overlays");
  assert.ok(
    markerIndex > saveCursorIndex,
    "partial live row must stay in the mutable overlay until another content row exists",
  );
});

test("draw keeps only the active rendered command row mutable", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-running-command-live",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-running-command-live",
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
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(state, richCapabilities(), "");
    writes.length = 0;
    draw({ ...state, thinkingFrame: state.thinkingFrame + 1 }, richCapabilities(), previous);
  });
  const output = writes.join("");
  const commandIndex = output.indexOf("npm run slow");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /\x1b\[\d+;1H\x1b\[J/u);
  assert.ok(commandIndex >= 0, "running command must be redrawn as mutable live output");
  assert.equal(
    output.includes("\x1b[u\r\n"),
    false,
    "unchanged active rows must not be appended again",
  );
});

test("draw keeps message text mutable when a running command starts", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  let liveTextState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [],
    permissions: [],
    sessions: [busySession],
  });
  liveTextState = reducer(liveTextState, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-command-gap",
          partID: "part-command-gap-text",
          field: "text",
          delta: "COMMAND_GAP_PREFACE",
        },
      },
    },
  });
  const commandState = reducer(liveTextState, {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-command-gap",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-command-gap-text",
            type: "text",
            text: "COMMAND_GAP_PREFACE",
          },
          {
            id: "part-command-gap-run",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run command-gap",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(liveTextState, plainCapabilities(), "");
    writes.length = 0;
    draw(commandState, plainCapabilities(), previous);
  });
  const output = writes.join("");
  const markerIndex = output.indexOf("COMMAND_GAP_PREFACE");
  const commandIndex = output.indexOf("npm run command-gap");
  const markerAnchor = lastAbsoluteCursorBefore(output, markerIndex);
  const commandAnchor = lastAbsoluteCursorBefore(output, commandIndex);

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /\x1b\[\d+;1H\x1b\[J/u);
  assert.ok(markerIndex >= 0, "message text must be redrawn");
  assert.ok(commandIndex > markerIndex, "running command must render after message text");
  assert.ok(markerAnchor, "message text must stay in the mutable overlay");
  assert.ok(commandAnchor, "running command must stay in the mutable overlay");
  assert.ok(
    markerAnchor.row <= commandAnchor.row,
    "message and command must keep their live overlay order",
  );
});

test("draw keeps a running command row mutable when a following live row starts", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const commandMessage = {
    id: "msg-command-before-followup",
    sessionID: "sess-1",
    role: "assistant" as const,
    created_at: 1_000,
    parts: [
      {
        id: "part-command-before-followup",
        type: "tool",
        tool: "command_run",
        state: {
          status: "running",
          input: {
            commands: [
              {
                step: 1,
                command_type: "shell_command",
                command_line: "npm run long-lived",
              },
            ],
          },
        },
      },
    ],
  };
  const runningState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [commandMessage],
    permissions: [],
    sessions: [busySession],
  });
  const followedState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      commandMessage,
      {
        id: "msg-following-command",
        sessionID: "sess-1",
        role: "assistant",
        created_at: 1_001,
        parts: [
          {
            id: "part-following-command",
            type: "text",
            text: "FOLLOWING_ASSISTANT_CONTENT",
          },
        ],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(runningState, richCapabilities(), "");
    writes.length = 0;
    draw(followedState, richCapabilities(), previous);
  });
  const output = writes.join("");
  const commandIndex = output.indexOf("npm run long-lived");
  const followingIndex = output.indexOf("FOLLOWING_ASSISTANT_CONTENT");
  const commandAnchor = lastAbsoluteCursorBefore(output, commandIndex);
  const followingAnchor = lastAbsoluteCursorBefore(output, followingIndex);

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /\x1b\[\d+;1H\x1b\[J/u);
  assert.ok(commandIndex >= 0, "running command must render");
  assert.ok(followingIndex > commandIndex, "following content must render after the command block");
  assert.ok(commandAnchor, "running command must be painted as mutable overlay");
  assert.ok(followingAnchor, "following content must be painted as mutable overlay");
  assert.ok(
    commandAnchor.row <= followingAnchor.row,
    "command and following content must keep their live overlay order",
  );
  assert.equal(regexCount(output, /npm run long-lived/gu), 1);
  assert.equal(regexCount(output, /FOLLOWING_ASSISTANT_CONTENT/gu), 1);
});

test("draw keeps unchanged completed command rows terminal-owned during handoff", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const runningState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-background-command",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-background-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run dev",
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    sessions: [busySession],
  });
  const completedState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-background-command",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          {
            id: "part-background-command",
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
    sessions: [busySession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(runningState, richCapabilities(), "");
    assert.match(writes.join(""), /shell_command running[\s\S]*npm run dev/);
    writes.length = 0;
    draw(completedState, richCapabilities(), previous);
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /npm run dev/);
  assert.match(output, /Active[\s\S]*Enter: send/);
});

test("draw promotes finalized live as new cache without repainting fixed cache", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const idleSession = { ...activeSession, status: "idle" as const };
  const baseMessages = [
    {
      id: "msg-fixed-before-live",
      sessionID: "sess-1",
      role: "assistant" as const,
      parts: [{ id: "part-fixed-before-live", type: "text", text: "FIXED_CACHE_MARKER" }],
    },
    {
      id: "msg-live-user",
      sessionID: "sess-1",
      role: "user" as const,
      parts: [{ id: "part-live-user", type: "text", text: "LIVE_USER_MARKER" }],
    },
  ];
  let liveState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: baseMessages,
    permissions: [],
    sessions: [busySession],
  });
  liveState = reducer(liveState, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-live-assistant",
          partID: "part-live-assistant",
          field: "text",
          delta: "LIVE_STREAM_MARKER",
        },
      },
    },
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(liveState, richCapabilities(), "");
    const previousParts = renderChatFrameParts(liveState, richCapabilities());
    const previousLiveRegion = previousParts.liveRegionCursor;
    writes.length = 0;
    const finalizedState = reducer(liveState, {
      type: "hydrate",
      session: idleSession,
      messages: [
        ...baseMessages,
        {
          id: "msg-live-assistant",
          sessionID: "sess-1",
          role: "assistant",
          parts: [
            {
              id: "part-live-assistant",
              type: "text",
              text: "LIVE_STREAM_MARKER",
            },
          ],
        },
      ],
      permissions: [],
      sessions: [idleSession],
    });
    draw(finalizedState, richCapabilities(), previous);
    assert.ok(previousLiveRegion);
    const output = writes.join("");
    assert.doesNotMatch(
      output,
      /LIVE_STREAM_MARKER/,
      "finalized live text is already visible and must not be rewritten during handoff",
    );
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /FIXED_CACHE_MARKER/);
  assert.doesNotMatch(output, /LIVE_USER_MARKER/);
  assert.doesNotMatch(output, /LIVE_STREAM_MARKER/);
  assert.match(output, /Active[\s\S]*Enter: send/);
});

test("draw promotes mixed live text and command cache without blinking unchanged rows", () => {
  const busySession = { ...activeSession, status: "busy" as const };
  const idleSession = { ...activeSession, status: "idle" as const };
  const now = Date.now();
  const commandMessage = (status: "running" | "completed") => ({
    id: "msg-live-command-handoff",
    sessionID: "sess-1",
    role: "assistant" as const,
    created_at: now + 100_000,
    updated_at: now + 100_000,
    parts: [
      {
        id: "part-live-command-handoff",
        type: "tool" as const,
        tool: "command_run",
        state: {
          status,
          input: {
            commands: [
              {
                step: 1,
                command_type: "shell_command",
                command_line: "LIVE_HANDOFF_COMMAND_MARKER",
              },
            ],
          },
          output:
            status === "running"
              ? undefined
              : {
                  streamed_command_run_result: {
                    results: [
                      {
                        step: 1,
                        status: "completed",
                        command_type: "shell_command",
                        command_line: "LIVE_HANDOFF_COMMAND_MARKER",
                      },
                    ],
                  },
                },
        },
      },
    ],
  });
  let liveState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: busySession,
    messages: [
      {
        id: "msg-live-text-handoff",
        sessionID: "sess-1",
        role: "assistant",
        created_at: now,
        updated_at: now,
        parts: [],
      },
      commandMessage("running"),
    ],
    permissions: [],
    sessions: [busySession],
  });
  liveState = reducer(liveState, {
    type: "event",
    event: {
      directory: "C:/repo",
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: "sess-1",
          messageID: "msg-live-text-handoff",
          partID: "part-live-text-handoff",
          field: "text",
          delta: "LIVE_HANDOFF_TEXT_MARKER",
        },
      },
    },
  });

  const finalizedState = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: idleSession,
    messages: [
      {
        id: "msg-live-text-handoff",
        sessionID: "sess-1",
        role: "assistant",
        created_at: now,
        updated_at: now,
        parts: [{ id: "part-live-text-handoff", type: "text", text: "LIVE_HANDOFF_TEXT_MARKER" }],
      },
      commandMessage("completed"),
    ],
    permissions: [],
    sessions: [idleSession],
  });

  const writes = captureDrawWrites((writes) => {
    const previous = draw(liveState, richCapabilities(), "");
    assert.match(writes.join(""), /LIVE_HANDOFF_TEXT_MARKER/);
    assert.match(writes.join(""), /LIVE_HANDOFF_COMMAND_MARKER/);
    writes.length = 0;
    draw(finalizedState, richCapabilities(), previous);
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /\x1b\[\d+;1H\x1b\[J/u);
  assert.doesNotMatch(
    output,
    /LIVE_HANDOFF_TEXT_MARKER/,
    "already visible streamed text must be adopted into cache without repaint",
  );
  assert.ok(
    regexCount(output, /LIVE_HANDOFF_COMMAND_MARKER/gu) <= 1,
    "command row may update status once, but must not duplicate during handoff",
  );
  assert.match(output, /Active[\s\S]*Enter: send/);
});
