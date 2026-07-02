import assert from "node:assert/strict"
import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import test from "node:test"

import { normalizeBusinessSummary } from "../lib/business_paths.mjs"

test("legacy benchmark summary bridge writes unified per-round contracts", () => {
  const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tura-benchmark-contract-"))
  const paths = {
    test_name: "bridge-contract",
    run_id: "run-1",
    user_workspace: runRoot,
    target_root: runRoot,
    run_root: runRoot,
    summary_path: path.join(runRoot, "summary.json"),
  }
  const callbacks = [
    {
      type: "pi.round.completed",
      agent_id: "pi",
      agent_mode: "direct",
      model: "gpt-5.5",
      reasoning: "medium",
      service_tier: "default",
      priority_enabled: false,
      turn_id: "pi-turn-1",
      messages: [{ role: "user", content: "Fix Pi case" }],
      message: { role: "assistant", content: "Pi patched it." },
      usage: { input_tokens: 11, cached_input_tokens: 2, output_tokens: 3, reasoning_tokens: 4, total_tokens: 20 },
      tool_calls: [{ id: "pi-tool", name: "shell_command", input: { command: "npm test" } }],
    },
    {
      type: "codex.round.completed",
      agent_id: "codex",
      agent_mode: "cli",
      model: "gpt-5.5",
      reasoning: "medium",
      service_tier: "default",
      priority_enabled: false,
      turn_id: "codex-turn-1",
      response: {
        output_text: "Codex patched it.",
        usage: { prompt_tokens: 12, input_tokens_details: { cached_tokens: 1 }, completion_tokens: 5, output_tokens_details: { reasoning_tokens: 2 }, total_tokens: 20 },
        output: [{ type: "function_call", id: "codex-tool", name: "apply_patch", arguments: "PATCH" }],
      },
    },
    {
      type: "claude.round.completed",
      agent_id: "claudecode",
      agent_mode: "cli",
      model: "claude-opus-4",
      reasoning: "medium",
      service_tier: "default",
      priority_enabled: false,
      session_id: "claude-turn-1",
      message: {
        content: [{ type: "text", text: "Claude patched it." }, { type: "tool_use", id: "claude-tool", name: "Bash", input: { command: "pytest" } }],
        usage: { input_tokens: 13, cache_read_input_tokens: 3, output_tokens: 4 },
      },
    },
    {
      type: "opencode.round.completed",
      agent_id: "opencode",
      agent_mode: "cli",
      model: "gpt-5.5",
      reasoning: "medium",
      service_tier: "default",
      priority_enabled: false,
      id: "opencode-turn-1",
      output: { message: { content: "OpenCode patched it." } },
      metrics: { inputTokens: 14, cacheInputTokens: 2, outputTokens: 6, reasoningTokens: 1, totalTokens: 23, durationMs: 321 },
      tool: { id: "opencode-tool", name: "edit", arguments: { file: "src/app.ts" } },
    },
    {
      type: "tura.round.completed",
      agent_id: "tura",
      agent_mode: "balanced",
      model: "openai/gpt-5.5",
      reasoning: "medium",
      service_tier: "default",
      priority_enabled: false,
      turn_id: "tura-turn-1",
      full_context: "Fix Tura case",
      assistant_message: "Tura patched it.",
      runtime_usage: { input_tokens: 15, cached_input_tokens: 1, output_tokens: 7, reasoning_tokens: 2, total_tokens: 25, latency_ms: 456 },
      tool_result: { tool_name: "command_run", input: { commands: [{ command_type: "shell_command", command_line: "cargo test", step: 1 }] } },
    },
  ]

  const summary = normalizeBusinessSummary({
    ok: true,
    results: [{ stdout: callbacks.map((callback) => JSON.stringify(callback)).join("\n") }],
  }, paths)

  const taskReport = JSON.parse(fs.readFileSync(summary.benchmark_contracts.task_report_path, "utf8"))
  assert.equal(taskReport.rounds.length, 5)
  assert.deepEqual(taskReport.rounds.map((round) => round.roundId), ["pi-turn-1", "codex-turn-1", "claude-turn-1", "opencode-turn-1", "tura-turn-1"])
  assert.deepEqual(taskReport.rounds.map((round) => round.output.assistantMessage), ["Pi patched it.", "Codex patched it.", "Claude patched it.", "OpenCode patched it.", "Tura patched it."])
  assert.deepEqual(taskReport.rounds.map((round) => [round.metadata.agentId, round.metadata.agentKind, round.metadata.agentMode, round.metadata.model, round.metadata.priorityEnabled]), [
    ["pi", "pi", "direct", "gpt-5.5", false],
    ["codex", "codex", "cli", "gpt-5.5", false],
    ["claudecode", "claudecode", "cli", "claude-opus-4", false],
    ["opencode", "opencode", "cli", "gpt-5.5", false],
    ["tura", "tura", "balanced", "openai/gpt-5.5", false],
  ])
  assert.equal(taskReport.usage.totalTokens, 108)
  assert.deepEqual(taskReport.rounds.at(-1).toolCalls.map((tool) => [tool.kind, tool.name, tool.commandLine]), [["command", "shell_command", "cargo test"]])
  assert.equal(fs.readdirSync(taskReport.roundsDirectory).length, 5)
})
