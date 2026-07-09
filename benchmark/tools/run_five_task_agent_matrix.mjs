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
const args = parseArgs(process.argv.slice(2))
const matrixName = args.name || "matrix-5tasks-gpt55-codex-medium-tura-high-default-agent"
const runId = args["run-id"] || `matrix5-agent-gpt55-codex-medium-tura-high-default-r6-${timestamp()}`
const matrixRoot = path.resolve(args.root || path.join(targetRoot, matrixName, runId))
const maxConcurrent = Number(args.concurrency || process.env.MATRIX_AGENT_CONCURRENCY || 6)
const maxAttempts = Number(args.attempts || process.env.MATRIX_AGENT_ATTEMPTS || 3)
const repeats = Number(args.repeats || 6)
const agents = (args.agents || "codex-main,tura-balanced,tura-direct").split(",").map((item) => item.trim()).filter(Boolean)
const model = args.model || "gpt-5.5"
const turaModel = args["tura-model"] || `openai/${model}`
const codexReasoning = args["codex-reasoning"] || "medium"
const turaReasoning = args["tura-reasoning"] || "high"
const turaEmbedded = truthy(args["tura-embedded"] || process.env.MATRIX_TURA_EMBEDDED || "0")
const serviceTier = args["service-tier"] || "default"
const timeoutMs = Number(args["timeout-ms"] || args.timeoutMs || process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 90 * 60_000)
const monitorMs = Number(args["monitor-ms"] || args.monitorMs || process.env.MATRIX_MONITOR_MS || 60_000)
const sourcePortTasks = ["zip-password-finder", "nushell", "eza", "xsv"]
const progressJson = path.join(matrixRoot, "progress.json")
const progressMd = path.join(matrixRoot, "progress.md")
const progressHtml = path.join(matrixRoot, "progress.html")
const progressPdf = path.join(matrixRoot, "progress.pdf")
const phaseFile = path.join(matrixRoot, "phase.txt")
const nodeExe = process.execPath

mkdirp(matrixRoot)

const units = []
for (let repeat = 1; repeat <= repeats; repeat += 1) {
  for (const task of sourcePortTasks) {
    for (const agent of agents) units.push(makeUnit("source-port", task, repeat, agent))
  }
  for (const agent of agents) units.push(makeUnit("prompt-gallery", "prompt-gallery-tanstack-fullstack-rebuild", repeat, agent))
}

const state = {
  run_id: runId,
  matrix_root: matrixRoot,
  started_at: new Date().toISOString(),
  updated_at: null,
  phase: "agent",
  config: {
    max_concurrent_agents: maxConcurrent,
    repeats,
    agents,
    model,
    tura_model: turaModel,
    codex_reasoning: codexReasoning,
    tura_reasoning: turaReasoning,
    tura_embedded: turaEmbedded,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
    max_attempts: maxAttempts,
  },
  units,
  pdf: { path: progressPdf, ok: false, error: null, updated_at: null },
}

let monitorTimer = null
let lastPdfAttempt = 0

async function main() {
  writePhase("agent")
  writeProgress(true)
  await runPhase("agent")
  state.phase = "harness"
  writePhase("harness")
  for (const unit of units) {
    unit.status = "pending"
    unit.phase = "harness"
    unit.pid = null
    unit.exit_code = null
    unit.signal = null
    unit.phase_attempt = 0
    unit.started_at = null
    unit.ended_at = null
  }
  writeProgress(true)
  await runPhase("harness")
  state.phase = "complete"
  state.ended_at = new Date().toISOString()
  writePhase("complete")
  writeProgress(true)
}

async function runPhase(phase) {
  const running = new Set()
  for (const unit of units) {
    unit.status = "pending"
    unit.phase = phase
    unit.phase_attempt = 0
    unit.pid = null
    unit.started_at = null
    unit.ended_at = null
    unit.exit_code = null
    unit.signal = null
  }

  return await new Promise((resolve) => {
    const launchMore = () => {
      while (running.size < maxConcurrent) {
        const next = units.find((unit) => unit.status === "pending" && canLaunchUnit(unit, running))
        if (!next) break
        next.status = "running"
        next.phase_attempt += 1
        if (phase === "agent") {
          next.attempt_total = Number(next.attempt_total || 0) + 1
          next.run_id = `${runId}-${unitSlug(next)}-a${next.attempt_total}`
        } else if (!next.run_id) {
          throw new Error(`missing agent run_id before harness phase for ${next.id}`)
        }
        updateUnitPaths(next)
        next.started_at = new Date().toISOString()
        next.ended_at = null
        const child = spawnUnit(next, phase)
        next.pid = child.pid
        running.add(next)
        child.on("exit", (code, signal) => {
          running.delete(next)
          next.exit_code = code
          next.signal = signal
          next.ended_at = new Date().toISOString()
          refreshUnit(next)
          const complete = phase === "agent" ? agentRecordComplete(next) : harnessRecordComplete(next)
          if (!complete && next.phase_attempt < maxAttempts) {
            next.status = "pending"
            next.requeued = true
            next.requeue_reason = phase === "agent" ? agentRecordMissingReason(next) : harnessRecordMissingReason(next)
          } else {
            next.status = "done"
            next.ok = complete
          }
          writeProgress(true)
          launchMore()
          if (running.size === 0 && !units.some((unit) => unit.status === "pending")) resolve()
        })
      }
      writeProgress()
    }

    monitorTimer = setInterval(() => {
      for (const unit of units) refreshUnit(unit)
      writeProgress()
    }, monitorMs)
    launchMore()
  }).finally(() => {
    if (monitorTimer) clearInterval(monitorTimer)
    monitorTimer = null
  })
}

function canLaunchUnit(candidate, running) {
  if (candidate.kind !== "source-port") return true
  for (const unit of running) {
    if (unit.kind === "source-port" && unit.task === candidate.task) return false
  }
  return true
}

function makeUnit(kind, task, repeat, agent) {
  const id = `${kind === "source-port" ? `source-port-${task}` : "prompt-gallery-fullstack"}-r${repeat}-${agent}`
  return {
    id,
    kind,
    task,
    repeat,
    agent,
    expected_groups: 1,
    status: "pending",
    phase: "agent",
    phase_attempt: 0,
    attempt_total: 0,
    runner: kind === "source-port"
      ? path.join(repoRoot, "benchmark", "tasks", "refactoring", "source-port-python", "runner.mjs")
      : path.join(repoRoot, "benchmark", "tasks", "refactoring", "prompt-gallery-tanstack-fullstack-rebuild", "runner.mjs"),
  }
}

function updateUnitPaths(unit) {
  const logDir = path.join(matrixRoot, "logs", unit.id)
  unit.log_dir = logDir
  if (unit.kind === "source-port") {
    const paths = sourcePortPaths(unit.run_id)
    unit.run_root = paths.run_root
    unit.summary_path = paths.summary_path
    unit.contracts_path = path.join(paths.run_root, "contracts", "agent-rounds.jsonl")
  } else {
    const paths = businessRunPaths("project-rebuild-makeup-tanstack-fullstack", unit.run_id)
    unit.run_root = paths.run_root
    unit.summary_path = paths.summary_path
    unit.contracts_path = path.join(paths.run_root, "contracts", "agent-rounds.jsonl")
  }
  unit.stdout = path.join(logDir, `${unit.id}.${unit.phase}.a${unit.phase_attempt}.stdout.log`)
  unit.stderr = path.join(logDir, `${unit.id}.${unit.phase}.a${unit.phase_attempt}.stderr.log`)
}

function spawnUnit(unit, phase) {
  mkdirp(unit.log_dir)
  const out = fs.openSync(unit.stdout, "a")
  const err = fs.openSync(unit.stderr, "a")
  const env = {
    ...process.env,
    COMMAND_RUN_AGENT_RUN_ID: unit.run_id,
    COMMAND_RUN_AGENT_AGENTS: unit.agent,
    COMMAND_RUN_AGENT_CODEX_MODEL: model,
    COMMAND_RUN_AGENT_TURA_MODEL: turaModel,
    COMMAND_RUN_AGENT_REASONING_EFFORT: reasoningForAgent(unit.agent),
    COMMAND_RUN_AGENT_SERVICE_TIER: serviceTier,
    COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    COMMAND_RUN_AGENT_ALLOW_FAILURE: "1",
    COMMAND_RUN_AGENT_SKIP_TURA_BUILD: "1",
    ...(turaEmbedded ? { COMMAND_RUN_AGENT_TURA_EMBEDDED: "1" } : {}),
  }
  if (unit.kind === "source-port") {
    env.SOURCE_PORT_TASKS = unit.task
    env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS = unit.task
    env.SOURCE_PORT_RUN_EVAL = phase === "harness" ? "1" : "0"
    env.COMMAND_RUN_AGENT_SOURCE_PORT_RUN_EVAL = phase === "harness" ? "1" : "0"
    env.COMMAND_RUN_AGENT_EVALUATE_ONLY = phase === "harness" ? "1" : "0"
    delete env.COMMAND_RUN_MAKEUP_TANSTACK_VERSION
    delete env.COMMAND_RUN_AGENT_SKIP_EVAL
  } else {
    env.COMMAND_RUN_MAKEUP_TANSTACK_VERSION = "fullstack"
    env.COMMAND_RUN_AGENT_SKIP_EVAL = phase === "agent" ? "1" : "0"
    env.COMMAND_RUN_AGENT_EVALUATE_ONLY = phase === "harness" ? "1" : "0"
    delete env.SOURCE_PORT_TASKS
    delete env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS
    delete env.SOURCE_PORT_RUN_EVAL
    delete env.COMMAND_RUN_AGENT_SOURCE_PORT_RUN_EVAL
  }
  return spawn(nodeExe, [unit.runner], {
    cwd: repoRoot,
    env,
    stdio: ["ignore", out, err],
    windowsHide: true,
  })
}

function refreshUnit(unit) {
  if (!unit.summary_path) return
  const summary = readJson(unit.summary_path)
  const rounds = readRounds(unit.contracts_path)
  const result = findResult(summary, unit.agent)
  const agentRounds = filterRounds(rounds, unit.agent)
  const usage = usageFromResult(result)
  const events = result?.events || {}
  const harness = summarizeOneHarness(result?.validation || result?.eval)
  unit.summary_exists = Boolean(summary)
  unit.rounds_exists = fs.existsSync(unit.contracts_path || "")
  unit.summary_ok = summary?.ok ?? null
  unit.in_progress = Boolean(summary?.in_progress || result?.in_progress)
  unit.result_present = Boolean(result)
  unit.result_groups = result ? 1 : 0
  unit.usage_source = result?.usage_source || result?.context_archive?.usage_source || null
  unit.token_total = Number(usage.total || 0)
  unit.token_reasoning = Number(usage.reasoning || 0)
  unit.turns = Number(events.turn_completed ?? events.turns ?? agentRounds.length ?? 0)
  unit.commands = Number(events.command_executions ?? events.commands ?? 0)
  unit.failures = Number(events.commands_failed ?? events.failures ?? 0)
  unit.provider_call_count = providerCallCount(result)
  unit.provider_log_exists = unit.agent.startsWith("tura-") ? Boolean(providerLogFor(unit, result)) : null
  unit.round_count = agentRounds.length
  unit.rounds_have_messages = agentRounds.length ? agentRounds.every((round) => roundHasMessages(round)) : null
  unit.rounds_have_usage = agentRounds.length ? agentRounds.every((round) => Boolean(round.usage || round.messageUsage || round.tokenUsage)) : null
  unit.rounds_have_tool_calls = agentRounds.length ? agentRounds.every((round) => Array.isArray(round.toolCalls) || Array.isArray(round.tool_calls) || Array.isArray(round.commands)) : null
  unit.harness = harness
}

function findResult(summary, agent) {
  const results = Array.isArray(summary?.results) ? summary.results : []
  return results.find((item) => String(item.agent || item.id) === agent) || (results.length === 1 ? results[0] : null)
}

function filterRounds(rounds, agent) {
  if (!rounds.length) return []
  const exact = rounds.filter((round) => String(round.agentId || round.metadata?.agentId || round.agent || "") === agent)
  if (exact.length) return exact
  const fuzzy = rounds.filter((round) => String(round.agentId || round.metadata?.agentId || round.agent || round.provider || "").includes(agent))
  return fuzzy.length ? fuzzy : rounds
}

function maybeJsonArray(value) {
  if (Array.isArray(value)) return value
  if (typeof value !== "string" || !value.trim().startsWith("[")) return null
  try {
    const parsed = JSON.parse(value)
    return Array.isArray(parsed) ? parsed : null
  } catch {
    return null
  }
}

function roundHasMessages(round) {
  if (Array.isArray(round.messages) && round.messages.length > 0) return true
  if (Array.isArray(round.input?.messages) && round.input.messages.length > 0) return true
  if (Array.isArray(round.output?.messages) && round.output.messages.length > 0) return true
  if (typeof round.output?.assistantMessage === "string" && round.output.assistantMessage.trim()) return true
  if (typeof round.output?.fullOutput === "string" && round.output.fullOutput.trim()) return true
  const fullContext = maybeJsonArray(round.input?.fullContext)
  return Boolean(fullContext?.length)
}

function agentRecordComplete(unit) {
  const logOk = !unit.agent.startsWith("tura-") || unit.provider_log_exists === true
  return Boolean(
    unit.result_present &&
      !unit.in_progress &&
      unit.usage_source &&
      unit.round_count > 0 &&
      unit.rounds_have_messages === true &&
      unit.rounds_have_usage === true &&
      unit.rounds_have_tool_calls === true &&
      Number.isFinite(unit.commands) &&
      logOk,
  )
}

function harnessRecordComplete(unit) {
  return agentRecordComplete(unit) && unit.harness?.ran === true && Number(unit.harness?.total || 0) > 0
}

function agentRecordMissingReason(unit) {
  const reasons = []
  if (!unit.result_present) reasons.push("missing result")
  if (unit.in_progress) reasons.push("summary still in_progress")
  if (!unit.usage_source) reasons.push("missing usage_source")
  if (!unit.round_count) reasons.push("missing rounds")
  if (unit.rounds_have_messages !== true) reasons.push("round messages incomplete")
  if (unit.rounds_have_usage !== true) reasons.push("round usage incomplete")
  if (unit.rounds_have_tool_calls !== true) reasons.push("round toolCalls incomplete")
  if (unit.agent.startsWith("tura-") && unit.provider_log_exists !== true) reasons.push("missing tura provider log")
  return reasons.join("; ") || "unknown"
}

function harnessRecordMissingReason(unit) {
  const base = agentRecordMissingReason(unit)
  const reasons = base === "unknown" ? [] : [base]
  if (unit.harness?.ran !== true) reasons.push("harness did not run")
  if (!Number(unit.harness?.total || 0)) reasons.push("harness total is zero")
  return reasons.join("; ") || "unknown"
}

function summarizeOneHarness(validation) {
  if (!validation) return { ran: false, passed: 0, total: 0, runtime_ok: null, error: null }
  if (validation.report?.reports) {
    const reports = validation.report.reports
    const passed = reports.reduce((sum, item) => sum + Number(item.passed || 0), 0)
    const failed = reports.reduce((sum, item) => sum + Number(item.failed || 0), 0)
    return { ran: Boolean(validation.ran), passed, total: passed + failed, runtime_ok: Number(validation.exit_code) === 0, error: validation.error || null }
  }
  if (Number.isFinite(Number(validation.standards_total))) {
    return {
      ran: true,
      passed: Number(validation.standards_passed || 0),
      total: Number(validation.standards_total || 0),
      runtime_ok: validation.runtime_ok ?? validation.runtime?.pass ?? validation.runtime?.ok ?? null,
      error: validation.runtime_error || validation.error || validation.runtime?.error || null,
    }
  }
  const scores = validation.scores || validation.score_breakdown || {}
  const total = Number(scores.total || validation.total || 0)
  const passed = Number(scores.passed || validation.passed || 0)
  const runtime = validation.runtime || validation.browser || {}
  return {
    ran: true,
    passed,
    total,
    runtime_ok: validation.runtime_ok ?? runtime.ok ?? null,
    error: validation.runtime_error || validation.error || runtime.error || null,
  }
}

function usageFromResult(result) {
  if (!result) return {}
  const raw = result.usage || result.aggregate_usage || result.tokens || {}
  const input = Number(raw.input ?? raw.input_tokens ?? raw.inputTokens ?? raw.prompt_tokens ?? 0)
  const cached = Number(raw.cached ?? raw.cached_input_tokens ?? raw.cacheInputTokens ?? 0)
  const output = Number(raw.output ?? raw.output_tokens ?? raw.outputTokens ?? raw.completion_tokens ?? 0)
  const reasoning = Number(raw.reasoning ?? raw.reasoning_tokens ?? raw.reasoningTokens ?? raw.reasoning_output_tokens ?? raw.output_tokens_details?.reasoning_tokens ?? 0)
  const total = Number(raw.total ?? raw.total_tokens ?? raw.totalTokens ?? 0) || input + output
  return { input, cached, output, reasoning, total }
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

function providerLogFor(unit, result) {
  const candidates = [
    result?.provider_calls_path,
    result?.context_archive?.provider_calls_full_path,
    path.join(unit.run_root || "", unit.agent, "context-and-calls", "provider-calls-full.jsonl"),
    path.join(unit.run_root || "", unit.task || "", `${unit.agent}-1`, "context-and-calls", "provider-calls-full.jsonl"),
  ].filter(Boolean)
  return candidates.find((candidate) => fs.existsSync(candidate)) || null
}

function writeProgress(forcePdf = false) {
  state.updated_at = new Date().toISOString()
  for (const unit of units) refreshUnit(unit)
  const running = units.filter((unit) => unit.status === "running")
  const done = units.filter((unit) => unit.status === "done")
  state.totals = {
    units_done: done.length,
    units_total: units.length,
    complete_records: units.filter((unit) => agentRecordComplete(unit)).length,
    harness_records: units.filter((unit) => harnessRecordComplete(unit)).length,
    expected_records: units.length,
    tokens: units.reduce((sum, unit) => sum + Number(unit.token_total || 0), 0),
    reasoning_tokens: units.reduce((sum, unit) => sum + Number(unit.token_reasoning || 0), 0),
    commands: units.reduce((sum, unit) => sum + Number(unit.commands || 0), 0),
    failures: units.reduce((sum, unit) => sum + Number(unit.failures || 0), 0),
    provider_calls: units.reduce((sum, unit) => sum + Number(unit.provider_call_count || 0), 0),
    harness_passed: units.reduce((sum, unit) => sum + Number(unit.harness?.passed || 0), 0),
    harness_total: units.reduce((sum, unit) => sum + Number(unit.harness?.total || 0), 0),
    running: running.map((unit) => ({ id: unit.id, pid: unit.pid, attempt: unit.phase_attempt, tokens: unit.token_total, commands: unit.commands, records: agentRecordComplete(unit), harness: harnessRecordComplete(unit) })),
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
  const t = state.totals || {}
  const lines = [
    `# ${runId}`,
    "",
    `- phase: ${state.phase}`,
    `- updated: ${state.updated_at}`,
    `- units: ${t.units_done || 0}/${t.units_total || units.length}`,
    `- complete agent records: ${t.complete_records || 0}/${t.expected_records || units.length}`,
    `- harness records: ${t.harness_records || 0}/${t.expected_records || units.length}`,
    `- tokens: ${t.tokens || 0}, reasoning: ${t.reasoning_tokens || 0}, commands: ${t.commands || 0}, failures: ${t.failures || 0}, provider calls: ${t.provider_calls || 0}`,
    `- harness: ${t.harness_passed || 0}/${t.harness_total || 0}`,
    `- pdf: ${state.pdf?.ok ? state.pdf.path : state.pdf?.error || "pending"}`,
    "",
    "## Running",
    ...(t.running || []).map((unit) => `- ${unit.id}: pid=${unit.pid}, attempt=${unit.attempt}, tokens=${unit.tokens}, commands=${unit.commands}, complete=${unit.records}, harness=${unit.harness}`),
    "",
    "## By Task",
    "",
    "| task | done | records | harness records | tokens | commands | harness |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  ]
  for (const row of groupedBy((unit) => unit.task)) {
    lines.push(`| ${row.key} | ${row.done}/${row.total} | ${row.records}/${row.total} | ${row.harnessRecords}/${row.total} | ${row.tokens} | ${row.commands} | ${row.harnessPassed}/${row.harnessTotal} |`)
  }
  lines.push("", "## By Agent", "", "| agent | done | records | harness records | tokens | reasoning | commands | provider calls | harness |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
  for (const row of groupedBy((unit) => unit.agent)) {
    lines.push(`| ${row.key} | ${row.done}/${row.total} | ${row.records}/${row.total} | ${row.harnessRecords}/${row.total} | ${row.tokens} | ${row.reasoning} | ${row.commands} | ${row.providerCalls} | ${row.harnessPassed}/${row.harnessTotal} |`)
  }
  lines.push("", "## Units", "", "| unit | phase/status | attempts | record | rounds | tokens | commands | provider log | harness | reason |", "| --- | --- | ---: | --- | ---: | ---: | ---: | --- | ---: | --- |")
  for (const unit of units) {
    lines.push(`| ${unit.id} | ${unit.phase}/${unit.status}${unit.pid ? ` pid ${unit.pid}` : ""} | ${unit.phase_attempt}/${unit.attempt_total || 0} | ${agentRecordComplete(unit)} | ${unit.round_count || 0} | ${unit.token_total || 0} | ${unit.commands || 0} | ${unit.provider_log_exists} | ${unit.harness?.passed || 0}/${unit.harness?.total || 0} | ${unit.requeue_reason || ""} |`)
  }
  return `${lines.join("\n")}\n`
}

function groupedBy(keyFn) {
  const map = new Map()
  for (const unit of units) {
    const key = keyFn(unit)
    if (!map.has(key)) map.set(key, [])
    map.get(key).push(unit)
  }
  return [...map.entries()].sort(([a], [b]) => String(a).localeCompare(String(b))).map(([key, group]) => ({
    key,
    total: group.length,
    done: group.filter((unit) => unit.status === "done").length,
    records: group.filter((unit) => agentRecordComplete(unit)).length,
    harnessRecords: group.filter((unit) => harnessRecordComplete(unit)).length,
    tokens: group.reduce((sum, unit) => sum + Number(unit.token_total || 0), 0),
    reasoning: group.reduce((sum, unit) => sum + Number(unit.token_reasoning || 0), 0),
    commands: group.reduce((sum, unit) => sum + Number(unit.commands || 0), 0),
    providerCalls: group.reduce((sum, unit) => sum + Number(unit.provider_call_count || 0), 0),
    harnessPassed: group.reduce((sum, unit) => sum + Number(unit.harness?.passed || 0), 0),
    harnessTotal: group.reduce((sum, unit) => sum + Number(unit.harness?.total || 0), 0),
  }))
}

function renderHtml() {
  return `<!doctype html><html><head><meta charset="utf-8"><title>${escapeHtml(runId)}</title><style>body{font-family:Arial,sans-serif;margin:32px;color:#1f2937}pre{white-space:pre-wrap;font-size:12px;line-height:1.45}@page{margin:18mm}</style></head><body><pre>${escapeHtml(renderMarkdown())}</pre></body></html>`
}

function tryPrintPdf() {
  const browser = findBrowser()
  if (!browser) {
    state.pdf = { path: progressPdf, ok: false, error: "no headless Edge/Chrome found", updated_at: new Date().toISOString() }
    return
  }
  const result = spawnSync(browser, ["--headless", "--disable-gpu", `--print-to-pdf=${progressPdf}`, pathToFileURL(progressHtml).href], {
    cwd: matrixRoot,
    encoding: "utf8",
    timeout: 60_000,
    windowsHide: true,
  })
  state.pdf = {
    path: progressPdf,
    ok: result.status === 0 && fs.existsSync(progressPdf),
    error: result.status === 0 ? null : String(result.stderr || result.error || `exit ${result.status}`).slice(0, 500),
    updated_at: new Date().toISOString(),
  }
  writeJson(progressJson, state)
}

function reasoningForAgent(agent) {
  return agent.startsWith("tura-") ? turaReasoning : codexReasoning
}

function unitSlug(unit) {
  return `${unit.kind === "source-port" ? `source-port-${unit.task}` : "prompt-gallery-fullstack"}-r${unit.repeat}-${unit.agent}`.replace(/[^a-zA-Z0-9_.-]+/g, "-")
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

function readRounds(file) {
  if (!file || !fs.existsSync(file)) return []
  return fs.readFileSync(file, "utf8").split(/\r?\n/).filter(Boolean).map((line) => {
    try {
      return JSON.parse(line)
    } catch {
      return {}
    }
  })
}

function readJson(file) {
  if (!file || !fs.existsSync(file)) return null
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

function writePhase(phase) {
  fs.writeFileSync(phaseFile, `${phase}\n`, "utf8")
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
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

function parseArgs(raw) {
  const parsed = {}
  for (let i = 0; i < raw.length; i += 1) {
    const item = raw[i]
    if (!item.startsWith("--")) continue
    const key = item.slice(2)
    const next = raw[i + 1]
    if (!next || next.startsWith("--")) parsed[key] = "1"
    else {
      parsed[key] = next
      i += 1
    }
  }
  return parsed
}

function truthy(value) {
  return ["1", "true", "yes", "on"].includes(String(value || "").toLowerCase())
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
  return String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;")
}

main().catch((error) => {
  state.phase = "error"
  state.error = String(error?.stack || error?.message || error)
  state.ended_at = new Date().toISOString()
  writeProgress(true)
  console.error(error)
  process.exitCode = 1
})
