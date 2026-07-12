#!/usr/bin/env node
import { spawn } from "node:child_process"
import path from "node:path"
import process from "node:process"
import { businessRunPaths } from "../business/business_lib_business_paths.mjs"

const repoRoot = path.resolve(import.meta.dirname, "..", "..")
const mainScript = path.join(
  repoRoot,
  "tests",
  "benchmark",
  "bug-fix",
  "retail_ops_defect_repair_agent_comparison.mjs",
)
const timeoutMs = process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || process.argv[2] || "300000"
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `long-task-${Date.now()}`
const runPaths = businessRunPaths("daily-ops-enterprise-task", runId)

const env = {
  ...process.env,
  COMMAND_RUN_AGENT_COMPACT_STRESS: "0",
  COMMAND_RUN_AGENT_ENTERPRISE_EXPANSION: "1",
  COMMAND_RUN_AGENT_HARD_ENTERPRISE_EXPANSION: process.env.COMMAND_RUN_AGENT_HARD_ENTERPRISE_EXPANSION || "1",
  COMMAND_RUN_AGENT_HARD_ACTIVE_GENERATED_CODE: process.env.COMMAND_RUN_AGENT_HARD_ACTIVE_GENERATED_CODE || "1",
  COMMAND_RUN_AGENT_HARD_ACTIVE_SCALE_MULTIPLIER: process.env.COMMAND_RUN_AGENT_HARD_ACTIVE_SCALE_MULTIPLIER || "10",
  COMMAND_RUN_AGENT_FIXTURE_SCALE_MULTIPLIER: process.env.COMMAND_RUN_AGENT_FIXTURE_SCALE_MULTIPLIER || "3",
  COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS: "0",
  COMMAND_RUN_AGENT_AGENTS:
    process.env.COMMAND_RUN_AGENT_AGENTS ||
    "tura-fast-planning-shll,tura-fast-shll,codex-cli",
  COMMAND_RUN_AGENT_CODEX_MODEL: process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5",
  COMMAND_RUN_AGENT_TURA_MODEL: process.env.COMMAND_RUN_AGENT_TURA_MODEL || "openai/gpt-5.5",
  COMMAND_RUN_AGENT_REASONING_EFFORT: process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low",
  COMMAND_RUN_AGENT_CODEX_SERVICE_TIER: process.env.COMMAND_RUN_AGENT_CODEX_SERVICE_TIER || "priority",
  COMMAND_RUN_AGENT_TURA_PRIORITY: process.env.COMMAND_RUN_AGENT_TURA_PRIORITY || "1",
  COMMAND_RUN_AGENT_TIMEOUT_MS: timeoutMs,
  COMMAND_RUN_AGENT_RUN_ID: runId,
  COMMAND_RUN_AGENT_RUN_ROOT: runPaths.run_root,
  COMMAND_RUN_AGENT_SUMMARY: runPaths.summary_path,
  COMMAND_RUN_AGENT_TEST_NAME: runPaths.test_name,
}

console.log(`[long-task-e2e] run_id=${runId}`)
console.log(`[long-task-e2e] timeout_ms=${timeoutMs}`)
console.log(`[long-task-e2e] fixture_scale_multiplier=${env.COMMAND_RUN_AGENT_FIXTURE_SCALE_MULTIPLIER}`)
console.log(`[long-task-e2e] hard_enterprise=${env.COMMAND_RUN_AGENT_HARD_ENTERPRISE_EXPANSION}`)
console.log(`[long-task-e2e] hard_active_generated_code=${env.COMMAND_RUN_AGENT_HARD_ACTIVE_GENERATED_CODE}`)
console.log(`[long-task-e2e] hard_active_scale_multiplier=${env.COMMAND_RUN_AGENT_HARD_ACTIVE_SCALE_MULTIPLIER}`)
console.log(`[long-task-e2e] fixed_context_tokens=${env.COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS}`)
console.log(`[long-task-e2e] agents=${env.COMMAND_RUN_AGENT_AGENTS}`)

const child = spawn(process.execPath, [mainScript], {
  cwd: repoRoot,
  env,
  stdio: "inherit",
  windowsHide: true,
})

child.on("exit", (code, signal) => {
  if (signal) {
    console.error(`[long-task-e2e] terminated by ${signal}`)
    process.exit(1)
  }
  process.exit(code ?? 1)
})
