import assert from "node:assert/strict";
import test from "node:test";

import { aggregateHarnessScore } from "../src/harness.js";
import { normalizeCliInstruction, parseAgentRound, parseJsonlRounds } from "../src/parser.js";

test("normalizeCliInstruction preserves command line and shell-like args", () => {
  const instruction = normalizeCliInstruction('node script.mjs --flag "two words"');

  assert.equal(instruction.commandName, "node");
  assert.deepEqual(instruction.args, ["node", "script.mjs", "--flag", "two words"]);
  assert.equal(instruction.commandLine, 'node script.mjs --flag "two words"');
});

test("parseAgentRound flattens command_run commands into unified calls", () => {
  const round = parseAgentRound(
    {
      id: "turn-1",
      startedAt: "2026-01-01T00:00:00.000Z",
      endedAt: "2026-01-01T00:00:01.000Z",
      fullContext: "system + user context",
      assistantMessage: "I will patch it.",
      usage: {
        input_tokens: 10,
        cached_input_tokens: 2,
        output_tokens: 4,
        reasoning_tokens: 1,
        total_tokens: 17,
      },
      provider_duration_ms: 900,
      tool_calls: [
        {
          id: "call-1",
          name: "command_run",
          arguments: JSON.stringify({
            commands: [
              { command_type: "shell_command", command_line: "npm test", step: 1 },
              { command_type: "apply_patch", command_line: "PATCH_BODY", step: 2 },
            ],
          }),
        },
      ],
    },
    0,
  );

  assert.equal(round.schema, "tura.benchmark.agent-round.v1");
  assert.equal(round.input.fullContext, "system + user context");
  assert.equal(round.output.assistantMessage, "I will patch it.");
  assert.deepEqual(round.usage, {
    inputTokens: 10,
    cacheInputTokens: 2,
    outputTokens: 4,
    reasoningTokens: 1,
    totalTokens: 17,
  });
  assert.equal(round.providerDurationMs, 900);
  assert.deepEqual(
    round.toolCalls.map((call) => [call.kind, call.name, call.commandLine, call.parentToolName, call.parallelGroupId]),
    [
      ["command", "shell_command", "npm test", "command_run", "1"],
      ["command", "apply_patch", "PATCH_BODY", "command_run", "2"],
    ],
  );
});

test("parseAgentRound keeps ordinary and parallel tools in the same contract", () => {
  const round = parseAgentRound({
    response: {
      output_text: "Done.",
      output: [
        {
          type: "function_call",
          id: "call-a",
          name: "web_discover",
          arguments: JSON.stringify({ query: "docs", parallel_group_id: "p1" }),
          parallel_group_id: "p1",
        },
        {
          type: "function_call",
          id: "call-b",
          name: "read_media",
          arguments: JSON.stringify({ path: "out.png" }),
          parallel_group_id: "p1",
        },
      ],
    },
  });

  assert.deepEqual(
    round.toolCalls.map((call) => [call.kind, call.name, call.parallelGroupId]),
    [
      ["tool", "web_discover", "p1"],
      ["tool", "read_media", "p1"],
    ],
  );
});

test("parseJsonlRounds normalizes the five benchmark agents' per-round callbacks", () => {
  const callbacks = [
    {
      type: "pi.round.completed",
      agent_id: "pi",
      turn_id: "pi-turn-1",
      started_at: "2026-01-01T00:00:00.000Z",
      ended_at: "2026-01-01T00:00:01.000Z",
      messages: [{ role: "user", content: "Fix Pi case" }],
      message: { role: "assistant", content: "Pi patched it." },
      usage: { input_tokens: 11, cached_input_tokens: 2, output_tokens: 3, reasoning_tokens: 4, total_tokens: 20 },
      tool_calls: [{ id: "pi-tool", name: "shell_command", input: { command: "npm test" } }],
    },
    {
      type: "codex.round.completed",
      agent_id: "codex",
      turn_id: "codex-turn-1",
      request: { input: [{ role: "user", content: "Fix Codex case" }] },
      response: {
        output_text: "Codex patched it.",
        usage: {
          prompt_tokens: 12,
          input_tokens_details: { cached_tokens: 1 },
          completion_tokens: 5,
          output_tokens_details: { reasoning_tokens: 2 },
          total_tokens: 20,
        },
        output: [{ type: "function_call", id: "codex-tool", name: "apply_patch", arguments: "PATCH" }],
      },
    },
    {
      type: "claude.round.completed",
      agent_id: "claudecode",
      session_id: "claude-turn-1",
      message: {
        role: "assistant",
        content: [{ type: "text", text: "Claude patched it." }, { type: "tool_use", id: "claude-tool", name: "Bash", input: { command: "pytest" } }],
        usage: { input_tokens: 13, cache_read_input_tokens: 3, output_tokens: 4 },
      },
    },
    {
      type: "opencode.round.completed",
      agent_id: "opencode",
      id: "opencode-turn-1",
      input: { messages: [{ role: "user", content: "Fix OpenCode case" }] },
      output: { message: { content: "OpenCode patched it." } },
      metrics: { inputTokens: 14, cacheInputTokens: 2, outputTokens: 6, reasoningTokens: 1, totalTokens: 23, durationMs: 321 },
      tool: { id: "opencode-tool", name: "edit", arguments: { file: "src/app.ts" } },
    },
    {
      type: "tura.round.completed",
      agent_id: "tura",
      turn_id: "tura-turn-1",
      full_context: "Fix Tura case",
      assistant_message: "Tura patched it.",
      runtime_usage: { input_tokens: 15, cached_input_tokens: 1, output_tokens: 7, reasoning_tokens: 2, total_tokens: 25, latency_ms: 456 },
      tool_result: {
        tool_name: "command_run",
        input: { commands: [{ command_type: "shell_command", command_line: "cargo test", step: 1 }] },
      },
    },
  ];

  const rounds = parseJsonlRounds(callbacks.map((callback) => JSON.stringify(callback)).join("\n"));

  assert.deepEqual(rounds.map((round) => round.roundId), [
    "pi-turn-1",
    "codex-turn-1",
    "claude-turn-1",
    "opencode-turn-1",
    "tura-turn-1",
  ]);
  assert.deepEqual(rounds.map((round) => round.output.assistantMessage), [
    "Pi patched it.",
    "Codex patched it.",
    "Claude patched it.",
    "OpenCode patched it.",
    "Tura patched it.",
  ]);
  assert.deepEqual(rounds.map((round) => round.usage.totalTokens), [20, 20, 20, 23, 25]);
  assert.deepEqual(
    rounds.map((round) => round.toolCalls.map((tool) => [tool.kind, tool.name, tool.commandLine])),
    [
      [["tool", "shell_command", "npm test"]],
      [["tool", "apply_patch", "PATCH"]],
      [["tool", "Bash", "pytest"]],
      [["tool", "edit", '{"file":"src/app.ts"}']],
      [["command", "shell_command", "cargo test"]],
    ],
  );
});

test("aggregateHarnessScore normalizes known max scores", () => {
  assert.equal(
    aggregateHarnessScore([
      { harnessId: "a", score: 8, maxScore: 10, passed: true },
      { harnessId: "b", score: 1, maxScore: 2, passed: false },
    ]),
    0.75,
  );
});
