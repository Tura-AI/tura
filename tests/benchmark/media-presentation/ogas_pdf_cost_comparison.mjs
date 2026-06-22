#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { businessRunPaths, normalizeBusinessSummary } from "../lib/business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `ogas-pdf-${Date.now()}`
const runPaths = businessRunPaths("media-presentation-ogas-pdf", runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "high"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 5 * 60_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "codex,tura-thinking,tura-fast")
const skipTuraBuild = (process.env.COMMAND_RUN_AGENT_SKIP_TURA_BUILD || "0") === "1"
const allowFailure = (process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE || "1") !== "0"
const reportOnly = (process.env.COMMAND_RUN_AGENT_REPORT_ONLY || "0") === "1"
const existingRunRoot = process.env.COMMAND_RUN_AGENT_EXISTING_RUN_ROOT || ""

const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const codexExe = path.join(
  process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex"),
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
)

function parseAgents(value) {
  const alias = new Map([
    ["codex", "codex"],
    ["current", "codex"],
    ["codex-current", "codex"],
    ["tura", "tura-thinking"],
    ["tura-thinking", "tura-thinking"],
    ["thinking", "tura-thinking"],
    ["tura-fast", "tura-fast"],
    ["fast", "tura-fast"],
    ["tura-fast-text-only", "tura-fast-text-only"],
    ["fast-text-only", "tura-fast-text-only"],
    ["text-only", "tura-fast-text-only"],
  ])
  const counts = new Map()
  return String(value)
    .split(",")
    .map((item) => alias.get(item.trim().toLowerCase()))
    .filter(Boolean)
    .map((agent) => {
      const next = (counts.get(agent) || 0) + 1
      counts.set(agent, next)
      return next === 1 ? agent : `${agent}-${next}`
    })
}

function agentKind(agentId) {
  return String(agentId).replace(/-\d+$/, "")
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text)
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"))
}

function run(command, args, options = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: options.maxBuffer || 256 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    shell: options.shell || false,
    windowsHide: true,
  })
  return {
    command,
    args,
    status: result.status,
    signal: result.signal,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    duration_ms: Math.round(performance.now() - started),
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

function endWritable(stream) {
  return new Promise((resolve) => {
    if (!stream) {
      resolve()
      return
    }
    stream.once("finish", resolve)
    stream.once("error", resolve)
    stream.end()
  })
}

async function runLive(command, args, options = {}) {
  const started = performance.now()
  const stdoutPath = options.stdoutPath
  const stderrPath = options.stderrPath
  const statusPath = options.statusPath
  mkdirp(path.dirname(stdoutPath))
  const stdoutStream = fs.createWriteStream(stdoutPath, { flags: "w" })
  const stderrStream = fs.createWriteStream(stderrPath, { flags: "w" })
  let stdout = ""
  let stderr = ""
  let timedOut = false
  let childExitStatus = null
  let childExitSignal = null
  let settled = false

  writeFile(statusPath, JSON.stringify({ status: "running", started_at: new Date().toISOString(), command, args, cwd: options.cwd || repoRoot }, null, 2))
  return await new Promise((resolve) => {
    let closeGraceTimer = null
    let timeoutGraceTimer = null
    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["ignore", "pipe", "pipe"],
      shell: options.shell || false,
      windowsHide: true,
    })

    function settle(status, signal, error = null) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      clearTimeout(closeGraceTimer)
      clearTimeout(timeoutGraceTimer)
      const result = {
        command,
        args,
        status,
        signal,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        error: error || (timedOut ? `timed out after ${timeoutMs}ms` : null),
      }
      Promise.all([endWritable(stdoutStream), endWritable(stderrStream)]).finally(() => {
        writeFile(statusPath, JSON.stringify({ status: timedOut ? "timeout" : "closed", result }, null, 2))
        resolve(result)
      })
    }

    const timer = setTimeout(() => {
      timedOut = true
      killProcessTree(child.pid)
      timeoutGraceTimer = setTimeout(() => {
        settle(childExitStatus ?? 1, childExitSignal)
      }, 3000)
    }, options.timeoutMs || timeoutMs)

    child.stdout.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stdout += text
      stdoutStream.write(text)
    })
    child.stderr.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stderr += text
      stderrStream.write(text)
    })
    child.on("error", (error) => {
      settle(null, null, String(error.stack || error.message || error))
    })
    child.on("exit", (status, signal) => {
      childExitStatus = status
      childExitSignal = signal
      closeGraceTimer = setTimeout(() => {
        settle(timedOut ? (status ?? 1) : status, signal)
      }, 1000)
    })
    child.on("close", (status, signal) => {
      settle(timedOut ? (status ?? 1) : status, signal)
    })
  })
}

function killProcessTree(pid) {
  if (!pid) return
  try {
    if (process.platform === "win32") {
      spawnSync("taskkill", ["/pid", String(pid), "/t", "/f"], { windowsHide: true })
    } else {
      process.kill(-pid, "SIGTERM")
    }
  } catch {}
}

function serviceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return ["-c", `service_tier="${tier}"`]
}

function turaServiceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return tier === "priority" ? ["-p"] : []
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

function addUsage(total, usage) {
  if (!usage) return
  const input = Number(usage.input_tokens ?? usage.inputTokens ?? usage.prompt_tokens ?? 0)
  const cached = Number(
    usage.cached_input_tokens ??
      usage.cache_read_input_tokens ??
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
  const totalTokens = Number(usage.total_tokens ?? usage.totalTokens ?? input + output + reasoningTokens)
  total.input_tokens += input
  total.cached_input_tokens += cached
  total.output_tokens += output
  total.reasoning_tokens += reasoningTokens
  total.total_tokens += totalTokens
  total.turns.push({ input_tokens: input, cached_input_tokens: cached, output_tokens: output, reasoning_tokens: reasoningTokens, total_tokens: totalTokens })
}

function emptyUsage() {
  return { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, total_tokens: 0, turns: [] }
}

function usageFromEvents(events) {
  const total = emptyUsage()
  for (const event of events) {
    addUsage(total, event.usage)
    addUsage(total, event.message?.usage)
    if (event.type === "event_msg" && event.payload?.type === "token_count") {
      addUsage(total, event.payload?.info?.last_token_usage || event.payload?.info)
    }
  }
  total.llm_turns = total.turns.length
  return total
}

function eventStats(events) {
  const commands = new Map()
  for (const event of events) {
    const item = event.item || {}
    if (item.type !== "command_execution") continue
    const key = item.id || item.command || JSON.stringify(item).slice(0, 160)
    const existing = commands.get(key) || {}
    commands.set(key, { ...existing, ...item })
  }
  const finalCommands = [...commands.values()]
  const completedCommands = finalCommands.filter((item) => item.status === "completed")
  const succeededCommands = completedCommands.filter((item) => Number(item.exit_code || 0) === 0)
  const failedCommands = completedCommands.filter((item) => Number(item.exit_code || 0) !== 0)
  return {
    events: events.length,
    turns: events.filter((event) => event.type === "turn.started" || event.type === "thread.started").length,
    command_executions: finalCommands.length,
    completed_command_executions: completedCommands.length,
    successful_command_executions: succeededCommands.length,
    failed_command_executions: failedCommands.length,
    command_success_rate: completedCommands.length ? succeededCommands.length / completedCommands.length : null,
    command_completion_rate: finalCommands.length ? completedCommands.length / finalCommands.length : null,
    command_stats_source: finalCommands.length ? "stdout-jsonl" : "none",
  }
}

function mergeUsage(items) {
  const total = emptyUsage()
  for (const usage of items) {
    for (const key of ["input_tokens", "cached_input_tokens", "output_tokens", "reasoning_tokens", "total_tokens"]) {
      total[key] += Number(usage?.[key] || 0)
    }
    total.turns.push(...(usage?.turns || []))
  }
  total.llm_turns = total.turns.length
  return total
}

function providerLogRoot() {
  return path.join(repoRoot, "log", "provider")
}

function providerLogsForAgent(agentId, runIdText, sinceMs = 0, untilMs = Number.POSITIVE_INFINITY) {
  const root = providerLogRoot()
  if (!fs.existsSync(root)) return []
  const runNeedle = String(runIdText || "")
  const agentNeedle = `Agent: ${agentKind(agentId)}`
  const agentPathNeedles = [
    `\\${agentKind(agentId)}\\workspace`,
    `/${agentKind(agentId)}/workspace`,
    `${agentKind(agentId)}\\\\workspace`,
    `${agentKind(agentId)}/workspace`,
  ]
  const files = []
  for (const day of fs.readdirSync(root)) {
    const dir = path.join(root, day)
    if (!fs.statSync(dir).isDirectory()) continue
    for (const name of fs.readdirSync(dir)) {
      if (name.endsWith(".json")) files.push(path.join(dir, name))
    }
  }
  return files
    .filter((file) => {
      const mtime = fs.statSync(file).mtimeMs
      if (mtime < sinceMs || mtime > untilMs) return false
      let text = ""
      try {
        text = fs.readFileSync(file, "utf8")
      } catch {
        return false
      }
      if (runNeedle && !text.includes(runNeedle)) return false
      return text.includes(agentNeedle) || agentPathNeedles.some((needle) => text.includes(needle))
    })
    .sort()
}

function usageFromProviderLogs(files) {
  const items = []
  for (const file of files) {
    try {
      const data = readJson(file)
      addUsageFromProviderValue(items, data.metrics?.usage || data.response?.usage)
    } catch {}
  }
  return mergeUsage(items)
}

function addUsageFromProviderValue(items, usage) {
  if (!usage) return
  const total = emptyUsage()
  addUsage(total, {
    input_tokens: usage.input_tokens,
    cached_input_tokens: usage.cached_input_tokens ?? usage.input_tokens_details?.cached_tokens,
    output_tokens: usage.output_tokens,
    reasoning_tokens: usage.reasoning_tokens ?? usage.output_tokens_details?.reasoning_tokens,
    total_tokens: usage.total_tokens,
  })
  items.push(total)
}

function providerTiming(files) {
  const durations = []
  let success = 0
  let failure = 0
  for (const file of files) {
    try {
      const data = readJson(file)
      const duration = Number(data.duration_ms)
      if (Number.isFinite(duration)) durations.push(duration)
      if (data.success === true) success += 1
      else if (data.success === false) failure += 1
    } catch {}
  }
  const sum = durations.reduce((acc, value) => acc + value, 0)
  return {
    provider_call_count: files.length,
    provider_success_count: success,
    provider_failure_count: failure,
    provider_duration_ms_sum: Math.round(sum),
    provider_duration_ms_avg: durations.length ? Math.round(sum / durations.length) : null,
    provider_duration_ms_min: durations.length ? Math.round(Math.min(...durations)) : null,
    provider_duration_ms_max: durations.length ? Math.round(Math.max(...durations)) : null,
  }
}

function commandStatsFromProviderLogs(files) {
  if (!files.length) return null
  const newest = [...files].sort((left, right) => fs.statSync(right).mtimeMs - fs.statSync(left).mtimeMs)[0]
  let data
  try {
    data = readJson(newest)
  } catch {
    return null
  }
  const commands = new Map()
  const results = []
  const messages = Array.isArray(data.request?.messages) ? data.request.messages : []
  for (const message of messages) {
    if (typeof message.arguments === "string") {
      try {
        const parsed = JSON.parse(message.arguments)
        for (const command of parsed.commands || []) {
          const id = command.command_id || `${command.provider_tool_call_id}:${command.command_index}`
          commands.set(id, command)
        }
      } catch {}
    }
    const outputValues = Array.isArray(message.output)
      ? message.output.map((item) => (typeof item === "string" ? item : item?.text)).filter(Boolean)
      : typeof message.output === "string"
        ? [message.output]
        : []
    for (const text of outputValues) {
      try {
        const parsed = JSON.parse(text)
        for (const result of parsed.results || []) results.push(result)
      } catch {}
    }
  }
  const commandCount = commands.size
  const successCount = results.filter((item) => item.success === true).length
  const failureCount = results.filter((item) => item.success === false).length
  const completedCount = successCount + failureCount
  return {
    command_executions: commandCount,
    completed_command_executions: completedCount,
    successful_command_executions: successCount,
    failed_command_executions: failureCount,
    command_success_rate: completedCount ? successCount / completedCount : null,
    command_completion_rate: commandCount ? completedCount / commandCount : null,
    command_stats_source: "provider-log-request-history",
  }
}

function mergeEventAndProviderCommandStats(eventStatsValue, providerStatsValue) {
  if (eventStatsValue?.command_executions) return eventStatsValue
  if (!providerStatsValue) return eventStatsValue
  return { ...eventStatsValue, ...providerStatsValue }
}

function tps(outputTokens, durationMs) {
  const durationSeconds = Number(durationMs) / 1000
  if (!outputTokens || !Number.isFinite(durationSeconds) || durationSeconds <= 0) return null
  return outputTokens / durationSeconds
}

function round(value, digits = 3) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return null
  const scale = 10 ** digits
  return Math.round(Number(value) * scale) / scale
}

function pct(value) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return ""
  return `${round(Number(value) * 100, 1)}%`
}

function artifactByPath(result, suffix) {
  const artifacts = result.artifacts || []
  if (suffix.endsWith(".pdf")) {
    return artifacts.find((item) => item.path === suffix || item.path?.endsWith(suffix)) ||
      artifacts.find((item) => item.path?.toLowerCase().endsWith(".pdf")) ||
      null
  }
  return artifacts.find((item) => item.path === suffix || item.path?.endsWith(suffix)) || null
}

function enrichResultForReport(result, summary) {
  const refreshedArtifacts = result.workspace && fs.existsSync(result.workspace)
    ? collectArtifacts(result.workspace)
    : (result.artifacts || [])
  result = { ...result, artifacts: refreshedArtifacts }
  const stdoutPath = result.stdout_path
  const events = stdoutPath && fs.existsSync(stdoutPath) ? parseJsonl(fs.readFileSync(stdoutPath, "utf8")) : []
  const stdoutUsage = usageFromEvents(events)
  const stdoutMtime = stdoutPath && fs.existsSync(stdoutPath) ? fs.statSync(stdoutPath).mtimeMs : Date.now()
  const runDurationMs = result.run?.duration_ms ?? result.elapsed_ms ?? 0
  const providerSinceMs = result.telemetry?.provider_since_ms ?? Math.max(0, stdoutMtime - runDurationMs - 120_000)
  const providerUntilMs = result.telemetry?.provider_until_ms ?? stdoutMtime + 120_000
  const provider_logs = result.provider_logs?.length
    ? result.provider_logs
    : providerLogsForAgent(result.id, summary.run_id || runPaths.run_id, providerSinceMs, providerUntilMs)
  const providerUsage = usageFromProviderLogs(provider_logs)
  const fallbackUsage = result.usage?.total_tokens ? result.usage : stdoutUsage
  const usage = providerUsage.total_tokens > 0 ? providerUsage : fallbackUsage
  const provider_timer = providerTiming(provider_logs)
  const event_stats = eventStats(events)
  const provider_command_stats = commandStatsFromProviderLogs(provider_logs)
  const eventsAndCommands = mergeEventAndProviderCommandStats(event_stats, provider_command_stats)
  const pdf = artifactByPath(result, "deliverables/ogas-system-10-page.pdf")
  const sources = artifactByPath(result, "deliverables/sources.md")
  return {
    ...result,
    usage,
    events: eventsAndCommands,
    provider_logs,
    telemetry: {
      ...(result.telemetry || {}),
      usage_source: providerUsage.total_tokens > 0 ? "provider-log" : (stdoutUsage.total_tokens > 0 ? "stdout-jsonl" : "summary-existing"),
      stdout_usage: stdoutUsage,
      provider_usage: providerUsage,
      ...provider_timer,
      wall_output_tps: tps(usage.output_tokens, runDurationMs),
      provider_output_tps: tps(usage.output_tokens, provider_timer.provider_duration_ms_sum),
      pdf_bytes: pdf?.bytes || 0,
      sources_bytes: sources?.bytes || 0,
    },
  }
}

function metricRow(result) {
  const usage = result.usage || emptyUsage()
  const events = result.events || {}
  const telemetry = result.telemetry || {}
  const pdf = artifactByPath(result, "deliverables/ogas-system-10-page.pdf")
  const sources = artifactByPath(result, "deliverables/sources.md")
  return {
    agent: result.id,
    kind: result.kind,
    model: result.model,
    tura_agent: result.tura_agent || "",
    status: result.run?.status ?? "",
    duration_ms: result.run?.duration_ms ?? result.elapsed_ms ?? "",
    provider_call_count: telemetry.provider_call_count ?? 0,
    provider_duration_ms_sum: telemetry.provider_duration_ms_sum ?? 0,
    provider_duration_ms_avg: telemetry.provider_duration_ms_avg ?? "",
    provider_duration_ms_min: telemetry.provider_duration_ms_min ?? "",
    provider_duration_ms_max: telemetry.provider_duration_ms_max ?? "",
    input_tokens: usage.input_tokens || 0,
    cached_input_tokens: usage.cached_input_tokens || 0,
    output_tokens: usage.output_tokens || 0,
    reasoning_tokens: usage.reasoning_tokens || 0,
    total_tokens: usage.total_tokens || 0,
    wall_output_tps: round(telemetry.wall_output_tps),
    provider_output_tps: round(telemetry.provider_output_tps),
    command_executions: events.command_executions ?? 0,
    completed_command_executions: events.completed_command_executions ?? 0,
    successful_command_executions: events.successful_command_executions ?? 0,
    failed_command_executions: events.failed_command_executions ?? 0,
    command_success_rate: round(events.command_success_rate),
    command_completion_rate: round(events.command_completion_rate),
    usage_source: telemetry.usage_source || "",
    command_stats_source: events.command_stats_source || "",
    provider_log_count: result.provider_logs?.length || 0,
    pdf_bytes: pdf?.bytes || 0,
    pdf_path: pdf ? path.join(result.workspace, pdf.path) : "",
    sources_bytes: sources?.bytes || 0,
    sources_path: sources ? path.join(result.workspace, sources.path) : "",
  }
}

function csvEscape(value) {
  const text = value === null || value === undefined ? "" : String(value)
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
}

function rowsToCsv(rows) {
  const columns = [
    "agent",
    "kind",
    "model",
    "tura_agent",
    "status",
    "duration_ms",
    "provider_call_count",
    "provider_duration_ms_sum",
    "provider_duration_ms_avg",
    "provider_duration_ms_min",
    "provider_duration_ms_max",
    "input_tokens",
    "cached_input_tokens",
    "output_tokens",
    "reasoning_tokens",
    "total_tokens",
    "wall_output_tps",
    "provider_output_tps",
    "command_executions",
    "completed_command_executions",
    "successful_command_executions",
    "failed_command_executions",
    "command_success_rate",
    "command_completion_rate",
    "usage_source",
    "command_stats_source",
    "provider_log_count",
    "pdf_bytes",
    "pdf_path",
    "sources_bytes",
    "sources_path",
  ]
  return [
    columns.join(","),
    ...rows.map((row) => columns.map((column) => csvEscape(row[column])).join(",")),
  ].join("\n") + "\n"
}

function metricsMarkdown(metrics) {
  const lines = [
    "# OGAS PDF Cost Metrics",
    "",
    `Run: \`${metrics.run_id}\``,
    `Generated: ${metrics.generated_at}`,
    "",
    "| agent | status | duration | provider time | tokens total | input/cached/output/reasoning | wall TPS | provider TPS | commands | success | source |",
    "| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- |",
  ]
  for (const row of metrics.rows) {
    lines.push([
      row.agent,
      row.status,
      `${round(Number(row.duration_ms) / 1000, 1)}s`,
      row.provider_duration_ms_sum ? `${round(row.provider_duration_ms_sum / 1000, 1)}s/${row.provider_call_count}` : "",
      row.total_tokens,
      `${row.input_tokens}/${row.cached_input_tokens}/${row.output_tokens}/${row.reasoning_tokens}`,
      row.wall_output_tps ?? "",
      row.provider_output_tps ?? "",
      row.command_executions,
      pct(row.command_success_rate),
      `${row.usage_source}; ${row.command_stats_source}`,
    ].map((value) => String(value).replaceAll("|", "\\|")).join(" | ").replace(/^/, "| ").replace(/$/, " |"))
  }
  lines.push(
    "",
    "## Files",
    "",
    ...metrics.rows.flatMap((row) => [
      `- ${row.agent} PDF: \`${row.pdf_path}\` (${row.pdf_bytes} bytes)`,
      `- ${row.agent} sources: \`${row.sources_path}\` (${row.sources_bytes} bytes)`,
    ]),
    "",
    "## Notes",
    "",
    "- `wall TPS` = output tokens / end-to-end agent duration.",
    "- `provider TPS` = output tokens / summed provider-call duration, when provider logs are available.",
    "- Tura command counts come from provider request history when stdout does not expose command execution events.",
  )
  return lines.join("\n") + "\n"
}

function writeMetricReports(summary) {
  const reportRoot = summary.run_root || runRoot
  const enrichedResults = (summary.results || []).map((result) => enrichResultForReport(result, summary))
  const rows = enrichedResults.map(metricRow)
  const aggregateUsage = mergeUsage(enrichedResults.map((result) => result.usage))
  const metrics = {
    schema: "tura.benchmark.cost-metrics.v1",
    generated_at: new Date().toISOString(),
    run_id: summary.run_id,
    run_root: reportRoot,
    summary_path: summary.summary_path || path.join(reportRoot, "summary.json"),
    task: summary.task,
    model: summary.model,
    tura_model: summary.tura_model,
    reasoning: summary.reasoning,
    service_tier: summary.service_tier,
    aggregate_usage: aggregateUsage,
    aggregate: {
      duration_ms_sum: rows.reduce((acc, row) => acc + Number(row.duration_ms || 0), 0),
      provider_duration_ms_sum: rows.reduce((acc, row) => acc + Number(row.provider_duration_ms_sum || 0), 0),
      command_executions: rows.reduce((acc, row) => acc + Number(row.command_executions || 0), 0),
      completed_command_executions: rows.reduce((acc, row) => acc + Number(row.completed_command_executions || 0), 0),
      successful_command_executions: rows.reduce((acc, row) => acc + Number(row.successful_command_executions || 0), 0),
      failed_command_executions: rows.reduce((acc, row) => acc + Number(row.failed_command_executions || 0), 0),
    },
    rows,
    results: enrichedResults,
  }
  const files = {
    json: path.join(reportRoot, "metrics.json"),
    csv: path.join(reportRoot, "metrics.csv"),
    markdown: path.join(reportRoot, "metrics.md"),
  }
  writeFile(files.json, JSON.stringify(metrics, null, 2))
  writeFile(files.csv, rowsToCsv(rows))
  writeFile(files.markdown, metricsMarkdown(metrics))
  return { files, metrics }
}

function latestSummaryPath() {
  const target = path.join(runPaths.target_root, "media-presentation-ogas-pdf")
  if (!fs.existsSync(target)) return null
  const summaries = collectFiles(target)
    .filter((file) => path.basename(file) === "summary.json")
    .sort((left, right) => fs.statSync(right).mtimeMs - fs.statSync(left).mtimeMs)
  return summaries[0] || null
}

async function reportExistingRun() {
  const summaryFile = existingRunRoot
    ? path.join(existingRunRoot, "summary.json")
    : latestSummaryPath()
  assert(summaryFile && fs.existsSync(summaryFile), "missing existing summary.json for report-only mode")
  const summary = readJson(summaryFile)
  const { files, metrics } = writeMetricReports(summary)
  const patchedSummary = { ...summary, metric_files: files, aggregate_usage: metrics.aggregate_usage }
  writeFile(summaryFile, JSON.stringify(patchedSummary, null, 2))
  await flushOutput(JSON.stringify({ ok: true, report_only: true, summary_path: summaryFile, metric_files: files }, null, 2))
  process.exit(0)
}

function flushOutput(text, stream = process.stdout) {
  return new Promise((resolve) => {
    stream.write(`${text}\n`, resolve)
  })
}

function prepareWorkspace(agentId) {
  const workspace = path.join(runRoot, agentId, "workspace")
  fs.rmSync(workspace, { recursive: true, force: true })
  mkdirp(workspace)
  writeFile(path.join(workspace, "README-task.md"), taskReadme(agentId))
  return workspace
}

function taskReadme(agentId) {
  return "Create a 10-page English illustrated PDF about the OGAS system.\n"
}

function promptText(agentId) {
  return "Create a 10-page English illustrated PDF about the OGAS system."
}

function turaAgentName(agentId) {
  if (agentKind(agentId) === "tura-fast-text-only") return "fast-text-only"
  return agentKind(agentId) === "tura-fast" ? "fast" : "thinking"
}

async function runCodex(agentId, workspace, agentDir) {
  const result = await runLive(codexExe, [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "-C",
    workspace,
    "-m",
    model,
    "--dangerously-bypass-approvals-and-sandbox",
    "-c",
    `model_reasoning_effort="${reasoning}"`,
    ...serviceTierConfigArgs(),
    promptText(agentId),
  ], {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "codex.stdout.jsonl"),
    stderrPath: path.join(agentDir, "codex.stderr.log"),
    statusPath: path.join(agentDir, "codex.status.json"),
  })
  return result
}

async function runTura(agentId, workspace, agentDir) {
  const sessionId = `${agentId}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  const result = await runLive(turaExe, [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    sessionId,
    "--sandbox",
    "--agent-id",
    turaAgentName(agentId),
    "-m",
    turaModel,
    ...turaServiceTierConfigArgs(),
    "--model-reasoning-effort",
    reasoning,
    "--cwd",
    workspace,
    promptText(agentId),
  ], {
    cwd: workspace,
    timeoutMs,
    env: {
      OPENAI_LOGIN: process.env.OPENAI_LOGIN || "oauth",
      TURA_ENV_PATH: process.env.TURA_ENV_PATH || path.join(repoRoot, ".env"),
      TURA_COMMAND_RUN_SHELL: process.env.TURA_COMMAND_RUN_SHELL || "shell_command",
      TURA_COMMAND_RUN_STRICT_JSON: "0",
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    },
    stdoutPath: path.join(agentDir, "tura.stdout.jsonl"),
    stderrPath: path.join(agentDir, "tura.stderr.log"),
    statusPath: path.join(agentDir, "tura.status.json"),
  })
  result.session_id = sessionId
  return result
}

function collectArtifacts(workspace) {
  return collectFiles(workspace)
    .filter((file) => {
      const relative = path.relative(workspace, file).replaceAll("\\", "/")
      if (relative.toLowerCase().endsWith(".pdf")) return true
      if (/\/node_modules\/|\/\.git\/|\/target\/|\/dist\/|\/build\//.test(`/${relative}/`)) return false
      return /\.(pdf|md|png|jpe?g|webp|svg)$/i.test(file)
    })
    .map((file) => ({
    path: path.relative(workspace, file).replaceAll("\\", "/"),
    bytes: fs.statSync(file).size,
  }))
}

function collectFiles(dir) {
  const out = []
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name)
    if (entry.isDirectory()) out.push(...collectFiles(file))
    else if (entry.isFile()) out.push(file)
  }
  return out.sort()
}

async function runAgent(agentId) {
  const agentDir = path.join(runRoot, agentId)
  const workspace = prepareWorkspace(agentId)
  const providerSinceMs = Date.now() - 2000
  const started = performance.now()
  let result
  if (agentKind(agentId) === "codex") result = await runCodex(agentId, workspace, agentDir)
  else result = await runTura(agentId, workspace, agentDir)
  const providerUntilMs = Date.now() + 2000
  const events = parseJsonl(result.stdout)
  const stdoutUsage = usageFromEvents(events)
  const provider_logs = providerLogsForAgent(agentId, runPaths.run_id, providerSinceMs, providerUntilMs)
  const providerUsage = usageFromProviderLogs(provider_logs)
  const usage = providerUsage.total_tokens > 0 ? providerUsage : stdoutUsage
  const provider_timer = providerTiming(provider_logs)
  const event_stats = eventStats(events)
  const provider_command_stats = commandStatsFromProviderLogs(provider_logs)
  const eventsAndCommands = mergeEventAndProviderCommandStats(event_stats, provider_command_stats)
  const stats = {
    id: agentId,
    kind: agentKind(agentId),
    workspace,
    model: agentKind(agentId) === "codex" ? model : turaModel,
    tura_agent: agentKind(agentId).startsWith("tura-") ? turaAgentName(agentId) : null,
    reasoning,
    service_tier: serviceTier,
    elapsed_ms: Math.round(performance.now() - started),
    run: {
      status: result.status,
      signal: result.signal,
      duration_ms: result.duration_ms,
      error: result.error,
      stderr_tail: result.stderr.split(/\r?\n/).filter(Boolean).slice(-25).join("\n"),
    },
    telemetry: {
      provider_since_ms: Math.round(providerSinceMs),
      provider_until_ms: Math.round(providerUntilMs),
      usage_source: providerUsage.total_tokens > 0 ? "provider-log" : "stdout-jsonl",
      stdout_usage: stdoutUsage,
      provider_usage: providerUsage,
      ...provider_timer,
      wall_output_tps: tps(usage.output_tokens, result.duration_ms),
      provider_output_tps: tps(usage.output_tokens, provider_timer.provider_duration_ms_sum),
    },
    usage,
    events: eventsAndCommands,
    provider_logs,
    artifacts: collectArtifacts(workspace),
    stdout_path: agentKind(agentId) === "codex" ? path.join(agentDir, "codex.stdout.jsonl") : path.join(agentDir, "tura.stdout.jsonl"),
    stderr_path: agentKind(agentId) === "codex" ? path.join(agentDir, "codex.stderr.log") : path.join(agentDir, "tura.stderr.log"),
  }
  writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
  return stats
}

async function main() {
  mkdirp(runRoot)
  if (reportOnly) await reportExistingRun()
  assert(agents.length > 0, "COMMAND_RUN_AGENT_AGENTS selected no supported agents")
  if (agents.some((agent) => agentKind(agent) === "codex")) {
    assert(fs.existsSync(codexExe), `missing codex exe: ${codexExe}`)
  }
  if (agents.some((agent) => agentKind(agent).startsWith("tura-"))) {
    if (!skipTuraBuild || !fs.existsSync(turaExe)) {
      runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_exec"], { cwd: repoRoot, timeoutMs: 5 * 60_000 })
    }
    assert(fs.existsSync(turaExe), `missing tura exe: ${turaExe}`)
  }
  const results = await Promise.all(agents.map((agent) => {
    console.log(`[ogas-pdf-cost] running ${agent}`)
    return runAgent(agent)
  }))
  const aggregateUsage = mergeUsage(results.map((result) => result.usage))
  const summary = normalizeBusinessSummary({
    ok: results.every((result) => result.run.status === 0) || allowFailure,
    accepted_failures: allowFailure,
    evaluation_mode: "cost-only-human-quality-review",
    task: "Create a 10-page English illustrated PDF about the OGAS system.",
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
    agents,
    aggregate_usage: aggregateUsage,
    results,
  }, runPaths)
  const { files, metrics } = writeMetricReports(summary)
  summary.metric_files = files
  summary.aggregate_usage = metrics.aggregate_usage
  summary.standard_metrics.token_usage = metrics.aggregate_usage
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  await flushOutput(JSON.stringify(summary, null, 2))
  process.exit(summary.ok ? 0 : 1)
}

main().catch(async (error) => {
  const summary = normalizeBusinessSummary({
    ok: false,
    error: String(error?.stack || error?.message || error),
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    agents,
  }, runPaths)
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  await flushOutput(JSON.stringify(summary, null, 2), process.stderr)
  process.exit(1)
})
