import assert from "node:assert/strict";
import test from "node:test";
import { buildRunResult } from "../../../src/output/final-result.js";
import { hasUserFacingAssistantText, type Message } from "../../../src/types/session.js";

test("run result ignores internal assistant completion placeholders", () => {
  const messages: Message[] = [
    {
      id: "msg-user",
      role: "user",
      parts: [{ id: "part-user", type: "text", text: "hello" }],
    },
    {
      id: "msg-internal",
      role: "assistant",
      parts: [
        {
          id: "part-internal",
          type: "text",
          text: "MANO completed without a user-facing message.",
        },
      ],
    },
  ];

  assert.equal(hasUserFacingAssistantText(messages, 1), false);
  assert.equal(buildRunResult("sess-1", messages).finalText, "");

  messages.push({
    id: "msg-final",
    role: "assistant",
    parts: [{ id: "part-final", type: "text", text: "TUI_BUSINESS_OK" }],
  });

  assert.equal(hasUserFacingAssistantText(messages, 1), true);
  assert.equal(buildRunResult("sess-1", messages).finalText, "TUI_BUSINESS_OK");
});

test("run result uses the newest assistant text when gateway messages are unordered", () => {
  const messages: Message[] = [
    {
      id: "msg-final",
      role: "assistant",
      created_at: 30,
      parts: [{ id: "part-final", type: "text", text: "FINAL_MARKER" }],
    },
    {
      id: "msg-user",
      role: "user",
      created_at: 10,
      parts: [{ id: "part-user", type: "text", text: "hello" }],
    },
    {
      id: "msg-progress",
      role: "assistant",
      created_at: 20,
      parts: [{ id: "part-progress", type: "text", text: "working" }],
    },
  ];

  assert.equal(buildRunResult("sess-1", messages).finalText, "FINAL_MARKER");
});

test("run result accepts user-facing runtime tool output as final assistant text", () => {
  const messages: Message[] = [
    {
      id: "msg-user",
      role: "user",
      created_at: 10,
      parts: [{ id: "part-user", type: "text", text: "hello" }],
    },
    {
      id: "msg-runtime",
      role: "assistant",
      created_at: 20,
      parts: [
        {
          id: "part-runtime",
          type: "tool",
          tool: "runtime",
          metadata: {
            status: "completed",
            output: "TUI_BUSINESS_OK",
          },
        },
      ],
    },
  ];

  assert.equal(hasUserFacingAssistantText(messages, 1), true);
  assert.equal(buildRunResult("sess-1", messages).finalText, "TUI_BUSINESS_OK");
});

test("run result includes runtime task_status payloads", () => {
  const messages: Message[] = [
    {
      id: "msg-user",
      role: "user",
      created_at: 10,
      parts: [{ id: "part-user", type: "text", text: "hello" }],
    },
    {
      id: "msg-runtime-status",
      role: "assistant",
      created_at: 20,
      parts: [
        {
          id: "part-runtime-status",
          type: "tool",
          tool: "runtime",
          metadata: {
            command_type: "task_status",
            output: { task_status: { status: "done", task_detail: "task status is visible" } },
          },
        },
      ],
    },
  ];

  assert.equal(hasUserFacingAssistantText(messages, 1), true);
  assert.equal(buildRunResult("sess-1", messages).finalText, "task status is visible");
});

test("run result includes CLI metadata usage, timing, command, and turn stats", () => {
  const messages: Message[] = [
    {
      id: "msg-user-1",
      role: "user",
      created_at: 1_000,
      updated_at: 1_000,
      parts: [{ id: "part-user-1", type: "text", text: "run checks" }],
    },
    {
      id: "msg-assistant-tool",
      role: "assistant",
      created_at: 2_000,
      updated_at: 5_000,
      parts: [
        {
          id: "part-runtime",
          type: "tool",
          tool: "runtime",
          metadata: {
            runtime_id: "runtime-1",
            usage: {
              input_tokens: 120,
              output_tokens: 30,
              cached_input_tokens: 40,
              latency_ms: 1_500,
            },
          },
          state: {
            metadata: {
              runtime_id: "runtime-1",
              usage: {
                input_tokens: 120,
                output_tokens: 30,
                cached_input_tokens: 40,
                latency_ms: 1_500,
              },
            },
          },
        },
        {
          id: "part-command-run",
          type: "tool",
          tool: "command_run",
          state: {
            input: {
              commands: [
                { step: 1, command_type: "shell_command", command_line: "npm run build" },
                { step: 2, command_type: "shell_command", command_line: "npm test" },
              ],
            },
            output: {
              streamed_command_run_result: {
                results: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "npm run build",
                    status: "completed",
                    success: true,
                  },
                  {
                    step: 2,
                    command_type: "shell_command",
                    command_line: "npm test",
                    status: "failed",
                    success: false,
                  },
                ],
              },
            },
          },
        },
      ],
    },
    {
      id: "msg-final",
      role: "assistant",
      created_at: 5_500,
      updated_at: 6_000,
      parts: [{ id: "part-final", type: "text", text: "Done." }],
    },
  ];

  const result = buildRunResult("sess-1", messages);

  assert.equal(result.finalText, "Done.");
  assert.deepEqual(result.metadata, {
    input_token_usage: 120,
    input_token_cache: 40,
    provider_time_ms: 1_500,
    total_time_ms: 5_000,
    commands: 2,
    failed_commands: 1,
    tps: 20,
    turns: 1,
  });
});
