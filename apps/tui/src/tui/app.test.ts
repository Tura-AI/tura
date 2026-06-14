import assert from "node:assert/strict";
import test from "node:test";
import type { Session } from "../types/session.js";
import { messageText } from "../types/session.js";
import {
  createAndSelectSession,
  draw,
  openSessionPicker,
  resetDrawState,
  submitPrompt,
} from "./app.js";
import { initialState, reducer, type AppAction, type AppState } from "./reducer.js";
import { plainCapabilities, richCapabilities } from "./capabilities.js";
import { clear as terminalClear } from "./render-terminal.js";

const activeSession: Session = {
  id: "sess-1",
  name: "Active",
  directory: "C:/repo",
  status: "idle",
  updated_at: 1,
  message_count: 2,
};

const otherSession: Session = {
  id: "sess-2",
  name: "Other",
  directory: "C:/repo",
  status: "idle",
  updated_at: 2,
  message_count: 3,
};

test("openSessionPicker returns immediately when remote session refresh is wedged", async () => {
  const client = {
    listSessions: () => new Promise<Session[]>(() => {}),
    listMessages: () => new Promise(() => {}),
  };
  const harness = stateHarness(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: activeSession,
      messages: [],
      permissions: [],
      sessions: [activeSession, otherSession],
    }),
  );

  await openSessionPicker(client as never, harness.getState, harness.dispatch);

  assert.equal(harness.getState().sessionsOpen, true);
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-2", "sess-1"],
  );
});

test("openSessionPicker clears the terminal before rendering the session picker", async () => {
  const client = {
    listSessions: () => new Promise<Session[]>(() => {}),
    listMessages: () => new Promise(() => {}),
  };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session: activeSession,
    messages: [
      {
        id: "msg-live",
        sessionID: "sess-1",
        role: "assistant",
        parts: [{ id: "part-live", type: "text", text: "OLD_CHAT_LIVE_MARKER" }],
      },
    ],
    permissions: [],
    sessions: [activeSession, otherSession],
  });

  const writes = await captureDrawWritesAsync(async (capturedWrites) => {
    let lastFrame = draw(state, richCapabilities(), "");
    capturedWrites.length = 0;
    await openSessionPicker(
      client as never,
      () => state,
      (action) => {
        state = reducer(state, action);
        lastFrame = draw(state, richCapabilities(), lastFrame);
      },
    );
  });

  const firstWrite = writes[0] ?? "";
  const output = writes.join("");
  const firstClearIndex = output.indexOf(terminalClear);
  const sessionPickerIndex = output.indexOf("Other");

  assert.ok(
    firstWrite.includes(terminalClear),
    "session picker should hard-clear before state paint",
  );
  assert.equal(
    regexCount(firstWrite, /\x1b\[2K\r\n/gu),
    0,
    "session picker clear must not fill scrollback with blank rows",
  );
  assert.ok(firstClearIndex >= 0, "session picker transition must emit a terminal clear");
  assert.ok(sessionPickerIndex > firstClearIndex, "session picker content must render after clear");
});

test("createAndSelectSession creates and selects a real session", async () => {
  const createdSession: Session = {
    id: "sess-created",
    name: "Created",
    directory: "C:/repo",
    status: "idle",
    updated_at: 4,
    message_count: 0,
  };
  const calls: unknown[] = [];
  const client = {
    createSession: async (payload: unknown) => {
      calls.push(payload);
      return createdSession;
    },
    listMessages: () => new Promise(() => {}),
    listProviders: () => new Promise(() => {}),
    getSessionConfig: () => new Promise(() => {}),
    listAgents: () => new Promise(() => {}),
    listPersonas: () => new Promise(() => {}),
    listSessions: () => new Promise(() => {}),
  };
  const harness = stateHarness(
    reducer(initialState("C:/repo"), {
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
      sessions: [activeSession],
    }),
  );

  await createAndSelectSession(client as never, harness.getState, harness.dispatch);

  assert.equal(calls.length, 1);
  assert.equal(harness.getState().session?.id, "sess-created");
  assert.equal(harness.getState().session?.draft, undefined);
  assert.deepEqual(harness.getState().messages, []);
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-1", "sess-created"],
  );
});

test("createAndSelectSession falls back to a draft when session creation fails", async () => {
  const client = {
    createSession: async () => {
      throw new Error("offline");
    },
  };
  const harness = stateHarness(
    reducer(initialState("C:/repo"), {
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
      sessions: [activeSession],
    }),
  );

  await createAndSelectSession(client as never, harness.getState, harness.dispatch);

  assert.equal(harness.getState().session?.draft, true);
  assert.match(harness.getState().session?.id ?? "", /^draft-session-/);
  assert.deepEqual(harness.getState().messages, []);
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-1"],
  );
});

test("submitPrompt promotes a draft session to a real session", async () => {
  const realSession: Session = {
    id: "sess-3",
    name: "Real",
    directory: "C:/repo",
    status: "idle",
    updated_at: 3,
  };
  const calls: Array<{ method: string; payload?: unknown; sessionID?: string }> = [];
  const client = {
    createSession: async (payload: unknown) => {
      calls.push({ method: "createSession", payload });
      return realSession;
    },
    sendPromptAsync: async (sessionID: string) => {
      calls.push({ method: "sendPromptAsync", sessionID });
    },
  };
  const draftClient = {
    createSession: async () => {
      throw new Error("offline");
    },
  };
  const harness = stateHarness(
    reducer(initialState("C:/repo"), {
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
      sessions: [activeSession],
      sessionConfig: {
        active_agent: "thinking",
        model: "codex/gpt-5.5",
        model_variant: "high",
        model_acceleration_enabled: true,
      },
    }),
  );

  await createAndSelectSession(draftClient as never, harness.getState, harness.dispatch);
  await submitPrompt(client as never, harness.getState, harness.dispatch, "hello");

  assert.deepEqual(
    calls.map((call) => call.method),
    ["createSession", "sendPromptAsync"],
  );
  assert.equal(calls[1].sessionID, "sess-3");
  assert.equal(harness.getState().session?.id, "sess-3");
  assert.equal(harness.getState().session?.draft, undefined);
  assert.equal(harness.getState().messages.length, 1);
  assert.equal(harness.getState().messages[0].role, "user");
  assert.equal(harness.getState().messages[0].sessionID, "sess-3");
  assert.equal(messageText(harness.getState().messages[0]), "hello");
  assert.deepEqual(
    harness.getState().sessions.map((session) => session.id),
    ["sess-1", "sess-3"],
  );
  assert.equal(harness.getState().status, "busy");
});

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

  assert.match(output, /^\x1b\[\?25l/);
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
  assert.match(output, /chat transcript[\s\S]*Active[\s\S]*回车输入/);
  assert.doesNotMatch(output, /\x1b\[999;1H/);
  assert.doesNotMatch(output, /\x1b7|\x1b8/);
  assert.match(output, /\x1b\[\d+(?:;\d+)?[HG]\x1b\[\?25h$/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
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
    writes.length = 0;
    draw(
      reducer(state, {
        type: "event",
        event: {
          directory: "C:/repo",
          payload: {
            type: "message.part.delta",
            properties: {
              session_id: "sess-1",
              message_id: "msg-plain-stream",
              part_id: "part-plain-stream",
              field: "text",
              delta: "streaming",
            },
          },
        },
      }),
      plainCapabilities(),
      previous,
    );
  });
  const output = writes.join("");

  assert.equal(output.includes(terminalClear), false);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.doesNotMatch(output, /plain transcript/);
  assert.match(output, /^\x1b\[\?25l/);
  assert.match(output, /streaming[\s\S]*Active[\s\S]*回车输入/);
  assert.doesNotMatch(output, /\x1b\[999;1H/);
  assert.doesNotMatch(output, /\x1b7|\x1b8/);
  assert.match(output, /\x1b\[\d+(?:;\d+)?[HG]\x1b\[\?25h$/);
  assert.doesNotMatch(output, /\x1b\[1;1H\x1b\[2K/);
  assert.match(output, /streaming/);
});

function captureDrawWrites(fn: (writes: string[]) => void): string[] {
  resetDrawState();
  const writes: string[] = [];
  const isTTY = Object.getOwnPropertyDescriptor(process.stdout, "isTTY");
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  const write = Object.getOwnPropertyDescriptor(process.stdout, "write");
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: 80 });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 20 });
  Object.defineProperty(process.stdout, "write", {
    configurable: true,
    value: (chunk: string | Uint8Array): boolean => {
      writes.push(typeof chunk === "string" ? chunk : chunk.toString());
      return true;
    },
  });
  try {
    fn(writes);
    return writes;
  } finally {
    resetDrawState();
    restoreProperty(process.stdout, "isTTY", isTTY);
    restoreProperty(process.stdout, "columns", columns);
    restoreProperty(process.stdout, "rows", rows);
    restoreProperty(process.stdout, "write", write);
  }
}

async function captureDrawWritesAsync(fn: (writes: string[]) => Promise<void>): Promise<string[]> {
  resetDrawState();
  const writes: string[] = [];
  const isTTY = Object.getOwnPropertyDescriptor(process.stdout, "isTTY");
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  const write = Object.getOwnPropertyDescriptor(process.stdout, "write");
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: 80 });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 20 });
  Object.defineProperty(process.stdout, "write", {
    configurable: true,
    value: (chunk: string | Uint8Array): boolean => {
      writes.push(typeof chunk === "string" ? chunk : chunk.toString());
      return true;
    },
  });
  try {
    await fn(writes);
    return writes;
  } finally {
    resetDrawState();
    restoreProperty(process.stdout, "isTTY", isTTY);
    restoreProperty(process.stdout, "columns", columns);
    restoreProperty(process.stdout, "rows", rows);
    restoreProperty(process.stdout, "write", write);
  }
}

function restoreProperty<T extends object>(
  target: T,
  key: keyof T,
  descriptor: PropertyDescriptor | undefined,
): void {
  if (descriptor) Object.defineProperty(target, key, descriptor);
  else Reflect.deleteProperty(target, key);
}

function regexCount(text: string, pattern: RegExp): number {
  return Array.from(text.matchAll(pattern)).length;
}

function stateHarness(initial: AppState): {
  getState: () => AppState;
  dispatch: (action: AppAction) => void;
} {
  let state = initial;
  return {
    getState: () => state,
    dispatch: (action) => {
      state = reducer(state, action);
    },
  };
}
