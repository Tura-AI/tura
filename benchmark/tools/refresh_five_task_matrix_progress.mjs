#!/usr/bin/env node
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { spawnSync } from "node:child_process"
import { pathToFileURL } from "node:url"

const root = path.resolve(process.argv[2] || "")
if (!root) throw new Error("usage: node benchmark/tools/refresh_five_task_matrix_progress.mjs <matrix-root>")

const progressPath = path.join(root, "progress.json")
const correctedJson = path.join(root, "progress-corrected.json")
const correctedMd = path.join(root, "progress-corrected.md")
const correctedHtml = path.join(root, "progress-corrected.html")
const correctedPdf = path.join(root, "progress-corrected.pdf")
const progress = readJson(progressPath)
if (!progress) throw new Error(`missing progress.json at ${progressPath}`)

const agents = progress.config?.agents || ["codex-main", "tura-balanced", "tura-direct"]
const rows = []
for (const job of progress.jobs || []) {
  const summary = readJson(job.summary_path)
  const rounds = readRounds(job.contracts_path)
  const results = Array.isArray(summary?.results) ? summary.results : []
  const agentRows = agents.map((agent) => {
    const result = results.find((item) => String(item.agent || item.id) === agent)
    const usage = normalizeUsage(result)
    const events = result?.events || {}
    const agentRounds = rounds.filter((round) => String(round.agentId || round.metadata?.agentId || "") === agent)
    const harness = summarizeHarness(result?.validation || result?.eval)
    return {
      agent,
      present: Boolean(result),
      usage_source: result?.usage_source || null,
      tokens: usage.total,
      reasoning: usage.reasoning,
      turns: Number(events.turn_completed ?? events.turns ?? agentRounds.length ?? 0),
      commands: Number(events.command_executions ?? events.commands ?? 0),
      failures: Number(events.commands_failed ?? events.failures ?? 0),
      provider_calls: providerCallCount(result),
      rounds: agentRounds.length,
      rounds_have_messages: agentRounds.length ? agentRounds.every((round) => Array.isArray(round.messages)) : null,
      rounds_have_usage: agentRounds.length ? agentRounds.every((round) => Boolean(round.usage)) : null,
      rounds_have_tool_calls: agentRounds.length ? agentRounds.every((round) => Array.isArray(round.toolCalls)) : null,
      harness,
    }
  })
  rows.push({
    id: job.id,
    kind: job.kind,
    task: job.task,
    status: job.status,
    pid: job.pid,
    ok: summary?.ok ?? job.ok ?? null,
    result_groups: results.length,
    expected_groups: agents.length,
    rounds: rounds.length,
    tokens: agentRows.reduce((sum, row) => sum + row.tokens, 0),
    reasoning: agentRows.reduce((sum, row) => sum + row.reasoning, 0),
    commands: agentRows.reduce((sum, row) => sum + row.commands, 0),
    failures: agentRows.reduce((sum, row) => sum + row.failures, 0),
    harness_passed: agentRows.reduce((sum, row) => sum + row.harness.passed, 0),
    harness_total: agentRows.reduce((sum, row) => sum + row.harness.total, 0),
    agents: agentRows,
  })
}

const corrected = {
  ...progress,
  corrected_at: new Date().toISOString(),
  corrected_progress_json: correctedJson,
  corrected_progress_pdf: correctedPdf,
  corrected_totals: {
    jobs_done: rows.filter((row) => row.status === "done").length,
    jobs_total: rows.length,
    result_groups: rows.reduce((sum, row) => sum + row.result_groups, 0),
    expected_groups: rows.length * agents.length,
    tokens: rows.reduce((sum, row) => sum + row.tokens, 0),
    reasoning: rows.reduce((sum, row) => sum + row.reasoning, 0),
    commands: rows.reduce((sum, row) => sum + row.commands, 0),
    failures: rows.reduce((sum, row) => sum + row.failures, 0),
    harness_passed: rows.reduce((sum, row) => sum + row.harness_passed, 0),
    harness_total: rows.reduce((sum, row) => sum + row.harness_total, 0),
  },
  corrected_jobs: rows,
}

writeJson(correctedJson, corrected)
fs.writeFileSync(correctedMd, renderMarkdown(corrected), "utf8")
fs.writeFileSync(correctedHtml, renderHtml(corrected), "utf8")
const pdf = printPdf(correctedHtml, correctedPdf)
corrected.corrected_pdf = pdf
writeJson(correctedJson, corrected)

printConsole(corrected)

function normalizeUsage(result) {
  const raw = result?.usage || result?.aggregate_usage || result?.tokens || {}
  const input = Number(raw.input ?? raw.input_tokens ?? 0)
  const output = Number(raw.output ?? raw.output_tokens ?? 0)
  const reasoning = Number(raw.reasoning ?? raw.reasoning_tokens ?? 0)
  const total = Number(raw.total ?? raw.total_tokens ?? 0) || input + output
  return { input, output, reasoning, total }
}

function providerCallCount(result) {
  if (!result) return 0
  if (Array.isArray(result.provider_calls)) return result.provider_calls.length
  if (Number.isFinite(Number(result.provider_calls))) return Number(result.provider_calls)
  if (Number.isFinite(Number(result.context_archive?.provider_call_count))) return Number(result.context_archive.provider_call_count)
  return 0
}

function summarizeHarness(validation) {
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
  return {
    ran: true,
    passed: Number(scores.passed || validation.passed || 0),
    total: Number(scores.total || validation.total || 0),
    runtime_ok: validation.runtime_ok ?? validation.runtime?.ok ?? null,
    error: validation.runtime_error || validation.error || validation.runtime?.error || null,
  }
}

function renderMarkdown(report) {
  const lines = [
    `# ${report.run_id}`,
    "",
    `- phase: ${report.phase}`,
    `- corrected_at: ${report.corrected_at}`,
    `- jobs: ${report.corrected_totals.jobs_done}/${report.corrected_totals.jobs_total}`,
    `- groups: ${report.corrected_totals.result_groups}/${report.corrected_totals.expected_groups}`,
    `- tokens: ${report.corrected_totals.tokens}`,
    `- reasoning: ${report.corrected_totals.reasoning}`,
    `- commands: ${report.corrected_totals.commands}`,
    `- harness: ${report.corrected_totals.harness_passed}/${report.corrected_totals.harness_total}`,
    "",
    "| job | status | groups | rounds | tokens | reasoning | commands | failures | harness |",
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
  ]
  for (const job of report.corrected_jobs) {
    lines.push(`| ${job.id} | ${job.status}${job.pid ? ` pid ${job.pid}` : ""} | ${job.result_groups}/${job.expected_groups} | ${job.rounds} | ${job.tokens} | ${job.reasoning} | ${job.commands} | ${job.failures} | ${job.harness_passed}/${job.harness_total} |`)
  }
  lines.push("", "## Agents")
  for (const job of report.corrected_jobs) {
    if (!job.result_groups && job.status === "pending") continue
    lines.push("", `### ${job.id}`)
    for (const agent of job.agents) {
      lines.push(`- ${agent.agent}: present=${agent.present}, source=${agent.usage_source}, tokens=${agent.tokens}, reasoning=${agent.reasoning}, turns=${agent.turns}, commands=${agent.commands}, failures=${agent.failures}, provider_calls=${agent.provider_calls}, rounds=${agent.rounds}, rounds(messages/usage/toolCalls)=${agent.rounds_have_messages}/${agent.rounds_have_usage}/${agent.rounds_have_tool_calls}, harness=${agent.harness.passed}/${agent.harness.total}, runtime=${agent.harness.runtime_ok}`)
    }
  }
  return `${lines.join("\n")}\n`
}

function renderHtml(report) {
  return `<!doctype html><html><head><meta charset="utf-8"><title>${escapeHtml(report.run_id)}</title><style>body{font-family:Arial,sans-serif;margin:32px;color:#1f2937}pre{white-space:pre-wrap;font-size:12px;line-height:1.45}@page{margin:18mm}</style></head><body><pre>${escapeHtml(renderMarkdown(report))}</pre></body></html>`
}

function printConsole(report) {
  const t = report.corrected_totals
  console.log(`phase=${report.phase} jobs=${t.jobs_done}/${t.jobs_total} groups=${t.result_groups}/${t.expected_groups} tokens=${t.tokens} reasoning=${t.reasoning} commands=${t.commands} harness=${t.harness_passed}/${t.harness_total}`)
  for (const job of report.corrected_jobs.filter((row) => row.status !== "pending" || row.result_groups > 0)) {
    console.log(`JOB ${job.id} status=${job.status} groups=${job.result_groups}/${job.expected_groups} ok=${job.ok} rounds=${job.rounds} tokens=${job.tokens} reasoning=${job.reasoning} commands=${job.commands} harness=${job.harness_passed}/${job.harness_total}`)
    for (const agent of job.agents) {
      console.log(`  AGENT ${agent.agent} present=${agent.present} source=${agent.usage_source} tokens=${agent.tokens} reasoning=${agent.reasoning} turns=${agent.turns} commands=${agent.commands} failures=${agent.failures} provider_calls=${agent.provider_calls} rounds=${agent.rounds} msg=${agent.rounds_have_messages} usage=${agent.rounds_have_usage} tools=${agent.rounds_have_tool_calls} harness=${agent.harness.passed}/${agent.harness.total} runtime=${agent.harness.runtime_ok}`)
    }
  }
  console.log(`corrected_pdf=${report.corrected_pdf?.ok ? correctedPdf : report.corrected_pdf?.error}`)
}

function printPdf(html, pdf) {
  const browser = findBrowser()
  if (!browser) return { ok: false, error: "no headless Edge/Chrome found" }
  const result = spawnSync(browser, ["--headless", "--disable-gpu", `--print-to-pdf=${pdf}`, pathToFileURL(html).href], {
    encoding: "utf8",
    timeout: 60_000,
    windowsHide: true,
  })
  return { ok: result.status === 0 && fs.existsSync(pdf), error: result.status === 0 ? null : String(result.stderr || result.error || `exit ${result.status}`).slice(0, 500) }
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

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"))
  } catch {
    return null
  }
}

function readRounds(file) {
  try {
    return fs.readFileSync(file, "utf8").trim().split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line))
  } catch {
    return []
  }
}

function writeJson(file, value) {
  fs.writeFileSync(file, JSON.stringify(value, null, 2), "utf8")
}

function escapeHtml(value) {
  return String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;")
}
