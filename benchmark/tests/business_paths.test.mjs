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
  assert.equal(taskReport.usage.totalTokens, 105)
  assert.deepEqual(taskReport.rounds.at(-1).toolCalls.map((tool) => [tool.kind, tool.name, tool.commandLine]), [["command", "shell_command", "cargo test"]])
  assert.equal(fs.readdirSync(taskReport.roundsDirectory).length, 5)
})

test("round contracts preserve codex token fixture provenance", () => {
  const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tura-benchmark-fixture-contract-"))
  const paths = {
    test_name: "fixture-contract",
    run_id: "run-fixture",
    user_workspace: runRoot,
    target_root: runRoot,
    run_root: runRoot,
    summary_path: path.join(runRoot, "summary.json"),
  }
  const stdout = JSON.stringify({
    type: "opencode.round.completed",
    agent_id: "opencode",
    agent_mode: "cli",
    model: "gpt-5.5",
    reasoning: "medium",
    service_tier: "default",
    priority_enabled: false,
    roundId: "opencode-codex-fixture-round-1",
    round_source: "codex-token-fixture",
    fixture_backend: "codex",
    fixture_source_path: "codex.stdout.jsonl",
    source_agent_id: "codex-main",
    source_event_type: "thread.token_usage.updated",
    source_round_index: 7,
    usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_tokens: 1, total_tokens: 12 },
  })

  const summary = normalizeBusinessSummary({
    ok: true,
    results: [{ agent_id: "opencode", stdout }],
  }, paths)

  const taskReport = JSON.parse(fs.readFileSync(summary.benchmark_contracts.task_report_path, "utf8"))
  assert.equal(taskReport.rounds.length, 1)
  assert.equal(taskReport.rounds[0].metadata.roundSource, "codex-token-fixture")
  assert.equal(taskReport.rounds[0].metadata.fixtureBackend, "codex")
  assert.equal(taskReport.rounds[0].metadata.fixtureSourcePath, "codex.stdout.jsonl")
  assert.equal(taskReport.rounds[0].metadata.sourceAgentId, "codex-main")
  assert.equal(taskReport.rounds[0].metadata.sourceEventType, "thread.token_usage.updated")
  assert.equal(taskReport.rounds[0].metadata.sourceRoundIndex, 7)
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
  assert.deepEqual(taskReport.rounds.map((round) => round.usage.totalTokens), [135, 240, 12, 14])
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

test("result bridge expands provider calls and codex token updates into contract rounds", () => {
  const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tura-benchmark-token-rounds-"))
  const paths = {
    test_name: "token-round-contract",
    run_id: "provider-and-codex-token-updates",
    user_workspace: runRoot,
    target_root: runRoot,
    run_root: runRoot,
    summary_path: path.join(runRoot, "summary.json"),
  }
  const turaDir = path.join(runRoot, "task", "tura-balanced-1")
  const codexDir = path.join(runRoot, "task", "codex-main-2")
  fs.mkdirSync(path.join(turaDir, "context-and-calls"), { recursive: true })
  fs.mkdirSync(codexDir, { recursive: true })
  const providerCallsPath = path.join(turaDir, "context-and-calls", "provider-calls-full.jsonl")
  const providerRecords = [
    {
      call_id: "call-tura-1",
      started_at: "2026-01-01T00:00:00.000Z",
      finished_at: "2026-01-01T00:00:01.000Z",
      duration_ms: 1000,
      response: {
        output_text: "Tura first.",
        usage: { input_tokens: 100, cached_input_tokens: 70, output_tokens: 20, reasoning_tokens: 5, total_tokens: 120 },
      },
    },
    {
      call_id: "call-tura-2",
      started_at: "2026-01-01T00:00:02.000Z",
      finished_at: "2026-01-01T00:00:04.000Z",
      duration_ms: 2000,
      response: {
        output_text: "Tura second.",
        usage: { input_tokens: 130, cached_input_tokens: 100, output_tokens: 30, reasoning_tokens: 6, total_tokens: 160 },
      },
    },
  ]
  fs.writeFileSync(providerCallsPath, providerRecords.map((record) => JSON.stringify(record)).join("\n") + "\n", "utf8")
  const codexStdout = [
    { type: "thread.started", thread_id: "thread-codex" },
    { type: "turn.started" },
    { type: "item.completed", item: { id: "msg-1", type: "agent_message", text: "Codex first." } },
    {
      type: "thread.token_usage.updated",
      usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1 },
      total_usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1 },
    },
    { type: "item.completed", item: { id: "msg-2", type: "agent_message", text: "Codex second." } },
    {
      type: "thread.token_usage.updated",
      usage: { input_tokens: 11, cached_input_tokens: 5, output_tokens: 3, reasoning_output_tokens: 2 },
      total_usage: { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_output_tokens: 3 },
    },
    { type: "turn.completed", usage: { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_output_tokens: 3 } },
  ].map((event) => JSON.stringify(event)).join("\n")

  const summary = normalizeBusinessSummary({
    ok: true,
    model: "gpt-5.5",
    tura_model: "openai/gpt-5.5",
    reasoning: "medium",
    service_tier: "default",
    results: [
      {
        agent: "tura-balanced",
        task: "task",
        context_archive: { provider_calls_full_path: providerCallsPath },
        provider_calls: providerRecords,
        usage: { input_tokens: 230, cached_input_tokens: 170, output_tokens: 50, reasoning_tokens: 11, total_tokens: 280 },
      },
      {
        agent: "codex-main",
        task: "task",
        stdout: codexStdout,
        stdout_path: path.join(codexDir, "stdout.jsonl"),
        elapsed_ms: 1000,
        usage: { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_tokens: 3, total_tokens: 26 },
      },
    ],
  }, paths)

  const taskReport = JSON.parse(fs.readFileSync(summary.benchmark_contracts.task_report_path, "utf8"))
  const webRun = JSON.parse(fs.readFileSync(summary.benchmark_contracts.web_run_path, "utf8"))
  const roundLogLines = fs.readFileSync(summary.benchmark_contracts.rounds_jsonl_path, "utf8").trim().split(/\r?\n/)
  assert.deepEqual(taskReport.rounds.map((round) => [round.metadata.agentId, round.metadata.roundSource, round.roundId, round.usage.totalTokens]), [
    ["tura-balanced", "provider-log", "call-tura-1", 120],
    ["tura-balanced", "provider-log", "call-tura-2", 160],
    ["codex-main", "token-usage-jsonl", "token-usage-4", 12],
    ["codex-main", "token-usage-jsonl", "token-usage-6", 14],
  ])
  assert.deepEqual(taskReport.rounds.map((round) => round.usage.cacheInputTokens), [70, 100, 4, 5])
  assert.deepEqual(taskReport.rounds.map((round) => [round.providerDurationMs, round.metadata.durationSource]), [
    [1000, "provider-log"],
    [2000, "provider-log"],
    [500, "result-elapsed-fallback"],
    [500, "result-elapsed-fallback"],
  ])
  assert.equal(taskReport.usage.totalTokens, 306)
  assert.equal(taskReport.usage.inputTokens, 251)
  assert.equal(taskReport.usage.outputTokens, 55)
  assert.equal(roundLogLines.length, 4)
  assert.equal(webRun.schema, "tura.benchmark.web-run.v1")
  assert.deepEqual(webRun.rounds.map((round) => [round.id, round.inputTokens, round.outputTokens, round.cacheInputTokens]), [
    ["call-tura-1", 100, 20, 70],
    ["call-tura-2", 130, 30, 100],
    ["token-usage-4", 10, 2, 4],
    ["token-usage-6", 11, 3, 5],
  ])
  assert.deepEqual(webRun.rounds.map((round) => round.durationSeconds), [1, 2, 1, 1])
  assert.deepEqual(Object.keys(summary.benchmark_contracts).sort(), [
    "git_diff_path",
    "harness_report_path",
    "rounds_jsonl_path",
    "task_report_path",
    "web_run_path",
    "cli_metadata_path",
  ].sort())
})

test("codex rollout token_count records become per-llm-round contracts", () => {
  const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tura-benchmark-codex-rollout-"))
  const paths = {
    test_name: "codex-rollout-contract",
    run_id: "run-codex-rollout",
    user_workspace: runRoot,
    target_root: runRoot,
    run_root: runRoot,
    summary_path: path.join(runRoot, "summary.json"),
  }
  const rolloutPath = path.join(runRoot, "rollout.jsonl")
  const commandRun = (timestamp, callId, commands) => ({
    timestamp,
    type: "response_item",
    payload: {
      type: "function_call",
      name: "command_run",
      call_id: callId,
      arguments: JSON.stringify({ commands }),
    },
  })
  const tokenCount = (timestamp, last, total) => ({
    timestamp,
    type: "event_msg",
    payload: { type: "token_count", info: { last_token_usage: last, total_token_usage: total } },
  })
  const records = [
    { timestamp: "2026-01-01T00:00:00.000Z", type: "event_msg", payload: { type: "task_started", turn_id: "turn-1" } },
    { timestamp: "2026-01-01T00:00:01.000Z", type: "event_msg", payload: { type: "agent_message", message: "First message.", phase: "commentary" } },
    commandRun("2026-01-01T00:00:02.000Z", "call-one", [{ command: "shell_command", command_line: "rg foo", step: 1 }]),
    tokenCount("2026-01-01T00:00:03.000Z", { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1, total_tokens: 12 }, { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1, total_tokens: 12 }),
    { timestamp: "2026-01-01T00:00:04.000Z", type: "response_item", payload: { type: "function_call_output", call_id: "call-one", output: JSON.stringify({ results: [{ step: 1, command: "shell_command", success: true, output: "foo.js" }] }) } },
    tokenCount("2026-01-01T00:00:05.000Z", { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1, total_tokens: 12 }, { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1, total_tokens: 12 }),
    commandRun("2026-01-01T00:00:06.000Z", "call-two", [{ command: "apply_patch", command_line: "PATCH", step: 1 }]),
    tokenCount("2026-01-01T00:00:07.000Z", { input_tokens: 11, cached_input_tokens: 5, output_tokens: 3, reasoning_output_tokens: 2, total_tokens: 14 }, { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_output_tokens: 3, total_tokens: 26 }),
    { timestamp: "2026-01-01T00:00:08.000Z", type: "response_item", payload: { type: "function_call_output", call_id: "call-two", output: JSON.stringify({ results: [{ step: 1, command: "apply_patch", success: true, output: {} }] }) } },
    { timestamp: "2026-01-01T00:00:09.000Z", type: "event_msg", payload: { type: "agent_message", message: "Done.", phase: "final" } },
    tokenCount("2026-01-01T00:00:10.000Z", { input_tokens: 12, cached_input_tokens: 6, output_tokens: 4, reasoning_output_tokens: 0, total_tokens: 16 }, { input_tokens: 33, cached_input_tokens: 15, output_tokens: 9, reasoning_output_tokens: 3, total_tokens: 42 }),
    { timestamp: "2026-01-01T00:00:10.500Z", type: "event_msg", payload: { type: "task_complete", turn_id: "turn-1" } },
  ]
  fs.writeFileSync(rolloutPath, records.map((record) => JSON.stringify(record)).join("\n") + "\n", "utf8")

  const summary = normalizeBusinessSummary({
    ok: true,
    model: "gpt-5.5",
    reasoning: "medium",
    service_tier: "default",
    results: [
      {
        agent: "codex-documents",
        task: "task",
        context_archive: { codex_rollout_paths: [rolloutPath] },
        usage: { input_tokens: 33, cached_input_tokens: 15, output_tokens: 9, reasoning_tokens: 3, total_tokens: 42 },
      },
    ],
  }, paths)

  const taskReport = JSON.parse(fs.readFileSync(summary.benchmark_contracts.task_report_path, "utf8"))
  const webRun = JSON.parse(fs.readFileSync(summary.benchmark_contracts.web_run_path, "utf8"))
  assert.equal(taskReport.rounds.length, 3)
  assert.deepEqual(taskReport.rounds.map((round) => [round.roundId, round.metadata.roundSource, round.metadata.usageEventSource, round.metadata.durationSource]), [
    ["call-one", "codex-rollout", "codex-rollout-token-count", "codex-rollout"],
    ["call-two", "codex-rollout", "codex-rollout-token-count", "codex-rollout"],
    ["codex-rollout-3", "codex-rollout", "codex-rollout-token-count", "codex-rollout"],
  ])
  assert.deepEqual(taskReport.rounds.map((round) => round.usage.totalTokens), [12, 14, 16])
  assert.deepEqual(taskReport.rounds.map((round) => round.messages.map((message) => message.text)), [["First message."], [], ["Done."]])
  assert.deepEqual(webRun.rounds.map((round) => [round.id, round.commands.length, round.commands[0]?.commandLine, round.commands[0]?.stdout]), [
    ["call-one", 1, "rg foo", "foo.js"],
    ["call-two", 1, "PATCH", "{}"],
    ["codex-rollout-3", 0, undefined, undefined],
  ])
  assert.deepEqual(taskReport.usage, {
    inputTokens: 33,
    cacheInputTokens: 15,
    outputTokens: 9,
    reasoningTokens: 3,
    totalTokens: 42,
    providerDurationMs: 0,
    llmRoundCount: 3,
  })
})

test("pi and opencode stdout tool events become unified web commands", () => {
  const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tura-benchmark-tools-"))
  const paths = {
    test_name: "tool-contract",
    run_id: "run-tools",
    user_workspace: runRoot,
    target_root: runRoot,
    run_root: runRoot,
    summary_path: path.join(runRoot, "summary.json"),
  }
  const piStdout = [
    { type: "turn_start", timestamp: 1783450000000 },
    {
      type: "message_update",
      message: {
        role: "assistant",
        content: [
          {
            type: "thinking",
            thinkingSignature: JSON.stringify({
              summary: [{ type: "summary_text", text: "Compacted previous parser investigation." }],
            }),
          },
        ],
      },
    },
    { type: "message_end", message: { role: "assistant", content: "Pi inspected files." } },
    { type: "tool_execution_start", toolCallId: "pi-call-1", toolName: "bash", args: { command: "rg Default" }, timestamp: 1783450000100 },
    { type: "tool_execution_end", toolCallId: "pi-call-1", toolName: "bash", result: { content: [{ type: "text", text: "parser/lexer.go" }] }, isError: false, timestamp: 1783450000300 },
    { type: "context.compact", usage: { input: 1, output: 2, cacheRead: 0, totalTokens: 3 } },
    { type: "turn_end", timestamp: 1783450000400, message: { usage: { input: 10, cacheRead: 5, output: 2, totalTokens: 17 } } },
  ].map((event) => JSON.stringify(event)).join("\n")
  const opencodeStdout = [
    { type: "step_start", timestamp: 1783450001000 },
    { type: "text", timestamp: 1783450001100, part: { text: "OpenCode ran a search." } },
    {
      type: "tool_use",
      timestamp: 1783450001200,
      part: {
        type: "tool",
        tool: "bash",
        callID: "open-call-1",
        state: {
          status: "completed",
          input: { command: "go test ./..." },
          output: "ok ./vm",
          time: { start: 1783450001210, end: 1783450001510 },
        },
      },
    },
    { type: "step_finish", timestamp: 1783450001600, part: { tokens: { input: 20, cache: { read: 7, write: 0 }, output: 3, total: 30 } } },
  ].map((event) => JSON.stringify(event)).join("\n")

  const summary = normalizeBusinessSummary({
    ok: true,
    model: "gpt-5.5",
    reasoning: "medium",
    service_tier: "default",
    results: [
      { agent: "pi-agent", task: "task", stdout: piStdout, usage: { input_tokens: 15, cached_input_tokens: 5, output_tokens: 2, total_tokens: 17 } },
      { agent: "opencode", task: "task", stdout: opencodeStdout, usage: { input_tokens: 27, cached_input_tokens: 7, output_tokens: 3, total_tokens: 30 } },
    ],
  }, paths)

  const taskReport = JSON.parse(fs.readFileSync(summary.benchmark_contracts.task_report_path, "utf8"))
  const webRun = JSON.parse(fs.readFileSync(summary.benchmark_contracts.web_run_path, "utf8"))
  assert.deepEqual(taskReport.rounds.map((round) => [round.metadata.agentId, round.toolCalls.length, round.providerDurationMs]), [
    ["pi-agent", 1, 400],
    ["opencode", 1, 600],
  ])
  assert.deepEqual(taskReport.rounds.map((round) => [round.metadata.agentId, round.metadata.usageEventSource, round.metadata.compactSummaryCount, round.metadata.compactSummaryTokenIncluded]), [
    ["pi-agent", "turn-end+compact-summary", 1, true],
    ["opencode", "step-finish", 0, false],
  ])
  assert.deepEqual(webRun.rounds.map((round) => [round.metadata.agentId, round.commands.length, round.commands[0].commandLine, round.commands[0].stdout]), [
    ["pi-agent", 1, "rg Default", "parser/lexer.go"],
    ["opencode", 1, "go test ./...", "ok ./vm"],
  ])
})
