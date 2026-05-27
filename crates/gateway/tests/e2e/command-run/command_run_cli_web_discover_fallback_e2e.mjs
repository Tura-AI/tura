#!/usr/bin/env node
import { spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..", "..")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `google-image-api-fallback-${Date.now()}`
const runRoot = path.join(repoRoot, "target", "command-run-google-image-api-fallback-e2e", runId)
const workspace = path.join(runRoot, "workspace")
const logs = path.join(runRoot, "logs")
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 120_000)
const model = process.env.COMMAND_RUN_AGENT_TURA_MODEL || "openai/gpt-5.3-codex-spark"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"

function run(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: options.maxBuffer || 128 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    windowsHide: true,
  })
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed: ${result.status || result.signal}\n${result.stdout || ""}\n${result.stderr || ""}`)
  }
  return result
}

function walkFiles(dir) {
  if (!fs.existsSync(dir)) return []
  const out = []
  for (const name of fs.readdirSync(dir)) {
    const file = path.join(dir, name)
    const stat = fs.statSync(file)
    if (stat.isDirectory()) out.push(...walkFiles(file))
    else out.push(file)
  }
  return out
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
        return { raw: line }
      }
    })
}

function countMatches(text, re) {
  return (String(text || "").match(re) || []).length
}

function usageFromStdout(stdout) {
  const total = { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, total_tokens: 0, turns: [] }
  for (const event of parseJsonl(stdout)) {
    const usage = event.type === "turn.completed" ? event.usage : event.type === "event_msg" && event.payload?.type === "token_count" ? event.payload?.info?.last_token_usage : null
    if (!usage) continue
    const input = Number(usage.input_tokens ?? usage.prompt_tokens ?? 0)
    const cached = Number(usage.cached_input_tokens ?? usage.input_token_details?.cached_tokens ?? usage.prompt_tokens_details?.cached_tokens ?? 0)
    const output = Number(usage.output_tokens ?? usage.completion_tokens ?? 0)
    const reasoningTokens = Number(usage.reasoning_output_tokens ?? usage.output_tokens_details?.reasoning_tokens ?? usage.completion_tokens_details?.reasoning_tokens ?? 0)
    const tokens = Number(usage.total_tokens ?? input + output + reasoningTokens)
    total.input_tokens += input
    total.cached_input_tokens += cached
    total.output_tokens += output
    total.reasoning_tokens += reasoningTokens
    total.total_tokens += tokens
    total.turns.push({ input_tokens: input, cached_input_tokens: cached, output_tokens: output, reasoning_tokens: reasoningTokens, total_tokens: tokens })
  }
  total.llm_turns = total.turns.length
  return total
}

function promptText() {
  return [
    "You are testing fallback web discovery. Brave/main web search is disabled by the environment; do not try to re-enable it.",
    "Task: find accurate official Google documentation for the latest Google image generation engine/API call flow and save the useful documentation as Markdown files under docs/google_image_api.",
    "Only restriction: web searching must use the fallback search path. You may use any other available tools or commands to inspect, fetch, validate, retry, and save results.",
    "Expected behavior: adjust search keywords multiple times if needed, download/fetch the best official Google docs as md, and briefly state the final relative md paths and why they are the correct API-call docs.",
  ].join("\n")
}

function main() {
  fs.rmSync(runRoot, { recursive: true, force: true })
  fs.mkdirSync(workspace, { recursive: true })
  fs.mkdirSync(logs, { recursive: true })
  runOk("cargo", ["build", "-p", "gateway", "--bin", "tura"], { cwd: repoRoot, timeoutMs: 300_000 })

  const lastMessagePath = path.join(logs, "last-message.md")
  const started = Date.now()
  const result = run(turaExe, [
    "exec",
    "--skip-git-repo-check",
    "--json",
    "-C",
    workspace,
    "-m",
    model,
    "--agent",
    "coding_agent_fast",
    "--dangerously-bypass-approvals-and-sandbox",
    "-c",
    `model_reasoning_effort="${reasoning}"`,
    "-c",
    `service_tier="${process.env.COMMAND_RUN_AGENT_CODEX_SERVICE_TIER || "priority"}"`,
    "--output-last-message",
    lastMessagePath,
  ], {
    cwd: workspace,
    input: promptText(),
    env: {
      TURA_BRAVE_SEARCH_DISABLED: "1",
      TURA_COMMAND_RUN_SHELL: "shell_command",
      TURA_COMMAND_RUN_DISABLE_STRICT_JSON: "0",
    },
    timeoutMs,
  })
  fs.writeFileSync(path.join(logs, "tura.stdout.jsonl"), result.stdout || "")
  fs.writeFileSync(path.join(logs, "tura.stderr.log"), result.stderr || "")
  const lastMessage = fs.existsSync(lastMessagePath) ? fs.readFileSync(lastMessagePath, "utf8") : ""
  const files = walkFiles(path.join(workspace, "docs")).map((file) => ({
    path: path.relative(workspace, file).replaceAll("\\", "/"),
    bytes: fs.statSync(file).size,
    text_head: fs.readFileSync(file, "utf8").slice(0, 500),
  }))
  const allText = `${result.stdout || ""}\n${result.stderr || ""}\n${lastMessage}\n${files.map((file) => file.text_head).join("\n")}`
  const summary = {
    ok: result.status === 0 && files.some((file) => /google|cloud|ai|gemini|imagen|image/i.test(`${file.path}\n${file.text_head}`)),
    run_id: runId,
    run_root: runRoot,
    workspace,
    timeout_ms: timeoutMs,
    duration_ms: Date.now() - started,
    exit_code: result.status,
    signal: result.signal,
    error: result.error ? String(result.error.message || result.error) : null,
    prompt: promptText(),
    files,
    operations: {
      web_discover_mentions: countMatches(allText, /\bweb_discover\b/g),
      fallback_error_mentions: countMatches(allText, /fallback|DuckDuckGo|duckduckgo|website DuckDuckGo HTML fallback failed/gi),
      site_operator_mentions: countMatches(allText, /\bsite:/gi),
    },
    llm: usageFromStdout(result.stdout || ""),
    last_message: lastMessage,
    stderr_tail: String(result.stderr || "").slice(-3000),
  }
  fs.writeFileSync(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok) process.exitCode = 1
}

main()
