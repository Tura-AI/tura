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
  assert.deepEqual(taskReport.rounds.map((round) => round.messages.map((message) => [message.role, message.text]).at(-1)), [
    ["assistant", "Pi patched it."],
    ["assistant", "Codex patched it."],
    ["assistant", "Claude patched it."],
    ["assistant", "OpenCode patched it."],
    ["assistant", "Tura patched it."],
  ])
  assert.deepEqual(taskReport.rounds[0].input.messages.map((message) => [message.role, message.text]), [["user", "Fix Pi case"]])
  assert.deepEqual(taskReport.rounds[0].output.messages.map((message) => [message.role, message.text]), [["assistant", "Pi patched it."]])
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

test("source-port result bridge aggregates lifecycle events into per-agent rounds", () => {
  const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tura-benchmark-source-port-contract-"))
  const paths = {
    test_name: "project-rebuild-source-port",
    run_id: "zip-password-direct-codex-test",
    user_workspace: runRoot,
    target_root: runRoot,
    run_root: runRoot,
    summary_path: path.join(runRoot, "summary.json"),
  }
  const turaDir = path.join(runRoot, "zip-password-finder", "tura-direct-1")
  const codexDir = path.join(runRoot, "zip-password-finder", "codex-documents-2")
  fs.mkdirSync(path.join(turaDir, "context-and-calls"), { recursive: true })
  fs.mkdirSync(codexDir, { recursive: true })
  const promptPath = path.join(turaDir, "PYTHON_PORT_TASK.md")
  fs.writeFileSync(promptPath, "Port zip-password-finder", "utf8")
  const providerCallsPath = path.join(turaDir, "context-and-calls", "provider-calls-full.jsonl")
  fs.writeFileSync(providerCallsPath, `${JSON.stringify({
    started_at: "2026-01-01T00:00:00.000Z",
    finished_at: "2026-01-01T00:00:10.000Z",
    duration_ms: 10_000,
    response: {
      output_text: "Tura patched it.",
      usage: { input_tokens: 100, cached_input_tokens: 20, output_tokens: 30, reasoning_tokens: 5, total_tokens: 135 },
      events: [
        {
          type: "response.function_call_arguments.done",
          call_id: "call_tura_1",
          item_id: "fc_tura_1",
          arguments: JSON.stringify({
            commands: [
              { command_type: "shell_command", command_line: "python -m py_compile executable", step: 1 },
              { command_type: "apply_patch", command_line: "PATCH", step: 2 },
            ],
          }),
        },
      ],
    },
  })}\n`, "utf8")
  const turaStdout = [
    { type: "thread.started", thread_id: "thread-tura" },
    { type: "turn.started" },
    { type: "item.started", item: { type: "command_execution", id: "runtime.tool.command_run:call_tura_1:0", provider_tool_call_id: "call_tura_1", command_index: 0, status: "in_progress" } },
    { type: "item.completed", item: { type: "command_execution", id: "runtime.tool.command_run:call_tura_1:0", provider_tool_call_id: "call_tura_1", command_index: 0, status: "completed" } },
    { type: "item.completed", item: { type: "assistant_message", text: "Implemented the Python port." } },
    { type: "turn.completed" },
  ].map((event) => JSON.stringify(event)).join("\n")
  const codexStdout = [
    { type: "thread.started", thread_id: "thread-codex" },
    { type: "turn.started" },
    { type: "item.completed", item: { id: "item_0", type: "agent_message", text: "Codex patched it." } },
    { type: "item.started", item: { id: "item_1", type: "command_execution", command: "python -m py_compile executable", status: "in_progress" } },
    { type: "item.completed", item: { id: "item_1", type: "command_execution", command: "python -m py_compile executable", status: "completed", exit_code: 0, aggregated_output: "" } },
    { type: "turn.completed", usage: { input_tokens: 200, cached_input_tokens: 50, output_tokens: 40, reasoning_output_tokens: 10 } },
  ].map((event) => JSON.stringify(event)).join("\n")
  const codexTwoTurnStdout = [
    { type: "thread.started", thread_id: "thread-codex-2" },
    { type: "turn.started", turn_id: "codex-turn-a" },
    { type: "item.completed", item: { id: "msg_a", type: "agent_message", text: "Codex first turn." } },
    { type: "item.completed", item: { id: "tool_a", type: "command_execution", command: "python first.py", status: "completed", exit_code: 0 } },
    { type: "turn.completed", usage: { input_tokens: 10, output_tokens: 2 } },
    { type: "turn.started", turn_id: "codex-turn-b" },
    { type: "item.completed", item: { id: "msg_b", type: "agent_message", text: "Codex second turn." } },
    { type: "item.completed", item: { id: "tool_b", type: "command_execution", command: "python second.py", status: "completed", exit_code: 0 } },
    { type: "turn.completed", usage: { input_tokens: 11, output_tokens: 3 } },
  ].map((event) => JSON.stringify(event)).join("\n")

  const summary = normalizeBusinessSummary({
    ok: true,
    model: "gpt-5.5",
    tura_model: "openai/gpt-5.5",
    reasoning: "medium",
    service_tier: "default",
    results: [
      {
        agent: "tura-direct",
        task: "zip-password-finder",
        stdout: turaStdout,
        stdout_path: path.join(turaDir, "stdout.jsonl"),
        prep: { prompt_path: promptPath },
        context_archive: { provider_calls_full_path: providerCallsPath },
        usage: { input_tokens: 100, cached_input_tokens: 20, output_tokens: 30, reasoning_tokens: 5, total_tokens: 135 },
        eval: { ran: true, exit_code: 1, stdout_path: path.join(turaDir, "eval.stdout.log"), stderr_path: path.join(turaDir, "eval.stderr.log"), report: { reports: [{ passed: 12, failed: 3 }] } },
      },
      {
        agent: "codex-documents",
        task: "zip-password-finder",
        stdout: codexStdout,
        stdout_path: path.join(codexDir, "stdout.jsonl"),
        usage: { input_tokens: 200, cached_input_tokens: 50, output_tokens: 40, reasoning_tokens: 10, total_tokens: 300 },
        eval: { ran: true, exit_code: 0, stdout_path: path.join(codexDir, "eval.stdout.log"), stderr_path: path.join(codexDir, "eval.stderr.log"), report: { reports: [{ passed: 15, failed: 0 }] } },
      },
      {
        agent: "codex-documents",
        task: "zip-password-finder-extra",
        stdout: codexTwoTurnStdout,
        stdout_path: path.join(codexDir, "stdout-two-turns.jsonl"),
        usage: { input_tokens: 21, output_tokens: 5, total_tokens: 26 },
        eval: { ran: true, exit_code: 0, stdout_path: path.join(codexDir, "eval-2.stdout.log"), stderr_path: path.join(codexDir, "eval-2.stderr.log"), report: { reports: [{ passed: 1, failed: 0 }] } },
      },
    ],
  }, paths)

  const taskReport = JSON.parse(fs.readFileSync(summary.benchmark_contracts.task_report_path, "utf8"))
  const harnessReport = JSON.parse(fs.readFileSync(summary.benchmark_contracts.harness_report_path, "utf8"))
  assert.equal(summary.ok, false)
  assert.equal(taskReport.rounds.length, 4)
  assert.deepEqual(taskReport.rounds.map((round) => round.metadata.agentId), ["tura-direct", "codex-documents", "codex-documents", "codex-documents"])
  assert.deepEqual(taskReport.rounds.map((round) => round.metadata.agentKind), ["tura", "codex", "codex", "codex"])
  assert.deepEqual(taskReport.rounds.map((round) => round.metadata.agentMode), ["direct", "documents", "documents", "documents"])
  assert.deepEqual(taskReport.rounds.map((round) => round.metadata.model), ["openai/gpt-5.5", "gpt-5.5", "gpt-5.5", "gpt-5.5"])
  assert(!taskReport.rounds.some((round) => ["item", "thread", "turn"].includes(round.metadata.agentId)))
  assert.deepEqual(taskReport.rounds.map((round) => round.output.assistantMessage), ["Tura patched it.", "Codex patched it.", "Codex first turn.", "Codex second turn."])
  assert.deepEqual(taskReport.rounds.map((round) => round.usage.totalTokens), [135, 300, 12, 14])
  assert.deepEqual(taskReport.rounds.map((round) => round.messages.map((message) => message.text)), [["Implemented the Python port.", "Tura patched it."], ["Codex patched it."], ["Codex first turn."], ["Codex second turn."]])
  assert.deepEqual(taskReport.rounds[0].toolCalls.map((tool) => [tool.kind, tool.name, tool.commandLine, tool.parentToolName]), [
    ["command", "shell_command", "python -m py_compile executable", "command_run"],
    ["command", "apply_patch", "PATCH", "command_run"],
  ])
  assert.deepEqual(taskReport.rounds[1].toolCalls.map((tool) => [tool.kind, tool.commandLine]), [["command", "python -m py_compile executable"]])
  assert.deepEqual(taskReport.rounds[2].toolCalls.map((tool) => [tool.kind, tool.commandLine]), [["command", "python first.py"]])
  assert.deepEqual(taskReport.rounds[3].toolCalls.map((tool) => [tool.kind, tool.commandLine]), [["command", "python second.py"]])
  assert.deepEqual(harnessReport.scores.map((score) => [score.details.agent, score.details.passed, score.details.failed, score.passed]), [
    ["tura-direct", 12, 3, false],
    ["codex-documents", 15, 0, true],
    ["codex-documents", 1, 0, true],
  ])
  assert.equal(harnessReport.finalScore, (12 / 15 + 1 + 1) / 3)
})
