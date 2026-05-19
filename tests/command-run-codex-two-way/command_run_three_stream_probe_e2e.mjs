#!/usr/bin/env node
import fs from "node:fs/promises"
import { createWriteStream, existsSync, readFileSync } from "node:fs"
import { spawn } from "node:child_process"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"

const repoRoot = process.env.REPO_ROOT || process.cwd()
loadDotEnv(path.join(repoRoot, ".env"))

const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `three-stream-${Date.now()}`
const runRoot =
  process.env.COMMAND_RUN_AGENT_RUN_ROOT ||
  path.join(repoRoot, "target", "command-run-three-stream-probe", runId)
const summaryPath =
  process.env.COMMAND_RUN_AGENT_SUMMARY ||
  path.join(repoRoot, "target", "codex-logs", `command-run-three-stream-probe-${runId}.json`)
const turaRoot = process.env.COMMAND_RUN_AGENT_TURA_ROOT || repoRoot
const codexCurrentRoot = process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex")
const codexModel = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.4"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (codexModel.includes("/") ? codexModel : `openai/${codexModel}`)
const reasoningEffort = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const codexServiceTier = process.env.COMMAND_RUN_AGENT_CODEX_SERVICE_TIER || "priority"
const turaPriority = (process.env.COMMAND_RUN_AGENT_TURA_PRIORITY || (codexServiceTier === "priority" ? "1" : "0")) === "1"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 180_000)
const providerLogRoot = path.join(runRoot, "tura-provider-log")

const prompt = [
  "This is a narrow command_run streaming probe.",
  "",
  "In the first response, call command_run exactly once with exactly three shell commands.",
  "Use the same step for all three commands: step 1, step 1, and step 1.",
  "Do not use apply_patch. Do not add extra commands.",
  "",
  "The three commands must be:",
  '1. `powershell -NoProfile -ExecutionPolicy Bypass -File tools/probe.ps1 cmd1`',
  '2. `powershell -NoProfile -ExecutionPolicy Bypass -File tools/probe.ps1 cmd2`',
  '3. `powershell -NoProfile -ExecutionPolicy Bypass -File tools/probe.ps1 cmd3`',
  "",
  "After the commands finish, provide a short final answer saying whether all three commands ran.",
].join("\n")

function loadDotEnv(file) {
  if (!existsSync(file)) return
  for (const line of readFileSync(file, "utf8").split(/\r?\n/)) {
    const match = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"?([^"]*)"?\s*$/)
    if (match && !process.env[match[1]]) process.env[match[1]] = match[2]
  }
}

function turaBinForRoot(root) {
  return path.join(root, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")
}

function codexBinForRoot(root) {
  return path.join(root, "codex-rs", "target", "debug", process.platform === "win32" ? "codex.exe" : "codex")
}

async function writeText(file, content) {
  await fs.mkdir(path.dirname(file), { recursive: true })
  await fs.writeFile(file, content, "utf8")
}

async function copyDir(from, to) {
  await fs.rm(to, { recursive: true, force: true })
  await fs.cp(from, to, { recursive: true })
}

async function writeFixture(root) {
  await fs.rm(root, { recursive: true, force: true })
  await writeText(path.join(root, "README.md"), "Three-command stream probe fixture.\n")
  await writeText(path.join(root, "stream_probe.log"), "")
  await writeText(
    path.join(root, "tools", "probe.ps1"),
    [
      'param([Parameter(Mandatory=$true)][string]$Name)',
      '$ErrorActionPreference = "Stop"',
      '$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)',
      '$log = Join-Path $root "stream_probe.log"',
      '$start = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()',
      'Add-Content -Path $log -Value "$Name start $start"',
      'Start-Sleep -Milliseconds 900',
      '$end = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()',
      'Add-Content -Path $log -Value "$Name end $end"',
      'Write-Output "$Name ok $start $end"',
      "",
    ].join("\n"),
  )
}

function spawnLogged(command, args, options = {}) {
  return new Promise((resolve) => {
    const started = performance.now()
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: { ...process.env, ...(options.env || {}) },
      windowsHide: true,
      stdio: [options.input ? "pipe" : "ignore", "pipe", "pipe"],
    })
    let stdout = ""
    let stderr = ""
    let firstOutputMs = null
    const stdoutStream = options.stdoutPath ? createWriteStream(options.stdoutPath, { flags: "w" }) : null
    const stderrStream = options.stderrPath ? createWriteStream(options.stderrPath, { flags: "w" }) : null
    const markFirst = () => {
      if (firstOutputMs === null) firstOutputMs = Math.round(performance.now() - started)
    }
    const timer = setTimeout(() => {
      stderr += `\nTimed out after ${options.timeoutMs || timeoutMs}ms`
      child.kill()
    }, options.timeoutMs || timeoutMs)
    child.stdout.on("data", (chunk) => {
      markFirst()
      stdout += chunk.toString()
      stdoutStream?.write(chunk)
    })
    child.stderr.on("data", (chunk) => {
      markFirst()
      stderr += chunk.toString()
      stderrStream?.write(chunk)
    })
    if (options.input) {
      child.stdin.write(options.input)
      child.stdin.end()
    }
    child.on("error", (error) => {
      clearTimeout(timer)
      stdoutStream?.end()
      stderrStream?.end()
      resolve({ status: -1, stdout, stderr: `${stderr}\n${error.stack || error.message}`, firstOutputMs, durationMs: Math.round(performance.now() - started) })
    })
    child.on("close", (status) => {
      clearTimeout(timer)
      stdoutStream?.end()
      stderrStream?.end()
      resolve({ status: status ?? -1, stdout, stderr, firstOutputMs, durationMs: Math.round(performance.now() - started) })
    })
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
        return null
      }
    })
    .filter(Boolean)
}

function parseJsonMaybe(text) {
  try {
    return JSON.parse(String(text || ""))
  } catch {
    return null
  }
}

async function collectFiles(root, suffix, out = []) {
  let entries = []
  try {
    entries = await fs.readdir(root, { withFileTypes: true })
  } catch {
    return out
  }
  for (const entry of entries) {
    const full = path.join(root, entry.name)
    if (entry.isDirectory()) await collectFiles(full, suffix, out)
    else if (entry.isFile() && entry.name.endsWith(suffix)) out.push(full)
  }
  return out
}

function commandTextFromValue(value) {
  if (!value || typeof value !== "object") return ""
  const line = String(value.command_line || value.command || value.cmd || "")
  const parsed = parseJsonMaybe(line)
  if (parsed && typeof parsed === "object") return String(parsed.command || parsed.cmd || line)
  return line
}

function commandNameFromValue(value) {
  return String(value?.command_type || value?.command || "")
}

function commandSummary(value) {
  return {
    step: Number(value?.step || 1),
    command_type: commandNameFromValue(value),
    command_line: commandTextFromValue(value),
  }
}

function findCommandsArrayStart(argumentsText) {
  const marker = '"commands"'
  let markerIndex = argumentsText.indexOf(marker)
  if (markerIndex < 0) markerIndex = argumentsText.indexOf("'commands'")
  if (markerIndex < 0) return -1
  const colon = argumentsText.indexOf(":", markerIndex + marker.length)
  if (colon < 0) return -1
  return argumentsText.indexOf("[", colon + 1)
}

function completeCommandObjects(argumentsText) {
  const arrayStart = findCommandsArrayStart(argumentsText)
  if (arrayStart < 0) return []
  const commands = []
  let inString = false
  let escape = false
  let depth = 0
  let objectStart = -1
  for (let i = arrayStart + 1; i < argumentsText.length; i += 1) {
    const ch = argumentsText[i]
    if (inString) {
      if (escape) escape = false
      else if (ch === "\\") escape = true
      else if (ch === '"') inString = false
      continue
    }
    if (ch === '"') {
      inString = true
      continue
    }
    if (ch === "{") {
      if (depth === 0) objectStart = i
      depth += 1
      continue
    }
    if (ch === "}") {
      depth -= 1
      if (depth === 0 && objectStart >= 0) {
        const parsed = parseJsonMaybe(argumentsText.slice(objectStart, i + 1))
        if (parsed) commands.push(parsed)
        objectStart = -1
      }
      continue
    }
    if (ch === "]" && depth === 0) break
  }
  return commands
}

function analyzeCodexStdout(stdout) {
  const events = parseJsonl(stdout)
  const commandEvents = events
    .filter((event) => event.item?.type === "command_execution")
    .map((event) => ({
      event_type: event.type,
      status: event.item?.status,
      command: String(event.item?.command || ""),
      timestamp: event.timestamp || event.time || null,
    }))
  const calls = events
    .filter((event) => event.type === "response_item" && event.payload?.type === "function_call" && event.payload?.name === "command_run")
    .map((event) => parseJsonMaybe(event.payload.arguments))
    .filter((value) => value?.commands)
  const outputs = events
    .filter((event) => event.type === "response_item" && event.payload?.type === "function_call_output")
    .map((event) => parseJsonMaybe(event.payload.output))
    .filter((value) => value?.results)
  return {
    event_count: events.length,
    command_events: commandEvents,
    function_call_batch_sizes: calls.map((call) => call.commands.length),
    function_call_commands: calls.flatMap((call) => call.commands.map(commandSummary)),
    output_result_lengths: outputs.map((output) => output.results.length),
  }
}

function analyzeTuraStdout(stdout) {
  const events = parseJsonl(stdout)
  const runtimeUsage = events
    .map((event) => event.item?.metadata || event.metadata || event)
    .find((metadata) => metadata?.kind === "mano_runtime_usage" && metadata?.usage)
  return {
    event_count: events.length,
    runtime_usage: runtimeUsage?.usage || null,
  }
}

async function providerCallLogs() {
  const files = await collectFiles(providerLogRoot, ".json")
  const parsed = []
  for (const file of files) {
    try {
      const stat = await fs.stat(file)
      const value = JSON.parse(await fs.readFile(file, "utf8"))
      if (value?.type === "llm_call") parsed.push({ file, stat, value })
    } catch {
      // Ignore partial files.
    }
  }
  parsed.sort((a, b) => a.stat.mtimeMs - b.stat.mtimeMs)
  return parsed
}

function firstProviderToolCallLog(logs) {
  return logs.find((log) =>
    (log.value?.response?.stream || log.value?.response?.events || []).some((event) =>
      String(event?.type || "").startsWith("response.function_call_arguments."),
    ),
  ) || null
}

function analyzeProviderStream(log) {
  const events = log?.value?.response?.stream || log?.value?.response?.events || []
  let args = ""
  let emitted = 0
  const ready = []
  let firstOutputIndex = null
  for (let index = 0; index < events.length; index += 1) {
    const event = events[index]
    const type = String(event?.type || "")
    if (
      firstOutputIndex === null &&
      [
        "response.output_text.delta",
        "response.function_call_arguments.delta",
        "response.output_item.added",
        "response.content_part.added",
      ].includes(type)
    ) {
      firstOutputIndex = index
    }
    if (type === "response.function_call_arguments.delta") {
      args += String(event.delta || "")
    } else if (type === "response.function_call_arguments.done") {
      args = String(event.arguments || args)
    } else {
      continue
    }
    const commands = completeCommandObjects(args)
    while (emitted < commands.length) {
      ready.push({
        event_index: index,
        event_type: type,
        command_index: emitted,
        command: commandSummary(commands[emitted]),
      })
      emitted += 1
    }
  }
  return {
    provider_file: log?.file || null,
    started_at: log?.value?.started_at || null,
    finished_at: log?.value?.finished_at || null,
    started_ms: log?.value?.started_at ? Date.parse(log.value.started_at) : null,
    finished_ms: log?.value?.finished_at ? Date.parse(log.value.finished_at) : null,
    event_count: events.length,
    first_output_event_index: firstOutputIndex,
    ready_command_count: ready.length,
    ready_commands: ready,
    duration_ms: log?.value?.duration_ms || null,
    usage: log?.value?.metrics?.usage || null,
  }
}

async function readProbeLog(workspace) {
  const text = await fs.readFile(path.join(workspace, "stream_probe.log"), "utf8").catch(() => "")
  const lines = text.split(/\r?\n/).map((line) => line.trim()).filter(Boolean)
  const parsed = lines.map((line) => {
    const match = line.match(/^(cmd\d)\s+(start|end)\s+(\d+)$/)
    return match ? { command: match[1], phase: match[2], ts: Number(match[3]), raw: line } : { raw: line }
  })
  return {
    lines,
    parsed,
    command_order: parsed.filter((item) => item.phase === "start").map((item) => item.command),
    first_command_start_ms: parsed.find((item) => item.phase === "start")?.ts || null,
    last_command_end_ms: [...parsed].reverse().find((item) => item.phase === "end")?.ts || null,
    ok:
      lines.length === 6 &&
      parsed.filter((item) => item.phase === "start").map((item) => item.command).join(",") === "cmd1,cmd2,cmd3" &&
      parsed.filter((item) => item.phase === "end").map((item) => item.command).join(",") === "cmd1,cmd2,cmd3",
  }
}

async function runTura(workspace) {
  const bin = turaBinForRoot(turaRoot)
  const logs = path.join(runRoot, "tura")
  await fs.mkdir(logs, { recursive: true })
  const stdoutPath = path.join(logs, "stdout.jsonl")
  const stderrPath = path.join(logs, "stderr.log")
  const lastMessagePath = path.join(logs, "last-message.md")
  const result = await spawnLogged(
    bin,
    [
      "exec",
      "--skip-git-repo-check",
      "--json",
      "-C",
      workspace,
      "-m",
      turaModel,
      "--dangerously-bypass-approvals-and-sandbox",
      "-c",
      `model_reasoning_effort="${reasoningEffort}"`,
      "-c",
      `service_tier="${turaPriority ? "priority" : "auto"}"`,
      "--output-last-message",
      lastMessagePath,
    ],
    {
      cwd: workspace,
      input: prompt,
      stdoutPath,
      stderrPath,
      timeoutMs,
      env: {
        LOG_PATH: providerLogRoot,
        TURA_COMMAND_RUN_SHELL: "shell_command",
        TURA_COMMAND_RUN_DISABLE_STRICT_JSON: "1",
      },
    },
  )
  const providerLogs = await providerCallLogs()
  const providerLog = firstProviderToolCallLog(providerLogs)
  const probeLog = await readProbeLog(workspace)
  return {
    agent: "tura",
    ok: result.status === 0 && probeLog.ok,
    exit_code: result.status,
    duration_ms: result.durationMs,
    first_cli_output_ms: result.firstOutputMs,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    last_message_path: lastMessagePath,
    stderr_tail: result.stderr.slice(-2000),
    stdout: analyzeTuraStdout(result.stdout),
    provider_stream: analyzeProviderStream(providerLog),
    provider_call_files: providerLogs.map((log) => log.file),
    probe_log: probeLog,
  }
}

async function runCurrent(workspace) {
  const bin = codexBinForRoot(codexCurrentRoot)
  const logs = path.join(runRoot, "current")
  await fs.mkdir(logs, { recursive: true })
  const stdoutPath = path.join(logs, "stdout.jsonl")
  const stderrPath = path.join(logs, "stderr.log")
  const lastMessagePath = path.join(logs, "last-message.md")
  const result = await spawnLogged(
    bin,
    [
      "exec",
      "--skip-git-repo-check",
      "--json",
      "-C",
      workspace,
      "-m",
      codexModel,
      "--dangerously-bypass-approvals-and-sandbox",
      "-c",
      `model_reasoning_effort="${reasoningEffort}"`,
      "-c",
      `service_tier="${codexServiceTier}"`,
      "--output-last-message",
      lastMessagePath,
    ],
    { cwd: workspace, input: prompt, stdoutPath, stderrPath, timeoutMs },
  )
  const probeLog = await readProbeLog(workspace)
  return {
    agent: "current",
    ok: result.status === 0 && probeLog.ok,
    exit_code: result.status,
    duration_ms: result.durationMs,
    first_cli_output_ms: result.firstOutputMs,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    last_message_path: lastMessagePath,
    stderr_tail: result.stderr.slice(-2000),
    stdout: analyzeCodexStdout(result.stdout),
    probe_log: probeLog,
  }
}

async function main() {
  const started = performance.now()
  await fs.rm(runRoot, { recursive: true, force: true })
  await fs.mkdir(runRoot, { recursive: true })
  await fs.rm(providerLogRoot, { recursive: true, force: true })
  await fs.mkdir(path.dirname(summaryPath), { recursive: true })

  const baseline = path.join(runRoot, "baseline")
  const turaWorkspace = path.join(runRoot, "repo-tura")
  const currentWorkspace = path.join(runRoot, "repo-current")
  await writeFixture(baseline)
  await copyDir(baseline, turaWorkspace)
  await copyDir(baseline, currentWorkspace)

  const [tura, current] = await Promise.all([runTura(turaWorkspace), runCurrent(currentWorkspace)])
  const turaReady = tura.provider_stream.ready_commands.map((item) => item.command.command_line)
  const currentCalls = current.stdout.function_call_commands.map((item) => item.command_line)
  const turaFirstCommandStartedBeforeProviderFinished =
    Number.isFinite(tura.provider_stream.finished_ms) &&
    Number.isFinite(tura.probe_log.first_command_start_ms) &&
    tura.probe_log.first_command_start_ms < tura.provider_stream.finished_ms
  const turaFirstCommandStartedBeforeLastCommandReady =
    Number.isFinite(tura.probe_log.first_command_start_ms) &&
    Number.isFinite(tura.provider_stream.started_ms) &&
    tura.provider_stream.ready_commands.length >= 3 &&
    tura.probe_log.first_command_start_ms <
      tura.provider_stream.started_ms +
        (tura.provider_stream.duration_ms *
          (tura.provider_stream.ready_commands[2].event_index /
            Math.max(1, tura.provider_stream.event_count)))
  const summary = {
    ok:
      tura.ok &&
      current.ok &&
      tura.provider_stream.ready_command_count === 3 &&
      tura.probe_log.command_order.join(",") === "cmd1,cmd2,cmd3" &&
      current.probe_log.command_order.join(",") === "cmd1,cmd2,cmd3",
    run_id: runId,
    run_root: runRoot,
    summary_path: summaryPath,
    prompt,
    model_config: {
      codex_model: codexModel,
      tura_model: turaModel,
      reasoning_effort: reasoningEffort,
      codex_service_tier: codexServiceTier,
      tura_priority: turaPriority,
    },
    duration_ms: Math.round(performance.now() - started),
    workspaces: { tura: turaWorkspace, current: currentWorkspace },
    parity: {
      both_ran_three_commands: tura.probe_log.ok && current.probe_log.ok,
      tura_stream_ready_three_commands: tura.provider_stream.ready_command_count === 3,
      tura_first_command_started_before_provider_finished: turaFirstCommandStartedBeforeProviderFinished,
      tura_first_command_started_before_last_command_ready: turaFirstCommandStartedBeforeLastCommandReady,
      current_function_call_batch_sizes: current.stdout.function_call_batch_sizes,
      tura_ready_command_lines: turaReady,
      current_function_call_command_lines: currentCalls,
    },
    runs: [current, tura],
  }
  await writeText(summaryPath, JSON.stringify(summary, null, 2))
  console.log(`[three-stream-probe] summary: ${summaryPath}`)
  console.log(`[three-stream-probe] ok=${summary.ok}`)
  if (!summary.ok) process.exitCode = 1
}

main().catch(async (error) => {
  await writeText(
    summaryPath,
    JSON.stringify({ ok: false, run_id: runId, run_root: runRoot, summary_path: summaryPath, error: error.stack || error.message }, null, 2),
  )
  console.error(error.stack || error.message)
  process.exitCode = 1
})
