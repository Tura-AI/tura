#!/usr/bin/env node
import fs from "node:fs/promises"
import { existsSync, readFileSync } from "node:fs"
import { spawn } from "node:child_process"
import crypto from "node:crypto"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..", "..")
loadDotEnv(path.join(repoRoot, ".env"))

const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `small-${Date.now()}`
const runRoot =
  process.env.COMMAND_RUN_AGENT_RUN_ROOT ||
  path.join(repoRoot, "target", "command-run-codex-two-way-small", runId)
const summaryPath =
  process.env.COMMAND_RUN_AGENT_SUMMARY ||
  path.join(repoRoot, "target", "codex-logs", `command-run-codex-two-way-small-${runId}.json`)
const turaRoot = process.env.COMMAND_RUN_AGENT_TURA_ROOT || repoRoot
const codexCurrentRoot = process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex")
const codexModel = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.1-codex"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || `openai/${codexModel}`
const reasoningEffort = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const codexServiceTier = process.env.COMMAND_RUN_AGENT_CODEX_SERVICE_TIER || "priority"
const turaPriority = (process.env.COMMAND_RUN_AGENT_TURA_PRIORITY || "1") === "1"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 240_000)

const taskPrompt = [
  "You are running a small single-round E2E command execution benchmark.",
  "",
  "Repository task:",
  "- First inspect `src/app.txt` with exactly `Get-Content -Raw src/app.txt`.",
  "- Use apply_patch to change the file content from `broken-by-agent` to `fixed-by-agent`.",
  "- Run `powershell -NoProfile -ExecutionPolicy Bypass -File tools/verify.ps1` until it passes.",
  "- Do not edit the verifier.",
  "- Finish only after the verification script passes, then summarize the exact command path you used.",
].join("\n")

function loadDotEnv(file) {
  if (!existsSync(file)) return
  for (const line of readFileSync(file, "utf8").split(/\r?\n/)) {
    const match = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"?([^"]*)"?\s*$/)
    if (match && !process.env[match[1]]) process.env[match[1]] = match[2]
  }
}

function codexBinForRoot(root) {
  return path.join(root, "codex-rs", "target", "debug", process.platform === "win32" ? "codex.exe" : "codex")
}

function turaBinForRoot(root) {
  return path.join(root, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
}

async function writeText(file, content) {
  await fs.mkdir(path.dirname(file), { recursive: true })
  await fs.writeFile(file, content, "utf8")
}

async function copyDir(from, to) {
  await fs.rm(to, { recursive: true, force: true })
  await fs.cp(from, to, { recursive: true })
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
    const markFirstOutput = () => {
      if (firstOutputMs === null) firstOutputMs = Math.round(performance.now() - started)
    }
    const timer = setTimeout(() => {
      stderr += `\nTimed out after ${options.timeoutMs || timeoutMs}ms`
      child.kill()
    }, options.timeoutMs || timeoutMs)
    child.stdout.on("data", (chunk) => {
      markFirstOutput()
      stdout += chunk.toString()
    })
    child.stderr.on("data", (chunk) => {
      markFirstOutput()
      stderr += chunk.toString()
    })
    if (options.input) {
      child.stdin.write(options.input)
      child.stdin.end()
    }
    child.on("error", (error) => {
      clearTimeout(timer)
      resolve({ status: -1, stdout, stderr: `${stderr}\n${error.stack || error.message}`, firstOutputMs, durationMs: Math.round(performance.now() - started) })
    })
    child.on("close", (status) => {
      clearTimeout(timer)
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

function sha256(text) {
  return crypto.createHash("sha256").update(String(text || ""), "utf8").digest("hex")
}

async function writeFixture(root) {
  await fs.rm(root, { recursive: true, force: true })
  await writeText(path.join(root, "README.md"), "Small command execution parity fixture.\n")
  await writeText(path.join(root, "src", "app.txt"), "broken-by-agent\n")
  await writeText(
    path.join(root, "tools", "verify.ps1"),
    [
      '$ErrorActionPreference = "Stop"',
      '$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)',
      '$content = (Get-Content (Join-Path $root "src/app.txt") -Raw).Trim()',
      'if ($content -ne "fixed-by-agent") {',
      '  Write-Error "expected fixed-by-agent, got $content"',
      '  exit 1',
      '}',
      'Write-Output "ok: fixed-by-agent"',
      'exit 0',
      "",
    ].join("\n"),
  )
}

async function verifyRepo(workspace) {
  const result = await spawnLogged("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(workspace, "tools", "verify.ps1")], {
    cwd: workspace,
    timeoutMs: 30_000,
  })
  const content = await fs.readFile(path.join(workspace, "src", "app.txt"), "utf8").catch(() => "")
  return {
    ok: result.status === 0 && content.trim() === "fixed-by-agent",
    exit_code: result.status,
    stdout_tail: result.stdout.slice(-1000),
    stderr_tail: result.stderr.slice(-1000),
    final_content: content.trim(),
  }
}

function analyzeEvents(stdout, agent) {
  const events = parseJsonl(stdout)
  const text = JSON.stringify(events)
  const commandExecutions = events.filter((event) => event.item?.type === "command_execution")
  const fileChanges = events.filter((event) => event.item?.type === "file_change")
  const sawInspect = /src[\\/]app\.txt|Get-Content|cat src\/app\.txt/i.test(text)
  const sawPatch = /apply_patch|file_change|fixed-by-agent/i.test(text)
  const sawVerify = /tools[\\/]verify\.ps1|tools\\verify\.ps1/i.test(text)
  const agentMessages = events.filter((event) => event.item?.type === "agent_message")
  const visibleAgentText = agentMessages.map((event) => String(event.item?.text || "")).join("\n")
  const phase_sequence = events
    .filter((event) => event.item?.type)
    .map((event) => {
      const itemType = event.item.type
      if (itemType === "agent_message") return "agent_message"
      return `${itemType}:${event.type}:${event.item?.status || ""}`
    })
  const completed_commands = commandExecutions
    .filter((event) => event.type === "item.completed")
    .map((event) => normalizeCommandText(event.item?.command || ""))
  const completed_file_changes = fileChanges
    .filter((event) => event.type === "item.completed")
    .flatMap((event) => event.item?.changes || [])
    .map((change) => ({ kind: change.kind, path: String(change.path || "").replace(/\\/g, "/").replace(/.*\/src\/app\.txt$/, "src/app.txt") }))
  const expectedPhaseSequence = [
    "agent_message",
    "command_execution:item.started:in_progress",
    "command_execution:item.completed:completed",
    "file_change:item.started:in_progress",
    "file_change:item.completed:completed",
    "command_execution:item.started:in_progress",
    "command_execution:item.completed:completed",
    "agent_message",
  ]
  return {
    agent,
    event_count: events.length,
    command_execution_events: commandExecutions.length,
    file_change_events: fileChanges.length,
    agent_message_events: agentMessages.length,
    saw_inspect: sawInspect,
    saw_patch: sawPatch,
    saw_verify: sawVerify,
    path_ok: sawInspect && sawPatch && sawVerify,
    forbidden_tool_mentions: /send_message_to_user/i.test(text),
    raw_tool_payload_visible: /"commands"\s*:|"step_summary"\s*:|"previous_command_evaluations"\s*:/.test(visibleAgentText),
    phase_sequence,
    phase_sequence_ok:
      JSON.stringify(phase_sequence) === JSON.stringify(expectedPhaseSequence) ||
      (agent === "tura" &&
        completed_commands.includes("task_status") &&
        sawInspect &&
        sawPatch &&
        sawVerify),
    completed_commands,
    completed_file_changes,
  }
}

function normalizeCommandText(command) {
  return String(command || "")
    .replace(/\\\\/g, "\\")
    .replace(/"C:\\Program Files\\PowerShell\\7\\pwsh\.exe"\s+-Command\s+'/i, "powershell:")
    .replace(/"pwsh\.exe"\s+-Command\s+'/i, "powershell:")
    .replace(/'$/, "")
    .trim()
}

function walkJson(value, visit) {
  if (!value || typeof value !== "object") return
  visit(value)
  if (Array.isArray(value)) {
    for (const item of value) walkJson(item, visit)
    return
  }
  for (const item of Object.values(value)) walkJson(item, visit)
}

function parseJsonMaybe(text) {
  try {
    return JSON.parse(String(text || ""))
  } catch {
    return null
  }
}

function commandSignature(command) {
  return {
    step: Number(command?.step || 1),
    command: String(command?.command_type || command?.command || ""),
    command_line: String(command?.command_line || command?.cmd || ""),
  }
}

function isTaskStatusSignature(command) {
  return String(command?.command || "").trim() === "task_status"
}

function stripTerminalTaskStatus(items) {
  const out = [...(items || [])]
  if (out.length > 0 && isTaskStatusSignature(out[out.length - 1])) out.pop()
  return out
}

function commandNameSequence(items) {
  return (items || []).map((item) => item.command)
}

function normalizeCoreCommandFlow(items) {
  const core = stripTerminalTaskStatus(items)
  const inspect = core.find((item) => /Get-Content\s+-Raw\s+src[\\/]app\.txt/i.test(item.command_line))
  const patch = core.find((item) => item.command === "apply_patch")
  const verify = core.find((item) => /tools[\\/]verify\.ps1|tools\\verify\.ps1/i.test(item.command_line))
  if (inspect && patch && verify) return [inspect, patch, verify]
  return core
}

function commandFlowHasRequiredPath(items) {
  const core = stripTerminalTaskStatus(items)
  return (
    core.some((item) => /Get-Content\s+-Raw\s+src[\\/]app\.txt/i.test(item.command_line)) &&
    core.some((item) => item.command === "apply_patch") &&
    core.some((item) => /tools[\\/]verify\.ps1|tools\\verify\.ps1/i.test(item.command_line))
  )
}

function hasTerminalTaskStatus(items) {
  return (items || []).length > 0 && isTaskStatusSignature(items[items.length - 1])
}

function extractTuraCommandRunCalls(calls) {
  const out = []
  for (const call of calls) {
    const matches = []
    walkJson(call.response, (value) => {
      if (value?.type === "function_call" && value?.name === "command_run" && value?.status === "completed") {
        const args = parseJsonMaybe(value.arguments)
        if (args?.commands) matches.push({ args, call_id: value.call_id })
      }
    })
    out.push(...matches)
  }
  return out
}

function inspectTuraToolFlow(calls) {
  const functionCalls = extractTuraCommandRunCalls(calls)
  const backfillPayloads = calls.slice(1).flatMap((call) =>
    (call?.request?.messages || [])
      .filter((message) => message?.type === "function_call_output" || message?.content)
      .map((message) => {
        if (message?.type === "function_call_output") return String(message.output || "")
        return String(message?.content || "")
      }),
  )
  const parsedBackfills = backfillPayloads
    .map(parseJsonMaybe)
    .filter((value) => value?.results && Array.isArray(value.results))
  const joinedBackfill = backfillPayloads.join("\n")
  const secondMessages = calls[1]?.request?.messages || []
  const secondMarkers = markerSequenceFromMessages(secondMessages)
  const observedCurrentBackfillShape = (calls.slice(1).flatMap((call) => call?.request?.messages || []))
    .some((message) => message?.type === "function_call_output")
  const commandSequence = functionCalls.flatMap((call) => call.args.commands.map(commandSignature))
  const coreCommandSequence = normalizeCoreCommandFlow(commandSequence)
  const pathOk = commandFlowHasRequiredPath(commandSequence)
  return {
    ok:
      [1, 2].includes(functionCalls.length) &&
      JSON.stringify(commandNameSequence(coreCommandSequence)) ===
        JSON.stringify(["shell_command", "apply_patch", "shell_command"]) &&
      hasTerminalTaskStatus(commandSequence) &&
      pathOk &&
      parsedBackfills.length >= 1 &&
      parsedBackfills.some((value) => value.results.length === functionCalls[0]?.args?.commands?.length) &&
      !/"cache_id"|"tool_name"|"input"/.test(joinedBackfill) &&
      !/Turn \d+ completed with \d+ tool calls/.test(joinedBackfill) &&
      secondMarkers.join("|").endsWith(
        "developer:workspace_snapshot|developer:environment_context|user:task",
      ),
    function_call_count: functionCalls.length,
    batch_sizes: functionCalls.map((call) => call.args.commands.length),
    command_sequence: commandSequence,
    normalized_core_sequence: coreCommandSequence,
    path_ok: pathOk,
    backfill_results_lengths: parsedBackfills.map((value) => value.results.length),
    backfill_uses_function_call_output: observedCurrentBackfillShape,
    backfill_has_legacy_wrapper: /"cache_id"|"tool_name"|"input"/.test(joinedBackfill),
    backfill_has_turn_status_noise: /Turn \d+ completed with \d+ tool calls/.test(joinedBackfill),
    second_marker_sequence: secondMarkers,
  }
}

function inspectCodexCurrentToolFlow(sessionFile) {
  if (!sessionFile || !existsSync(sessionFile)) {
    return { ok: false, error: "codex-current session file missing" }
  }
  const events = parseJsonl(readFileSync(sessionFile, "utf8"))
  const functionCalls = events
    .filter((event) => event.type === "response_item" && event.payload?.type === "function_call" && event.payload?.name === "command_run")
    .map((event) => ({ args: parseJsonMaybe(event.payload.arguments), call_id: event.payload.call_id }))
    .filter((item) => item.args?.commands)
  const outputs = events
    .filter((event) => event.type === "response_item" && event.payload?.type === "function_call_output")
    .map((event) => parseJsonMaybe(event.payload.output))
    .filter((value) => value?.results && Array.isArray(value.results))
  const outputText = outputs.map((value) => JSON.stringify(value)).join("\n")
  return {
    ok:
      functionCalls.length === 1 &&
      functionCalls[0]?.args?.commands?.length === 3 &&
      outputs.length === 1 &&
      outputs[0].results.length === 3 &&
      !/"cache_id"|"tool_name"|"input"/.test(outputText),
    function_call_count: functionCalls.length,
    batch_sizes: functionCalls.map((call) => call.args.commands.length),
    command_sequence: functionCalls.flatMap((call) => call.args.commands.map(commandSignature)),
    backfill_results_lengths: outputs.map((value) => value.results.length),
    backfill_has_legacy_wrapper: /"cache_id"|"tool_name"|"input"/.test(outputText),
  }
}

function markerSequenceFromMessages(messages) {
  const markerSequence = []
  for (const message of messages) {
    const role = String(message?.role || "")
    const content = String(message?.content || "")
    if (content.startsWith("You are Codex")) markerSequence.push(`${role}:base_instructions`)
    if (content.includes("<permissions instructions>")) markerSequence.push(`${role}:permissions`)
    if (content.includes("<WORKSPACE_SNAPSHOT>")) markerSequence.push(`${role}:workspace_snapshot`)
    if (content.includes("<environment_context>")) markerSequence.push(`${role}:environment_context`)
    if (content.includes("small single-round E2E command execution benchmark")) markerSequence.push(`${role}:task`)
  }
  return markerSequence
}

async function collectJsonFiles(root, out = []) {
  let entries = []
  try {
    entries = await fs.readdir(root, { withFileTypes: true })
  } catch {
    return out
  }
  for (const entry of entries) {
    const full = path.join(root, entry.name)
    if (entry.isDirectory()) await collectJsonFiles(full, out)
    else if (entry.isFile() && entry.name.endsWith(".json")) out.push(full)
  }
  return out
}

async function inspectTuraProviderContract(sinceMs) {
  const logRoot = path.join(turaRoot, "log", "provider")
  const files = await collectJsonFiles(logRoot)
  const candidates = []
  for (const file of files) {
    const stat = await fs.stat(file).catch(() => null)
    if (!stat || stat.mtimeMs < sinceMs - 1000) continue
    try {
      const parsed = JSON.parse(await fs.readFile(file, "utf8"))
      candidates.push({ file, stat, parsed })
    } catch {
      // Ignore partial or unrelated log files.
    }
  }
  candidates.sort((a, b) => a.stat.mtimeMs - b.stat.mtimeMs)
  const calls = candidates.map((item) => item.parsed).filter((item) => item?.type === "llm_call")
  const firstToolCall = calls.find((call) => Array.isArray(call?.request?.params?.tools) && call.request.params.tools.length > 0)
  const firstMessages = firstToolCall?.request?.messages || []
  const promptText = calls
    .flatMap((call) => call?.request?.messages || [])
    .map((message) => String(message?.content || ""))
    .join("\n")
  const toolNames = (firstToolCall?.request?.params?.tools || [])
    .map((tool) => tool?.function?.name)
    .filter(Boolean)
  const commandDescriptions = JSON.stringify(firstToolCall?.request?.params?.tools || [])
  const commandItems = firstToolCall?.request?.params?.tools?.[0]?.function?.parameters?.properties?.commands?.items || {}
  const commandRequired = commandItems?.required || []
  const commandHasEnum = !!commandItems?.properties?.command_type?.enum
  const oldPathPattern = /crates[\\/]tools[\\/]interface|Pcommand_run|Icommand_run|scripts[\\/]command_run|handler\.py/i
  const toolFlow = inspectTuraToolFlow(calls)
  return {
    ok:
      toolNames.length === 1 &&
      toolNames[0] === "command_run" &&
      commandItems?.properties?.command_type?.type === "string" &&
      !commandHasEnum &&
      JSON.stringify(commandRequired) === JSON.stringify(["command_type", "command_line"]) &&
      !/send_message_to_user/i.test(promptText) &&
      !/powershell:\*/i.test(promptText) &&
      !/powershell:/i.test(commandDescriptions) &&
      !oldPathPattern.test(promptText) &&
      !oldPathPattern.test(commandDescriptions) &&
      toolFlow.ok,
    tool_names: toolNames,
    command_required: commandRequired,
    command_has_enum: commandHasEnum,
    prompt_has_send_message: /send_message_to_user/i.test(promptText),
    prompt_has_powershell_alias: /powershell:\*/i.test(promptText),
    schema_has_powershell_alias: /powershell:/i.test(commandDescriptions),
    prompt_or_schema_has_old_paths: oldPathPattern.test(promptText) || oldPathPattern.test(commandDescriptions),
    inspected_log: firstToolCall ? candidates.find((item) => item.parsed === firstToolCall)?.file : null,
    context_contract: contextContractFromMessages(firstMessages, { requireBase: false }),
    tool_flow: toolFlow,
  }
}

function contextContractFromMessages(messages, options = {}) {
  const requireBase = options.requireBase ?? true
  const normalized = messages.map((message) => ({
    role: String(message?.role || ""),
    content: String(message?.content || ""),
  }))
  const markerSequence = markerSequenceFromMessages(normalized)
  const base = normalized.find((message) => message.content.startsWith("You are Codex"))?.content || ""
  const workspaceSnapshot = normalized.find((message) => message.content.includes("<WORKSPACE_SNAPSHOT>"))?.content || ""
  const environment = normalized.find((message) => message.content.includes("<environment_context>"))?.content || ""
  const task = normalized.find((message) => message.content.includes("small single-round E2E command execution benchmark"))?.content || ""
  const joined = normalized.map((message) => message.content).join("\n")
  const expected = requireBase
    ? [
        "system:base_instructions",
        "developer:workspace_snapshot",
        "developer:environment_context",
        "user:task",
      ]
    : ["developer:workspace_snapshot", "developer:environment_context", "user:task"]
  return {
    ok:
      JSON.stringify(markerSequence) === JSON.stringify(expected) &&
      (!requireBase || base.startsWith("You are Codex, a coding agent")) &&
      (requireBase
        ? workspaceSnapshot.includes("columns: modified_utc | lines | suffix | path")
        : workspaceSnapshot.length > 0) &&
      environment.includes("<shell>powershell</shell>") &&
      task.includes("Get-Content -Raw src/app.txt") &&
      !/Permanent runtime context|Task continuity reminder|Tool reporting requirement|Current workspace directory:|Initial workspace file snapshot/i.test(joined),
    marker_sequence: markerSequence,
    expected_marker_sequence: expected,
    base_sha256: sha256(base),
    base_chars: base.length,
    workspace_snapshot_sha256: sha256(workspaceSnapshot.replace(/repo-(tura|codex-current)/g, "repo-agent")),
    environment_sha256: sha256(environment.replace(/repo-(tura|codex-current)/g, "repo-agent")),
    task_sha256: sha256(task),
    has_tura_runtime_noise: /Permanent runtime context|Task continuity reminder|Tool reporting requirement|Current workspace directory:|Initial workspace file snapshot/i.test(joined),
  }
}

async function inspectCodexCurrentContext(sinceMs, workspace) {
  const sessionsRoot = path.join(homeDir, ".codex", "sessions")
  const files = await collectJsonlFiles(sessionsRoot)
  const candidates = []
  const normalizedWorkspace = workspace.replace(/\\/g, "/").toLowerCase()
  const workspaceName = path.basename(workspace).toLowerCase()
  for (const file of files) {
    const stat = await fs.stat(file).catch(() => null)
    if (!stat || stat.mtimeMs < sinceMs - 300_000) continue
    const text = await fs.readFile(file, "utf8").catch(() => "")
    const normalizedText = text.replace(/\\/g, "/").toLowerCase()
    if (!normalizedText.includes(normalizedWorkspace) && !normalizedText.includes(workspaceName)) continue
    const events = parseJsonl(text)
    const base = events.find((event) => event.type === "session_meta")?.payload?.base_instructions?.text || ""
    const messages = []
    for (const event of events) {
      const payload = event?.payload
      if (event.type !== "response_item") continue
      if (payload?.type === "function_call") break
      if (!payload?.role) continue
      const content = Array.isArray(payload.content)
        ? payload.content.map((item) => item?.text || "").join("\n")
        : String(payload.content || "")
      if (payload.role === "assistant" || payload.type === "reasoning") continue
      if (payload.role === "user" && content.includes("<WORKSPACE_SNAPSHOT>")) {
        const normalizedSnapshot = content.replace(/\\/g, "/").toLowerCase()
        if (!normalizedSnapshot.includes(normalizedWorkspace) && !normalizedSnapshot.includes(workspaceName)) {
          continue
        }
      }
      messages.push({ role: payload.role, content })
    }
    if (!base || !messages.some((message) => message.content.includes("<WORKSPACE_SNAPSHOT>"))) continue
    candidates.push({ file, stat, base, messages })
  }
  candidates.sort((a, b) => b.stat.mtimeMs - a.stat.mtimeMs)
  const selected = candidates[0]
  if (!selected) {
    return { ok: false, inspected_session: null, error: "codex-current session context not found" }
  }
  return {
    ...contextContractFromMessages([{ role: "system", content: selected.base }, ...selected.messages]),
    inspected_session: selected.file,
  }
}

async function collectJsonlFiles(root, out = []) {
  let entries = []
  try {
    entries = await fs.readdir(root, { withFileTypes: true })
  } catch {
    return out
  }
  for (const entry of entries) {
    const full = path.join(root, entry.name)
    if (entry.isDirectory()) await collectJsonlFiles(full, out)
    else if (entry.isFile() && entry.name.endsWith(".jsonl")) out.push(full)
  }
  return out
}

function compareContextContracts(tura, codex) {
  return {
    ok:
      !!tura?.ok &&
      !!codex?.ok &&
      tura.task_sha256 === codex.task_sha256 &&
      !tura.has_tura_runtime_noise,
    base_sha256_match: !tura?.base_chars || tura?.base_sha256 === codex?.base_sha256,
    marker_sequence_match:
      JSON.stringify(tura?.marker_sequence) === JSON.stringify(codex?.marker_sequence) ||
      JSON.stringify(tura?.marker_sequence) ===
        JSON.stringify(["developer:workspace_snapshot", "developer:environment_context", "user:task"]),
    task_sha256_match: tura?.task_sha256 === codex?.task_sha256,
    tura_marker_sequence: tura?.marker_sequence,
    codex_current_marker_sequence: codex?.marker_sequence,
    tura_base_chars: tura?.base_chars,
    codex_current_base_chars: codex?.base_chars,
    tura_has_runtime_noise: !!tura?.has_tura_runtime_noise,
    tura_context: tura,
    codex_current_context: codex,
  }
}

function collectCommandEnums(value, out = []) {
  if (!value || typeof value !== "object") return out
  if (Array.isArray(value)) {
    for (const item of value) collectCommandEnums(item, out)
    return out
  }
  if (value.command && Array.isArray(value.command.enum)) {
    out.push([...value.command.enum].sort())
  }
  if (value.properties?.command?.enum) {
    out.push([...value.properties.command.enum].sort())
  }
  for (const child of Object.values(value)) collectCommandEnums(child, out)
  return out
}

function uniqueJsonArrays(items) {
  const seen = new Set()
  const unique = []
  for (const item of items) {
    const key = JSON.stringify(item)
    if (seen.has(key)) continue
    seen.add(key)
    unique.push(item)
  }
  return unique
}

async function inspectTuraSourceContract() {
  const paths = {
    commandRunHandler: path.join(turaRoot, "crates", "tools", "src", "command_run", "handler.rs"),
    commandRunSchema: path.join(turaRoot, "crates", "tools", "src", "command_run", "schema.json"),
    shellPrompt: path.join(turaRoot, "crates", "tools", "src", "commands", "shell_command", "prompt.md"),
    applyPatchPrompt: path.join(turaRoot, "crates", "tools", "src", "commands", "apply_patch", "prompt.md"),
    planningPrompt: path.join(turaRoot, "crates", "tools", "src", "commands", "planning", "prompt.md"),
    compactContextPrompt: path.join(turaRoot, "crates", "tools", "src", "commands", "compact_context", "prompt.md"),
    fileLocksPolicy: path.join(turaRoot, "crates", "tools", "src", "runtime", "file_locks", "policy.toml"),
    fileLocksModule: path.join(turaRoot, "crates", "tools", "src", "runtime", "file_locks", "mod.rs"),
    commandsModule: path.join(turaRoot, "crates", "tools", "src", "commands", "mod.rs"),
  }
  const filesExist = Object.values(paths).every((file) => existsSync(file))
  const handler = existsSync(paths.commandRunHandler) ? await fs.readFile(paths.commandRunHandler, "utf8") : ""
  const fileLocks = existsSync(paths.fileLocksModule) ? await fs.readFile(paths.fileLocksModule, "utf8") : ""
  const commandsModule = existsSync(paths.commandsModule) ? await fs.readFile(paths.commandsModule, "utf8") : ""
  const schema = existsSync(paths.commandRunSchema) ? JSON.parse(await fs.readFile(paths.commandRunSchema, "utf8")) : {}
  const commandItems = schema?.input_schema?.properties?.commands?.items || {}
  const commandRequired = commandItems?.required || []
  const commandHasEnum = !!commandItems?.properties?.command_type?.enum
  const commandsModuleText = commandsModule.replace(/\s+/g, " ")
  const oldDirectoriesAbsent = [
    path.join(turaRoot, "crates", "tools", "interface"),
    path.join(turaRoot, "crates", "tools", "prompt"),
    path.join(turaRoot, "crates", "tools", "scripts"),
    path.join(turaRoot, "crates", "tools", "command_run"),
    path.join(turaRoot, "crates", "tools", "commands"),
    path.join(turaRoot, "crates", "tools", "modes"),
    path.join(turaRoot, "crates", "tools", "runtime"),
  ].every((file) => !existsSync(file))
  return {
    ok:
      filesExist &&
      oldDirectoriesAbsent &&
      schema?.input_schema?.properties?.commands?.minItems === 5 &&
      schema?.input_schema?.properties?.commands?.maxItems === 15 &&
      !schema?.input_schema?.properties?.task_status &&
      /task_status/.test(schema?.description || "") &&
      commandItems?.properties?.command_type?.type === "string" &&
      !commandHasEnum &&
      JSON.stringify(commandRequired) === JSON.stringify(["command_type", "command_line"]) &&
      /struct FileLockManager/.test(fileLocks) &&
      /fn run_command_run_step/.test(handler) &&
      /workspace_write/.test(fileLocks) &&
      /pub fn execute/.test(commandsModule) &&
      /pub mod planning/.test(commandsModule) &&
      /pub mod compact_context/.test(commandsModule) &&
      /"apply_patch"\s*=>\s*apply_patch::execute/.test(commandsModuleText) &&
      /"compact_context"\s*=>\s*compact_context::execute/.test(commandsModuleText) &&
      /"planning"[^=]*=>\s*\{\s*planning::execute/.test(commandsModuleText) &&
      /"bash"\s*=>\s*bash::execute/.test(commandsModuleText) &&
      /"shell_command"\s*=>\s*shell_command::execute/.test(commandsModuleText) &&
      /unsupported command_run command/.test(commandsModule) &&
      !/handler\.py|invoke_python_script|command_run_service|services[\\/]command_run/.test(handler + fileLocks + commandsModule),
    files_exist: filesExist,
    old_directories_absent: oldDirectoriesAbsent,
    command_min_items: schema?.input_schema?.properties?.commands?.minItems,
    command_max_items: schema?.input_schema?.properties?.commands?.maxItems,
    command_required: commandRequired,
    command_run_has_task_status: /task_status/.test(schema?.description || ""),
    command_has_enum: commandHasEnum,
    has_file_lock_manager: /struct FileLockManager/.test(fileLocks),
    has_step_grouping: /fn run_command_run_step/.test(handler),
    has_workspace_write_lock: /workspace_write/.test(fileLocks),
    has_command_dispatch_module: /pub fn execute/.test(commandsModule),
    has_exact_internal_command_dispatch:
      /"apply_patch"\s*=>\s*apply_patch::execute/.test(commandsModuleText) &&
      /"compact_context"\s*=>\s*compact_context::execute/.test(commandsModuleText) &&
      /"planning"[^=]*=>\s*\{\s*planning::execute/.test(commandsModuleText) &&
      /"bash"\s*=>\s*bash::execute/.test(commandsModuleText) &&
      /"shell_command"\s*=>\s*shell_command::execute/.test(commandsModuleText) &&
      /unsupported command_run command/.test(commandsModule),
    has_forbidden_legacy_code: /handler\.py|invoke_python_script|command_run_service|services[\\/]command_run/.test(handler + fileLocks + commandsModule),
  }
}

async function runCodex(workspace) {
  const bin = codexBinForRoot(codexCurrentRoot)
  if (!existsSync(bin)) throw new Error(`missing codex-current binary: ${bin}`)
  const logs = path.join(runRoot, "codex-current")
  await fs.mkdir(logs, { recursive: true })
  const stdoutPath = path.join(logs, "stdout.jsonl")
  const stderrPath = path.join(logs, "stderr.log")
  const lastMessagePath = path.join(logs, "last-message.md")
  const args = [
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
  ]
  const contextSinceMs = Date.now()
  const result = await spawnLogged(bin, args, { cwd: workspace, input: taskPrompt, timeoutMs })
  await writeText(stdoutPath, result.stdout)
  await writeText(stderrPath, result.stderr)
  const verify = await verifyRepo(workspace)
  const analysis = analyzeEvents(result.stdout, "codex-current")
  const context_contract = await inspectCodexCurrentContext(contextSinceMs, workspace)
  const tool_flow = inspectCodexCurrentToolFlow(context_contract.inspected_session)
  return { agent: "codex-current", bin, workspace, ok: result.status === 0 && verify.ok && analysis.path_ok && context_contract.ok && tool_flow.ok, exit_code: result.status, verify, analysis, context_contract, tool_flow, stdout_path: stdoutPath, stderr_path: stderrPath, last_message_path: lastMessagePath, stderr_tail: result.stderr.slice(-2000), duration_ms: result.durationMs, first_output_ms: result.firstOutputMs }
}

async function runTura(workspace) {
  const bin = turaBinForRoot(turaRoot)
  if (!existsSync(bin)) throw new Error(`missing tura binary: ${bin}`)
  const logs = path.join(runRoot, "tura")
  await fs.mkdir(logs, { recursive: true })
  const stdoutPath = path.join(logs, "stdout.jsonl")
  const stderrPath = path.join(logs, "stderr.log")
  const lastMessagePath = path.join(logs, "last-message.md")
  const args = [
    "exec",
    "--skip-git-repo-check",
    "--json",
    "-C",
    workspace,
    "-m",
    turaModel,
    ...(turaPriority ? ["-p"] : []),
    "--sandbox",
    "--model-reasoning-effort",
    reasoningEffort,
    "--output-last-message",
    lastMessagePath,
  ]
  const providerSinceMs = Date.now()
  const result = await spawnLogged(bin, args, { cwd: workspace, input: taskPrompt, timeoutMs })
  await writeText(stdoutPath, result.stdout)
  await writeText(stderrPath, result.stderr)
  const verify = await verifyRepo(workspace)
  const analysis = analyzeEvents(result.stdout, "tura")
  const provider_contract = await inspectTuraProviderContract(providerSinceMs)
  const source_contract = await inspectTuraSourceContract()
  const pathOk = analysis.path_ok || provider_contract.tool_flow?.path_ok
  const phaseOk = analysis.phase_sequence_ok || provider_contract.tool_flow?.path_ok
  return { agent: "tura", bin, workspace, ok: result.status === 0 && verify.ok && pathOk && phaseOk && !analysis.raw_tool_payload_visible && !analysis.forbidden_tool_mentions && provider_contract.ok && provider_contract.context_contract.ok && source_contract.ok, exit_code: result.status, verify, analysis, provider_contract, source_contract, stdout_path: stdoutPath, stderr_path: stderrPath, last_message_path: lastMessagePath, stderr_tail: result.stderr.slice(-2000), duration_ms: result.durationMs, first_output_ms: result.firstOutputMs }
}

async function main() {
  const started = performance.now()
  await fs.rm(runRoot, { recursive: true, force: true })
  await fs.mkdir(runRoot, { recursive: true })
  await fs.mkdir(path.dirname(summaryPath), { recursive: true })

  const baseline = path.join(runRoot, "baseline")
  const turaWorkspace = path.join(runRoot, "repo-tura")
  const codexWorkspace = path.join(runRoot, "repo-codex-current")
  await writeFixture(baseline)
  await copyDir(baseline, turaWorkspace)
  await copyDir(baseline, codexWorkspace)

  const runs = await Promise.all([runTura(turaWorkspace), runCodex(codexWorkspace)])
  const turaRun = runs.find((run) => run.agent === "tura")
  const codexRun = runs.find((run) => run.agent === "codex-current")
  const turaCommandSequence = turaRun?.provider_contract?.tool_flow?.command_sequence || []
  const codexCommandSequence = codexRun?.tool_flow?.command_sequence || []
  const turaCoreCommandSequence = normalizeCoreCommandFlow(turaCommandSequence)
  const codexCoreCommandSequence = normalizeCoreCommandFlow(codexCommandSequence)
  const turaCompletedCommands = turaRun?.analysis.completed_commands || []
  const codexCompletedCommands = codexRun?.analysis.completed_commands || []
  const turaCoreCompletedCommands = turaCompletedCommands.filter((command) => command !== "task_status")
  const turaPathOk = !!(turaRun?.analysis.path_ok || turaRun?.provider_contract?.tool_flow?.path_ok)
  const parity = {
    ok:
      !!turaRun &&
      !!codexRun &&
      turaRun.verify.ok &&
      codexRun.verify.ok &&
      turaPathOk &&
      codexRun.analysis.path_ok &&
      JSON.stringify(commandNameSequence(turaCoreCommandSequence)) === JSON.stringify(commandNameSequence(codexCoreCommandSequence)) &&
      turaCoreCompletedCommands.length >= codexCompletedCommands.length &&
      hasTerminalTaskStatus(turaCommandSequence),
    event_count_match: turaRun?.analysis.event_count === (codexRun?.analysis.event_count || 0) + 1,
    phase_sequence_match:
      turaRun?.analysis.phase_sequence?.includes("command_execution:item.completed:completed") &&
      codexRun?.analysis.phase_sequence?.includes("command_execution:item.completed:completed"),
    completed_commands_match: turaCoreCompletedCommands.length >= codexCompletedCommands.length,
    tool_flow_match:
      JSON.stringify(commandNameSequence(turaCoreCommandSequence)) === JSON.stringify(commandNameSequence(codexCoreCommandSequence)) &&
      hasTerminalTaskStatus(turaCommandSequence),
    tura_completed_commands: turaRun?.analysis.completed_commands,
    codex_current_completed_commands: codexRun?.analysis.completed_commands,
    tura_phase_sequence: turaRun?.analysis.phase_sequence,
    codex_current_phase_sequence: codexRun?.analysis.phase_sequence,
    tura_tool_flow: turaRun?.provider_contract?.tool_flow,
    codex_current_tool_flow: codexRun?.tool_flow,
  }
  const context_parity = compareContextContracts(
    turaRun?.provider_contract?.context_contract,
    codexRun?.context_contract,
  )
  const summary = {
    ok: runs.every((run) => run.ok) && parity.ok && context_parity.ok,
    run_id: runId,
    run_root: runRoot,
    summary_path: summaryPath,
    prompt: taskPrompt,
    model_config: { tura_model: turaModel, codex_model: codexModel, reasoning_effort: reasoningEffort, codex_service_tier: codexServiceTier, tura_priority: turaPriority },
    workspaces: { tura: turaWorkspace, codex_current: codexWorkspace },
    duration_ms: Math.round(performance.now() - started),
    parity,
    context_parity,
    runs,
  }
  await writeText(summaryPath, JSON.stringify(summary, null, 2))
  console.log(`[command-run-single-round] summary: ${summaryPath}`)
  console.log(`[command-run-single-round] ok=${summary.ok}`)
  if (!summary.ok) process.exitCode = 1
}

main().catch(async (error) => {
  await writeText(summaryPath, JSON.stringify({ ok: false, run_id: runId, run_root: runRoot, summary_path: summaryPath, error: error.stack || error.message }, null, 2))
  console.error(error.stack || error.message)
  process.exitCode = 1
})
