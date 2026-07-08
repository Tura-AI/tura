#!/usr/bin/env node
import assert from "node:assert/strict"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { spawnSync } from "node:child_process"
import { fileURLToPath } from "node:url"

import { parseStringList, safeName, timestampId } from "./lib/debug_suite_matrix.mjs"

const benchmarkRoot = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(benchmarkRoot, "..")
const debugRoot = path.join(benchmarkRoot, "tasks", "debug")
const args = parseArgs(process.argv.slice(2))
const runId = args.runId || process.env.COMMAND_RUN_BENCHMARK_RUN_ID || `debug-matrix-${timestampId()}`
const selectedTaskIds = parseStringList(args.tasks || process.env.COMMAND_RUN_BENCHMARK_TASKS)
const agents = parseStringList(args.agents || process.env.COMMAND_RUN_BENCHMARK_AGENTS || process.env.COMMAND_RUN_AGENT_AGENTS, [
  "tura-balanced",
  "tura-direct",
  "codex-main",
])
const runHarnessArg = args.runHarness
assert(selectedTaskIds.length > 0, "pass --tasks or COMMAND_RUN_BENCHMARK_TASKS with benchmark task ids")
assert(agents.length > 0, "pass --agents or COMMAND_RUN_BENCHMARK_AGENTS with agent ids")

const declarations = discoverDebugDeclarations()
const byId = new Map(declarations.map((declaration) => [declaration.id, declaration]))
const selected = selectedTaskIds.map((taskId) => {
  const declaration = byId.get(taskId)
  assert(declaration, `unknown debug benchmark task id: ${taskId}`)
  return declaration
})
const groups = groupDeclarations(selected)
const harnessRequested = shouldRunHarness()
const agentResults = []
for (const group of groups) {
  agentResults.push(runGroup(group, "agent"))
}
const harnessResults = []
if (harnessRequested) {
  for (const group of groups) {
    harnessResults.push(runGroup(group, "harness"))
  }
}

const summary = {
  ok: [...agentResults, ...harnessResults].every((result) => result.exit_code === 0),
  run_id: runId,
  agents,
  selected_task_ids: selectedTaskIds,
  harness_requested: harnessRequested,
  phases: {
    agent: agentResults,
    harness: harnessResults,
  },
  groups: agentResults.map((agentResult) => ({
    ...agentResult,
    harness: harnessResults.find((result) => result.key === agentResult.key) || null,
  })),
}
console.log(JSON.stringify(summary, null, 2))
if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1

function parseArgs(argv) {
  const parsed = {}
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === "--tasks") parsed.tasks = argv[++index]
    else if (arg.startsWith("--tasks=")) parsed.tasks = arg.slice("--tasks=".length)
    else if (arg === "--agents") parsed.agents = argv[++index]
    else if (arg.startsWith("--agents=")) parsed.agents = arg.slice("--agents=".length)
    else if (arg === "--run-id") parsed.runId = argv[++index]
    else if (arg.startsWith("--run-id=")) parsed.runId = arg.slice("--run-id=".length)
    else if (arg === "--run-harness") {
      const next = argv[index + 1]
      parsed.runHarness = next && !next.startsWith("--") ? argv[++index] : "1"
    }
    else if (arg === "--no-harness") parsed.runHarness = "0"
    else if (arg.startsWith("--run-harness=")) parsed.runHarness = arg.slice("--run-harness=".length)
  }
  return parsed
}

function discoverDebugDeclarations() {
  return fs.readdirSync(debugRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(debugRoot, entry.name, "benchmark.task.json"))
    .filter((file) => fs.existsSync(file))
    .map((file) => JSON.parse(fs.readFileSync(file, "utf8")))
    .sort((left, right) => left.id.localeCompare(right.id))
}

function groupDeclarations(items) {
  const groups = new Map()
  for (const declaration of items) {
    const suite = declaration.upstream?.suite || declaration.id
    const key = suite === "deepswe" || suite === "swebench-verified" || suite === "swebench-pro"
      ? suite
      : declaration.id
    if (!groups.has(key)) groups.set(key, { key, suite, declarations: [] })
    groups.get(key).declarations.push(declaration)
  }
  return [...groups.values()]
}

function groupRunner(group) {
  if (group.suite === "deepswe") return path.join(debugRoot, "deepswe-anko-default-arguments", "runner.mjs")
  if (group.suite === "swebench-verified" || group.suite === "swebench-pro") {
    return path.join(debugRoot, "swebench-verified-issue-patch", "runner.mjs")
  }
  const declaration = group.declarations[0]
  const variant = declaration.variants.find((item) => item.default) || declaration.variants[0]
  return path.join(repoRoot, declaration.directory, variant.runner)
}

function groupEnv(group) {
  const env = {
    COMMAND_RUN_AGENT_AGENTS: agents.join(","),
    COMMAND_RUN_AGENT_RUN_ID: `${runId}-${safeName(group.key)}`,
    COMMAND_RUN_AGENT_BENCHMARK_TASK_NAME: `${safeName(group.key)}-matrix`,
  }
  if (group.suite === "deepswe") {
    env.COMMAND_RUN_AGENT_TASKS = JSON.stringify(unique(group.declarations.map((item) => item.upstream?.task).filter(Boolean)))
  }
  if (group.suite === "swebench-verified" || group.suite === "swebench-pro") {
    env.COMMAND_RUN_AGENT_TASKS = JSON.stringify(unique(group.declarations.map((item) => item.upstream?.instance_id).filter(Boolean)))
    env.COMMAND_RUN_AGENT_SWEBENCH_PREFIX = group.suite
    env.COMMAND_RUN_AGENT_SWEBENCH_DATASET = firstText(group.declarations.map((item) => item.upstream?.dataset))
    env.COMMAND_RUN_AGENT_SWEBENCH_REPOS = unique(group.declarations.map((item) => item.upstream?.repo).filter(Boolean)).join(",")
  }
  return env
}

function phaseEnv(group, phase) {
  const env = groupEnv(group)
  if (phase === "agent") {
    env.COMMAND_RUN_AGENT_HARNESS_ONLY = "0"
    env.COMMAND_RUN_AGENT_EVAL_ONLY = "0"
    if (group.suite === "deepswe") {
      env.COMMAND_RUN_AGENT_RUN_EVAL = "0"
    } else {
      env.COMMAND_RUN_AGENT_RUN_HARNESS = "0"
    }
    return env
  }
  assert.equal(phase, "harness")
  env.COMMAND_RUN_AGENT_HARNESS_ONLY = "1"
  if (group.suite === "deepswe") {
    env.COMMAND_RUN_AGENT_EVAL_ONLY = "1"
    env.COMMAND_RUN_AGENT_RUN_EVAL = "1"
  } else {
    env.COMMAND_RUN_AGENT_RUN_HARNESS = "1"
  }
  return env
}

function runGroup(group, phase) {
  const runner = groupRunner(group)
  const env = phaseEnv(group, phase)
  const startedAt = Date.now()
  const result = spawnSync(process.execPath, [runner], {
    cwd: repoRoot,
    env: { ...process.env, ...env },
    encoding: "utf8",
    text: true,
    timeout: Number(process.env.COMMAND_RUN_BENCHMARK_GROUP_TIMEOUT_MS || 24 * 60 * 60_000),
    maxBuffer: 1024 * 1024 * 1024,
    windowsHide: true,
  })
  return {
    phase,
    key: group.key,
    suite: group.suite,
    runner,
    task_ids: group.declarations.map((item) => item.id),
    upstream_tasks: JSON.parse(env.COMMAND_RUN_AGENT_TASKS || "[]"),
    agents,
    env,
    exit_code: result.status,
    signal: result.signal,
    duration_ms: Date.now() - startedAt,
    stdout_tail: tail(result.stdout || "", 12000),
    stderr_tail: tail((result.stderr || "") + (result.error ? `\n${result.error}` : ""), 12000),
  }
}

function shouldRunHarness() {
  if (runHarnessArg === "1" || runHarnessArg === "true") return true
  if (runHarnessArg === "0" || runHarnessArg === "false") return false
  if (process.env.COMMAND_RUN_AGENT_HARNESS_ONLY === "1") return true
  if (process.env.COMMAND_RUN_AGENT_RUN_HARNESS === "1") return true
  if (process.env.COMMAND_RUN_AGENT_RUN_EVAL === "1") return true
  return false
}

function firstText(values) {
  return values.find((value) => typeof value === "string" && value.trim()) || ""
}

function unique(values) {
  return [...new Set(values)]
}

function tail(text, maxChars) {
  return String(text || "").slice(-maxChars)
}
