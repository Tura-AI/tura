import assert from "node:assert/strict";
import test from "node:test";

import { aggregateHarnessScore } from "../src/harness.js";
import { normalizeCliInstruction, parseAgentRound } from "../src/parser.js";

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

test("aggregateHarnessScore normalizes known max scores", () => {
  assert.equal(
    aggregateHarnessScore([
      { harnessId: "a", score: 8, maxScore: 10, passed: true },
      { harnessId: "b", score: 1, maxScore: 2, passed: false },
    ]),
    0.75,
  );
});
