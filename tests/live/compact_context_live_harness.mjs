#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { businessRunPaths, normalizeBusinessSummary } from "../business/business_lib_business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..")
loadDotEnv(path.join(repoRoot, ".env"))

const DEFAULT_CONTEXT_TOKEN_LIMIT = 250_000
const runId = process.env.COMMAND_RUN_COMPACT_LIVE_RUN_ID || process.env.COMMAND_RUN_AGENT_RUN_ID || `compact-live-${Date.now()}`
const runPaths = businessRunPaths("compact-context-live", runId)
const runRoot = runPaths.run_root
const turaHome = path.join(runRoot, "tura-home")
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const model = resolveLiveModel(process.env.COMMAND_RUN_AGENT_TURA_MODEL)
const agentId = process.env.COMMAND_RUN_COMPACT_LIVE_AGENT_ID || process.env.COMMAND_RUN_AGENT_ID || "fast"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const priority = (process.env.COMMAND_RUN_AGENT_TURA_PRIORITY || "1") === "1"
const skipBuild = (process.env.COMMAND_RUN_AGENT_SKIP_TURA_BUILD || "0") === "1"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || process.argv[2] || 30_000)
const scenarioMode = process.env.COMMAND_RUN_COMPACT_LIVE_SCENARIOS || "compact-resume"
const autoMode = process.env.COMMAND_RUN_COMPACT_AUTO_MODE || "compact-threshold"
const autoContextLimitTokens = Number(process.env.COMMAND_RUN_COMPACT_AUTO_CONTEXT_LIMIT_TOKENS || 16_000)

function loadDotEnv(file) {
  if (!fs.existsSync(file)) return
  for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    const match = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"?([^"#]*)"?\s*(?:#.*)?$/)
    if (match && !process.env[match[1]]) process.env[match[1]] = match[2].trim()
  }
}

function readLocalTuraConfig() {
  const config = path.join(repoRoot, ".tura", "config.conf")
  if (!fs.existsSync(config)) return {}
  const values = {}
  const lines = fs.readFileSync(config, "utf8").split(/\r?\n/)
  for (const line of lines) {
    const index = line.indexOf("=")
    if (index <= 0) continue
    values[line.slice(0, index).trim()] = line.slice(index + 1).trim()
  }
  return values
}

function qualifiedModel(provider, modelName) {
  if (!modelName) return null
  if (modelName.includes("/")) return modelName
  return provider ? `${provider}/${modelName}` : modelName
}

function readLocalTuraModel() {
  const config = readLocalTuraConfig()
  return qualifiedModel(config.active_provider, config.active_model) || config.model || null
}

function defaultModelForGroup(groupName) {
  const catalogPath = path.join(repoRoot, "crates", "provider", "config", "provider_config.json")
  const catalog = readJsonMaybe(catalogPath)
  const providers = catalog?.model_groups?.[groupName]?.providers
  const first = Array.isArray(providers) ? providers[0] : null
  return first ? qualifiedModel(first.provider, first.model) : null
}

function isModelGroup(value) {
  const catalogPath = path.join(repoRoot, "crates", "provider", "config", "provider_config.json")
  const catalog = readJsonMaybe(catalogPath)
  return !!catalog?.model_groups?.[value]
}

function resolveLiveModel(rawModel) {
  const localModel = readLocalTuraModel()
  const requested = String(rawModel || "").trim()
  if (!requested) return localModel || "codex/gpt-5.5"
  if (requested.includes("/")) return requested
  if (isModelGroup(requested)) return localModel || defaultModelForGroup(requested) || requested
  return requested
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeText(file, text) {
  ensureDir(path.dirname(file))
  fs.writeFileSync(file, text, "utf8")
}

function spawnLogged(command, args, options = {}) {
  return new Promise((resolve) => {
    const started = performance.now()
    let settled = false
    let killTimer = null
    const finish = (payload) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      if (killTimer) clearTimeout(killTimer)
      resolve(payload)
    }
    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      windowsHide: true,
      stdio: [options.input ? "pipe" : "ignore", "pipe", "pipe"],
    })
    let stdout = ""
    let stderr = ""
    let timedOut = false
    const limitMs = options.timeoutMs || timeoutMs
    const timer = setTimeout(() => {
      timedOut = true
      stderr += `\nTimed out after ${limitMs}ms`
      if (process.platform === "win32" && child.pid) {
        spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
          stdio: "ignore",
          windowsHide: true,
        })
      } else {
        child.kill("SIGKILL")
      }
      killTimer = setTimeout(() => {
        finish({ status: -1, stdout, stderr, timedOut, durationMs: Math.round(performance.now() - started) })
      }, 5_000)
      killTimer.unref?.()
    }, limitMs)
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString()
    })
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString()
    })
    if (options.input) {
      child.stdin.write(options.input)
    }
    child.stdin.end()
    child.on("error", (error) => {
      finish({ status: -1, stdout, stderr: `${stderr}\n${error.stack || error.message}`, timedOut, durationMs: Math.round(performance.now() - started) })
    })
    child.on("close", (status) => {
      finish({ status: status ?? -1, stdout, stderr, timedOut, durationMs: Math.round(performance.now() - started) })
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

function walkJson(value, visit) {
  if (!value || typeof value !== "object") return
  visit(value)
  if (Array.isArray(value)) {
    for (const item of value) walkJson(item, visit)
    return
  }
  for (const item of Object.values(value)) walkJson(item, visit)
}

function readJsonMaybe(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"))
  } catch {
    return null
  }
}

function collectJsonFiles(root, out = []) {
  if (!fs.existsSync(root)) return out
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const full = path.join(root, entry.name)
    if (entry.isDirectory()) collectJsonFiles(full, out)
    else if (entry.isFile() && entry.name.endsWith(".json")) out.push(full)
  }
  return out
}

function providerLogsSince(sinceMs) {
  const logRoot = path.join(repoRoot, "log", "provider")
  return collectJsonFiles(logRoot)
    .map((file) => ({ file, stat: fs.statSync(file) }))
    .filter((item) => item.stat.mtimeMs >= sinceMs - 1000)
    .map((item) => ({ ...item, value: readJsonMaybe(item.file) }))
    .filter((item) => item.value)
    .sort((a, b) => a.stat.mtimeMs - b.stat.mtimeMs)
}

function providerLogMatches(log, needles) {
  const requestText = messageTextFromCall(log.value)
  const responseText = JSON.stringify(log.value?.response || {})
  return needles.some((needle) => requestText.includes(needle) || responseText.includes(needle))
}

function relevantProviderLogsSince(sinceMs, needles) {
  return providerLogsSince(sinceMs).filter((log) => providerLogMatches(log, needles))
}

function messageTextFromCall(call) {
  const messages = call?.request?.messages || call?.request?.params?.messages || []
  return messages.map((message) => {
    const content = message?.content
    if (typeof content === "string") return content
    return JSON.stringify(content || "")
  }).join("\n")
}

function commandRunCallsFromProviderLogs(logs, requiredText) {
  const out = []
  for (const log of logs) {
    const requestText = messageTextFromCall(log.value)
    if (requiredText && !requestText.includes(requiredText) && !JSON.stringify(log.value?.response || {}).includes(requiredText)) {
      continue
    }
    walkJson(log.value?.response, (value) => {
      if (value?.type === "function_call" && value?.name === "command_run" && value?.arguments) {
        const args = parseJson(value.arguments)
        if (args?.commands) out.push({ file: log.file, requestText, args })
      }
    })
  }
  return out
}

function parseJson(text) {
  try {
    return JSON.parse(String(text || ""))
  } catch {
    return null
  }
}

function commandNames(commands) {
  return (commands || []).map((command) => String(command.command_type || command.command || "").trim())
}

function hasCompactCommand(commandRunCalls) {
  return commandRunCalls.some((call) => commandNames(call.args.commands).includes("compact_context"))
}

function hasShellCommand(commandRunCalls, pattern) {
  return commandRunCalls.some((call) =>
    (call.args.commands || []).some((command) =>
      String(command.command_type || command.command) === "shell_command" &&
      pattern.test(String(command.command_line || "")),
    ),
  )
}

function usageFromEvents(events) {
  const usage = { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, total_tokens: 0 }
  for (const event of events) {
    const u = event.usage || event.payload?.info?.last_token_usage
    if (!u) continue
    usage.input_tokens += Number(u.input_tokens || u.prompt_tokens || 0)
    usage.cached_input_tokens += Number(u.cached_input_tokens || u.input_tokens_details?.cached_tokens || 0)
    usage.output_tokens += Number(u.output_tokens || u.completion_tokens || 0)
    usage.reasoning_tokens += Number(u.reasoning_output_tokens || u.reasoning_tokens || 0)
    usage.total_tokens += Number(u.total_tokens || 0)
  }
  return usage
}

function turaArgs(sessionId, workspace, lastMessagePath) {
  return [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    sessionId,
    "--agent-id",
    agentId,
    "-m",
    model,
    ...(priority ? ["-p"] : []),
    "--model-reasoning-effort",
    reasoning,
    "--planning",
    "off",
    "--shll",
    "-C",
    workspace,
    "--output-last-message",
    lastMessagePath,
  ]
}

async function runTuraTurn({ sessionId, workspace, prompt, label, extraEnv = {} }) {
  const turnDir = path.join(workspace, ".compact-live", label)
  ensureDir(turnDir)
  const stdoutPath = path.join(turnDir, "stdout.jsonl")
  const stderrPath = path.join(turnDir, "stderr.log")
  const lastMessagePath = path.join(turnDir, "last-message.md")
  const routerStderrPath = path.join(turnDir, "router.stderr.log")
  const workerStderrPath = path.join(turnDir, "worker.stderr.log")
  const sinceMs = Date.now()
  const result = await spawnLogged(turaExe, turaArgs(sessionId, workspace, lastMessagePath), {
    cwd: workspace,
    input: prompt,
    timeoutMs,
    env: {
      TURA_HOME: turaHome,
      TURA_COMMAND_RUN_SHELL: "shell_command",
      TURA_COMMAND_RUN_STRICT_JSON: "1",
      TURA_DEBUG_RUNTIME: process.env.TURA_DEBUG_RUNTIME || "1",
      TURA_PROJECT_ROOT: repoRoot,
      TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS: process.env.TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS || "160000",
      TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS: process.env.TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS || "80000",
      TURA_PROVIDER_TOTAL_TIMEOUT_MS: process.env.TURA_PROVIDER_TOTAL_TIMEOUT_MS || String(timeoutMs),
      TURA_PROVIDER_RETRY_BACKOFF_MS: process.env.TURA_PROVIDER_RETRY_BACKOFF_MS || "0,0,0",
      TURA_NO_TOOL_RETRY_LIMIT: process.env.TURA_NO_TOOL_RETRY_LIMIT || "0",
      TURA_ROUTER_IDLE_SHUTDOWN_SECS: process.env.TURA_ROUTER_IDLE_SHUTDOWN_SECS || "1",
      TURA_ROUTER_STDERR_LOG: routerStderrPath,
      TURA_RUNTIME_WORKER_STDERR_LOG: workerStderrPath,
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
      ...extraEnv,
    },
  })
  writeText(stdoutPath, result.stdout)
  writeText(stderrPath, result.stderr)
  return {
    label,
    status: result.status,
    ok: result.status === 0,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    last_message_path: lastMessagePath,
    router_stderr_path: routerStderrPath,
    worker_stderr_path: workerStderrPath,
    stderr_tail: result.stderr.slice(-4000),
    duration_ms: result.durationMs,
    usage: usageFromEvents(parseJsonl(result.stdout)),
    provider_logs: providerLogsSince(sinceMs).map((item) => item.file),
    provider_since_ms: sinceMs,
  }
}

function writeFixture(workspace, scenario) {
  fs.rmSync(workspace, { recursive: true, force: true })
  ensureDir(workspace)
  writeText(path.join(workspace, "README.md"), `Compact context live fixture for ${scenario}.\n`)
}

function verifyFile(workspace, name, expected) {
  const file = path.join(workspace, name)
  const text = fs.existsSync(file) ? fs.readFileSync(file, "utf8") : ""
  return { ok: text.includes(expected), path: file, expected, text: text.slice(0, 1000) }
}

function explicitInitialPrompt() {
  return [
    "This is a live compact_context trigger test.",
    "Your next assistant response must call command_run. Do not answer only in prose.",
    "Use one command_run batch with exactly five commands.",
    "Before the checkpoint, run four lightweight shell_command entries:",
    "1. Create `explicit_probe.txt` containing exactly `EXPLICIT_PHASE_ONE_OK ORCHID-417`.",
    "2. Read `explicit_probe.txt`.",
    "3. Check that `explicit_probe.txt` exists.",
    "4. Print `explicit-precheck-ok`.",
    "Then use `compact_context` as command 5, and it must be the final command in the highest step.",
    "The compact_context handoff must preserve: sentinel ORCHID-417, created file explicit_probe.txt, and next action create explicit_followup.txt with EXPLICIT_FOLLOWUP_OK ORCHID-417.",
    "After command_run completes, reply briefly.",
  ].join("\n")
}

function explicitFollowupPrompt() {
  return [
    "Continue from the compacted handoff context.",
    "Your next assistant response must call command_run. Do not answer only in prose.",
    "Use one command_run batch with exactly six lightweight entries:",
    "1. Create `explicit_followup.txt` containing exactly `EXPLICIT_FOLLOWUP_OK ORCHID-417`.",
    "2. Read `explicit_followup.txt`.",
    "3. Check that `explicit_followup.txt` exists.",
    "4. Print the current directory.",
    "5. Print `explicit-followup-ok`.",
    "6. Use `task_status` with status `done` and a brief summary mentioning `EXPLICIT_FOLLOWUP_OK ORCHID-417`.",
  ].join("\n")
}

function resolveModelContextTokens(modelName) {
  const catalogPath = path.join(repoRoot, "crates", "provider", "config", "provider_config.json")
  const catalog = readJsonMaybe(catalogPath)
  if (!catalog) return null
  const [providerFromModel, bareModel] = modelName.includes("/")
    ? modelName.split("/", 2)
    : [null, modelName]
  const providers = catalog.model_catalog?.providers || {}
  const providerEntries = providerFromModel ? [[providerFromModel, providers[providerFromModel]]] : Object.entries(providers)
  for (const [providerId, provider] of providerEntries) {
    if (!provider?.models) continue
    for (const list of Object.values(provider.models)) {
      for (const item of list || []) {
        const id = typeof item === "string" ? item : item?.id
        if (!id) continue
        const normalized = id.includes("/") ? id : `${providerId}/${id}`
        if (id === modelName || id === bareModel || normalized === modelName) {
          return Number(item?.limit?.context || item?.detail?.limit?.context || 0) || null
        }
      }
    }
  }
  return null
}

function autoContextPlan() {
  const modelContextTokens = resolveModelContextTokens(model)
  const compactLimitTokens = autoContextLimitTokens > 0
    ? autoContextLimitTokens
    : modelContextTokens
      ? Math.min(DEFAULT_CONTEXT_TOKEN_LIMIT, Math.floor(modelContextTokens * 0.6))
      : DEFAULT_CONTEXT_TOKEN_LIMIT
  const requested = Number(process.env.COMMAND_RUN_COMPACT_AUTO_TARGET_TOKENS || 0)
  const hardOverLimit = autoMode === "hard-over-limit"
  const targetTokens = requested || (hardOverLimit && modelContextTokens
    ? modelContextTokens + 12_000
    : Math.max(compactLimitTokens + 2_000, Math.ceil(compactLimitTokens * 1.25)))
  const maxChars = Number(process.env.COMMAND_RUN_COMPACT_AUTO_MAX_CHARS || 120_000)
  const targetChars = Math.min(Math.max(targetTokens * 4, 32_000), maxChars)
  return { model_context_tokens: modelContextTokens, compact_limit_tokens: compactLimitTokens, target_tokens: targetTokens, target_chars: targetChars, mode: autoMode }
}

function makeLongAutoPrompt(plan) {
  const header = [
    "Live long-reference operation.",
    "Do not summarize this reference block. Do not discuss it.",
    "After the block, perform the requested workspace operation with command_run.",
    "",
    "BEGIN_LONG_REFERENCE",
  ].join("\n")
  const footer = [
    "END_LONG_REFERENCE",
    "",
    "Requested operation:",
    "Your next assistant response must call command_run. Do not answer only in prose.",
    "Use one command_run batch with shell commands to create `auto_probe.txt` containing exactly `AUTO_PHASE_ONE_OK TOPAZ-903`, read it back, check that it exists, print the current directory, and print `auto-precheck-ok`.",
    "Reply briefly after the operation succeeds.",
  ].join("\n")
  const line = "AUTO_FILLER_LINE stable reference payload 0123456789 abcdefghijklmnopqrstuvwxyz TOPAZ-903 marker retained.\n"
  const need = Math.max(plan.target_chars - header.length - footer.length, line.length)
  const repeat = Math.ceil(need / line.length)
  return `${header}\n${line.repeat(repeat).slice(0, need)}\n${footer}`
}

function autoFollowupPrompt() {
  return [
    "Continue the prior long-reference operation.",
    "Your next assistant response must call command_run. Do not answer only in prose.",
    "Use one command_run batch with exactly six lightweight entries:",
    "1. Create `auto_followup.txt` containing exactly `AUTO_FOLLOWUP_OK TOPAZ-903`.",
    "2. Read `auto_followup.txt`.",
    "3. Check that `auto_followup.txt` exists.",
    "4. Print the current directory.",
    "5. Print `auto-followup-ok`.",
    "6. Use `task_status` with status `done` and a brief summary mentioning `AUTO_FOLLOWUP_OK TOPAZ-903`.",
  ].join("\n")
}

async function runExplicitScenario() {
  const workspace = path.join(runRoot, "explicit-workspace")
  const sessionId = `compact-live-explicit-${runId}`
  writeFixture(workspace, "explicit")
  const first = await runTuraTurn({ sessionId, workspace, prompt: explicitInitialPrompt(), label: "turn1-explicit-compact" })
  const firstLogs = relevantProviderLogsSince(first.provider_since_ms, ["ORCHID-417", sessionId, workspace])
  const firstCalls = commandRunCallsFromProviderLogs(firstLogs, "ORCHID-417")
  if (!first.ok) {
    const probe = verifyFile(workspace, "explicit_probe.txt", "EXPLICIT_PHASE_ONE_OK ORCHID-417")
    return {
      scenario: "explicit",
      ok: false,
      session_id: sessionId,
      workspace,
      turns: [first],
      validation: {
        compact_command_seen: hasCompactCommand(firstCalls),
        followup_path_updated: false,
        followup_request_chars: 0,
        probe,
        followup: verifyFile(workspace, "explicit_followup.txt", "EXPLICIT_FOLLOWUP_OK ORCHID-417"),
        command_run_call_count: firstCalls.length,
        command_sequences: firstCalls.map((call) => commandNames(call.args.commands)),
        provider_logs: firstLogs.map((item) => item.file),
        stopped_after_first_failed_turn: true,
      },
    }
  }
  const second = await runTuraTurn({ sessionId, workspace, prompt: explicitFollowupPrompt(), label: "turn2-explicit-followup" })
  const logs = relevantProviderLogsSince(first.provider_since_ms, ["ORCHID-417", sessionId, workspace])
  const calls = commandRunCallsFromProviderLogs(logs, "ORCHID-417")
  const requestTexts = logs.map((item) => ({ file: item.file, text: messageTextFromCall(item.value) }))
  const followupRequest = requestTexts.find((item) => item.text.includes("explicit_followup.txt"))
  const followupPathUpdated =
    !!followupRequest &&
    followupRequest.text.includes(workspace) &&
    !followupRequest.text.includes(path.join(runRoot, "auto-workspace"))
  const probe = verifyFile(workspace, "explicit_probe.txt", "EXPLICIT_PHASE_ONE_OK ORCHID-417")
  const followup = verifyFile(workspace, "explicit_followup.txt", "EXPLICIT_FOLLOWUP_OK ORCHID-417")
  return {
    scenario: "explicit",
    ok: first.ok && second.ok && probe.ok && followup.ok && hasCompactCommand(calls) && hasShellCommand(calls, /explicit_probe\.txt/i) && followupPathUpdated,
    session_id: sessionId,
    workspace,
    turns: [first, second],
    validation: {
      compact_command_seen: hasCompactCommand(calls),
      followup_path_updated: followupPathUpdated,
      followup_request_chars: followupRequest?.text.length || 0,
      probe,
      followup,
      command_run_call_count: calls.length,
      command_sequences: calls.map((call) => commandNames(call.args.commands)),
      provider_logs: logs.map((item) => item.file),
    },
  }
}

async function runAutoScenario() {
  const workspace = path.join(runRoot, "auto-workspace")
  const sessionId = `compact-live-auto-${runId}`
  writeFixture(workspace, "auto")
  const plan = autoContextPlan()
  const autoEnv = {
    TURA_CONTEXT_LIMIT_TOKENS: String(plan.compact_limit_tokens),
  }
  const prompt = makeLongAutoPrompt(plan)
  writeText(path.join(workspace, "auto_prompt.md"), prompt)
  const first = await runTuraTurn({ sessionId, workspace, prompt, label: "turn1-auto-threshold", extraEnv: autoEnv })
  const firstLogs = relevantProviderLogsSince(first.provider_since_ms, ["TOPAZ-903", sessionId, workspace])
  const firstCalls = commandRunCallsFromProviderLogs(firstLogs, "TOPAZ-903")
  if (!first.ok) {
    const requestTexts = firstLogs.map((item) => ({ file: item.file, text: messageTextFromCall(item.value) }))
    return {
      scenario: "auto",
      ok: false,
      session_id: sessionId,
      workspace,
      auto_context_plan: {
        ...plan,
        prompt_chars: prompt.length,
      },
      turns: [first],
      validation: {
        auto_prompt_injected: requestTexts.some((item) => item.text.includes("Context checkpoint required") && item.text.includes("compact_context as the final command")),
        compact_command_seen: hasCompactCommand(firstCalls),
        followup_context_compacted: false,
        followup_path_updated: false,
        followup_request_chars: 0,
        original_prompt_chars: prompt.length,
        probe: verifyFile(workspace, "auto_probe.txt", "AUTO_PHASE_ONE_OK TOPAZ-903"),
        followup: verifyFile(workspace, "auto_followup.txt", "AUTO_FOLLOWUP_OK TOPAZ-903"),
        command_run_call_count: firstCalls.length,
        command_sequences: firstCalls.map((call) => commandNames(call.args.commands)),
        provider_logs: firstLogs.map((item) => item.file),
        stopped_after_first_failed_turn: true,
      },
    }
  }
  const second = await runTuraTurn({ sessionId, workspace, prompt: autoFollowupPrompt(), label: "turn2-auto-followup", extraEnv: autoEnv })
  const logs = relevantProviderLogsSince(first.provider_since_ms, ["TOPAZ-903", sessionId, workspace])
  const calls = commandRunCallsFromProviderLogs(logs, "TOPAZ-903")
  const requestTexts = logs.map((item) => ({ file: item.file, text: messageTextFromCall(item.value) }))
  const injected = requestTexts.some((item) => item.text.includes("Context checkpoint required") && item.text.includes("compact_context as the final command"))
  const followupRequest = requestTexts.find((item) => item.text.includes("auto_followup.txt"))
  const followupContextCompacted =
    !!followupRequest &&
    followupRequest.text.includes("TOPAZ-903") &&
    followupRequest.text.length < Math.max(40_000, Math.floor(prompt.length / 2)) &&
    !followupRequest.text.includes("AUTO_FILLER_LINE stable reference payload")
  const followupPathUpdated =
    !!followupRequest &&
    followupRequest.text.includes(workspace) &&
    !followupRequest.text.includes(path.join(runRoot, "explicit-workspace"))
  const probe = verifyFile(workspace, "auto_probe.txt", "AUTO_PHASE_ONE_OK TOPAZ-903")
  const followup = verifyFile(workspace, "auto_followup.txt", "AUTO_FOLLOWUP_OK TOPAZ-903")
  return {
    scenario: "auto",
    ok: first.ok && second.ok && injected && hasCompactCommand(calls) && probe.ok && followup.ok && followupContextCompacted && followupPathUpdated,
    session_id: sessionId,
    workspace,
    auto_context_plan: {
      ...plan,
      prompt_chars: prompt.length,
    },
    turns: [first, second],
    validation: {
      auto_prompt_injected: injected,
      compact_command_seen: hasCompactCommand(calls),
      followup_context_compacted: followupContextCompacted,
      followup_path_updated: followupPathUpdated,
      followup_request_chars: followupRequest?.text.length || 0,
      original_prompt_chars: prompt.length,
      probe,
      followup,
      command_run_call_count: calls.length,
      command_sequences: calls.map((call) => commandNames(call.args.commands)),
      provider_logs: logs.map((item) => item.file),
    },
  }
}

async function main() {
  const started = performance.now()
  ensureDir(runRoot)
  ensureDir(turaHome)
  if (!skipBuild || !fs.existsSync(turaExe)) {
    const build = await spawnLogged("cargo", ["build", "--bins", "-p", "gateway", "-p", "router", "-p", "runtime", "-p", "session_log"], { cwd: repoRoot, timeoutMs: 240_000 })
    assert.equal(build.status, 0, `cargo build failed\nSTDOUT:\n${build.stdout}\nSTDERR:\n${build.stderr}`)
  }
  assert(fs.existsSync(turaExe), `missing tura_exec binary: ${turaExe}`)

  console.log(`[compact-live] run_id=${runId}`)
  console.log(`[compact-live] run_root=${runRoot}`)
  console.log(`[compact-live] tura_home=${turaHome}`)
  console.log(`[compact-live] model=${model}`)
  console.log(`[compact-live] agent=${agentId}`)
  console.log(`[compact-live] auto_mode=${autoMode}`)
  console.log(`[compact-live] scenarios=${scenarioMode}`)

  const results = []
  const scenarios = scenarioMode === "all"
    ? [runExplicitScenario, runAutoScenario]
    : scenarioMode === "auto"
      ? [runAutoScenario]
      : [runExplicitScenario]
  for (const runScenario of scenarios) {
    const result = await runScenario()
    results.push(result)
    if (!result.ok) break
  }
  const summary = normalizeBusinessSummary({
    ok: results.every((result) => result.ok),
    run_id: runId,
    run_root: runRoot,
    tura_home: turaHome,
    model,
    agent_id: agentId,
    reasoning_effort: reasoning,
    priority,
    timeout_ms: timeoutMs,
    duration_ms: Math.round(performance.now() - started),
    results,
  }, runPaths)
  writeText(runPaths.summary_path, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  assert(summary.ok, `compact live harness failed; summary: ${runPaths.summary_path}`)
}

main().catch((error) => {
  ensureDir(runRoot)
  ensureDir(turaHome)
  const summary = normalizeBusinessSummary({
    ok: false,
    run_id: runId,
    run_root: runRoot,
    tura_home: turaHome,
    model,
    agent_id: agentId,
    error: error.stack || error.message,
  }, runPaths)
  writeText(runPaths.summary_path, JSON.stringify(summary, null, 2))
  console.error(error.stack || error.message)
  process.exitCode = 1
})
