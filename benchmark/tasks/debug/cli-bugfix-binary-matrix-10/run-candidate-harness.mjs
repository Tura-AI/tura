import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import { spawnSync } from "node:child_process"
import { fileURLToPath } from "node:url"
import { loadOracleMatrix, oracleMatrixForTask, runCandidateAudit } from "./harness.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const tasksPath = path.join(scriptDir, "tasks.json")
const matrixConfig = loadOracleMatrix(scriptDir)
const taskConfig = JSON.parse(fs.readFileSync(tasksPath, "utf8"))
const tasksById = new Map(taskConfig.tasks.map((task) => [task.id, task]))

const args = parseArgs(process.argv.slice(2))
const runRoot = path.resolve(requiredArg(args, "run-root"))
const progressPath = path.join(runRoot, "all-candidate-harness-progress.json")
const summaryPath = path.join(runRoot, "all-candidate-harness-summary.json")
const agentProgressPath = path.join(runRoot, "agent-progress.json")
const agentProgress = JSON.parse(fs.readFileSync(agentProgressPath, "utf8"))
const selected = agentProgress.results.filter((row) => tasksById.has(row.task_id || row.task))

const toolsRoot = path.join(os.homedir(), "Documents", "tura_workspace", "tools")
const mavenCmd = path.join(toolsRoot, "maven", "apache-maven-3.9.9", "bin", process.platform === "win32" ? "mvn.cmd" : "mvn")
const graalRoot = path.join(toolsRoot, "graalvm-jdk-21.0.2")
const graalHome = findAncestorDir(findFile(graalRoot, process.platform === "win32" ? "native-image.cmd" : "native-image"), "bin")
const vcvars64 = firstExisting([
  "C:\\Program Files\\Microsoft Visual Studio\\18\\Community\\VC\\Auxiliary\\Build\\vcvars64.bat",
  "C:\\Program Files\\Microsoft Visual Studio\\18\\Insiders\\VC\\Auxiliary\\Build\\vcvars64.bat",
  "C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Auxiliary\\Build\\vcvars64.bat",
])
const binaryCache = path.join(runRoot, "_candidate-oracle-binary-cache")
process.env.COMMAND_RUN_AGENT_BINARY_CACHE_ROOT = binaryCache

const matrixTotalsByTask = new Map()
for (const row of selected) {
  const task = tasksById.get(row.task_id || row.task)
  const matrix = oracleMatrixForTask(task, matrixConfig)
  matrixTotalsByTask.set(task.id, {
    fail_to_pass_total: matrix.failToPass.length,
    pass_to_pass_total: matrix.passToPass.length,
    total: matrix.failToPass.length + matrix.passToPass.length,
  })
}

const rows = selected.map((agentRow) => {
  const taskId = agentRow.task_id || agentRow.task
  return {
    task: taskId,
    label: agentRow.label,
    agent: agentRow.agent_id || agentRow.agent,
    agent_kind: agentRow.agent_kind,
    agent_completed: agentRow.phase === "agent_completed" && agentRow.exit_code === 0,
    agent_exit_code: agentRow.exit_code,
    workspace: agentRow.workspace,
    status: "pending",
    build: null,
    harness: emptyHarness(matrixTotalsByTask.get(taskId), "pending"),
    usage: agentRow.usage || null,
    events: agentRow.events || null,
    log_completeness: logCompleteness(agentRow),
  }
})

writeState("starting")

for (const row of rows) {
  const task = tasksById.get(row.task)
  const runDir = path.join(runRoot, "agent-runs", row.task, row.agent, "candidate-harness-full")
  mkdirp(runDir)
  try {
    row.status = "building"
    writeState(`building ${row.task}/${row.agent}`)
    const built = buildCandidate(task, row, runDir)
    row.build = built.build
    row.candidate_binary = built.candidateBinary
    if (!built.ok) {
      row.status = "build_failed"
      row.harness = emptyHarness(matrixTotalsByTask.get(row.task), built.reason)
      writeState(`build failed ${row.task}/${row.agent}`)
      continue
    }

    row.status = "harness_running"
    writeState(`harness ${row.task}/${row.agent}`)
    const auditRunDir = shortAuditRunDir(row)
    resetChildDir(auditRunDir, shortAuditRoot())
    const report = await runCandidateAudit({
      task,
      matrixConfig,
      runRoot: auditRunDir,
      candidateBinary: built.candidateBinary,
    })
    row.harness = summarizeAudit(report, path.join(auditRunDir, "candidate-audit", safeName(task.id), "candidate-audit.json"))
    row.status = row.harness.ok ? "passed" : "failed"
    writeState(`done ${row.task}/${row.agent}`)
  } catch (error) {
    row.status = "error"
    row.error = String(error?.stack || error?.message || error)
    row.harness = emptyHarness(matrixTotalsByTask.get(row.task), row.error)
    writeState(`error ${row.task}/${row.agent}`)
  }
}

const summary = summaryObject("complete")
writeJson(summaryPath, summary)
writeJson(progressPath, summary)
console.log(JSON.stringify({
  summary_path: summaryPath,
  ok: summary.ok,
  rows: summary.results.map((row) => ({
    task: row.task,
    agent: row.agent,
    status: row.status,
    score: row.harness.score,
    passed: row.harness.passed,
    total: row.harness.total,
    reason: row.harness.reason,
  })),
}, null, 2))

function buildCandidate(task, row, runDir) {
  const buildLog = path.join(runDir, "build.log")
  const workspace = row.workspace
  if (!fs.existsSync(workspace)) {
    return buildFailure(buildLog, `workspace missing: ${workspace}`)
  }
  if (task.id === "eza-grid-non-tty") {
    const targetRoot = process.platform === "win32"
      ? path.join("C:\\", "tura-candidate-builds")
      : path.join(os.tmpdir(), "tura-candidate-builds")
    const targetDir = path.join(targetRoot, `eza-${safeName(row.agent)}`)
    const binary = path.join(targetDir, "release", process.platform === "win32" ? "eza.exe" : "eza")
    if (truthy(process.env.COMMAND_RUN_AGENT_REUSE_CANDIDATE_BUILDS) && fs.existsSync(binary)) {
      fs.writeFileSync(buildLog, `reused existing candidate binary: ${binary}\n`, "utf8")
      return buildSuccess(buildLog, binary, "reuse-existing-binary", [], {
        status: 0,
        signal: null,
        error: null,
        started_at: new Date().toISOString(),
        ended_at: new Date().toISOString(),
      })
    }
    resetChildDir(targetDir, targetRoot)
    const command = "cargo"
    const args = ["build", "--release", "--bin", "eza", "-j", "1"]
    const result = runLogged(command, args, {
      cwd: workspace,
      env: { ...process.env, CARGO_TARGET_DIR: targetDir },
      timeoutMs: 45 * 60_000,
      logPath: buildLog,
    })
    if (result.status !== 0) return buildFailure(buildLog, `cargo build failed with ${result.status}`, command, args, result)
    if (!fs.existsSync(binary)) return buildFailure(buildLog, `candidate binary missing after build: ${binary}`, command, args, result)
    return buildSuccess(buildLog, binary, command, args, result)
  }

  if (task.id === "google-java-format-native-reflection") {
    const binary = path.join(workspace, "core", "target", process.platform === "win32" ? "google-java-format.exe" : "google-java-format")
    const command = process.platform === "win32" ? path.join(runDir, "build-native.cmd") : mavenCmd
    const args = process.platform === "win32" ? [] : ["-pl", "core", "-Pnative", "-DskipTests", "package"]
    const missing = []
    if (!fs.existsSync(mavenCmd)) missing.push(`maven missing: ${mavenCmd}`)
    if (!graalHome || !fs.existsSync(path.join(graalHome, "bin", process.platform === "win32" ? "native-image.cmd" : "native-image"))) missing.push(`GraalVM native-image missing under ${graalRoot}`)
    if (process.platform === "win32" && !vcvars64) missing.push("vcvars64.bat missing")
    if (missing.length > 0) return buildFailure(buildLog, missing.join("; "), command, args)
    if (process.platform === "win32") {
      fs.writeFileSync(command, [
        "@echo off",
        `call "${vcvars64}"`,
        `set "JAVA_HOME=${graalHome}"`,
        `set "GRAALVM_HOME=${graalHome}"`,
        `set "PATH=${path.join(graalHome, "bin")};${path.dirname(mavenCmd)};%PATH%"`,
        `call "${mavenCmd}" -pl core -Pnative -DskipTests package`,
        "",
      ].join("\r\n"), "utf8")
    }
    const env = {
      ...process.env,
      JAVA_HOME: graalHome,
      GRAALVM_HOME: graalHome,
      PATH: [path.join(graalHome, "bin"), path.dirname(mavenCmd), process.env.PATH || ""].join(path.delimiter),
    }
    const result = runLogged(command, args, {
      cwd: workspace,
      env,
      timeoutMs: 60 * 60_000,
      logPath: buildLog,
      shell: process.platform === "win32",
    })
    if (result.status !== 0) return buildFailure(buildLog, `native Maven build failed with ${result.status}`, command, args, result)
    if (!fs.existsSync(binary)) return buildFailure(buildLog, `candidate native binary missing after build: ${binary}`, command, args, result)
    return buildSuccess(buildLog, binary, command, args, result)
  }

  return buildFailure(buildLog, `unsupported task for candidate harness runner: ${task.id}`)
}

function runLogged(command, args, options) {
  const startedAt = new Date().toISOString()
  const header = [
    `started_at=${startedAt}`,
    `cwd=${options.cwd}`,
    `command=${command} ${args.join(" ")}`,
    "",
  ].join("\n")
  fs.writeFileSync(options.logPath, header, "utf8")
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env || process.env,
    encoding: "utf8",
    errors: "replace",
    timeout: options.timeoutMs,
    maxBuffer: 256 * 1024 * 1024,
    shell: Boolean(options.shell),
    windowsHide: true,
  })
  const endedAt = new Date().toISOString()
  const output = [
    "===== STDOUT =====",
    result.stdout || "",
    "===== STDERR =====",
    result.stderr || "",
    result.error ? `ERROR=${result.error.message || result.error}` : "",
    `ended_at=${endedAt}`,
  ].join("\n")
  fs.appendFileSync(options.logPath, output, "utf8")
  return {
    status: result.status === null ? 124 : result.status,
    signal: result.signal || null,
    error: result.error ? String(result.error.message || result.error) : null,
    started_at: startedAt,
    ended_at: endedAt,
  }
}

function summarizeAudit(report, reportPath) {
  const oracle = report.oracle || {}
  const f2p = oracle.fail_to_pass_passed || 0
  const p2p = oracle.pass_to_pass_passed || 0
  const f2pTotal = oracle.fail_to_pass_total || 0
  const p2pTotal = oracle.pass_to_pass_total || 0
  const passed = f2p + p2p
  const total = f2pTotal + p2pTotal
  return {
    ran: true,
    ok: Boolean(oracle.ok),
    report_path: reportPath,
    fail_to_pass_passed: f2p,
    fail_to_pass_total: f2pTotal,
    pass_to_pass_passed: p2p,
    pass_to_pass_total: p2pTotal,
    passed,
    total,
    score: total ? passed / total : 0,
    score_percent: total ? Number((100 * passed / total).toFixed(2)) : 0,
    failed: oracle.failed || [],
    reason: oracle.ok ? "pass" : "oracle mismatches",
  }
}

function emptyHarness(totals, reason) {
  const total = totals?.total || 0
  return {
    ran: false,
    ok: false,
    report_path: null,
    fail_to_pass_passed: 0,
    fail_to_pass_total: totals?.fail_to_pass_total || 0,
    pass_to_pass_passed: 0,
    pass_to_pass_total: totals?.pass_to_pass_total || 0,
    passed: 0,
    total,
    score: 0,
    score_percent: 0,
    failed: [],
    reason,
  }
}

function buildSuccess(logPath, candidateBinary, command, args, result) {
  return {
    ok: true,
    candidateBinary,
    build: {
      status: "passed",
      command: [command, ...args],
      log_path: logPath,
      exit_code: result.status,
      signal: result.signal,
      error: result.error,
      started_at: result.started_at,
      ended_at: result.ended_at,
    },
  }
}

function buildFailure(logPath, reason, command = null, args = [], result = null) {
  if (!fs.existsSync(logPath)) fs.writeFileSync(logPath, `${reason}\n`, "utf8")
  return {
    ok: false,
    reason,
    candidateBinary: null,
    build: {
      status: "failed",
      reason,
      command: command ? [command, ...args] : null,
      log_path: logPath,
      exit_code: result?.status ?? null,
      signal: result?.signal ?? null,
      error: result?.error ?? null,
      started_at: result?.started_at ?? null,
      ended_at: result?.ended_at ?? null,
    },
  }
}

function logCompleteness(row) {
  const archive = row.context_archive || {}
  const paths = {
    prompt_path: row.prompt_path,
    stdout_path: row.stdout_path,
    stderr_path: row.stderr_path,
    workspace: row.workspace,
    provider_log_path: row.provider_log_path,
    input_prompt_path: archive.input_prompt_path,
    stdout_snapshot_path: archive.stdout_snapshot_path,
    provider_calls_full_path: archive.provider_calls_full_path,
    codex_rollout_paths_path: archive.codex_rollout_paths_path,
    patch_path: row.patch?.patch_path,
    status_path: row.patch?.status_path,
  }
  const exists = Object.fromEntries(Object.entries(paths).map(([key, value]) => [key, value ? fs.existsSync(value) : false]))
  return {
    required_paths: exists,
    usage_present: Boolean(row.usage && row.usage.total_tokens !== undefined),
    usage_source: row.usage_source || null,
    provider_call_count: archive.provider_call_count ?? row.provider_calls?.count ?? null,
    provider_calls_complete: archive.provider_calls_complete ?? null,
    callback_ok: Boolean(row.events?.callback_ok),
    llm_rounds: row.events?.llm_rounds ?? null,
    command_executions: row.events?.command_executions ?? null,
  }
}

function summaryObject(status) {
  const totals = rows.reduce((acc, row) => {
    const usage = row.usage || {}
    for (const key of ["input_tokens", "cached_input_tokens", "output_tokens", "reasoning_tokens", "total_tokens", "usage_events"]) {
      acc[key] = (acc[key] || 0) + (usage[key] || 0)
    }
    return acc
  }, {})
  const harnessTotals = rows.reduce((acc, row) => {
    acc.passed += row.harness.passed || 0
    acc.total += row.harness.total || 0
    acc.fail_to_pass_passed += row.harness.fail_to_pass_passed || 0
    acc.fail_to_pass_total += row.harness.fail_to_pass_total || 0
    acc.pass_to_pass_passed += row.harness.pass_to_pass_passed || 0
    acc.pass_to_pass_total += row.harness.pass_to_pass_total || 0
    return acc
  }, { passed: 0, total: 0, fail_to_pass_passed: 0, fail_to_pass_total: 0, pass_to_pass_passed: 0, pass_to_pass_total: 0 })
  return {
    schema: "tura.debug.cli-bugfix-all-candidate-harness.v1",
    status,
    run_root: runRoot,
    generated_at: new Date().toISOString(),
    ok: rows.every((row) => row.harness.ok),
    token_totals: totals,
    harness_totals: {
      ...harnessTotals,
      score: harnessTotals.total ? harnessTotals.passed / harnessTotals.total : 0,
      score_percent: harnessTotals.total ? Number((100 * harnessTotals.passed / harnessTotals.total).toFixed(2)) : 0,
    },
    tools: {
      maven_cmd: mavenCmd,
      graal_home: graalHome,
      vcvars64,
      binary_cache: binaryCache,
    },
    results: rows,
  }
}

function writeState(status) {
  writeJson(progressPath, summaryObject(status))
}

function parseArgs(argv) {
  const parsed = {}
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (!arg.startsWith("--")) continue
    parsed[arg.slice(2)] = argv[index + 1]
    index += 1
  }
  return parsed
}

function requiredArg(args, key) {
  if (!args[key]) throw new Error(`missing --${key}`)
  return args[key]
}

function findFile(root, name) {
  if (!fs.existsSync(root)) return null
  const stack = [root]
  while (stack.length > 0) {
    const current = stack.pop()
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(full)
      else if (entry.isFile() && entry.name.toLowerCase() === name.toLowerCase()) return full
    }
  }
  return null
}

function findAncestorDir(file, childName) {
  if (!file) return null
  let current = path.dirname(file)
  if (path.basename(current).toLowerCase() === childName.toLowerCase()) return path.dirname(current)
  return null
}

function firstExisting(candidates) {
  return candidates.find((candidate) => fs.existsSync(candidate)) || null
}

function shortAuditRoot() {
  if (process.platform === "win32") return path.join("C:\\", "tura-candidate-audits")
  return path.join(os.tmpdir(), "tura-candidate-audits")
}

function shortAuditRunDir(row) {
  return path.join(shortAuditRoot(), `${safeName(row.task)}-${safeName(row.agent)}`)
}

function resetChildDir(target, allowedRoot) {
  const resolved = path.resolve(target)
  const root = path.resolve(allowedRoot)
  if (!(resolved === root || resolved.startsWith(`${root}${path.sep}`))) throw new Error(`refusing to remove outside ${root}: ${resolved}`)
  fs.rmSync(resolved, { recursive: true, force: true })
  mkdirp(resolved)
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeJson(file, value) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8")
}

function safeName(value) {
  return String(value).replace(/[^A-Za-z0-9_.-]+/g, "-").replace(/^-+|-+$/g, "") || "item"
}

function truthy(value) {
  return ["1", "true", "yes", "on", "enabled"].includes(String(value || "").trim().toLowerCase())
}
