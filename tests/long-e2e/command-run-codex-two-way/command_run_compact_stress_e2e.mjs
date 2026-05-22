#!/usr/bin/env node
import { spawn } from "node:child_process"
import path from "node:path"
import process from "node:process"

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..")
const mainScript = path.join(import.meta.dirname, "command_run_context_compact_e2e.mjs")
const timeoutMs = process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || process.argv[2] || "120000"
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `compact-stress-${Date.now()}`

const env = {
  ...process.env,
  COMMAND_RUN_AGENT_TIMEOUT_MS: timeoutMs,
  COMMAND_RUN_AGENT_RUN_ID: runId,
}

console.log(`[compact-stress] run_id=${runId}`)
console.log(`[compact-stress] timeout_ms=${timeoutMs}`)
console.log(`[compact-stress] agents=${env.COMMAND_RUN_AGENT_AGENTS}`)

const child = spawn(process.execPath, [mainScript], {
  cwd: repoRoot,
  env,
  stdio: "inherit",
  windowsHide: true,
})

child.on("exit", (code, signal) => {
  if (signal) {
    console.error(`[compact-stress] terminated by ${signal}`)
    process.exit(1)
  }
  process.exit(code ?? 1)
})
