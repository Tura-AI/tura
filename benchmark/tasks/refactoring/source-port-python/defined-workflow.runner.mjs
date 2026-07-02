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
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `source-port-suite-defined-workflow-${Date.now()}`
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
const turaExplicitSessionId = truthy(process.env.COMMAND_RUN_AGENT_TURA_SESSION_ID || "0")
const turaGoalMode = truthy(process.env.COMMAND_RUN_AGENT_TURA_GOAL || process.env.TURA_GOAL_MODE || "0")
const turaTestPromptStyle = String(process.env.COMMAND_RUN_AGENT_TEST_PROMPT_STYLE || "").trim()
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
  return `You are in a benchmark workspace containing a Rust reference application at ./rust-reference and an official release binary path recorded in ./REFERENCE_BINARY.txt.

Goal:
Create a Python implementation that replicates the reference application's supported CLI/API behavior for this benchmark. The evaluator invokes the official binary at runtime as an oracle and compares your port against that oracle for the same inputs.

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
- Do not fake tests by special-casing harness file names only.

Refactoring Task Operation Manual

Positive examples:
- Good workflow example: To refactor this project safely, first confirm the CLI and API input/output behavior before changing the structure. Use --help, subcommand help, local API docs, source registries, tests, fixtures, and the official binary as the oracle, then reproduce and compare the input/output results one by one.
- Good oracle example: for each discovered behavior, create at least two distinct correct/valid input cases that succeed on the official implementation and compare status/stdout/stderr against the port, plus at least one incorrect/invalid input case that compares the official nonzero status/error behavior against the port.
- Good oracle example: when external or evaluator-like fixtures and invocations are visible or derivable, mirror that same distribution and generate expected status/stdout/stderr live from the official binary for each case.
- Good reflection example: before each tool batch, check the current artifact against the required workflow. If the verifier uses command metadata, help length, a sampled smoke set, a single valid case, or invalid-only checks as coverage, identify the exact workflow step that diverged and return to that step before continuing.

Negative examples:
- Bad workflow example: building a command inventory from scope/help output, adding one generic valid probe such as \`null | command\`, adding one invalid flag probe, and calling the result a complete oracle.
- Bad workflow example: using 20 to 30 hand-written smoke cases plus hundreds of metadata/help rows and claiming all commands were tested.
- Bad workflow example: using the official binary but on a different fixture tree or narrower invocation distribution than the evaluator, then treating the verifier as final because it is green.
- Bad workflow example: claiming a flag or command is covered while combining it with options or fixtures that suppress the visible behavior being claimed.
- Bad workflow example: letting the port read behavior_map, oracle observations, provider logs, previous harness reports, or verifier artifacts to decide runtime output.
- Bad reflection example: continuing to patch the implementation after discovering that the oracle only has one valid case, invalid-only coverage, metadata-only coverage, or failed valid probes. The correct response is to return to the oracle construction step and fix the verifier before relying on it.

Workflow:
- First establish the complete CLI and API behavior inventory before claiming implementation is complete. Use --help, subcommand help, API docs, source signatures, tests, fixtures, and direct official-binary probes as needed.
- For every discovered command, subcommand, alias, mode, flag family, and public API behavior, record a behavior-map row with the item name, supported input shape, concrete executable invocation or API call, stdin/file fixtures, the official-binary oracle command that produces status/stdout/stderr live, the source tests or fixtures that justify the behavior, the official error code or exit status for invalid input observed from the official binary, and the Python implementation path responsible for it.
- For every correct input/output behavior, the oracle must contain at least two distinct correct/valid input cases that both succeed on the official implementation and compare status/stdout/stderr against the port. A single valid case is insufficient coverage for that behavior.
- For every discovered behavior, the oracle must also contain at least one incorrect/invalid input case and must compare the corresponding official error code or nonzero status, stdout, and stderr against the port.
- Inventory/help/metadata checks may exist only as diagnostics; they never satisfy functional coverage unless the behavior being tested is itself a help/scope/metadata command and still has two valid cases plus one invalid case.
- Before each implementation or verification phase, check the current work against this workflow. If the work skipped an earlier workflow step, replaced the required oracle with metadata/help/smoke checks, used only one valid case, used invalid-only checks, or drifted into a smaller acceptance boundary, stop extending that wrong branch. Return to the exact point where the workflow diverged, restate the missed step, and redo the work from that point.
- Compare the submitted port against the official implementation for the same invocation and fixtures. The verification standard is the official binary's live behavior plus the project source tests and fixtures, not self-authored expected-output assets. Keep oracle commands, behavior maps, logs, and verifier artifacts outside the submitted executable/runtime implementation. The port may not read behavior_map, oracle observations, verifier artifacts, provider logs, previous harness reports, or official-reference observations to decide runtime output.
- A self-written verifier is only a work aid, never the acceptance boundary. It is valid evidence only when it reproduces same-distribution oracle feedback: the same visible or derivable fixtures, invocations, stdin, environment, comparison mode, and official-binary oracle generation used by the evaluator or external harness.
- Do not hard-code expected stdout, stderr, or status in verifier cases. For each exact invocation and fixture, run the official implementation live to produce expected status/stdout/stderr, then run the port and compare those three channels.
- Do not count flag inventory coverage when the case suppresses or bypasses the observable effect being claimed. Each verifier row must exercise the behavior it claims to cover.
- Treat a passing self-verifier as provisional until you audit that its fixture distribution, case list, and comparison semantics match the evaluator scope. If the verifier uses a different fixture tree, sampled cases, metadata/help rows, or weakened/deleted failures, return to oracle construction before implementation or final reporting.
- Implement real parser and execution semantics, then use the behavior map to drive fixes until valid-value behavior, invalid-value behavior, error codes/status, stdout, stderr, files, and exit behavior match the official implementation for every mapped behavior.
- Before final response, run syntax checks, compile/wrapper checks, the full behavior-map oracle, and the external benchmark harness when available. Treat any failed, skipped, timed-out, or weakly scoped verification as unfinished work.
- At the end of each turn, explicitly verify that the behavior inventory, behavior-map rows, official-binary oracle probes, implementation fixes, and final verification still cover the exact requested scope.

Required benchmark scope:
- Determine the required CLI/API surface from authoritative local sources: the official binary's --help output, command-listing commands, local API docs when present, the Rust command dispatcher, source registries, tests, fixtures, and official binary behavior.
- You must identify the complete supported local CLI/API surface that affects this task, including subcommands, aliases, modes, flags, stdin/file inputs, output formats, stdout/stderr behavior, status codes, and realistic error cases.
- Treat the official binary and local source as the source of truth for which commands, flags, inputs, outputs, exit codes, and error cases matter.

Equivalence requirements:
- For every required command/flag/input you identify, match the official binary's observable behavior: success/failure status, stdout, stderr, parsed data results, selected files, ordering when meaningful, filtering, transformations, and error handling.
- Match data behavior: ordering where meaningful, delimiters, quoting, escaping, headers, path selection, numeric/string coercion, and realistic failure cases.
- If the official binary prints nothing, your program must print nothing. If the official binary writes to stderr, your program must write to stderr, not stdout.
- The evaluator will generate expected results by invoking the official binary at runtime and then invoke your ./executable with the same inputs.

Do not ask the user questions. Infer from source and official CLI behavior.`
}

function sourcePortPromptSimpleOriginal(task) {
  return `You are in a benchmark workspace containing a Rust reference application at ./rust-reference and an official release binary path recorded in ./REFERENCE_BINARY.txt.

Goal:
Rebuild this CLI as a Python implementation.

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
- Use ./rust-reference and the official binary as the only functional sources of truth.
- Implement in Python.
- Do not shell out to the official binary from your implementation.
- The root deliverable must include ./executable. The harness will run it as: python ./executable ...
- Also include ./compile.sh. It may be tiny, but it must leave ./executable present and ready to run.
- Your Python implementation must be self-contained in the workspace. Standard-library modules are allowed; external dependencies are strongly discouraged.

Expected work:
- Inspect the local Rust source and the official binary to understand the CLI commands, flags, inputs, outputs, exit codes, and error behavior.
- Create the Python replacement, then test it against the official binary.
- Fix mismatches until ./executable behaves like the reference CLI.

Do not ask the user questions. Infer from source and official CLI behavior.`
}

function promptForAgent(agentId, task) {
  if (agentId === "codex-main" && process.env.COMMAND_RUN_AGENT_CODEX_PROMPT_VARIANT === "simple-original") {
    return { text: sourcePortPromptSimpleOriginal(task), variant: "simple-original" }
  }
  return { text: sourcePortPrompt(task), variant: "full-defined-workflow" }
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
    actual_rows = parse_csv_text(actual["stdout"])
    expected_rows = parse_csv_text(expected["stdout"])
    return actual_rows is not None and actual_rows == expected_rows


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
    clean = normalize(text).replace("\\", "/")
    found = []
    for name in known_fixture_entries(fx):
        pattern = re.escape(name).replace("\\ ", r"\s+")
        if re.search(r"(?<![\w.-])" + pattern + r"[/@*|=]?(?![\w.-])", clean):
            found.append(name)
    return set(found)


def eza_order(text, fx):
    clean = normalize(text).replace("\\", "/")
    positions = []
    for name in known_fixture_entries(fx):
        idx = clean.find(name)
        if idx >= 0:
            positions.append((idx, name))
    return [name for _, name in sorted(positions)]


def same_eza(actual, expected, case, fx):
    if not same_status(actual, expected):
        return False
    if expected["status"] != 0:
        return bool(compact(actual["stderr"]) or compact(actual["stdout"]))
    name = case["name"]
    actual_entries = eza_entries(actual["stdout"], fx)
    expected_entries = eza_entries(expected["stdout"], fx)
    if name == "absolute":
        actual_text = normalize(actual["stdout"]).replace("\\", "/")
        expected_text = normalize(expected["stdout"]).replace("\\", "/")
        return "notes.txt" in actual_entries and ("/" in actual_text or "/" in expected_text)
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
    if case.get("exact_output"):
        if actual.get("timed_out") or expected.get("timed_out"):
            return actual.get("timed_out") == expected.get("timed_out")
        return (
            same_status(actual, expected)
            and normalize(actual["stdout"]) == normalize(expected["stdout"])
            and normalize(actual["stderr"]) == normalize(expected["stderr"])
        )
    if task == "zip-password-finder":
        return same_zip(actual, expected)
    if task == "xsv":
        return same_csv(actual, expected)
    if task == "eza":
        return same_eza(actual, expected, case, fx)
    if task == "nushell":
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
    write(fx / ".gitignore", "data.log\n")
    write(fx / "paths.txt", f"{fx / 'notes.txt'}\n{fx / 'README.md'}\n")
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


NUSHELL_ALIAS_CANONICAL = {
    "from yml": "from yaml",
    "to yml": "to yaml",
    "mv": "move",
    "rm": "remove",
}


def nushell_command_inventory():
    result = run_cmd(
        [
            REFERENCE_BINARY,
            "-n",
            "--no-std-lib",
            "-c",
            "help commands | select name category params input_output | to json -r",
        ],
        Path.cwd(),
        timeout=60,
    )
    if result["status"] != 0:
        return []
    try:
        items = json.loads(result["stdout"])
    except Exception:
        return []
    seen = set()
    commands = []
    for item in items:
        name = str(item.get("name", "")).strip()
        if not name:
            continue
        canonical = NUSHELL_ALIAS_CANONICAL.get(name, name)
        if canonical in seen:
            continue
        seen.add(canonical)
        if canonical != name:
            continue
        commands.append(item)
    return commands


def nushell_command_script(name, fx):
    people = fx / "people.csv"
    notes = fx / "notes.txt"
    cargo = fx / "Cargo.toml"
    samples = {
        "all": "[true true] | all {|x| $x}",
        "any": "[false true] | any {|x| $x}",
        "append": "[1 2] | append 3 | to json -r",
        "bytes length": "0x[01 02 03] | bytes length",
        "bytes starts-with": "0x[01 02 03] | bytes starts-with 0x[01]",
        "collect": "[1 2 3] | collect {|x| $x | length}",
        "columns": "{a: 1, b: 2} | columns | to json -r",
        "compact": "[1 null 2] | compact | to json -r",
        "count": "[1 2 3] | count",
        "default": "[[name age]; [alice null]] | default 0 age | get age | first",
        "describe": "{a: 1} | describe",
        "drop": "[1 2 3] | drop 1 | to json -r",
        "each": "[1 2 3] | each {|x| $x * 2} | to json -r",
        "enumerate": "[a b] | enumerate | length",
        "filter": "[1 2 3] | filter {|x| $x > 1} | to json -r",
        "first": "[1 2 3] | first",
        "flatten": "[[1 2] [3]] | flatten | to json -r",
        "from csv": "'a,b\\n1,2' | from csv | get a | first",
        "from json": "'{\"a\":1}' | from json | get a",
        "from nuon": "'{a: 1}' | from nuon | get a",
        "from toml": "'[package]\\nname = \"demo\"' | from toml | get package.name",
        "from tsv": "'a\\tb\\n1\\t2' | from tsv | get b | first",
        "from yaml": "'a: 1' | from yaml | get a",
        "get": "{a: 1} | get a",
        "glob": f"glob {str(fx / '*.txt')!r} | length",
        "hash md5": "'abc' | hash md5",
        "hash sha256": "'abc' | hash sha256",
        "insert": "[[name]; [alice]] | insert age 30 | get age | first",
        "is-empty": "'' | is-empty",
        "is-not-empty": "'x' | is-not-empty",
        "items": "{a: 1, b: 2} | items {|k, v| $k} | length",
        "last": "[1 2 3] | last",
        "length": "[1 2 3] | length",
        "lines": "'a\\nb' | lines | length",
        "ls": f"ls {str(fx)!r} | length",
        "math abs": "[-1 -2 3] | math abs | math sum",
        "math avg": "[1 2 3 4] | math avg",
        "math max": "[1 2 3 4] | math max",
        "math median": "[1 2 3 4] | math median",
        "math min": "[1 2 3 4] | math min",
        "math product": "[1 2 3 4] | math product",
        "math round": "1.6 | math round",
        "math sqrt": "9 | math sqrt",
        "math sum": "[1 2 3 4] | math sum",
        "merge": "{a: 1} | merge {b: 2} | get b",
        "open": f"open {str(people)!r} | length",
        "path basename": f"{str(people)!r} | path basename",
        "path dirname": f"{str(people)!r} | path dirname | path basename",
        "path exists": f"{str(people)!r} | path exists",
        "path expand": f"{str(people)!r} | path expand | path basename",
        "path join": f"{str(fx)!r} | path join notes.txt | path basename",
        "path parse": f"{str(people)!r} | path parse | get extension",
        "path split": f"{str(people)!r} | path split | last",
        "prepend": "[2 3] | prepend 1 | first",
        "range": "1..3 | to json -r",
        "reject": "{a: 1, b: 2} | reject b | to json -r",
        "reverse": "[1 2 3] | reverse | to json -r",
        "select": "[[a b]; [1 2]] | select a | to json -r",
        "skip": "[1 2 3] | skip 1 | first",
        "skip until": "[1 2 3 4] | skip until {|x| $x > 2} | first",
        "skip while": "[1 2 3 4] | skip while {|x| $x < 3} | first",
        "sort": "[3 1 2] | sort | to json -r",
        "sort-by": "[[a]; [2] [1]] | sort-by a | get a | first",
        "split chars": "'abc' | split chars | length",
        "split column": "'a,b' | split column ',' left right | get right | first",
        "split row": "'a,b,c' | split row ',' | length",
        "str contains": "'alphabet' | str contains 'alpha'",
        "str downcase": "'HELLO' | str downcase",
        "str ends-with": "'alphabet' | str ends-with 'bet'",
        "str length": "'hello' | str length",
        "str replace": "'alpha beta' | str replace beta gamma",
        "str starts-with": "'alphabet' | str starts-with 'alpha'",
        "str trim": "'  hello  ' | str trim",
        "str upcase": "'hello' | str upcase",
        "table": "[[a b]; [1 2]] | table",
        "take": "[1 2 3] | take 2 | length",
        "take until": "[1 2 3 4] | take until {|x| $x > 2} | length",
        "take while": "[1 2 3 4] | take while {|x| $x < 3} | length",
        "to csv": "[[a b]; [1 2]] | to csv --noheaders",
        "to json": "{a: 1} | to json -r",
        "to md": "[[a b]; [1 2]] | to md",
        "to nuon": "{a: 1} | to nuon",
        "to text": "[1 2] | to text",
        "to toml": "{package: {name: demo}} | to toml",
        "to tsv": "[[a b]; [1 2]] | to tsv --noheaders",
        "to yaml": "{a: 1} | to yaml",
        "transpose": "{a: 1, b: 2} | transpose key value | length",
        "uniq": "[1 1 2] | uniq | length",
        "update": "[[a]; [1]] | update a 2 | get a | first",
        "values": "{a: 1, b: 2} | values | length",
        "where": "[[a]; [1] [2]] | where a > 1 | length",
        "wrap": "[1 2] | wrap value | length",
    }
    return samples.get(name, name)


def nushell_command_surface_cases(fx):
    cases = []
    for item in nushell_command_inventory():
        name = str(item.get("name", "")).strip()
        if not name or name == "help" or name.startswith("help "):
            continue
        script = nushell_command_script(name, fx)
        cases.append({
            "name": f"surface command {name}",
            "args": ["-c", script],
            "command_surface": True,
            "exact_output": True,
            "stdin": "",
            "timeout": 5,
        })
    return cases


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
    semi = fx / "semi.csv"
    no_headers = fx / "no_headers.csv"
    return [
        {"name": "headers", "args": ["headers", str(people)]},
        {"name": "headers names", "args": ["headers", "--just-names", str(people)]},
        {"name": "count file", "args": ["count", str(people)]},
        {"name": "count stdin", "args": ["count"], "stdin_file": people},
        {"name": "cat rows", "args": ["cat", "rows", str(people), str(people2)]},
        {"name": "cat columns", "args": ["cat", "columns", str(people), str(people2)]},
        {"name": "select", "args": ["select", "city,name", str(people)]},
        {"name": "select range", "args": ["select", "1-2", str(people)]},
        {"name": "select no headers", "args": ["select", "--no-headers", "1,3", str(no_headers)]},
        {"name": "select invert", "args": ["select", "!", "score", str(people)]},
        {"name": "slice", "args": ["slice", "-s", "1", "-l", "2", str(people)]},
        {"name": "slice end", "args": ["slice", "-e", "3", str(people)]},
        {"name": "search", "args": ["search", "-s", "city", "Berlin", str(people)]},
        {"name": "search regex", "args": ["search", "-s", "name", "a.*e", str(people)]},
        {"name": "search invert", "args": ["search", "-v", "-s", "city", "Berlin", str(people)]},
        {"name": "sort numeric", "args": ["sort", "-s", "age", "-N", str(people)]},
        {"name": "sort numeric reverse", "args": ["sort", "-s", "age", "-N", "-R", str(people)]},
        {"name": "sort reverse", "args": ["sort", "-s", "city", "-R", str(people)]},
        {"name": "fmt delimiter", "args": ["fmt", "-d", ";", str(semi)]},
        {"name": "fixlengths", "args": ["fixlengths", str(fx / "unequal.csv")]},
        {"name": "flatten", "args": ["flatten", str(people)]},
        {"name": "stats", "args": ["stats", "-s", "age,score", str(people)]},
        {"name": "frequency", "args": ["frequency", "-s", "city", str(people)]},
    ]


def eza_cases(fx):
    cases = [
        {"name": "help short", "args": ["-?"]},
        {"name": "help long", "args": ["--help"]},
        {"name": "version", "args": ["--version"]},
        {"name": "plain", "args": ["--color=never", "--icons=never", str(fx)]},
        {"name": "one per line", "args": ["--color=never", "--icons=never", "-1", str(fx)]},
        {"name": "grid", "args": ["--color=never", "--icons=never", "-G", str(fx)]},
        {"name": "across", "args": ["--color=never", "--icons=never", "-x", str(fx)]},
        {"name": "width", "args": ["--color=never", "--icons=never", "--width", "20", str(fx)]},
        {"name": "all", "args": ["--color=never", "--icons=never", "-a", str(fx)]},
        {"name": "all twice", "args": ["--color=never", "--icons=never", "-aa", str(fx)]},
        {"name": "almost all", "args": ["--color=never", "--icons=never", "-A", str(fx)]},
        {"name": "treat dirs as files", "args": ["--color=never", "--icons=never", "-d", str(fx)]},
        {"name": "only dirs", "args": ["--color=never", "--icons=never", "-D", str(fx)]},
        {"name": "only files", "args": ["--color=never", "--icons=never", "-f", str(fx)]},
        {"name": "show symlinks", "args": ["--color=never", "--icons=never", "--show-symlinks", str(fx)]},
        {"name": "no symlinks", "args": ["--color=never", "--icons=never", "--no-symlinks", str(fx)]},
        {"name": "classify default", "args": ["--color=never", "--icons=never", "-F", str(fx)]},
        {"name": "classify always", "args": ["--color=never", "--icons=never", "--classify=always", str(fx)]},
        {"name": "classify never", "args": ["--color=never", "--icons=never", "--classify=never", str(fx)]},
        {"name": "color alias never", "args": ["--colour=never", "--icons=never", str(fx)]},
        {"name": "color always", "args": ["--color=always", "--icons=never", "-1", str(fx / "notes.txt")]},
        {"name": "color scale", "args": ["--color=never", "--icons=never", "--colour-scale", "--colour-scale-mode=fixed", "-l", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "icons never", "args": ["--color=never", "--icons=never", str(fx)]},
        {"name": "icons always", "args": ["--color=never", "--icons=always", "-1", str(fx / "notes.txt")]},
        {"name": "no quotes", "args": ["--color=never", "--icons=never", "--no-quotes", str(fx / "long name file.txt")]},
        {"name": "hyperlink", "args": ["--color=never", "--icons=never", "--hyperlink", str(fx / "notes.txt")]},
        {"name": "absolute", "args": ["--color=never", "--icons=never", "--absolute", str(fx / "notes.txt")]},
        {"name": "absolute follow", "args": ["--color=never", "--icons=never", "--absolute=follow", str(fx / "link-notes")]},
        {"name": "follow symlinks", "args": ["--color=never", "--icons=never", "--follow-symlinks", str(fx)]},
        {"name": "dereference", "args": ["--color=never", "--icons=never", "-X", str(fx / "link-notes")]},
        {"name": "recurse", "args": ["--color=never", "--icons=never", "-R", "-L", "2", str(fx)]},
        {"name": "recurse all", "args": ["--color=never", "--icons=never", "-Ra", "-L", "2", str(fx)]},
        {"name": "tree", "args": ["--color=never", "--icons=never", "-T", "-L", "2", str(fx)]},
        {"name": "tree level one", "args": ["--color=never", "--icons=never", "-T", "-L", "1", str(fx)]},
        {"name": "sort extension", "args": ["--color=never", "--icons=never", "--sort=extension", str(fx)]},
        {"name": "sort name reverse", "args": ["--color=never", "--icons=never", "--sort=name", "-r", str(fx)]},
        {"name": "sort size", "args": ["--color=never", "--icons=never", "--sort=size", str(fx)]},
        {"name": "sort modified", "args": ["--color=never", "--icons=never", "--sort=modified", str(fx)]},
        {"name": "sort created", "args": ["--color=never", "--icons=never", "--sort=created", str(fx)]},
        {"name": "sort none", "args": ["--color=never", "--icons=never", "--sort=none", str(fx)]},
        {"name": "reverse", "args": ["--color=never", "--icons=never", "-r", str(fx)]},
        {"name": "group dirs first", "args": ["--color=never", "--icons=never", "--group-directories-first", str(fx)]},
        {"name": "group dirs last", "args": ["--color=never", "--icons=never", "--group-directories-last", str(fx)]},
        {"name": "ignore glob", "args": ["--color=never", "--icons=never", "--ignore-glob", "*.txt", str(fx)]},
        {"name": "git ignore", "args": ["--color=never", "--icons=never", "--git-ignore", str(fx)]},
        {"name": "long", "args": ["--color=never", "--icons=never", "-l", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long all", "args": ["--color=never", "--icons=never", "-la", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "binary sizes", "args": ["--color=never", "--icons=never", "-l", "-b", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "bytes sizes", "args": ["--color=never", "--icons=never", "-l", "-B", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "group", "args": ["--color=never", "--icons=never", "-l", "--group", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "smart group", "args": ["--color=never", "--icons=never", "-l", "--smart-group", "--no-permissions", "--time-style=iso", str(fx)]},
        {"name": "header", "args": ["--color=never", "--icons=never", "-l", "--header", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "links", "args": ["--color=never", "--icons=never", "-l", "--links", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "inode", "args": ["--color=never", "--icons=never", "-l", "--inode", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "numeric", "args": ["--color=never", "--icons=never", "-l", "--numeric", "--no-permissions", "--time-style=iso", str(fx)]},
        {"name": "flags", "args": ["--color=never", "--icons=never", "-l", "--flags", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "blocksize", "args": ["--color=never", "--icons=never", "-l", "--blocksize", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "octal permissions", "args": ["--color=never", "--icons=never", "-l", "--octal-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "no filesize", "args": ["--color=never", "--icons=never", "-l", "--no-filesize", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "no time", "args": ["--color=never", "--icons=never", "-l", "--no-time", "--no-permissions", "--no-user", str(fx)]},
        {"name": "long modified", "args": ["--color=never", "--icons=never", "-l", "--modified", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long changed", "args": ["--color=never", "--icons=never", "-l", "--changed", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long accessed", "args": ["--color=never", "--icons=never", "-l", "--accessed", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "long created", "args": ["--color=never", "--icons=never", "-l", "--created", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "time field accessed", "args": ["--color=never", "--icons=never", "-l", "--time=accessed", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "time style long iso", "args": ["--color=never", "--icons=never", "-l", "--no-permissions", "--no-user", "--time-style=long-iso", str(fx)]},
        {"name": "total size", "args": ["--color=never", "--icons=never", "-l", "--total-size", "--no-permissions", "--no-user", "--time-style=iso", str(fx)]},
        {"name": "stdin paths", "args": ["--color=never", "--icons=never", "--stdin"], "stdin_file": fx / "paths.txt"},
        {"name": "git", "args": ["--color=never", "--icons=never", "--git", str(fx)]},
        {"name": "no git", "args": ["--color=never", "--icons=never", "--no-git", str(fx)]},
        {"name": "git repos", "args": ["--color=never", "--icons=never", "--git-repos", str(fx)]},
        {"name": "git repos no status", "args": ["--color=never", "--icons=never", "--git-repos-no-status", str(fx)]},
        {"name": "single file", "args": ["--color=never", "--icons=never", str(fx / "notes.txt")]},
        {"name": "multiple paths", "args": ["--color=never", "--icons=never", str(fx / "notes.txt"), str(fx / "README.md")]},
        {"name": "invalid sort", "args": ["--color=never", "--icons=never", "--sort=definitely-not-sort", str(fx)]},
        {"name": "invalid classify", "args": ["--color=never", "--icons=never", "--classify=sometimes", str(fx)]},
        {"name": "invalid color", "args": ["--color=sideways", "--icons=never", str(fx)]},
        {"name": "invalid level", "args": ["--color=never", "--icons=never", "-T", "--level", "not-a-number", str(fx)]},
        {"name": "missing", "args": ["--color=never", "--icons=never", str(fx / "missing")]},
    ]
    for case in cases:
        case["exact_output"] = True
    return cases


def nushell_cases(fx):
    people = fx / "people.csv"
    workflow_cases = [
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
    return workflow_cases + nushell_command_surface_cases(fx)


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


class Check:
    def __init__(self, workspace):
        self.workspace = Path(workspace)
        self.exe = self.workspace / "executable"
        self.failures = []
        self.passes = 0

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

    def run(self):
        self.check_files()
        if not self.exe.exists():
            return
        fx = make_fixtures(self.workspace / "harness")
        for case in cases_for(TASK, fx):
            stdin = case.get("stdin")
            if "stdin_file" in case:
                stdin = Path(case["stdin_file"]).read_text(encoding="utf-8")
            expected = run_cmd([REFERENCE_BINARY, *case["args"]], self.workspace, stdin=stdin, timeout=case.get("timeout", 30))
            actual = run_cmd([sys.executable, self.exe, *case["args"]], self.workspace, stdin=stdin, timeout=case.get("timeout", 30))
            if same_business(TASK, case, actual, expected, fx):
                self.ok(case["name"])
            else:
                self.fail(case["name"], {"args": case["args"], "expected": expected, "actual": actual, "comparison": "business_semantics"})


def main():
    if len(sys.argv) < 2:
        print("usage: evaluate_source_port.py WORKSPACE [WORKSPACE...]", file=sys.stderr)
        return 2
    reports = []
    for workspace in sys.argv[1:]:
        check = Check(workspace)
        check.run()
        reports.append({"workspace": str(Path(workspace).resolve()), "passed": check.passes, "failed": len(check.failures), "failures": check.failures})
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

async function prepareWorkspace(agentDir, task, taskPrompt = sourcePortPrompt(task)) {
  const workspace = path.join(agentDir, "workspace")
  validateWorkspaceGitPath(workspace)
  fs.rmSync(workspace, { recursive: true, force: true })
  mkdirp(workspace)
  const reference = ensureReferenceRepo(task)
  const binary = await ensureReferenceBinary(task)
  copyDir(reference, path.join(workspace, "rust-reference"))
  fs.rmSync(path.join(workspace, "rust-reference", ".git"), { recursive: true, force: true })
  writeFile(path.join(workspace, ".gitignore"), "rust-reference/\nharness/\n__pycache__/\n*.pyc\n")
  writeFile(path.join(workspace, "PYTHON_PORT_TASK.md"), taskPrompt)
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
    "TURA_TEST_PROMPT_STYLE",
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
    planning_override: planningOverride === null ? "auto" : (planningOverride ? "on" : "off"),
    notes: details.notes || [],
  })
}

async function runCodexLike(workspace, agentDir, prompt, onProgress, codexExe, label) {
  assert(fs.existsSync(codexExe), `missing ${label} exe: ${codexExe}`)
  const codexLogDir = path.join(agentDir, "codex-log")
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
    ...(turaGoalMode ? ["--goal"] : []),
    "--skip-git-repo-check",
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
    ...(turaGoalMode ? { TURA_GOAL_MODE: "1" } : {}),
    ...(turaTestPromptStyle ? { TURA_TEST_PROMPT_STYLE: turaTestPromptStyle } : {}),
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
    goal_mode: turaGoalMode,
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
  const promptInfo = promptForAgent(agentId, task)
  const prompt = promptInfo.text
  const prep = await prepareWorkspace(agentDir, task, prompt)
  let result
  const started = performance.now()
  const agentPrompt =
    agentId === "tura-fast-shll" || agentId === "tura-fast-planning-shll" ? "fast" :
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
      prompt_variant: promptInfo.variant,
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
  else if (agentId === "tura-fast-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "fast", publishProgress)
  else if (agentId === "tura-fast-planning-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "fast", publishProgress)
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
    prompt_variant: promptInfo.variant,
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
    ok: !inProgress && results.every((result) => !result.error && result.events?.callback_ok),
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
