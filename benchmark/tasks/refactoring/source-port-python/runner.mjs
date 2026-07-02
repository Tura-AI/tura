#!/usr/bin/env node
import assert from "node:assert/strict"
import crypto from "node:crypto"
import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import process from "node:process"
import { spawn, spawnSync } from "node:child_process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { agentEventStats, agentUsageFromJsonl, claudeCodeArgs, findClaudeExe, findPiExe, piAgentArgs } from "../../../lib/agent_cli.mjs"
import { businessRunPaths, normalizeBusinessSummary } from "../../../lib/business_paths.mjs"
import { endStream, isolatedProcessOptions, killProcessTree } from "../../../lib/process_helpers.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `source-port-suite-${Date.now()}`
const baseRunPaths = businessRunPaths("project-rebuild-source-port", runId)
const runPaths = businessRunPaths("project-rebuild-source-port", runId, {
  targetRoot: baseRunPaths.target_root,
  runRoot: path.join(baseRunPaths.target_root, baseRunPaths.test_name, shortRunDirName(runId)),
})
const suiteRoot =
  process.env.SOURCE_PORT_SUITE_ROOT ||
  process.env.COMMAND_RUN_AGENT_SOURCE_PORT_ROOT ||
  path.join(runPaths.target_root, runPaths.test_name, "_cache")
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "medium"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 60 * 60_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "tura-planning-shll")
const printProviderLog = truthy(process.env.COMMAND_RUN_AGENT_PRINT_PROVIDER_LOG || "0")
const selectedTasksRaw = process.env.SOURCE_PORT_TASKS || process.env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS || "all"
const prepOnly = truthy(process.env.COMMAND_RUN_AGENT_PREP_ONLY || "0")
const selfTest = truthy(process.env.SOURCE_PORT_SELF_TEST || process.env.COMMAND_RUN_AGENT_SOURCE_PORT_SELF_TEST || "0")
const binaryOnly = truthy(process.env.SOURCE_PORT_BINARY_ONLY || "0")
const runEval = truthy(process.env.SOURCE_PORT_RUN_EVAL || process.env.COMMAND_RUN_AGENT_SOURCE_PORT_RUN_EVAL || "1")
const complexTodoHint = defaultTruthy(process.env.SOURCE_PORT_COMPLEX_TODO_HINT || process.env.COMMAND_RUN_AGENT_SOURCE_PORT_COMPLEX_TODO_HINT)
const planningOverride = parsePlanningOverride(process.env.COMMAND_RUN_AGENT_TURA_PLANNING || "auto")
const codexGoalsEnabled = truthy(process.env.COMMAND_RUN_AGENT_CODEX_GOALS || "0")
const turaGoalEnabled = truthy(process.env.COMMAND_RUN_AGENT_TURA_GOAL || "0")
const turaExplicitSessionId = truthy(process.env.COMMAND_RUN_AGENT_TURA_SESSION_ID || "0")
const turaExe =
  process.env.COMMAND_RUN_AGENT_TURA_EXE ||
  path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const codexMainExe = findCodexMainExe()
const codexDocumentsExe = findCodexDocumentsExe()
const claudeExe = findClaudeExe()
const piExe = findPiExe()

function shortRunDirName(value) {
  return `r-${crypto.createHash("sha1").update(String(value)).digest("hex").slice(0, 10)}`
}

const TASKS = {
  "zip-password-finder": {
    id: "agourlay__zip-password-finder.source-port-python",
    label: "zip-password-finder",
    repo: "https://github.com/agourlay/zip-password-finder.git",
    owner: "agourlay",
    repoName: "zip-password-finder",
    tag: "v0.11.1",
    commit: "7c1a4c93841220fc740ed81d3b97784e450fc6a6",
    binaryNames: ["zip-password-finder"],
    releaseAssetRules: [
      { os: "win32", arch: "x64", includes: ["x86_64-pc-windows-msvc", ".zip"] },
      { os: "win32", arch: "arm64", includes: ["aarch64-pc-windows-msvc", ".zip"] },
      { os: "linux", arch: "x64", includes: ["x86_64-unknown-linux-gnu", ".tar.gz"] },
      { os: "linux", arch: "arm64", includes: ["aarch64-unknown-linux-gnu", ".tar.gz"] },
      { os: "darwin", arch: "x64", includes: ["x86_64-apple-darwin", ".tar.gz"] },
      { os: "darwin", arch: "arm64", includes: ["aarch64-apple-darwin", ".tar.gz"] },
    ],
    commands: ["single command CLI", "argument validation", "dictionary/bruteforce ZIP password search"],
  },
  xsv: {
    id: "burntsushi__xsv.source-port-python-complete",
    label: "xsv",
    repo: "https://github.com/BurntSushi/xsv.git",
    owner: "BurntSushi",
    repoName: "xsv",
    tag: "0.13.0",
    commit: "2b4cbaa0eecf7b507a612632fe00289b1b358c15",
    binaryNames: ["xsv"],
    releaseAssetRules: [
      { os: "win32", arch: "x64", includes: ["x86_64-pc-windows-msvc", ".zip"] },
      { os: "win32", arch: "ia32", includes: ["i686-pc-windows-msvc", ".zip"] },
      { os: "linux", arch: "x64", includes: ["x86_64-unknown-linux-musl", ".tar.gz"] },
      { os: "darwin", arch: "x64", includes: ["x86_64-apple-darwin", ".tar.gz"] },
    ],
    commands: ["headers", "count", "select", "slice", "search", "sort", "table", "fmt", "stats", "frequency"],
  },
  eza: {
    id: "eza-community__eza.source-port-python-complete",
    label: "eza",
    repo: "https://github.com/eza-community/eza.git",
    owner: "eza-community",
    repoName: "eza",
    tag: "v0.23.3",
    commit: "05d20d11c488b2ad3f0d63ac0b529281cc1c16ef",
    binaryNames: ["eza"],
    releaseAssetRules: [
      { os: "win32", arch: "x64", includes: ["eza.exe_x86_64-pc-windows-gnu", ".zip"] },
      { os: "linux", arch: "x64", includes: ["eza_x86_64-unknown-linux-musl", ".tar.gz"] },
      { os: "linux", arch: "arm64", includes: ["eza_aarch64-unknown-linux-gnu", ".tar.gz"] },
      { os: "darwin", arch: "x64", includes: ["eza_x86_64-apple-darwin", ".tar.gz"] },
      { os: "darwin", arch: "arm64", includes: ["eza_aarch64-apple-darwin", ".tar.gz"] },
    ],
    commands: ["directory listing", "long view", "tree", "sort", "hidden", "icons/colors disabled"],
  },
  nushell: {
    id: "nushell__nushell.source-port-python-complete",
    label: "nushell",
    repo: "https://github.com/nushell/nushell.git",
    owner: "nushell",
    repoName: "nushell",
    tag: "0.106.1",
    commit: "682d593d3f53e5337dceedf98c9603a698af6a64",
    binaryNames: ["nu"],
    releaseAssetRules: [
      { os: "win32", arch: "x64", includes: ["x86_64-pc-windows-msvc", ".zip"], excludes: [".msi"] },
      { os: "win32", arch: "arm64", includes: ["aarch64-pc-windows-msvc", ".zip"], excludes: [".msi"] },
      { os: "linux", arch: "x64", includes: ["x86_64-unknown-linux-gnu", ".tar.gz"] },
      { os: "linux", arch: "arm64", includes: ["aarch64-unknown-linux-gnu", ".tar.gz"] },
      { os: "darwin", arch: "x64", includes: ["x86_64-apple-darwin", ".tar.gz"] },
      { os: "darwin", arch: "arm64", includes: ["aarch64-apple-darwin", ".tar.gz"] },
    ],
    commands: ["nu -c expressions", "tables", "json", "csv", "strings", "math", "filesystem snippets"],
  },
}

const selectedTasks = parseTasks(selectedTasksRaw)

function truthy(value) {
  return ["1", "true", "yes", "on", "enabled"].includes(String(value || "").trim().toLowerCase())
}

function defaultTruthy(value) {
  const normalized = String(value ?? "").trim().toLowerCase()
  if (!normalized) return true
  return !["0", "false", "no", "off", "disabled"].includes(normalized)
}

function findCodexMainExe() {
  const exeName = process.platform === "win32" ? "codex.exe" : "codex"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_CODEX_MAIN_ROOT,
    path.join(homeDir, "Documents", "codex-main"),
    path.join(homeDir, "codex-main"),
  ].filter(Boolean).map((root) => path.join(root, "codex-rs", "target", "debug", exeName))
  return candidates.find((candidate) => fs.existsSync(candidate)) || candidates[0]
}

function findCodexDocumentsExe() {
  const exeName = process.platform === "win32" ? "codex.exe" : "codex"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_CODEX_ROOT,
    path.join(homeDir, "Documents", "Codex"),
  ].filter(Boolean).map((root) => path.join(root, "codex-rs", "target", "debug", exeName))
  return candidates.find((candidate) => fs.existsSync(candidate)) || candidates[0]
}

function parseAgents(value) {
  const alias = new Map([
    ["tura", "tura-planning-shll"],
    ["tura-balanced", "tura-balanced"],
    ["balanced", "tura-balanced"],
    ["tura-blanced", "tura-balanced"],
    ["blanced", "tura-balanced"],
    ["tura-direct", "tura-direct"],
    ["direct", "tura-direct"],
    ["tura-thinking", "tura-thinking-shll"],
    ["tura-thinking-shll", "tura-thinking-shll"],
    ["thinking", "tura-thinking-shll"],
    ["tura-thinking-visual", "tura-thinking-visual-shll"],
    ["tura-thinking-visual-shll", "tura-thinking-visual-shll"],
    ["thinking-visual", "tura-thinking-visual-shll"],
    ["think-visual", "tura-thinking-visual-shll"],
    ["tura-planning", "tura-planning-shll"],
    ["tura-planning-shll", "tura-planning-shll"],
    ["tura-fast", "tura-fast-shll"],
    ["tura-fast-shll", "tura-fast-shll"],
    ["tura-fast-planning", "tura-fast-planning-shll"],
    ["tura-fast-planning-shll", "tura-fast-planning-shll"],
    ["codex-main", "codex-main"],
    ["main", "codex-main"],
    ["codex", "codex-documents"],
    ["codex-documents", "codex-documents"],
    ["codex-docs", "codex-documents"],
    ["codex-alt", "codex-documents"],
    ["codex-ponytail", "codex-ponytail"],
    ["ponytail", "codex-ponytail"],
    ["claude", "claude-code"],
    ["claude-code", "claude-code"],
    ["claude-opus", "claude-code"],
    ["pi", "pi-agent"],
    ["pi-agent", "pi-agent"],
    ["pi-coding-agent", "pi-agent"],
  ])
  return String(value).split(",").map((item) => alias.get(item.trim().toLowerCase())).filter(Boolean)
}

function parseTasks(value) {
  const all = Object.keys(TASKS)
  const normalized = String(value || "all").trim()
  if (!normalized || normalized === "all") return all
  return normalized.split(",").map((item) => item.trim()).filter(Boolean).map((id) => {
    if (!TASKS[id]) throw new Error(`unknown source-port task ${id}; expected one of ${all.join(", ")}`)
    return id
  })
}

function parsePlanningOverride(value) {
  const normalized = String(value || "auto").trim().toLowerCase()
  if (["auto", "default", "agent"].includes(normalized)) return null
  if (["on", "true", "1", "yes", "enabled"].includes(normalized)) return true
  if (["off", "false", "0", "no", "disabled"].includes(normalized)) return false
  throw new Error(`COMMAND_RUN_AGENT_TURA_PLANNING must be auto, on, or off; got ${value}`)
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text)
}

function writeJson(file, value) {
  writeFile(file, JSON.stringify(value, null, 2))
}

function copyDir(src, dest) {
  mkdirp(dest)
  fs.cpSync(src, dest, { recursive: true, force: true, dereference: false })
}

const MAX_WINDOWS_GIT_CONFIG_PATH_CHARS = 256

function validateWorkspaceGitPath(workspace) {
  if (process.platform !== "win32") return
  const gitConfigPath = path.join(workspace, ".git", "config")
  if (gitConfigPath.length <= MAX_WINDOWS_GIT_CONFIG_PATH_CHARS) return
  throw new Error(
    `workspace Git metadata path is too long for Windows (${gitConfigPath.length} > ${MAX_WINDOWS_GIT_CONFIG_PATH_CHARS} chars): ${gitConfigPath}. Use a shorter COMMAND_RUN_AGENT_RUN_ID or run root.`
  )
}

function taskRunDirName(task) {
  const raw = String(task.label || task.id || "task").trim()
  const safe = raw.replace(/[^A-Za-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "")
  return (safe || "task").slice(0, 48)
}

function formatGitSetupFailure(step, result) {
  return [
    `${step} failed with ${result.status}`,
    `STDOUT:\n${result.stdout}`,
    `STDERR:\n${result.stderr}`,
    `ERROR:\n${result.error || ""}`,
  ].join("\n")
}

function setupWorkspaceGit(workspace, addArgs, commitMessage) {
  const failures = []
  const steps = [
    ["git init", ["init"]],
    ["git config user.email", ["config", "user.email", "benchmark@example.invalid"]],
    ["git config user.name", ["config", "user.name", "Benchmark"]],
    [`git add ${addArgs.join(" ")}`, ["add", ...addArgs]],
    ["git commit", ["commit", "-m", commitMessage]],
  ]
  for (const [step, args] of steps) {
    const result = run("git", args, { cwd: workspace, timeoutMs: 60_000 })
    if (result.status !== 0) {
      failures.push(formatGitSetupFailure(step, result))
      break
    }
  }
  if (failures.length === 0) return { ok: true, warning_path: null, failures: [] }
  const warningPath = path.join(workspace, ".tura-benchmark-git-setup-warning.log")
  writeFile(warningPath, failures.join("\n\n"))
  console.warn(`[source-port-suite] workspace git setup failed; continuing without git checkpoint: ${warningPath}`)
  return { ok: false, warning_path: warningPath, failures }
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

function hasDispatchProgress(stdout) {
  return (
    stdout.includes('"type":"turn.completed"') ||
    stdout.includes('"type":"agent_message"') ||
    stdout.includes('"type":"command_execution"') ||
    stdout.includes('"type":"file_change"') ||
    stdout.includes('"provider_tool_call_id"')
  )
}

async function runLive(command, args, options = {}) {
  const maxAttempts = Math.max(1, Number(options.maxAttempts || 1))
  let last = null
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    last = await runLiveAttempt(command, args, { ...options, attempt, maxAttempts })
    if (!last.dispatch_stalled) return last
    if (attempt < maxAttempts) {
      options.onProgress?.({
        ...last,
        status: null,
        error: `dispatch stalled on attempt ${attempt}; retrying`,
      })
    }
  }
  return last
}

function runLiveAttempt(command, args, options = {}) {
  const started = performance.now()
  const stdoutPath = options.stdoutPath
  const stderrPath = options.stderrPath
  const statusPath = options.statusPath
  if (stdoutPath) mkdirp(path.dirname(stdoutPath))
  const stdoutStream = stdoutPath ? fs.createWriteStream(stdoutPath) : null
  const stderrStream = stderrPath ? fs.createWriteStream(stderrPath) : null
  return new Promise((resolve) => {
    let stdout = ""
    let stderr = ""
    let firstOutputMs = null
    let settled = false
    let timedOut = false
    let completionResolved = false
    let turnCompletedTimer = null
    let lastProgressMs = performance.now()
    let dispatchStalled = false
    let progressQueued = false
    const child = spawn(command, args, isolatedProcessOptions({
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    }))
    if (options.input) {
      child.stdin.write(options.input)
      child.stdin.end()
    }
    const timer = setTimeout(() => {
      timedOut = true
      killProcessTree(child.pid)
      settle(1, null, `timed out after ${options.timeoutMs || timeoutMs}ms`)
    }, options.timeoutMs || timeoutMs)
    const dispatchWatchdogMs = Number(options.dispatchWatchdogMs || 0)
    const dispatchTimer = dispatchWatchdogMs > 0
      ? setInterval(() => {
          if (settled) return
          const elapsed = performance.now() - started
          if (elapsed < dispatchWatchdogMs) return
          if (hasDispatchProgress(stdout)) return
          dispatchStalled = true
          killProcessTree(child.pid)
          settle(1, null, `dispatch stalled after ${Math.round(elapsed)}ms`)
        }, Math.min(dispatchWatchdogMs, 5_000))
      : null
    function record(kind, chunk) {
      if (settled) return
      if (firstOutputMs == null) firstOutputMs = Math.round(performance.now() - started)
      lastProgressMs = performance.now()
      const text = chunk.toString()
      if (kind === "stdout") {
        stdout += text
        stdoutStream?.write(text)
        queueProgress()
        if (!completionResolved && options.resolveOnTurnCompleted && stdout.includes("\"type\":\"turn.completed\"")) {
          completionResolved = true
          turnCompletedTimer = setTimeout(() => {
            killProcessTree(child.pid)
            settle(0, null, null)
          }, options.turnCompletedGraceMs || 1000)
        }
      } else {
        stderr += text
        stderrStream?.write(text)
        queueProgress()
      }
    }
    function queueProgress() {
      if (progressQueued) return
      progressQueued = true
      setImmediate(() => {
        progressQueued = false
        if (settled) return
        options.onProgress?.({
          status: null,
          signal: null,
          stdout,
          stderr,
          duration_ms: Math.round(performance.now() - started),
          first_output_ms: firstOutputMs,
          timed_out: false,
          dispatch_stalled: false,
          attempt: options.attempt || 1,
          max_attempts: options.maxAttempts || 1,
          error: null,
          pid: child.pid,
        })
      })
    }
    function settle(status, signal, error = null) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      if (dispatchTimer) clearInterval(dispatchTimer)
      if (turnCompletedTimer) clearTimeout(turnCompletedTimer)
      endStream(stdoutStream)
      endStream(stderrStream)
      try {
        child.stdin?.destroy()
        child.stdout?.destroy()
        child.stderr?.destroy()
        child.removeAllListeners("close")
        child.removeAllListeners("error")
        child.unref?.()
      } catch {}
      const summary = {
        status,
        signal,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        first_output_ms: firstOutputMs,
        last_progress_ms: Math.round(lastProgressMs - started),
        timed_out: timedOut,
        dispatch_stalled: dispatchStalled,
        attempt: options.attempt || 1,
        max_attempts: options.maxAttempts || 1,
        error,
        pid: child.pid,
      }
      if (statusPath) writeFile(statusPath, JSON.stringify(summary, null, 2))
      options.onProgress?.(summary)
      resolve(summary)
    }
    child.stdout?.on("data", (chunk) => record("stdout", chunk))
    child.stderr?.on("data", (chunk) => record("stderr", chunk))
    child.on("error", (error) => settle(null, null, String(error.stack || error.message || error)))
    child.on("close", (status, signal) => settle(status, signal, timedOut ? `timed out after ${options.timeoutMs || timeoutMs}ms` : null))
  })
}

function ensureReferenceRepo(task) {
  const referenceCache = path.join(suiteRoot, "reference", task.label)
  mkdirp(path.dirname(referenceCache))
  if (!fs.existsSync(path.join(referenceCache, ".git"))) {
    fs.rmSync(referenceCache, { recursive: true, force: true })
    runOk("git", ["clone", "--filter=blob:none", task.repo, referenceCache], { timeoutMs: 20 * 60_000 })
  } else {
    run("git", ["fetch", "--all", "--tags"], { cwd: referenceCache, timeoutMs: 10 * 60_000 })
  }
  runOk("git", ["checkout", "--force", task.commit], { cwd: referenceCache, timeoutMs: 120_000 })
  const rev = runOk("git", ["rev-parse", "HEAD"], { cwd: referenceCache, timeoutMs: 60_000 }).stdout.trim()
  assert.equal(rev, task.commit)
  return referenceCache
}

function platformInfo() {
  const arch = process.arch === "x64" ? "x64" : process.arch === "arm64" ? "arm64" : process.arch
  return { os: process.platform, arch }
}

async function githubRelease(task) {
  const cachePath = path.join(suiteRoot, "release-metadata", `${task.label}-${task.tag}.json`)
  if (fs.existsSync(cachePath)) return JSON.parse(fs.readFileSync(cachePath, "utf8"))
  const url = `https://api.github.com/repos/${task.owner}/${task.repoName}/releases/tags/${task.tag}`
  const response = await fetch(url, { headers: { "User-Agent": "tura-source-port-suite" } })
  if (!response.ok) throw new Error(`failed to fetch ${url}: ${response.status} ${await response.text()}`)
  const release = await response.json()
  writeFile(cachePath, JSON.stringify(release, null, 2))
  return release
}

function selectAsset(task, assets) {
  const info = platformInfo()
  const candidates = task.releaseAssetRules.filter((rule) => rule.os === info.os && (!rule.arch || rule.arch === info.arch))
  for (const rule of candidates) {
    const asset = assets.find((item) => {
      const name = item.name || ""
      return rule.includes.every((part) => name.includes(part)) && (rule.excludes || []).every((part) => !name.includes(part))
    })
    if (asset) return asset
  }
  throw new Error(`no release asset for ${task.label} on ${info.os}/${info.arch}; assets: ${assets.map((a) => a.name).join(", ")}`)
}

async function downloadFile(url, dest) {
  if (fs.existsSync(dest) && fs.statSync(dest).size > 0) return dest
  mkdirp(path.dirname(dest))
  const response = await fetch(url, { headers: { "User-Agent": "tura-source-port-suite" } })
  if (!response.ok) throw new Error(`failed to download ${url}: ${response.status} ${await response.text()}`)
  const arrayBuffer = await response.arrayBuffer()
  fs.writeFileSync(dest, Buffer.from(arrayBuffer))
  return dest
}

function cleanExtractDir(dir) {
  fs.rmSync(dir, { recursive: true, force: true })
  mkdirp(dir)
}

function extractArchive(archive, dest) {
  cleanExtractDir(dest)
  const lower = archive.toLowerCase()
  if (lower.endsWith(".zip")) {
    const ps = [
      "-NoProfile",
      "-Command",
      `Expand-Archive -LiteralPath ${JSON.stringify(archive)} -DestinationPath ${JSON.stringify(dest)} -Force`,
    ]
    runOk("powershell", ps, { timeoutMs: 5 * 60_000 })
  } else if (lower.endsWith(".tar.gz") || lower.endsWith(".tgz")) {
    runOk("tar", ["-xzf", archive, "-C", dest], { timeoutMs: 5 * 60_000 })
  } else if (lower.endsWith(".tar.xz")) {
    runOk("tar", ["-xJf", archive, "-C", dest], { timeoutMs: 5 * 60_000 })
  } else {
    throw new Error(`unsupported archive type: ${archive}`)
  }
}

function isExecutableCandidate(file, binaryNames) {
  const base = path.basename(file).toLowerCase()
  const names = binaryNames.flatMap((name) => [name.toLowerCase(), `${name.toLowerCase()}.exe`])
  return names.includes(base)
}

function findBinaryInDir(dir, binaryNames) {
  const stack = [dir]
  const candidates = []
  while (stack.length > 0) {
    const current = stack.pop()
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(full)
      else if (entry.isFile() && isExecutableCandidate(full, binaryNames)) candidates.push(full)
    }
  }
  if (candidates.length === 0) throw new Error(`could not find binary ${binaryNames.join("/")} under ${dir}`)
  candidates.sort((a, b) => a.length - b.length)
  return candidates[0]
}

async function ensureReferenceBinary(task) {
  const binName = process.platform === "win32" ? `${task.binaryNames[0]}.exe` : task.binaryNames[0]
  const stable = path.join(suiteRoot, "binaries", task.label, task.tag, binName)
  if (fs.existsSync(stable)) {
    smokeReferenceBinary(task, stable)
    return stable
  }
  const release = await githubRelease(task)
  const asset = selectAsset(task, release.assets || [])
  const archive = path.join(suiteRoot, "downloads", task.label, task.tag, asset.name)
  await downloadFile(asset.browser_download_url, archive)
  const extractDir = path.join(suiteRoot, "extract", task.label, task.tag)
  extractArchive(archive, extractDir)
  const found = findBinaryInDir(extractDir, task.binaryNames)
  mkdirp(path.dirname(stable))
  fs.copyFileSync(found, stable)
  if (process.platform !== "win32") fs.chmodSync(stable, 0o755)
  smokeReferenceBinary(task, stable)
  return stable
}

function smokeReferenceBinary(task, binary) {
  assert(fs.existsSync(binary), `missing reference binary for ${task.label}: ${binary}`)
  const result = run(binary, ["--version"], { timeoutMs: 60_000, maxBuffer: 16 * 1024 * 1024 })
  if (result.status !== 0) {
    throw new Error(`reference binary smoke failed for ${task.label}: ${binary}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}\nerror:${result.error || ""}`)
  }
  return result
}

function sourcePortPrompt(task) {
  const todoHint = complexTodoHint
    ? "\nThis is a complex task. Prefer breaking it into fine-grained TODOs and work through them systematically before marking the task done.\n"
    : ""
  return `You are in a benchmark workspace containing a Rust reference application at ./rust-reference and an official release binary path recorded in ./REFERENCE_BINARY.txt.

Goal:
Create a Python implementation that replicates the reference application's business-relevant CLI behavior needed for this benchmark. The evaluator focuses on functional outputs for representative real workflows rather than exhaustive help text or parser-copy minutiae.

Reference:
- Project: ${task.label}
- Repository: ${task.repo}
- Release/tag: ${task.tag}
- Commit: ${task.commit}
- Local source copy: ./rust-reference
- Official binary: read ./REFERENCE_BINARY.txt

Hard constraints:
- Do not use Docker.
- Do not search the internet.
- Do not look for, copy, adapt, vendor, install, or import an existing Python implementation, clone, wrapper, compatibility layer, or package for this application.
- Do not use package names, GitHub searches, package indexes, web pages, StackOverflow posts, examples, or generated snippets that implement the same tool or command family.
- Use ./rust-reference and the official binary as the only functional sources of truth.
- Implement in Python.
- Do not shell out to the official binary from your implementation.
- Do not install packages that already implement this application or its command suite.
- The root deliverable must include ./executable. The harness will run it as: python ./executable ...
- Also include ./compile.sh. It may be tiny, but it must leave ./executable present and ready to run.
- Your Python implementation must be self-contained in the workspace. Standard-library modules are allowed; external dependencies are strongly discouraged and must not be used to bypass implementing the CLI behavior.
- Do not fake tests by special-casing harness file names only. Implement the general command semantics for the requested scope.

Required benchmark scope:
- Determine the required CLI surface from the authoritative sources before planning implementation work: inspect the Rust source, command dispatcher, tests/fixtures when present, and official binary behavior.
- Do not assume the required scope is limited to an obvious subset or to the first commands you inspect. Prioritize command behaviors needed for realistic data-processing or file-processing workflows over copying static help text.
- Treat the official binary and local source as the source of truth for which commands, flags, inputs, outputs, exit codes, and error cases matter.

Equivalence requirements:
- For every required command/flag/input you identify, match the official binary's observable business behavior: success/failure status, parsed data results, selected files, ordering when semantically meaningful, filtering, transformations, and realistic error handling.
- Do not chase cosmetic formatting that does not change the business result. Column widths, decorative tree glyphs, long help prose, exact spacing, and platform-specific path spelling are lower priority than correct data and file-processing behavior.
- Do not spend disproportionate effort cloning long static help text. Match parser behavior enough to support the evaluated workflows.
- Match data behavior: ordering where meaningful, delimiters, quoting, escaping, headers, path selection, numeric/string coercion, and realistic failure cases.
- If the official binary prints nothing, your program must print nothing. If the official binary writes to stderr, your program must write to stderr, not stdout.
- The evaluator will generate expected results by invoking the official binary at runtime and then invoke your ./executable with the same inputs. It will score semantic behavior rather than require byte-for-byte formatting for display-oriented commands.
- Passing local hand-written examples is not enough. You must probe the official binary on representative business workflows and reconcile functional differences before marking the task done.
- A command metadata/help/inventory lookup does not count as testing that command's behavior. For each discovered command, the oracle checklist must include at least one executable invocation that exercises that command's functional semantics, with reference-vs-port status/stdout/stderr comparison. Metadata/help cases may be additional evidence only, never a replacement for functional behavior cases.
${todoHint}

Required workflow:
1. Inspect README/Cargo metadata and the relevant Rust source files under ./rust-reference.
2. Use the official binary from ./REFERENCE_BINARY.txt to probe representative business CLI input/output/exit-code behavior before implementing and again after implementation.
3. Implement the Python port.
4. Run local checks against the official binary behavior and fix every mismatch you find.
5. Finish by leaving ./executable and ./compile.sh in the workspace root.

Do not ask the user questions. Infer from source and official CLI behavior.`
}

function harnessTemplate() {
  return String.raw`#!/usr/bin/env python3
import csv
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


TASK = os.environ["SOURCE_PORT_TASK"]
REFERENCE_BINARY = Path(os.environ["SOURCE_PORT_REFERENCE_BINARY"])


def run_cmd(argv, cwd, stdin=None, timeout=30):
    try:
        proc = subprocess.run(
            [str(x) for x in argv],
            cwd=str(cwd),
            input=stdin,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
            timeout=timeout,
            env={**os.environ, "NO_COLOR": "1", "CLICOLOR": "0", "TERM": "dumb", "PYTHONIOENCODING": "utf-8", "PYTHONUTF8": "1"},
        )
        return {"status": proc.returncode, "stdout": proc.stdout, "stderr": proc.stderr, "timed_out": False}
    except subprocess.TimeoutExpired as exc:
        stdout = exc.stdout or ""
        stderr = exc.stderr or ""
        if isinstance(stdout, bytes):
            stdout = stdout.decode("utf-8", "replace")
        if isinstance(stderr, bytes):
            stderr = stderr.decode("utf-8", "replace")
        timeout_stderr = (stderr + "\n" if stderr else "") + f"TIMEOUT after {timeout}s: {' '.join(str(x) for x in argv)}"
        return {"status": 124, "stdout": stdout, "stderr": timeout_stderr, "timed_out": True}


def normalize(text):
    text = text or ""
    text = text.replace("\r\n", "\n")
    text = re.sub(r"\x1b\[[0-9;?]*[A-Za-z]", "", text)
    text = re.sub(r"Time elapsed: [^\n]*", "Time elapsed: <DURATION>", text)
    text = re.sub(r"\b\d+(?:\.\d+)?\s*(?:ns|us|µs|ms|s)\b", "<DURATION>", text)
    return text


def compact(text):
    return " ".join(normalize(text).split())


def split_lines(text):
    return [line.rstrip() for line in normalize(text).splitlines() if line.strip()]


def parse_csv_text(text):
    try:
        return list(csv.reader(normalize(text).splitlines()))
    except Exception:
        return None


def normalize_scalar(text):
    value = compact(text).strip()
    if re.fullmatch(r"-?\d+(?:\.\d+)?", value):
        return str(float(value)).rstrip("0").rstrip(".")
    return value.strip('"').strip("'")


def same_status(actual, expected):
    return actual["status"] == expected["status"]


def same_csv(actual, expected):
    if not same_status(actual, expected):
        return False
    if normalize(actual["stderr"]) != normalize(expected["stderr"]):
        return False
    actual_rows = parse_csv_text(actual["stdout"])
    expected_rows = parse_csv_text(expected["stdout"])
    return actual_rows is not None and actual_rows == expected_rows


def same_csv_unordered_body(actual, expected):
    if not same_status(actual, expected):
        return False
    if normalize(actual["stderr"]) != normalize(expected["stderr"]):
        return False
    actual_rows = parse_csv_text(actual["stdout"])
    expected_rows = parse_csv_text(expected["stdout"])
    if actual_rows is None or expected_rows is None:
        return False
    return actual_rows[:1] == expected_rows[:1] and sorted(actual_rows[1:]) == sorted(expected_rows[1:])


def same_sample_csv(actual, expected):
    if not same_status(actual, expected):
        return False
    if normalize(actual["stderr"]) != normalize(expected["stderr"]):
        return False
    actual_rows = parse_csv_text(actual["stdout"])
    expected_rows = parse_csv_text(expected["stdout"])
    return (
        actual_rows is not None
        and expected_rows is not None
        and len(actual_rows) == len(expected_rows)
        and (not expected_rows or actual_rows[0] == expected_rows[0])
    )


def same_normalized_streams(actual, expected):
    return (
        same_status(actual, expected)
        and normalize(actual["stdout"]) == normalize(expected["stdout"])
        and normalize(actual["stderr"]) == normalize(expected["stderr"])
    )


def same_scalar(actual, expected):
    if not same_status(actual, expected):
        return False
    if expected["status"] != 0:
        return bool(compact(actual["stderr"]) or compact(actual["stdout"]))
    expected_out = normalize_scalar(expected["stdout"])
    actual_out = normalize_scalar(actual["stdout"])
    if expected_out in {"true", "false"}:
        return actual_out.lower() == expected_out
    return actual_out == expected_out


def same_json_or_scalar(actual, expected):
    if not same_status(actual, expected):
        return False
    if expected["status"] != 0:
        return bool(compact(actual["stderr"]) or compact(actual["stdout"]))
    try:
        return json.loads(actual["stdout"]) == json.loads(expected["stdout"])
    except Exception:
        return same_scalar(actual, expected)


def known_fixture_entries(fx):
    names = {
        "Cargo.toml", "README.md", "empty.txt", "exec.sh", "long name file.txt",
        "notes.txt", "script.py", "semi.csv", "people.csv", "people2.csv",
        "no_headers.csv", "unequal.csv", "sub", "nested.txt", "data.log",
        "deep", "final.md", ".hidden", "link-notes",
    }
    return sorted(names, key=len, reverse=True)


def eza_entries(text, fx):
    clean = normalize(text).replace("\\", "/").replace("//?/", "")
    found = []
    for name in known_fixture_entries(fx):
        pattern = re.escape(name).replace("\\ ", r"\s+")
        if re.search(r"(?<![\w.-])" + pattern + r"[/@*|=]?(?![\w.-])", clean):
            found.append(name)
    return set(found)


def eza_order(text, fx):
    clean = normalize(text).replace("\\", "/").replace("//?/", "")
    positions = []
    for name in known_fixture_entries(fx):
        idx = clean.find(name)
        if idx >= 0:
            positions.append((idx, name))
    return [name for _, name in sorted(positions)]


def eza_classifiers(text, fx):
    clean = normalize(text).replace("\\", "/").replace("//?/", "")
    markers = {}
    for name in known_fixture_entries(fx):
        pattern = re.escape(name).replace("\\ ", r"\s+")
        match = re.search(r"(?<![\w.-])" + pattern + r"([/@*|=])?(?:\s+->\s+[^\\n]+)?", clean)
        if match:
            marker = match.group(1) or ("@" if " -> " in match.group(0) else "")
            markers[name] = marker
    return markers


def same_eza(actual, expected, case, fx):
    if not same_status(actual, expected):
        return False
    if case.get("comparison") == "normalized_streams":
        return same_normalized_streams(actual, expected)
    if expected["status"] != 0:
        return same_normalized_streams(actual, expected)
    if normalize(actual["stderr"]) != normalize(expected["stderr"]):
        return False
    name = case["name"]
    actual_entries = eza_entries(actual["stdout"], fx)
    expected_entries = eza_entries(expected["stdout"], fx)
    if name == "absolute":
        actual_text = normalize(actual["stdout"]).replace("\\", "/")
        expected_text = normalize(expected["stdout"]).replace("\\", "/")
        return "notes.txt" in actual_entries and ("/" in actual_text or "/" in expected_text)
    if name == "classify always":
        return eza_classifiers(actual["stdout"], fx) == eza_classifiers(expected["stdout"], fx)
    if name.startswith("sort ") or name in {"group dirs first", "multiple paths"}:
        return eza_order(actual["stdout"], fx) == eza_order(expected["stdout"], fx)
    return actual_entries == expected_entries


def zip_password(text):
    match = re.search(r"(?:password|found)[^A-Za-z0-9]+([A-Za-z0-9_!@#$%^&*.-]+)", normalize(text), re.I)
    return match.group(1) if match else None


def same_zip(actual, expected):
    if not same_status(actual, expected):
        return False
    if expected["status"] != 0:
        return bool(compact(actual["stderr"]) or compact(actual["stdout"]))
    expected_password = zip_password(expected["stdout"] + "\n" + expected["stderr"])
    actual_password = zip_password(actual["stdout"] + "\n" + actual["stderr"])
    if expected_password:
        return actual_password == expected_password
    return compact(actual["stdout"]) == compact(expected["stdout"])


def same_business(task, case, actual, expected, fx):
    if task == "zip-password-finder":
        return same_zip(actual, expected)
    if task == "xsv":
        if case.get("comparison") == "normalized_streams":
            return same_normalized_streams(actual, expected)
        if case.get("comparison") == "csv_unordered_body":
            return same_csv_unordered_body(actual, expected)
        if case.get("comparison") == "sample_csv":
            return same_sample_csv(actual, expected)
        return same_csv(actual, expected)
    if task == "eza":
        return same_eza(actual, expected, case, fx)
    if task == "nushell":
        if case["name"].startswith("help "):
            topic = case["name"].replace("help ", "")
            return same_status(actual, expected) and topic in normalize(actual["stdout"]).lower()
        if case["name"] in {"csv select"}:
            return same_csv(actual, expected)
        return same_json_or_scalar(actual, expected)
    return same_status(actual, expected)


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def make_fixtures(root):
    fx = root / "fixtures"
    fx.mkdir(parents=True, exist_ok=True)
    write(fx / "people.csv", "name,city,age,score\nalice,Paris,30,8.5\nbob,Berlin,22,7\ncarol,Paris,41,9.25\ndave,New York,30,8.5\neve,Berlin,,6\n")
    write(fx / "people2.csv", "name,city,age,score\nfrank,Rome,28,6.5\ngrace,Paris,35,8\n")
    write(fx / "cities.csv", "city,country\nParis,FR\nBerlin,DE\nRome,IT\n")
    write(fx / "semi.csv", "name;city;age\nana;Lisbon;10\nbea;Porto;12\n")
    write(fx / "no_headers.csv", "a,1,red\nb,2,blue\nc,3,red\n")
    write(fx / "unequal.csv", "a,b,c\n1,2\n3,4,5,6\n")
    write(fx / "notes.txt", "alpha\nbeta\nalphabet\n")
    write(fx / "empty.txt", "")
    write(fx / "long name file.txt", "spaces\n")
    write(fx / "script.py", "print('hello')\n# TODO: inspect\n")
    write(fx / "exec.sh", "#!/bin/sh\necho hi\n")
    write(fx / "README.md", "# Demo\n\nhello world\n")
    write(fx / "Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n")
    (fx / ".hidden").write_text("hidden\n", encoding="utf-8")
    sub = fx / "sub"
    sub.mkdir(exist_ok=True)
    write(sub / "nested.txt", "nested\n")
    write(sub / "data.log", "alpha log\n")
    deep = sub / "deep"
    deep.mkdir(exist_ok=True)
    write(deep / "final.md", "final\n")
    try:
        (fx / "link-notes").symlink_to(fx / "notes.txt")
    except Exception:
        pass
    return fx


def zip_cases(fx):
    zips = Path("rust-reference") / "test-files"
    dict_file = zips / "generated-passwords-lowercase.txt"
    two = zips / "2.test.txt.zip"
    three = zips / "3.test.txt.zip"
    return [
        {"name": "find generated", "args": ["-i", str(two), "-c", "l", "--maxPasswordLen", "2", "-w", "1"], "timeout": 90},
        {"name": "find generated starting password", "args": ["-i", str(three), "-c", "l", "--maxPasswordLen", "3", "-s", "abc", "-w", "1"], "timeout": 90},
        {"name": "not found", "args": ["-i", str(two), "-c", "l", "--maxPasswordLen", "1", "-w", "1"], "timeout": 90},
        {"name": "dictionary", "args": ["-i", str(two), "-p", str(dict_file), "-w", "1"], "timeout": 90},
        {"name": "mask two lowercase", "args": ["-i", str(two), "--mask", "?l?l", "-w", "1"], "timeout": 90},
        {"name": "mask custom charset", "args": ["-i", str(two), "--mask", "?1?1", "-1", "ab", "-w", "1"], "timeout": 90},
        {"name": "missing input", "args": ["-i", "missing.zip"]},
        {"name": "workers zero", "args": ["-i", str(two), "-w", "0"]},
        {"name": "min zero", "args": ["-i", str(two), "--minPasswordLen", "0"]},
        {"name": "max before min", "args": ["-i", str(two), "--minPasswordLen", "3", "--maxPasswordLen", "2"]},
        {"name": "file number missing", "args": ["-i", str(two), "--fileNumber", "99", "-c", "l", "--maxPasswordLen", "2"]},
    ]


def xsv_cases(fx):
    people = fx / "people.csv"
    people2 = fx / "people2.csv"
    cities = fx / "cities.csv"
    semi = fx / "semi.csv"
    no_headers = fx / "no_headers.csv"
    split_dir = fx / "split-out"
    partition_dir = fx / "partition-out"
    return [
        {"name": "headers", "kind": "success", "args": ["headers", str(people)]},
        {"name": "headers missing", "kind": "error", "args": ["headers", str(fx / "missing.csv")], "comparison": "normalized_streams"},
        {"name": "headers names", "kind": "success", "args": ["headers", "--just-names", str(people)]},
        {"name": "count file", "kind": "success", "args": ["count", str(people)]},
        {"name": "count stdin", "kind": "success", "args": ["count"], "stdin_file": people},
        {"name": "count missing", "kind": "error", "args": ["count", str(fx / "missing.csv")], "comparison": "normalized_streams"},
        {"name": "cat rows", "kind": "success", "args": ["cat", "rows", str(people), str(people2)]},
        {"name": "cat columns", "kind": "success", "args": ["cat", "columns", str(people), str(people2)]},
        {"name": "cat invalid mode", "kind": "error", "args": ["cat", "diagonal", str(people)], "comparison": "normalized_streams"},
        {"name": "select", "kind": "success", "args": ["select", "city,name", str(people)]},
        {"name": "select range", "kind": "success", "args": ["select", "1-2", str(people)]},
        {"name": "select no headers", "kind": "success", "args": ["select", "--no-headers", "1,3", str(no_headers)]},
        {"name": "select invert", "kind": "success", "args": ["select", "!score", str(people)]},
        {"name": "select missing column", "kind": "error", "args": ["select", "not-a-column", str(people)], "comparison": "normalized_streams"},
        {"name": "slice", "kind": "success", "args": ["slice", "-s", "1", "-l", "2", str(people)]},
        {"name": "slice end", "kind": "success", "args": ["slice", "-e", "3", str(people)]},
        {"name": "slice invalid range", "kind": "error", "args": ["slice", "-s", "3", "-e", "1", str(people)], "comparison": "normalized_streams"},
        {"name": "search", "kind": "success", "args": ["search", "-s", "city", "Berlin", str(people)]},
        {"name": "search regex", "kind": "success", "args": ["search", "-s", "name", "a.*e", str(people)]},
        {"name": "search invert", "kind": "success", "args": ["search", "-v", "-s", "city", "Berlin", str(people)]},
        {"name": "search missing column", "kind": "error", "args": ["search", "-s", "not-a-column", "Berlin", str(people)], "comparison": "normalized_streams"},
        {"name": "sort numeric", "kind": "success", "args": ["sort", "-s", "age", "-N", str(people)]},
        {"name": "sort numeric reverse", "kind": "success", "args": ["sort", "-s", "age", "-N", "-R", str(people)]},
        {"name": "sort reverse", "kind": "success", "args": ["sort", "-s", "city", "-R", str(people)]},
        {"name": "sort missing column", "kind": "error", "args": ["sort", "-s", "not-a-column", str(people)], "comparison": "normalized_streams"},
        {"name": "fmt delimiter", "kind": "success", "args": ["fmt", "-d", ";", str(semi)]},
        {"name": "fmt missing", "kind": "error", "args": ["fmt", str(fx / "missing.csv")], "comparison": "normalized_streams"},
        {"name": "input delimiter", "kind": "success", "args": ["input", "-d", ";", str(semi)]},
        {"name": "input missing", "kind": "error", "args": ["input", str(fx / "missing.csv")], "comparison": "normalized_streams"},
        {"name": "fixlengths", "kind": "success", "args": ["fixlengths", str(fx / "unequal.csv")]},
        {"name": "fixlengths missing", "kind": "error", "args": ["fixlengths", str(fx / "missing.csv")], "comparison": "normalized_streams"},
        {"name": "flatten", "kind": "success", "args": ["flatten", str(people)]},
        {"name": "flatten missing", "kind": "error", "args": ["flatten", str(fx / "missing.csv")], "comparison": "normalized_streams"},
        {"name": "table", "kind": "success", "args": ["table", str(people)]},
        {"name": "table missing", "kind": "error", "args": ["table", str(fx / "missing.csv")], "comparison": "normalized_streams"},
        {"name": "stats", "kind": "success", "args": ["stats", "-s", "age,score", str(people)]},
        {"name": "stats missing column", "kind": "error", "args": ["stats", "-s", "not-a-column", str(people)], "comparison": "normalized_streams"},
        {"name": "frequency", "kind": "success", "args": ["frequency", "-s", "city", str(people)], "comparison": "csv_unordered_body"},
        {"name": "frequency missing column", "kind": "error", "args": ["frequency", "-s", "not-a-column", str(people)], "comparison": "normalized_streams"},
        {"name": "join", "kind": "success", "args": ["join", "city", str(people), "city", str(cities)]},
        {"name": "join missing key", "kind": "error", "args": ["join", "not-a-column", str(people), "city", str(cities)], "comparison": "normalized_streams"},
        {"name": "sample", "kind": "success", "args": ["sample", "3", str(people)], "comparison": "sample_csv"},
        {"name": "sample invalid count", "kind": "error", "args": ["sample", "not-a-number", str(people)], "comparison": "normalized_streams"},
        {"name": "index", "kind": "success", "args": ["index", str(people)], "side_effects": [str(people) + ".idx"], "side_effect_mode": "exists"},
        {"name": "index missing", "kind": "error", "args": ["index", str(fx / "missing.csv")], "comparison": "normalized_streams"},
        {"name": "split", "kind": "success", "args": ["split", "-s", "2", str(split_dir), str(people)], "side_effects": [str(split_dir)]},
        {"name": "split invalid size", "kind": "error", "args": ["split", "-s", "0", str(split_dir), str(people)], "comparison": "normalized_streams"},
        {"name": "partition", "kind": "success", "args": ["partition", "city", str(partition_dir), str(people)], "side_effects": [str(partition_dir)]},
        {"name": "partition missing column", "kind": "error", "args": ["partition", "not-a-column", str(partition_dir), str(people)], "comparison": "normalized_streams"},
    ]


def eza_cases(fx):
    notes = fx / "notes.txt"
    readme = fx / "README.md"
    return [
        {"name": "plain", "kind": "success", "feature": "listing", "args": ["--color=never", "--icons=never", str(fx)]},
        {"name": "all", "kind": "success", "feature": "hidden", "args": ["--color=never", "--icons=never", "-a", str(fx)]},
        {"name": "almost all", "kind": "success", "feature": "hidden", "args": ["--color=never", "--icons=never", "-A", str(fx)]},
        {"name": "long", "kind": "success", "feature": "long-view", "args": ["--color=never", "--icons=never", "-l", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long all", "kind": "success", "feature": "long-view", "args": ["--color=never", "--icons=never", "-la", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long modified", "kind": "success", "feature": "time-fields", "args": ["--color=never", "--icons=never", "-l", "--modified", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long changed", "kind": "success", "feature": "time-fields", "args": ["--color=never", "--icons=never", "-l", "--changed", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long accessed", "kind": "success", "feature": "time-fields", "args": ["--color=never", "--icons=never", "-l", "--accessed", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long created", "kind": "success", "feature": "time-fields", "args": ["--color=never", "--icons=never", "-l", "--created", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "tree", "kind": "success", "feature": "tree-recursion", "args": ["--color=never", "--icons=never", "-T", "-L", "2", str(fx)]},
        {"name": "tree level one", "kind": "success", "feature": "tree-recursion", "args": ["--color=never", "--icons=never", "-T", "-L", "1", str(fx)]},
        {"name": "sort extension", "kind": "success", "feature": "sorting", "args": ["--color=never", "--icons=never", "--sort=extension", str(fx)]},
        {"name": "sort name reverse", "kind": "success", "feature": "sorting", "args": ["--color=never", "--icons=never", "--sort=name", "-r", str(fx)]},
        {"name": "sort size", "kind": "success", "feature": "sorting", "args": ["--color=never", "--icons=never", "--sort=size", str(fx)]},
        {"name": "sort modified", "kind": "success", "feature": "sorting", "args": ["--color=never", "--icons=never", "--sort=modified", str(fx)]},
        {"name": "sort none", "kind": "success", "feature": "sorting", "args": ["--color=never", "--icons=never", "-U", str(fx)]},
        {"name": "one per line", "kind": "success", "feature": "display-modes", "args": ["--color=never", "--icons=never", "-1", str(fx)]},
        {"name": "classify", "kind": "success", "feature": "classify", "args": ["--color=never", "--icons=never", "-F", str(fx)]},
        {"name": "classify always", "kind": "success", "feature": "classify", "args": ["--color=never", "--icons=never", "--classify=always", str(fx)]},
        {"name": "only dirs", "kind": "success", "feature": "filtering", "args": ["--color=never", "--icons=never", "-D", str(fx)]},
        {"name": "only files", "kind": "success", "feature": "filtering", "args": ["--color=never", "--icons=never", "-f", str(fx)]},
        {"name": "treat dirs as files", "kind": "success", "feature": "filtering", "args": ["--color=never", "--icons=never", "-d", str(fx)]},
        {"name": "ignore glob", "kind": "success", "feature": "filtering", "args": ["--color=never", "--icons=never", "-I", "*.csv|*.toml", str(fx)]},
        {"name": "recurse", "kind": "success", "feature": "tree-recursion", "args": ["--color=never", "--icons=never", "-R", "-L", "2", str(fx)]},
        {"name": "recurse all", "kind": "success", "feature": "tree-recursion", "args": ["--color=never", "--icons=never", "-Ra", "-L", "2", str(fx)]},
        {"name": "grid", "kind": "success", "feature": "display-modes", "args": ["--color=never", "--icons=never", "-G", str(fx)]},
        {"name": "across", "kind": "success", "feature": "display-modes", "args": ["--color=never", "--icons=never", "-x", str(fx)]},
        {"name": "binary sizes", "kind": "success", "feature": "size-format", "args": ["--color=never", "--icons=never", "-l", "-b", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "bytes sizes", "kind": "success", "feature": "size-format", "args": ["--color=never", "--icons=never", "-l", "-B", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "header", "kind": "success", "feature": "long-view", "args": ["--color=never", "--icons=never", "-l", "--header", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "group dirs first", "kind": "success", "feature": "sorting", "args": ["--color=never", "--icons=never", "--group-directories-first", str(fx)]},
        {"name": "group dirs last", "kind": "success", "feature": "sorting", "args": ["--color=never", "--icons=never", "--group-directories-last", str(fx)]},
        {"name": "absolute", "kind": "success", "feature": "path-display", "args": ["--color=never", "--icons=never", "--absolute", str(notes)]},
        {"name": "no quotes", "kind": "success", "feature": "path-display", "args": ["--color=never", "--icons=never", "--no-quotes", str(fx)]},
        {"name": "single file", "kind": "success", "feature": "path-display", "args": ["--color=never", "--icons=never", str(notes)]},
        {"name": "multiple paths", "kind": "success", "feature": "path-display", "args": ["--color=never", "--icons=never", str(notes), str(readme)]},
        {"name": "stdin paths", "kind": "success", "feature": "stdin", "args": ["--color=never", "--icons=never", "--stdin"], "stdin": f"{notes}\\n{readme}\\n"},
        {"name": "extension display", "kind": "success", "feature": "path-display", "args": ["--color=never", "--icons=never", "--extension", str(fx)]},
        {"name": "invalid sort", "kind": "error", "feature": "invalid-option-value", "args": ["--color=never", "--icons=never", "--sort=definitely-not-sort", str(fx)], "comparison": "normalized_streams"},
        {"name": "invalid color", "kind": "error", "feature": "invalid-option-value", "args": ["--color=bogus", "--icons=never", str(fx)], "comparison": "normalized_streams"},
        {"name": "invalid icons", "kind": "error", "feature": "invalid-option-value", "args": ["--color=never", "--icons=bogus", str(fx)], "comparison": "normalized_streams"},
        {"name": "invalid classify", "kind": "error", "feature": "invalid-option-value", "args": ["--color=never", "--icons=never", "--classify=bogus", str(fx)], "comparison": "normalized_streams"},
        {"name": "invalid level", "kind": "error", "feature": "invalid-numeric-value", "args": ["--color=never", "--icons=never", "-T", "-L", "not-a-number", str(fx)], "comparison": "normalized_streams"},
        {"name": "missing level value", "kind": "error", "feature": "missing-required-value", "args": ["--color=never", "--icons=never", "-T", "-L"], "comparison": "normalized_streams"},
        {"name": "missing", "kind": "error", "feature": "missing-path", "args": ["--color=never", "--icons=never", str(fx / "missing")], "comparison": "normalized_streams"},
        {"name": "unknown option", "kind": "error", "feature": "unknown-option", "args": ["--color=never", "--icons=never", "--definitely-not-an-eza-flag", str(fx)], "comparison": "normalized_streams"},
    ]


def nushell_cases(fx):
    people = fx / "people.csv"
    return [
        {"name": "help math", "args": ["-c", "help math"]},
        {"name": "help open", "args": ["-c", "help open"]},
        {"name": "math", "args": ["-c", "1 + 2"]},
        {"name": "pipeline math", "args": ["-c", "[1 2 3 4] | math sum"]},
        {"name": "math avg", "args": ["-c", "[1 2 3 4] | math avg"]},
        {"name": "math min", "args": ["-c", "[1 2 3 4] | math min"]},
        {"name": "math max", "args": ["-c", "[1 2 3 4] | math max"]},
        {"name": "string", "args": ["-c", "'hello' | str upcase"]},
        {"name": "string downcase", "args": ["-c", "'HELLO' | str downcase"]},
        {"name": "string contains", "args": ["-c", "'alphabet' | str contains 'alpha'"]},
        {"name": "string replace", "args": ["-c", "'alpha beta' | str replace beta gamma"]},
        {"name": "split row", "args": ["-c", "'a,b,c' | split row ',' | length"]},
        {"name": "json compact", "args": ["-c", "{name: alice, age: 30} | to json --raw"]},
        {"name": "from json", "args": ["-c", "'{\"name\":\"alice\",\"age\":30}' | from json | get name"]},
        {"name": "list length", "args": ["-c", "[1 2 3 4] | length"]},
        {"name": "first", "args": ["-c", "[1 2 3 4] | first"]},
        {"name": "last", "args": ["-c", "[1 2 3 4] | last"]},
        {"name": "skip", "args": ["-c", "[1 2 3 4] | skip 2 | first"]},
        {"name": "take", "args": ["-c", "[1 2 3 4] | take 2 | length"]},
        {"name": "each", "args": ["-c", "[1 2 3] | each { |x| $x * 2 } | math sum"]},
        {"name": "where filter", "args": ["-c", "[[name age]; [alice 30] [bob 22]] | where age > 25 | get name | first"]},
        {"name": "sort table", "args": ["-c", "[[name age]; [alice 30] [bob 22]] | sort-by age | get name | first"]},
        {"name": "select table", "args": ["-c", "[[name age city]; [alice 30 Paris]] | select name city | to json --raw"]},
        {"name": "insert table", "args": ["-c", "[[name age]; [alice 30]] | insert city Paris | to json --raw"]},
        {"name": "update table", "args": ["-c", "[[name age]; [alice 30]] | update age 31 | get age | first"]},
        {"name": "default value", "args": ["-c", "[[name age]; [alice null]] | default 0 age | get age | first"]},
        {"name": "transpose record", "args": ["-c", "{a: 1, b: 2} | transpose key value | length"]},
        {"name": "csv count", "args": ["-c", f"open {str(people)!r} | length"]},
        {"name": "csv select", "args": ["-c", f"open {str(people)!r} | select name city | to csv --noheaders"]},
        {"name": "csv where", "args": ["-c", f"open {str(people)!r} | where city == Paris | length"]},
        {"name": "open text", "args": ["-c", f"open {str(fx / 'notes.txt')!r} | lines | length"]},
        {"name": "open toml", "args": ["-c", f"open {str(fx / 'Cargo.toml')!r} | get package.name"]},
        {"name": "path exists", "args": ["-c", f"({str(people)!r} | path exists)"]},
        {"name": "path basename", "args": ["-c", f"{str(people)!r} | path basename"]},
        {"name": "path dirname", "args": ["-c", f"{str(people)!r} | path dirname | path basename"]},
        {"name": "ls fixture", "args": ["-c", f"ls {str(fx)!r} | length"]},
        {"name": "glob txt", "args": ["-c", f"glob {str(fx / '*.txt')!r} | length"]},
        {"name": "empty input", "args": ["-c", "'' | is-empty"]},
        {"name": "invalid json", "args": ["-c", "'not-json' | from json"]},
        {"name": "missing file", "args": ["-c", f"open {str(fx / 'missing.txt')!r}"]},
        {"name": "bad expression", "args": ["-c", "definitely-not-a-command"]},
    ]


def cases_for(task, fx):
    if task == "zip-password-finder":
        return zip_cases(fx)
    if task == "xsv":
        return xsv_cases(fx)
    if task == "eza":
        return eza_cases(fx)
    if task == "nushell":
        return nushell_cases(fx)
    raise AssertionError(task)


def command_name_from_case(task, case):
    if task == "xsv" and case.get("args"):
        return case["args"][0]
    if task == "eza":
        return case.get("feature")
    return None


def safe_case_name(name):
    return re.sub(r"[^A-Za-z0-9_.-]+", "-", name).strip("-") or "case"


def read_side_effect(path, mode=None):
    path = Path(path)
    if mode == "exists":
        return {"kind": "exists", "exists": path.exists()}
    if path.is_dir():
        rows = []
        for child in sorted(p for p in path.rglob("*") if p.is_file()):
            rows.append({"path": str(child.relative_to(path)), "content": child.read_bytes().decode("utf-8", "replace")})
        return {"kind": "dir", "entries": rows}
    if path.exists():
        return {"kind": "file", "content": path.read_bytes().decode("utf-8", "replace")}
    return {"kind": "missing"}


def side_effect_outputs(case):
    return [
        {"index": index, "output": read_side_effect(path, case.get("side_effect_mode"))}
        for index, path in enumerate(case.get("side_effects", []))
    ]


def eza_required_success_features():
    return {
        "listing", "hidden", "long-view", "time-fields", "tree-recursion",
        "sorting", "display-modes", "classify", "filtering", "size-format",
        "path-display", "stdin",
    }


def eza_required_error_features():
    return {
        "invalid-option-value", "invalid-numeric-value", "missing-required-value",
        "missing-path", "unknown-option",
    }


def discover_cli_help(task, workspace):
    root = run_cmd([REFERENCE_BINARY, "--help"], workspace, timeout=15)
    help_data = {
        "root_status": root["status"],
        "root_stdout": normalize(root["stdout"])[:12000],
        "root_stderr": normalize(root["stderr"])[:12000],
        "help_commands": [],
        "source_commands": [],
        "commands": [],
        "command_help": {},
    }
    if task == "eza":
        text = normalize(root["stdout"] + "\n" + root["stderr"])
        help_data["help_options"] = sorted(set(re.findall(r"(?<![\\w-])--[A-Za-z][A-Za-z0-9-]*(?:=\\w+)?", text)))
        help_data["commands"] = sorted(eza_required_success_features() | eza_required_error_features())
        return help_data
    if task != "xsv":
        return help_data
    text = normalize(root["stdout"] + "\n" + root["stderr"])
    commands = set()
    known = {
        "cat", "count", "fixlengths", "flatten", "fmt", "frequency", "headers",
        "index", "input", "join", "partition", "sample", "search", "select",
        "slice", "sort", "split", "stats", "table",
    }
    for line in text.splitlines():
        match = re.match(r"^\s{2,}([a-z][a-z0-9_-]+)\b", line)
        if match and match.group(1) in known:
            commands.add(match.group(1))
    if not commands:
        commands = known
    help_commands = set(commands)
    source_commands = set()
    source_path = workspace / "rust-reference" / "src" / "main.rs"
    if source_path.exists():
        source_text = source_path.read_text(encoding="utf-8", errors="replace")
        for command in known:
            quoted_command = "[\"']" + re.escape(command) + "[\"']"
            if re.search(quoted_command, source_text) or re.search(rf"\b{re.escape(command)}\b", source_text):
                source_commands.add(command)
    commands = help_commands | source_commands
    for command in sorted(commands):
        sub = run_cmd([REFERENCE_BINARY, command, "--help"], workspace, timeout=15)
        help_data["command_help"][command] = {
            "status": sub["status"],
            "stdout": normalize(sub["stdout"])[:4000],
            "stderr": normalize(sub["stderr"])[:4000],
        }
    help_data["help_commands"] = sorted(help_commands)
    help_data["source_commands"] = sorted(source_commands)
    help_data["commands"] = sorted(commands)
    return help_data


class Check:
    def __init__(self, workspace):
        self.workspace = Path(workspace)
        self.exe = self.workspace / "executable"
        self.failures = []
        self.passes = 0
        self.cli_help = None
        self.discovered_commands = []
        self.covered_commands = []
        self.covered_success_commands = []
        self.covered_error_commands = []

    def fail(self, name, detail):
        self.failures.append({"name": name, "detail": detail})

    def ok(self, name):
        self.passes += 1

    def check_files(self):
        for rel in ["executable", "compile.sh"]:
            if not (self.workspace / rel).exists():
                self.fail(f"{rel} exists", "missing")
            else:
                self.ok(f"{rel} exists")
        if (self.workspace / "REFERENCE_BINARY.txt").exists() or REFERENCE_BINARY.exists():
            self.ok("reference binary available")
        else:
            self.fail("reference binary available", "missing REFERENCE_BINARY.txt and SOURCE_PORT_REFERENCE_BINARY")
        if self.exe.exists() and self.exe.read_bytes()[:4] == b"\x7fELF":
            self.fail("executable is python", "./executable must be a Python script, not a binary")
        elif self.exe.exists():
            self.ok("executable is python")

    def check_cli_help_coverage(self, cases):
        self.cli_help = discover_cli_help(TASK, self.workspace)
        self.discovered_commands = list(self.cli_help.get("commands") or [])
        if TASK not in {"xsv", "eza"}:
            return
        if self.cli_help.get("root_status") != 0:
            self.fail("reference root help", {"status": self.cli_help.get("root_status"), "stderr": self.cli_help.get("root_stderr")})
            return
        covered = {
            command_name_from_case(TASK, case)
            for case in cases
            if command_name_from_case(TASK, case)
        }
        success_covered = {
            command_name_from_case(TASK, case)
            for case in cases
            if command_name_from_case(TASK, case) and case.get("kind", "success") == "success"
        }
        error_covered = {
            command_name_from_case(TASK, case)
            for case in cases
            if command_name_from_case(TASK, case) and case.get("kind") == "error"
        }
        self.covered_commands = sorted(covered)
        self.covered_success_commands = sorted(success_covered)
        self.covered_error_commands = sorted(error_covered)
        if TASK == "xsv":
            required_success = set(self.discovered_commands)
            required_error = set(self.discovered_commands)
            check_name = "success and error coverage for discovered xsv commands"
        else:
            required_success = eza_required_success_features()
            required_error = eza_required_error_features()
            check_name = "success and error coverage for discovered eza option groups"
        missing_success = sorted(required_success - success_covered)
        missing_error = sorted(required_error - error_covered)
        if missing_success or missing_error:
            self.fail(check_name, {
                "missing_success": missing_success,
                "missing_error": missing_error,
                "covered_success": self.covered_success_commands,
                "covered_error": self.covered_error_commands,
                "discovered": self.discovered_commands,
            })
        else:
            self.ok(check_name)

    def run(self):
        self.check_files()
        if not self.exe.exists():
            return
        fx = make_fixtures(self.workspace / "harness")
        cases = cases_for(TASK, fx)
        self.check_cli_help_coverage(cases)
        for case in cases:
            stdin = None
            if "stdin_file" in case:
                stdin = Path(case["stdin_file"]).read_text(encoding="utf-8")
            elif "stdin" in case:
                stdin = case["stdin"]
            expected_case = case
            actual_case = case
            expected_fx = fx
            actual_fx = fx
            if case.get("side_effects"):
                case_root = self.workspace / "harness" / "side-effects" / safe_case_name(case["name"])
                expected_fx = make_fixtures(case_root / "reference")
                actual_fx = make_fixtures(case_root / "actual")
                expected_case = next(item for item in cases_for(TASK, expected_fx) if item["name"] == case["name"])
                actual_case = next(item for item in cases_for(TASK, actual_fx) if item["name"] == case["name"])
            expected = run_cmd([REFERENCE_BINARY, *expected_case["args"]], self.workspace, stdin=stdin, timeout=case.get("timeout", 30))
            actual = run_cmd([sys.executable, self.exe, *actual_case["args"]], self.workspace, stdin=stdin, timeout=case.get("timeout", 30))
            ok = same_business(TASK, actual_case, actual, expected, actual_fx)
            expected_side_effects = side_effect_outputs(expected_case)
            actual_side_effects = side_effect_outputs(actual_case)
            side_effect_failures = expected_side_effects != actual_side_effects
            if ok and not side_effect_failures:
                self.ok(case["name"])
            else:
                self.fail(case["name"], {
                    "args": actual_case["args"],
                    "expected": expected,
                    "actual": actual,
                    "comparison": "business_semantics",
                    "expected_side_effects": expected_side_effects,
                    "actual_side_effects": actual_side_effects,
                })


def main():
    if len(sys.argv) < 2:
        print("usage: evaluate_source_port.py WORKSPACE [WORKSPACE...]", file=sys.stderr)
        return 2
    reports = []
    for workspace in sys.argv[1:]:
        check = Check(workspace)
        check.run()
        reports.append({
            "workspace": str(Path(workspace).resolve()),
            "passed": check.passes,
            "failed": len(check.failures),
            "discovered_commands": check.discovered_commands,
            "covered_commands": check.covered_commands,
            "covered_success_commands": check.covered_success_commands,
            "covered_error_commands": check.covered_error_commands,
            "cli_help": check.cli_help,
            "failures": check.failures,
        })
    print(json.dumps({"task": TASK, "reference_binary": str(REFERENCE_BINARY), "reports": reports}, indent=2))
    return 0 if all(report["failed"] == 0 for report in reports) else 1


if __name__ == "__main__":
    raise SystemExit(main())
`
}

function writeHarness(task) {
  const harnessPath = path.join(runRoot, "harness", "evaluate_source_port.py")
  writeFile(harnessPath, harnessTemplate())
  return harnessPath
}

async function prepareWorkspace(agentDir, task) {
  const workspace = path.join(agentDir, "workspace")
  validateWorkspaceGitPath(workspace)
  fs.rmSync(workspace, { recursive: true, force: true })
  mkdirp(workspace)
  const reference = ensureReferenceRepo(task)
  const binary = await ensureReferenceBinary(task)
  copyDir(reference, path.join(workspace, "rust-reference"))
  fs.rmSync(path.join(workspace, "rust-reference", ".git"), { recursive: true, force: true })
  writeFile(path.join(workspace, ".gitignore"), "rust-reference/\nharness/\n__pycache__/\n*.pyc\n")
  writeFile(path.join(workspace, "PYTHON_PORT_TASK.md"), sourcePortPrompt(task))
  writeFile(path.join(workspace, "REFERENCE_BINARY.txt"), binary)
  writeFile(path.join(workspace, "compile.sh"), "#!/usr/bin/env sh\nset -eu\n[ -f executable ]\n")
  const gitSetup = setupWorkspaceGit(workspace, [".gitignore", "PYTHON_PORT_TASK.md", "REFERENCE_BINARY.txt", "compile.sh"], "benchmark source-port fixture")
  return { workspace, reference_path: reference, reference_binary: binary, prompt_path: path.join(workspace, "PYTHON_PORT_TASK.md"), git_setup: gitSetup, error: null }
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

function escapeConfigPath(value) {
  return String(value).replace(/\\/g, "\\\\").replace(/"/g, "\\\"")
}

function contextArchiveDir(agentDir) {
  return path.join(agentDir, "context-and-calls")
}

function safeArchiveEnv(env) {
  const allowed = new Set([
    "CODEX_LOG_DIR",
    "COMMAND_RUN_AGENT_CONTEXT_ARCHIVE",
    "COMMAND_RUN_AGENT_TIMEOUT_MS",
    "LOG_PATH",
    "TURA_COMMAND_RUN_SHELL",
    "TURA_COMMAND_RUN_STRICT_JSON",
    "TURA_FORCE_EXECUTE_TOOLS_PLANNING",
    "TURA_PROJECT_ROOT",
    "TURA_SESSION_REASONING_EFFORT",
  ])
  const out = {}
  for (const [key, value] of Object.entries(env || {})) {
    if (allowed.has(key)) out[key] = value
  }
  return out
}

function writeAgentInvocationArchive(agentDir, details) {
  const archive = contextArchiveDir(agentDir)
  mkdirp(archive)
  writeFile(path.join(archive, "input-prompt.md"), details.input || "")
  const taskPromptPath = path.join(details.workspace, "PYTHON_PORT_TASK.md")
  if (fs.existsSync(taskPromptPath)) {
    writeFile(path.join(archive, "workspace-PYTHON_PORT_TASK.md"), fs.readFileSync(taskPromptPath, "utf8"))
  }
  writeJson(path.join(archive, "invocation.json"), {
    agent: details.agent,
    context_kind: details.context_kind,
    command: details.command,
    args: details.args,
    cwd: details.cwd,
    workspace: details.workspace,
    env: safeArchiveEnv(details.env),
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
    codex_goals_enabled: codexGoalsEnabled,
    tura_goal_enabled: turaGoalEnabled,
    planning_override: planningOverride === null ? "auto" : (planningOverride ? "on" : "off"),
    notes: details.notes || [],
  })
}

function codexHomeForAgent(agentDir, label) {
  const defaultCodexHome = process.env.CODEX_HOME || path.join(homeDir, ".codex")
  if (label === "codex-ponytail") return process.env.COMMAND_RUN_AGENT_CODEX_PONYTAIL_HOME || defaultCodexHome
  if (!truthy(process.env.COMMAND_RUN_AGENT_CODEX_CLEAN_HOME || "0")) return process.env.CODEX_HOME || undefined
  const cleanHome = path.join(agentDir, "codex-home-clean")
  mkdirp(cleanHome)
  const authSource = path.join(defaultCodexHome, "auth.json")
  if (fs.existsSync(authSource)) fs.copyFileSync(authSource, path.join(cleanHome, "auth.json"))
  writeFile(path.join(cleanHome, "config.toml"), [
    `model = ${JSON.stringify(model)}`,
    `model_reasoning_effort = ${JSON.stringify(reasoning)}`,
    `approval_policy = "never"`,
    `sandbox_mode = "danger-full-access"`,
    `service_tier = ${JSON.stringify(serviceTier)}`,
    "",
  ].join("\n"))
  return cleanHome
}

async function runCodexLike(workspace, agentDir, prompt, onProgress, codexExe, label) {
  assert(fs.existsSync(codexExe), `missing ${label} exe: ${codexExe}`)
  const codexLogDir = path.join(agentDir, "codex-log")
  const codexHome = codexHomeForAgent(agentDir, label)
  const command = codexExe
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
    ...(codexGoalsEnabled ? ["-c", "goals=true"] : []),
    "-c",
    `log_dir="${escapeConfigPath(codexLogDir)}"`,
    ...serviceTierConfigArgs(),
  ]
  const env = {
    COMMAND_RUN_AGENT_CONTEXT_ARCHIVE: "1",
    CODEX_LOG_DIR: codexLogDir,
    ...(codexHome ? { CODEX_HOME: codexHome } : {}),
  }
  writeAgentInvocationArchive(agentDir, {
    agent: label,
    workspace,
    command,
    args,
    cwd: workspace,
    env,
    input: prompt,
    context_kind: `${label}-stdin`,
    notes: [
      "Codex exec does not expose Tura provider LOG_PATH. The harness saves stdin prompt, visible JSONL events, configured log_dir, and any rollout files discoverable from events.",
    ],
  })
  return runLive(command, args, {
    cwd: workspace,
    input: prompt,
    timeoutMs,
    resolveOnTurnCompleted: true,
    dispatchWatchdogMs: Number(process.env.COMMAND_RUN_AGENT_DISPATCH_WATCHDOG_MS || 45_000),
    maxAttempts: Number(process.env.COMMAND_RUN_AGENT_TURA_ATTEMPTS || 3),
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
    onProgress,
    env,
  })
}

async function runCodexMain(workspace, agentDir, prompt, onProgress) {
  return runCodexLike(workspace, agentDir, prompt, onProgress, codexMainExe, "codex-main")
}

async function runCodexDocuments(workspace, agentDir, prompt, onProgress) {
  return runCodexLike(workspace, agentDir, prompt, onProgress, codexDocumentsExe, "codex-documents")
}

async function runCodexPonytail(workspace, agentDir, prompt, onProgress) {
  return runCodexLike(workspace, agentDir, prompt, onProgress, codexDocumentsExe, "codex-ponytail")
}

async function runTuraPlanning(workspace, agentDir, prompt, agentPrompt, onProgress) {
  assert(fs.existsSync(turaExe), `missing Tura exe: ${turaExe}`)
  const launchId = `source-port-${agentPrompt}-${process.pid}-${Date.now()}`
  const sessionCwd = prepareTuraSessionCwd(launchId)
  const providerLogPath = path.join(agentDir, "provider-log")
  snapshotTuraInternalPrompt(agentDir, agentPrompt)
  snapshotTuraAgentConfig(agentDir, agentPrompt)
  const planningMode = planningOverride ?? path.basename(agentDir).includes("planning")
  const command = turaExe
  const args = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    ...(turaGoalEnabled ? ["--goal"] : []),
    ...(turaExplicitSessionId ? ["--session-id", launchId] : []),
    "--sandbox",
    "--agent-id",
    agentPrompt,
    "-m",
    turaModel,
    ...turaServiceTierConfigArgs(),
    ...(planningOverride !== null || path.basename(agentDir).includes("planning") ? ["--planning", planningMode ? "on" : "off"] : []),
    "--model-reasoning-effort",
    reasoning,
    "--cwd",
    workspace,
  ]
  const env = {
    TURA_PROJECT_ROOT: repoRoot,
    LOG_PATH: providerLogPath,
    TURA_COMMAND_RUN_SHELL: process.env.COMMAND_RUN_AGENT_TURA_SHELL || "shell_command",
    TURA_COMMAND_RUN_STRICT_JSON: "0",
    TURA_SESSION_REASONING_EFFORT: reasoning,
    ...optionalEnv([
      "TURA_PROFILE_TURN_TIMINGS",
      "TURA_PROFILE_TIMINGS",
      "TURA_PROFILE_TURN_TIMING_BYTES",
      "TURA_PROFILE_TIMING_BYTES",
      "TURA_RUNTIME_WORKER_STDERR_LOG",
      "TURA_ROUTER_STDERR_LOG",
      "TURA_DEBUG_RUNTIME",
    ]),
    ...(planningMode ? { TURA_FORCE_EXECUTE_TOOLS_PLANNING: "1" } : {}),
    COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    COMMAND_RUN_AGENT_CONTEXT_ARCHIVE: "1",
  }
  writeAgentInvocationArchive(agentDir, {
    agent: path.basename(agentDir).replace(/-\d+$/, ""),
    workspace,
    command,
    args,
    cwd: sessionCwd,
    env,
    session_id: turaExplicitSessionId ? launchId : null,
    launch_id: launchId,
    input: prompt,
    context_kind: "tura-stdin-plus-provider-log",
    notes: [
      "Tura provider calls are expected under provider-log with full request.messages and response payloads.",
    ],
  })
  return runLive(command, args, {
    cwd: sessionCwd,
    input: prompt,
    timeoutMs,
    resolveOnTurnCompleted: true,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
    onProgress,
    env,
  })
}

function optionalEnv(keys) {
  return Object.fromEntries(
    keys
      .map((key) => [key, process.env[key]])
      .filter(([, value]) => value != null && String(value).trim() !== ""),
  )
}

async function runExternalCliAgent(workspace, agentDir, prompt, agentId, onProgress) {
  const isClaude = agentId === "claude-code"
  const command = isClaude ? claudeExe : piExe
  const args = isClaude
    ? claudeCodeArgs(prompt, { model: process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus" })
    : piAgentArgs(prompt)
  writeAgentInvocationArchive(agentDir, {
    agent: agentId,
    workspace,
    command,
    args,
    cwd: workspace,
    env: {},
    input: prompt,
    context_kind: `${agentId}-prompt-arg`,
    notes: [`${agentId} is launched through its CLI and scored only by the shared source-port harness.`],
  })
  return runLive(command, args, {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
    onProgress,
  })
}

function snapshotTuraInternalPrompt(agentDir, agentPrompt) {
  const promptPath = path.join(repoRoot, "agents", "src", agentPrompt, "prompt.md")
  if (!fs.existsSync(promptPath)) return null
  const content = fs.readFileSync(promptPath, "utf8")
  const snapshotPath = path.join(agentDir, "tura-internal-prompt.md")
  writeFile(snapshotPath, content)
  return { prompt_path: promptPath, snapshot_path: snapshotPath, sha256: crypto.createHash("sha256").update(content).digest("hex") }
}

function snapshotTuraAgentConfig(agentDir, agentPrompt) {
  const configPath = path.join(repoRoot, "agents", "src", agentPrompt, "agent_config.json")
  if (!fs.existsSync(configPath)) return null
  const content = fs.readFileSync(configPath, "utf8")
  const snapshotPath = path.join(agentDir, "tura-agent-config.json")
  writeFile(snapshotPath, content)
  return { config_path: configPath, snapshot_path: snapshotPath, sha256: crypto.createHash("sha256").update(content).digest("hex") }
}

function turaCapabilityInfo(agentPrompt, agentDir) {
  if (!agentPrompt) return null
  const configPath = path.join(repoRoot, "agents", "src", agentPrompt, "agent_config.json")
  let capabilities = []
  if (fs.existsSync(configPath)) {
    const config = JSON.parse(fs.readFileSync(configPath, "utf8"))
    capabilities = (config.agent_capabilities || []).map((item) => item.capability_name).filter(Boolean)
  }
  const planningMode = planningOverride ?? path.basename(agentDir).includes("planning")
  return {
    agent_prompt: agentPrompt,
    config_path: configPath,
    configured_capabilities: capabilities,
    config_has_task_status: capabilities.includes("task_status"),
    config_has_planning: capabilities.includes("planning"),
    planning_override: planningOverride === null ? "auto" : (planningOverride ? "on" : "off"),
    planning_cli_override_effective: planningMode,
    effective_planning_available: planningMode || capabilities.includes("planning"),
  }
}

function prepareTuraSessionCwd(sessionId) {
  const safe = sessionId.replace(/[^A-Za-z0-9_.-]/g, "_").slice(0, 80)
  const dir = path.join(suiteRoot, "tura-session-cwd", safe)
  mkdirp(path.join(dir, "crates", "session_log"))
  writeFile(path.join(dir, "Cargo.toml"), "[workspace]\n")
  return dir
}

function parseJsonl(text) {
  return String(text || "").split(/\r?\n/).map((line) => line.trim()).filter(Boolean).map((line) => {
    try { return JSON.parse(line) } catch { return null }
  }).filter(Boolean)
}

function addUsage(totals, usage) {
  totals.usage_events += 1
  totals.input_tokens += Number(usage.input_tokens || usage.prompt_tokens || 0)
  totals.output_tokens += Number(usage.output_tokens || usage.completion_tokens || 0)
  totals.reasoning_tokens += Number(usage.reasoning_tokens || usage.reasoning_output_tokens || usage.output_tokens_details?.reasoning_tokens || 0)
  totals.cached_input_tokens += Number(usage.cached_input_tokens || usage.input_tokens_details?.cached_tokens || usage.prompt_tokens_details?.cached_tokens || 0)
  totals.cache_write_tokens += Number(usage.cache_write_tokens || usage.input_tokens_details?.cache_write_tokens || usage.prompt_tokens_details?.cache_creation_tokens || 0)
  totals.total_tokens += Number(usage.total_tokens || 0)
  totals.latency_ms += Number(usage.latency_ms || 0)
}

function usageFromJsonl(stdout) {
  const totals = emptyUsage()
  for (const event of parseJsonl(stdout)) {
    const usage = event.usage || event.message?.usage || event.payload?.info?.last_token_usage || (event.type === "runtime_usage" ? event.usage : null)
    if (usage) addUsage(totals, usage)
  }
  return totals
}

function emptyUsage() {
  return {
    usage_events: 0,
    input_tokens: 0,
    output_tokens: 0,
    reasoning_tokens: 0,
    cached_input_tokens: 0,
    cache_write_tokens: 0,
    total_tokens: 0,
    latency_ms: 0,
  }
}

function jsonFilesUnder(root) {
  if (!fs.existsSync(root)) return []
  const files = []
  const stack = [root]
  while (stack.length > 0) {
    const current = stack.pop()
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(full)
      else if (entry.isFile() && entry.name.endsWith(".json")) files.push(full)
    }
  }
  return files
}

function usageFromProviderLogs(logRoot) {
  const totals = emptyUsage()
  const calls = []
  for (const file of jsonFilesUnder(logRoot)) {
    let payload
    try { payload = JSON.parse(fs.readFileSync(file, "utf8")) } catch { continue }
    if (payload?.type !== "llm_call") continue
    const usage = payload.metrics?.usage
    if (!usage) continue
    addUsage(totals, usage)
    calls.push({
      file,
      call_id: payload.call_id,
      success: payload.success,
      provider: payload.provider,
      model: payload.model,
      started_at: payload.started_at,
      finished_at: payload.finished_at,
      duration_ms: payload.duration_ms,
      usage,
    })
  }
  calls.sort((a, b) => String(a.started_at || "").localeCompare(String(b.started_at || "")))
  return { totals, calls }
}

function responseOutputText(response) {
  const text = response?.output_text
  if (typeof text === "string") return text
  if (Array.isArray(response?.output)) {
    return response.output
      .flatMap((item) => Array.isArray(item?.content) ? item.content : [])
      .map((content) => content?.text || "")
      .filter(Boolean)
      .join("\n")
  }
  return ""
}

function providerCallDebugRows(agentDir, seen = new Set()) {
  const rows = []
  for (const file of jsonFilesUnder(path.join(agentDir, "provider-log")).sort()) {
    let payload
    try { payload = JSON.parse(fs.readFileSync(file, "utf8")) } catch { continue }
    if (payload?.type !== "llm_call") continue
    const key = `${file}:${payload.finished_at || payload.duration_ms || payload.error || ""}`
    if (seen.has(key)) continue
    seen.add(key)
    const response = payload.response || null
    const outputText = responseOutputText(response)
    rows.push({
      file,
      call_id: payload.call_id,
      success: payload.success,
      provider: payload.provider,
      model: payload.model,
      started_at: payload.started_at,
      finished_at: payload.finished_at,
      duration_ms: payload.duration_ms,
      error: payload.error || response?.error || null,
      request_messages_count: Array.isArray(payload.request?.messages) ? payload.request.messages.length : null,
      request_tools_count: Array.isArray(payload.request?.params?.tools) ? payload.request.params.tools.length : null,
      response_status: response?.status || null,
      response_output_count: Array.isArray(response?.output) ? response.output.length : null,
      response_text_preview: outputText.slice(0, 500),
      tool_call_count: payload.metrics?.tool_call_count ?? null,
      usage: payload.metrics?.usage || null,
    })
  }
  return rows
}

function discoverRolloutPathsFromStdout(stdout) {
  const paths = new Set()
  const visit = (value) => {
    if (typeof value === "string") {
      if (/rollout/i.test(value) && /\.(jsonl|json)$/i.test(value)) paths.add(value)
      return
    }
    if (!value || typeof value !== "object") return
    if (!Array.isArray(value)) {
      for (const [key, item] of Object.entries(value)) {
        if (key === "rollout_path" && typeof item === "string") paths.add(item)
        else visit(item)
      }
      return
    }
    for (const item of value) visit(item)
  }
  for (const event of parseJsonl(stdout)) visit(event)
  return [...paths].filter((item) => fs.existsSync(item))
}

function copyFileIfReadable(src, dest) {
  try {
    mkdirp(path.dirname(dest))
    fs.copyFileSync(src, dest)
    return true
  } catch {
    return false
  }
}

function refreshContextAndCallArchive(agentDir, stdout = "", options = {}) {
  const archive = contextArchiveDir(agentDir)
  mkdirp(archive)

  const providerLogRoot = path.join(agentDir, "provider-log")
  const providerFiles = jsonFilesUnder(providerLogRoot).sort()
  const fullCalls = []
  for (const file of providerFiles) {
    let payload = null
    try { payload = JSON.parse(fs.readFileSync(file, "utf8")) } catch {}
    if (payload?.type !== "llm_call") continue
    fullCalls.push({
      source_file: file,
      ...payload,
    })
  }
  writeFile(
    path.join(archive, "provider-calls-full.jsonl"),
    fullCalls.map((call) => JSON.stringify(call)).join("\n") + (fullCalls.length ? "\n" : ""),
  )
  writeJson(path.join(archive, "provider-calls-index.json"), fullCalls.map((call, index) => ({
    index,
    source_file: call.source_file,
    call_id: call.call_id,
    success: call.success,
    provider: call.provider,
    model: call.model,
    started_at: call.started_at,
    finished_at: call.finished_at,
    duration_ms: call.duration_ms,
    usage: call.metrics?.usage || null,
    request_messages_count: Array.isArray(call.request?.messages) ? call.request.messages.length : null,
    has_request_messages: Array.isArray(call.request?.messages),
    has_response: Boolean(call.response),
  })))

  const rolloutPaths = discoverRolloutPathsFromStdout(stdout)
  const copiedRollouts = []
  rolloutPaths.forEach((rolloutPath, index) => {
    const dest = path.join(archive, "codex-rollouts", `${String(index + 1).padStart(2, "0")}-${path.basename(rolloutPath)}`)
    if (copyFileIfReadable(rolloutPath, dest)) copiedRollouts.push({ source: rolloutPath, archive: dest })
  })
  writeJson(path.join(archive, "codex-rollout-paths.json"), copiedRollouts)

  const visibleEventsPath = path.join(agentDir, "stdout.jsonl")
  if (stdout && options.preferStdoutSnapshot) {
    writeFile(path.join(archive, "visible-agent-events.jsonl"), stdout)
  } else if (fs.existsSync(visibleEventsPath)) {
    copyFileIfReadable(visibleEventsPath, path.join(archive, "visible-agent-events.jsonl"))
  } else if (stdout) {
    writeFile(path.join(archive, "visible-agent-events.jsonl"), stdout)
  }

  writeJson(path.join(archive, "archive-summary.json"), {
    provider_log_root: providerLogRoot,
    provider_call_count: fullCalls.length,
    provider_calls_full_path: path.join(archive, "provider-calls-full.jsonl"),
    provider_calls_include_full_request_messages: fullCalls.some((call) => Array.isArray(call.request?.messages)),
    codex_rollout_count: copiedRollouts.length,
    codex_rollout_archive_dir: path.join(archive, "codex-rollouts"),
    visible_events_path: path.join(archive, "visible-agent-events.jsonl"),
    limitation: fullCalls.length > 0
      ? "Provider call logs are available and include the raw request object saved by the provider logger."
      : "No provider call logs were found for this agent. The archive contains stdin prompt, invocation config, visible JSONL events, and any discoverable rollout files.",
  })
  return {
    archive_dir: archive,
    provider_call_count: fullCalls.length,
    provider_calls_full_path: path.join(archive, "provider-calls-full.jsonl"),
    codex_rollout_count: copiedRollouts.length,
  }
}

function usageForAgent(agentDir, stdout) {
  const provider = usageFromProviderLogs(path.join(agentDir, "provider-log"))
  if (provider.totals.usage_events > 0) {
    return { usage: provider.totals, usage_source: "provider_log", provider_calls: provider.calls }
  }
  return { usage: usageFromJsonl(stdout), usage_source: "stdout_jsonl", provider_calls: [] }
}

function eventStats(stdout) {
  const events = parseJsonl(stdout)
  const stats = {
    events: events.length,
    thread_started: 0,
    turn_started: 0,
    turn_completed: 0,
    agent_messages: 0,
    command_executions: 0,
    commands_completed: 0,
    commands_failed: 0,
    file_changes: 0,
    task_status_callbacks: 0,
    planning_mentions: 0,
    planning_command_executions: 0,
  }
  for (const event of events) {
    const text = JSON.stringify(event)
    if (event.type === "thread.started") stats.thread_started += 1
    if (event.type === "turn.started") stats.turn_started += 1
    if (event.type === "turn.completed") stats.turn_completed += 1
    if (event.item?.type === "agent_message") stats.agent_messages += 1
    if (event.item?.type === "file_change") stats.file_changes += 1
    if (event.item?.type === "command_execution") {
      stats.command_executions += 1
      if (event.item.status === "completed") stats.commands_completed += 1
      if (event.item.status === "failed") stats.commands_failed += 1
      if (event.item.command === "task_status" || /"task_status"\s*:/.test(text)) stats.task_status_callbacks += 1
      if (event.item.command === "planning" || /"planning"\s*:/.test(text)) stats.planning_command_executions += 1
    }
    if (text.includes("planning")) stats.planning_mentions += 1
  }
  stats.dispatch_ok = stats.command_executions > 0 || stats.file_changes > 0 || stats.planning_mentions > 0
  stats.callback_ok = stats.task_status_callbacks > 0 || stats.turn_completed > 0
  return stats
}

function collectPatch(workspace, agentDir) {
  const patchPath = path.join(agentDir, "model.patch")
  const statusPath = path.join(agentDir, "git-status.txt")
  const diff = run("git", ["diff", "--binary"], { cwd: workspace, timeoutMs: 120_000 })
  const status = run("git", ["status", "--short"], { cwd: workspace, timeoutMs: 120_000 })
  writeFile(patchPath, diff.stdout || "")
  writeFile(statusPath, status.stdout || "")
  return {
    patch_path: patchPath,
    patch_bytes: Buffer.byteLength(diff.stdout || "", "utf8"),
    changed_files: status.stdout.split(/\r?\n/).filter(Boolean).length,
    git_status: status.stdout,
  }
}

function evaluateWorkspace(workspace, agentDir, task, binary) {
  if (!runEval) return { ran: false, reason: "SOURCE_PORT_RUN_EVAL is not 1" }
  const harnessPath = writeHarness(task)
  const result = run(process.env.PYTHON || "python", [harnessPath, workspace], {
    cwd: workspace,
    timeoutMs: Number(process.env.SOURCE_PORT_EVAL_TIMEOUT_MS || 10 * 60_000),
    env: {
      SOURCE_PORT_TASK: task.label,
      SOURCE_PORT_REFERENCE_BINARY: binary,
    },
  })
  writeFile(path.join(agentDir, "source-port-eval.stdout.log"), result.stdout)
  writeFile(path.join(agentDir, "source-port-eval.stderr.log"), result.stderr)
  let parsed = null
  try { parsed = JSON.parse(result.stdout) } catch {}
  return {
    ran: true,
    exit_code: result.status,
    stdout_path: path.join(agentDir, "source-port-eval.stdout.log"),
    stderr_path: path.join(agentDir, "source-port-eval.stderr.log"),
    error: result.error,
    report: parsed,
  }
}

async function runAgent(agentId, task, taskIndex, agentIndex, onAgentUpdate = null) {
  const agentDir = path.join(runRoot, taskRunDirName(task), `${agentId}-${agentIndex + 1}`)
  const prep = await prepareWorkspace(agentDir, task)
  const prompt = sourcePortPrompt(task)
  let result
  const started = performance.now()
  const agentPrompt =
    agentId === "tura-fast-shll" || agentId === "tura-fast-planning-shll" ? "fast" :
    agentId === "tura-balanced" ? "balanced" :
    agentId === "tura-direct" ? "direct" :
    agentId === "tura-thinking-shll" ? "thinking" :
    agentId === "tura-thinking-visual-shll" ? "thinking-visual" :
    agentId === "tura-planning-shll" ? "thinking-planning" :
    null
  let lastContextArchive = null
  let lastContextArchiveRefreshMs = 0
  const seenProviderDebugRows = new Set()
  const publishProgress = (liveResult) => {
    const isLive = liveResult.status === null && !liveResult.error
    if (printProviderLog) {
      for (const row of providerCallDebugRows(agentDir, seenProviderDebugRows)) {
        console.log(`[source-port-provider] ${JSON.stringify({ agent: agentId, task: task.label, ...row })}`)
      }
    }
    const usageInfo = isLive
      ? { usage: usageFromJsonl(liveResult.stdout || ""), usage_source: "stdout_jsonl", provider_calls: [] }
      : usageForAgent(agentDir, liveResult.stdout || "")
    const now = performance.now()
    const shouldRefreshArchive =
      !isLive && (!lastContextArchive || liveResult.status !== null || now - lastContextArchiveRefreshMs > 10_000)
    if (shouldRefreshArchive) {
      lastContextArchive = refreshContextAndCallArchive(agentDir, liveResult.stdout || "", {
        preferStdoutSnapshot: liveResult.status === null,
      })
      lastContextArchiveRefreshMs = now
    } else if (!lastContextArchive) {
      lastContextArchive = {
        archive_dir: contextArchiveDir(agentDir),
        provider_call_count: 0,
        provider_calls_full_path: path.join(contextArchiveDir(agentDir), "provider-calls-full.jsonl"),
        codex_rollout_count: 0,
      }
    }
    const stats = {
      agent: agentId,
      task: task.label,
      instance_id: task.id,
      workspace: prep.workspace,
      prep,
      in_progress: liveResult.status === null && !liveResult.error,
      elapsed_ms: Math.round(performance.now() - started),
      exit_code: liveResult.status,
      timed_out: liveResult.timed_out || false,
      dispatch_stalled: liveResult.dispatch_stalled || false,
      attempt: liveResult.attempt || 1,
      max_attempts: liveResult.max_attempts || 1,
      first_output_ms: liveResult.first_output_ms,
      last_progress_ms: liveResult.last_progress_ms ?? null,
      error: liveResult.error || null,
      stdout_path: path.join(agentDir, "stdout.jsonl"),
      stderr_path: path.join(agentDir, "stderr.log"),
      provider_log_path: path.join(agentDir, "provider-log"),
      tura_capability_info: turaCapabilityInfo(agentPrompt, agentDir),
      usage: agentId === "claude-code" || agentId === "pi-agent" ? agentUsageFromJsonl(liveResult.stdout || "") : usageInfo.usage,
      usage_source: agentId === "claude-code" || agentId === "pi-agent" ? `${agentId}-jsonl` : usageInfo.usage_source,
      provider_calls: usageInfo.provider_calls,
      context_archive: lastContextArchive,
      events: agentId === "claude-code" || agentId === "pi-agent" ? agentEventStats(liveResult.stdout || "") : eventStats(liveResult.stdout || ""),
    }
    writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
    onAgentUpdate?.(stats)
  }
  if (agentId === "codex-main") result = await runCodexMain(prep.workspace, agentDir, prompt, publishProgress)
  else if (agentId === "codex-documents") result = await runCodexDocuments(prep.workspace, agentDir, prompt, publishProgress)
  else if (agentId === "codex-ponytail") result = await runCodexPonytail(prep.workspace, agentDir, prompt, publishProgress)
  else if (agentId === "tura-fast-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "fast", publishProgress)
  else if (agentId === "tura-fast-planning-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "fast", publishProgress)
  else if (agentId === "tura-balanced") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "balanced", publishProgress)
  else if (agentId === "tura-direct") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "direct", publishProgress)
  else if (agentId === "tura-thinking-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "thinking", publishProgress)
  else if (agentId === "tura-thinking-visual-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "thinking-visual", publishProgress)
  else if (agentId === "tura-planning-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "thinking-planning", publishProgress)
  else if (agentId === "claude-code" || agentId === "pi-agent") result = await runExternalCliAgent(prep.workspace, agentDir, prompt, agentId, publishProgress)
  else throw new Error(`unsupported agent ${agentId}`)

  const patch = collectPatch(prep.workspace, agentDir)
  const evalResult = evaluateWorkspace(prep.workspace, agentDir, task, prep.reference_binary)
  const usageInfo = usageForAgent(agentDir, result.stdout)
  const contextArchive = refreshContextAndCallArchive(agentDir, result.stdout)
  const stats = {
    agent: agentId,
    task: task.label,
    instance_id: task.id,
    workspace: prep.workspace,
    prep,
    in_progress: false,
    elapsed_ms: Math.round(performance.now() - started),
    exit_code: result.status,
    timed_out: result.timed_out || false,
    first_output_ms: result.first_output_ms,
    error: result.error || null,
    stdout_path: path.join(agentDir, "stdout.jsonl"),
    stderr_path: path.join(agentDir, "stderr.log"),
    provider_log_path: path.join(agentDir, "provider-log"),
    tura_capability_info: turaCapabilityInfo(agentPrompt, agentDir),
    usage: agentId === "claude-code" || agentId === "pi-agent" ? agentUsageFromJsonl(result.stdout) : usageInfo.usage,
    usage_source: agentId === "claude-code" || agentId === "pi-agent" ? `${agentId}-jsonl` : usageInfo.usage_source,
    provider_calls: usageInfo.provider_calls,
    context_archive: contextArchive,
    events: agentId === "claude-code" || agentId === "pi-agent" ? agentEventStats(result.stdout) : eventStats(result.stdout),
    patch,
    eval: evalResult,
  }
  writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
  onAgentUpdate?.(stats)
  return stats
}

async function runSelfTest() {
  mkdirp(runRoot)
  for (const id of selectedTasks) {
    const task = TASKS[id]
    const prompt = sourcePortPrompt(task)
    for (const expected of ["official binary", "REFERENCE_BINARY.txt", "python ./executable", "Do not use Docker", "Do not shell out"]) {
      assert(prompt.includes(expected), `${task.label} prompt missing ${expected}`)
    }
  }
  const harness = harnessTemplate()
  for (const expected of ["run_cmd([REFERENCE_BINARY", "zip_cases", "xsv_cases", "eza_cases", "nushell_cases", "same_business"]) {
    assert(harness.includes(expected), `harness missing ${expected}`)
  }
  return { ok: true, self_test: "source-port rewrite suite", tasks: selectedTasks }
}

async function ensureTaskAssets(task) {
  const reference = ensureReferenceRepo(task)
  const binary = await ensureReferenceBinary(task)
  const smoke = smokeReferenceBinary(task, binary)
  return { task: task.label, reference, binary, version_stdout: smoke.stdout.trim(), version_stderr: smoke.stderr.trim(), version_status: smoke.status }
}

function buildSuiteSummary(results, assets, inProgress = false) {
  return normalizeBusinessSummary({
    ok: !inProgress && results.every(resultPassed),
    in_progress: inProgress,
    suite_root: suiteRoot,
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
    complex_todo_hint: complexTodoHint,
    agents,
    tasks: selectedTasks,
    assets,
    results,
  }, runPaths)
}

function resultPassed(result) {
  if (result.error || !result.events?.callback_ok) return false
  if (!result.eval?.ran) return true
  const reports = Array.isArray(result.eval?.report?.reports) ? result.eval.report.reports : []
  const failed = reports.reduce((total, report) => total + Number(report?.failed || 0), 0)
  return Number(result.eval.exit_code) === 0 && failed === 0
}

async function main() {
  mkdirp(runRoot)
  if (selfTest) {
    const summary = normalizeBusinessSummary(await runSelfTest(), runPaths)
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  const taskObjects = selectedTasks.map((id) => TASKS[id])
  const assets = []
  for (const task of taskObjects) {
    assets.push(await ensureTaskAssets(task))
  }
  if (binaryOnly) {
    const summary = normalizeBusinessSummary({ ok: true, binary_only: true, suite_root: suiteRoot, assets }, runPaths)
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  if (prepOnly) {
    const preps = []
    for (const task of taskObjects) {
      const prepDir = path.join(runRoot, taskRunDirName(task), "prep-only")
      const prep = await prepareWorkspace(prepDir, task)
      const harnessPath = writeHarness(task)
      preps.push({ task: task.label, prep, harness_path: harnessPath })
    }
    const summary = normalizeBusinessSummary({ ok: true, prep_only: true, suite_root: suiteRoot, complex_todo_hint: complexTodoHint, assets, preps }, runPaths)
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  const jobs = []
  const partialResults = new Map()
  let finalSummaryWritten = false
  const writeProgressSummary = () => {
    if (finalSummaryWritten) return
    const results = [...partialResults.values()].sort((a, b) =>
      `${a.task}:${a.agent}`.localeCompare(`${b.task}:${b.agent}`))
    writeFile(summaryPath, JSON.stringify(buildSuiteSummary(results, assets, true), null, 2))
  }
  for (let t = 0; t < taskObjects.length; t += 1) {
    for (let a = 0; a < agents.length; a += 1) {
      console.log(`[source-port-suite] running ${agents[a]} on ${taskObjects[t].label} for ${Math.round(timeoutMs / 1000)}s`)
      const key = `${taskObjects[t].id}:${agents[a]}:${a}`
      jobs.push(runAgent(agents[a], taskObjects[t], t, a, (stats) => {
        partialResults.set(key, stats)
        writeProgressSummary()
      }))
    }
  }
  const results = await Promise.all(jobs)
  const summary = buildSuiteSummary(results, assets, false)
  finalSummaryWritten = true
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
}

await main()
