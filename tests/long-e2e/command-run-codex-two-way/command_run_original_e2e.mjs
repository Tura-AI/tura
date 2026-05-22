#!/usr/bin/env node
import { spawn } from "node:child_process"
import path from "node:path"
import process from "node:process"

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..")
const mainScript = path.join(import.meta.dirname, "command_run_codex_two_way_e2e.mjs")
const timeoutMs = process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || process.argv[2] || "720000"
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `original-${Date.now()}`

const env = {
  ...process.env,
  COMMAND_RUN_AGENT_COMPACT_STRESS: "0",
  COMMAND_RUN_AGENT_ENTERPRISE_EXPANSION: "0",
  COMMAND_RUN_AGENT_FIXTURE_SCALE_MULTIPLIER: "1",
  COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS: "0",
  COMMAND_RUN_AGENT_TIMEOUT_MS: timeoutMs,
  COMMAND_RUN_AGENT_RUN_ID: runId,
}

console.log(`[original-e2e] run_id=${runId}`)
console.log(`[original-e2e] timeout_ms=${timeoutMs}`)
console.log(`[original-e2e] agents=${env.COMMAND_RUN_AGENT_AGENTS || "default"}`)

const child = spawn(process.execPath, [mainScript], {
  cwd: repoRoot,
  env,
  stdio: "inherit",
  windowsHide: true,
})

child.on("exit", (code, signal) => {
  if (signal) {
    console.error(`[original-e2e] terminated by ${signal}`)
    process.exit(1)
  }
  process.exit(code ?? 1)
})
