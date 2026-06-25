#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { agentEventStats, agentUsageFromJsonl } from "./live_lib_agent_cli.mjs"
import { businessRunPaths, normalizeBusinessSummary } from "../business/business_lib_business_paths.mjs"
import { isolatedProcessOptions, killProcessTree } from "../business/business_lib_process_helpers.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `official-docs-search-smoke-${Date.now()}`
const runPaths = businessRunPaths("official-docs-search-smoke", runId)
const runRoot = runPaths.run_root
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 360_000)
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "openai/gpt-5.3-codex-spark"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const agentId = process.env.COMMAND_RUN_AGENT_AGENTS || "tura-fast-shll"
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")

function turaCliAgentName(id) {
  if (id.includes("fast")) return "fast"
  if (id.includes("thinking") || id.includes("planning")) return "thinking-planning"
  return "coding"
}

function runOk(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || 300_000,
    maxBuffer: 64 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    windowsHide: true,
  })
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}\n${result.stdout || ""}\n${result.stderr || ""}\n${result.error || ""}`)
  }
  return result
}

function runAsync(command, args, options = {}) {
  return new Promise((resolve) => {
    let stdout = ""
    let stderr = ""
    let settled = false
    let timedOut = false
    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
      ...isolatedProcessOptions(),
    })
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString()
    })
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString()
    })
    child.on("error", (error) => {
      if (settled) return
      settled = true
      resolve({ status: 1, signal: null, stdout, stderr, error: String(error?.stack || error?.message || error) })
    })
    child.on("close", (status, signal) => {
      if (settled) return
      settled = true
      resolve({
        status,
        signal,
        stdout,
        stderr,
        error: timedOut ? `Error: ${command} timed out after ${timeoutMs}ms` : null,
      })
    })
    const timer = setTimeout(() => {
      timedOut = true
      killProcessTree(child.pid)
    }, timeoutMs)
    child.on("close", () => clearTimeout(timer))
    if (options.input) {
      child.stdin.write(options.input)
    }
    child.stdin.end()
  })
}

function promptText() {
  return [
    "Use command_run with web_discover for a concise official documentation search.",
    "Find the current official Google Gemini image generation or text-to-image API documentation.",
    "Save a cleaned Markdown note to docs/gemini_image_api/official_image_generation.md.",
    "The note must include the official source URL, the model name or model family, and a minimal API call pattern.",
    "Use only official Google/Gemini documentation as the source; do not use blogs or mirrors.",
    "Final answer: list the kept relative path and the source URL.",
  ].join("\n")
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

function countMatches(text, pattern) {
  return (String(text || "").match(pattern) || []).length
}

function docsFiles(workspace) {
  const dir = path.join(workspace, "docs", "gemini_image_api")
  if (!fs.existsSync(dir)) return []
  return fs
    .readdirSync(dir)
    .map((name) => path.join(dir, name))
    .filter((file) => fs.statSync(file).isFile())
    .filter((file) => /\.(md|markdown|txt)$/i.test(file))
    .map((file) => {
      const text = fs.readFileSync(file, "utf8")
      return {
        path: path.relative(workspace, file).replaceAll("\\", "/"),
        bytes: fs.statSync(file).size,
        mentions_gemini: /\bgemini\b/i.test(text),
        mentions_image: /\bimage|text-to-image|imagen\b/i.test(text),
        official_source: /ai\.google\.dev|cloud\.google\.com|developers\.google\.com/i.test(text),
        text_tail: text.slice(-1200),
      }
    })
}

async function main() {
  fs.mkdirSync(runRoot, { recursive: true })
  if (!fs.existsSync(turaExe)) {
    runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_exec"], { cwd: repoRoot, timeoutMs: 300_000 })
  }
  assert(fs.existsSync(turaExe), `missing cli executable: ${turaExe}`)
  const workspace = path.join(runRoot, agentId, "workspace")
  const logs = path.join(runRoot, agentId, "logs")
  fs.mkdirSync(workspace, { recursive: true })
  fs.mkdirSync(logs, { recursive: true })
  const stdoutPath = path.join(logs, "tura.stdout.jsonl")
  const stderrPath = path.join(logs, "tura.stderr.log")
  const lastMessagePath = path.join(logs, "last-message.md")
  const started = Date.now()
  const result = await runAsync(turaExe, [
    "exec",
    "--skip-git-repo-check",
    "--json",
    "-C",
    workspace,
    "-m",
    model,
    "--agent-id",
    turaCliAgentName(agentId),
    "--sandbox",
    ...(process.env.COMMAND_RUN_AGENT_CODEX_SERVICE_TIER === "auto" ? [] : ["-p"]),
    "--model-reasoning-effort",
    reasoning,
    "--output-last-message",
    lastMessagePath,
  ], {
    cwd: workspace,
    input: promptText(),
    env: {
      TURA_COMMAND_RUN_SHELL: "shell_command",
      TURA_COMMAND_RUN_DISABLE_STRICT_JSON: "0",
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    },
  })
  fs.writeFileSync(stdoutPath, result.stdout)
  fs.writeFileSync(stderrPath, result.stderr)
  const files = docsFiles(workspace)
  const combined = [
    result.stdout,
    result.stderr,
    fs.existsSync(lastMessagePath) ? fs.readFileSync(lastMessagePath, "utf8") : "",
    files.map((file) => file.text_tail).join("\n"),
  ].join("\n")
  const events = parseJsonl(result.stdout)
  const checks = {
    process_exit_zero: result.status === 0,
    used_web_discover: countMatches(combined, /\bweb_discover\b/g) > 0,
    wrote_docs_file: files.some((file) => file.bytes > 200),
    docs_are_gemini_image_related: files.some((file) => file.mentions_gemini && file.mentions_image),
    docs_have_official_source: files.some((file) => file.official_source),
  }
  const summary = normalizeBusinessSummary({
    ok: Object.values(checks).every(Boolean),
    prompt: promptText(),
    agent: agentId,
    model,
    reasoning,
    timeout_ms: timeoutMs,
    duration_ms: Date.now() - started,
    exit_code: result.status,
    signal: result.signal,
    error: result.error,
    checks,
    workspace,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    last_message_path: lastMessagePath,
    docs_files: files,
    usage: agentUsageFromJsonl(result.stdout),
    events: agentEventStats(result.stdout),
    event_count: events.length,
    stderr_tail: result.stderr.slice(-2000),
  }, runPaths)
  fs.writeFileSync(runPaths.summary_path, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
