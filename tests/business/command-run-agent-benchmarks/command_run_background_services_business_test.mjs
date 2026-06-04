#!/usr/bin/env node
import fs from "node:fs"
import http from "node:http"
import { spawn, spawnSync } from "node:child_process"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { isolatedProcessOptions, killProcessTree } from "./process_helpers.mjs"

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..")
const runId = process.env.COMMAND_RUN_BACKGROUND_SERVICES_RUN_ID || `background-services-${Date.now()}`
const runRoot = path.join(repoRoot, "target", "command-run-background-services", runId)
const timeoutMs = Number(process.env.COMMAND_RUN_BACKGROUND_SERVICES_TIMEOUT_MS || 240_000)
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || "openai/gpt-5.5"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const agents = (process.env.COMMAND_RUN_BACKGROUND_SERVICES_AGENTS || "tura-fast-shll,tura-shll,tura-fast-bash,tura-bash")
  .split(",")
  .map((item) => item.trim())
  .filter(Boolean)

const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text)
}

function run(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    shell: options.shell || false,
    windowsHide: true,
    timeout: options.timeoutMs || 120_000,
  })
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

function agentPrompt(agentId) {
  return agentId.includes("fast") ? "fast" : "coding_agent"
}

function shellSurface(agentId) {
  return agentId.endsWith("-bash") ? "bash" : "shell_command"
}

function agentWorkspace(agentId) {
  return path.join(runRoot, agentId, "workspace")
}

function createWorkspace(workspace, basePort) {
  mkdirp(workspace)
  writeFile(
    path.join(workspace, "server.mjs"),
    `import http from "node:http";

const port = Number(process.argv[2]);
const name = process.argv[3] || "service";
const server = http.createServer((req, res) => {
  if (req.url === "/ready") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ ok: true, name, port, pid: process.pid }));
    return;
  }
  res.writeHead(200, { "content-type": "text/plain" });
  res.end(name + "\\n");
});

server.listen(port, "127.0.0.1", () => {
  console.log(JSON.stringify({ ready: true, name, port, pid: process.pid }));
});

function shutdown() {
  server.close(() => process.exit(0));
  setTimeout(() => process.exit(0), 1000).unref();
}

process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
`,
  )
  writeFile(
    path.join(workspace, "README.md"),
    `Background service fixture.

Use node server.mjs <port> <name> to start a persistent local HTTP service.
The two assigned ports are ${basePort} and ${basePort + 1}.
`,
  )
}

function promptFor(basePort, surface) {
  const first = basePort
  const second = basePort + 1
  return `You are testing command_run background service behavior.

In this workspace, start two persistent local Node HTTP services from server.mjs on ports ${first} and ${second}. They must be background services, not foreground blocking commands. Start both services in one command_run batch if possible, wait until both /ready endpoints return HTTP 200, run probes against both endpoints, then stop both services in cleanup.

The active shell surface is ${surface}. Use a reliable background process approach for that surface, keep enough process identity to stop both services, wait for readiness with bounded probes, and clean up even if a probe fails.

Write service-results.json with this shape:
{
  "ok": true,
  "surface": "${surface}",
  "service_count": 2,
  "ports": [${first}, ${second}],
  "ready": [true, true],
  "foreground_blocked": false,
  "cleanup_done": true
}

Do not leave either service running. Finish with a short summary.`
}

async function spawnLogged(command, args, options = {}) {
  mkdirp(path.dirname(options.stdoutPath))
  const started = performance.now()
  let stdout = ""
  let stderr = ""
  let timedOut = false
  let settled = false
  let childExitStatus = null
  let childExitSignal = null
  const timeoutLimitMs = options.timeoutMs || timeoutMs
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
  child.stdout.on("data", (chunk) => {
    const text = chunk.toString("utf8")
    stdout += text
    fs.appendFileSync(options.stdoutPath, text)
  })
  child.stderr.on("data", (chunk) => {
    const text = chunk.toString("utf8")
    stderr += text
    fs.appendFileSync(options.stderrPath, text)
  })
  const timer = setTimeout(() => {
    timedOut = true
    killProcessTree(child.pid)
    setTimeout(() => {
      settle(childExitStatus ?? 1, childExitSignal)
    }, Number(options.timeoutCloseGraceMs || 3_000))
  }, timeoutLimitMs)
  function settle(status, signal, error = null) {
    if (settled) return
    settled = true
    clearTimeout(timer)
    resolveResult({
      status,
      signal,
      stdout,
      stderr,
      timed_out: timedOut,
      duration_ms: Math.round(performance.now() - started),
      error: error || (timedOut ? `timed out after ${timeoutLimitMs}ms` : null),
    })
  }
  let resolveResult
  return await new Promise((resolve) => {
    resolveResult = resolve
    child.on("exit", (status, signal) => {
      childExitStatus = status
      childExitSignal = signal
      setTimeout(() => settle(timedOut ? (status ?? 1) : status, signal), Number(options.exitCloseGraceMs || 1_000))
    })
    child.on("close", (status, signal) => {
      settle(timedOut ? (status ?? 1) : status, signal)
    })
    child.on("error", (error) => {
      settle(null, null, String(error.stack || error.message || error))
    })
  })
}

function cleanupWorkspaceServices(workspace) {
  const marker = path.join(workspace, "server.mjs")
  if (process.platform === "win32") {
    const script = `$marker = ${JSON.stringify(marker)}; Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like "*$marker*" } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }`
    run("powershell", ["-NoProfile", "-Command", script], { timeoutMs: 30_000 })
  } else {
    run("pkill", ["-f", marker], { timeoutMs: 30_000 })
  }
}

function portReady(port) {
  return new Promise((resolve) => {
    const req = http.get({ host: "127.0.0.1", port, path: "/ready", timeout: 500 }, (res) => {
      res.resume()
      resolve(res.statusCode === 200)
    })
    req.on("timeout", () => {
      req.destroy()
      resolve(false)
    })
    req.on("error", () => resolve(false))
  })
}

async function verifyAgent(agentId, workspace, basePort, result) {
  const reportPath = path.join(workspace, "service-results.json")
  let report = null
  if (fs.existsSync(reportPath)) {
    report = JSON.parse(fs.readFileSync(reportPath, "utf8"))
  }
  const aliveBeforeCleanup = [await portReady(basePort), await portReady(basePort + 1)]
  cleanupWorkspaceServices(workspace)
  await new Promise((resolve) => setTimeout(resolve, 500))
  const aliveAfterCleanup = [await portReady(basePort), await portReady(basePort + 1)]
  return {
    agent: agentId,
    status: result.status,
    timed_out: result.timed_out,
    duration_ms: result.duration_ms,
    report_path: reportPath,
    report,
    alive_before_external_cleanup: aliveBeforeCleanup,
    alive_after_external_cleanup: aliveAfterCleanup,
    ok:
      result.status === 0 &&
      !result.timed_out &&
      report?.ok === true &&
      report?.service_count === 2 &&
      report?.cleanup_done === true &&
      aliveAfterCleanup.every((alive) => !alive),
  }
}

async function runAgent(agentId, index) {
  const workspace = agentWorkspace(agentId)
  const logs = path.join(runRoot, agentId)
  const surface = shellSurface(agentId)
  const basePort = Number(process.env.COMMAND_RUN_BACKGROUND_SERVICES_BASE_PORT || 47130) + index * 10
  createWorkspace(workspace, basePort)
  cleanupWorkspaceServices(workspace)

  const args = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    `background-services-${agentId}-${process.pid}-${Date.now()}`,
    "--agent-id",
    agentPrompt(agentId),
    "-m",
    turaModel,
    ...(process.env.COMMAND_RUN_AGENT_TURA_PRIORITY === "0" ? [] : ["-p"]),
    "--model-reasoning-effort",
    reasoning,
    "--cwd",
    workspace,
  ]

  writeFile(path.join(logs, "prompt.txt"), promptFor(basePort, surface))
  const result = await spawnLogged(turaExe, args, {
    cwd: workspace,
    input: promptFor(basePort, surface),
    timeoutMs,
    stdoutPath: path.join(logs, "stdout.jsonl"),
    stderrPath: path.join(logs, "stderr.log"),
    env: {
      TURA_PROJECT_ROOT: repoRoot,
      TURA_COMMAND_RUN_SHELL: surface,
      TURA_COMMAND_RUN_STRICT_JSON: "0",
      TURA_SESSION_REASONING_EFFORT: reasoning,
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    },
  })
  return await verifyAgent(agentId, workspace, basePort, result)
}

async function main() {
  mkdirp(runRoot)
  console.log(`[background-services-e2e] run_id=${runId}`)
  console.log(`[background-services-e2e] timeout_ms=${timeoutMs}`)
  console.log(`[background-services-e2e] agents=${agents.join(",")}`)

  if (process.env.COMMAND_RUN_BACKGROUND_SERVICES_SKIP_BUILD !== "1") {
    const build = run("cargo", ["build", "-p", "gateway", "--bin", "tura"], { cwd: repoRoot, timeoutMs: 240_000 })
    assert(build.status === 0, `cargo build failed\nSTDOUT:\n${build.stdout}\nSTDERR:\n${build.stderr}`)
  }
  assert(fs.existsSync(turaExe), `missing Tura binary: ${turaExe}`)

  const results = []
  for (let i = 0; i < agents.length; i += 1) {
    const agentId = agents[i]
    console.log(`[background-services-e2e] running ${agentId}`)
    results.push(await runAgent(agentId, i))
  }

  writeFile(path.join(runRoot, "summary.json"), JSON.stringify({ run_id: runId, results }, null, 2))
  for (const result of results) {
    console.log(`[background-services-e2e] ${result.agent}: ${result.ok ? "ok" : "failed"} duration=${result.duration_ms}ms`)
  }
  if (results.some((result) => !result.ok)) {
    console.error(`[background-services-e2e] summary=${path.join(runRoot, "summary.json")}`)
    process.exit(1)
  }
}

main().catch((error) => {
  console.error(error.stack || error.message || String(error))
  process.exit(1)
})

