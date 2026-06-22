#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { agentEventStats, agentUsageFromJsonl, claudeCodeArgs, findClaudeExe, findPiExe, piAgentArgs } from "./live_lib_agent_cli.mjs"
import { businessRunPaths, normalizeBusinessSummary } from "../business/business_lib_business_paths.mjs"
import { isolatedProcessOptions, killProcessTree } from "../business/business_lib_process_helpers.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `multimedia-research-${Date.now()}`
const runPaths = businessRunPaths("media-official-research", runId)
const runRoot = runPaths.run_root
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const codexMainExe = findCodexMainExe()
const claudeExe = findClaudeExe()
const piExe = findPiExe()
const codexModel = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.3-codex-spark"
const model = process.env.COMMAND_RUN_AGENT_TURA_MODEL || `openai/${codexModel}`
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 240_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "tura-fast-shll,codex-main")
const reinforcedPrompt = envFlag("COMMAND_RUN_AGENT_REINFORCED_PROMPT")

function findCodexMainExe() {
  const exeName = process.platform === "win32" ? "codex.exe" : "codex"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_CODEX_MAIN_ROOT,
    path.join(homeDir, "Documents", "codex-main"),
    path.join(homeDir, "codex-main"),
    path.join(homeDir, "RustroverProjects", "codex-main"),
    path.join(homeDir, "Documents", "Codex"),
  ]
    .filter(Boolean)
    .map((root) => path.join(root, "codex-rs", "target", "debug", exeName))
  return candidates.find((candidate) => fs.existsSync(candidate)) || candidates[0]
}

function parseAgents(value) {
  const aliases = new Map([
    ["main", "codex-main"],
    ["codex-main", "codex-main"],
    ["codex_main", "codex-main"],
    ["tura", "tura-shll"],
    ["tura-fast", "tura-fast-shll"],
    ["tura-fast-shell", "tura-fast-shll"],
    ["tura-shell", "tura-shll"],
    ["tura-shll", "tura-shll"],
    ["tura-fast-shll", "tura-fast-shll"],
    ["claude", "claude-code"],
    ["claude-code", "claude-code"],
    ["claude-opus", "claude-code"],
    ["pi", "pi-agent"],
    ["pi-agent", "pi-agent"],
    ["pi-coding-agent", "pi-agent"],
  ])
  return String(value)
    .split(",")
    .map((item) => aliases.get(item.trim().toLowerCase()))
    .filter(Boolean)
    .filter((item, index, list) => list.indexOf(item) === index)
}

function envFlag(name) {
  const value = process.env[name]
  return value === "1" || value?.toLowerCase() === "true" || value?.toLowerCase() === "yes"
}

function turaCliAgentName(id) {
  return id.includes("fast") ? "fast" : "coding_agent"
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: options.maxBuffer || 256 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    windowsHide: true,
  })
  return {
    command,
    args,
    status: result.status,
    signal: result.signal,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    error: result.error ? String(result.error.stack || result.error.message || result.error) : null,
  }
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status || result.signal}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
  }
  return result
}

function runAsync(command, args, options = {}) {
  return new Promise((resolve) => {
    let settled = false
    let childExitStatus = null
    let childExitSignal = null
    const timeoutLimitMs = options.timeoutMs || timeoutMs
    let closeGraceTimer = null
    let timeoutGraceTimer = null
    const child = spawn(command, args, isolatedProcessOptions({
      cwd: options.cwd || repoRoot,
      shell: false,
      windowsHide: true,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["pipe", "pipe", "pipe"],
    }))
    let stdout = ""
    let stderr = ""
    let timedOut = false

    function settle(status, signal, error = null) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      clearTimeout(closeGraceTimer)
      clearTimeout(timeoutGraceTimer)
      resolve({
        command,
        args,
        status,
        signal,
        stdout,
        stderr,
        error: error || (timedOut ? `Error: ${command} timed out after ${timeoutLimitMs}ms` : null),
      })
    }

    const timer = setTimeout(() => {
      timedOut = true
      killProcessTree(child.pid)
      timeoutGraceTimer = setTimeout(() => {
        settle(childExitStatus ?? 1, childExitSignal)
      }, Number(options.timeoutCloseGraceMs || 3_000))
    }, timeoutLimitMs)
    child.stdout.setEncoding("utf8")
    child.stderr.setEncoding("utf8")
    child.stdout.on("data", (chunk) => {
      stdout += chunk
    })
    child.stderr.on("data", (chunk) => {
      stderr += chunk
    })
    child.on("error", (error) => {
      settle(null, null, String(error.stack || error.message || error))
    })
    child.on("exit", (status, signal) => {
      childExitStatus = status
      childExitSignal = signal
      closeGraceTimer = setTimeout(() => {
        settle(timedOut ? (status ?? 1) : status, signal)
      }, Number(options.exitCloseGraceMs || 1_000))
    })
    child.on("close", (status, signal) => {
      settle(timedOut ? (status ?? 1) : status, signal)
    })
    if (options.input) child.stdin.end(options.input)
    else child.stdin.end()
  })
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

function findCodexSessionFile(stdout) {
  const threadId = parseJsonl(stdout).find((event) => event.type === "thread.started")?.thread_id
  if (!threadId || !homeDir) return null
  const sessionsRoot = path.join(homeDir, ".codex", "sessions")
  if (!fs.existsSync(sessionsRoot)) return null
  const files = collectFiles(sessionsRoot)
    .filter((file) => file.endsWith(".jsonl"))
    .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs)
    .slice(0, 200)
  for (const file of files) {
    try {
      if (fs.readFileSync(file, "utf8").includes(threadId)) return file
    } catch {
      // Ignore unreadable or concurrently written session files.
    }
  }
  return null
}

function sessionStats(sessionPath) {
  if (!sessionPath || !fs.existsSync(sessionPath)) {
    return { found: false, session_path: sessionPath }
  }
  const text = fs.readFileSync(sessionPath, "utf8")
  const events = parseJsonl(text)
  return {
    found: true,
    session_path: sessionPath,
    bytes: Buffer.byteLength(text, "utf8"),
    event_count: events.length,
    turn_context_count: events.filter((event) => event.type === "turn_context").length,
    response_item_count: events.filter((event) => event.type === "response_item").length,
    token_count_events: events.filter((event) => event.type === "event_msg" && event.payload?.type === "token_count").length,
  }
}

function addUsage(total, usage) {
  if (!usage) return
  const input = Number(usage.input_tokens ?? usage.inputTokens ?? usage.prompt_tokens ?? 0)
  const cached = Number(
    usage.cached_input_tokens ??
      usage.input_token_details?.cached_tokens ??
      usage.input_tokens_details?.cached_tokens ??
      usage.prompt_tokens_details?.cached_tokens ??
      0,
  )
  const output = Number(usage.output_tokens ?? usage.outputTokens ?? usage.completion_tokens ?? 0)
  const reasoningTokens = Number(
    usage.reasoning_output_tokens ??
      usage.reasoning_tokens ??
      usage.reasoningTokens ??
      usage.output_tokens_details?.reasoning_tokens ??
      usage.completion_tokens_details?.reasoning_tokens ??
      0,
  )
  const tokens = Number(usage.total_tokens ?? usage.totalTokens ?? input + output + reasoningTokens)
  total.input_tokens += input
  total.cached_input_tokens += cached
  total.output_tokens += output
  total.reasoning_tokens += reasoningTokens
  total.total_tokens += tokens
  total.turns.push({
    input_tokens: input,
    cached_input_tokens: cached,
    output_tokens: output,
    reasoning_tokens: reasoningTokens,
    total_tokens: tokens,
  })
}

function usageFromStdout(stdout) {
  const total = {
    source: "tura-jsonl",
    input_tokens: 0,
    cached_input_tokens: 0,
    output_tokens: 0,
    reasoning_tokens: 0,
    total_tokens: 0,
    turns: [],
  }
  for (const event of parseJsonl(stdout)) {
    if (event.type === "turn.completed") addUsage(total, event.usage)
    if (event.type === "event_msg" && event.payload?.type === "token_count") {
      addUsage(total, event.payload?.info?.last_token_usage)
    }
  }
  total.llm_turns = total.turns.length
  return total
}

function usageFromLogText(text, source) {
  const total = {
    source,
    input_tokens: 0,
    cached_input_tokens: 0,
    output_tokens: 0,
    reasoning_tokens: 0,
    total_tokens: 0,
    turns: [],
  }
  const parsedLines = parseJsonl(text).filter((event) => !event.raw)
  if (parsedLines.length > 1) {
    for (const event of parsedLines) {
      const usage =
        event.usage ||
        (event.type === "event_msg" && event.payload?.type === "token_count"
          ? event.payload?.info?.last_token_usage || event.payload?.info
          : null)
      if (usage) addUsage(total, usage)
    }
  } else {
    const usages = []
    try {
      collectUsageObjects(JSON.parse(text), usages)
    } catch {
      // Ignore non-JSON logs.
    }
    const usage = usages.at(-1)
    if (usage) addUsage(total, usage)
  }
  total.llm_turns = total.turns.length
  return total
}

function usageFromFiles(paths, source) {
  return mergeUsage(
    ...paths
      .filter(Boolean)
      .filter((file) => fs.existsSync(file))
      .map((file) => usageFromLogText(fs.readFileSync(file, "utf8"), source)),
  )
}

function collectUsageObjects(value, out) {
  if (!value || typeof value !== "object") return
  if (value.usage) out.push(value.usage)
  if (value.payload?.type === "token_count") out.push(value.payload?.info?.last_token_usage || value.payload?.info)
  if (value.response?.usage) out.push(value.response.usage)
  if (value.result?.usage) out.push(value.result.usage)
  if (Array.isArray(value)) {
    for (const item of value) collectUsageObjects(item, out)
  } else {
    for (const item of Object.values(value)) collectUsageObjects(item, out)
  }
}

function mergeUsage(...items) {
  const total = {
    source: "aggregate",
    input_tokens: 0,
    cached_input_tokens: 0,
    output_tokens: 0,
    reasoning_tokens: 0,
    total_tokens: 0,
    turns: [],
  }
  for (const usage of items) {
    if (!usage) continue
    for (const key of ["input_tokens", "cached_input_tokens", "output_tokens", "reasoning_tokens", "total_tokens"]) {
      total[key] += Number(usage[key] || 0)
    }
    total.turns.push(...(usage.turns || []))
  }
  total.llm_turns = total.turns.length
  return total
}

function latestProviderLogs(sinceMs) {
  const logRoot = path.join(repoRoot, "log", "provider")
  if (!fs.existsSync(logRoot)) return []
  const files = []
  for (const day of fs.readdirSync(logRoot)) {
    const dir = path.join(logRoot, day)
    if (!fs.statSync(dir).isDirectory()) continue
    for (const name of fs.readdirSync(dir)) {
      if (!name.endsWith(".json")) continue
      const file = path.join(dir, name)
      if (fs.statSync(file).mtimeMs >= sinceMs) files.push(file)
    }
  }
  return files.sort()
}

function operationStats(stdout, providerLogs) {
  const text = `${stdout}\n${providerLogs.map((file) => fs.readFileSync(file, "utf8")).join("\n")}`
  const commandEvents = parseJsonl(stdout).filter((event) => event.type === "item.completed" && event.item?.type === "command_execution")
  return {
    command_execution_events: commandEvents.length,
    web_discover_mentions: countMatches(text, /\bweb_discover\b/g),
    read_media_mentions: countMatches(text, /\bread_media\b/g),
    apply_patch_mentions: countMatches(text, /\bapply_patch\b/g),
    shell_command_mentions: countMatches(text, /\bshell_command\b/g),
    command_events: commandEvents.map((event, index) => ({
      index,
      command: event.item?.command || "command_run",
      exit_code: event.item?.exit_code ?? null,
      output_tail: String(event.item?.aggregated_output || "").slice(-1200),
    })),
  }
}

function countMatches(text, pattern) {
  return (String(text || "").match(pattern) || []).length
}

function mediaFiles(workspace) {
  const mediaDir = path.join(workspace, "media", "newjeans_official")
  if (!fs.existsSync(mediaDir)) return []
  return fs
    .readdirSync(mediaDir)
    .map((name) => path.join(mediaDir, name))
    .filter((file) => fs.statSync(file).isFile())
    .filter((file) => /\.(png|jpe?g|webp|bmp)$/i.test(file))
    .map((file) => ({
      path: path.relative(workspace, file).replaceAll("\\", "/"),
      bytes: fs.statSync(file).size,
      member: memberFromPath(file),
      official_evidence: null,
    }))
}

function videoFiles(workspace) {
  const mediaDir = path.join(workspace, "media", "newjeans_video")
  if (!fs.existsSync(mediaDir)) return []
  return fs
    .readdirSync(mediaDir)
    .map((name) => path.join(mediaDir, name))
    .filter((file) => fs.statSync(file).isFile())
    .filter((file) => /\.(mp4|webm|mov|mkv)$/i.test(file))
    .map((file) => ({
      path: path.relative(workspace, file).replaceAll("\\", "/"),
      bytes: fs.statSync(file).size,
    }))
}

function mediaFilesWithEvidence(workspace, evidenceByPath) {
  return mediaFiles(workspace).map((file) => ({
    ...file,
    official_evidence: evidenceByPath.get(file.path) || null,
  }))
}

function memberFromPath(file) {
  const lower = file.toLowerCase()
  for (const member of ["minji", "hanni", "danielle", "haerin", "hyein"]) {
    if (lower.includes(member)) return member
  }
  return null
}

function verdictSeen(text) {
  return /\bSAME_STYLE\b/i.test(text)
}

function memberCoverage(text, files = []) {
  const lower = String(text || "").toLowerCase()
  const members = ["minji", "hanni", "danielle", "haerin", "hyein"]
  const required_members = ["minji", "hanni", "haerin", "hyein"]
  return {
    members,
    required_members,
    present: members.filter((member) => lower.includes(member) || files.some((file) => file.member === member)),
    required_present: required_members.filter((member) => lower.includes(member) || files.some((file) => file.member === member)),
  }
}

function certifiedHeadshots(files) {
  const members = ["minji", "hanni", "haerin", "hyein", "danielle"]
  return members
    .map((member) =>
      files
        .filter((file) => file.member === member && file.bytes > 5_000 && file.official_evidence)
        .sort((a, b) => b.bytes - a.bytes)[0],
    )
    .filter(Boolean)
}

function officialEvidenceFromUrl(url) {
  const lower = String(url || "").toLowerCase()
  if (/\/\/([^/]+\.)?newjeans\.jp\//.test(lower)) return "newjeans.jp"
  if (/\/\/([^/]+\.)?officialsite\.cds-jp\.online\//.test(lower)) return "officialsite.cds-jp.online"
  if (/\/\/([^/]+\.)?weverse\.io\//.test(lower)) return "weverse.io"
  if (/\/\/([^/]+\.)?hybecorp\.com\//.test(lower)) return "hybecorp.com"
  if (/\/\/([^/]+\.)?ador\.world\//.test(lower)) return "ador.world"
  if (/\/\/([^/]+\.)?newjeans\.kr\//.test(lower)) return "newjeans.kr"
  if (/\/\/([^/]+\.)?newjeans-official\.com\//.test(lower)) return "newjeans-official.com"
  return null
}

function downloadedEvidenceByPath(stdout, providerLogs) {
  const evidence = new Map()
  const ingestObject = (value) => {
    if (!value || typeof value !== "object") return
    if (Array.isArray(value)) {
      for (const item of value) ingestObject(item)
      return
    }
    const rawPath = typeof value.path === "string" ? value.path : null
    if (rawPath) {
      const normalized = rawPath.replaceAll("\\", "/")
      const official =
        officialEvidenceFromUrl(value.source_page_url) ||
        officialEvidenceFromUrl(value.page_url) ||
        officialEvidenceFromUrl(value.source_url) ||
        officialEvidenceFromUrl(value.url)
      if (official) evidence.set(normalized, official)
    }
    for (const item of Object.values(value)) ingestValue(item)
  }
  const ingestValue = (value) => {
    if (typeof value === "string") {
      const text = value.trim()
      if (!text.includes("downloaded_files") && !text.includes("source_page_url")) return
      for (const parsed of parsePossibleJsonStrings(text)) ingestObject(parsed)
      return
    }
    ingestObject(value)
  }
  for (const event of parseJsonl(stdout)) ingestObject(event)
  for (const file of providerLogs) {
    try {
      ingestObject(JSON.parse(fs.readFileSync(file, "utf8")))
    } catch {
      // Ignore malformed or concurrently written provider logs.
    }
  }
  return evidence
}

function parsePossibleJsonStrings(text) {
  const candidates = [text]
  if (text.includes('\\"')) candidates.push(text.replaceAll('\\"', '"').replaceAll("\\\\", "\\"))
  const parsed = []
  for (const candidate of candidates) {
    const start = candidate.indexOf("{")
    const end = candidate.lastIndexOf("}")
    if (start < 0 || end <= start) continue
    try {
      parsed.push(JSON.parse(candidate.slice(start, end + 1)))
    } catch {
      // Not a complete JSON payload.
    }
  }
  return parsed
}

function promptText() {
  const lines = [
    "Use command_run as one complete multimedia research batch. You have 4 minutes, so work efficiently.",
    "Goal: gather and describe three deliverable sets: official NewJeans member photos from the NewJeans official website, one NewJeans video, and the latest Gemini text-to-image API documentation.",
    "Requirements:",
    "- First delete useless old media/doc artifacts if present: media/newjeans_official, media/newjeans_video, docs/gemini_image_api, and docs/newjeans_sources.",
    "- Download all available official member photos for NewJeans members from the official NewJeans website/profile pages. Find the official website yourself, fetch the relevant profile page as a website, use the saved media links from the page, then download the direct official image URLs under media/newjeans_official. Include Minji, Hanni, Haerin, and Hyein; include Danielle only if an official NewJeans website profile/photo link is available.",
    "- The member photos must be exactly one solo photo per member.",
    "- The member photos must come from the same official visual set or clearly matching style and dimensions.",
    "- Inspect the downloaded official images with read_media and describe what they show. Delete only clearly unrelated junk if it is obvious.",
    "- Download one NewJeans group performance video. It does not need to be retro style. Keep it reasonably small, around 540p unless you need a different format. Save it under media/newjeans_video.",
    "- Find and download cleaned Markdown for the latest Google Gemini text-to-image/image generation API documentation, including the model name and call pattern. Save it under docs/gemini_image_api.",
    "- Use web_discover for search/download and read_media to inspect the downloaded images, video, and docs. You may also use shell_command for deleting bad files or listing final directories.",
    "Final answer must list the relative paths you kept and briefly describe the images, the video, and the Gemini API documentation.",
  ]
  if (reinforcedPrompt) {
    lines.splice(
      lines.length - 1,
      0,
      "Reinforced completion rule:",
      "- Do not exit or final-answer if the required official member images or Gemini API documentation are missing, mixed-source, unverified, or do not satisfy the count/style/source requirements.",
      "- If read_media or file/source metadata shows the images or docs are wrong, delete or ignore bad files, run another targeted web_discover/shell step, download direct relevant URLs, and read_media again before final-answering.",
      "- A final caveat that the required images or documentation were not found is not completion; continue searching and verifying until the requested files are present or the external timeout stops the run.",
    )
  }
  return lines.join("\n")
}

function artifactFiles(workspace) {
  const roots = ["media/newjeans_official", "media/newjeans_video", "docs/gemini_image_api", "docs/newjeans_sources"]
  return roots.flatMap((root) => {
    const dir = path.join(workspace, root)
    if (!fs.existsSync(dir)) return []
    return collectFiles(dir).map((file) => ({
      path: path.relative(workspace, file).replaceAll("\\", "/"),
      bytes: fs.statSync(file).size,
      modified_ms: Math.round(fs.statSync(file).mtimeMs),
    }))
  })
}

function collectFiles(dir) {
  const out = []
  for (const name of fs.readdirSync(dir)) {
    const file = path.join(dir, name)
    const stat = fs.statSync(file)
    if (stat.isDirectory()) out.push(...collectFiles(file))
    else if (stat.isFile()) out.push(file)
  }
  return out.sort()
}

async function runTuraAgent(id) {
  const workspace = path.join(runRoot, id, "workspace")
  const logs = path.join(runRoot, id, "logs")
  fs.mkdirSync(workspace, { recursive: true })
  fs.mkdirSync(logs, { recursive: true })
  const stdoutPath = path.join(logs, "tura.stdout.jsonl")
  const stderrPath = path.join(logs, "tura.stderr.log")
  const lastMessagePath = path.join(logs, "last-message.md")
  const providerSinceMs = Date.now() - 2000
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
    turaCliAgentName(id),
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
  const providerLogs = latestProviderLogs(providerSinceMs)
  const files = mediaFilesWithEvidence(workspace, downloadedEvidenceByPath(result.stdout, providerLogs))
  const videos = videoFiles(workspace)
  const fullText = `${result.stdout}\n${result.stderr}\n${fs.existsSync(lastMessagePath) ? fs.readFileSync(lastMessagePath, "utf8") : ""}`
  const operations = operationStats(result.stdout, providerLogs)
  const llm = mergeUsage(usageFromStdout(result.stdout), usageFromFiles(providerLogs, "tura-provider-log"))
  const coverage = memberCoverage(fullText, files)
  const certified = certifiedHeadshots(files)
  const requiredMemberImages = coverage.required_members.filter((member) =>
    files.some((file) => file.member === member && file.bytes > 5_000 && file.official_evidence),
  )
  const certificationComplete = requiredMemberImages.length === coverage.required_members.length && videos.some((file) => file.bytes > 50_000) && operations.web_discover_mentions > 0 && operations.read_media_mentions > 0
  const checks = {
    process_exit_zero_or_certified_before_timeout: result.status === 0 || certificationComplete,
    used_web_discover: operations.web_discover_mentions > 0,
    used_read_media: operations.read_media_mentions > 0,
    downloaded_required_member_images: requiredMemberImages.length === coverage.required_members.length,
    downloaded_group_dance_video: videos.some((file) => file.bytes > 50_000),
    mentioned_required_members: coverage.required_present.length === coverage.required_members.length,
    final_description_seen_or_harness_certified_media: result.status === 0 || certificationComplete,
  }
  return {
    id,
    agent: "tura",
    cli_agent: turaCliAgentName(id),
    model,
    reasoning,
    ok: Object.values(checks).every(Boolean),
    checks,
    duration_ms: Date.now() - started,
    exit_code: result.status,
    signal: result.signal,
    error: result.error,
    workspace,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    last_message_path: lastMessagePath,
    provider_logs: providerLogs,
    session_log_paths: {
      stdout_jsonl: stdoutPath,
      stderr_log: stderrPath,
      last_message: lastMessagePath,
      provider_logs: providerLogs,
    },
    downloaded_media: files,
    downloaded_videos: videos,
    certified_headshots: certified,
    required_member_images: requiredMemberImages,
    harness_verdict: certificationComplete ? "MEDIA_DOWNLOADED_AND_INSPECTED" : null,
    harness_officiality_evidence: certificationComplete
      ? "Each certified file name includes an official/agency source marker such as Weverse, ADOR, HYBE, or NewJeans and was downloaded to media/newjeans_official."
      : null,
    member_coverage: coverage,
    operations,
    llm,
    stderr_tail: result.stderr.slice(-3000),
  }
}

async function runCodexMainAgent(id) {
  const workspace = path.join(runRoot, id, "workspace")
  const logs = path.join(runRoot, id, "logs")
  fs.mkdirSync(workspace, { recursive: true })
  fs.mkdirSync(logs, { recursive: true })
  const stdoutPath = path.join(logs, "codex.stdout.jsonl")
  const stderrPath = path.join(logs, "codex.stderr.log")
  const lastMessagePath = path.join(logs, "last-message.md")
  const started = Date.now()
  const result = await runAsync(codexMainExe, [
    "exec",
    "--skip-git-repo-check",
    "--json",
    "-C",
    workspace,
    "-m",
    codexModel,
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
      TURA_COMMAND_RUN_SHELL: "shell_command",
      TURA_COMMAND_RUN_DISABLE_STRICT_JSON: "0",
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    },
  })
  fs.writeFileSync(stdoutPath, result.stdout)
  fs.writeFileSync(stderrPath, result.stderr)
  const fullText = `${result.stdout}\n${result.stderr}\n${fs.existsSync(lastMessagePath) ? fs.readFileSync(lastMessagePath, "utf8") : ""}`
  const sessionPath = findCodexSessionFile(result.stdout)
  const session = sessionStats(sessionPath)
  const llm = mergeUsage(usageFromStdout(result.stdout), usageFromFiles([sessionPath], "codex-session-jsonl"))
  if (session.token_count_events > llm.llm_turns) llm.llm_turns = session.token_count_events
  return {
    id,
    agent: "codex-main",
    model: codexModel,
    reasoning,
    ok: result.status === 0,
    duration_ms: Date.now() - started,
    exit_code: result.status,
    signal: result.signal,
    error: result.error,
    workspace,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    last_message_path: lastMessagePath,
    session_path: sessionPath,
    session_stats: session,
    session_log_paths: {
      stdout_jsonl: stdoutPath,
      stderr_log: stderrPath,
      last_message: lastMessagePath,
      codex_session_jsonl: sessionPath,
    },
    artifacts: artifactFiles(workspace),
    downloaded_media: mediaFiles(workspace),
    downloaded_videos: videoFiles(workspace),
    operations: operationStats(result.stdout, []),
    llm,
    final_text_tail: fullText.slice(-4000),
    stderr_tail: result.stderr.slice(-3000),
  }
}

async function runExternalCliAgent(id) {
  const workspace = path.join(runRoot, id, "workspace")
  const logs = path.join(runRoot, id, "logs")
  fs.mkdirSync(workspace, { recursive: true })
  fs.mkdirSync(logs, { recursive: true })
  const stdoutPath = path.join(logs, `${id}.stdout.jsonl`)
  const stderrPath = path.join(logs, `${id}.stderr.log`)
  const prompt = promptText()
  const isClaude = id === "claude-code"
  const started = Date.now()
  const result = await runAsync(isClaude ? claudeExe : piExe, isClaude
    ? claudeCodeArgs(prompt, { model: process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus" })
    : piAgentArgs(prompt), {
    cwd: workspace,
  })
  fs.writeFileSync(stdoutPath, result.stdout)
  fs.writeFileSync(stderrPath, result.stderr)
  const fullText = `${result.stdout}\n${result.stderr}`
  const files = mediaFiles(workspace)
  const videos = videoFiles(workspace)
  const coverage = memberCoverage(fullText, files)
  const operations = operationStats(result.stdout, [])
  const checks = {
    process_exit_zero: result.status === 0,
    downloaded_member_images: files.filter((file) => file.bytes > 5_000).length >= 4,
    downloaded_group_dance_video: videos.some((file) => file.bytes > 50_000),
    mentioned_members: coverage.present.length >= 4,
    saved_gemini_docs: artifactFiles(workspace).some((file) => /docs\/gemini_image_api\//.test(file.path) && file.bytes > 500),
  }
  const llm = agentUsageFromJsonl(result.stdout)
  return {
    id,
    agent: id,
    model: isClaude ? process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus" : "pi",
    reasoning,
    ok: Object.values(checks).every(Boolean),
    checks,
    duration_ms: Date.now() - started,
    exit_code: result.status,
    signal: result.signal,
    error: result.error,
    workspace,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    session_log_paths: {
      stdout_jsonl: stdoutPath,
      stderr_log: stderrPath,
    },
    artifacts: artifactFiles(workspace),
    downloaded_media: files,
    downloaded_videos: videos,
    operations,
    events: agentEventStats(result.stdout),
    llm,
    final_text_tail: fullText.slice(-4000),
    stderr_tail: result.stderr.slice(-3000),
  }
}

async function main() {
  fs.mkdirSync(runRoot, { recursive: true })
  if (agents.some((agent) => agent.startsWith("tura-"))) {
    runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_exec"], { cwd: repoRoot, timeoutMs: 300_000 })
    assert(fs.existsSync(turaExe), `missing cli executable: ${turaExe}`)
  }
  if (agents.includes("codex-main")) assert(fs.existsSync(codexMainExe), `missing codex-main executable: ${codexMainExe}`)
  assert(agents.length > 0, "COMMAND_RUN_AGENT_AGENTS did not select any supported agents")
  const results = await Promise.all(
    agents.map((agent) => {
      if (agent === "codex-main") return runCodexMainAgent(agent)
      if (agent === "claude-code" || agent === "pi-agent") return runExternalCliAgent(agent)
      return runTuraAgent(agent)
    })
  )
  for (const result of results) {
    if (!result.artifacts) result.artifacts = artifactFiles(result.workspace)
  }
  const totals = results.reduce(
    (acc, result) => {
      for (const key of ["input_tokens", "cached_input_tokens", "output_tokens", "reasoning_tokens", "total_tokens"]) {
        acc[key] += Number(result.llm?.[key] || 0)
      }
      return acc
    },
    { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, total_tokens: 0 },
  )
  const byAgentTokens = Object.fromEntries(results.map((result) => [result.id, result.llm || usageFromStdout("")]))
  const aggregateUsage = mergeUsage(...results.map((result) => result.llm))
  const summary = normalizeBusinessSummary({
    ok: results.every((result) => result.ok),
    prompt: promptText(),
    reinforced_prompt: reinforcedPrompt,
    agents,
    model,
    reasoning,
    timeout_ms: timeoutMs,
    results,
    by_agent_tokens: byAgentTokens,
    aggregate_usage: aggregateUsage,
    token_totals: totals,
  }, runPaths)
  const summaryPath = runPaths.summary_path
  fs.writeFileSync(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
