import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import process from "node:process"
import { codexTokenUsageReport } from "../../benchmark/lib/codex_token_usage.mjs"

export function agentHome() {
  return process.env.USERPROFILE || process.env.HOME || ""
}

export function findClaudeExe() {
  const home = agentHome()
  const exeName = process.platform === "win32" ? "claude.exe" : "claude"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_CLAUDE_EXE,
    process.platform === "win32"
      ? path.join(home, "AppData", "Local", "Packages", "Claude_pzs8sxrjxfjjc", "LocalCache", "Roaming", "Claude", "claude-code", "2.1.128", exeName)
      : null,
    "claude",
  ].filter(Boolean)
  return candidates.find((candidate) => candidate === "claude" || fs.existsSync(candidate)) || candidates[0]
}

export function findPiExe() {
  if (process.env.COMMAND_RUN_AGENT_PI_EXE) return process.env.COMMAND_RUN_AGENT_PI_EXE
  const cliJs = defaultPiCliJs()
  if (cliJs && fs.existsSync(cliJs)) return process.execPath
  const exeName = process.platform === "win32" ? "pi.cmd" : "pi"
  const candidates = [
    exeName,
    "pi",
  ].filter(Boolean)
  return candidates.find((candidate) => candidate === exeName || candidate === "pi" || fs.existsSync(candidate)) || candidates[0]
}

export function findOpencodeExe() {
  if (process.env.COMMAND_RUN_AGENT_OPENCODE_EXE) return process.env.COMMAND_RUN_AGENT_OPENCODE_EXE
  const packagedExe = defaultOpencodeExe()
  if (packagedExe && fs.existsSync(packagedExe)) return packagedExe
  const exeName = process.platform === "win32" ? "opencode.cmd" : "opencode"
  const candidates = [
    exeName,
    "opencode",
  ].filter(Boolean)
  return candidates.find((candidate) => candidate === exeName || candidate === "opencode" || fs.existsSync(candidate)) || candidates[0]
}

function defaultPiCliJs() {
  const home = agentHome()
  return process.platform === "win32"
    ? path.join(home, "AppData", "Roaming", "npm", "node_modules", "@earendil-works", "pi-coding-agent", "dist", "cli.js")
    : null
}

function defaultOpencodeExe() {
  const home = agentHome()
  if (process.platform !== "win32") return null
  return path.join(home, "AppData", "Roaming", "npm", "node_modules", "opencode-ai", "bin", "opencode.exe")
}

export function claudeCodeArgs(prompt, options = {}) {
  return [
    "--print",
    "--model",
    options.model || process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus",
    "--output-format",
    "stream-json",
    "--verbose",
    "--dangerously-skip-permissions",
    prompt,
  ]
}

export function piAgentArgs(prompt, options = {}) {
  const model = options.model || process.env.COMMAND_RUN_AGENT_PI_MODEL || process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "openai-codex/gpt-5.5"
  const thinking = options.reasoning || process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "medium"
  const normalizedModel = model.includes("/") ? model : `openai-codex/${model}`
  const cliJs = process.env.COMMAND_RUN_AGENT_PI_CLI_JS || (process.env.COMMAND_RUN_AGENT_PI_EXE ? "" : defaultPiCliJs())
  const prefix = cliJs ? [cliJs] : []
  if (process.platform === "win32" || String(prompt || "").length > 8000) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tura-pi-prompt-"))
    const promptPath = path.join(dir, "prompt.md")
    fs.writeFileSync(promptPath, prompt, "utf8")
    return [
      ...prefix,
      "--mode",
      "json",
      "--print",
      "--model",
      normalizedModel,
      "--thinking",
      thinking,
      `@${promptPath}`,
      "Complete the task exactly as described in the attached prompt file.",
    ]
  }
  return [...prefix, "--mode", "json", "--print", "--model", normalizedModel, "--thinking", thinking, prompt]
}

export function opencodeArgs(prompt, options = {}) {
  const model = options.model || process.env.COMMAND_RUN_AGENT_OPENCODE_MODEL || process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
  const normalizedModel = model.includes("/") ? model : `openai/${model}`
  const variant = options.reasoning || process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "medium"
  const baseArgs = ["run", "--format", "json", "--model", normalizedModel, "--variant", variant, "--auto"]
  if (String(prompt || "").length > 8000) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tura-opencode-prompt-"))
    const promptPath = path.join(dir, "prompt.md")
    fs.writeFileSync(promptPath, prompt, "utf8")
    return [...baseArgs, "--file", promptPath, "Complete the task exactly as described in the attached prompt file."]
  }
  return [...baseArgs, prompt]
}

export function agentUsageFromJsonl(text) {
  const codexUsage = codexTokenUsageReport(text)
  if (codexUsage.raw_event_count > 0) {
    return {
      input: codexUsage.totals.input_tokens,
      cached: codexUsage.totals.cached_input_tokens,
      output: codexUsage.totals.output_tokens,
      reasoning: codexUsage.totals.reasoning_tokens,
      total: codexUsage.totals.total_tokens,
    }
  }
  const usage = { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 }
  for (const event of parseJsonl(text)) {
    const candidates = [
      event.usage,
      event.message?.usage,
      event.result?.usage,
      event.assistantMessageEvent?.usage,
    ].filter(Boolean)
    for (const u of candidates) {
      usage.input += Number(u.input_tokens || u.prompt_tokens || 0)
      usage.cached += Number(u.cached_input_tokens || u.cache_read_input_tokens || u.input_tokens_details?.cached_tokens || 0)
      usage.output += Number(u.output_tokens || u.completion_tokens || 0)
      usage.reasoning += Number(u.reasoning_output_tokens || u.reasoning_tokens || u.output_tokens_details?.reasoning_tokens || 0)
      usage.total += Number(u.total_tokens || 0)
    }
  }
  return usage
}

export function agentEventStats(text) {
  const events = parseJsonl(text)
  return {
    events: events.length,
    turns: events.filter((event) => /turn[_-]start|turn\.started/i.test(String(event.type || ""))).length,
    tool_starts: events.filter((event) => /tool[_-]execution[_-]start/i.test(String(event.type || ""))).length,
    tool_ends: events.filter((event) => /tool[_-]execution[_-]end/i.test(String(event.type || ""))).length,
    errors: events.filter((event) => event.isError || /error/i.test(String(event.type || ""))).length,
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
