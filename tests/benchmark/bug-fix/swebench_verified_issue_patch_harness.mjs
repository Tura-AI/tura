#!/usr/bin/env node
import assert from "node:assert/strict"
import os from "node:os"
import { spawn, spawnSync } from "node:child_process"
import crypto from "node:crypto"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { agentEventStats, agentUsageFromJsonl, claudeCodeArgs, findClaudeExe, findPiExe, piAgentArgs } from "../lib/agent_cli.mjs"
import { businessRunPaths, defaultUserWorkspace, normalizeBusinessSummary } from "../lib/business_paths.mjs"
import { endStream, isolatedProcessOptions, killProcessTree } from "../lib/process_helpers.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const benchmarkRoot = process.env.COMMAND_RUN_AGENT_BENCHMARK_ROOT || path.join(defaultUserWorkspace(), "benchmarks")
const swebenchRoot = process.env.COMMAND_RUN_AGENT_SWEBENCH_ROOT || path.join(benchmarkRoot, "SWE-bench")
const swebenchDataset = process.env.COMMAND_RUN_AGENT_SWEBENCH_DATASET || "SWE-bench/SWE-bench_Verified"
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `agent-swebench-test-${Date.now()}`
const runPaths = businessRunPaths("bug-fix-swebench", runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path

const VERIFIED_INSTANCE_PREFIX_BY_REPO = Object.freeze({
  "astropy/astropy": "astropy__astropy",
  "django/django": "django__django",
  "matplotlib/matplotlib": "matplotlib__matplotlib",
  "mwaskom/seaborn": "mwaskom__seaborn",
  "pallets/flask": "pallets__flask",
  "psf/requests": "psf__requests",
  "pydata/xarray": "pydata__xarray",
  "pylint-dev/pylint": "pylint-dev__pylint",
  "pytest-dev/pytest": "pytest-dev__pytest",
  "scikit-learn/scikit-learn": "scikit-learn__scikit-learn",
  "sphinx-doc/sphinx": "sphinx-doc__sphinx",
  "sympy/sympy": "sympy__sympy",
})

const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 60 * 60_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "tura-fast-shll,current-shll,codex-main")
const agentRuns = buildAgentRuns(agents)
const workspacePrepConcurrency = Number(process.env.COMMAND_RUN_AGENT_WORKSPACE_PREP_CONCURRENCY || (process.platform === "win32" ? 1 : Math.min(4, Math.max(1, os.cpus().length - 1 || 1))))
const harnessOnly = (process.env.COMMAND_RUN_AGENT_HARNESS_ONLY || "0") === "1"
const requestedInstances = process.env.COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS || ""
const requestedInstanceIds = parseInstanceSpecs(requestedInstances)
if (!harnessOnly) assert(requestedInstanceIds.length > 0, "COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS is required")
const requestedRepos = process.env.COMMAND_RUN_AGENT_SWEBENCH_REPOS || process.env.COMMAND_RUN_AGENT_SWEBENCH_REPO
  ? parseRepoSpecs(process.env.COMMAND_RUN_AGENT_SWEBENCH_REPOS || process.env.COMMAND_RUN_AGENT_SWEBENCH_REPO)
  : requestedInstanceIds.length > 0 ? inferReposFromInstanceIds(requestedInstanceIds) : []
const prepOnly = (process.env.COMMAND_RUN_AGENT_PREP_ONLY || "0") === "1"
const runHarness = (process.env.COMMAND_RUN_AGENT_RUN_HARNESS || "0") === "1"
const harnessMaxWorkers = Number(process.env.COMMAND_RUN_AGENT_HARNESS_MAX_WORKERS || Math.max(1, Math.min(8, os.cpus().length - 1 || 1)))
const harnessCacheLevel = process.env.COMMAND_RUN_AGENT_HARNESS_CACHE_LEVEL || "instance"
const harnessClean = process.env.COMMAND_RUN_AGENT_HARNESS_CLEAN || "false"
const harnessBackend = process.env.COMMAND_RUN_AGENT_HARNESS_BACKEND || (process.platform === "win32" ? "docker-linux" : "native")
const harnessImage = process.env.COMMAND_RUN_AGENT_HARNESS_IMAGE || "tura-swebench-harness:latest"

const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const gatewayExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway")
const claudeExe = findClaudeExe()
const piExe = findPiExe()
const codexCurrentExe = path.join(
  process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex"),
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
)
const codexMainExe = findCodexMainExe()

function findCodexMainExe() {
  const exeName = process.platform === "win32" ? "codex.exe" : "codex"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_CODEX_MAIN_ROOT,
    path.join(homeDir, "Documents", "codex-main"),
    path.join(homeDir, "codex-main"),
    path.join(homeDir, "RustroverProjects", "codex-main"),
  ]
    .filter(Boolean)
    .map((root) => path.join(root, "codex-rs", "target", "debug", exeName))
  return candidates.find((candidate) => fs.existsSync(candidate)) || candidates[0]
}

function parseAgents(value) {
  const alias = new Map([
    ["current", "current-shll"],
    ["current-shll", "current-shll"],
    ["codex-current", "current-shll"],
    ["main", "codex-main"],
    ["codex-main", "codex-main"],
    ["tura", "tura-fast-shll"],
    ["tura-shll", "tura-shll"],
    ["tura-coding", "tura-shll"],
    ["tura-coding-agent", "tura-shll"],
    ["tura-fast", "tura-fast-shll"],
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
    .map((item) => alias.get(item.trim().toLowerCase()))
    .filter(Boolean)
}

function buildAgentRuns(agentIds) {
  const counts = new Map()
  return agentIds.map((agentId) => {
    const count = (counts.get(agentId) || 0) + 1
    counts.set(agentId, count)
    return {
      agent_id: agentId,
      run_id: count === 1 ? agentId : `${agentId}-${count}`,
    }
  })
}

async function mapWithConcurrency(items, concurrency, fn) {
  const results = new Array(items.length)
  let nextIndex = 0
  const workerCount = Math.max(1, Math.min(concurrency, items.length || 1))
  await Promise.all(Array.from({ length: workerCount }, async () => {
    for (;;) {
      const index = nextIndex
      nextIndex += 1
      if (index >= items.length) return
      results[index] = await fn(items[index], index)
    }
  }))
  return results
}

function parseRepoSpecs(value) {
  const raw = String(value || "").trim()
  let items
  if (raw.startsWith("[")) {
    const parsed = JSON.parse(raw)
    assert(Array.isArray(parsed), "COMMAND_RUN_AGENT_SWEBENCH_REPOS JSON must be an array")
    items = parsed
  } else {
    items = raw.split(",")
  }
  const repos = items
    .map((item) => normalizeRepoName(String(item).trim()))
    .filter(Boolean)
  assert(repos.length > 0, "at least one SWE-bench repo is required")
  return [...new Set(repos)]
}

function parseInstanceSpecs(value) {
  const raw = String(value || "").trim()
  if (!raw) return []
  if (raw.startsWith("[")) {
    const parsed = JSON.parse(raw)
    assert(Array.isArray(parsed), "COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS JSON must be an array")
    return parsed.map((item) => String(item).trim()).filter(Boolean)
  }
  return raw.split(",").map((item) => item.trim()).filter(Boolean)
}

function inferReposFromInstanceIds(instanceIds) {
  const repos = []
  for (const id of instanceIds) {
    const repo = repoForInstanceId(id)
    if (repo && !repos.includes(repo)) repos.push(repo)
  }
  assert(repos.length > 0, `could not infer any SWE-bench repo from issue ids: ${instanceIds.join(", ")}`)
  return repos
}

function repoForInstanceId(instanceId) {
  for (const [repo, prefix] of Object.entries(VERIFIED_INSTANCE_PREFIX_BY_REPO)) {
    if (instanceId.startsWith(`${prefix}-`)) return repo
  }
  return null
}

function normalizeRepoName(value) {
  if (!value) return ""
  return value.includes("/") ? value : value.replace("__", "/")
}

function repoSlug(repo) {
  return normalizeRepoName(repo).replace("/", "__")
}

function sourceRepoFor(repo) {
  const normalized = normalizeRepoName(repo)
  const slug = repoSlug(normalized)
  const specific = process.env[`COMMAND_RUN_AGENT_REPO_${slug.toUpperCase().replaceAll("-", "_")}_PATH`]
  if (specific) return specific
  if (normalized === "pydata/xarray" && process.env.COMMAND_RUN_AGENT_XARRAY_REPO) return process.env.COMMAND_RUN_AGENT_XARRAY_REPO
  return path.join(benchmarkRoot, "repos", slug)
}

function selectedIssueTable(rows) {
  const table = Object.fromEntries(requestedRepos.map((repo) => [repo, []]))
  for (const row of rows) {
    const repo = normalizeRepoName(row.repo)
    if (table[repo]) table[repo].push(row.instance_id)
  }
  for (const ids of Object.values(table)) ids.sort((a, b) => a.localeCompare(b))
  return table
}

function issueBelongsToSelectedRepo(instanceId) {
  for (const repo of requestedRepos) {
    const prefix = VERIFIED_INSTANCE_PREFIX_BY_REPO[repo]
    if (prefix && instanceId.startsWith(`${prefix}-`)) return true
  }
  return false
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text)
}

function run(command, args, options = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: options.maxBuffer || 512 * 1024 * 1024,
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
    duration_ms: Math.round(performance.now() - started),
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

function assertInside(parent, child) {
  const relative = path.relative(path.resolve(parent), path.resolve(child))
  assert(relative && !relative.startsWith("..") && !path.isAbsolute(relative), `${child} is not inside ${parent}`)
}

function runLive(command, args, options = {}) {
  const started = performance.now()
  const stdoutPath = options.stdoutPath
  const stderrPath = options.stderrPath
  const statusPath = options.statusPath
  if (stdoutPath) mkdirp(path.dirname(stdoutPath))
  if (stderrPath) mkdirp(path.dirname(stderrPath))
  const stdoutStream = stdoutPath ? fs.createWriteStream(stdoutPath, { flags: "w" }) : null
  const stderrStream = stderrPath ? fs.createWriteStream(stderrPath, { flags: "w" }) : null
  let stdout = ""
  let stderr = ""
  let firstOutputMs = null
  let timedOut = false
  let settled = false
  let childExitStatus = null
  let childExitSignal = null

  function writeStatus(extra) {
    if (!statusPath) return
    writeFile(statusPath, JSON.stringify({
      command,
      args,
      cwd: options.cwd || repoRoot,
      elapsed_ms: Math.round(performance.now() - started),
      ...extra,
    }, null, 2))
  }

  writeStatus({ status: "running" })
  return new Promise((resolve) => {
    const timeoutLimitMs = options.timeoutMs || timeoutMs
    let closeGraceTimer = null
    let timeoutGraceTimer = null

    function settle(statusLabel, status, signal, error) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      clearTimeout(closeGraceTimer)
      clearTimeout(timeoutGraceTimer)
      endStream(stdoutStream)
      endStream(stderrStream)
      const result = {
        command,
        args,
        status,
        signal,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        first_output_ms: firstOutputMs,
        error,
      }
      writeStatus({ status: statusLabel, result })
      resolve(result)
    }

    const child = spawn(command, args, isolatedProcessOptions({
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    }))

    const timer = setTimeout(() => {
      timedOut = true
      try {
        killProcessTree(child.pid)
      } catch {}
      timeoutGraceTimer = setTimeout(() => {
        settle("timeout", childExitStatus ?? 1, childExitSignal, `timed out after ${timeoutLimitMs}ms`)
      }, Number(options.timeoutCloseGraceMs || 3_000))
    }, timeoutLimitMs)

    child.stdout?.on("data", (chunk) => {
      if (firstOutputMs === null) firstOutputMs = Math.round(performance.now() - started)
      const text = chunk.toString("utf8")
      stdout += text
      stdoutStream?.write(text)
      writeStatus({ status: "running", stdout_bytes: Buffer.byteLength(stdout), stderr_bytes: Buffer.byteLength(stderr) })
    })
    child.stderr?.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stderr += text
      stderrStream?.write(text)
      writeStatus({ status: "running", stdout_bytes: Buffer.byteLength(stdout), stderr_bytes: Buffer.byteLength(stderr) })
    })
    child.on("error", (error) => {
      settle("error", null, null, String(error.stack || error.message || error))
    })
    child.on("exit", (status, signal) => {
      childExitStatus = status
      childExitSignal = signal
      closeGraceTimer = setTimeout(() => {
        settle(
          timedOut ? "timeout" : "closed",
          timedOut ? (status ?? 1) : status,
          signal,
          timedOut ? `timed out after ${timeoutLimitMs}ms` : null,
        )
      }, Number(options.exitCloseGraceMs || 1_000))
    })
    child.on("close", (status, signal) => {
      settle(
        timedOut ? "timeout" : "closed",
        timedOut ? (status ?? 1) : status,
        signal,
        timedOut ? `timed out after ${timeoutLimitMs}ms` : null,
      )
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

function usageFromJsonl(stdout) {
  const totals = { usage_events: 0, input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, cached_input_tokens: 0, total_tokens: 0 }
  for (const event of parseJsonl(stdout)) {
    const usage = event.type === "turn.completed"
      ? event.usage
      : event.type === "event_msg" && event.payload?.type === "token_count"
        ? event.payload?.info?.last_token_usage
        : event.type === "runtime_usage"
          ? event.usage
          : null
    if (!usage) continue
    addUsage(totals, usage)
  }
  return totals
}

function usageFromTuraSessions(workspace) {
  const totals = { usage_events: 0, input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, cached_input_tokens: 0, total_tokens: 0 }
  const result = run(gatewayExe, ["session-log"], {
    input: JSON.stringify({
      command: "list_sessions",
      workspace,
      page: 0,
      page_size: 500,
    }),
  })
  if (result.status !== 0) return totals
  let response
  try {
    response = JSON.parse(result.stdout)
  } catch {
    return totals
  }
  for (const snapshot of response?.sessions || []) {
    collectRuntimeUsage(snapshot?.management, totals)
    collectRuntimeUsage(snapshot?.session, totals)
  }
  return totals
}

function collectRuntimeUsage(value, totals, depth = 0) {
  if (depth > 8 || value === null || value === undefined) return
  if (typeof value === "string") {
    const parsed = tryParseJson(value)
    if (parsed) collectRuntimeUsage(parsed, totals, depth + 1)
    return
  }
  if (Array.isArray(value)) {
    for (const item of value) collectRuntimeUsage(item, totals, depth + 1)
    return
  }
  if (typeof value !== "object") return
  if (value.type === "runtime_usage" && value.usage) addUsage(totals, value.usage)
  for (const child of Object.values(value)) collectRuntimeUsage(child, totals, depth + 1)
}

function listFiles(rootDir) {
  const files = []
  const stack = [rootDir]
  while (stack.length > 0) {
    const current = stack.pop()
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(fullPath)
      else files.push(fullPath)
    }
  }
  return files
}

function tryParseJson(text) {
  try {
    return JSON.parse(text)
  } catch {
    return null
  }
}

function addUsage(totals, usage) {
  totals.usage_events += 1
  totals.input_tokens += Number(usage.input_tokens || usage.prompt_tokens || 0)
  totals.output_tokens += Number(usage.output_tokens || usage.completion_tokens || 0)
  totals.reasoning_tokens += Number(usage.reasoning_tokens || usage.output_tokens_details?.reasoning_tokens || usage.completion_tokens_details?.reasoning_tokens || 0)
  totals.cached_input_tokens += Number(usage.cached_input_tokens || usage.input_tokens_details?.cached_tokens || usage.prompt_tokens_details?.cached_tokens || 0)
  totals.total_tokens += Number(usage.total_tokens || usage.input_tokens + usage.output_tokens || usage.prompt_tokens + usage.completion_tokens || 0)
}

function chooseUsage(stdout, workspace) {
  const stdoutUsage = usageFromJsonl(stdout)
  const sessionUsage = usageFromTuraSessions(workspace)
  if (sessionUsage.usage_events > stdoutUsage.usage_events) return { ...sessionUsage, source: "tura_sessions" }
  return { ...stdoutUsage, source: stdoutUsage.usage_events > 0 ? "stdout_jsonl" : "none" }
}

function eventStats(stdout) {
  const events = parseJsonl(stdout)
  return {
    events: events.length,
    agent_messages: events.filter((event) => event.item?.type === "agent_message").length,
    commands_started: events.filter((event) => event.type === "item.started" && event.item?.type === "command_execution").length,
    commands_completed: events.filter((event) => event.type === "item.completed" && event.item?.type === "command_execution").length,
    commands_failed: events.filter((event) => event.type === "item.completed" && event.item?.type === "command_execution" && event.item?.status === "failed").length,
    file_changes: events.filter((event) => event.item?.type === "file_change").length,
    file_changes_failed: events.filter((event) => event.item?.type === "file_change" && event.item?.status === "failed").length,
  }
}

function performanceStats(result, usage, patch) {
  const elapsedSeconds = result.duration_ms > 0 ? result.duration_ms / 1000 : null
  const outputTps = elapsedSeconds ? Number((Number(usage.output_tokens || 0) / elapsedSeconds).toFixed(2)) : null
  const totalTps = elapsedSeconds ? Number((Number(usage.total_tokens || usage.input_tokens + usage.output_tokens || 0) / elapsedSeconds).toFixed(2)) : null
  return {
    elapsed_ms: result.duration_ms,
    elapsed_s: elapsedSeconds === null ? null : Number(elapsedSeconds.toFixed(3)),
    first_output_ms: result.first_output_ms,
    usage_events: usage.usage_events,
    input_tokens: usage.input_tokens,
    output_tokens: usage.output_tokens,
    reasoning_tokens: usage.reasoning_tokens,
    cached_input_tokens: usage.cached_input_tokens,
    total_tokens: usage.total_tokens,
    output_tps_total_wall: outputTps,
    total_tps_total_wall: totalTps,
    patch_bytes: patch.patch_bytes,
    changed_files: patch.changed_files,
  }
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

async function fetchVerifiedInstances() {
  const cachePath = path.join(runRoot, "verified-instances.json")
  mkdirp(path.dirname(cachePath))
  const localCandidates = [
    path.join(benchmarkRoot, "verified-instances.json"),
    path.join(benchmarkRoot, "verified-xarray-instances.json"),
    path.join(benchmarkRoot, "verified_repo_difficulty_stats.json"),
  ]
  for (const candidate of localCandidates) {
    if (!fs.existsSync(candidate)) continue
    try {
      const data = JSON.parse(fs.readFileSync(candidate, "utf8"))
      const rows = Array.isArray(data)
        ? data
        : Array.isArray(data.instance_stats)
          ? data.instance_stats
          : []
      if (rows.length > 0 && rows.every((row) => row.instance_id && row.repo && row.base_commit && row.problem_statement)) {
        const selectedRows = rows.filter((row) => requestedRepos.includes(normalizeRepoName(row.repo)))
        const coveredRepos = new Set(selectedRows.map((row) => normalizeRepoName(row.repo)))
        const coversAllRequestedRepos = requestedRepos.every((repo) => coveredRepos.has(repo))
        if (selectedRows.length > 0 && coversAllRequestedRepos) {
          writeFile(cachePath, JSON.stringify(selectedRows.map(redactedInstance), null, 2))
          return selectedRows
        }
      }
    } catch {
      // Ignore malformed local caches and try Hugging Face.
    }
  }
  const rows = []
  for (let offset = 0; ; offset += 100) {
    const url = new URL("https://datasets-server.huggingface.co/rows")
    url.searchParams.set("dataset", "SWE-bench/SWE-bench_Verified")
    url.searchParams.set("config", "default")
    url.searchParams.set("split", "test")
    url.searchParams.set("offset", String(offset))
    url.searchParams.set("length", "100")
    const response = await fetchWithRetry(url)
    const data = await response.json()
    const got = data.rows || []
    rows.push(...got.map((item) => item.row).filter((row) => requestedRepos.includes(normalizeRepoName(row.repo))))
    if (got.length < 100) break
  }
  writeFile(cachePath, JSON.stringify(rows.map(redactedInstance), null, 2))
  return rows
}

async function fetchWithRetry(url, tries = 5) {
  let lastError = null
  for (let attempt = 1; attempt <= tries; attempt += 1) {
    try {
      const response = await fetch(url, { headers: { "user-agent": "tura-agent-swebench-test" } })
      if (response.ok) return response
      lastError = new Error(`HTTP ${response.status}`)
    } catch (error) {
      lastError = error
    }
    await new Promise((resolve) => setTimeout(resolve, 1000 * attempt * 2))
  }
  throw new Error(`failed to fetch ${url}: ${lastError?.message || lastError}`)
}

function selectInstances(allInstances) {
  const byId = new Map(allInstances.map((item) => [item.instance_id, item]))
  const dropped = []
  const selected = []
  for (const id of requestedInstanceIds) {
    if (!issueBelongsToSelectedRepo(id)) {
      dropped.push({ instance_id: id, reason: "not in selected repos" })
      continue
    }
    const found = byId.get(id)
    if (!found) {
      dropped.push({ instance_id: id, reason: "not found in selected Verified rows" })
      continue
    }
    selected.push(found)
  }
  if (selected.length === 0) {
    throw new Error(`no runnable issues remain after filtering; dropped ${JSON.stringify(dropped)}`)
  }
  return { instances: selected, dropped_instance_ids: dropped }
}

function buildTasks(instances) {
  return instances.map((instance) => ({
    task_id: instance.instance_id,
    repo: normalizeRepoName(instance.repo),
    mode: "single_issue",
    instances: [instance],
    harness_compatible: true,
  }))
}

function prepareWorkspace(task, agentRun) {
  const agentDir = path.join(runRoot, task.task_id, agentRun.run_id)
  const workspace = path.join(agentDir, "workspace")
  const sourceRepo = sourceRepoFor(task.repo)
  mkdirp(agentDir)
  assert(fs.existsSync(path.join(sourceRepo, ".git")), `missing source repo for ${task.repo}: ${sourceRepo}`)
  if (fs.existsSync(workspace)) fs.rmSync(workspace, { recursive: true, force: true })
  runOk("git", ["clone", "--no-hardlinks", sourceRepo, workspace], { timeoutMs: 20 * 60_000 })
  if (task.mode === "single_issue") {
    runOk("git", ["checkout", "--force", task.instances[0].base_commit], { cwd: workspace, timeoutMs: 10 * 60_000 })
  }
  runOk("git", ["clean", "-fdx"], { cwd: workspace, timeoutMs: 10 * 60_000 })
  isolateWorkspaceGitHistory(workspace, task)
  writeFile(path.join(agentDir, "task.json"), JSON.stringify({
    ...redactedTask(task),
    agent: agentRun.agent_id,
    agent_run: agentRun.run_id,
  }, null, 2))
  writeFile(path.join(agentDir, "prompt.md"), taskPrompt(task))
  return { agentDir, workspace }
}

function isolateWorkspaceGitHistory(workspace, task) {
  const originalHead = runOk("git", ["rev-parse", "HEAD"], { cwd: workspace, timeoutMs: 30_000 }).stdout.trim()
  const gitDir = path.join(workspace, ".git")
  assertInside(runRoot, gitDir)
  fs.rmSync(gitDir, { recursive: true, force: true })
  runOk("git", ["init"], { cwd: workspace, timeoutMs: 60_000 })
  runOk("git", ["config", "user.email", "agent-swebench-test@example.invalid"], { cwd: workspace, timeoutMs: 30_000 })
  runOk("git", ["config", "user.name", "agent-swebench-test"], { cwd: workspace, timeoutMs: 30_000 })
  runOk("git", ["config", "core.autocrlf", "false"], { cwd: workspace, timeoutMs: 30_000 })
  repairWorkspaceGitPermissions(workspace)
  writeFile(path.join(workspace, ".tura", "swebench-workspace.json"), JSON.stringify({
    instance_ids: task.instances.map((instance) => instance.instance_id),
    original_base_commit: originalHead,
    future_history_hidden: true,
  }, null, 2))
  runGitAddWithRetry(workspace)
  runOk("git", ["commit", "-m", "SWE-bench base snapshot"], { cwd: workspace, timeoutMs: 5 * 60_000 })
}

function runGitAddWithRetry(workspace) {
  const attempts = []
  for (let attempt = 1; attempt <= 4; attempt += 1) {
    repairWorkspaceGitPermissions(workspace)
    const result = run("git", ["-c", "core.fscache=false", "-c", "core.preloadindex=false", "add", "-A"], { cwd: workspace, timeoutMs: 5 * 60_000 })
    attempts.push(result)
    if (result.status === 0) return
    const output = `${result.stderr}\n${result.error || ""}`
    if (!/Permission denied|unable to write file|index\.lock|File exists|resource busy/i.test(output)) {
      throw new Error(`git add -A failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
    }
    repairWorkspaceGitPermissions(workspace)
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 500 * attempt)
  }
  const details = attempts.map((attempt, index) => [
    `ATTEMPT ${index + 1} STATUS ${attempt.status}`,
    `STDOUT:\n${attempt.stdout}`,
    `STDERR:\n${attempt.stderr}`,
    `ERROR:\n${attempt.error || ""}`,
  ].join("\n")).join("\n\n")
  throw new Error(`git add -A failed after ${attempts.length} attempts\n${details}`)
}

function repairWorkspaceGitPermissions(workspace) {
  const gitDir = path.join(workspace, ".git")
  if (!fs.existsSync(gitDir)) return
  try {
    chmodTree(gitDir)
  } catch {
    // Best-effort: Windows ACL repair below is usually the important part.
  }
  if (process.platform !== "win32") return
  run("attrib", ["-R", path.join(gitDir, "*"), "/S", "/D"], { cwd: workspace, timeoutMs: 60_000 })
  const grants = ["*S-1-5-32-545:(OI)(CI)F", "*S-1-1-0:(OI)(CI)F", "Users:(OI)(CI)F", "Everyone:(OI)(CI)F"]
  const whoami = run("whoami", [], { cwd: workspace, timeoutMs: 30_000 })
  if (whoami.status === 0 && whoami.stdout.trim()) {
    grants.unshift(`${whoami.stdout.trim()}:(OI)(CI)F`)
  }
  for (const grant of grants) {
    run("icacls", [gitDir, "/grant", grant, "/T", "/C", "/Q"], { cwd: workspace, timeoutMs: 120_000 })
  }
}

function chmodTree(rootDir) {
  const stack = [rootDir]
  while (stack.length > 0) {
    const current = stack.pop()
    fs.chmodSync(current, 0o777)
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(fullPath)
      else fs.chmodSync(fullPath, 0o666)
    }
  }
}

function redactedInstance(instance) {
  return {
    repo: instance.repo,
    instance_id: instance.instance_id,
    base_commit: instance.base_commit,
    created_at: instance.created_at,
    version: instance.version,
    difficulty: instance.difficulty,
    problem_statement: instance.problem_statement,
  }
}

function redactedTask(task) {
  return {
    task_id: task.task_id,
    repo: task.repo,
    mode: task.mode,
    harness_compatible: task.harness_compatible,
    instances: task.instances.map(redactedInstance),
  }
}

function taskPrompt(task) {
  const instance = task.instances[0]
  return `Fix the bug or issue described below. Treat the report, docs, stack traces, and suggested causes as clues, not proof of the root cause or correct fix. First identify the underlying contract and the smallest stable boundary where the behavior should be guaranteed; be especially suspicious of lazy evaluation, deferred execution, caches, cloning, shared mutable state, partial or repeated execution, and compile/render/serialization steps that may trigger failures later than the reported call site. After any transformation that changes the externally visible shape or meaning of data, aggressively revalidate dependent references, aliases, indexes, caches, and invariants against the final exposed shape instead of reusing assumptions from an earlier internal shape. Validate with focused tests or checks that would fail on the original bug and cover equivalent callers or nearby paths, not only the exact reproduction. Make the minimal necessary production change, avoid unrelated refactors or new abstractions, and do not mask the failure at the call site when the invariant belongs deeper in the system. Do not search the internet.

${instance.problem_statement}
`
}

async function runCurrentLike(agentId, exe, workspace, agentDir, prompt) {
  const args = [
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
  ]
  return runLive(exe, args, {
    cwd: workspace,
    input: prompt,
    timeoutMs,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
  })
}

async function runTura(agentId, workspace, agentDir, prompt, agentPrompt) {
  const sessionId = `agent-swebench-test-${agentId}-${process.pid}-${Date.now()}`
  const internalPrompt = snapshotTuraInternalPrompt(agentDir, agentPrompt)
  const args = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    sessionId,
    "--agent-id",
    agentPrompt,
    "-m",
    turaModel,
    ...turaServiceTierConfigArgs(),
    "--model-reasoning-effort",
    reasoning,
    "--cwd",
    workspace,
  ]
  const result = await runLive(turaExe, args, {
    cwd: workspace,
    input: prompt,
    timeoutMs,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
    env: {
      TURA_PROJECT_ROOT: repoRoot,
      TURA_COMMAND_RUN_SHELL: "shell_command",
      TURA_COMMAND_RUN_STRICT_JSON: "0",
      TURA_SESSION_REASONING_EFFORT: reasoning,
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    },
  })
  return { ...result, tura_internal_prompt: internalPrompt }
}

async function runExternalCliAgent(agentId, workspace, agentDir, prompt) {
  const isClaude = agentId === "claude-code"
  return runLive(isClaude ? claudeExe : piExe, isClaude
    ? claudeCodeArgs(prompt, { model: process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus" })
    : piAgentArgs(prompt), {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
  })
}

function snapshotTuraInternalPrompt(agentDir, agentPrompt) {
  const promptPath = path.join(repoRoot, "crates", "agents", "src", agentPrompt, "prompt.md")
  assert(fs.existsSync(promptPath), `missing Tura internal prompt: ${promptPath}`)
  const content = fs.readFileSync(promptPath, "utf8")
  const snapshotPath = path.join(agentDir, "tura-internal-prompt.md")
  writeFile(snapshotPath, content)
  return {
    agent_prompt: agentPrompt,
    prompt_path: promptPath,
    snapshot_path: snapshotPath,
    sha256: crypto.createHash("sha256").update(content).digest("hex"),
    bytes: Buffer.byteLength(content),
  }
}

function collectPatch(workspace, agentDir) {
  if (!fs.existsSync(path.join(workspace, ".git"))) {
    const patchPath = path.join(agentDir, "model.patch")
    writeFile(patchPath, "")
    writeFile(path.join(agentDir, "git-status.txt"), "")
    writeFile(path.join(agentDir, "git-diff-check.txt"), "workspace git repository was not prepared")
    return {
      patch_path: patchPath,
      patch_bytes: 0,
      changed_files: 0,
      git_status: "",
      diff_check_exit_code: null,
      diff_check_output: "workspace git repository was not prepared",
    }
  }
  const diff = run("git", ["diff", "--binary"], { cwd: workspace, timeoutMs: 120_000 })
  const status = run("git", ["status", "--short"], { cwd: workspace, timeoutMs: 120_000 })
  const patchText = normalizePatchLineEndings(diff.stdout || "")
  const diffCheck = checkNormalizedPatchWhitespace(patchText)
  writeFile(path.join(agentDir, "model.patch"), patchText)
  writeFile(path.join(agentDir, "git-status.txt"), status.stdout)
  writeFile(path.join(agentDir, "git-diff-check.txt"), diffCheck.output)
  return {
    patch_path: path.join(agentDir, "model.patch"),
    patch_bytes: Buffer.byteLength(patchText, "utf8"),
    changed_files: status.stdout.split(/\r?\n/).filter(Boolean).length,
    git_status: status.stdout,
    diff_check_exit_code: diffCheck.exit_code,
    diff_check_output: diffCheck.output.slice(-2000),
  }
}

function normalizePatchLineEndings(patchText) {
  return String(patchText || "").replace(/\r\n/g, "\n").replace(/\r/g, "\n")
}

function normalizePatchForHarness(patchText) {
  return normalizePatchLineEndings(patchText)
    .split("\n")
    .map((line) => line.endsWith("\r") ? line.slice(0, -1) : line)
    .join("\n")
}

function checkNormalizedPatchWhitespace(patchText) {
  const problems = []
  let currentFile = null
  let newLine = 0
  for (const line of String(patchText || "").split("\n")) {
    if (line.startsWith("+++ b/")) {
      currentFile = line.slice("+++ b/".length)
      continue
    }
    if (line.startsWith("@@")) {
      const match = /\+(\d+)(?:,\d+)?/.exec(line)
      newLine = match ? Number(match[1]) : 0
      continue
    }
    if (line.startsWith("+") && !line.startsWith("+++")) {
      const content = line.slice(1)
      if (/[ \t]$/.test(content)) problems.push(`${currentFile || "(unknown)"}:${newLine}: trailing whitespace.`)
      newLine += 1
      continue
    }
    if (line.startsWith("-") && !line.startsWith("---")) continue
    if (newLine > 0) newLine += 1
  }
  return {
    exit_code: problems.length > 0 ? 2 : 0,
    output: problems.length > 0 ? `${problems.join("\n")}\n` : "",
  }
}

function shellQuotePs(value) {
  return `'${String(value).replaceAll("'", "''")}'`
}

function shellQuoteSh(value) {
  return `'${String(value).replaceAll("'", "'\"'\"'")}'`
}

function writePredictionBundles(results) {
  const predictionsRoot = path.join(runRoot, "predictions")
  mkdirp(predictionsRoot)
  const byAgent = new Map()
  for (const item of results) {
    for (const result of item.results) {
      if (!byAgent.has(result.agent)) byAgent.set(result.agent, [])
      const patch = fs.existsSync(result.patch.patch_path) ? normalizePatchForHarness(fs.readFileSync(result.patch.patch_path, "utf8")) : ""
      for (const instanceId of result.prediction_instance_ids) byAgent.get(result.agent).push({
        instance_id: instanceId,
        model_name_or_path: `${result.agent}:${result.agent.startsWith("tura-") ? turaModel : model}`,
        model_patch: patch,
      })
    }
  }

  const bundles = []
  for (const [agent, predictions] of byAgent) {
    const dir = path.join(predictionsRoot, agent)
    mkdirp(dir)
    const predsJsonl = path.join(dir, "all_preds.jsonl")
    writeFile(predsJsonl, predictions.map((prediction) => JSON.stringify(prediction)).join("\n") + "\n")
    writeFile(path.join(dir, "README.md"), [
      `# ${agent} SWE-bench predictions`,
      "",
      `Dataset: ${swebenchDataset}`,
      `Run ID: ${runId}`,
      `Model: ${agent.startsWith("tura-") ? turaModel : model}`,
      `Reasoning effort: ${reasoning}`,
      `Service tier: ${serviceTier}`,
      "",
      "This folder is a prediction bundle. It is not a completed SWE-bench harness evaluation until run_evaluation produces logs and report.json files.",
      "",
    ].join("\n"))
    bundles.push({ agent, predictions: predictions.length, all_preds_jsonl: predsJsonl, dir })
  }
  return bundles
}

function loadPredictionBundlesFromDisk() {
  const predictionsRoot = path.join(runRoot, "predictions")
  if (!fs.existsSync(predictionsRoot)) {
    throw new Error(`missing predictions directory for harness-only run: ${predictionsRoot}`)
  }
  return fs.readdirSync(predictionsRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => {
      const dir = path.join(predictionsRoot, entry.name)
      const allPredsJsonl = path.join(dir, "all_preds.jsonl")
      if (!fs.existsSync(allPredsJsonl)) throw new Error(`missing predictions file: ${allPredsJsonl}`)
      normalizePredictionBundleFile(allPredsJsonl)
      const predictions = fs.readFileSync(allPredsJsonl, "utf8").split(/\r?\n/).filter(Boolean).length
      return { agent: entry.name, predictions, all_preds_jsonl: allPredsJsonl, dir }
    })
}

function normalizePredictionBundleFile(allPredsJsonl) {
  const lines = fs.readFileSync(allPredsJsonl, "utf8").split(/\r?\n/).filter(Boolean)
  let changed = false
  const normalized = lines.map((line) => {
    const prediction = JSON.parse(line)
    if (typeof prediction.model_patch === "string") {
      const patch = normalizePatchForHarness(prediction.model_patch)
      if (patch !== prediction.model_patch) changed = true
      prediction.model_patch = patch
    }
    return JSON.stringify(prediction)
  })
  if (changed) writeFile(allPredsJsonl, normalized.join("\n") + "\n")
}

function comparisonRows(results) {
  return results.flatMap((item) =>
    item.results.map((result) => ({
      task_id: item.task.task_id,
      repo: item.task.repo,
      instance_ids: item.task.instances.map((instance) => instance.instance_id),
      harness_compatible: item.task.harness_compatible,
      agent: result.agent,
      exit_code: result.exit_code,
      elapsed_ms: result.elapsed_ms,
      first_output_ms: result.first_output_ms,
      input_tokens: result.performance.input_tokens,
      output_tokens: result.performance.output_tokens,
      reasoning_tokens: result.performance.reasoning_tokens,
      cached_input_tokens: result.performance.cached_input_tokens,
      total_tokens: result.performance.total_tokens,
      output_tps_total_wall: result.performance.output_tps_total_wall,
      total_tps_total_wall: result.performance.total_tps_total_wall,
      usage_events: result.performance.usage_events,
      events: result.events.events,
      agent_messages: result.events.agent_messages,
      commands_started: result.events.commands_started,
      commands_completed: result.events.commands_completed,
      commands_failed: result.events.commands_failed,
      file_changes: result.events.file_changes,
      file_changes_failed: result.events.file_changes_failed,
      patch_bytes: result.patch.patch_bytes,
      changed_files: result.patch.changed_files,
      has_tracked_patch: result.patch.patch_bytes > 0,
    })),
  )
}

function writeHarnessScripts(predictionBundles, instances, mode = "single_issue") {
  const commands = []
  const dockerCommands = []
  const instanceIds = instances.map((instance) => instance.instance_id)
  const effectiveMaxWorkers = Math.max(1, Math.min(harnessMaxWorkers, new Set(instanceIds).size || 1))
  const instanceArgs = instanceIds.length > 0 ? ` --instance_ids ${instanceIds.map(shellQuotePs).join(" ")}` : ""
  for (const bundle of predictionBundles) {
    const harnessRunId = `${runId}-${bundle.agent}`.replace(/[^A-Za-z0-9_.-]+/g, "-")
    commands.push([
      "python -m swebench.harness.run_evaluation",
      `  --dataset_name ${shellQuotePs(swebenchDataset)}`,
      `  --predictions_path ${shellQuotePs(bundle.all_preds_jsonl)}`,
      `  --max_workers ${effectiveMaxWorkers}`,
      `  --cache_level ${shellQuotePs(harnessCacheLevel)}`,
      `  --clean ${shellQuotePs(harnessClean)}`,
      `  --run_id ${shellQuotePs(harnessRunId)}`,
      instanceArgs ? ` ${instanceArgs.trimStart()}` : "",
    ].filter(Boolean).join(" `\n"))
    const dockerPredictionPath = `/runroot/${path.relative(runRoot, bundle.all_preds_jsonl).replaceAll(path.sep, "/")}`
    const dockerArgs = [
      "python",
      "-m",
      "swebench.harness.run_evaluation",
      "--dataset_name",
      swebenchDataset,
      "--predictions_path",
      dockerPredictionPath,
      "--max_workers",
      String(effectiveMaxWorkers),
      "--cache_level",
      harnessCacheLevel,
      "--clean",
      harnessClean,
      "--run_id",
      harnessRunId,
      "--namespace",
      "swebench",
      ...(instanceIds.length > 0 ? ["--instance_ids", ...instanceIds] : []),
    ]
    dockerCommands.push([
      "docker run --rm",
      "  -v /var/run/docker.sock:/var/run/docker.sock",
      `  -v ${shellQuotePs(`${swebenchRoot}:/swebench`)}`,
      `  -v ${shellQuotePs(`${runRoot}:/runroot`)}`,
      "  -w /swebench",
      `  ${shellQuotePs(harnessImage)}`,
      `  ${dockerArgs.map(shellQuoteSh).join(" ")}`,
    ].join(" `\n"))
  }
  const selectedCommands = harnessBackend === "docker-linux" ? dockerCommands : commands
  const checkedCommands = selectedCommands.flatMap((command, index) => [
    `Write-Host ${shellQuotePs(`[harness] command ${index + 1}/${selectedCommands.length}`)}`,
    command,
    "if ($LASTEXITCODE -ne 0) {",
    "  Write-Host \"SWE-bench harness command failed with exit code $LASTEXITCODE\"",
    "  $global:HarnessExitCode = $LASTEXITCODE",
    "}",
    "",
  ])
  const ps1 = path.join(runRoot, "run_swebench_harness.ps1")
  writeFile(ps1, [
    "$ErrorActionPreference = 'Continue'",
    "$global:HarnessExitCode = 0",
    `Set-Location ${shellQuotePs(swebenchRoot)}`,
    "",
    ...checkedCommands,
    "exit $global:HarnessExitCode",
  ].join("\n"))
  writeFile(path.join(runRoot, "harness-plan.json"), JSON.stringify({
    swebench_root: swebenchRoot,
    dataset: swebenchDataset,
    mode,
    max_workers: effectiveMaxWorkers,
    requested_max_workers: harnessMaxWorkers,
    backend: harnessBackend,
    harness_image: harnessBackend === "docker-linux" ? harnessImage : null,
    cache_level: harnessCacheLevel,
    clean: harnessClean,
    instance_ids: instanceIds,
    commands: selectedCommands,
    native_commands: commands,
    docker_commands: dockerCommands,
    note: "Run this script after ensuring SWE-bench harness dependencies and Docker are ready. Completed harness runs produce official evaluation logs/report.json files.",
  }, null, 2))
  return { script: ps1, commands: selectedCommands, mode, max_workers: effectiveMaxWorkers, backend: harnessBackend }
}

function ensureDockerHarnessImage() {
  if (harnessBackend !== "docker-linux") return { skipped: true, reason: `harness backend is ${harnessBackend}` }
  const inspect = run("docker", ["image", "inspect", harnessImage], { timeoutMs: 30_000 })
  if (inspect.status === 0) return { skipped: false, existed: true, image: harnessImage }
  const buildDir = path.join(runRoot, "harness-image")
  mkdirp(buildDir)
  const dockerfile = path.join(buildDir, "Dockerfile")
  writeFile(dockerfile, [
    "FROM python:3.11-slim",
    "RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*",
    "RUN pip install --no-cache-dir beautifulsoup4 chardet datasets docker ghapi GitPython modal pre-commit python-dotenv requests rich tenacity tqdm unidiff",
    "ENV PYTHONPATH=/swebench",
    "ENV PYTHONUNBUFFERED=1",
  ].join("\n"))
  const build = run("docker", ["build", "-t", harnessImage, buildDir], { timeoutMs: 20 * 60_000 })
  writeFile(path.join(runRoot, "harness-image-build.stdout.log"), build.stdout)
  writeFile(path.join(runRoot, "harness-image-build.stderr.log"), build.stderr)
  if (build.status !== 0) throw new Error(`failed to build ${harnessImage}; see harness-image-build logs`)
  return { skipped: false, existed: false, image: harnessImage }
}

async function maybeRunHarness(harnessPlan) {
  if (!runHarness && !harnessOnly) return { ran: false, reason: "COMMAND_RUN_AGENT_RUN_HARNESS is not 1" }
  if (!fs.existsSync(swebenchRoot)) return { ran: false, error: `missing SWE-bench root: ${swebenchRoot}` }
  const harnessImageStatus = ensureDockerHarnessImage()
  const result = run(process.platform === "win32" ? "powershell.exe" : "pwsh", [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    harnessPlan.script,
  ], { cwd: swebenchRoot, timeoutMs: Number(process.env.COMMAND_RUN_AGENT_HARNESS_TIMEOUT_MS || 6 * 60 * 60_000) })
  writeFile(path.join(runRoot, "harness.stdout.log"), result.stdout)
  writeFile(path.join(runRoot, "harness.stderr.log"), result.stderr)
  return {
    ran: true,
    mode: harnessPlan.mode,
    backend: harnessPlan.backend,
    harness_image_status: harnessImageStatus,
    max_workers: harnessPlan.max_workers,
    exit_code: result.status,
    stdout_path: path.join(runRoot, "harness.stdout.log"),
    stderr_path: path.join(runRoot, "harness.stderr.log"),
    error: result.error,
  }
}

async function runAgentOnTask(agentRun, task, prepared = null) {
  const agentId = agentRun.agent_id
  const agentDir = prepared?.agentDir || path.join(runRoot, task.task_id, agentRun.run_id)
  const workspace = prepared?.workspace || path.join(agentDir, "workspace")
  const prompt = taskPrompt(task)
  const started = performance.now()
  let result
  let error = null
  try {
    if (prepared?.error) throw new Error(prepared.error)
    if (agentId === "current-shll") result = await runCurrentLike(agentId, codexCurrentExe, workspace, agentDir, prompt)
    else if (agentId === "codex-main") result = await runCurrentLike(agentId, codexMainExe, workspace, agentDir, prompt)
    else if (agentId === "tura-fast-shll") result = await runTura(agentId, workspace, agentDir, prompt, "fast")
    else if (agentId === "tura-shll") result = await runTura(agentId, workspace, agentDir, prompt, "coding_agent")
    else if (agentId === "claude-code" || agentId === "pi-agent") result = await runExternalCliAgent(agentId, workspace, agentDir, prompt)
    else throw new Error(`unsupported agent ${agentId}`)
  } catch (err) {
    error = String(err?.stack || err?.message || err)
    result = { status: null, signal: null, stdout: "", stderr: "", duration_ms: 0, first_output_ms: null, error }
  }
  const patch = collectPatch(workspace, agentDir)
  const usage = agentId === "claude-code" || agentId === "pi-agent" ? agentUsageFromJsonl(result.stdout) : chooseUsage(result.stdout, workspace)
  const events = agentId === "claude-code" || agentId === "pi-agent" ? agentEventStats(result.stdout) : eventStats(result.stdout)
  const summary = {
    agent: agentRun.run_id,
    agent_kind: agentId,
    task_id: task.task_id,
    repo: task.repo,
    mode: task.mode,
    harness_compatible: task.harness_compatible,
    prediction_instance_ids: task.instances.map((instance) => instance.instance_id),
    workspace,
    elapsed_ms: Math.round(performance.now() - started),
    exit_code: result.status,
    signal: result.signal,
    first_output_ms: result.first_output_ms,
    error: error || result.error || null,
    stdout_path: path.join(agentDir, "stdout.jsonl"),
    stderr_path: path.join(agentDir, "stderr.log"),
    usage,
    events,
    tura_internal_prompt: result.tura_internal_prompt || null,
    performance: performanceStats(result, usage, patch),
    patch,
  }
  writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(summary, null, 2))
  return summary
}

async function main() {
  mkdirp(runRoot)
  if (harnessOnly) {
    if (!fs.existsSync(summaryPath)) throw new Error(`COMMAND_RUN_AGENT_HARNESS_ONLY=1 requires existing summary: ${summaryPath}`)
    const existingSummary = JSON.parse(fs.readFileSync(summaryPath, "utf8"))
    const instances = existingSummary.instances || []
    assert(instances.length > 0, `existing summary has no instances: ${summaryPath}`)
    const predictionBundles = loadPredictionBundlesFromDisk()
    const harnessPlan = writeHarnessScripts(predictionBundles, instances, "single_issue")
    const harness = await maybeRunHarness(harnessPlan)
    const summary = normalizeBusinessSummary({
      ...existingSummary,
      harness_only: true,
      harness_plan: harnessPlan,
      harness,
    }, runPaths)
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(normalizeBusinessSummary({ ok: harness.ran && harness.exit_code === 0, harness_only: true, harness }, runPaths), null, 2))
    if (harness.ran && harness.exit_code !== 0 && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
    return
  }
  const allInstances = await fetchVerifiedInstances()
  const selection = selectInstances(allInstances)
  const instances = selection.instances
  const tasks = buildTasks(instances)
  const plan = {
    run_id: runId,
    run_root: runRoot,
    source_repos: Object.fromEntries(requestedRepos.map((repo) => [repo, sourceRepoFor(repo)])),
    requested_repos: requestedRepos,
    requested_instance_ids: requestedInstanceIds,
    dropped_instance_ids: selection.dropped_instance_ids,
    issue_table: selectedIssueTable(allInstances),
    mode: "single_issue",
    harness_compatible: tasks.every((task) => task.harness_compatible),
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
    harness_max_workers: harnessMaxWorkers,
    workspace_prep_concurrency: workspacePrepConcurrency,
    agents,
    agent_runs: agentRuns,
    instances: instances.map(redactedInstance),
    tasks: tasks.map(redactedTask),
  }
  writeFile(path.join(runRoot, "plan.json"), JSON.stringify(plan, null, 2))
  if (prepOnly) {
    const summary = normalizeBusinessSummary({ ok: true, prep_only: true, ...plan }, runPaths)
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  if (agentRuns.some((agentRun) => agentRun.agent_id.startsWith("tura-"))) {
    runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_exec"], { cwd: repoRoot, timeoutMs: 240_000 })
    assert(fs.existsSync(turaExe), `missing cli executable after build: ${turaExe}`)
  }
  if (agentRuns.some((agentRun) => agentRun.agent_id === "current-shll")) {
    assert(fs.existsSync(codexCurrentExe), `missing current codex executable: ${codexCurrentExe}`)
  }
  if (agentRuns.some((agentRun) => agentRun.agent_id === "codex-main")) {
    assert(fs.existsSync(codexMainExe), `missing codex-main executable: ${codexMainExe}`)
  }

  const results = []
  for (const task of tasks) {
    console.log(`[agent-swebench-test] preparing ${task.task_id} workspaces with concurrency ${workspacePrepConcurrency}`)
    const prepared = await mapWithConcurrency(agentRuns, workspacePrepConcurrency, async (agentRun) => {
      try {
        return prepareWorkspace(task, agentRun)
      } catch (err) {
        const agentDir = path.join(runRoot, task.task_id, agentRun.run_id)
        const workspace = path.join(agentDir, "workspace")
        mkdirp(agentDir)
        const error = String(err?.stack || err?.message || err)
        writeFile(path.join(agentDir, "prepare-error.log"), error)
        writeFile(path.join(agentDir, "task.json"), JSON.stringify({
          ...redactedTask(task),
          agent: agentRun.agent_id,
          agent_run: agentRun.run_id,
          prepare_error: error,
        }, null, 2))
        writeFile(path.join(agentDir, "prompt.md"), taskPrompt(task))
        return { agentDir, workspace, error }
      }
    })
    console.log(`[agent-swebench-test] running ${task.task_id} with ${agentRuns.map((agentRun) => agentRun.run_id).join(", ")}`)
    const taskResults = await Promise.all(agentRuns.map((agentRun, index) => runAgentOnTask(agentRun, task, prepared[index])))
    results.push({ task: redactedTask(task), results: taskResults })
  }
  const predictionBundles = writePredictionBundles(results)
  const harnessPlan = writeHarnessScripts(predictionBundles, instances)
  const harness = await maybeRunHarness(harnessPlan)
  const comparison = comparisonRows(results)
  const summary = normalizeBusinessSummary({
    ok: results.every((item) => item.results.every((result) => result.exit_code === 0 && result.patch.patch_bytes > 0)),
    ...plan,
    comparison,
    prediction_bundles: predictionBundles,
    harness_plan: harnessPlan,
    harness,
    results,
  }, runPaths)
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
}

await main()
