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
  process.env.PLANNING_BACKEND_TOPOLOGY_RUN_ID ||
  `planning-backend-topology-${Date.now()}`
const runRoot = path.join(repoRoot, "target", "planning-backend-topology", runId)
const summaryPath = path.join(runRoot, "summary.json")
const cargoTimeoutMs = Number(process.env.PLANNING_BACKEND_TOPOLOGY_TIMEOUT_MS || 180_000)

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

addCheck("schema hides ids and deliverable", () => {
  const schema = JSON.parse(read("crates/tools/src/commands/planning/schema.json"))
  assert.equal(schema.type, "array")
  assert.equal(schema.minItems, 2)
  assert.ok(!schema.items.properties.task_id, "schema must not expose task_id")
  assert.ok(!schema.items.properties.deliverable, "schema must not expose deliverable")
  assert.deepEqual(Object.keys(schema.items.properties).sort(), ["step", "task_summary"])
})

addCheck("prompt describes ordered replacement tasks without plan mutation or parallel steps", () => {
  const prompt = read("crates/tools/src/commands/planning/prompt.md")
  assert.match(prompt, /identify the authoritative sources/)
  assert.match(prompt, /required behavior, acceptance tests, fixtures, expected inputs\/outputs/)
  assert.match(prompt, /Only dispatch tasks after that discovery is done/)
  assert.match(prompt, /first planned step must be executable work/)
  assert.match(prompt, /not exploration/)
  assert.match(prompt, /Intermediate notes, summaries, markdown files, or generated checklists are working notes only/)
  assert.match(prompt, /not the source of truth/)
  assert.match(prompt, /authoritative sources/)
  assert.match(prompt, /final acceptance criteria/)
  assert.doesNotMatch(prompt, /Avoid `planning`/)
  assert.doesNotMatch(prompt, /only for the most complex 20%/)
  assert.doesNotMatch(prompt, /only when the user explicitly asks/)
  assert.doesNotMatch(prompt, /task_id/)
  assert.doesNotMatch(prompt, /`delivery`/)
  assert.doesNotMatch(prompt, /Omitted existing tasks are kept unchanged/)
  assert.doesNotMatch(prompt, /share the same `step`/)
  assert.doesNotMatch(prompt, /handoff/i)
  assert.doesNotMatch(prompt, /Write complete spec/)
  assert.match(prompt, /put it in the final position of that batch/)
  assert.match(prompt, /applies the new plan only after every command in the current batch has finished/)
  assert.match(prompt, /Each step needs a unique order number/)
  assert.match(prompt, /Map APIs, flags, fixtures, and source-backed behavior/)
  assert.match(prompt, /Implement the shared parser, IO, and core behavior model/)
  assert.match(prompt, /Run focused equivalence checks and finalize deliverable artifacts/)
  assert.doesNotMatch(prompt, /zip-password-finder/)
  assert.doesNotMatch(prompt, /official source, release binary, bundled fixtures/)
  assert.doesNotMatch(prompt, /unsupported scope/)
  assert.doesNotMatch(prompt, /Overall task requirements:/)
  assert.doesNotMatch(prompt, /Phase task:/)
  assert.doesNotMatch(prompt, /full overall task requirements/)
})

addCheck("command_run routes planning through the shared command consumer path", () => {
  const handler = read("crates/tools/src/command_run/handler.rs")
  const commandRunTests = read("crates/tools/tests/command_run_current_flow.rs")
  const commandRunSchema = JSON.parse(read("crates/tools/src/command_run/schema.json"))
  assert.equal(commandRunSchema.input_schema.properties.commands.minItems, 5)
  assert.match(handler, /"planning"\s*=>\s*ToolPayload::Function/)
  assert.match(handler, /normalize_planning_arguments/)
  assert.doesNotMatch(handler, /validate_planning_position/)
  assert.doesNotMatch(handler, /planning must be the final command in command_run/)
  assert.match(handler, /normalize_command_steps/)
  assert.match(handler, /run_macro_command_batch/)
  assert.match(handler, /run_command_run_step/)
  assert.match(commandRunTests, /pass_planning_command_routes_through_command_run/)
  assert.match(commandRunTests, /pass_same_step_commands_are_extended_to_unique_order/)
})

addCheck("runtime plan mapping replaces active task and renumbers ordered steps", () => {
  const runtime = read("crates/runtime/src/tool_flow/task_status.rs")
  assert.match(runtime, /object\.get\("deliverable"\)/)
  assert.match(runtime, /fn random_task_id\(\) -> String/)
  assert.match(runtime, /planning_plan_uses_unique_sequential_steps/)
  assert.match(runtime, /replace_active_task_with_planning/)
  assert.match(runtime, /splice\(index\.\.=index, incoming\)/)
  assert.match(runtime, /renumber_task_steps/)
  assert.match(runtime, /command_run_applies_status_before_later_planning_topology/)
  assert.match(runtime, /planning_replaces_active_task_and_preserves_queued_tail/)
  assert.doesNotMatch(runtime, /upsert_planning_plan/)
})

addCheck("gateway accepts task fields and derives child session context", () => {
  const taskStore = read("crates/gateway/src/session/store_task_management.rs")
  const storeTests = read("crates/gateway/src/session/store_tests.rs")
  assert.match(taskStore, /"task_id"/)
  assert.match(taskStore, /"deliverable"/)
  assert.match(taskStore, /string_field\(object, &\["deliverable"\]\)/)
  assert.match(storeTests, /multi_task_patch_matches_task_id_and_creates_defaulted_tasks/)
  assert.match(storeTests, /child_session_derives_workspace_and_task_instruction_context/)
})

addCheck("planning reuses command_run execution gate and file locks", () => {
  const router = read("crates/tools/src/runtime/tool.rs")
  const dispatch = read("crates/tools/src/runtime/dispatch.rs")
  const locks = read("crates/tools/src/runtime/file_locks/mod.rs")
  const handler = read("crates/tools/src/command_run/handler.rs")
  assert.match(router, /execution_gate/)
  assert.match(dispatch, /file_locks::acquire\(&access\)/)
  assert.match(dispatch, /force_exclusive/)
  assert.match(handler, /is_failed_apply_patch_result/)
  assert.match(handler, /ctx\.cancellation\.cancel\(\)/)
  assert.match(handler, /halt_after_apply_patch_failure/)
  assert.match(locks, /workspace_write_blocks_path_locks_until_released/)
})

const cargoTests = [
  {
    name: "tools planning command and schema coverage",
    args: ["test", "-p", "tools", "planning"],
  },
  {
    name: "runtime planning replacement keeps queued task order",
    args: [
      "test",
      "-p",
      "runtime",
      "planning_replaces_active_task_and_preserves_queued_tail",
    ],
  },
  {
    name: "command_run file lock workspace write barrier",
    args: ["test", "-p", "tools", "workspace_write_blocks_path_locks_until_released"],
  },
  {
    name: "command_run stops queued commands after apply_patch failure",
    args: ["test", "-p", "tools", "later_batch_commands_stop_after_apply_patch_failure"],
  },
  {
    name: "streaming command_run ignores commands after failed patch",
    args: ["test", "-p", "tools", "streaming_executor_ignores_commands_after_failed_apply_patch"],
  },
  {
    name: "gateway multi-task patch deliverable fields",
    args: ["test", "-p", "gateway", "multi_task_patch_matches_task_id_and_creates_defaulted_tasks"],
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
