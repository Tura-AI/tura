#!/usr/bin/env node
import { spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { businessRunPaths, normalizeBusinessSummary } from "../lib/business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `claude-apply-patch-${Date.now()}`
const runPaths = businessRunPaths("tooling-claude-apply-patch-probe", runId)
const runRoot = runPaths.run_root
const workspace = path.join(runRoot, "workspace")
const summaryPath = runPaths.summary_path
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")
const model = process.env.COMMAND_RUN_AGENT_TURA_MODEL || "claude-code/claude-opus-4-8"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 300_000)
const sessionId = `apply-patch-probe-${Date.now()}`

function loadDotEnv(file) {
  if (!fs.existsSync(file)) return
  for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    const match = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"?([^"]*)"?\s*$/)
    if (match && !process.env[match[1]]) process.env[match[1]] = match[2]
  }
}

function runTura(prompt, label) {
  const stdoutPath = path.join(runRoot, `${label}.stdout.jsonl`)
  const stderrPath = path.join(runRoot, `${label}.stderr.log`)
  const started = performance.now()
  const result = spawnSync(
    turaExe,
    [
      "exec",
      "--json",
      "--skip-git-repo-check",
      "--session-id",
      sessionId,
      "--agent-id",
      "fast",
      "-m",
      model,
      "-p",
      "--model-reasoning-effort",
      reasoning,
      "--cwd",
      workspace,
      prompt,
    ],
    {
      cwd: workspace,
      env: {
        ...process.env,
        TURA_COMMAND_RUN_SHELL: "shell_command",
        TURA_COMMAND_RUN_STRICT_JSON: "1",
        COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
      },
      encoding: "utf8",
      text: true,
      timeout: timeoutMs,
      maxBuffer: 256 * 1024 * 1024,
    },
  )
  const durationMs = Math.round(performance.now() - started)
  const stdout = result.stdout || ""
  const stderr = result.stderr || ""
  fs.writeFileSync(stdoutPath, stdout)
  fs.writeFileSync(stderrPath, stderr)
  return {
    label,
    status: result.status,
    error: result.error ? String(result.error.stack || result.error.message || result.error) : null,
    duration_ms: durationMs,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    stdout,
    stderr,
  }
}

function parseJsonl(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try {
        return JSON.parse(line)
      } catch {
        return null
      }
    })
    .filter(Boolean)
}

function usageFromEvents(events) {
  const usage = { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, total_tokens: 0 }
  for (const event of events) {
    const u = event.usage || event.payload?.info?.last_token_usage
    if (!u) continue
    usage.input_tokens += Number(u.input_tokens || u.prompt_tokens || 0)
    usage.cached_input_tokens += Number(u.cached_input_tokens || u.input_tokens_details?.cached_tokens || u.prompt_tokens_details?.cached_tokens || 0)
    usage.output_tokens += Number(u.output_tokens || u.completion_tokens || 0)
    usage.reasoning_tokens += Number(u.reasoning_output_tokens || u.reasoning_tokens || u.output_tokens_details?.reasoning_tokens || u.completion_tokens_details?.reasoning_tokens || 0)
    usage.total_tokens += Number(u.total_tokens || 0)
  }
  return usage
}

function latestProviderLogs(sinceMs) {
  const root = path.join(repoRoot, "log", "provider")
  if (!fs.existsSync(root)) return []
  const files = []
  for (const day of fs.readdirSync(root)) {
    const dir = path.join(root, day)
    if (!fs.statSync(dir).isDirectory()) continue
    for (const name of fs.readdirSync(dir)) {
      if (!name.endsWith(".json")) continue
      const file = path.join(dir, name)
      if (fs.statSync(file).mtimeMs >= sinceMs) files.push(file)
    }
  }
  return files.sort()
}

function providerToolUseStats(files) {
  const stats = {
    response_tool_uses: 0,
    command_run_tool_uses: 0,
    native_edit_or_write_tool_uses: 0,
    apply_patch_in_response_tool_use: 0,
    apply_patch_in_response_text: 0,
    logs_with_apply_patch: [],
  }
  for (const file of files) {
    let value
    try {
      value = JSON.parse(fs.readFileSync(file, "utf8"))
    } catch {
      continue
    }
    const response = JSON.stringify(value.response || value.raw_response || value.data || "")
    if (response.includes("apply_patch")) {
      stats.apply_patch_in_response_text += 1
      stats.logs_with_apply_patch.push(file)
    }
    const blocks = value.response?.content || value.raw?.content || value.data?.content || []
    if (!Array.isArray(blocks)) continue
    for (const block of blocks) {
      if (block?.type !== "tool_use") continue
      stats.response_tool_uses += 1
      if (block.name === "command_run") stats.command_run_tool_uses += 1
      if (block.name === "Edit" || block.name === "Write") stats.native_edit_or_write_tool_uses += 1
      if (JSON.stringify(block.input || "").includes("apply_patch")) stats.apply_patch_in_response_tool_use += 1
    }
  }
  return stats
}

function commandStats(events) {
  const commands = events.filter((event) => event.item?.type === "command_execution").map((event) => event.item)
  return {
    command_execution_count: commands.length,
    apply_patch_executions: commands.filter((item) => item.command === "apply_patch" || item.command_type === "apply_patch").length,
    shell_executions: commands.filter((item) => item.command === "shell_command" || item.command_type === "shell_command").length,
    failed_executions: commands.filter((item) => item.status === "failed" || item.success === false).length,
    commands: commands.map((item) => ({
      command: item.command || item.command_type,
      status: item.status,
      output_head: String(item.aggregated_output || item.output || item.error || "").slice(0, 500),
    })),
  }
}

function containsAll(text, needles) {
  return needles.every((needle) => text.includes(needle))
}

loadDotEnv(path.join(repoRoot, ".env"))
fs.rmSync(runRoot, { recursive: true, force: true })
fs.mkdirSync(workspace, { recursive: true })
fs.writeFileSync(
  path.join(workspace, "README.md"),
  [
    "Claude apply_patch probe fixture.",
    "Create or update index.html only.",
    "",
  ].join("\n"),
)

const sinceMs = Date.now() - 2000
const prompt1 = [
  "Create a complete single-file browser Snake game in index.html.",
  "You must create or edit files through command_run using an apply_patch command, not shell redirection or heredocs.",
  "The file must contain the literal text SNAKE_SENTINEL.",
  "After patching, run a shell command that verifies index.html exists and contains SNAKE_SENTINEL.",
  "Keep the final answer brief.",
].join("\n")

const prompt2 = [
  "Update the same index.html.",
  "Add a start menu and a second Tetris mode. Keep the Snake game too.",
  "You must modify the file through command_run using an apply_patch command, not shell redirection or heredocs.",
  "The file must contain the literal texts MENU_SENTINEL and TETRIS_SENTINEL.",
  "After patching, run a shell command that verifies all three sentinels exist: SNAKE_SENTINEL, MENU_SENTINEL, TETRIS_SENTINEL.",
  "Keep the final answer brief.",
].join("\n")

const first = runTura(prompt1, "turn1")
const second = runTura(prompt2, "turn2")
const events = [...parseJsonl(first.stdout), ...parseJsonl(second.stdout)]
const providerLogs = latestProviderLogs(sinceMs)
const indexPath = path.join(workspace, "index.html")
const indexText = fs.existsSync(indexPath) ? fs.readFileSync(indexPath, "utf8") : ""
const summary = normalizeBusinessSummary({
  ok:
    first.status === 0 &&
    second.status === 0 &&
    containsAll(indexText, ["SNAKE_SENTINEL", "MENU_SENTINEL", "TETRIS_SENTINEL"]),
  run_id: runId,
  run_root: runRoot,
  workspace,
  session_id: sessionId,
  model,
  reasoning,
  wall_ms: first.duration_ms + second.duration_ms,
  turns: [
    { label: first.label, status: first.status, duration_ms: first.duration_ms, error: first.error },
    { label: second.label, status: second.status, duration_ms: second.duration_ms, error: second.error },
  ],
  usage: usageFromEvents(events),
  command_stats: commandStats(events),
  provider_log_count: providerLogs.length,
  provider_stats: providerToolUseStats(providerLogs),
  sentinels: {
    snake: indexText.includes("SNAKE_SENTINEL"),
    menu: indexText.includes("MENU_SENTINEL"),
    tetris: indexText.includes("TETRIS_SENTINEL"),
  },
  index_path: indexPath,
  summary_path: summaryPath,
}, runPaths)

fs.writeFileSync(summaryPath, JSON.stringify(summary, null, 2))
console.log(JSON.stringify(summary, null, 2))
process.exit(summary.ok ? 0 : 1)
