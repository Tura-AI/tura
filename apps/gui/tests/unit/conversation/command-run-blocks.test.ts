import { describe, expect, test } from "bun:test";
import type { MessagePart } from "@tura/gateway-sdk";
import {
  assistantPartBlocks,
  assistantToolBlockForPart,
} from "../../../app/src/conversation/assistant-blocks";

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
  test("keeps each runtime command_run in its own created_at ordered block", () => {
    const late = commandRunPart("runtime-late", 200, "echo late");
    const early = commandRunPart("runtime-early", 100, "echo early");

    const blocks = assistantPartBlocks([late, early], new Set());

    expect(blocks).toHaveLength(2);
    expect(blocks.map((block) => block.parts.map((part) => part.id))).toEqual([
      ["runtime-early.tool.command_run"],
      ["runtime-late.tool.command_run"],
    ]);
  });

  test("selects only the clicked runtime command group for the inspector", () => {
    const early = commandRunPart("runtime-early", 100, "echo early");
    const late = commandRunPart("runtime-late", 200, "echo late");

    const block = assistantToolBlockForPart([early, late], "runtime-late.tool.command_run");

    expect(block?.parts.map((part) => part.id)).toEqual(["runtime-late.tool.command_run"]);
  });
});
