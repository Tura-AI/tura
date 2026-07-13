import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render } from "../../../src/tui/render.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { stripAnsi } from "../../../src/tui/render-terminal.js";
import {
  providerEnums,
  withTerminalSize,
  assertLineWidths,
  assertWideMenuGap,
} from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

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
    /\x1b\[48;2;20;23;24m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;20;23;24m/,
  );
  assert.doesNotMatch(transcript, /(?:assistant|user|system)/);
  assert.doesNotMatch(transcript, /\[runtime:/);
  assert.match(transcript, /permission/);
  assert.match(transcript, /question/);
  assert.match(stripAnsi(transcript), /> Enter: send/);
  assert.doesNotMatch(transcript, /\x1b\[48;2;24;27;28m/);

  state = reducer(state, { type: "toggle-models" });
  assert.match(render(state, richCapabilities()), /openai\/gpt-5\.5/);

  state = reducer(state, { type: "toggle-models" });
  state = reducer(state, { type: "toggle-sessions" });
  const sessions = render(state, richCapabilities());
  const sessionLines = stripAnsi(sessions).split("\n");
  assert.equal(sessionLines[0], "C:/repo");
  assert.match(sessions, /Work/);
  assert.match(sessions, /New session/);
  assert.match(sessions, /> New session/);
  assert.match(sessions, /System ready/);
  assert.match(sessions, /Shift\+Enter copy context/);
  assert.match(sessions, /Delete remove/);
  assert.match(sessions, /─── .*Sessions.* ─────────/);
  assert.match(stripAnsi(sessions), /session select page 1\/1/);
  assert.doesNotMatch(sessions, /> Work/);
  assert.doesNotMatch(sessions, /\/resume <id>/);
  assert.match(sessions, /\x1b\[48;2;20;23;24m/);
  assert.doesNotMatch(sessions, /Enter: send/);
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

  test("sessions panel keeps question animation on the active session until status changes", () => {
    const question = {
      id: "sess-question",
      session_display_name: "Needs Input",
      status: "idle" as const,
      message_count: 2,
      task_management: { status: "question" },
    };
    const base = reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: question,
      messages: [],
      permissions: [],
      sessions: [question],
    });

    test("sessions panel uses busy animation while any command in the session is running", () => {
      const commandSession = {
        id: "sess-command",
        session_display_name: "Command Chat",
        status: "idle" as const,
        message_count: 1,
      };
      const base = reducer(initialState("C:/repo"), {
        type: "hydrate",
        session: commandSession,
        messages: [],
        permissions: [],
        sessions: [commandSession],
      });
      const state = {
        ...base,
        sessionsOpen: true,
        thinkingFrame: 0,
        commandStatesBySession: {
          "sess-command": {
            first: { status: "completed", eventSeq: 2 },
            second: { status: "running", eventSeq: 1 },
          },
        },
      };

      assert.match(stripAnsi(render(state, richCapabilities())), /Command Chat ◇/);
      const completed = {
        ...state,
        commandStatesBySession: {
          "sess-command": {
            first: { status: "completed", eventSeq: 2 },
            second: { status: "completed", eventSeq: 2 },
          },
        },
      };
      assert.doesNotMatch(stripAnsi(render(completed, richCapabilities())), /Command Chat [◇◆◈]/u);
    });
    const state = { ...base, sessionsOpen: true, thinkingFrame: 0 };

    const first = stripAnsi(render(state, richCapabilities()));
    const next = stripAnsi(render({ ...state, thinkingFrame: 1 }, richCapabilities()));

    assert.match(first, /Needs Input \?/);
    assert.match(next, /Needs Input ‽/);

    const resolved = {
      ...question,
      task_management: { status: "done" },
    };
    const resolvedOutput = stripAnsi(
      render({ ...state, session: resolved, sessions: [resolved] }, richCapabilities()),
    );
    assert.doesNotMatch(resolvedOutput, /Needs Input [?!‽¿]/u);
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
  assert.doesNotMatch(plain, /Enter: send/);
});

test("sessions panel shows dedicated loading animation and hides unlocked controls", () => {
  const active = {
    id: "sess-loading-active",
    session_display_name: "Active Chat",
    status: "idle" as const,
    message_count: 1,
  };
  const loading = {
    id: "sess-loading-target",
    session_display_name: "Target Chat",
    status: "idle" as const,
    message_count: 2,
  };
  const state = {
    ...reducer(initialState("C:/repo"), {
      type: "hydrate",
      session: active,
      messages: [],
      permissions: [],
      sessions: [active, loading],
    }),
    sessionsOpen: true,
    selectedSessionIndex: 1,
    thinkingFrame: 10,
    sessionLoading: { sessionID: loading.id, title: loading.session_display_name },
  };

  const plain = stripAnsi(render(state, richCapabilities()));

  assert.match(plain, /Loading session Target Chat/);
  assert.doesNotMatch(plain, /Up\/Down select/);
  assert.doesNotMatch(plain, /Shift\+Enter copy context/);
  assert.doesNotMatch(plain, /Delete remove/);
});

test("startup session loading uses the session picker loading animation", () => {
  const state = {
    ...initialState("C:/repo"),
    thinkingFrame: 10,
    sessionLoading: { title: "Startup Chat" },
  };

  const plain = stripAnsi(render(state, richCapabilities()));

  assert.match(plain, /Loading session Startup Chat/);
  assert.doesNotMatch(plain, /> Enter: send/);
  assert.doesNotMatch(plain, /Up\/Down select/);
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
