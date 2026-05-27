#!/usr/bin/env node
import assert from "node:assert/strict"
import fs from "node:fs/promises"
import { readFileSync } from "node:fs"
import { spawnSync } from "node:child_process"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = process.env.REPO_ROOT || path.resolve(scriptDir, "..", "..", "..", "..")
const runId =
  process.env.MULTIPLE_TASKS_BACKEND_TOPOLOGY_RUN_ID ||
  `multiple-tasks-backend-topology-${Date.now()}`
const runRoot = path.join(repoRoot, "target", "multiple-tasks-backend-topology", runId)
const summaryPath = path.join(runRoot, "summary.json")
const cargoTimeoutMs = Number(process.env.MULTIPLE_TASKS_BACKEND_TOPOLOGY_TIMEOUT_MS || 180_000)

const checks = []
const cargoRuns = []
const started = performance.now()

function repoFile(relativePath) {
  return path.join(repoRoot, relativePath)
}

function read(relativePath) {
  return readFileSync(repoFile(relativePath), "utf8")
}

function addCheck(name, fn) {
  const checkStarted = performance.now()
  try {
    fn()
    checks.push({
      name,
      ok: true,
      duration_ms: Math.round(performance.now() - checkStarted),
    })
  } catch (error) {
    checks.push({
      name,
      ok: false,
      duration_ms: Math.round(performance.now() - checkStarted),
      error: error?.stack || String(error),
    })
  }
}

function runCargo(name, args) {
  const runStarted = performance.now()
  const result = spawnSync("cargo", args, {
    cwd: repoRoot,
    env: process.env,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    timeout: cargoTimeoutMs,
    windowsHide: true,
  })
  cargoRuns.push({
    name,
    command: ["cargo", ...args].join(" "),
    ok: result.status === 0,
    status: result.status,
    signal: result.signal,
    duration_ms: Math.round(performance.now() - runStarted),
    stdout_tail: tail(result.stdout || ""),
    stderr_tail: tail(result.stderr || ""),
    error: result.error ? String(result.error) : undefined,
  })
}

function tail(text, max = 6000) {
  return text.length > max ? text.slice(text.length - max) : text
}

addCheck("schema uses one task-management name set", () => {
  const schema = JSON.parse(read("crates/tools/src/commands/multiple_tasks/schema.json"))
  assert.equal(schema.type, "array")
  assert.equal(schema.minItems, 2)
  assert.ok(schema.items.properties.delivery, "schema must expose delivery")
  assert.deepEqual(Object.keys(schema.items.properties).sort(), [
    "delivery",
    "nonce_id",
    "step",
    "task_summary",
  ])
})

addCheck("prompt describes topology, ordered barriers, and doc-driven parallel work", () => {
  const prompt = read("crates/tools/src/commands/multiple_tasks/prompt.md")
  assert.match(prompt, /task topology/)
  assert.match(prompt, /task-management state machine/)
  assert.match(prompt, /only for the most complex 10%/)
  assert.match(prompt, /only when the user explicitly asks/)
  assert.match(prompt, /share the same `step`/)
  assert.match(prompt, /Integration, cross-module contract changes, and e2e validation must be later barrier steps/)
  assert.match(prompt, /Go to docs\/architecture/)
  assert.match(prompt, /Go to services\/server/)
  assert.match(prompt, /Go to apps\/frontend/)
  assert.match(prompt, /Go to docs\/acceptance/)
})

addCheck("command_run routes multiple_tasks through the shared command consumer path", () => {
  const handler = read("crates/tools/src/command_run/handler.rs")
  assert.match(handler, /"multiple_tasks"\s*=>\s*ToolPayload::Function/)
  assert.match(handler, /normalize_multiple_tasks_arguments/)
  assert.match(handler, /run_parallel_items/)
  assert.match(handler, /run_command_run_step/)
  assert.match(handler, /multiple_tasks_command_routes_through_command_run/)
})

addCheck("runtime plan mapping preserves same-step parallelism and delivery spelling", () => {
  const runtime = read("crates/runtime/src/manas/tool_execution.rs")
  assert.match(runtime, /object\.get\("delivery"\)/)
  assert.match(runtime, /multiple_tasks_plan_preserves_parallel_steps_and_delivery/)
  assert.match(runtime, /"step":\s*1[\s\S]*"step":\s*1/)
})

addCheck("gateway accepts task fields and derives child session context", () => {
  const store = read("crates/gateway/src/session/store.rs")
  assert.match(store, /"delivery"/)
  assert.match(store, /string_field\(object, &\["delivery"\]\)/)
  assert.match(store, /multi_task_patch_matches_nonce_and_creates_defaulted_tasks/)
  assert.match(store, /child_session_derives_workspace_and_task_instruction_context/)
})

addCheck("multiple_tasks reuses command_run execution gate and file locks", () => {
  const router = read("crates/tools/src/runtime/tool.rs")
  const locks = read("crates/tools/src/runtime/file_locks/mod.rs")
  const handler = read("crates/tools/src/command_run/handler.rs")
  assert.match(router, /execution_gate/)
  assert.match(router, /file_locks::acquire\(&access\)/)
  assert.match(router, /force_exclusive/)
  assert.match(handler, /is_failed_apply_patch_result/)
  assert.match(handler, /ctx\.cancellation\.cancel\(\)/)
  assert.match(handler, /halt_after_apply_patch_failure/)
  assert.match(locks, /workspace_write_blocks_path_locks_until_released/)
})

const cargoTests = [
  {
    name: "code-tools multiple_tasks command and schema coverage",
    args: ["test", "-p", "code-tools", "multiple_tasks"],
  },
  {
    name: "runtime multiple_tasks topology preserves parallel step order",
    args: [
      "test",
      "-p",
      "code-tools-suite",
      "multiple_tasks_plan_preserves_parallel_steps_and_delivery",
    ],
  },
  {
    name: "command_run file lock workspace write barrier",
    args: ["test", "-p", "code-tools", "workspace_write_blocks_path_locks_until_released"],
  },
  {
    name: "command_run stops queued commands after apply_patch failure",
    args: ["test", "-p", "code-tools", "later_batch_commands_stop_after_apply_patch_failure"],
  },
  {
    name: "streaming command_run ignores commands after failed patch",
    args: ["test", "-p", "code-tools", "streaming_executor_ignores_commands_after_failed_apply_patch"],
  },
  {
    name: "gateway multi-task patch delivery fields",
    args: ["test", "-p", "gateway", "multi_task_patch_matches_nonce_and_creates_defaulted_tasks"],
  },
  {
    name: "gateway child session derives workspace and task context",
    args: ["test", "-p", "gateway", "child_session_derives_workspace_and_task_instruction_context"],
  },
]

for (const test of cargoTests) {
  runCargo(test.name, test.args)
}

const summary = {
  ok: checks.every((check) => check.ok) && cargoRuns.every((run) => run.ok),
  run_id: runId,
  duration_ms: Math.round(performance.now() - started),
  checks,
  cargo_runs: cargoRuns,
}

await fs.mkdir(runRoot, { recursive: true })
await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2), "utf8")

console.log(JSON.stringify(summary, null, 2))
console.log(`summary: ${summaryPath}`)

if (!summary.ok) {
  process.exitCode = 1
}
