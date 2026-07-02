import { describe, expect, test } from "bun:test";
import type { MessagePart } from "@tura/gateway-sdk";
import {
  assistantPartBlocks,
  assistantToolBlockForPart,
} from "../../../app/src/conversation/assistant-blocks";
import { groupConversationTurns } from "../../../app/src/conversation/conversation-turns";
import {
  commandRunGroupDurationMs,
  formatCommandTiming,
  toolRecords,
} from "../../../app/src/conversation/message-tools";

function commandRunPart(runtimeId: string, createdAt: number, command: string): MessagePart {
  return {
    id: `${runtimeId}.tool.command_run`,
    sessionID: "s1",
    messageID: `${runtimeId}.message`,
    type: "tool",
    tool: "command_run",
    state: {
      status: "completed",
      created_at: createdAt,
      input: {
        commands: [
          {
            command_id: `${runtimeId}.tool.command_run:call_1:0`,
            command_type: "shell_command",
            command_line: command,
          },
        ],
      },
      streamed_command_run_result: {
        results: [
          {
            command_id: `${runtimeId}.tool.command_run:call_1:0`,
            command_type: "shell_command",
            command_line: command,
            success: true,
          },
        ],
      },
    },
  };
}

describe("assistant command run blocks", () => {
  test("keeps each runtime command_run in gateway part order", () => {
    const late = commandRunPart("runtime-late", 200, "echo late");
    const early = commandRunPart("runtime-early", 100, "echo early");

    const blocks = assistantPartBlocks([late, early], new Set());

    expect(blocks).toHaveLength(2);
    expect(blocks.map((block) => block.parts.map((part) => part.id))).toEqual([
      ["runtime-late.tool.command_run"],
      ["runtime-early.tool.command_run"],
    ]);
  });

  test("selects only the clicked runtime command group for the inspector", () => {
    const early = commandRunPart("runtime-early", 100, "echo early");
    const late = commandRunPart("runtime-late", 200, "echo late");

    const block = assistantToolBlockForPart([early, late], "runtime-late.tool.command_run");

    expect(block?.parts.map((part) => part.id)).toEqual(["runtime-late.tool.command_run"]);
  });

  test("keeps task_status command records when they share the command_run batch", () => {
    const part: MessagePart = {
      id: "runtime-mixed.tool.command_run",
      sessionID: "s1",
      messageID: "runtime-mixed.message",
      type: "tool",
      tool: "command_run",
      state: {
        status: "completed",
        input: {
          commands: [
            {
              command_id: "runtime-mixed.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: "npm test",
              step: 1,
            },
            {
              command_id: "runtime-mixed.tool.command_run:call_1:1",
              command_type: "task_status",
              command_line: '{"status":"done"}',
              step: 2,
            },
          ],
        },
        streamed_command_run_result: {
          results: [
            {
              command_id: "runtime-mixed.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: "npm test",
              step: 1,
              success: true,
              output: "tests passed",
            },
            {
              command_id: "runtime-mixed.tool.command_run:call_1:1",
              command_type: "task_status",
              step: 2,
              success: true,
              output: { task_status: { status: "done" } },
            },
          ],
        },
      },
    };

    const records = toolRecords([part]);

    expect(records).toHaveLength(2);
    expect(records.map((record) => record.command)).toEqual(["npm test", '{"status":"done"}']);
    expect(records.map((record) => record.step)).toEqual([1, 2]);
    expect(records.map((record) => record.hasResult)).toEqual([true, true]);
    expect(records[1]?.output).toContain("task_status");
  });

  test("marks scheduled command records without streamed results", () => {
    const part: MessagePart = {
      id: "runtime-scheduled.tool.command_run",
      sessionID: "s1",
      messageID: "runtime-scheduled.message",
      type: "tool",
      tool: "command_run",
      state: {
        status: "running",
        input: {
          commands: [
            {
              command_id: "runtime-scheduled.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: '{"command":"npm test","timeout_ms":300000}',
              step: 3,
              created_at: 100,
            },
          ],
        },
      },
    };

    const records = toolRecords([part]);

    expect(records[0]?.step).toBe(3);
    expect(records[0]?.hasResult).toBe(false);
    expect(records[0]?.timeoutMs).toBe(300_000);
  });

  test("does not merge consecutive assistant command_run messages", () => {
    const first = commandRunPart("runtime-first", 100, "echo first");
    const second = commandRunPart("runtime-second", 200, "echo second");

    const messages = groupConversationTurns([
      {
        id: "runtime-first.message",
        sessionID: "s1",
        role: "assistant",
        created_at: 100,
        parts: [first],
      },
      {
        id: "runtime-second.message",
        sessionID: "s1",
        role: "assistant",
        created_at: 200,
        parts: [second],
      },
    ]);

    expect(messages.map((message) => message.id)).toEqual([
      "runtime-first.message",
      "runtime-second.message",
    ]);
    expect(messages.flatMap((message) => message.parts).map((part) => part.id)).toEqual([
      "runtime-first.tool.command_run",
      "runtime-second.tool.command_run",
    ]);
  });

  test("keeps command records stable across mock streaming truncations", () => {
    const commands = ["prepare", "build", "verify"].map((command, index) => ({
      command_id: `runtime-stream.tool.command_run:call_1:${index}`,
      command_type: "shell_command",
      command_line: command,
    }));
    const results = commands.map((command) => ({ ...command, success: true }));

    for (let size = 1; size <= commands.length; size += 1) {
      const part: MessagePart = {
        id: "runtime-stream.tool.command_run",
        sessionID: "s1",
        messageID: "runtime-stream.message",
        type: "tool",
        tool: "command_run",
        state: {
          status: size === commands.length ? "completed" : "running",
          input: { commands: commands.slice(0, size) },
          streamed_command_run_result: { results: results.slice(0, size) },
        },
      };

      expect(toolRecords([part]).map((record) => record.command)).toEqual(
        commands.slice(0, size).map((command) => command.command_line),
      );
    }
  });

  test("deduplicates mirrored command_run result fields", () => {
    const part: MessagePart = {
      id: "runtime-mirror.tool.command_run",
      sessionID: "s1",
      messageID: "runtime-mirror.message",
      type: "tool",
      tool: "command_run",
      state: {
        status: "completed",
        input: {
          commands: [
            {
              command_id: "runtime-mirror.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: "npm test",
            },
          ],
        },
        streamed_command_run_result: {
          results: [
            {
              command_id: "runtime-mirror.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: "npm test",
              success: true,
              output: "from stream mirror",
            },
          ],
        },
        output: {
          streamed_command_run_result: {
            results: [
              {
                command_id: "runtime-mirror.tool.command_run:call_1:0",
                command_type: "shell_command",
                command_line: "npm test",
                success: true,
                output: "from output mirror",
              },
            ],
          },
        },
      },
    };

    const records = toolRecords([part]);

    expect(records).toHaveLength(1);
    expect(records[0]?.command).toBe("npm test");
    expect(records[0]?.output).toBe("from output mirror");
  });

  test("keeps running command elapsed time live instead of freezing at last update", () => {
    const originalNow = Date.now;
    Date.now = () => 160_000;
    try {
      const part: MessagePart = {
        id: "runtime-live.tool.command_run",
        sessionID: "s1",
        messageID: "runtime-live.message",
        type: "tool",
        tool: "command_run",
        state: {
          status: "running",
          input: {
            commands: [
              {
                command_id: "runtime-live.tool.command_run:call_1:0",
                command_type: "shell_command",
                command_line: "npm test",
                created_at: 100,
                updated_at: 105,
              },
            ],
          },
        },
      };

      const records = toolRecords([part]);

      expect(records[0]?.durationMs).toBe(60_000);
    } finally {
      Date.now = originalNow;
    }
  });

  test("extracts command timeout parameters for compact elapsed-over-limit display", () => {
    const part: MessagePart = {
      id: "runtime-timeout.tool.command_run",
      sessionID: "s1",
      messageID: "runtime-timeout.message",
      type: "tool",
      tool: "command_run",
      state: {
        status: "running",
        input: {
          commands: [
            {
              command_id: "runtime-timeout.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: '{"command":"npm test","timeout_ms":300000}',
              created_at: 100,
            },
          ],
        },
        streamed_command_run_result: {
          results: [
            {
              command_id: "runtime-timeout.tool.command_run:call_1:0",
              command_type: "shell_command",
              status: "running",
            },
          ],
        },
      },
    };

    const records = toolRecords([part]);

    expect(records[0]?.command).toBe("npm test");
    expect(records[0]?.timeoutMs).toBe(300_000);
    expect(formatCommandTiming(212_000, records[0]?.timeoutMs)).toBe("3m32s/5m");
    expect(formatCommandTiming(212_000, undefined)).toBe("3m32s");
  });

  test("summarizes elapsed time from the whole command_run group instead of the last command", () => {
    const part: MessagePart = {
      id: "runtime-group.tool.command_run",
      sessionID: "s1",
      messageID: "runtime-group.message",
      type: "tool",
      tool: "command_run",
      state: {
        status: "completed",
        created_at: 100_000,
        updated_at: 180_000,
        time: { start: 100_000, end: 180_000 },
        input: {
          commands: [
            {
              command_id: "runtime-group.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: "prepare",
              started_at: 100_000,
              completed_at: 170_000,
            },
            {
              command_id: "runtime-group.tool.command_run:call_1:1",
              command_type: "shell_command",
              command_line: "verify",
              started_at: 175_000,
              completed_at: 180_000,
            },
          ],
        },
        streamed_command_run_result: {
          results: [
            {
              command_id: "runtime-group.tool.command_run:call_1:0",
              command_type: "shell_command",
              command_line: "prepare",
              success: true,
              started_at: 100_000,
              completed_at: 170_000,
            },
            {
              command_id: "runtime-group.tool.command_run:call_1:1",
              command_type: "shell_command",
              command_line: "verify",
              success: true,
              started_at: 175_000,
              completed_at: 180_000,
            },
          ],
        },
      },
    };

    const records = toolRecords([part]);

    expect(records.map((record) => record.durationMs)).toEqual([70_000, 5_000]);
    expect(commandRunGroupDurationMs([part])).toBe(80_000);
  });

  test("keeps running command_run group elapsed time live from the group start", () => {
    const originalNow = Date.now;
    Date.now = () => 190_000;
    try {
      const part: MessagePart = {
        id: "runtime-running-group.tool.command_run",
        sessionID: "s1",
        messageID: "runtime-running-group.message",
        type: "tool",
        tool: "command_run",
        state: {
          status: "running",
          created_at: 100_000,
          updated_at: 180_000,
          time: { start: 100_000, updated: 180_000 },
          input: {
            commands: [
              {
                command_id: "runtime-running-group.tool.command_run:call_1:0",
                command_type: "shell_command",
                command_line: "verify",
                started_at: 175_000,
              },
            ],
          },
        },
      };

      expect(commandRunGroupDurationMs([part])).toBe(90_000);
    } finally {
      Date.now = originalNow;
    }
  });
});
