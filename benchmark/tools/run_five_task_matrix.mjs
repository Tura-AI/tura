#!/usr/bin/env node
import crypto from "node:crypto"
import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import process from "node:process"
import { spawn, spawnSync } from "node:child_process"
import { fileURLToPath, pathToFileURL } from "node:url"
import { businessRunPaths } from "../lib/business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..")
const targetRoot = path.join(userHome(), "Documents", "tura_workspace", "target")
const matrixName = "matrix-5tasks-gpt55-medium-default"
const args = parseArgs(process.argv.slice(2))
const runId = args["run-id"] || `matrix5-gpt55-medium-default-${timestamp()}`
const matrixRoot = path.resolve(args["root"] || path.join(targetRoot, matrixName, runId))
const maxConcurrentJobs = Number(args.concurrency || process.env.MATRIX_JOB_CONCURRENCY || 2)
const maxAttempts = Number(args.attempts || process.env.MATRIX_JOB_ATTEMPTS || 2)
const agents = (args.agents || "codex-main,tura-balanced,tura-direct").split(",").map((item) => item.trim()).filter(Boolean)
const model = args.model || "gpt-5.5"
const turaModel = args["tura-model"] || `openai/${model}`
const reasoning = args.reasoning || "medium"
const serviceTier = args["service-tier"] || "default"
const timeoutMs = Number(args.timeoutMs || args["timeout-ms"] || process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 60 * 60_000)
const monitorMs = Number(args.monitorMs || args["monitor-ms"] || process.env.MATRIX_MONITOR_MS || 60_000)
const sourcePortTasks = ["zip-password-finder", "nushell", "eza", "xsv"]
const repeatCount = Number(args.repeats || 2)
const progressJson = path.join(matrixRoot, "progress.json")
const progressMd = path.join(matrixRoot, "progress.md")
const progressHtml = path.join(matrixRoot, "progress.html")
const progressPdf = path.join(matrixRoot, "progress.pdf")
const phaseFile = path.join(matrixRoot, "phase.txt")
const nodeExe = process.execPath
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx"

mkdirp(matrixRoot)

const jobs = []
for (let repeat = 1; repeat <= repeatCount; repeat += 1) {
  for (const task of sourcePortTasks) {
    jobs.push(makeSourcePortJob(task, repeat))
  }
  jobs.push(makePromptGalleryJob(repeat))
}

const state = {
  run_id: runId,
  matrix_root: matrixRoot,
  started_at: new Date().toISOString(),
  updated_at: null,
  phase: "agent",
  config: {
    max_concurrent_runner_jobs: maxConcurrentJobs,
    max_concurrent_agents: maxConcurrentJobs * agents.length,
    repeats: repeatCount,
    agents,
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
  },
  jobs,
  pdf: { path: progressPdf, ok: false, error: null, updated_at: null },
}

let lastPdfAttempt = 0
let monitorTimer = null

async function main() {
  writePhase("agent")
  writeProgress()
  await runPhase("agent")
  writePhase("harness")
  for (const job of jobs) {
    job.status = "pending"
    job.started_at = null
    job.ended_at = null
    job.exit_code = null
    job.attempt = 0
    job.pid = null
    job.stdout = path.join(job.log_dir, `${job.id}.harness.stdout.log`)
    job.stderr = path.join(job.log_dir, `${job.id}.harness.stderr.log`)
  }
  state.phase = "harness"
  writeProgress(true)
  await runPhase("harness")
  state.phase = "complete"
  state.ended_at = new Date().toISOString()
  writePhase("complete")
  writeProgress(true)
}

async function runPhase(phase) {
  const pending = [...jobs]
  const running = new Set()

  return await new Promise((resolve) => {
    const launchMore = () => {
      while (running.size < maxConcurrentJobs) {
        const next = pending.find((job) => job.status === "pending")
        if (!next) break
        next.status = "running"
        next.attempt += 1
        next.phase = phase
        next.started_at = new Date().toISOString()
        next.ended_at = null
        const child = spawnJob(next, phase)
        next.pid = child.pid
        running.add(next)
        child.on("exit", (code, signal) => {
          running.delete(next)
          next.exit_code = code
          next.signal = signal
          next.ended_at = new Date().toISOString()
          refreshJobSummary(next)
          if (phase === "agent" && expectedResults(next) < agents.length && next.attempt < maxAttempts) {
            next.status = "pending"
            next.requeued = true
          } else {
            next.status = "done"
          }
          writeProgress(true)
          launchMore()
          if (running.size === 0 && !pending.some((job) => job.status === "pending")) resolve()
        })
      }
      writeProgress()
    }

    monitorTimer = setInterval(() => {
      for (const job of jobs) refreshJobSummary(job)
      writeProgress()
    }, monitorMs)
    launchMore()
  }).finally(() => {
    if (monitorTimer) clearInterval(monitorTimer)
    monitorTimer = null
  })
}

function makeSourcePortJob(task, repeat) {
  const jobRunId = `${runId}-source-port-${task}-r${repeat}`
  const paths = sourcePortPaths(jobRunId)
  const id = `source-port-${task}-r${repeat}`
  const logDir = path.join(matrixRoot, "logs", id)
  return {
    id,
    kind: "source-port",
    task,
    repeat,
    runner: path.join(repoRoot, "benchmark", "tasks", "refactoring", "source-port-python", "runner.mjs"),
    run_id: jobRunId,
    run_root: paths.run_root,
    summary_path: paths.summary_path,
    contracts_path: path.join(paths.run_root, "contracts", "agent-rounds.jsonl"),
    log_dir: logDir,
    stdout: path.join(logDir, `${id}.agent.stdout.log`),
    stderr: path.join(logDir, `${id}.agent.stderr.log`),
    status: "pending",
    attempt: 0,
  }
}

function makePromptGalleryJob(repeat) {
  const jobRunId = `${runId}-prompt-gallery-fullstack-r${repeat}`
  const paths = businessRunPaths("project-rebuild-makeup-tanstack-fullstack", jobRunId)
  const id = `prompt-gallery-fullstack-r${repeat}`
  const logDir = path.join(matrixRoot, "logs", id)
  return {
    id,
    kind: "prompt-gallery",
    task: "prompt-gallery-tanstack-fullstack-rebuild",
    repeat,
    runner: path.join(repoRoot, "benchmark", "tasks", "refactoring", "prompt-gallery-tanstack-fullstack-rebuild", "runner.mjs"),
    run_id: jobRunId,
    run_root: paths.run_root,
    summary_path: paths.summary_path,
    contracts_path: path.join(paths.run_root, "contracts", "agent-rounds.jsonl"),
    log_dir: logDir,
    stdout: path.join(logDir, `${id}.agent.stdout.log`),
    stderr: path.join(logDir, `${id}.agent.stderr.log`),
    status: "pending",
    attempt: 0,
  }
}

function spawnJob(job, phase) {
  mkdirp(job.log_dir)
  const out = fs.openSync(job.stdout, "a")
  const err = fs.openSync(job.stderr, "a")
  const env = {
    ...process.env,
    COMMAND_RUN_AGENT_RUN_ID: job.run_id,
    COMMAND_RUN_AGENT_AGENTS: agents.join(","),
    COMMAND_RUN_AGENT_CODEX_MODEL: model,
    COMMAND_RUN_AGENT_TURA_MODEL: turaModel,
    COMMAND_RUN_AGENT_REASONING_EFFORT: reasoning,
    COMMAND_RUN_AGENT_SERVICE_TIER: serviceTier,
    COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    COMMAND_RUN_AGENT_ALLOW_FAILURE: "1",
    COMMAND_RUN_AGENT_SKIP_TURA_BUILD: "1",
  }
  if (job.kind === "source-port") {
    env.SOURCE_PORT_TASKS = job.task
    env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS = job.task
    env.SOURCE_PORT_RUN_EVAL = phase === "harness" ? "1" : "0"
    env.COMMAND_RUN_AGENT_SOURCE_PORT_RUN_EVAL = phase === "harness" ? "1" : "0"
    env.COMMAND_RUN_AGENT_EVALUATE_ONLY = phase === "harness" ? "1" : "0"
  } else {
    env.COMMAND_RUN_MAKEUP_TANSTACK_VERSION = "fullstack"
    env.COMMAND_RUN_AGENT_SKIP_EVAL = phase === "agent" ? "1" : "0"
    env.COMMAND_RUN_AGENT_EVALUATE_ONLY = phase === "harness" ? "1" : "0"
  }
  return spawn(nodeExe, [job.runner], {
    cwd: repoRoot,
    env,
    stdio: ["ignore", out, err],
    windowsHide: true,
  })
}

function refreshJobSummary(job) {
  const summary = readJson(job.summary_path)
  const rounds = readRounds(job.contracts_path)
  job.summary_exists = Boolean(summary)
  job.rounds_exists = fs.existsSync(job.contracts_path)
  job.round_count = rounds.length
  job.expected_groups = agents.length
  job.result_groups = Array.isArray(summary?.results) ? summary.results.length : 0
  job.ok = summary?.ok ?? null
  job.in_progress = summary?.in_progress ?? null
  job.aggregate_usage = summary?.aggregate_usage || aggregateUsage(summary?.results || [])
  job.harness = summarizeHarness(summary)
  job.agent_status = summarizeAgents(summary, rounds, job)
}

function summarizeAgents(summary, rounds, job) {
  const results = Array.isArray(summary?.results) ? summary.results : []
  return agents.map((agent) => {
    const result = results.find((item) => String(item.agent || item.id) === agent)
    const resultRounds = rounds.filter((round) => String(round.agentId || round.metadata?.agentId || round.agent || round.provider || "").includes(agent))
    const usage = usageFromResult(result)
    const events = result?.events || {}
    const validation = result?.validation || result?.eval || null
    const providerLog = providerLogFor(job, agent)
    return {
      agent,
      present: Boolean(result),
      run_status: result?.run?.status || result?.status || (result?.error ? "error" : null),
      usage_source: result?.usage_source || result?.context_archive?.usage_source || null,
      token_total: Number(usage.total || 0),
      token_reasoning: Number(usage.reasoning || 0),
      turns: Number(events.turns || resultRounds.length || 0),
      commands: Number(events.commands || 0),
      failures: Number(events.failures || 0),
      provider_call_count: providerCallCount(result),
      provider_log_exists: providerLog ? fs.existsSync(providerLog) : null,
      rounds_have_messages: resultRounds.length ? resultRounds.every((round) => Array.isArray(round.messages) && round.messages.length > 0) : null,
      rounds_have_usage: resultRounds.length ? resultRounds.every((round) => Boolean(round.usage || round.messageUsage || round.tokenUsage)) : null,
      rounds_have_tool_calls: resultRounds.length ? resultRounds.every((round) => Array.isArray(round.toolCalls) || Array.isArray(round.tool_calls) || Array.isArray(round.commands)) : null,
      harness: summarizeOneHarness(validation),
    }
  })
}

function summarizeHarness(summary) {
  const results = Array.isArray(summary?.results) ? summary.results : []
  const rows = results.map((result) => summarizeOneHarness(result.validation || result.eval))
  return {
    agents_with_harness: rows.filter((row) => row.ran).length,
    passed: rows.reduce((sum, row) => sum + row.passed, 0),
    total: rows.reduce((sum, row) => sum + row.total, 0),
    runtime_ok: rows.length ? rows.every((row) => row.runtime_ok !== false) : null,
  }
}

function summarizeOneHarness(validation) {
  if (!validation) return { ran: false, passed: 0, total: 0, runtime_ok: null, error: null }
  if (validation.report?.reports) {
    const reports = validation.report.reports
    const passed = reports.reduce((sum, item) => sum + Number(item.passed || 0), 0)
    const failed = reports.reduce((sum, item) => sum + Number(item.failed || 0), 0)
    return { ran: Boolean(validation.ran), passed, total: passed + failed, runtime_ok: Number(validation.exit_code) === 0, error: validation.error || null }
  }
  const scores = validation.scores || validation.score_breakdown || {}
  const total = Number(scores.total || validation.total || 0)
  const passed = Number(scores.passed || validation.passed || validation.comparison?.passed || 0)
  const runtime = validation.runtime || validation.browser || {}
  return {
    ran: true,
    passed,
    total,
    runtime_ok: validation.runtime_ok ?? runtime.ok ?? null,
    error: validation.runtime_error || validation.error || runtime.error || null,
  }
}

function aggregateUsage(results) {
  const total = { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 }
  for (const result of results || []) {
    const usage = usageFromResult(result)
    total.input += Number(usage.input || 0)
    total.cached += Number(usage.cached || 0)
    total.output += Number(usage.output || 0)
    total.reasoning += Number(usage.reasoning || 0)
    total.total += Number(usage.total || 0)
  }
  return total
}

function usageFromResult(result) {
  if (!result) return {}
  const raw = result.usage || result.aggregate_usage || result.tokens || {}
  const input = Number(raw.input ?? raw.input_tokens ?? 0)
  const cached = Number(raw.cached ?? raw.cached_input_tokens ?? 0)
  const output = Number(raw.output ?? raw.output_tokens ?? 0)
  const reasoning = Number(raw.reasoning ?? raw.reasoning_tokens ?? 0)
  const total = Number(raw.total ?? raw.total_tokens ?? 0) || input + output
  return { ...raw, input, cached, output, reasoning, total }
}

function providerCallCount(result) {
  if (!result) return 0
  if (Array.isArray(result.provider_calls)) return result.provider_calls.length
  if (Number.isFinite(Number(result.provider_calls))) return Number(result.provider_calls)
  if (Number.isFinite(Number(result.context_archive?.provider_call_count))) return Number(result.context_archive.provider_call_count)
  if (Array.isArray(result.context_archive?.provider_calls)) return result.context_archive.provider_calls.length
  if (Number.isFinite(Number(result.context_archive?.provider_calls))) return Number(result.context_archive.provider_calls)
  return 0
}

function providerLogFor(job, agent) {
  if (!agent.startsWith("tura-")) return null
  if (job.kind === "source-port") {
    return path.join(job.run_root, job.task, `${agent}-${agents.indexOf(agent) + 1}`, "context-and-calls", "provider-calls-full.jsonl")
  }
  return path.join(job.run_root, agent, "context-and-calls", "provider-calls-full.jsonl")
}

function expectedResults(job) {
  refreshJobSummary(job)
  return Number(job.result_groups || 0)
}

function readRounds(file) {
  if (!fs.existsSync(file)) return []
  return fs.readFileSync(file, "utf8").split(/\r?\n/).filter(Boolean).map((line) => {
    try {
      return JSON.parse(line)
    } catch {
      return {}
    }
  })
}

function writeProgress(forcePdf = false) {
  state.updated_at = new Date().toISOString()
  for (const job of jobs) refreshJobSummary(job)
  const finished = jobs.filter((job) => job.status === "done").length
  state.totals = {
    jobs_done: finished,
    jobs_total: jobs.length,
    result_groups: jobs.reduce((sum, job) => sum + Number(job.result_groups || 0), 0),
    expected_groups: jobs.length * agents.length,
    tokens: jobs.reduce((sum, job) => sum + Number(job.aggregate_usage?.total || 0), 0),
    reasoning_tokens: jobs.reduce((sum, job) => sum + Number(job.aggregate_usage?.reasoning || 0), 0),
    commands: jobs.reduce((sum, job) => sum + (job.agent_status || []).reduce((inner, agent) => inner + Number(agent.commands || 0), 0), 0),
  }
  writeJson(progressJson, state)
  fs.writeFileSync(progressMd, renderMarkdown(), "utf8")
  fs.writeFileSync(progressHtml, renderHtml(), "utf8")
  const now = Date.now()
  if (forcePdf || now - lastPdfAttempt > 60_000) {
    lastPdfAttempt = now
    tryPrintPdf()
  }
}

function renderMarkdown() {
  const lines = [
    `# ${runId}`,
    "",
    `- phase: ${state.phase}`,
    `- updated: ${state.updated_at}`,
    `- progress: ${state.totals?.jobs_done || 0}/${state.totals?.jobs_total || jobs.length} jobs`,
    `- groups: ${state.totals?.result_groups || 0}/${state.totals?.expected_groups || jobs.length * agents.length}`,
    `- tokens: ${state.totals?.tokens || 0}, reasoning: ${state.totals?.reasoning_tokens || 0}, commands: ${state.totals?.commands || 0}`,
    `- pdf: ${state.pdf?.ok ? state.pdf.path : state.pdf?.error || "pending"}`,
    "",
    "| job | status | groups | ok | tokens | commands | harness |",
    "| --- | --- | ---: | --- | ---: | ---: | --- |",
  ]
  for (const job of jobs) {
    const commands = (job.agent_status || []).reduce((sum, agent) => sum + Number(agent.commands || 0), 0)
    lines.push(`| ${job.id} | ${job.status}${job.pid ? ` pid ${job.pid}` : ""} | ${job.result_groups || 0}/${agents.length} | ${job.ok} | ${job.aggregate_usage?.total || 0} | ${commands} | ${job.harness?.passed || 0}/${job.harness?.total || 0} |`)
  }
  lines.push("")
  lines.push("## Agents")
  for (const job of jobs) {
    lines.push("")
    lines.push(`### ${job.id}`)
    for (const row of job.agent_status || []) {
      lines.push(`- ${row.agent}: present=${row.present}, status=${row.run_status}, usage_source=${row.usage_source}, tokens=${row.token_total}, reasoning=${row.token_reasoning}, turns=${row.turns}, commands=${row.commands}, failures=${row.failures}, provider_calls=${row.provider_call_count}, provider_log=${row.provider_log_exists}, rounds(messages/usage/toolCalls)=${row.rounds_have_messages}/${row.rounds_have_usage}/${row.rounds_have_tool_calls}, harness=${row.harness.passed}/${row.harness.total}, runtime=${row.harness.runtime_ok}${row.harness.error ? `, error=${String(row.harness.error).replace(/\r?\n/g, " ").slice(0, 140)}` : ""}`)
    }
  }
  return `${lines.join("\n")}\n`
}

function renderHtml() {
  const escaped = escapeHtml(renderMarkdown())
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>${escapeHtml(runId)}</title>
<style>
body { font-family: Arial, sans-serif; margin: 32px; color: #1f2937; }
pre { white-space: pre-wrap; font-size: 12px; line-height: 1.45; }
@page { margin: 18mm; }
</style>
</head>
<body><pre>${escaped}</pre></body>
</html>`
}

function tryPrintPdf() {
  const browser = findBrowser()
  if (!browser) {
    state.pdf = { path: progressPdf, ok: false, error: "no headless Edge/Chrome found", updated_at: new Date().toISOString() }
    return
  }
  const result = spawnSync(browser, [
    "--headless",
    "--disable-gpu",
    `--print-to-pdf=${progressPdf}`,
    pathToFileURL(progressHtml).href,
  ], { cwd: matrixRoot, encoding: "utf8", timeout: 60_000, windowsHide: true })
  state.pdf = {
    path: progressPdf,
    ok: result.status === 0 && fs.existsSync(progressPdf),
    error: result.status === 0 ? null : String(result.stderr || result.error || `exit ${result.status}`).slice(0, 500),
    updated_at: new Date().toISOString(),
  }
  writeJson(progressJson, state)
}

function findBrowser() {
  const candidates = [
    process.env.CHROME_PATH,
    process.env.EDGE_PATH,
    path.join(process.env["ProgramFiles(x86)"] || "C:\\Program Files (x86)", "Microsoft", "Edge", "Application", "msedge.exe"),
    path.join(process.env.ProgramFiles || "C:\\Program Files", "Microsoft", "Edge", "Application", "msedge.exe"),
    path.join(process.env.ProgramFiles || "C:\\Program Files", "Google", "Chrome", "Application", "chrome.exe"),
  ].filter(Boolean)
  return candidates.find((candidate) => fs.existsSync(candidate)) || null
}

function sourcePortPaths(runIdValue) {
  const baseRunPaths = businessRunPaths("project-rebuild-source-port", runIdValue)
  return businessRunPaths("project-rebuild-source-port", runIdValue, {
    targetRoot: baseRunPaths.target_root,
    runRoot: path.join(baseRunPaths.target_root, baseRunPaths.test_name, shortRunDirName(runIdValue)),
  })
}

function shortRunDirName(value) {
  return `r-${crypto.createHash("sha1").update(String(value)).digest("hex").slice(0, 10)}`
}

function writePhase(phase) {
  fs.writeFileSync(phaseFile, `${phase}\n`, "utf8")
}

function readJson(file) {
  if (!fs.existsSync(file)) return null
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"))
  } catch {
    return null
  }
}

function writeJson(file, value) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, JSON.stringify(value, null, 2), "utf8")
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function parseArgs(raw) {
  const parsed = {}
  for (let i = 0; i < raw.length; i += 1) {
    const item = raw[i]
    if (!item.startsWith("--")) continue
    const key = item.slice(2)
    const next = raw[i + 1]
    if (!next || next.startsWith("--")) {
      parsed[key] = "1"
    } else {
      parsed[key] = next
      i += 1
    }
  }
  return parsed
}

function timestamp() {
  const date = new Date()
  const pad = (value) => String(value).padStart(2, "0")
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`
}

function userHome() {
  return process.env.USERPROFILE || process.env.HOME || os.homedir()
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
}

main().catch((error) => {
  state.phase = "error"
  state.error = String(error?.stack || error?.message || error)
  state.ended_at = new Date().toISOString()
  writeProgress(true)
  console.error(error)
  process.exitCode = 1
})
