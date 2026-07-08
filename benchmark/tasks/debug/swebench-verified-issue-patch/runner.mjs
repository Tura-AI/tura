#!/usr/bin/env node
import assert from "node:assert/strict"
import os from "node:os"
import { spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { businessRunPaths, defaultUserWorkspace, normalizeBusinessSummary } from "../../../lib/business_paths.mjs"
import {
  aggregateGenericUsage,
  buildGenericAgentRuns,
  ensureGenericAgentExecutables,
  eventsForAgent,
  eventsWithUsageRounds,
  genericAgentKind,
  genericAgentMode,
  modelForGenericAgent,
  parseGenericAgents,
  priorityEnabled,
  runGenericAgentCli,
  usageForAgent,
} from "../../../lib/generic_agent_cli.mjs"
import {
  buildMatrix,
  mapWithConcurrency as mapMatrixWithConcurrency,
  parseStringList,
  safeName,
  timestampId,
} from "../../../lib/debug_suite_matrix.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const benchmarkRoot = process.env.COMMAND_RUN_AGENT_BENCHMARK_ROOT || path.join(defaultUserWorkspace(), "benchmarks")
const fallbackBenchmarkRoot = path.join(homeDir, "Documents", "benchmark")
const swebenchRoot = process.env.COMMAND_RUN_AGENT_SWEBENCH_ROOT || firstExistingPath([
  path.join(benchmarkRoot, "SWE-bench"),
  path.join(fallbackBenchmarkRoot, "SWE-bench"),
])
const swebenchDataset = process.env.COMMAND_RUN_AGENT_SWEBENCH_DATASET || "SWE-bench/SWE-bench_Verified"
const swebenchConfig = process.env.COMMAND_RUN_AGENT_SWEBENCH_CONFIG || "default"
const swebenchSplit = process.env.COMMAND_RUN_AGENT_SWEBENCH_SPLIT || "test"
const swebenchSuitePrefix = process.env.COMMAND_RUN_AGENT_SWEBENCH_PREFIX
  || (swebenchDataset.toLowerCase().includes("pro") ? "swebench-pro" : "swebench-verified")
const benchmarkTaskName = process.env.COMMAND_RUN_AGENT_BENCHMARK_TASK_NAME || `${swebenchSuitePrefix}-matrix`
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `${benchmarkTaskName}-${timestampId()}`
const runPaths = businessRunPaths(benchmarkTaskName, runId)
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
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "default"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 60 * 60_000)
const agents = parseGenericAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "tura-balanced,tura-direct,codex-main")
const agentRuns = buildGenericAgentRuns(agents)
const workspacePrepConcurrency = Number(process.env.COMMAND_RUN_AGENT_WORKSPACE_PREP_CONCURRENCY || (process.platform === "win32" ? 1 : Math.min(4, Math.max(1, os.cpus().length - 1 || 1))))
const agentConcurrency = Number(process.env.COMMAND_RUN_AGENT_AGENT_CONCURRENCY || agentRuns.length || 1)
const harnessOnly = (process.env.COMMAND_RUN_AGENT_HARNESS_ONLY || "0") === "1"
const requestedInstances = process.env.COMMAND_RUN_AGENT_TASKS
  || process.env.COMMAND_RUN_AGENT_SWEBENCH_TASKS
  || process.env.COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS
  || ""
const requestedInstanceIds = parseStringList(requestedInstances)
if (!harnessOnly) assert(requestedInstanceIds.length > 0, "COMMAND_RUN_AGENT_TASKS or COMMAND_RUN_AGENT_SWEBENCH_INSTANCE_IDS is required")
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

function firstExistingPath(candidates) {
  return candidates.find((candidate) => candidate && fs.existsSync(candidate)) || candidates.find(Boolean) || ""
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
  const normalized = String(instanceId || "").replace(/^instance_/, "")
  let inferred = /^(.+)__(.+)-[0-9a-f]{7,}(?:-.+)?$/i.exec(normalized)
  if (inferred) return `${inferred[1]}/${inferred[2]}`
  inferred = /^(.+)__(.+)-\d+$/.exec(normalized)
  if (inferred) return `${inferred[1]}/${inferred[2]}`
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
  return firstExistingPath([
    path.join(benchmarkRoot, "repos", slug),
    path.join(fallbackBenchmarkRoot, "repos", slug),
  ])
}

function ensureSourceRepo(task) {
  const sourceRepo = sourceRepoFor(task.repo)
  if (!fs.existsSync(path.join(sourceRepo, ".git"))) {
    mkdirp(path.dirname(sourceRepo))
    runOk("git", ["clone", `https://github.com/${normalizeRepoName(task.repo)}.git`, sourceRepo], {
      timeoutMs: Number(process.env.COMMAND_RUN_AGENT_REPO_CLONE_TIMEOUT_MS || 30 * 60_000),
    })
  }
  const baseCommit = task.instances[0]?.base_commit
  if (baseCommit) {
    const hasCommit = run("git", ["cat-file", "-e", `${baseCommit}^{commit}`], {
      cwd: sourceRepo,
      timeoutMs: 60_000,
    })
    if (hasCommit.status !== 0) {
      runOk("git", ["fetch", "--tags", "--force", "origin", baseCommit], {
        cwd: sourceRepo,
        timeoutMs: Number(process.env.COMMAND_RUN_AGENT_REPO_FETCH_TIMEOUT_MS || 20 * 60_000),
      })
    }
  }
  return sourceRepo
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
    if (repoForInstanceId(instanceId) === repo) return true
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

async function fetchVerifiedInstances() {
  const cachePath = path.join(runRoot, `${safeName(swebenchSuitePrefix)}-instances.json`)
  mkdirp(path.dirname(cachePath))
  const localCandidates = [
    path.join(benchmarkRoot, "verified-instances.json"),
    path.join(benchmarkRoot, "verified-xarray-instances.json"),
    path.join(benchmarkRoot, "verified_repo_difficulty_stats.json"),
    path.join(fallbackBenchmarkRoot, "verified-instances.json"),
    path.join(fallbackBenchmarkRoot, "verified-xarray-instances.json"),
    path.join(fallbackBenchmarkRoot, "verified_repo_difficulty_stats.json"),
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
  const rows = await fetchDatasetRowsForRepos(requestedRepos)
  writeFile(cachePath, JSON.stringify(rows.map(redactedInstance), null, 2))
  return rows
}

async function fetchDatasetRowsForRepos(repos) {
  const normalizedRepos = [...new Set(repos.map((repo) => normalizeRepoName(repo)).filter(Boolean))]
  assert(normalizedRepos.length > 0, "at least one repo is required to fetch SWE-bench rows")
  const rows = []
  for (let offset = 0; ; offset += 100) {
    const url = new URL("https://datasets-server.huggingface.co/rows")
    url.searchParams.set("dataset", swebenchDataset)
    url.searchParams.set("config", swebenchConfig)
    url.searchParams.set("split", swebenchSplit)
    url.searchParams.set("offset", String(offset))
    url.searchParams.set("length", "100")
    const response = await fetchWithRetry(url)
    const data = await response.json()
    const got = data.rows || []
    rows.push(...got.map((item) => item.row).filter((row) => normalizedRepos.includes(normalizeRepoName(row.repo))))
    if (got.length < 100) break
  }
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
      dropped.push({ instance_id: id, reason: "not found in selected SWE-bench rows" })
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
  const sourceRepo = ensureSourceRepo(task)
  mkdirp(agentDir)
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
  runOk("git", ["config", "core.longpaths", "true"], { cwd: workspace, timeoutMs: 30_000 })
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
    if (!/Permission denied|unable to write file|index\.lock|File exists|resource busy|failed to insert into database/i.test(output)) {
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
  if (!fs.existsSync(workspace)) return
  try {
    chmodTree(workspace)
  } catch {
    // Best-effort: Windows ACL repair below is usually the important part.
  }
  if (process.platform !== "win32") return
  run("attrib", ["-R", path.join(workspace, "*"), "/S", "/D"], { cwd: workspace, timeoutMs: 120_000 })
  const grants = ["*S-1-5-32-545:(OI)(CI)F", "*S-1-1-0:(OI)(CI)F", "Users:(OI)(CI)F", "Everyone:(OI)(CI)F"]
  const whoami = run("whoami", [], { cwd: workspace, timeoutMs: 30_000 })
  if (whoami.status === 0 && whoami.stdout.trim()) {
    grants.unshift(`${whoami.stdout.trim()}:(OI)(CI)F`)
  }
  for (const grant of grants) {
    run("icacls", [workspace, "/grant", grant, "/T", "/C", "/Q"], { cwd: workspace, timeoutMs: 120_000 })
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

function isSwebenchProDataset() {
  return /swe-bench[_-]?pro|swebench-pro/i.test(`${swebenchDataset} ${swebenchSuitePrefix}`)
}

function parseMaybeJsonList(value) {
  if (Array.isArray(value)) {
    if (value.length === 1 && typeof value[0] === "string") {
      const parsed = parseMaybeJsonList(value[0])
      if (parsed.length > 1) return parsed
    }
    return value
  }
  if (value === null || value === undefined || value === "") return []
  if (typeof value === "string") {
    const text = value.trim()
    if (!text) return []
    try {
      const parsed = JSON.parse(text)
      return Array.isArray(parsed) ? parsed : [parsed]
    } catch {
      const parsedList = parseLooseQuotedList(text)
      return parsedList || [text]
    }
  }
  return [value]
}

function parseLooseQuotedList(text) {
  if (!text.startsWith("[") || !text.endsWith("]")) return null
  const values = []
  let index = 1
  while (index < text.length - 1) {
    while (index < text.length - 1 && /[\s,]/.test(text[index])) index += 1
    if (index >= text.length - 1) break
    const quote = text[index]
    if (quote !== "'" && quote !== "\"") {
      const start = index
      while (index < text.length - 1 && text[index] !== ",") index += 1
      const raw = text.slice(start, index).trim()
      if (raw) values.push(raw)
      continue
    }
    index += 1
    let current = ""
    while (index < text.length) {
      const char = text[index]
      if (char === "\\" && index + 1 < text.length) {
        current += text[index + 1]
        index += 2
        continue
      }
      if (char === quote) {
        index += 1
        break
      }
      current += char
      index += 1
    }
    values.push(current)
    while (index < text.length - 1 && /[\s,]/.test(text[index])) index += 1
  }
  return values
}

function hasHarnessDatasetFields(instance) {
  return Boolean(instance?.test_patch && (instance.FAIL_TO_PASS || instance.fail_to_pass) && (instance.PASS_TO_PASS || instance.pass_to_pass))
}

async function hydrateHarnessInstances(instances) {
  if (!isSwebenchProDataset() || instances.every(hasHarnessDatasetFields)) return instances
  const adaptedPath = path.join(runRoot, "swebench-pro-adapted-dataset.json")
  if (fs.existsSync(adaptedPath)) {
    const cached = JSON.parse(fs.readFileSync(adaptedPath, "utf8"))
    if (Array.isArray(cached) && cached.every(hasHarnessDatasetFields)) return cached
  }
  const repos = [...new Set([
    ...requestedRepos,
    ...instances.map((instance) => normalizeRepoName(instance.repo)).filter(Boolean),
  ])]
  const rows = await fetchDatasetRowsForRepos(repos)
  const byId = new Map(rows.map((row) => [row.instance_id, row]))
  return instances.map((instance) => byId.get(instance.instance_id) || instance)
}

function adaptHarnessInstance(instance) {
  if (!isSwebenchProDataset()) return instance
  const failToPass = parseMaybeJsonList(instance.FAIL_TO_PASS ?? instance.fail_to_pass)
  const passToPass = parseMaybeJsonList(instance.PASS_TO_PASS ?? instance.pass_to_pass)
  return {
    ...instance,
    repo: normalizeRepoName(instance.repo),
    version: instance.version || "pro",
    FAIL_TO_PASS: failToPass,
    PASS_TO_PASS: passToPass,
    hints_text: instance.hints_text || "",
    created_at: instance.created_at || "",
    environment_setup_commit: instance.environment_setup_commit || instance.base_commit,
  }
}

function runRootPosix(file) {
  return `/runroot/${path.relative(runRoot, file).replaceAll(path.sep, "/")}`
}

function prepareHarnessDataset(instances) {
  if (!isSwebenchProDataset()) {
    return {
      original_dataset: swebenchDataset,
      native_dataset_name: swebenchDataset,
      docker_dataset_name: swebenchDataset,
      adapted: false,
    }
  }
  const adaptedInstances = instances.map(adaptHarnessInstance)
  const adaptedPath = path.join(runRoot, "swebench-pro-adapted-dataset.json")
  writeFile(adaptedPath, JSON.stringify(adaptedInstances, null, 2))
  const runtimeScript = writeSwebenchProRuntimeScript()
  return {
    original_dataset: swebenchDataset,
    native_dataset_name: adaptedPath,
    docker_dataset_name: runRootPosix(adaptedPath),
    adapted_dataset_path: adaptedPath,
    docker_adapted_dataset_path: runRootPosix(adaptedPath),
    runtime_script_path: runtimeScript,
    docker_runtime_script_path: runRootPosix(runtimeScript),
    adapted: true,
    note: "SWE-bench Pro rows are adapted to the stock harness schema and NodeBB/NodeBB is injected at runtime.",
  }
}

function writeSwebenchProRuntimeScript() {
  const runtimeScript = path.join(runRoot, "swebench-pro-runtime", "run_pro_evaluation.py")
  writeFile(runtimeScript, String.raw`import re
import runpy

import swebench.harness.constants as constants
import swebench.harness.log_parsers as log_parsers
from swebench.harness.constants import TestStatus
from swebench.harness.log_parsers.javascript import parse_log_tap


NODEBB_SPEC = {
    "docker_specs": {
        "node_version": "20",
        "_variant": "js_2",
    },
    "apt-pkgs": ["python3", "redis-server"],
    "install": [
        "cp install/package.json package.json",
        """cat > config.json <<'JSON'
{
  "url": "http://127.0.0.1:4567",
  "secret": "swebench-nodebb",
  "database": "redis",
  "redis": {
    "host": "127.0.0.1",
    "port": 6379,
    "password": "",
    "database": 0
  },
  "test_database": {
    "host": "127.0.0.1",
    "port": 6379,
    "password": "",
    "database": 1
  }
}
JSON""",
        "npm install",
    ],
    "test_cmd": [
        "redis-server --daemonize yes --save '' --appendonly no",
        "for i in $(seq 1 20); do redis-cli ping && break || sleep 1; done",
        "npx mocha -R tap --exit test/database.js test/user/emails.js",
    ],
}


def normalize_nodebb_case(name):
    text = str(name or "").strip()
    text = re.sub(r"\s+#\s*time=.*$", "", text)
    if " | " in text:
        text = text.split(" | ", 1)[1]
    text = re.sub(r"(^|\s)test/[^:\s]+\.js::", " ", text)
    text = re.sub(r"\s+", " ", text).strip()
    return text


def parse_log_nodebb(log, test_spec):
    raw_status = parse_log_tap(log, test_spec)
    status = dict(raw_status)
    normalized_raw = {}
    for raw_name, raw_value in raw_status.items():
        normalized_raw.setdefault(normalize_nodebb_case(raw_name), raw_value)

    expected_cases = list(test_spec.FAIL_TO_PASS) + list(test_spec.PASS_TO_PASS)
    for expected in expected_cases:
        normalized_expected = normalize_nodebb_case(expected)
        if normalized_expected in normalized_raw:
            status[expected] = normalized_raw[normalized_expected]
            continue
        tail = normalize_nodebb_case(str(expected).split("::")[-1])
        for raw_name, raw_value in normalized_raw.items():
            if raw_name.endswith(tail) or tail.endswith(raw_name):
                status[expected] = raw_value
                break
    return status


constants.MAP_REPO_VERSION_TO_SPECS.setdefault("NodeBB/NodeBB", {})["pro"] = NODEBB_SPEC
constants.MAP_REPO_TO_EXT["NodeBB/NodeBB"] = "js"
log_parsers.MAP_REPO_TO_PARSER["NodeBB/NodeBB"] = parse_log_nodebb

runpy.run_module("swebench.harness.run_evaluation", run_name="__main__")
`)
  return runtimeScript
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
      events: Number(result.events.events || 0),
      llm_rounds: Number(result.events.llm_rounds || 0),
      agent_messages: Number(result.events.agent_messages || 0),
      commands_started: Number(result.events.commands_started || result.events.command_executions || 0),
      commands_completed: Number(result.events.commands_completed || 0),
      commands_failed: Number(result.events.commands_failed || 0),
      file_changes: Number(result.events.file_changes || 0),
      file_changes_failed: Number(result.events.file_changes_failed || 0),
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
  const harnessDataset = prepareHarnessDataset(instances)
  for (const bundle of predictionBundles) {
    const harnessNamespace = harnessDataset.adapted ? "none" : "swebench"
    const harnessRunId = `${runId}-${bundle.agent}${harnessDataset.adapted ? "-pro-local" : ""}`.replace(/[^A-Za-z0-9_.-]+/g, "-")
    const nativeEntrypoint = harnessDataset.runtime_script_path
      ? `python ${shellQuotePs(harnessDataset.runtime_script_path)}`
      : "python -m swebench.harness.run_evaluation"
    const nativeReportDir = path.join(runRoot, "reports", safeName(bundle.agent))
    commands.push([
      nativeEntrypoint,
      `  --dataset_name ${shellQuotePs(harnessDataset.native_dataset_name)}`,
      `  --predictions_path ${shellQuotePs(bundle.all_preds_jsonl)}`,
      `  --max_workers ${effectiveMaxWorkers}`,
      `  --cache_level ${shellQuotePs(harnessCacheLevel)}`,
      `  --clean ${shellQuotePs(harnessClean)}`,
      `  --run_id ${shellQuotePs(harnessRunId)}`,
      `  --namespace ${shellQuotePs(harnessNamespace)}`,
      harnessDataset.adapted ? "  --force_rebuild true" : "",
      `  --report_dir ${shellQuotePs(nativeReportDir)}`,
      instanceArgs ? ` ${instanceArgs.trimStart()}` : "",
    ].filter(Boolean).join(" `\n"))
    const dockerPredictionPath = `/runroot/${path.relative(runRoot, bundle.all_preds_jsonl).replaceAll(path.sep, "/")}`
    const dockerEntrypoint = harnessDataset.docker_runtime_script_path
      ? ["python", harnessDataset.docker_runtime_script_path]
      : ["python", "-m", "swebench.harness.run_evaluation"]
    const dockerArgs = [
      ...dockerEntrypoint,
      "--dataset_name",
      harnessDataset.docker_dataset_name,
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
      harnessNamespace,
      ...(harnessDataset.adapted ? ["--force_rebuild", "true"] : []),
      "--report_dir",
      `/runroot/reports/${safeName(bundle.agent)}`,
      ...(instanceIds.length > 0 ? ["--instance_ids", ...instanceIds] : []),
    ]
    dockerCommands.push([
      "docker run --rm",
      "  -v /var/run/docker.sock:/var/run/docker.sock",
      "  -e PYTHONPATH=/swebench",
      `  -v ${shellQuotePs(`${swebenchRoot}:/swebench`)}`,
      `  -v ${shellQuotePs(`${runRoot}:/runroot`)}`,
      "  -w /runroot",
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
    `$env:PYTHONPATH = ${shellQuotePs(swebenchRoot)} + [System.IO.Path]::PathSeparator + $env:PYTHONPATH`,
    `Set-Location ${shellQuotePs(runRoot)}`,
    "",
    ...checkedCommands,
    "exit $global:HarnessExitCode",
  ].join("\n"))
  writeFile(path.join(runRoot, "harness-plan.json"), JSON.stringify({
    swebench_root: swebenchRoot,
    dataset: swebenchDataset,
    harness_dataset: harnessDataset,
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
    result = await runGenericAgentCli({
      agentId,
      workspace,
      agentDir,
      prompt,
      repoRoot,
      model,
      turaModel,
      reasoning,
      serviceTier,
      timeoutMs,
    })
  } catch (err) {
    error = String(err?.stack || err?.message || err)
    result = {
      status: null,
      signal: null,
      stdout: "",
      stderr: "",
      duration_ms: 0,
      first_output_ms: null,
      last_progress_ms: null,
      error,
      context_archive: null,
      events: eventsWithUsageRounds(eventsForAgent("", agentId), usageForAgent(agentDir, "", agentId).usage),
    }
  }
  const patch = collectPatch(workspace, agentDir)
  const usageInfo = usageForAgent(agentDir, result.stdout || "", agentId)
  const usage = usageInfo.usage
  const events = eventsWithUsageRounds(result.events || eventsForAgent(result.stdout || "", agentId), usage)
  const summary = {
    agent: agentRun.run_id,
    agent_id: agentId,
    agent_kind: genericAgentKind(agentId),
    agent_mode: genericAgentMode(agentId),
    model: modelForGenericAgent(agentId, { model, turaModel }),
    tura_model: genericAgentKind(agentId) === "tura" ? turaModel : null,
    reasoning,
    service_tier: serviceTier,
    priority_enabled: priorityEnabled(serviceTier),
    task: task.task_id,
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
    last_progress_ms: result.last_progress_ms,
    error: error || result.error || null,
    stdout_path: path.join(agentDir, "stdout.jsonl"),
    stderr_path: path.join(agentDir, "stderr.log"),
    provider_log_path: path.join(agentDir, "provider-log"),
    usage,
    usage_source: usageInfo.usage_source,
    provider_calls: usageInfo.provider_calls,
    context_archive: result.context_archive,
    events,
    fixture_backend: result.fixture_backend || null,
    fixture_source_path: result.fixture_source_path || null,
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
    const instances = await hydrateHarnessInstances(existingSummary.instances || [])
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
    agent_concurrency: agentConcurrency,
    agents,
    agent_runs: agentRuns,
    instances: instances.map(redactedInstance),
    tasks: tasks.map(redactedTask),
    matrix: buildMatrix(tasks, agentRuns).map((job) => ({
      task_id: job.task.task_id,
      agent: job.agentRun.agent_id,
      agent_run: job.agentRun.run_id,
    })),
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
  ensureGenericAgentExecutables(agents, { repoRoot })

  const matrix = buildMatrix(tasks, agentRuns)
  console.log(`[agent-swebench-test] preparing ${matrix.length} task-agent workspaces with concurrency ${workspacePrepConcurrency}`)
  const prepared = await mapMatrixWithConcurrency(matrix, workspacePrepConcurrency, async (job) => {
    try {
      return prepareWorkspace(job.task, job.agentRun)
    } catch (err) {
      const agentDir = path.join(runRoot, job.task.task_id, job.agentRun.run_id)
      const workspace = path.join(agentDir, "workspace")
      mkdirp(agentDir)
      const error = String(err?.stack || err?.message || err)
      writeFile(path.join(agentDir, "prepare-error.log"), error)
      writeFile(path.join(agentDir, "task.json"), JSON.stringify({
        ...redactedTask(job.task),
        agent: job.agentRun.agent_id,
        agent_run: job.agentRun.run_id,
        prepare_error: error,
      }, null, 2))
      writeFile(path.join(agentDir, "prompt.md"), taskPrompt(job.task))
      return { agentDir, workspace, error }
    }
  })

  console.log(`[agent-swebench-test] running ${matrix.length} task-agent jobs with concurrency ${agentConcurrency}`)
  const flatResults = await mapMatrixWithConcurrency(matrix, agentConcurrency, async (job, index) => {
    return runAgentOnTask(job.agentRun, job.task, prepared[index])
  })
  const byTaskId = new Map(tasks.map((task) => [task.task_id, { task: redactedTask(task), results: [] }]))
  for (const result of flatResults) {
    byTaskId.get(result.task_id)?.results.push(result)
  }
  const taskResults = tasks.map((task) => byTaskId.get(task.task_id)).filter(Boolean)
  const predictionBundles = writePredictionBundles(taskResults)
  const harnessInstances = await hydrateHarnessInstances(instances)
  const harnessPlan = writeHarnessScripts(predictionBundles, harnessInstances)
  const harness = await maybeRunHarness(harnessPlan)
  const comparison = comparisonRows(taskResults)
  const summary = normalizeBusinessSummary({
    ok: flatResults.every((result) => result.exit_code === 0 && result.patch.patch_bytes > 0),
    ...plan,
    aggregate_usage: aggregateGenericUsage(flatResults),
    comparison,
    prediction_bundles: predictionBundles,
    harness_plan: harnessPlan,
    harness,
    task_results: taskResults,
    results: flatResults,
  }, runPaths)
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
}

await main()
