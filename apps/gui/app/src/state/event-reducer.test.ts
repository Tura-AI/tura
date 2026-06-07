import { describe, expect, test } from "bun:test";
import { applyGatewayEvent } from "./event-reducer";
import type { AppState } from "./global-store";
import { initialAppState, sessionTitle } from "./global-store";

describe("applyGatewayEvent", () => {
  test("upserts sessions and messages", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4096");
    state = applyGatewayEvent(state, {
      directory: "C:/repo",
      payload: {
        type: "session.created",
        properties: {
          sessionID: "s1",
          info: {
            id: "s1",
            title: "Build",
            status: "idle",
            time: { created: 1, updated: 1 },
          },
        },
      },
    });
    state = applyGatewayEvent(state, {
      directory: "C:/repo",
      payload: {
        type: "message.updated",
        properties: {
          sessionID: "s1",
          info: {
            id: "m1",
            sessionID: "s1",
            role: "assistant",
            parts: [{ id: "p1", type: "text", text: "hello" }],
            time: { created: 2, updated: 2 },
          },
        },
      },
    });

    expect(state.sessions).toHaveLength(1);
    expect(state.selectedSessionId).toBe("s1");
    expect(state.messagesBySession.s1[0]?.parts[0]?.text).toBe("hello");
  });

  test("applies part deltas", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4096"),
      messagesBySession: {
        s1: [
          {
            id: "m1",
            sessionID: "s1",
            role: "assistant",
            parts: [{ id: "p1", type: "text", text: "hel" }],
          },
        ],
      },
    };

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "s1",
          message_id: "m1",
          part_id: "p1",
          field: "text",
          delta: "lo",
        },
      },
    });

    expect(state.messagesBySession.s1[0]?.parts[0]?.text).toBe("hello");
  });

  test("keeps streaming deltas that arrive before full message hydration", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4096");

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "s1",
          message_id: "m1",
          part_id: "p1",
          field: "text",
          delta: "Thinking ",
        },
      },
    });
    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "s1",
          message_id: "m1",
          part_id: "p1",
          field: "text",
          delta: "through the task",
        },
      },
    });

    expect(state.messagesBySession.s1[0]?.id).toBe("m1");
    expect(state.messagesBySession.s1[0]?.parts[0]?.text).toBe("Thinking through the task");
  });

  test("adds delta parts to existing messages before part hydration", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4096"),
      messagesBySession: {
        s1: [
          {
            id: "m1",
            sessionID: "s1",
            role: "assistant",
            parts: [],
          },
        ],
      },
    };

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "s1",
          message_id: "m1",
          part_id: "p1",
          field: "content",
          delta: "Visible process text",
        },
      },
    });

    expect(state.messagesBySession.s1[0]?.parts[0]?.content).toBe("Visible process text");
  });

  test("applies gateway status objects", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4096"),
      sessions: [{ id: "s1", title: "Build", status: "idle" }],
    };

    state = applyGatewayEvent(state, {
      payload: {
        type: "session.status",
        properties: {
          sessionID: "s1",
          status: { type: "busy" },
        },
      },
    });

    expect(state.sessions[0]?.status).toBe("busy");
  });

  test("keeps local fallback name when gateway event has no session name", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4096"),
      sessions: [
        {
          id: "s1",
          name: "用户输入生成的临时会话名",
          session_display_name: "用户输入生成的临时会话名",
          status: "idle",
        },
      ],
    };

    state = applyGatewayEvent(state, {
      payload: {
        type: "session.updated",
        properties: {
          sessionID: "s1",
          info: {
            id: "s1",
            name: "",
            session_display_name: "",
            plan_summary: "",
            status: "busy",
            time: { created: 1, updated: 3 },
          },
        },
      },
    });

    expect(sessionTitle(state.sessions[0]!)).toBe("用户输入生成的临时会话名");
    expect(state.sessions[0]?.status).toBe("busy");
  });

  test("adds updated parts that arrive before message hydration", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4096");

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.updated",
        properties: {
          sessionID: "s1",
          part: {
            id: "p1",
            messageID: "m1",
            sessionID: "s1",
            type: "tool",
            tool: "runtime",
            state: { status: "running" },
          },
        },
      },
    });

    expect(state.messagesBySession.s1[0]?.id).toBe("m1");
    expect(state.messagesBySession.s1[0]?.parts[0]?.tool).toBe("runtime");
  });

  test("removes matching optimistic user prompt when gateway echoes real message", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4096"),
      messagesBySession: {
        s1: [
          {
            id: "prompt:s1:1",
            sessionID: "s1",
            role: "user",
            parts: [{ id: "prompt:s1:1:text", type: "text", text: "hello" }],
          },
        ],
      },
    };

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.updated",
        properties: {
          sessionID: "s1",
          info: {
            id: "m-real",
            sessionID: "s1",
            role: "user",
            parts: [{ id: "p1", type: "text", text: "hello" }],
            time: { created: 2, updated: 2 },
          },
        },
      },
    });

    expect(state.messagesBySession.s1).toHaveLength(1);
    expect(state.messagesBySession.s1[0]?.id).toBe("m-real");
  });

  test("removes a deleted conversation and selects the next available session", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4096"),
      selectedSessionId: "s1",
      sessions: [
        { id: "s1", status: "idle", updated_at: 2 },
        { id: "s2", status: "idle", updated_at: 1 },
      ],
      messagesBySession: {
        s1: [{ id: "m1", sessionID: "s1", role: "user", parts: [] }],
        s2: [{ id: "m2", sessionID: "s2", role: "assistant", parts: [] }],
      },
      todosBySession: {
        s1: [{ id: "todo", content: "old", status: "pending" }],
      },
    };

    state = applyGatewayEvent(state, {
      payload: {
        type: "session.deleted",
        properties: {
          sessionID: "s1",
        },
      },
    });

    expect(state.sessions.map((session) => session.id)).toEqual(["s2"]);
    expect(state.messagesBySession.s1).toBeUndefined();
    expect(state.todosBySession.s1).toBeUndefined();
    expect(state.selectedSessionId).toBe("s2");
  });

  test("hydrates assistant messages over previously streamed placeholder parts", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4096");

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "s1",
          message_id: "m1",
          part_id: "tool",
          field: "content",
          delta: "running",
        },
      },
    });
    state = applyGatewayEvent(state, {
      payload: {
        type: "message.updated",
        properties: {
          sessionID: "s1",
          info: {
            id: "m1",
            sessionID: "s1",
            role: "assistant",
            parts: [
              { id: "tool", type: "tool", tool: "shell_command", state: { status: "completed" } },
              { id: "final", type: "text", text: "done" },
            ],
          },
        },
      },
    });

    expect(state.messagesBySession.s1).toHaveLength(1);
    expect(state.messagesBySession.s1[0]?.parts.map((part) => part.id)).toEqual(["tool", "final"]);
    expect(state.messagesBySession.s1[0]?.parts[1]?.text).toBe("done");
  });
});
