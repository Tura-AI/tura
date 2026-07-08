#!/usr/bin/env node
import assert from "node:assert/strict"
import crypto from "node:crypto"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { spawnSync } from "node:child_process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"

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
import { businessRunPaths, defaultUserWorkspace, normalizeBusinessSummary } from "../../../lib/business_paths.mjs"
import {
  buildMatrix,
  flattenResults,
  mapWithConcurrency,
  maxResultElapsedMs,
  parseTaskList,
  safeName,
  timestampId,
} from "../../../lib/debug_suite_matrix.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..")
const benchmarkRoot = process.env.COMMAND_RUN_AGENT_BENCHMARK_ROOT || path.join(defaultUserWorkspace(), "benchmarks")
const deepSweRoot = process.env.COMMAND_RUN_AGENT_DEEPSWE_ROOT || path.join(benchmarkRoot, "deep-swe")
const requestedTaskIds = parseTaskList({
  value: process.env.COMMAND_RUN_AGENT_TASKS,
  suiteValue: process.env.COMMAND_RUN_AGENT_DEEPSWE_TASKS || process.env.COMMAND_RUN_AGENT_DEEPSWE_TASK,
  fallback: ["anko-default-function-arguments"],
  label: "COMMAND_RUN_AGENT_TASKS/COMMAND_RUN_AGENT_DEEPSWE_TASKS",
})
const selectedTaskIds = expandRequestedTaskIds(requestedTaskIds)
const benchmarkTaskName = process.env.COMMAND_RUN_AGENT_BENCHMARK_TASK_NAME
  || (selectedTaskIds.length === 1 ? `deepswe-${safeName(selectedTaskIds[0])}` : "deepswe-matrix")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `${benchmarkTaskName}-${timestampId()}`
const runPaths = businessRunPaths(benchmarkTaskName, runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path

const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "medium"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "default"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 60 * 60_000)
const agents = parseGenericAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "tura-balanced,tura-direct,codex-main")
const agentRuns = buildGenericAgentRuns(agents)
const worksetAgents = parseGenericAgents(process.env.COMMAND_RUN_AGENT_WORKSET_AGENTS || agents.join(","))
const archivedCompletedAgents = parseGenericAgents(process.env.COMMAND_RUN_AGENT_ARCHIVED_COMPLETED_AGENTS || agents.join(","))
const workspacePrepConcurrency = Number(process.env.COMMAND_RUN_AGENT_WORKSPACE_PREP_CONCURRENCY || 1)
const taskConcurrency = Number(process.env.COMMAND_RUN_AGENT_TASK_CONCURRENCY || 1)
const agentConcurrencyPerTask = Number(process.env.COMMAND_RUN_AGENT_AGENT_CONCURRENCY_PER_TASK || agentRuns.length || 1)
const scheduleMode = String(process.env.COMMAND_RUN_AGENT_SCHEDULE_MODE || "task").toLowerCase()
const routeConcurrency = Number(process.env.COMMAND_RUN_AGENT_ROUTE_CONCURRENCY
  || (Math.max(1, taskConcurrency) * Math.max(1, agentConcurrencyPerTask)))
const routeStartStaggerMs = Number(process.env.COMMAND_RUN_AGENT_ROUTE_START_STAGGER_MS || 0)
const runEval = (process.env.COMMAND_RUN_AGENT_RUN_EVAL || "1") === "1"
const evalOnly = (process.env.COMMAND_RUN_AGENT_EVAL_ONLY || process.env.COMMAND_RUN_AGENT_HARNESS_ONLY || "0") === "1"
const evalCompletedOnly = (process.env.COMMAND_RUN_AGENT_EVAL_COMPLETED_ONLY || "0") === "1"
const evalPendingOnly = (process.env.COMMAND_RUN_AGENT_EVAL_PENDING_ONLY || "0") === "1"
const harnessTaskConcurrency = Number(process.env.COMMAND_RUN_AGENT_HARNESS_TASK_CONCURRENCY || process.env.COMMAND_RUN_AGENT_DOCKER_CONCURRENCY || 1)
const contractOnly = process.env.COMMAND_RUN_AGENT_CONTRACT_ONLY === "1"
const autoStartDocker = (process.env.COMMAND_RUN_AGENT_START_DOCKER || "1") !== "0"
const dockerReadyTimeoutMs = Number(process.env.COMMAND_RUN_AGENT_DOCKER_READY_TIMEOUT_MS || 5 * 60_000)
const allowFailure = process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE === "1"
const selfTest = process.env.COMMAND_RUN_AGENT_SELF_TEST === "1"
const verifierContextVersion = "lf-context-v1"
const minimalPromptConfig = loadMinimalPromptConfig()

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text, "utf8")
}

function writeJson(file, value) {
  writeFile(file, `${JSON.stringify(value, null, 2)}\n`)
}

function readJsonIfExists(file) {
  if (!file || !fs.existsSync(file)) return null
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"))
  } catch {
    return null
  }
}

function loadMinimalPromptConfig() {
  const rawPath = process.env.COMMAND_RUN_AGENT_DEEPSWE_MINIMAL_PROMPTS
  if (!rawPath) return null
  const configPath = path.isAbsolute(rawPath) ? rawPath : path.resolve(repoRoot, rawPath)
  const config = readJsonIfExists(configPath)
  assert(config, `missing or invalid DeepSWE minimal prompt config: ${configPath}`)
  assert(config.tasks && typeof config.tasks === "object", `DeepSWE minimal prompt config must include tasks: ${configPath}`)
  return {
    ...config,
    config_path: configPath,
    common_rules: Array.isArray(config.common_rules) ? config.common_rules : [],
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function allDeepSweTaskIds() {
  ensureDeepSweRepo()
  const tasksDir = path.join(deepSweRoot, "tasks")
  assert(fs.existsSync(tasksDir), `missing DeepSWE tasks directory: ${tasksDir}`)
  return fs.readdirSync(tasksDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .filter((taskId) => fs.existsSync(path.join(tasksDir, taskId, "task.toml")))
    .sort((left, right) => left.localeCompare(right))
}

function expandRequestedTaskIds(taskIds) {
  if (!taskIds.some((taskId) => /^(all|\*)$/i.test(taskId))) return taskIds
  const expanded = allDeepSweTaskIds()
  assert(expanded.length > 0, "DeepSWE all-task expansion found no tasks")
  return expanded
}

function run(command, args, options = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || 10 * 60_000,
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

function readText(file) {
  return fs.readFileSync(file, "utf8")
}

function copyDir(src, dest) {
  mkdirp(dest)
  fs.cpSync(src, dest, { recursive: true, force: true })
}

function walkFiles(rootDir, options = {}) {
  const files = []
  const stack = [rootDir]
  const skipDirectoryNames = new Set(options.skipDirectoryNames || [])
  while (stack.length > 0) {
    const current = stack.pop()
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name)
      if (entry.isDirectory()) {
        if (!skipDirectoryNames.has(entry.name)) stack.push(full)
      }
      else if (entry.isFile()) files.push(full)
    }
  }
  return files.sort((a, b) => a.localeCompare(b))
}

function isLikelyText(buffer) {
  if (buffer.includes(0)) return false
  if (buffer.length === 0) return true
  const decoded = buffer.toString("utf8")
  const replacementCount = (decoded.match(/\uFFFD/g) || []).length
  return replacementCount / buffer.length < 0.01
}

function normalizeTextLineEndings(file) {
  const buffer = fs.readFileSync(file)
  if (!isLikelyText(buffer)) return false
  const normalized = Buffer.from(buffer.toString("utf8").replace(/\r\n/g, "\n").replace(/\r/g, "\n"), "utf8")
  if (Buffer.compare(buffer, normalized) === 0) return false
  fs.writeFileSync(file, normalized)
  return true
}

function normalizeTreeLineEndings(rootDir, options = {}) {
  const normalized = []
  for (const file of walkFiles(rootDir, options)) {
    if (normalizeTextLineEndings(file)) normalized.push(path.relative(rootDir, file).replace(/\\/g, "/"))
  }
  return normalized
}

function prepareVerifierBuildContext(task) {
  const contextDir = path.join(runRoot, "verifier-build-context", safeName(task.id))
  if (fs.existsSync(contextDir)) fs.rmSync(contextDir, { recursive: true, force: true })
  copyDir(task.testsDir, contextDir)
  const normalized = normalizeTreeLineEndings(contextDir)
  writeJson(path.join(runRoot, `verifier-build-context-${safeName(task.id)}.json`), {
    source: task.testsDir,
    context_dir: contextDir,
    version: verifierContextVersion,
    normalized_line_endings: normalized,
  })
  return contextDir
}

function parseTomlString(text, key) {
  const match = new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, "m").exec(text)
  return match ? match[1] : ""
}

function parseTomlNumber(text, key) {
  const match = new RegExp(`^${key}\\s*=\\s*([0-9.]+)`, "m").exec(text)
  return match ? Number(match[1]) : null
}

function loadDeepSweTask(taskId) {
  const taskDir = path.join(deepSweRoot, "tasks", taskId)
  const tomlPath = path.join(taskDir, "task.toml")
  const instructionPath = path.join(taskDir, "instruction.md")
  assert(fs.existsSync(tomlPath), `missing DeepSWE task.toml: ${tomlPath}`)
  assert(fs.existsSync(instructionPath), `missing DeepSWE instruction.md: ${instructionPath}`)
  const toml = readText(tomlPath)
  const instruction = readText(instructionPath)
  const metadata = {
    name: parseTomlString(toml, "name"),
    ext_id: parseTomlString(toml, "ext_id"),
    task_id: parseTomlString(toml, "task_id") || taskId,
    display_title: parseTomlString(toml, "display_title"),
    display_description: parseTomlString(toml, "display_description"),
    language: parseTomlString(toml, "language"),
    repository_url: parseTomlString(toml, "repository_url"),
    base_commit_hash: parseTomlString(toml, "base_commit_hash"),
    docker_image: parseTomlString(toml, "docker_image"),
    agent_timeout_sec: parseTomlNumber(toml, "timeout_sec"),
  }
  assert(metadata.repository_url, `missing repository_url in ${tomlPath}`)
  assert(metadata.base_commit_hash, `missing base_commit_hash in ${tomlPath}`)
  return {
    id: taskId,
    label: taskId,
    taskDir,
    testsDir: path.join(taskDir, "tests"),
    instruction,
    metadata,
  }
}

function ensureDeepSweRepo() {
  if (fs.existsSync(path.join(deepSweRoot, ".git"))) return
  mkdirp(path.dirname(deepSweRoot))
  runOk("git", ["clone", "--depth", "1", "https://github.com/datacurve-ai/deep-swe.git", deepSweRoot], {
    timeoutMs: 5 * 60_000,
  })
}

function repoCacheDir(task) {
  const repoKey = task.metadata.repository_url
    .replace(/^https?:\/\/github\.com\//, "")
    .replace(/\.git$/, "")
  return path.join(benchmarkRoot, "repos", "deepswe", safeName(repoKey))
}

function ensureSourceRepo(task) {
  const cacheDir = repoCacheDir(task)
  if (!fs.existsSync(path.join(cacheDir, ".git"))) {
    mkdirp(path.dirname(cacheDir))
    runOk("git", ["clone", task.metadata.repository_url, cacheDir], {
      timeoutMs: 20 * 60_000,
    })
  }
  const hasCommit = run("git", ["cat-file", "-e", `${task.metadata.base_commit_hash}^{commit}`], {
    cwd: cacheDir,
    timeoutMs: 60_000,
  })
  if (hasCommit.status !== 0) {
    runOk("git", ["fetch", "--tags", "--force", "origin", task.metadata.base_commit_hash], {
      cwd: cacheDir,
      timeoutMs: 10 * 60_000,
    })
  }
  return cacheDir
}

function prepareWorkspace(task, agentRun, index) {
  const taskRunDir = safeName(task.id)
  const agentDir = path.join(runRoot, taskRunDir, `${agentRun.run_id}-${index + 1}`)
  const workspace = path.join(agentDir, "workspace")
  const sourceRepo = ensureSourceRepo(task)
  mkdirp(agentDir)
  if (fs.existsSync(workspace)) fs.rmSync(workspace, { recursive: true, force: true })
  runOk("git", ["-c", "core.longpaths=true", "clone", "--no-hardlinks", sourceRepo, workspace], {
    timeoutMs: 20 * 60_000,
  })
  runOk("git", ["checkout", "--force", task.metadata.base_commit_hash], {
    cwd: workspace,
    timeoutMs: 5 * 60_000,
  })
  runOk("git", ["clean", "-fdx"], { cwd: workspace, timeoutMs: 5 * 60_000 })
  const normalizedLineEndings = normalizeTreeLineEndings(workspace, { skipDirectoryNames: [".git"] })
  try { run("git", ["remote", "remove", "origin"], { cwd: workspace, timeoutMs: 30_000 }) } catch {}
  const deepsweDir = path.join(workspace, ".deepswe")
  mkdirp(deepsweDir)
  writeFile(path.join(deepsweDir, "instruction.md"), taskAgentInstruction(task))
  writeJson(path.join(deepsweDir, "task.json"), {
    task_id: task.id,
    name: task.metadata.name,
    title: task.metadata.display_title,
    language: task.metadata.language,
    repository_url: task.metadata.repository_url,
    base_commit_hash: task.metadata.base_commit_hash,
    note: "The hidden verifier files and reference solution are intentionally not present in this workspace.",
    prompt_variant: minimalPromptConfig ? "minimal" : "upstream",
  })
  isolateWorkspaceGit(workspace)
  const baseSnapshot = runOk("git", ["rev-parse", "HEAD"], { cwd: workspace, timeoutMs: 30_000 }).stdout.trim()
  const prompt = taskPrompt(task)
  writeJson(path.join(agentDir, "task.json"), {
    task: redactedTask(task),
    agent: agentRun.agent_id,
    agent_run: agentRun.run_id,
    workspace,
    base_snapshot: baseSnapshot,
    normalized_line_endings: normalizedLineEndings,
  })
  writeFile(path.join(agentDir, "prompt.md"), prompt)
  return { agentDir, workspace, baseSnapshot, prompt, prompt_path: path.join(agentDir, "prompt.md") }
}

function isolateWorkspaceGit(workspace) {
  const gitDir = path.join(workspace, ".git")
  repairWorkspacePermissions(workspace)
  if (fs.existsSync(gitDir)) fs.rmSync(gitDir, { recursive: true, force: true })
  runOk("git", ["init"], { cwd: workspace, timeoutMs: 60_000 })
  repairWorkspacePermissions(workspace)
  runOk("git", ["config", "user.email", "deepswe-benchmark@example.invalid"], { cwd: workspace, timeoutMs: 30_000 })
  runOk("git", ["config", "user.name", "DeepSWE Benchmark"], { cwd: workspace, timeoutMs: 30_000 })
  runOk("git", ["config", "core.autocrlf", "false"], { cwd: workspace, timeoutMs: 30_000 })
  runOk("git", ["config", "core.longpaths", "true"], { cwd: workspace, timeoutMs: 30_000 })
  runGitAddWithRetry(workspace)
  runGitCommitWithRetry(workspace)
}

function runGitAddWithRetry(workspace) {
  const attempts = []
  for (let attempt = 1; attempt <= 5; attempt += 1) {
    repairWorkspacePermissions(workspace)
    const result = run("git", ["-c", "core.fscache=false", "-c", "core.preloadindex=false", "add", "-A"], {
      cwd: workspace,
      timeoutMs: 5 * 60_000,
    })
    attempts.push(result)
    if (result.status === 0) return
    const output = `${result.stderr}\n${result.error || ""}`
    if (!/Permission denied|unable to write file|index\.lock|File exists|resource busy|failed to insert into database/i.test(output)) {
      throw new Error(`git add -A failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
    }
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

function runGitCommitWithRetry(workspace) {
  const attempts = []
  for (let attempt = 1; attempt <= 5; attempt += 1) {
    repairWorkspacePermissions(workspace)
    const result = run("git", ["-c", "core.fscache=false", "-c", "core.preloadindex=false", "commit", "-m", "DeepSWE base snapshot"], {
      cwd: workspace,
      timeoutMs: 5 * 60_000,
    })
    attempts.push(result)
    if (result.status === 0) return
    const output = `${result.stderr}\n${result.error || ""}`
    if (!/Permission denied|unable to write file|index\.lock|File exists|resource busy|failed to write commit object|failed to insert into database/i.test(output)) {
      throw new Error(`git commit -m DeepSWE base snapshot failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
    }
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 500 * attempt)
  }
  const details = attempts.map((attempt, index) => [
    `ATTEMPT ${index + 1} STATUS ${attempt.status}`,
    `STDOUT:\n${attempt.stdout}`,
    `STDERR:\n${attempt.stderr}`,
    `ERROR:\n${attempt.error || ""}`,
  ].join("\n")).join("\n\n")
  throw new Error(`git commit -m DeepSWE base snapshot failed after ${attempts.length} attempts\n${details}`)
}

function repairWorkspacePermissions(workspace) {
  if (!fs.existsSync(workspace)) return
  try { chmodTree(workspace) } catch {}
  if (process.platform !== "win32") return
  run("attrib", ["-R", path.join(workspace, "*"), "/S", "/D"], { cwd: workspace, timeoutMs: 120_000 })
  const grants = ["*S-1-5-32-545:(OI)(CI)F", "*S-1-1-0:(OI)(CI)F", "Users:(OI)(CI)F", "Everyone:(OI)(CI)F"]
  const whoami = run("whoami", [], { cwd: workspace, timeoutMs: 30_000 })
  if (whoami.status === 0 && whoami.stdout.trim()) grants.unshift(`${whoami.stdout.trim()}:(OI)(CI)F`)
  for (const grant of grants) {
    run("icacls", [workspace, "/grant", grant, "/T", "/C", "/Q"], { cwd: workspace, timeoutMs: 120_000 })
  }
}

function chmodTree(rootDir) {
  const stack = [rootDir]
  while (stack.length > 0) {
    const current = stack.pop()
    try { fs.chmodSync(current, 0o777) } catch {}
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(full)
      else {
        try { fs.chmodSync(full, 0o666) } catch {}
      }
    }
  }
}

function taskPrompt(task) {
  if (minimalPromptConfig) {
    return `You are working on a DeepSWE task adapted into this benchmark workspace.

Repository: ${task.metadata.repository_url}
Base commit: ${task.metadata.base_commit_hash}
Language: ${task.metadata.language}
Task: ${task.metadata.display_title || task.id}

${taskAgentInstruction(task)}
`
  }

  return `You are working on a DeepSWE task adapted into this benchmark workspace.

Repository: ${task.metadata.repository_url}
Base commit: ${task.metadata.base_commit_hash}
Language: ${task.metadata.language}
Task: ${task.metadata.display_title || task.id}

Rules:
- Do not search the internet.
- Do not inspect or rely on hidden tests, verifier files, or reference solutions. They are not in this workspace.
- Implement the requested behavior in the repository code.
- Keep the change focused and avoid unrelated refactors.
- Run relevant local checks when available.
- The official verifier will grade your patch in a pristine container after you finish.
- It is fine to leave changes committed or uncommitted; the benchmark captures the diff from the base snapshot.

DeepSWE instruction:

${task.instruction}
`
}

function taskAgentInstruction(task) {
  if (!minimalPromptConfig) return task.instruction
  const taskPromptConfig = minimalPromptConfig.tasks[task.id]
  assert(taskPromptConfig, `missing minimal prompt entry for DeepSWE task: ${task.id}`)
  const description = String(taskPromptConfig.task_goal || taskPromptConfig.bug_description || taskPromptConfig.description || "").trim()
  assert(description, `missing minimal task_goal for DeepSWE task: ${task.id}`)
  const rules = minimalPromptConfig.common_rules.length > 0
    ? minimalPromptConfig.common_rules
    : [
      "Do not search the internet.",
      "Do not inspect or rely on hidden tests, verifier files, or reference solutions.",
      "Fix the described behavior in the repository code and avoid unrelated refactors.",
      "Run relevant local checks when available.",
    ]
  return [
    "Rules:",
    ...rules.map((rule) => `- ${rule}`),
    "",
    "Task goal:",
    description,
    "",
  ].join("\n")
}

function redactedTask(task) {
  return {
    id: task.id,
    label: task.label,
    metadata: task.metadata,
    instruction_path: path.join(task.taskDir, "instruction.md"),
    prompt_variant: minimalPromptConfig ? "minimal" : "upstream",
    minimal_prompt_config_path: minimalPromptConfig?.config_path || null,
    tests_dir: task.testsDir,
  }
}

function collectPatch(workspace, agentDir, baseSnapshot) {
  const patchPath = path.join(agentDir, "model.patch")
  const statusPath = path.join(agentDir, "git-status.txt")
  const diff = run("git", ["diff", "--binary", baseSnapshot, "--", ".", ":(exclude).deepswe"], {
    cwd: workspace,
    timeoutMs: 120_000,
  })
  const status = run("git", ["status", "--short"], { cwd: workspace, timeoutMs: 120_000 })
  const patchText = normalizeUnifiedPatchLineEndings(diff.stdout || "")
  writeFile(patchPath, patchText)
  writeFile(statusPath, status.stdout || "")
  return {
    patch_path: patchPath,
    patch_bytes: Buffer.byteLength(patchText, "utf8"),
    changed_files: countChangedFiles(patchText),
    git_status_path: statusPath,
    git_status: status.stdout || "",
    diff_status: diff.status,
    diff_error: diff.error || (diff.status === 0 ? null : diff.stderr),
  }
}

function normalizeUnifiedPatchLineEndings(patchText) {
  return String(patchText || "").replace(/\r\n/g, "\n").replace(/\r/g, "\n")
}

function countChangedFiles(patchText) {
  const files = new Set()
  for (const line of String(patchText || "").split(/\r?\n/)) {
    const match = /^diff --git a\/(.+?) b\/(.+)$/.exec(line)
    if (match) files.add(match[2])
  }
  return files.size
}

function dockerServerVersion() {
  const info = run("docker", ["info", "--format", "{{.ServerVersion}}"], { timeoutMs: 30_000 })
  return info.status === 0 && info.stdout.trim() ? info.stdout.trim() : ""
}

function dockerAvailable() {
  return Boolean(dockerServerVersion())
}

function ensureDockerDaemon() {
  const current = dockerServerVersion()
  if (current) return { ready: true, server_version: current, started: false }
  if (!autoStartDocker || process.platform !== "win32") return { ready: false, server_version: "", started: false }
  const dockerDesktop = "C:\\Program Files\\Docker\\Docker\\Docker Desktop.exe"
  if (!fs.existsSync(dockerDesktop)) return { ready: false, server_version: "", started: false, error: `missing ${dockerDesktop}` }
  try {
    spawnSync("powershell.exe", [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-Command",
      `Start-Process -FilePath ${JSON.stringify(dockerDesktop)} -WindowStyle Hidden`,
    ], { windowsHide: true, timeout: 30_000 })
  } catch (err) {
    return { ready: false, server_version: "", started: false, error: String(err?.message || err) }
  }
  const startedAt = performance.now()
  while (performance.now() - startedAt < dockerReadyTimeoutMs) {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 5_000)
    const version = dockerServerVersion()
    if (version) return { ready: true, server_version: version, started: true }
  }
  return { ready: false, server_version: "", started: true, error: `Docker did not become ready within ${dockerReadyTimeoutMs}ms` }
}

function verifierImageTag(task, contextDir) {
  const hash = crypto.createHash("sha1")
  hash.update(`${task.id}\0${task.metadata.ext_id}\0${verifierContextVersion}\0`)
  for (const file of walkFiles(contextDir)) {
    const rel = path.relative(contextDir, file).replace(/\\/g, "/")
    hash.update(rel)
    hash.update("\0")
    hash.update(fs.readFileSync(file))
    hash.update("\0")
  }
  const digest = hash.digest("hex")
    .slice(0, 12)
  return `tura-deepswe-${safeName(task.id).toLowerCase()}:${digest}`
}

function ensureVerifierImage(task) {
  if (!runEval) return null
  const docker = ensureDockerDaemon()
  writeJson(path.join(runRoot, `docker-status-${safeName(task.id)}.json`), docker)
  if (!docker.ready) throw new Error(`Docker is not available; cannot run DeepSWE verifier${docker.error ? `: ${docker.error}` : ""}`)
  const contextDir = prepareVerifierBuildContext(task)
  const tag = verifierImageTag(task, contextDir)
  const inspect = run("docker", ["image", "inspect", tag], { timeoutMs: 30_000 })
  if (inspect.status === 0) return tag
  const build = run("docker", ["build", "-t", tag, contextDir], {
    timeoutMs: Number(process.env.COMMAND_RUN_AGENT_DEEPSWE_DOCKER_BUILD_TIMEOUT_MS || 45 * 60_000),
  })
  writeFile(path.join(runRoot, `verifier-image-build-${safeName(task.id)}.stdout.log`), build.stdout)
  writeFile(path.join(runRoot, `verifier-image-build-${safeName(task.id)}.stderr.log`), build.stderr)
  if (build.status !== 0) {
    throw new Error(`failed to build DeepSWE verifier image ${tag}; see verifier-image-build logs`)
  }
  return tag
}

function evaluatePatch(task, agentDir, patch, verifierImage, verifierSetupError = null) {
  const stdoutPath = path.join(agentDir, "deepswe-eval.stdout.log")
  const stderrPath = path.join(agentDir, "deepswe-eval.stderr.log")
  if (!runEval) {
    writeFile(stdoutPath, "")
    writeFile(stderrPath, "COMMAND_RUN_AGENT_RUN_EVAL is not 1\n")
    return {
      ran: false,
      reason: "COMMAND_RUN_AGENT_RUN_EVAL is not 1",
      stdout_path: stdoutPath,
      stderr_path: stderrPath,
      report: { reports: [] },
    }
  }
  if (!verifierImage) {
    const message = verifierSetupError || "DeepSWE verifier image is unavailable"
    writeFile(stdoutPath, "")
    writeFile(stderrPath, `${message}\n`)
    return {
      ran: true,
      exit_code: 1,
      infrastructure_exit_code: null,
      stdout_path: stdoutPath,
      stderr_path: stderrPath,
      error: message,
      report: {
        reports: [{
          task: task.id,
          passed: 0,
          failed: 1,
          score: 0,
          reward: 0,
          exit_code: 1,
          message,
        }],
      },
    }
  }
  const logsDir = path.join(agentDir, "deepswe-verifier")
  const artifactsDir = path.join(logsDir, "artifacts")
  const verifierDir = path.join(logsDir, "verifier")
  if (fs.existsSync(artifactsDir)) fs.rmSync(artifactsDir, { recursive: true, force: true })
  if (fs.existsSync(verifierDir)) fs.rmSync(verifierDir, { recursive: true, force: true })
  mkdirp(artifactsDir)
  mkdirp(verifierDir)
  writeFile(path.join(artifactsDir, "model.patch"), normalizeUnifiedPatchLineEndings(readText(patch.patch_path)))
  const docker = run("docker", [
    "run",
    "--rm",
    "-v",
    `${artifactsDir}:/logs/artifacts`,
    "-v",
    `${verifierDir}:/logs/verifier`,
    verifierImage,
    "bash",
    "/tests/test.sh",
  ], {
    timeoutMs: Number(process.env.COMMAND_RUN_AGENT_DEEPSWE_EVAL_TIMEOUT_MS || 40 * 60_000),
  })
  writeFile(stdoutPath, docker.stdout)
  writeFile(stderrPath, docker.stderr + (docker.error ? `\n${docker.error}\n` : ""))
  writeFile(path.join(verifierDir, "test-stdout.txt"), docker.stdout)
  const rewardPath = path.join(verifierDir, "reward.json")
  const ctrfPath = path.join(verifierDir, "ctrf.json")
  const reward = fs.existsSync(rewardPath) ? tryReadJson(rewardPath) : null
  const report = rewardToReport(task, reward, docker.status, {
    rewardPath,
    ctrfPath: fs.existsSync(ctrfPath) ? ctrfPath : null,
    verifierDir,
  })
  return {
    ran: true,
    exit_code: report.exit_code,
    infrastructure_exit_code: docker.status,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    reward_path: fs.existsSync(rewardPath) ? rewardPath : null,
    ctrf_path: fs.existsSync(ctrfPath) ? ctrfPath : null,
    verifier_dir: verifierDir,
    error: docker.error,
    report: { reports: [report] },
  }
}

function rewardToReport(task, reward, dockerStatus, paths) {
  if (!reward) {
    return {
      task: task.id,
      passed: 0,
      failed: 1,
      score: 0,
      reward: 0,
      exit_code: 1,
      message: `verifier did not produce reward.json; docker status ${dockerStatus}`,
      artifacts: paths,
    }
  }
  const f2pTotal = Number(reward.f2p_total || 0)
  const f2pPassed = Number(reward.f2p_passed || 0)
  const p2pTotal = Number(reward.p2p_total || 0)
  const p2pPassed = Number(reward.p2p_passed || 0)
  const passed = f2pPassed + p2pPassed
  const failed = Math.max(0, f2pTotal - f2pPassed) + Math.max(0, p2pTotal - p2pPassed)
  const score = Number.isFinite(Number(reward.partial)) ? Number(reward.partial) : (passed + failed > 0 ? passed / (passed + failed) : 0)
  const binaryReward = Number(reward.reward || 0)
  return {
    task: task.id,
    passed,
    failed,
    score,
    reward: binaryReward,
    f2p_total: f2pTotal,
    f2p_passed: f2pPassed,
    p2p_total: p2pTotal,
    p2p_passed: p2pPassed,
    apply_failed: Number(reward.apply_failed || 0),
    exit_code: binaryReward === 1 ? 0 : 1,
    message: `DeepSWE reward=${binaryReward} f2p=${f2pPassed}/${f2pTotal} p2p=${p2pPassed}/${p2pTotal}`,
    artifacts: paths,
  }
}

function tryReadJson(file) {
  try { return JSON.parse(readText(file)) } catch { return null }
}

async function runAgentOnTask(agentRun, task, prepared, onUpdate) {
  const agentId = agentRun.agent_id
  const started = performance.now()
  let liveResult
  let error = null
  const writeAgentStarted = () => {
    const usageInfo = usageForAgent(prepared.agentDir, "", agentId)
    const stats = {
      agent: agentRun.run_id,
      agent_id: agentId,
      agent_kind: genericAgentKind(agentId),
      agent_mode: genericAgentMode(agentId),
      model: modelForGenericAgent(agentId, { model, turaModel }),
      tura_model: genericAgentKind(agentId) === "tura" ? turaModel : null,
      reasoning,
      service_tier: serviceTier,
      priority_enabled: priorityEnabled(serviceTier),
      task: task.id,
      task_id: task.id,
      workspace: prepared.workspace,
      prep: { prompt_path: prepared.prompt_path, base_snapshot: prepared.baseSnapshot },
      phase: "agent_started",
      in_progress: true,
      elapsed_ms: Math.round(performance.now() - started),
      exit_code: null,
      signal: null,
      first_output_ms: null,
      last_progress_ms: null,
      error: null,
      stdout_path: path.join(prepared.agentDir, "stdout.jsonl"),
      stderr_path: path.join(prepared.agentDir, "stderr.log"),
      provider_log_path: path.join(prepared.agentDir, "provider-log"),
      usage: usageInfo.usage,
      usage_source: usageInfo.usage_source,
      provider_calls: usageInfo.provider_calls,
      context_archive: null,
      patch: null,
      eval: {
        ran: false,
        reason: "deferred until unified harness phase",
      },
    }
    if (!fs.existsSync(stats.stdout_path)) writeFile(stats.stdout_path, "")
    if (!fs.existsSync(stats.stderr_path)) writeFile(stats.stderr_path, "")
    writeJson(path.join(prepared.agentDir, "agent-summary.json"), stats)
    onUpdate?.(stats)
  }
  const writeProgress = (live) => {
    const usageInfo = usageForAgent(prepared.agentDir, live.stdout || "", agentId)
    const stats = {
      agent: agentRun.run_id,
      agent_id: agentId,
      agent_kind: genericAgentKind(agentId),
      agent_mode: genericAgentMode(agentId),
      model: modelForGenericAgent(agentId, { model, turaModel }),
      tura_model: genericAgentKind(agentId) === "tura" ? turaModel : null,
      reasoning,
      service_tier: serviceTier,
      priority_enabled: priorityEnabled(serviceTier),
      task: task.id,
      workspace: prepared.workspace,
      prep: { prompt_path: prepared.prompt_path, base_snapshot: prepared.baseSnapshot },
      in_progress: true,
      elapsed_ms: Math.round(performance.now() - started),
      exit_code: live.status,
      first_output_ms: live.first_output_ms,
      last_progress_ms: live.last_progress_ms,
      error: live.error || null,
      stdout_path: path.join(prepared.agentDir, "stdout.jsonl"),
      stderr_path: path.join(prepared.agentDir, "stderr.log"),
      usage: usageInfo.usage,
      usage_source: usageInfo.usage_source,
      provider_calls: usageInfo.provider_calls,
      events: eventsWithUsageRounds(eventsForAgent(live.stdout || "", agentId), usageInfo.usage),
      fixture_backend: live.fixture_backend || null,
      fixture_source_path: live.fixture_source_path || null,
    }
    writeJson(path.join(prepared.agentDir, "agent-summary.json"), stats)
    onUpdate?.(stats)
  }
  try {
    if (prepared.error) throw new Error(prepared.error)
    writeAgentStarted()
    liveResult = await runGenericAgentCli({
      agentId,
      workspace: prepared.workspace,
      agentDir: prepared.agentDir,
      prompt: prepared.prompt,
      repoRoot,
      model,
      turaModel,
      reasoning,
      serviceTier,
      timeoutMs,
      onProgress: throttle(writeProgress, 10_000),
    })
  } catch (err) {
    error = String(err?.stack || err?.message || err)
    liveResult = {
      status: null,
      signal: null,
      stdout: "",
      stderr: "",
      duration_ms: 0,
      first_output_ms: null,
      last_progress_ms: null,
      error,
      context_archive: null,
      usage_info: { usage: usageForAgent(prepared.agentDir, "", agentId).usage, usage_source: "none", provider_calls: [] },
      events: eventsWithUsageRounds(eventsForAgent("", agentId), usageForAgent(prepared.agentDir, "", agentId).usage),
    }
  }
  const patch = collectPatch(prepared.workspace, prepared.agentDir, prepared.baseSnapshot)
  const usageInfo = usageForAgent(prepared.agentDir, liveResult.stdout || "", agentId)
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
    task: task.id,
    task_id: task.id,
    workspace: prepared.workspace,
    prep: { prompt_path: prepared.prompt_path, base_snapshot: prepared.baseSnapshot },
    in_progress: false,
    elapsed_ms: Math.round(performance.now() - started),
    exit_code: liveResult.status,
    signal: liveResult.signal,
    first_output_ms: liveResult.first_output_ms,
    last_progress_ms: liveResult.last_progress_ms,
    error: error || liveResult.error || null,
    stdout_path: path.join(prepared.agentDir, "stdout.jsonl"),
    stderr_path: path.join(prepared.agentDir, "stderr.log"),
    provider_log_path: path.join(prepared.agentDir, "provider-log"),
    usage: usageInfo.usage,
    usage_source: usageInfo.usage_source,
    provider_calls: usageInfo.provider_calls,
    context_archive: liveResult.context_archive,
    events: eventsWithUsageRounds(liveResult.events || eventsForAgent(liveResult.stdout || "", agentId), usageInfo.usage),
    fixture_backend: liveResult.fixture_backend || null,
    fixture_source_path: liveResult.fixture_source_path || null,
    patch,
    eval: {
      ran: false,
      reason: "deferred until unified harness phase",
    },
  }
  writeJson(path.join(prepared.agentDir, "agent-summary.json"), summary)
  onUpdate?.(summary)
  return summary
}

function prepareProgressResult(task, agentRun, prepared, index, phase) {
  const agentId = agentRun.agent_id
  const usageInfo = usageForAgent(prepared.agentDir, "", agentId)
  const stats = {
    agent: agentRun.run_id,
    agent_id: agentId,
    agent_kind: genericAgentKind(agentId),
    agent_mode: genericAgentMode(agentId),
    model: modelForGenericAgent(agentId, { model, turaModel }),
    tura_model: genericAgentKind(agentId) === "tura" ? turaModel : null,
    reasoning,
    service_tier: serviceTier,
    priority_enabled: priorityEnabled(serviceTier),
    task: task.id,
    task_id: task.id,
    workspace: prepared.workspace,
    prep: {
      prompt_path: prepared.prompt_path,
      base_snapshot: prepared.baseSnapshot,
      prepared_index: index,
    },
    phase,
    in_progress: phase !== "prepare_failed",
    elapsed_ms: 0,
    exit_code: null,
    signal: null,
    first_output_ms: null,
    last_progress_ms: null,
    error: prepared.error || null,
    stdout_path: path.join(prepared.agentDir, "stdout.jsonl"),
    stderr_path: path.join(prepared.agentDir, "stderr.log"),
    provider_log_path: path.join(prepared.agentDir, "provider-log"),
    usage: usageInfo.usage,
    usage_source: usageInfo.usage_source,
    provider_calls: usageInfo.provider_calls,
    context_archive: null,
    events: eventsWithUsageRounds(eventsForAgent("", agentId), usageInfo.usage),
    patch: null,
    eval: {
      ran: false,
      reason: "deferred until unified harness phase",
    },
  }
  writeJson(path.join(prepared.agentDir, "agent-summary.json"), stats)
  return stats
}

function throttle(fn, intervalMs) {
  let last = 0
  return (value) => {
    const now = performance.now()
    if (now - last < intervalMs) return
    last = now
    fn(value)
  }
}

function selectedTaskSummary(tasks) {
  const taskList = Array.isArray(tasks) ? tasks : [tasks]
  if (taskList.length === 1) return redactedTask(taskList[0])
  return {
    id: "deep-swe-matrix",
    label: `${taskList.length} DeepSWE tasks`,
    tasks: taskList.map(redactedTask),
  }
}

function buildSummary(tasks, results, inProgress, extras = {}) {
  const taskList = Array.isArray(tasks) ? tasks : [tasks]
  const flatResults = flattenResults(results)
  const elapsedMs = maxResultElapsedMs(flatResults)
  return normalizeBusinessSummary({
    ok: !inProgress && flatResults.every(resultPassed),
    in_progress: inProgress,
    suite: "deep-swe",
    upstream_repository: "https://github.com/datacurve-ai/deep-swe",
    deep_swe_root: deepSweRoot,
    selected_task: selectedTaskSummary(taskList),
    selected_tasks: taskList.map(redactedTask),
    task_ids: taskList.map((task) => task.id),
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    priority_enabled: priorityEnabled(serviceTier),
    timeout_ms: timeoutMs,
    contract_only: contractOnly,
    workspace_prep_concurrency: workspacePrepConcurrency,
    task_concurrency: taskConcurrency,
    agent_concurrency_per_task: agentConcurrencyPerTask,
    schedule_mode: scheduleMode,
    route_concurrency: routeConcurrency,
    route_start_stagger_ms: routeStartStaggerMs,
    agents,
    agent_runs: agentRuns,
    harness_directory: runRoot,
    elapsed_ms: elapsedMs,
    duration_ms: elapsedMs,
    aggregate_usage: aggregateGenericUsage(flatResults),
    results: flatResults,
    ...extras,
  }, runPaths)
}

function resultAgentId(result) {
  return result?.agent_id || result?.agent
}

function resultTaskId(result) {
  return result?.task_id || result?.task
}

function compactAgentState(result) {
  if (!result) return "pending"
  const rounds = Number(result.events?.llm_rounds || 0)
  const usageEvents = Number(result.usage?.usage_events || 0)
  if (result.in_progress) {
    const phase = result.phase || "agent"
    if (phase === "prepared" || phase === "prepare_failed") return phase
    return `running r${rounds} u${usageEvents}`
  }
  if (result.error || Number(result.exit_code) !== 0) return `failed r${rounds} u${usageEvents}`
  if (!result.patch || Number(result.patch.patch_bytes || 0) <= 0) return `no_patch r${rounds} u${usageEvents}`
  return `done r${rounds} u${usageEvents}`
}

function taskProgressRows(summary) {
  const results = Array.isArray(summary?.results) ? summary.results : []
  const byKey = new Map()
  for (const result of results) byKey.set(`${resultTaskId(result)}:${resultAgentId(result)}`, result)
  return (summary?.task_ids || []).map((taskId) => {
    const agentsForTask = Object.fromEntries(agents.map((agentId) => {
      const result = byKey.get(`${taskId}:${agentId}`)
      return [agentId, compactAgentState(result)]
    }))
    const taskResults = agents.map((agentId) => byKey.get(`${taskId}:${agentId}`)).filter(Boolean)
    const completed = taskResults.filter((result) => {
      return !result.in_progress
        && !result.error
        && Number(result.exit_code) === 0
        && Number(result.patch?.patch_bytes || 0) > 0
        && Number(result.usage?.usage_events || 0) > 0
        && Number(result.events?.llm_rounds || 0) > 0
    }).length
    const failed = taskResults.filter((result) => {
      return !result.in_progress
        && (result.error || Number(result.exit_code) !== 0 || Number(result.patch?.patch_bytes || 0) <= 0)
    }).length
    const active = taskResults.filter((result) => result.in_progress).length
    const status = failed > 0
      ? "failed"
      : completed === agents.length
        ? "done"
        : active > 0
          ? "active"
          : completed > 0
            ? "partial"
            : "pending"
    return {
      task_id: taskId,
      status,
      location: "current_run",
      completed_agents: completed,
      active_agents: active,
      failed_agents: failed,
      agents: agentsForTask,
    }
  })
}

function progressCounts(rows) {
  return {
    total_tasks: rows.length,
    done_archived: rows.filter((row) => row.status === "done_archived").length,
    done: rows.filter((row) => row.status === "done").length,
    active: rows.filter((row) => row.status === "active").length,
    partial: rows.filter((row) => row.status === "partial").length,
    failed: rows.filter((row) => row.status === "failed").length,
    pending: rows.filter((row) => row.status === "pending").length,
  }
}

function archivedProgressRows(worksetRoot, displayAgents = agents) {
  const completedArchive = process.env.COMMAND_RUN_AGENT_COMPLETED_ARCHIVE || path.join(worksetRoot, "completed")
  const completedIds =
    readJsonIfExists(path.join(worksetRoot, "completed-task-ids.json"))
    || readJsonIfExists(path.join(worksetRoot, "manifest.json"))?.completed
    || []
  return completedIds.map((taskId) => ({
    task_id: taskId,
    status: "done_archived",
    location: completedArchive,
    completed_agents: archivedCompletedAgents.length,
    active_agents: 0,
    failed_agents: 0,
    agents: Object.fromEntries(displayAgents.map((agentId) => [
      agentId,
      archivedCompletedAgents.includes(agentId) ? "done_archived" : "pending",
    ])),
  }))
}

function worksetVisibleRows(rows, displayAgents = agents) {
  return rows.map((row) => {
    const agentsForTask = Object.fromEntries(displayAgents.map((agentId) => {
      const state = row.agents?.[agentId] || "pending"
      return [agentId, row.status === "failed" && (state.startsWith("failed") || state.startsWith("no_patch")) ? "pending" : state]
    }))
    if (row.status !== "failed") return { ...row, agents: agentsForTask }
    const active = Object.values(agentsForTask).filter((state) => String(state).startsWith("running")).length
    const completed = Object.values(agentsForTask).filter((state) => String(state).startsWith("done")).length
    return {
      ...row,
      status: active > 0 ? "active" : completed > 0 ? "partial" : "pending",
      completed_agents: completed,
      active_agents: active,
      failed_agents: 0,
      agents: agentsForTask,
    }
  })
}

function pendingWorksetRows(worksetRoot, existingTaskIds, displayAgents = agents) {
  const remainingIds = readJsonIfExists(path.join(worksetRoot, "remaining-task-ids.json")) || []
  return remainingIds
    .filter((taskId) => !existingTaskIds.has(taskId))
    .map((taskId) => ({
      task_id: taskId,
      status: "pending",
      location: path.join(deepSweRoot, "tasks"),
      completed_agents: 0,
      active_agents: 0,
      failed_agents: 0,
      agents: Object.fromEntries(displayAgents.map((agentId) => [agentId, "pending"])),
    }))
}

function mergeWorksetRows(archivedRows, visibleRows, displayAgents = agents) {
  const byTask = new Map()
  for (const row of archivedRows) byTask.set(row.task_id, row)
  for (const row of visibleRows) {
    const archived = byTask.get(row.task_id)
    if (!archived) {
      byTask.set(row.task_id, row)
      continue
    }
    const mergedAgents = Object.fromEntries(displayAgents.map((agentId) => {
      const visibleState = row.agents?.[agentId]
      const archivedState = archived.agents?.[agentId]
      return [agentId, visibleState && visibleState !== "pending" ? visibleState : archivedState || "pending"]
    }))
    const active = Object.values(mergedAgents).filter((state) => String(state).startsWith("running") || state === "prepared").length
    const completed = Object.values(mergedAgents).filter((state) => String(state).startsWith("done")).length
    byTask.set(row.task_id, {
      ...row,
      status: active > 0 ? "active" : archived.status,
      location: row.location || archived.location,
      completed_agents: completed,
      active_agents: active,
      failed_agents: 0,
      agents: mergedAgents,
    })
  }
  return [...byTask.values()]
}

function progressMarkdown(progress) {
  const rows = progress.rows || []
  const counts = progress.counts || progressCounts(rows)
  const displayAgents = Array.isArray(progress.agents) && progress.agents.length > 0 ? progress.agents : agents
  const header = ["task", "status", "done", "active", "failed", "location", ...displayAgents].join(" | ")
  const sep = ["---", "---", "---:", "---:", "---:", "---", ...displayAgents.map(() => "---")].join(" | ")
  const lines = [
    `# DeepSWE task progress`,
    ``,
    `updated_at: ${progress.updated_at}`,
    `run_root: ${runRoot}`,
    `tasks_dir: ${path.join(deepSweRoot, "tasks")}`,
    `completed_archive: ${process.env.COMMAND_RUN_AGENT_COMPLETED_ARCHIVE || ""}`,
    `schedule_mode: ${scheduleMode}`,
    `route_concurrency: ${routeConcurrency}`,
    `route_start_stagger_ms: ${routeStartStaggerMs}`,
    `counts: total=${counts.total_tasks} done_archived=${counts.done_archived || 0} done=${counts.done} active=${counts.active} partial=${counts.partial} failed=${counts.failed} pending=${counts.pending}`,
    ``,
    `| ${header} |`,
    `| ${sep} |`,
    ...rows.map((row) => {
      const values = [
        row.task_id,
        row.status,
        row.completed_agents,
        row.active_agents,
        row.failed_agents,
        row.location || "",
        ...displayAgents.map((agentId) => row.agents[agentId] || "pending"),
      ]
      return `| ${values.map((value) => String(value).replace(/\|/g, "\\|")).join(" | ")} |`
    }),
    ``,
  ]
  return lines.join("\n")
}

function writeTaskProgress(summary) {
  const rows = taskProgressRows(summary)
  const updatedAt = new Date().toISOString()
  const progress = {
    schema: "tura.deepswe.task-progress.v1",
    updated_at: updatedAt,
    run_id: runId,
    run_root: runRoot,
    tasks_dir: path.join(deepSweRoot, "tasks"),
    completed_archive: process.env.COMMAND_RUN_AGENT_COMPLETED_ARCHIVE || null,
    schedule_mode: scheduleMode,
    route_concurrency: routeConcurrency,
    task_concurrency: taskConcurrency,
    agent_concurrency_per_task: agentConcurrencyPerTask,
    agents,
    counts: progressCounts(rows),
    rows,
  }
  writeJson(path.join(runRoot, "task-progress.json"), progress)
  const worksetRoot = process.env.COMMAND_RUN_AGENT_WORKSET_ROOT
  writeFile(path.join(runRoot, "task-progress.md"), progressMarkdown(progress))
  if (worksetRoot) {
    const archivedRows = archivedProgressRows(worksetRoot, worksetAgents)
    const visibleRows = worksetVisibleRows(rows, worksetAgents)
    const mergedRows = mergeWorksetRows(archivedRows, visibleRows, worksetAgents)
    const existingTaskIds = new Set(mergedRows.map((row) => row.task_id))
    const worksetRows = [
      ...mergedRows,
      ...pendingWorksetRows(worksetRoot, existingTaskIds, worksetAgents),
    ]
    const worksetProgress = {
      ...progress,
      schema: "tura.deepswe.workset-progress.v1",
      workset_root: worksetRoot,
      agents: worksetAgents,
      counts: progressCounts(worksetRows),
      rows: worksetRows,
    }
    writeJson(path.join(worksetRoot, "task-status.json"), worksetProgress)
    writeFile(path.join(worksetRoot, "task-status.md"), progressMarkdown(worksetProgress))
  }
}

function writeSummary(summary) {
  writeJson(summaryPath, summary)
  writeTaskProgress(summary)
  return summary
}

function clearContractRounds() {
  const roundsDir = path.join(runRoot, "contracts", "rounds")
  if (fs.existsSync(roundsDir)) fs.rmSync(roundsDir, { recursive: true, force: true })
}

function resultPassed(result) {
  if (!result || result.in_progress || result.error) return false
  if (Number(result.exit_code) !== 0) return false
  if (contractOnly) {
    return Number(result.usage?.usage_events || 0) > 0 && Number(result.events?.llm_rounds || 0) > 0
  }
  if (!result.patch || Number(result.patch.patch_bytes || 0) <= 0) return false
  if (!result.eval?.ran) return true
  const reports = Array.isArray(result.eval?.report?.reports) ? result.eval.report.reports : []
  const failed = reports.reduce((total, report) => total + Number(report?.failed || 0), 0)
  return Number(result.eval.exit_code) === 0 && failed === 0
}

function taskForResult(taskById, result) {
  const taskId = result?.task_id || result?.task
  return taskById.get(taskId) || [...taskById.values()][0]
}

function verifierErrorPath(task) {
  return path.join(runRoot, `verifier-setup-error-${safeName(task.id)}.log`)
}

async function prepareVerifierImages(tasks) {
  const images = new Map()
  await mapWithConcurrency(tasks, harnessTaskConcurrency, async (task) => {
    let verifierImage = null
    let verifierSetupError = null
    if (runEval) {
      try {
        verifierImage = ensureVerifierImage(task)
      } catch (err) {
        verifierSetupError = String(err?.stack || err?.message || err)
        writeFile(verifierErrorPath(task), verifierSetupError)
      }
    }
    images.set(task.id, {
      verifier_image: verifierImage,
      verifier_setup_error: verifierSetupError,
    })
  })
  return images
}

async function evaluateAllPatches(tasks, results) {
  const taskById = new Map(tasks.map((task) => [task.id, task]))
  const verifierImages = await prepareVerifierImages(tasks)
  const evaluated = []
  const grouped = new Map()
  for (const result of results) {
    const task = taskForResult(taskById, result)
    if (!grouped.has(task.id)) grouped.set(task.id, { task, results: [] })
    grouped.get(task.id).results.push(result)
  }
  await mapWithConcurrency([...grouped.values()], harnessTaskConcurrency, async (group) => {
    const task = group.task
    const verifier = verifierImages.get(task.id) || {}
    for (const result of group.results) {
      const patch = result.patch || {}
      const agentDir = path.dirname(patch.patch_path || result.stdout_path || runRoot)
      const evalResult = evaluatePatch(task, agentDir, patch, verifier.verifier_image, verifier.verifier_setup_error)
      const updated = {
        ...result,
        in_progress: false,
        eval: evalResult,
      }
      writeJson(path.join(agentDir, "agent-summary.json"), updated)
      evaluated.push(updated)
    }
  })
  return {
    results: evaluated,
    verifier_images: Object.fromEntries([...verifierImages.entries()]),
  }
}

async function runSelfTest(tasks) {
  const task = tasks[0]
  if (tasks.length === 1 && task.id === "anko-default-function-arguments") {
    assert(task.instruction.includes("default argument"), "expected selected DeepSWE instruction text")
  }
  assert(fs.existsSync(path.join(task.testsDir, "Dockerfile")), "expected verifier Dockerfile")
  assert(fs.existsSync(path.join(task.testsDir, "test.sh")), "expected verifier test.sh")
  assert(agents.includes("tura-balanced"), "default agents should include tura-balanced")
  assert(agents.includes("tura-direct"), "default agents should include tura-direct")
  assert(agents.includes("codex-main"), "default agents should include codex-main")
  return { ok: true, self_test: true, tasks: tasks.map(redactedTask), agents }
}

async function main() {
  mkdirp(runRoot)
  ensureDeepSweRepo()
  const tasks = selectedTaskIds.map(loadDeepSweTask)
  if (selfTest) {
    const summary = normalizeBusinessSummary(await runSelfTest(tasks), runPaths)
    writeSummary(summary)
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  if (evalOnly) {
    await runEvalOnly(tasks)
    return
  }
  ensureGenericAgentExecutables(agents, { repoRoot })
  const matrix = buildMatrix(tasks, agentRuns)
  writeJson(path.join(runRoot, "plan.json"), {
    run_id: runId,
    run_root: runRoot,
    deep_swe_root: deepSweRoot,
    requested_task_ids: requestedTaskIds,
    tasks: tasks.map(redactedTask),
    task_ids: tasks.map((task) => task.id),
    matrix: matrix.map((job, index) => ({
      task_id: job.task.id,
      agent: job.agentRun.agent_id,
      agent_run: job.agentRun.run_id,
      prepared_index: index,
    })),
    run_eval: runEval,
    contract_only: contractOnly,
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    priority_enabled: priorityEnabled(serviceTier),
    timeout_ms: timeoutMs,
    workspace_prep_concurrency: workspacePrepConcurrency,
    task_concurrency: taskConcurrency,
    agent_concurrency_per_task: agentConcurrencyPerTask,
    schedule_mode: scheduleMode,
    route_concurrency: routeConcurrency,
    route_start_stagger_ms: routeStartStaggerMs,
    max_task_agent_jobs: Math.max(1, taskConcurrency) * Math.max(1, agentConcurrencyPerTask),
    agents,
    agent_runs: agentRuns,
  })
  const partial = new Map()
  let finalWritten = false
  writeSummary(buildSummary(tasks, [], true, {
    harness_phase: "prepare",
    harness_pending: true,
  }))
  const writeProgressSummary = (stats) => {
    if (finalWritten) return
    partial.set(`${stats.task}:${stats.agent}`, stats)
    const results = [...partial.values()].sort((a, b) => `${a.task}:${a.agent}`.localeCompare(`${b.task}:${b.agent}`))
    const hasAgentOutput = results.some((result) => result.phase !== "prepared" && result.phase !== "prepare_failed")
    const phase = hasAgentOutput ? "agent" : "prepare"
    writeSummary(buildSummary(tasks, results, true, { harness_phase: phase }))
  }
  const prepareJob = async (job, index) => {
    try {
      const preparedWorkspace = prepareWorkspace(job.task, job.agentRun, index)
      writeProgressSummary(prepareProgressResult(job.task, job.agentRun, preparedWorkspace, index, "prepared"))
      return preparedWorkspace
    } catch (err) {
      const agentDir = path.join(runRoot, safeName(job.task.id), `${job.agentRun.run_id}-${index + 1}`)
      const workspace = path.join(agentDir, "workspace")
      mkdirp(agentDir)
      const error = String(err?.stack || err?.message || err)
      writeFile(path.join(agentDir, "prepare-error.log"), error)
      const prompt = taskPrompt(job.task)
      writeJson(path.join(agentDir, "task.json"), {
        task: redactedTask(job.task),
        agent: job.agentRun.agent_id,
        agent_run: job.agentRun.run_id,
        workspace,
        prepare_error: error,
      })
      writeFile(path.join(agentDir, "prompt.md"), prompt)
      const preparedWorkspace = {
        agentDir,
        workspace,
        prompt,
        prompt_path: path.join(agentDir, "prompt.md"),
        baseSnapshot: "HEAD",
        error,
      }
      writeProgressSummary(prepareProgressResult(job.task, job.agentRun, preparedWorkspace, index, "prepare_failed"))
      return preparedWorkspace
    }
  }
  let agentResults
  if (scheduleMode === "job") {
    agentResults = await mapWithConcurrency(matrix, routeConcurrency, async (job, index) => {
      const prepared = await prepareJob(job, index)
      if (routeStartStaggerMs > 0) {
        const staggerMs = (index % Math.max(1, routeConcurrency)) * routeStartStaggerMs
        if (staggerMs > 0) await sleep(staggerMs)
      }
      console.log(`[deepswe-debug] running ${job.agentRun.run_id} on ${job.task.id} for ${Math.round(timeoutMs / 1000)}s`)
      return runAgentOnTask(job.agentRun, job.task, prepared, writeProgressSummary)
    })
  } else {
    const taskResultGroups = await mapWithConcurrency(tasks, taskConcurrency, async (task) => {
      const jobs = agentRuns.map((agentRun) => ({ task, agentRun }))
      const prepared = await mapWithConcurrency(jobs, workspacePrepConcurrency, prepareJob)
      return mapWithConcurrency(jobs, agentConcurrencyPerTask, async (job, index) => {
        console.log(`[deepswe-debug] running ${job.agentRun.run_id} on ${job.task.id} for ${Math.round(timeoutMs / 1000)}s`)
        return runAgentOnTask(job.agentRun, job.task, prepared[index], writeProgressSummary)
      })
    })
    agentResults = taskResultGroups.flat()
  }
  if (!runEval) {
    finalWritten = true
    clearContractRounds()
    const summary = buildSummary(tasks, agentResults.map((result) => ({
      ...result,
      in_progress: false,
    })), false, {
      harness_phase: "agent",
      harness_pending: true,
    })
    writeSummary(summary)
    console.log(JSON.stringify(summary, null, 2))
    if (!summary.ok && !allowFailure) process.exitCode = 1
    return
  }
  writeSummary(buildSummary(tasks, agentResults, true, { harness_phase: "eval" }))
  const evaluation = await evaluateAllPatches(tasks, agentResults)
  finalWritten = true
  clearContractRounds()
  const summary = buildSummary(tasks, evaluation.results, false, {
    harness_phase: "complete",
    verifier_images: evaluation.verifier_images,
  })
  writeSummary(summary)
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && !allowFailure) process.exitCode = 1
}

async function runEvalOnly(tasks) {
  assert(fs.existsSync(summaryPath), `COMMAND_RUN_AGENT_EVAL_ONLY=1 requires existing summary: ${summaryPath}`)
  const existingSummary = JSON.parse(readText(summaryPath))
  const taskById = new Map(tasks.map((task) => [task.id, task]))
  const sourceResults = evalCompletedOnly
    ? (existingSummary.results || []).filter((result) => {
        if (result.in_progress) return false
        if (Number(result.exit_code) !== 0) return false
        if (evalPendingOnly) {
          const patchForDir = result.patch || {}
          const resultAgentDir = path.dirname(patchForDir.patch_path || result.stdout_path || runRoot)
          const agentSummary = readJsonIfExists(path.join(resultAgentDir, "agent-summary.json"))
          if (agentSummary?.eval?.ran) return false
        }
        const patch = result.patch || {}
        return Boolean(patch.patch_path) && Number(patch.patch_bytes || 0) > 0
      })
    : (existingSummary.results || [])
  const verifierImages = await prepareVerifierImages(tasks)
  const grouped = new Map()
  for (const result of sourceResults) {
    const task = taskForResult(taskById, result)
    if (!grouped.has(task.id)) grouped.set(task.id, { task, results: [] })
    grouped.get(task.id).results.push(result)
  }
  const results = []
  await mapWithConcurrency([...grouped.values()], harnessTaskConcurrency, async (group) => {
    const task = group.task
    const verifier = verifierImages.get(task.id) || {}
    for (const result of group.results) {
      const patch = result.patch || {}
      const agentDir = path.dirname(patch.patch_path || result.stdout_path || runRoot)
      const evalResult = evaluatePatch(task, agentDir, patch, verifier.verifier_image, verifier.verifier_setup_error)
      const agentId = result.agent_id || result.agent
      const stdoutPath = result.stdout_path || path.join(agentDir, "stdout.jsonl")
      const stdout = fs.existsSync(stdoutPath) ? readText(stdoutPath) : ""
      const usageInfo = usageForAgent(agentDir, stdout, agentId)
      const events = eventsWithUsageRounds(eventsForAgent(stdout, agentId), usageInfo.usage)
      const updated = {
        ...result,
        in_progress: false,
        usage: usageInfo.usage,
        usage_source: usageInfo.usage_source,
        provider_calls: usageInfo.provider_calls,
        events,
        eval: evalResult,
      }
      writeJson(path.join(agentDir, "agent-summary.json"), updated)
      results.push(updated)
    }
  })
  const finalResults = evalCompletedOnly
    ? mergeUpdatedResults(existingSummary.results || [], results)
    : results
  const finalInProgress = evalCompletedOnly && finalResults.some((result) => result.in_progress)
  clearContractRounds()
  const summary = buildSummary(tasks, finalResults, finalInProgress, {
    verifier_images: Object.fromEntries([...verifierImages.entries()]),
    eval_only: true,
    eval_completed_only: evalCompletedOnly,
    eval_pending_only: evalPendingOnly,
    harness_task_concurrency: harnessTaskConcurrency,
    harness_phase: finalInProgress ? "eval-partial" : "complete",
  })
  writeSummary(summary)
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && !allowFailure) process.exitCode = 1
}

function resultKey(result) {
  return `${resultTaskId(result)}:${resultAgentId(result)}`
}

function mergeUpdatedResults(existingResults, updatedResults) {
  const updates = new Map(updatedResults.map((result) => [resultKey(result), result]))
  const merged = []
  const seen = new Set()
  for (const result of existingResults) {
    const key = resultKey(result)
    if (updates.has(key)) {
      merged.push(updates.get(key))
      seen.add(key)
    } else {
      merged.push(result)
    }
  }
  for (const result of updatedResults) {
    const key = resultKey(result)
    if (!seen.has(key)) merged.push(result)
  }
  return merged
}

await main()
