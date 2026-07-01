import { describe, expect, test } from "bun:test";
import { applyGatewayEvent } from "../../../app/src/state/event-reducer";
import type { AppState } from "../../../app/src/state/global-store";
import { initialAppState, sessionTitle } from "../../../app/src/state/global-store";
import { toolRecords } from "../../../app/src/conversation/message-tools";

describe("applyGatewayEvent", () => {
  test("upserts sessions and messages", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");
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
    expect(state.selectedSessionId).toBeUndefined();
    expect(state.messagesBySession.s1[0]?.parts[0]?.text).toBe("hello");
  });

  test("does not activate a session from background create or update events", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4126"),
      selectedSessionId: "s-active",
      sessions: [{ id: "s-active", status: "idle", name: "Active" }],
    };

    state = applyGatewayEvent(state, {
      directory: "C:/repo",
      payload: {
        type: "session.created",
        properties: {
          sessionID: "s-background",
          info: {
            id: "s-background",
            name: "Background",
            status: "busy",
            time: { created: 1, updated: 1 },
          },
        },
      },
    });

    expect(state.selectedSessionId).toBe("s-active");

    state = {
      ...state,
      selectedSessionId: undefined,
    };
    state = applyGatewayEvent(state, {
      directory: "C:/repo",
      payload: {
        type: "session.updated",
        properties: {
          sessionID: "s-background",
          info: {
            id: "s-background",
            name: "Background updated",
            status: "idle",
            time: { created: 1, updated: 2 },
          },
        },
      },
    });

    expect(state.selectedSessionId).toBeUndefined();
    expect(state.sessions.find((session) => session.id === "s-background")?.name).toBe(
      "Background updated",
    );
  });

  test("applies part deltas", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4126"),
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
    let state: AppState = initialAppState("http://127.0.0.1:4126");

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

  test("does not let full message hydration rewrite streamed text deltas", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "s1",
          message_id: "m1",
          part_id: "p1",
          field: "text",
          delta: "already drawn",
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
            parts: [{ id: "p1", type: "text", text: "stale hydration" }],
          },
        },
      },
    });

    expect(state.messagesBySession.s1[0]?.parts[0]?.text).toBe("already drawn");
  });

  test("does not let part hydration rewrite streamed content deltas", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.delta",
        properties: {
          session_id: "s1",
          message_id: "m1",
          part_id: "p1",
          field: "content",
          delta: "visible delta",
        },
      },
    });
    state = applyGatewayEvent(state, {
      payload: {
        type: "message.part.updated",
        properties: {
          sessionID: "s1",
          part: {
            id: "p1",
            messageID: "m1",
            sessionID: "s1",
            type: "text",
            content: "late replacement",
          },
        },
      },
    });

    expect(state.messagesBySession.s1[0]?.parts[0]?.content).toBe("visible delta");
  });

  test("adds delta parts to existing messages before part hydration", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4126"),
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
      ...initialAppState("http://127.0.0.1:4126"),
      sessions: [{ id: "s1", title: "Build", status: "idle" }],
    };

    state = applyGatewayEvent(state, {
      payload: {
        type: "session.status",
        properties: {
          sessionID: "s1",
          status: { type: "busy" },
          context_tokens: { input: 10_000, limit: 200_000 },
          usage: {
            context_tokens: { input: 12_000, limit: 200_000 },
            tokens: { total_tokens: 123 },
            cost: 0.045,
            currency: "USD",
          },
        },
      },
    });

    expect(state.sessions[0]?.status).toBe("busy");
    expect(state.sessions[0]?.context_tokens?.input).toBe(12_000);
    expect(state.sessions[0]?.usage?.tokens).toEqual({ total_tokens: 123 });
    expect(state.sessions[0]?.usage?.cost).toBe(0.045);
    expect(state.sessions[0]?.usage?.currency).toBe("USD");
  });

  test("keeps session object identity for duplicate status metrics", () => {
    const session = {
      id: "s1",
      title: "Build",
      status: "busy" as const,
      context_tokens: { input: 12_000, limit: 200_000 },
      usage: {
        context_tokens: { input: 12_000, limit: 200_000 },
        tokens: { total_tokens: 123 },
        cost: 0.045,
        currency: "USD",
      },
    };
    const state: AppState = {
      ...initialAppState("http://127.0.0.1:4126"),
      sessions: [session],
    };

    const next = applyGatewayEvent(state, {
      payload: {
        type: "session.status",
        properties: {
          sessionID: "s1",
          status: { type: "busy" },
          usage: {
            context_tokens: { input: 12_000, limit: 200_000 },
            tokens: { total_tokens: 123 },
            cost: 0.045,
            currency: "USD",
          },
        },
      },
    });

    expect(next.sessions).toBe(state.sessions);
    expect(next.sessions[0]).toBe(session);
  });

  test("uses runtime session names without preserving local fallback names", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4126"),
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

    expect(sessionTitle(state.sessions[0]!)).toBe("New Session");
    expect(state.sessions[0]?.status).toBe("busy");
  });

  test("adds updated parts that arrive before message hydration", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");

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
      ...initialAppState("http://127.0.0.1:4126"),
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
      ...initialAppState("http://127.0.0.1:4126"),
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
    let state: AppState = initialAppState("http://127.0.0.1:4126");

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

  test("keeps cached part order stable when static snapshots update command-run parts", () => {
    let state: AppState = {
      ...initialAppState("http://127.0.0.1:4126"),
      messagesBySession: {
        s1: [
          {
            id: "m1",
            sessionID: "s1",
            role: "assistant",
            parts: [
              { id: "intro", sessionID: "s1", messageID: "m1", type: "text", text: "start" },
              {
                id: "runtime.tool.command_run",
                sessionID: "s1",
                messageID: "m1",
                type: "tool",
                tool: "command_run",
                state: { status: "running" },
              },
            ],
          },
        ],
      },
    };

    state = applyGatewayEvent(state, {
      payload: {
        type: "command.updated",
        properties: {
          sessionID: "s1",
          messageID: "m1",
          partID: "runtime.tool.command_run",
          runtimeID: "runtime",
          commandRunID: "runtime.tool.command_run",
          commandID: "runtime.tool.command_run:call_1:0",
          providerToolCallID: "call_1",
          commandIndex: 0,
          eventSeq: 2,
          status: "completed",
          createdAt: 1,
          updatedAt: 2,
          command: {
            command_id: "runtime.tool.command_run:call_1:0",
            command_type: "shell_command",
            command_line: "npm test",
          },
          result: {
            command_id: "runtime.tool.command_run:call_1:0",
            command_type: "shell_command",
            command_line: "npm test",
            success: true,
          },
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
              {
                id: "runtime.tool.command_run",
                sessionID: "s1",
                messageID: "m1",
                type: "tool",
                tool: "command_run",
                state: { status: "completed" },
              },
              { id: "intro", sessionID: "s1", messageID: "m1", type: "text", text: "start" },
              { id: "final", sessionID: "s1", messageID: "m1", type: "text", text: "done" },
            ],
          },
        },
      },
    });

    expect(state.messagesBySession.s1[0]?.parts.map((part) => part.id)).toEqual([
      "intro",
      "runtime.tool.command_run",
      "final",
    ]);
  });

  test("merges command updates by command id and ignores stale timestamps", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");

    const commandEvent = (status: string, eventSeq: number, result: unknown = null) =>
      ({
        payload: {
          type: "command.updated",
          properties: {
            sessionID: "s1",
            messageID: "runtime-command-id.message",
            partID: "runtime-command-id.tool.command_run",
            runtimeID: "runtime-command-id",
            commandRunID: "runtime-command-id.tool.command_run",
            commandID: "runtime-command-id.tool.command_run:call_1:0",
            providerToolCallID: "call_1",
            commandIndex: 0,
            eventSeq,
            status,
            createdAt: 20,
            command: {
              command_id: "runtime-command-id.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: "npm test",
            },
            result,
            updatedAt: eventSeq,
          },
        },
      }) as const;

    state = applyGatewayEvent(state, commandEvent("running", 30));
    state = applyGatewayEvent(
      state,
      commandEvent("completed", 40, {
        command_id: "runtime-command-id.tool.command_run:call_1:0",
        command_type: "shell_command",
        command_line: "npm test",
        success: true,
      }),
    );
    state = applyGatewayEvent(state, commandEvent("running", 30));

    const part = state.messagesBySession.s1?.[0]?.parts.find((item) => item.tool === "command_run");
    const commandState = part?.state as
      | {
          status?: string;
          input?: { commands?: Array<{ command_id?: string }> };
          streamed_command_run_result?: { results?: Array<{ success?: boolean }> };
        }
      | undefined;

    expect(commandState?.status).toBe("completed");
    expect(commandState?.input?.commands).toHaveLength(1);
    expect(commandState?.streamed_command_run_result?.results).toHaveLength(1);
    expect(commandState?.streamed_command_run_result?.results?.[0]?.success).toBe(true);
  });

  test("keeps command run batch running until the batch status is terminal", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");

    const commandEvent = (status: string, eventSeq: number, result: unknown = null) =>
      ({
        payload: {
          type: "command.updated",
          properties: {
            sessionID: "s1",
            messageID: "runtime-batch.message",
            partID: "runtime-batch.tool.command_run",
            runtimeID: "runtime-batch",
            commandRunID: "runtime-batch.tool.command_run",
            commandID: "runtime-batch.tool.command_run:call_1:0",
            providerToolCallID: "call_1",
            commandIndex: 0,
            eventSeq,
            status,
            createdAt: 20,
            command: {
              command_id: "runtime-batch.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: "npm test",
            },
            result,
            updatedAt: eventSeq,
          },
        },
      }) as const;

    state = applyGatewayEvent(state, commandEvent("running", 30));
    state = applyGatewayEvent(
      state,
      commandEvent("running", 40, {
        command_id: "runtime-batch.tool.command_run:call_1:0",
        command_type: "shell_command",
        command_line: "npm test",
        success: true,
      }),
    );

    let part = state.messagesBySession.s1?.[0]?.parts.find((item) => item.tool === "command_run");
    let commandState = part?.state as { status?: string } | undefined;
    expect(commandState?.status).toBe("running");

    state = applyGatewayEvent(state, commandEvent("completed", 50));
    part = state.messagesBySession.s1?.[0]?.parts.find((item) => item.tool === "command_run");
    commandState = part?.state as { status?: string } | undefined;
    expect(commandState?.status).toBe("completed");
  });

  test("does not let stale command snapshots overwrite live command results", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");

    state = applyGatewayEvent(state, {
      payload: {
        type: "command.updated",
        properties: {
          sessionID: "s1",
          messageID: "runtime-live.message",
          partID: "runtime-live.tool.command_run",
          runtimeID: "runtime-live",
          commandRunID: "runtime-live.tool.command_run",
          commandID: "runtime-live.tool.command_run:call_1:0",
          providerToolCallID: "call_1",
          commandIndex: 0,
          eventSeq: 40,
          status: "completed",
          createdAt: 1,
          updatedAt: 40,
          command: {
            command_id: "runtime-live.tool.command_run:call_1:0",
            command_type: "shell_command",
            command_line: "npm test",
          },
          result: {
            command_id: "runtime-live.tool.command_run:call_1:0",
            command_type: "shell_command",
            command_line: "npm test",
            event_seq: 40,
            updated_at: 40,
            success: true,
            output: "fresh live result",
          },
        },
      },
    });

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.updated",
        properties: {
          sessionID: "s1",
          info: {
            id: "runtime-live.message",
            sessionID: "s1",
            role: "assistant",
            parts: [
              {
                id: "runtime-live.tool.command_run",
                sessionID: "s1",
                messageID: "runtime-live.message",
                type: "tool",
                tool: "command_run",
                state: {
                  status: "running",
                  updated_at: 10,
                  input: {
                    commands: [
                      {
                        command_id: "runtime-live.tool.command_run:call_1:0",
                        command_type: "shell_command",
                        command_line: "npm test",
                        event_seq: 20,
                        updated_at: 10,
                      },
                    ],
                  },
                  streamed_command_run_result: {
                    results: [
                      {
                        command_id: "runtime-live.tool.command_run:call_1:0",
                        command_type: "shell_command",
                        command_line: "npm test",
                        event_seq: 20,
                        updated_at: 10,
                        status: "running",
                        output: "stale snapshot",
                      },
                    ],
                  },
                },
              },
            ],
          },
        },
      },
    });

    const commandState = state.messagesBySession.s1?.[0]?.parts[0]?.state as
      | {
          status?: string;
          streamed_command_run_result?: { results?: Array<{ output?: string; success?: boolean }> };
        }
      | undefined;

    expect(commandState?.status).toBe("completed");
    expect(commandState?.streamed_command_run_result?.results).toHaveLength(1);
    expect(commandState?.streamed_command_run_result?.results?.[0]?.output).toBe(
      "fresh live result",
    );
    expect(commandState?.streamed_command_run_result?.results?.[0]?.success).toBe(true);
  });

  test("deduplicates live command updates against unkeyed message hydration snapshots", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");

    state = applyGatewayEvent(state, {
      payload: {
        type: "command.updated",
        properties: {
          sessionID: "s1",
          messageID: "runtime-hydration.message",
          partID: "runtime-hydration.tool.command_run",
          runtimeID: "runtime-hydration",
          commandRunID: "runtime-hydration.tool.command_run",
          commandID: "runtime-hydration.tool.command_run:call_1:0",
          providerToolCallID: "call_1",
          commandIndex: 0,
          eventSeq: 30,
          status: "running",
          createdAt: 1,
          updatedAt: 30,
          command: {
            command_id: "runtime-hydration.tool.command_run:call_1:0",
            command_run_id: "runtime-hydration.tool.command_run",
            provider_tool_call_id: "call_1",
            command_index: 0,
            step: 1,
            command_type: "shell_command",
            command_line: "npm test",
          },
          result: {
            command_id: "runtime-hydration.tool.command_run:call_1:0",
            command_run_id: "runtime-hydration.tool.command_run",
            provider_tool_call_id: "call_1",
            command_index: 0,
            step: 1,
            command_type: "shell_command",
            command_line: "npm test",
            status: "running",
          },
        },
      },
    });

    state = applyGatewayEvent(state, {
      payload: {
        type: "message.updated",
        properties: {
          sessionID: "s1",
          info: {
            id: "runtime-hydration.message",
            sessionID: "s1",
            role: "assistant",
            parts: [
              {
                id: "runtime-hydration.tool.command_run",
                sessionID: "s1",
                messageID: "runtime-hydration.message",
                type: "tool",
                tool: "command_run",
                state: {
                  status: "running",
                  input: {
                    commands: [
                      {
                        step: 1,
                        command_type: "shell_command",
                        command_line: "npm test",
                      },
                    ],
                  },
                  streamed_command_run_result: {
                    results: [
                      {
                        step: 1,
                        command_type: "shell_command",
                        command_line: "npm test",
                        status: "running",
                      },
                    ],
                  },
                },
              },
            ],
          },
        },
      },
    });

    const part = state.messagesBySession.s1?.[0]?.parts[0];
    const commandState = part?.state as
      | {
          input?: { commands?: Array<{ command_id?: string; command_line?: string }> };
          streamed_command_run_result?: {
            results?: Array<{ command_id?: string; command_line?: string }>;
          };
        }
      | undefined;

    expect(commandState?.input?.commands).toHaveLength(1);
    expect(commandState?.input?.commands?.[0]?.command_id).toBe(
      "runtime-hydration.tool.command_run:call_1:0",
    );
    expect(commandState?.streamed_command_run_result?.results).toHaveLength(1);
    expect(toolRecords(part ? [part] : [])).toHaveLength(1);
  });

  test("keeps command run runtime messages in gateway stream order", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");

    const commandEvent = (runtimeID: string, createdAt: number) =>
      ({
        payload: {
          type: "command.updated",
          properties: {
            sessionID: "s1",
            messageID: `${runtimeID}.message`,
            partID: `${runtimeID}.tool.command_run`,
            runtimeID,
            commandRunID: `${runtimeID}.tool.command_run`,
            commandID: `${runtimeID}.tool.command_run:call_1:0`,
            providerToolCallID: "call_1",
            commandIndex: 0,
            eventSeq: createdAt,
            status: "running",
            createdAt,
            updatedAt: createdAt + 5,
            command: {
              command_id: `${runtimeID}.tool.command_run:call_1:0`,
              command_type: "shell_command",
              command_line: `echo ${runtimeID}`,
            },
          },
        },
      }) as const;

    state = applyGatewayEvent(state, commandEvent("runtime-late", 200));
    state = applyGatewayEvent(state, commandEvent("runtime-early", 100));

    expect(state.messagesBySession.s1?.map((message) => message.id)).toEqual([
      "runtime-late.message",
      "runtime-early.message",
    ]);
    const earlyState = state.messagesBySession.s1?.[1]?.parts[0]?.state as
      | { created_at?: number }
      | undefined;
    expect(earlyState?.created_at).toBe(100);
  });

  test("keeps every mock streaming truncation in first gateway occurrence order", () => {
    let state: AppState = initialAppState("http://127.0.0.1:4126");
    const expectedOrder: string[] = [];
    const seen = new Set<string>();

    const commandEvent = (runtimeID: string, createdAt: number, status = "running") =>
      ({
        payload: {
          type: "command.updated",
          properties: {
            sessionID: "s1",
            messageID: `${runtimeID}.message`,
            partID: `${runtimeID}.tool.command_run`,
            runtimeID,
            commandRunID: `${runtimeID}.tool.command_run`,
            commandID: `${runtimeID}.tool.command_run:call_1:0`,
            providerToolCallID: "call_1",
            commandIndex: 0,
            eventSeq: createdAt,
            status,
            createdAt,
            updatedAt: createdAt + 5,
            command: {
              command_id: `${runtimeID}.tool.command_run:call_1:0`,
              command_type: "shell_command",
              command_line: `echo ${runtimeID}`,
            },
            result:
              status === "completed"
                ? {
                    command_id: `${runtimeID}.tool.command_run:call_1:0`,
                    command_type: "shell_command",
                    command_line: `echo ${runtimeID}`,
                    success: true,
                  }
                : null,
          },
        },
      }) as const;

    const messageEvent = (id: string, createdAt: number, text: string) =>
      ({
        payload: {
          type: "message.updated",
          properties: {
            sessionID: "s1",
            info: {
              id,
              sessionID: "s1",
              role: "assistant",
              parts: [{ id: `${id}.text`, type: "text", text }],
              time: { created: createdAt, updated: createdAt },
              created_at: createdAt,
              updated_at: createdAt,
            },
          },
        },
      }) as const;

    const stream = [
      { messageId: "runtime-a.message", event: commandEvent("runtime-a", 300) },
      { messageId: "assistant-first", event: messageEvent("assistant-first", 100, "first") },
      { messageId: "runtime-b.message", event: commandEvent("runtime-b", 200) },
      { messageId: "runtime-a.message", event: commandEvent("runtime-a", 300, "completed") },
      { messageId: "assistant-second", event: messageEvent("assistant-second", 400, "second") },
      { messageId: "runtime-b.message", event: commandEvent("runtime-b", 200, "completed") },
    ];

    for (const item of stream) {
      state = applyGatewayEvent(state, item.event);
      if (!seen.has(item.messageId)) {
        seen.add(item.messageId);
        expectedOrder.push(item.messageId);
      }
      expect(state.messagesBySession.s1?.map((message) => message.id)).toEqual(expectedOrder);
    }
  });
});
