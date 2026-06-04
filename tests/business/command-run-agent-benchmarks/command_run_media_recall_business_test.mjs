#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runRoot = path.join(repoRoot, "target", "command-run-media-recall-e2e", String(Date.now()))
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")
const codexExe = path.join(
  process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex"),
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
)
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const agents = String(process.env.COMMAND_RUN_MEDIA_RECALL_AGENTS || "current,tura")
  .split(",")
  .map((item) => item.trim().toLowerCase())
  .filter(Boolean)
const skipTuraBuild = (process.env.COMMAND_RUN_AGENT_SKIP_TURA_BUILD || "0") === "1"

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || 240_000,
    maxBuffer: options.maxBuffer || 128 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
  })
  return {
    command,
    args,
    status: result.status,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    error: result.error ? String(result.error.stack || result.error.message || result.error) : null,
  }
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
  }
  return result
}

function writeFixture() {
  fs.mkdirSync(runRoot, { recursive: true })
  const py = String.raw`
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(r"${runRoot.replaceAll("\\", "\\\\")}")
img = Image.new("RGB", (420, 260), "white")
d = ImageDraw.Draw(img)
d.rectangle([0, 0, 90, 70], fill=(180, 0, 220))
d.text((18, 24), "STAR", fill=(255, 255, 255))
d.polygon([(45, 8), (54, 30), (78, 30), (58, 43), (66, 66), (45, 52), (24, 66), (32, 43), (12, 30), (36, 30)], fill=(255, 255, 255))
d.ellipse([145, 54, 275, 184], fill=(255, 210, 0), outline=(120, 90, 0), width=4)
d.text((178, 108), "YELLOW", fill=(0, 0, 0))
d.polygon([(330, 62), (285, 190), (380, 190)], fill=(0, 150, 70))
d.text((305, 202), "GREEN TRIANGLE", fill=(0, 90, 50))
img.save(root / "recall_panel.png")
`
  runOk("python", ["-c", py], { cwd: repoRoot })
  return path.join(runRoot, "recall_panel.png")
}

function parseJsonl(text) {
  return text
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

function collectText(...texts) {
  return texts.join("\n").toLowerCase()
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

function toolCount(text, pattern) {
  const matches = text.match(new RegExp(pattern, "gi"))
  return matches ? matches.length : 0
}

function latestProviderLogs(sinceMs) {
  const logDir = path.join(repoRoot, "log", "provider")
  if (!fs.existsSync(logDir)) return []
  const files = []
  for (const day of fs.readdirSync(logDir)) {
    const dayDir = path.join(logDir, day)
    if (!fs.statSync(dayDir).isDirectory()) continue
    for (const name of fs.readdirSync(dayDir)) {
      if (!name.endsWith(".json")) continue
      const file = path.join(dayDir, name)
      const stat = fs.statSync(file)
      if (stat.mtimeMs >= sinceMs) files.push(file)
    }
  }
  return files.sort()
}

function runTura(imagePath) {
  const sessionId = `media-recall-${Date.now()}`
  const turaDir = path.join(runRoot, "tura")
  fs.mkdirSync(turaDir, { recursive: true })
  const sinceMs = Date.now() - 2000
  const firstPrompt = [
    "Use command_run read_media to inspect this image, then answer only this question:",
    imagePath,
    "What color is the large circle in the center?",
  ].join("\n")
  const secondPrompt = "Without calling any tool or reading the image again, answer from the previous image context: what symbol is in the top-left box, and what color is the triangle on the right?"
  const common = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    sessionId,
    "--agent-id",
    "fast",
    "-m",
    process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`),
    "-p",
    "--model-reasoning-effort",
    reasoning,
    "--cwd",
    turaDir,
  ]
  const env = {
    TURA_COMMAND_RUN_SHELL: "shell_command",
    TURA_COMMAND_RUN_STRICT_JSON: "1",
    COMMAND_RUN_AGENT_TIMEOUT_MS: "180000",
  }
  const first = runOk(turaExe, [...common, firstPrompt], { cwd: turaDir, env })
  const second = runOk(turaExe, [...common, secondPrompt], { cwd: turaDir, env })
  fs.writeFileSync(path.join(turaDir, "turn1.stdout.jsonl"), first.stdout)
  fs.writeFileSync(path.join(turaDir, "turn2.stdout.jsonl"), second.stdout)
  fs.writeFileSync(path.join(turaDir, "turn1.stderr.log"), first.stderr)
  fs.writeFileSync(path.join(turaDir, "turn2.stderr.log"), second.stderr)
  const providerLogs = latestProviderLogs(sinceMs)
  const providerText = providerLogs.map((file) => fs.readFileSync(file, "utf8")).join("\n")
  const secondRequestHasImage = providerLogs.some((file) => {
    try {
      const value = JSON.parse(fs.readFileSync(file, "utf8"))
      const messages = value.request?.messages || []
      const text = JSON.stringify(messages)
      return /top-left|triangle/i.test(text) && /input_image[\s\S]{0,2000}data:image/i.test(text)
    } catch {
      return false
    }
  })
  const allText = collectText(first.stdout, second.stdout, providerText)
  const events = [...parseJsonl(first.stdout), ...parseJsonl(second.stdout)]
  const unsupportedMedia =
    /provider\/model does not support [`'"]?input_image/i.test(allText) ||
    /no endpoints found that support image input/i.test(allText)
  return {
    id: "tura",
    ok: allText.includes("yellow") && allText.includes("star") && allText.includes("green"),
    recall_ok: allText.includes("star") && allText.includes("green"),
    unsupported_media: unsupportedMedia,
    image_context_present_in_second_request: secondRequestHasImage,
    media_tool_calls: parseJsonl(first.stdout + "\n" + second.stdout).filter((event) => {
      const item = event.item || {}
      const output = item.aggregated_output || ""
      return item.type === "command_execution" && item.status === "completed" && /media_results/.test(output)
    }).length,
    usage: usageFromEvents(events),
    stdout_bytes: Buffer.byteLength(first.stdout + second.stdout, "utf8"),
    provider_log_count: providerLogs.length,
    provider_logs: providerLogs,
    dir: turaDir,
  }
}

function runCurrent(imagePath) {
  const currentDir = path.join(runRoot, "current")
  fs.mkdirSync(currentDir, { recursive: true })
  const firstPrompt = [
    "Use the attached image, then answer only this question:",
    "What color is the large circle in the center?",
  ].join("\n")
  const secondPrompt = "Without calling any tool or reading the image again, answer from the previous image context: what symbol is in the top-left box, and what color is the triangle on the right?"
  const common = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "-C",
    currentDir,
    "-m",
    model,
    "-i",
    imagePath,
    "--dangerously-bypass-approvals-and-sandbox",
    "-c",
    `model_reasoning_effort="${reasoning}"`,
    "-c",
    `service_tier="priority"`,
  ]
  const first = runOk(codexExe, [...common, firstPrompt], { cwd: currentDir })
  const threadId = parseJsonl(first.stdout).find((event) => event.type === "thread.started")?.thread_id
  assert(threadId, "current should emit thread.started")
  const second = runOk(
    codexExe,
    [
      "exec",
      "resume",
      "--json",
      "--skip-git-repo-check",
      "-m",
      model,
      "--dangerously-bypass-approvals-and-sandbox",
      "-c",
      `model_reasoning_effort="${reasoning}"`,
      "-c",
      `service_tier="priority"`,
      threadId,
      secondPrompt,
    ],
    { cwd: currentDir },
  )
  fs.writeFileSync(path.join(currentDir, "turn1.stdout.jsonl"), first.stdout)
  fs.writeFileSync(path.join(currentDir, "turn2.stdout.jsonl"), second.stdout)
  fs.writeFileSync(path.join(currentDir, "turn1.stderr.log"), first.stderr)
  fs.writeFileSync(path.join(currentDir, "turn2.stderr.log"), second.stderr)
  const allText = collectText(first.stdout, second.stdout)
  const events = [...parseJsonl(first.stdout), ...parseJsonl(second.stdout)]
  return {
    id: "current",
    ok: allText.includes("yellow") && allText.includes("star") && allText.includes("green"),
    recall_ok: allText.includes("star") && allText.includes("green"),
    image_context_present_in_second_request: null,
    media_tool_calls: 1,
    usage: usageFromEvents(events),
    stdout_bytes: Buffer.byteLength(first.stdout + second.stdout, "utf8"),
    thread_id: threadId,
    dir: currentDir,
  }
}

function main() {
  fs.mkdirSync(runRoot, { recursive: true })
  const imagePath = writeFixture()
  if (agents.includes("tura")) {
    if (!skipTuraBuild || !fs.existsSync(turaExe)) {
      runOk("cargo", ["build", "-p", "gateway", "--bin", "tura"], { cwd: repoRoot, timeoutMs: 240_000 })
    }
    assert(fs.existsSync(turaExe), `missing tura exe: ${turaExe}`)
  }
  if (agents.includes("current")) {
    assert(fs.existsSync(codexExe), `missing current codex exe: ${codexExe}`)
  }
  const results = [
    ...(agents.includes("current") ? [runCurrent(imagePath)] : []),
    ...(agents.includes("tura") ? [runTura(imagePath)] : []),
  ]
  assert(results.length > 0, "COMMAND_RUN_MEDIA_RECALL_AGENTS selected no supported agents")
  const expectMediaUnsupported = process.env.COMMAND_RUN_AGENT_EXPECT_MEDIA_UNSUPPORTED === "1"
  const ok = results.every((result) => {
    if (expectMediaUnsupported && result.id === "tura") {
      return result.unsupported_media && result.media_tool_calls > 0
    }
    return result.ok
  })
  const summary = {
    ok,
    run_root: runRoot,
    image_path: imagePath,
    model,
    reasoning,
    agents,
    skip_tura_build: skipTuraBuild,
    results,
  }
  fs.writeFileSync(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  assert(
    summary.ok,
    expectMediaUnsupported
      ? "media recall should pass for current and tura should clearly report unsupported image input"
      : "media recall should pass for current and tura",
  )
}

main()

