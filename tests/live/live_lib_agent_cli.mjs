import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import process from "node:process"

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
  const exeName = process.platform === "win32" ? "pi.cmd" : "pi"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_PI_EXE,
    exeName,
    "pi",
  ].filter(Boolean)
  return candidates.find((candidate) => candidate === exeName || candidate === "pi" || fs.existsSync(candidate)) || candidates[0]
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

export function piAgentArgs(prompt) {
  const model = process.env.COMMAND_RUN_AGENT_PI_MODEL || process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
  const thinking = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "medium"
  const normalizedModel = model.includes("/") ? model : `openai/${model}`
  const prefix = process.env.COMMAND_RUN_AGENT_PI_CLI_JS ? [process.env.COMMAND_RUN_AGENT_PI_CLI_JS] : []
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

export function agentUsageFromJsonl(text) {
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
