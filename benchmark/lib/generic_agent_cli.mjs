import assert from "node:assert/strict"
import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import process from "node:process"
import { spawn, spawnSync } from "node:child_process"
import { performance } from "node:perf_hooks"

import { agentEventStats, agentUsageFromJsonl, claudeCodeArgs, findClaudeExe, findOpencodeExe, findPiExe, opencodeArgs, piAgentArgs } from "./agent_cli.mjs"
import { endStream, isolatedProcessOptions, killProcessTree } from "./process_helpers.mjs"

export function genericAgentHome() {
  return process.env.USERPROFILE || process.env.HOME || ""
}

export function parseGenericAgents(value, fallback = "tura-balanced,tura-direct,codex-main") {
  const raw = String(value || fallback)
  const agents = raw
    .split(",")
    .map((item) => item.trim().toLowerCase())
    .filter(Boolean)
    .map(normalizeGenericAgentId)
  assert(agents.length > 0, "at least one agent is required")
  return agents
}

const GENERIC_AGENT_ALIASES = new Map([
  ["balanced", "tura-balanced"],
  ["direct", "tura-direct"],
  ["tura", "tura-fast-shll"],
  ["tura-balanced", "tura-balanced"],
  ["tura-direct", "tura-direct"],
  ["tura-fast", "tura-fast-shll"],
  ["tura-fast-shll", "tura-fast-shll"],
  ["tura-shll", "tura-shll"],
  ["tura-coding", "tura-shll"],
  ["tura-coding-agent", "tura-shll"],
  ["codex", "codex-main"],
  ["main", "codex-main"],
  ["codex-main", "codex-main"],
  ["codex-main-ponytail", "codex-main-ponytail"],
  ["codex-ponytail", "codex-main-ponytail"],
  ["ponytail", "codex-main-ponytail"],
  ["current", "current-shll"],
  ["current-shll", "current-shll"],
  ["codex-current", "current-shll"],
  ["codex-documents", "codex-documents"],
  ["codex-docs", "codex-documents"],
  ["claude", "claude-code"],
  ["claude-code", "claude-code"],
  ["claude-opus", "claude-code"],
  ["pi", "pi-agent"],
  ["pi-agent", "pi-agent"],
  ["pi-coding-agent", "pi-agent"],
  ["opencode", "opencode"],
  ["open-code", "opencode"],
])

export function normalizeGenericAgentId(value) {
  const text = String(value || "").trim().toLowerCase()
  return GENERIC_AGENT_ALIASES.get(text) || text
}

export function buildGenericAgentRuns(agentIds) {
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

export function genericAgentKind(agentId) {
  const text = String(agentId || "")
  if (text.startsWith("tura-")) return "tura"
  if (text.startsWith("codex-") || text.startsWith("current-")) return "codex"
  if (text === "claude-code") return "claudecode"
  if (text === "pi-agent") return "pi"
  if (text === "opencode") return "opencode"
  return text || "unknown"
}

export function genericAgentMode(agentId) {
  const text = String(agentId || "")
  if (text === "tura-balanced") return "balanced"
  if (text === "tura-direct") return "direct"
  if (text === "tura-fast-shll") return "fast"
  if (text === "tura-shll") return "coding_agent"
  if (text === "codex-main") return "main"
  if (text === "codex-main-ponytail") return "main-ponytail"
  if (text === "codex-documents") return "documents"
  if (text === "current-shll") return "current"
  if (text === "claude-code" || text === "pi-agent" || text === "opencode") return "cli"
  return "unknown"
}

export function modelForGenericAgent(agentId, options = {}) {
  return genericAgentKind(agentId) === "tura" ? options.turaModel : options.model
}

export function priorityEnabled(serviceTier) {
  return String(serviceTier || "").trim().toLowerCase() === "priority"
}

export function findCodexMainExe(repoRoot = process.cwd()) {
  return findCodexExe("main", repoRoot)
}

export function findCodexDocumentsExe(repoRoot = process.cwd()) {
  return findCodexExe("documents", repoRoot)
}

function findCodexExe(kind, repoRoot) {
  const home = genericAgentHome()
  const exeName = process.platform === "win32" ? "codex.exe" : "codex"
  const roots = kind === "main"
    ? [
        process.env.COMMAND_RUN_AGENT_CODEX_MAIN_ROOT,
        path.join(home, "Documents", "codex-main"),
        path.join(home, "codex-main"),
        path.join(home, "RustroverProjects", "codex-main"),
      ]
    : [
        process.env.COMMAND_RUN_AGENT_CODEX_DOCUMENTS_ROOT,
        process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT,
        path.join(home, "Documents", "Codex"),
        path.join(home, "Codex"),
        repoRoot,
      ]
  const candidates = roots
    .filter(Boolean)
    .map((root) => path.join(root, "codex-rs", "target", "debug", exeName))
  return candidates.find((candidate) => fs.existsSync(candidate)) || candidates[0]
}

export function ensureGenericAgentExecutables(agentIds, options = {}) {
  const repoRoot = options.repoRoot || process.cwd()
  const turaExe = options.turaExe || defaultTuraExe(repoRoot)
  const codexMainExe = options.codexMainExe || findCodexMainExe(repoRoot)
  const codexDocumentsExe = options.codexDocumentsExe || findCodexDocumentsExe(repoRoot)
  for (const agentId of agentIds) {
    if (tokenFixtureSourceForAgent(agentId, options)) continue
    if (agentId.startsWith("tura-")) assert(fs.existsSync(turaExe), `missing Tura executable: ${turaExe}`)
    if (agentId === "codex-main" || agentId === "codex-main-ponytail") assert(fs.existsSync(codexMainExe), `missing codex-main executable: ${codexMainExe}`)
    if (agentId === "codex-documents" || agentId === "current-shll") {
      assert(fs.existsSync(codexDocumentsExe), `missing codex documents/current executable: ${codexDocumentsExe}`)
    }
    if (agentId === "pi-agent") assertCommandExists(options.piExe || findPiExe(), "pi")
    if (agentId === "opencode") assertCommandExists(options.opencodeExe || findOpencodeExe(), "opencode")
  }
}

function assertCommandExists(command, label) {
  if (commandExists(command)) return
  assert(false, `missing ${label} executable: ${command}`)
}

function commandExists(command) {
  if (typeof command !== "string" || !command.trim()) return false
  if (command.includes("/") || command.includes("\\")) return fs.existsSync(command)
  const lookup = process.platform === "win32"
    ? spawnSync("where.exe", [command], { encoding: "utf8", windowsHide: true })
    : spawnSync("sh", ["-lc", `command -v ${shellQuote(command)}`], { encoding: "utf8" })
  return lookup.status === 0 && Boolean(String(lookup.stdout || "").trim())
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, "'\\''")}'`
}

function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value || "").trim())
}

function envForAgent(prefix, agentId) {
  const suffix = String(agentId || "")
    .trim()
    .toUpperCase()
    .replace(/[^A-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
  return suffix ? process.env[`${prefix}_${suffix}`] : undefined
}

function defaultDesktopCodexHome() {
  return path.join(genericAgentHome(), ".codex")
}

export function codexHomeForAgent(agentId, options = {}) {
  const explicitHome = firstText(
    envForAgent("COMMAND_RUN_AGENT_CODEX_HOME", agentId),
    agentId === "codex-main" ? process.env.COMMAND_RUN_AGENT_CODEX_MAIN_HOME : "",
    process.env.COMMAND_RUN_AGENT_CODEX_HOME,
  )
  const shouldPrepare = truthy(envForAgent("COMMAND_RUN_AGENT_CODEX_PREPARE_HOME", agentId))
    || truthy(process.env.COMMAND_RUN_AGENT_CODEX_CLEAN_HOME)
    || truthy(process.env.COMMAND_RUN_AGENT_CODEX_PREPARE_HOME)

  if (explicitHome) {
    const resolved = path.resolve(explicitHome)
    if (shouldPrepare) prepareCodexCliHomeForAgent(agentId, resolved, options)
    return resolved
  }

  if (truthy(process.env.COMMAND_RUN_AGENT_CODEX_CLEAN_HOME)) {
    const home = path.join(options.agentDir, "codex-home-clean")
    prepareCodexCliHomeForAgent(agentId, home, options)
    return home
  }

  const home = path.join(options.agentDir, "codex-home")
  prepareCodexCliHomeForAgent(agentId, home, options)
  return home
}

export function prepareCodexCliHomeForAgent(agentId, codexHome, options = {}) {
  const resolvedHome = path.resolve(codexHome)
  guardNonDesktopCodexHome(resolvedHome)
  mkdirp(resolvedHome)
  copyCodexAuth(resolvedHome)
  return resolvedHome
}

function guardNonDesktopCodexHome(codexHome) {
  if (truthy(process.env.COMMAND_RUN_AGENT_ALLOW_GLOBAL_CODEX_HOME)) return
  const target = path.resolve(codexHome).toLowerCase()
  const desktop = path.resolve(defaultDesktopCodexHome()).toLowerCase()
  assert(target !== desktop, `refusing to prepare Desktop CODEX_HOME: ${codexHome}`)
}

function copyCodexAuth(codexHome) {
  const sourceHome = process.env.COMMAND_RUN_AGENT_CODEX_SOURCE_HOME || defaultDesktopCodexHome()
  const authSource = path.join(sourceHome, "auth.json")
  if (!fs.existsSync(authSource)) return
  fs.copyFileSync(authSource, path.join(codexHome, "auth.json"))
}

export function seedCodexPluginCache(codexHome, options = {}) {
  const marketplaceName = pluginSegment(options.marketplaceName || "benchmark")
  const pluginName = pluginSegment(options.pluginName)
  const sourcePluginDir = options.sourcePluginDir
  assert(pluginName, "pluginName is required")
  assert(sourcePluginDir && fs.existsSync(sourcePluginDir), `missing plugin source directory: ${sourcePluginDir}`)
  const cacheVersion = pluginSegment(options.version || "local")
  const targetPlugin = path.join(path.resolve(codexHome), "plugins", "cache", marketplaceName, pluginName, cacheVersion)
  assertPathInside(codexHome, targetPlugin)
  fs.rmSync(targetPlugin, { recursive: true, force: true })
  mkdirp(path.dirname(targetPlugin))
  fs.cpSync(sourcePluginDir, targetPlugin, { recursive: true, force: true })
  return targetPlugin
}

function pluginSegment(value) {
  const text = String(value || "").trim()
  assert(/^[A-Za-z0-9._-]+$/.test(text), `invalid plugin path segment: ${text}`)
  return text
}

function assertPathInside(root, target) {
  const relative = path.relative(path.resolve(root), path.resolve(target))
  assert(relative && !relative.startsWith("..") && !path.isAbsolute(relative), `refusing to write outside ${root}: ${target}`)
}

export async function runGenericAgentCli(options) {
  const {
    agentId,
    workspace,
    agentDir,
    prompt,
    repoRoot = process.cwd(),
    model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5",
    turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`),
    reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "medium",
    serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "default",
    timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 60 * 60_000),
    idleTimeoutMs = Number(process.env.COMMAND_RUN_AGENT_IDLE_TIMEOUT_MS || 0),
    onProgress = null,
  } = options
  mkdirp(agentDir)
  const commandOptions = {
    agentDir,
    workspace,
    prompt,
    repoRoot,
    model,
    turaModel,
    reasoning,
    serviceTier,
  }
  const fixtureSource = tokenFixtureSourceForAgent(agentId, { ...options, ...commandOptions })
  const commandSpec = fixtureSource
    ? tokenFixtureCommand(agentId, fixtureSource, commandOptions)
    : commandForAgent(agentId, commandOptions)
  writeInvocationArchive(agentDir, {
    ...commandSpec,
    agent: agentId,
    workspace,
    input: prompt,
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
    idle_timeout_ms: commandSpec.idleTimeoutMs ?? idleTimeoutMs,
  })
  const result = commandSpec.tokenFixtureSource
    ? await runTokenFixture(commandSpec, {
      agentId,
      agentDir,
      workspace,
      stdoutPath: path.join(agentDir, "stdout.jsonl"),
      stderrPath: path.join(agentDir, "stderr.log"),
      statusPath: path.join(agentDir, "status.json"),
      model: modelForGenericAgent(agentId, { model, turaModel }),
      reasoning,
      serviceTier,
      onProgress,
    })
    : await runLive(commandSpec.command, commandSpec.args, {
    cwd: workspace,
    input: commandSpec.passStdin === false ? null : prompt,
    timeoutMs,
    idleTimeoutMs: commandSpec.idleTimeoutMs ?? idleTimeoutMs,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
    resolveOnTurnCompleted: commandSpec.resolveOnTurnCompleted,
    turnCompletedGraceMs: 1000,
    env: commandSpec.env,
    onProgress,
  })
  result.context_archive = refreshContextAndCallArchive(agentDir, prompt, result.stdout, {
    codexHome: commandSpec.env?.CODEX_HOME,
  })
  result.usage_info = usageForAgent(agentDir, result.stdout, agentId)
  result.events = eventsWithUsageRounds(eventsForAgent(result.stdout, agentId), result.usage_info.usage)
  return result
}

function commandForAgent(agentId, options) {
  if (agentId.startsWith("tura-")) return turaCommand(agentId, options)
  if (agentId === "codex-main" || agentId === "codex-main-ponytail") return codexCommand(agentId, findCodexMainExe(options.repoRoot), options)
  if (agentId === "codex-documents" || agentId === "current-shll") {
    return codexCommand(agentId, findCodexDocumentsExe(options.repoRoot), options)
  }
  if (agentId === "claude-code") return externalCommand(agentId, findClaudeExe(), claudeCodeArgs, options)
  if (agentId === "pi-agent") return externalCommand(agentId, findPiExe(), piAgentArgs, options)
  if (agentId === "opencode") return externalCommand(agentId, findOpencodeExe(), opencodeArgs, options)
  throw new Error(`unsupported agent ${agentId}`)
}

function turaCommand(agentId, options) {
  const command = defaultTuraExe(options.repoRoot)
  assert(fs.existsSync(command), `missing Tura executable: ${command}`)
  const turaMode = genericAgentMode(agentId)
  const embeddedArgs = truthy(envForAgent("COMMAND_RUN_AGENT_TURA_EMBEDDED", agentId))
    || truthy(process.env.COMMAND_RUN_AGENT_TURA_EMBEDDED)
    ? ["--embedded"]
    : []
  const launchId = `benchmark-${agentId}-${process.pid}-${Date.now()}`
  const providerLogPath = path.join(options.agentDir, "provider-log")
  return {
    command,
    args: [
      "exec",
      "--json",
      ...embeddedArgs,
      "--skip-git-repo-check",
      "--session-id",
      launchId,
      "--sandbox",
      "--agent-id",
      turaMode === "coding_agent" ? "coding_agent" : turaMode,
      "-m",
      options.turaModel,
      ...turaServiceTierArgs(options.serviceTier),
      "--model-reasoning-effort",
      options.reasoning,
      "--cwd",
      options.workspace,
    ],
    env: {
      LOG_PATH: providerLogPath,
      TURA_PROJECT_ROOT: options.repoRoot,
      TURA_COMMAND_RUN_SHELL: process.env.COMMAND_RUN_AGENT_TURA_SHELL || "shell_command",
      TURA_COMMAND_RUN_STRICT_JSON: "0",
      TURA_SESSION_REASONING_EFFORT: options.reasoning,
      COMMAND_RUN_AGENT_CONTEXT_ARCHIVE: "1",
    },
    context_kind: `${agentId}-stdin`,
    resolveOnTurnCompleted: false,
  }
}

function codexCommand(agentId, command, options) {
  assert(fs.existsSync(command), `missing ${agentId} executable: ${command}`)
  const codexLogDir = path.join(options.agentDir, "codex-log")
  const providerLogPath = path.join(options.agentDir, "provider-log")
  const codexHome = codexHomeForAgent(agentId, options)
  const cliConfig = codexCliConfigOverrides(agentId, options)
  const extraArgs = codexCliExtraArgs(agentId, options)
  const setupCommands = codexCliSetupCommands(agentId, options)
  const setupLog = setupCommands.length
    ? runCodexCliSetupCommands(agentId, codexCliSetupExe(agentId, command, options), setupCommands, {
      codexHome,
      agentDir: options.agentDir,
      cwd: options.workspace,
    })
    : null
  return {
    command,
    args: [
      "exec",
      "--json",
      "--skip-git-repo-check",
      "-C",
      options.workspace,
      "-m",
      options.model,
      "--dangerously-bypass-approvals-and-sandbox",
      "-c",
      `model_reasoning_effort="${options.reasoning}"`,
      "-c",
      `log_dir="${escapeConfigPath(codexLogDir)}"`,
      ...codexServiceTierArgs(options.serviceTier),
      ...cliConfig.flatMap((override) => ["-c", override]),
      ...extraArgs,
    ],
    env: {
      COMMAND_RUN_AGENT_CONTEXT_ARCHIVE: "1",
      CODEX_LOG_DIR: codexLogDir,
      LOG_PATH: providerLogPath,
      OPENAI_PROVIDER_LOG: providerLogPath,
      ...(codexHome ? { CODEX_HOME: codexHome } : {}),
    },
    codex_home: codexHome,
    codex_cli_config_overrides: cliConfig,
    codex_cli_extra_args: extraArgs,
    codex_cli_setup_commands: setupCommands,
    codex_cli_setup_log: setupLog,
    idleTimeoutMs: codexIdleTimeoutMs(agentId),
    context_kind: `${agentId}-stdin`,
    resolveOnTurnCompleted: true,
  }
}

function codexIdleTimeoutMs(agentId) {
  return Number(
    envForAgent("COMMAND_RUN_AGENT_CODEX_IDLE_TIMEOUT_MS", agentId)
    || envForAgent("COMMAND_RUN_AGENT_IDLE_TIMEOUT_MS", agentId)
    || process.env.COMMAND_RUN_AGENT_CODEX_IDLE_TIMEOUT_MS
    || process.env.COMMAND_RUN_AGENT_IDLE_TIMEOUT_MS
    || 20 * 60_000,
  )
}

function codexCliConfigOverrides(agentId, options = {}) {
  return [
    ...normalizeCliConfigOverrides(process.env.COMMAND_RUN_AGENT_CODEX_CLI_CONFIG),
    ...normalizeCliConfigOverrides(envForAgent("COMMAND_RUN_AGENT_CODEX_CLI_CONFIG", agentId)),
    ...normalizeCliConfigOverrides(options.codexCliConfig),
  ]
}

function codexCliExtraArgs(agentId, options = {}) {
  return [
    ...normalizeCliArgs(process.env.COMMAND_RUN_AGENT_CODEX_CLI_EXTRA_ARGS),
    ...normalizeCliArgs(envForAgent("COMMAND_RUN_AGENT_CODEX_CLI_EXTRA_ARGS", agentId)),
    ...normalizeCliArgs(options.codexCliExtraArgs),
  ]
}

function codexCliSetupCommands(agentId, options = {}) {
  return [
    ...normalizeCliCommandList(process.env.COMMAND_RUN_AGENT_CODEX_CLI_SETUP),
    ...normalizeCliCommandList(envForAgent("COMMAND_RUN_AGENT_CODEX_CLI_SETUP", agentId)),
    ...normalizeCliCommandList(options.codexCliSetup),
  ]
}

function codexCliSetupExe(agentId, defaultCommand, options = {}) {
  return firstText(
    options.codexCliSetupExe,
    envForAgent("COMMAND_RUN_AGENT_CODEX_CLI_SETUP_EXE", agentId),
    process.env.COMMAND_RUN_AGENT_CODEX_CLI_SETUP_EXE,
    defaultCommand,
  )
}

export function runCodexCliSetupCommands(agentId, command, setupCommands, options = {}) {
  assert(options.codexHome, "Codex CLI setup commands require an isolated CODEX_HOME")
  assert(command && commandExists(command), `missing Codex CLI setup executable: ${command}`)
  const setupCommand = resolveSpawnCommand(command)
  const codexHome = path.resolve(options.codexHome)
  guardNonDesktopCodexHome(codexHome)
  mkdirp(codexHome)
  const logPath = path.join(options.agentDir || codexHome, "codex-cli-setup.jsonl")
  const env = { ...process.env, CODEX_HOME: codexHome }
  const records = []
  for (const args of normalizeCliCommandList(setupCommands)) {
    const startedAt = new Date().toISOString()
    const started = performance.now()
    const result = spawnSync(setupCommand, args, {
      cwd: options.cwd || process.cwd(),
      env,
      encoding: "utf8",
      shell: process.platform === "win32" && /\.(cmd|bat)$/i.test(setupCommand),
      windowsHide: true,
      timeout: Number(options.timeoutMs || process.env.COMMAND_RUN_AGENT_CODEX_CLI_SETUP_TIMEOUT_MS || 120_000),
    })
    const record = {
      type: "codex_cli_setup.completed",
      agent_id: agentId,
      command: setupCommand,
      requested_command: command,
      args,
      codex_home: codexHome,
      started_at: startedAt,
      duration_ms: Math.round(performance.now() - started),
      exit_code: result.status,
      signal: result.signal || null,
      error: result.error ? String(result.error.message || result.error) : null,
      stdout_tail: tailText(result.stdout, 4000),
      stderr_tail: tailText(result.stderr, 4000),
    }
    records.push(record)
    appendJsonl(logPath, record)
    assert.equal(result.status, 0, `Codex CLI setup failed for ${agentId}: ${args.join(" ")}\n${record.stderr_tail || record.stdout_tail}`)
  }
  return logPath
}

function resolveSpawnCommand(command) {
  const text = String(command || "").trim()
  if (process.platform !== "win32") return text
  if (text.includes("/") || text.includes("\\")) return text
  const lookup = spawnSync("where.exe", [text], { encoding: "utf8", windowsHide: true })
  if (lookup.status !== 0) return text
  const candidates = String(lookup.stdout || "").split(/\r?\n/).map((line) => line.trim()).filter(Boolean)
  return candidates.find((candidate) => /\.(cmd|exe|bat)$/i.test(candidate)) || candidates[0] || text
}

function normalizeCliConfigOverrides(value) {
  if (!value) return []
  if (Array.isArray(value)) return value.map(String).map((item) => item.trim()).filter(Boolean)
  if (typeof value === "object") {
    return Object.entries(value).map(([key, item]) => `${key}=${tomlLiteral(item)}`)
  }
  const text = String(value).trim()
  if (!text) return []
  try {
    const parsed = JSON.parse(text)
    return normalizeCliConfigOverrides(parsed)
  } catch {}
  return text.split(/\r?\n/).map((item) => item.trim()).filter(Boolean)
}

function normalizeCliCommandList(value) {
  if (!value) return []
  if (Array.isArray(value)) {
    if (value.every((item) => typeof item === "string")) return [value.map(String).filter(Boolean)]
    return value.map((item) => normalizeCliArgs(item)).filter((args) => args.length > 0)
  }
  const text = String(value).trim()
  if (!text) return []
  try {
    return normalizeCliCommandList(JSON.parse(text))
  } catch {}
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.split(/\s+/).filter(Boolean))
}

function normalizeCliArgs(value) {
  if (!value) return []
  if (Array.isArray(value)) return value.map(String).filter(Boolean)
  const text = String(value).trim()
  if (!text) return []
  try {
    const parsed = JSON.parse(text)
    return Array.isArray(parsed) ? parsed.map(String).filter(Boolean) : []
  } catch {}
  return text.split(/\r?\n/).map((item) => item.trim()).filter(Boolean)
}

function tailText(value, limit) {
  const text = String(value || "")
  return text.slice(Math.max(0, text.length - limit))
}

function appendJsonl(filePath, record) {
  mkdirp(path.dirname(filePath))
  fs.appendFileSync(filePath, `${JSON.stringify(record)}\n`, "utf8")
}

function tomlLiteral(value) {
  if (typeof value === "boolean") return value ? "true" : "false"
  if (typeof value === "number" && Number.isFinite(value)) return String(value)
  if (Array.isArray(value)) return `[${value.map(tomlLiteral).join(", ")}]`
  return JSON.stringify(String(value ?? ""))
}

function externalCommand(agentId, command, argBuilder, options) {
  return {
    command,
    args: argBuilder(options.prompt || "", options),
    env: { COMMAND_RUN_AGENT_CONTEXT_ARCHIVE: "1" },
    context_kind: `${agentId}-print`,
    resolveOnTurnCompleted: false,
    passStdin: false,
  }
}

function tokenFixtureSourceForAgent(agentId, options = {}) {
  const source = options.tokenFixtureSource || process.env.COMMAND_RUN_AGENT_TOKEN_FIXTURE_SOURCE
  if (!source) return null
  const agents = String(options.tokenFixtureAgents || process.env.COMMAND_RUN_AGENT_TOKEN_FIXTURE_AGENTS || "pi-agent,opencode")
  const normalizedAgent = normalizeGenericAgentId(agentId)
  const enabled = agents.trim() === "*"
    || new Set(agents.split(",").map(normalizeGenericAgentId).filter(Boolean)).has(normalizedAgent)
  if (!enabled) return null
  assert(fs.existsSync(source), `missing token fixture source: ${source}`)
  return source
}

function tokenFixtureCommand(agentId, source, options) {
  return {
    command: "benchmark-token-fixture",
    args: [source],
    env: {
      COMMAND_RUN_AGENT_CONTEXT_ARCHIVE: "1",
      COMMAND_RUN_AGENT_TOKEN_FIXTURE_SOURCE: source,
      COMMAND_RUN_AGENT_TOKEN_FIXTURE_BACKEND: "codex",
    },
    context_kind: `${agentId}-codex-token-fixture`,
    resolveOnTurnCompleted: false,
    tokenFixtureSource: source,
    model: modelForGenericAgent(agentId, options),
    reasoning: options.reasoning,
    service_tier: options.serviceTier,
  }
}

async function runTokenFixture(commandSpec, options = {}) {
  const started = performance.now()
  const stdout = buildCodexTokenFixtureStdout(fs.readFileSync(commandSpec.tokenFixtureSource, "utf8"), options.agentId, {
    fixtureSourcePath: commandSpec.tokenFixtureSource,
    model: options.model,
    reasoning: options.reasoning,
    serviceTier: options.serviceTier,
  })
  writeFile(options.stdoutPath, stdout)
  writeFile(options.stderrPath, "")
  const result = {
    command: commandSpec.command,
    args: commandSpec.args,
    status: 0,
    signal: null,
    stdout,
    stderr: "",
    duration_ms: Math.round(performance.now() - started),
    first_output_ms: 0,
    last_progress_ms: Math.round(performance.now() - started),
    timed_out: false,
    error: null,
    fixture_backend: "codex",
    fixture_source_path: commandSpec.tokenFixtureSource,
  }
  writeJson(options.statusPath, {
    command: commandSpec.command,
    args: commandSpec.args,
    cwd: options.workspace,
    elapsed_ms: result.duration_ms,
    stdout_bytes: Buffer.byteLength(stdout),
    stderr_bytes: 0,
    status: "closed",
    result,
  })
  options.onProgress?.(result)
  return result
}

export function buildCodexTokenFixtureStdout(sourceJsonl, agentId, options = {}) {
  const records = parseJsonl(sourceJsonl)
  const tokenUsageRecords = records
    .map((record, index) => ({ record, index, usage: record?.type === "thread.token_usage.updated" ? record.usage : null }))
    .filter((item) => hasTokenUsage(item.usage))
  const usageRecords = tokenUsageRecords.length > 0
    ? tokenUsageRecords
    : records
      .map((record, index) => ({ record, index, usage: usageCandidate(record) }))
      .filter((item) => hasTokenUsage(item.usage))
  const sourceAgentId = firstText(options.sourceAgentId, "codex-main")
  const eventPrefix = fixtureEventPrefix(agentId)
  const now = new Date().toISOString()
  return usageRecords.map((item, roundIndex) => {
    const usage = normalizeEventUsage(item.usage)
    const totalUsage = normalizeEventUsage(item.record?.total_usage || item.usage)
    const timestamp = firstText(item.record?.timestamp, item.record?.time, now)
    return JSON.stringify({
      type: `${eventPrefix}.round.completed`,
      agent_id: agentId,
      agent_kind: genericAgentKind(agentId),
      agent_mode: genericAgentMode(agentId),
      model: firstText(options.model, item.record?.model, "unknown"),
      reasoning: firstText(options.reasoning, "unknown"),
      service_tier: firstText(options.serviceTier, "unknown"),
      priority_enabled: priorityEnabled(options.serviceTier),
      roundId: `${agentId}-codex-fixture-round-${roundIndex + 1}`,
      turn_id: firstText(item.record?.turn_id, item.record?.turnId, item.record?.id, `${sourceAgentId}-source-${item.index + 1}`),
      started_at: timestamp,
      ended_at: timestamp,
      round_source: "codex-token-fixture",
      fixture_backend: "codex",
      fixture_source_path: options.fixtureSourcePath || null,
      source_agent_id: sourceAgentId,
      source_event_type: firstText(item.record?.type, "unknown"),
      source_round_index: item.index,
      usage,
      metrics: { durationMs: 0 },
      source_total_usage: totalUsage,
      messages: [{
        role: "assistant",
        text: `${agentId} token fixture round ${roundIndex + 1} generated from ${sourceAgentId} ${firstText(item.record?.type, "usage")} data.`,
      }],
    })
  }).join("\n") + (usageRecords.length ? "\n" : "")
}

function fixtureEventPrefix(agentId) {
  if (agentId === "pi-agent") return "pi"
  if (agentId === "opencode") return "opencode"
  if (String(agentId || "").startsWith("codex-")) return "codex"
  return genericAgentKind(agentId)
}

function normalizeEventUsage(usage) {
  const normalized = {
    input_tokens: inputTokenCount(usage || {}),
    cached_input_tokens: cacheReadTokenCount(usage || {}),
    output_tokens: Number(usage?.output_tokens ?? usage?.outputTokens ?? usage?.completion_tokens ?? 0),
    reasoning_tokens: Number(usage?.reasoning_tokens ?? usage?.reasoningTokens ?? usage?.reasoning_output_tokens ?? usage?.reasoning ?? usage?.output_tokens_details?.reasoning_tokens ?? usage?.completion_tokens_details?.reasoning_tokens ?? 0),
    total_tokens: Number(usage?.total_tokens ?? usage?.totalTokens ?? 0),
  }
  if (!normalized.total_tokens) normalized.total_tokens = normalized.input_tokens + normalized.output_tokens
  return normalized
}

export async function runLive(command, args, options = {}) {
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
  let lastProgressMs = null
  let timedOut = false
  let settled = false
  let completedTimer = null
  let exitTimer = null

  const writeStatus = (extra) => {
    if (!statusPath) return
    writeJson(statusPath, {
      command,
      args,
      cwd: options.cwd,
      elapsed_ms: Math.round(performance.now() - started),
      stdout_bytes: Buffer.byteLength(stdout),
      stderr_bytes: Buffer.byteLength(stderr),
      ...extra,
    })
  }

  writeStatus({ status: "running" })
  return new Promise((resolve) => {
    const timeoutLimitMs = Number(options.timeoutMs || 60 * 60_000)
    const idleTimeoutMs = Number(options.idleTimeoutMs || process.env.COMMAND_RUN_AGENT_IDLE_TIMEOUT_MS || 0)
    const child = spawn(command, args, isolatedProcessOptions({
      cwd: options.cwd || process.cwd(),
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
      shell: shouldUseWindowsShell(command),
    }))
    const timer = setTimeout(() => {
      timedOut = true
      try { killProcessTree(child.pid) } catch {}
      settle(1, null, `timed out after ${timeoutLimitMs}ms`)
    }, timeoutLimitMs)
    const idleTimer = idleTimeoutMs > 0
      ? setInterval(() => {
        if (settled) return
        const elapsed = performance.now() - started
        const progressAt = lastProgressMs ?? 0
        if (elapsed - progressAt < idleTimeoutMs) return
        timedOut = true
        try { killProcessTree(child.pid) } catch {}
        settle(1, null, `no stdout/stderr progress for ${idleTimeoutMs}ms`)
      }, Math.min(Math.max(Math.floor(idleTimeoutMs / 4), 1000), 30_000))
      : null

    function settle(status, signal, error) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      if (idleTimer) clearInterval(idleTimer)
      clearTimeout(completedTimer)
      clearTimeout(exitTimer)
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
        last_progress_ms: lastProgressMs,
        timed_out: timedOut,
        error,
      }
      writeStatus({ status: error ? "error" : "closed", result })
      resolve(result)
    }

    const record = (kind, chunk) => {
      const text = chunk.toString("utf8")
      if (firstOutputMs === null) firstOutputMs = Math.round(performance.now() - started)
      lastProgressMs = Math.round(performance.now() - started)
      if (kind === "stdout") {
        stdout += text
        stdoutStream?.write(text)
        if (options.resolveOnTurnCompleted && stdout.includes('"type":"turn.completed"')) {
          completedTimer ??= setTimeout(() => {
            try { killProcessTree(child.pid) } catch {}
            settle(0, null, null)
          }, Number(options.turnCompletedGraceMs || 1000))
        }
      } else {
        stderr += text
        stderrStream?.write(text)
      }
      writeStatus({ status: "running" })
      options.onProgress?.({
        status: null,
        signal: null,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        first_output_ms: firstOutputMs,
        last_progress_ms: lastProgressMs,
        timed_out: false,
        error: null,
      })
    }

    child.stdout?.on("data", (chunk) => record("stdout", chunk))
    child.stderr?.on("data", (chunk) => record("stderr", chunk))
    child.on("error", (error) => settle(null, null, String(error?.stack || error?.message || error)))
    child.on("exit", (status, signal) => {
      exitTimer ??= setTimeout(() => {
        settle(status, signal, timedOut ? `timed out after ${timeoutLimitMs}ms` : null)
      }, Number(options.exitGraceMs || 5000))
    })
    child.on("close", (status, signal) => settle(status, signal, timedOut ? `timed out after ${timeoutLimitMs}ms` : null))
    if (options.input) child.stdin.end(options.input)
    else child.stdin.end()
  })
}

export function usageForAgent(agentDir, stdout, agentId = "") {
  if (agentId === "claude-code") {
    const usage = agentUsageFromJsonl(stdout)
    return {
      usage: normalizeUsage({
        usage_events: usage.input || usage.output || usage.total ? 1 : 0,
        input_tokens: usage.input,
        cached_input_tokens: usage.cached,
        output_tokens: usage.output,
        reasoning_tokens: usage.reasoning,
        total_tokens: usage.total,
      }),
      usage_source: `${agentId}-jsonl`,
      provider_calls: [],
    }
  }
  const stdoutUsage = usageFromJsonl(stdout)
  const provider = usageFromProviderLogs(path.join(agentDir, "provider-log"))
  if (provider.totals.usage_events > 0) {
    return { usage: normalizeUsage(provider.totals), usage_source: "provider-log", provider_calls: provider.calls }
  }
  const codexRollout = usageFromCodexRollouts(agentDir)
  if (codexRollout.totals.usage_events > 0) {
    return { usage: normalizeUsage(codexRollout.totals), usage_source: "codex-rollout", provider_calls: [], codex_rollouts: codexRollout.paths }
  }
  return { usage: normalizeUsage(stdoutUsage), usage_source: stdoutUsage.usage_events > 0 ? "stdout-jsonl" : "none", provider_calls: [] }
}

export function eventsForAgent(stdout, agentId = "") {
  if (agentId === "claude-code") return agentEventStats(stdout)
  const events = parseJsonl(stdout)
  const usageEvents = usageFromJsonl(stdout).usage_events
  const stats = {
    events: events.length,
    thread_started: 0,
    turn_started: 0,
    turn_completed: 0,
    round_completed: 0,
    token_usage_updates: 0,
    agent_messages: 0,
    command_executions: 0,
    commands_completed: 0,
    commands_failed: 0,
    file_changes: 0,
  }
  for (const event of events) {
    if (event.type === "thread.started") stats.thread_started += 1
    if (event.type === "turn.started" || event.type === "turn_start") stats.turn_started += 1
    if (event.type === "turn.completed" || event.type === "turn_end") stats.turn_completed += 1
    if (event.type === "thread.token_usage.updated") stats.token_usage_updates += 1
    if (event.type === "step_start") stats.turn_started += 1
    if (event.type === "step_finish") stats.turn_completed += 1
    if (/(^|\.)round\.completed$/.test(String(event.type || ""))) stats.round_completed += 1
    if (event.item?.type === "agent_message" || event.item?.type === "assistant_message" || (event.type === "message_end" && event.message?.role === "assistant") || (event.type === "text" && event.part?.text)) stats.agent_messages += 1
    if (event.item?.type === "file_change") stats.file_changes += 1
    if (event.item?.type === "command_execution") {
      stats.command_executions += 1
      if (event.item.status === "completed") stats.commands_completed += 1
      if (event.item.status === "failed") stats.commands_failed += 1
    }
  }
  stats.llm_rounds = usageEvents || stats.token_usage_updates || stats.round_completed || stats.turn_completed
  if (String(agentId || "").startsWith("codex-")) stats.llm_rounds = Math.max(stats.llm_rounds, stats.agent_messages)
  stats.callback_ok = stats.llm_rounds > 0 || stats.command_executions > 0 || stats.file_changes > 0
  return stats
}

export function eventsWithUsageRounds(events = {}, usage = {}) {
  const usageRounds = Number(usage?.usage_events || 0)
  const currentRounds = Number(events?.llm_rounds || 0)
  const llmRounds = Math.max(currentRounds, usageRounds)
  return {
    ...(events || {}),
    llm_rounds: llmRounds,
    callback_ok: Boolean(events?.callback_ok) || llmRounds > 0,
  }
}

export function aggregateGenericUsage(results) {
  const total = emptyUsage()
  for (const result of Array.isArray(results) ? results : []) {
    const usage = normalizeUsage(result?.usage || {})
    total.usage_events += Number(usage.usage_events || 0)
    total.input_tokens += Number(usage.input_tokens || 0)
    total.output_tokens += Number(usage.output_tokens || 0)
    total.reasoning_tokens += Number(usage.reasoning_tokens || 0)
    total.cached_input_tokens += Number(usage.cached_input_tokens || 0)
    total.cache_write_tokens += Number(usage.cache_write_tokens || 0)
    total.total_tokens += Number(usage.total_tokens || 0)
    total.latency_ms += Number(usage.latency_ms || 0)
  }
  return normalizeUsage(total)
}

export function refreshContextAndCallArchive(agentDir, prompt = "", stdout = "", options = {}) {
  const archiveDir = path.join(agentDir, "context-and-calls")
  mkdirp(archiveDir)
  const inputPromptPath = path.join(archiveDir, "input-prompt.md")
  const stdoutSnapshotPath = path.join(archiveDir, "stdout-snapshot.jsonl")
  const providerCallsPath = path.join(archiveDir, "provider-calls-full.jsonl")
  const codexRolloutPathsPath = path.join(archiveDir, "codex-rollout-paths.json")
  writeFile(inputPromptPath, prompt || "")
  writeFile(stdoutSnapshotPath, stdout || "")
  const providerCalls = providerLogRecords(path.join(agentDir, "provider-log"))
  const codexRolloutPaths = archiveCodexRollouts(archiveDir, stdout, options)
  writeFile(providerCallsPath, providerCalls.map((record) => JSON.stringify(record)).join("\n") + (providerCalls.length ? "\n" : ""))
  writeJson(codexRolloutPathsPath, codexRolloutPaths)
  return {
    archive_dir: archiveDir,
    input_prompt_path: inputPromptPath,
    stdout_snapshot_path: stdoutSnapshotPath,
    provider_calls_full_path: providerCallsPath,
    provider_call_count: providerCalls.length,
    codex_rollout_paths_path: codexRolloutPathsPath,
    codex_rollout_paths: codexRolloutPaths,
    codex_rollout_count: codexRolloutPaths.length,
  }
}

function archiveCodexRollouts(archiveDir, stdout = "", options = {}) {
  const threadIds = codexThreadIdsFromStdout(stdout)
  if (threadIds.length === 0) return []
  const rolloutDir = path.join(archiveDir, "codex-rollouts")
  const sourcePaths = findCodexRolloutFiles(threadIds, options.codexHome)
  const archivedPaths = []
  for (const [index, sourcePath] of sourcePaths.entries()) {
    const targetPath = path.join(rolloutDir, `${String(index + 1).padStart(3, "0")}-${path.basename(sourcePath)}`)
    try {
      mkdirp(path.dirname(targetPath))
      fs.copyFileSync(sourcePath, targetPath)
      archivedPaths.push(targetPath)
    } catch {}
  }
  return archivedPaths
}

function codexThreadIdsFromStdout(stdout = "") {
  const ids = []
  const seen = new Set()
  for (const event of parseJsonl(stdout)) {
    const id = firstText(event.thread_id, event.threadId, event.payload?.id)
    if (event?.type !== "thread.started" || !id || seen.has(id)) continue
    seen.add(id)
    ids.push(id)
  }
  return ids
}

function findCodexRolloutFiles(threadIds = [], codexHome = null) {
  const needles = new Set(threadIds.map((id) => String(id || "").trim()).filter(Boolean))
  if (needles.size === 0) return []
  const roots = [
    ...(codexHome ? [
      path.join(codexHome, "sessions"),
      path.join(codexHome, "archived_sessions"),
    ] : []),
    path.join(os.homedir(), ".codex", "sessions"),
    path.join(os.homedir(), ".codex", "archived_sessions"),
  ]
  const files = []
  const seen = new Set()
  for (const root of roots) collectCodexRolloutFiles(root, needles, files, seen)
  files.sort((a, b) => fs.statSync(a).mtimeMs - fs.statSync(b).mtimeMs || a.localeCompare(b))
  return files
}

function collectCodexRolloutFiles(root, needles, files, seen) {
  if (!fs.existsSync(root)) return
  const stack = [root]
  while (stack.length > 0) {
    const current = stack.pop()
    let entries = []
    try {
      entries = fs.readdirSync(current, { withFileTypes: true })
    } catch {
      continue
    }
    for (const entry of entries) {
      const full = path.join(current, entry.name)
      if (entry.isDirectory()) {
        stack.push(full)
        continue
      }
      if (!entry.isFile() || !entry.name.endsWith(".jsonl")) continue
      if (![...needles].some((id) => entry.name.includes(id))) continue
      if (seen.has(full)) continue
      seen.add(full)
      files.push(full)
    }
  }
}

function writeInvocationArchive(agentDir, details) {
  const archiveDir = path.join(agentDir, "context-and-calls")
  mkdirp(archiveDir)
  writeFile(path.join(archiveDir, "input-prompt.md"), details.input || "")
  writeJson(path.join(archiveDir, "invocation.json"), {
    agent: details.agent,
    context_kind: details.context_kind,
    command: details.command,
    args: details.args,
    cwd: details.workspace,
    workspace: details.workspace,
    env: safeArchiveEnv(details.env),
    model: details.model,
    tura_model: details.tura_model,
    reasoning: details.reasoning,
    service_tier: details.service_tier,
    timeout_ms: details.timeout_ms,
    idle_timeout_ms: details.idle_timeout_ms || null,
    codex_home: details.codex_home || null,
    codex_cli_config_overrides: details.codex_cli_config_overrides || [],
    codex_cli_extra_args: details.codex_cli_extra_args || [],
  })
}

function safeArchiveEnv(env) {
  const allowed = new Set([
    "CODEX_HOME",
    "CODEX_LOG_DIR",
    "COMMAND_RUN_AGENT_CONTEXT_ARCHIVE",
    "COMMAND_RUN_AGENT_TOKEN_FIXTURE_BACKEND",
    "COMMAND_RUN_AGENT_TOKEN_FIXTURE_SOURCE",
    "LOG_PATH",
    "OPENAI_PROVIDER_LOG",
    "TURA_COMMAND_RUN_SHELL",
    "TURA_COMMAND_RUN_STRICT_JSON",
    "TURA_PROJECT_ROOT",
    "TURA_SESSION_REASONING_EFFORT",
  ])
  return Object.fromEntries(Object.entries(env || {}).filter(([key]) => allowed.has(key)))
}

function parseJsonl(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try { return JSON.parse(line) } catch { return null }
    })
    .filter(Boolean)
}

function addUsage(totals, usage) {
  if (!hasTokenUsage(usage)) return
  totals.usage_events += 1
  totals.input_tokens += inputTokenCount(usage)
  totals.output_tokens += Number(usage.output_tokens ?? usage.outputTokens ?? usage.completion_tokens ?? usage.output ?? 0)
  totals.reasoning_tokens += Number(usage.reasoning_tokens ?? usage.reasoningTokens ?? usage.reasoning_output_tokens ?? usage.reasoning ?? usage.output_tokens_details?.reasoning_tokens ?? usage.completion_tokens_details?.reasoning_tokens ?? 0)
  totals.cached_input_tokens += cacheReadTokenCount(usage)
  totals.cache_write_tokens += cacheWriteTokenCount(usage)
  totals.total_tokens += Number(usage.total_tokens ?? usage.totalTokens ?? usage.total ?? 0)
  totals.latency_ms += Number(usage.latency_ms ?? usage.durationMs ?? usage.duration_ms ?? 0)
}

function inputTokenCount(usage) {
  const standardInput = firstFiniteNumber(usage.input_tokens, usage.inputTokens, usage.prompt_tokens)
  if (standardInput !== null) return standardInput
  return Number(usage.input || 0) + cacheReadTokenCount(usage) + cacheWriteTokenCount(usage)
}

function cacheReadTokenCount(usage) {
  return Number(usage.cached_input_tokens ?? usage.cacheInputTokens ?? usage.cache_read_input_tokens ?? usage.cacheRead ?? usage.cache?.read ?? usage.input_tokens_details?.cached_tokens ?? usage.prompt_tokens_details?.cached_tokens ?? 0)
}

function cacheWriteTokenCount(usage) {
  return Number(usage.cache_write_tokens ?? usage.cacheWriteTokens ?? usage.cacheWrite ?? usage.cache?.write ?? usage.input_tokens_details?.cache_write_tokens ?? usage.prompt_tokens_details?.cache_creation_tokens ?? 0)
}

function firstFiniteNumber(...values) {
  for (const value of values) {
    const number = Number(value)
    if (Number.isFinite(number)) return number
  }
  return null
}

function usageFromJsonl(stdout) {
  const totals = emptyUsage()
  const events = parseJsonl(stdout)
  const compactUsage = compactUsageEvents(events)
  const tokenUsageUpdates = events.filter((event) => event?.type === "thread.token_usage.updated" && hasTokenUsage(event.usage))
  if (tokenUsageUpdates.length > 0) {
    for (const event of mergeUsageEvents(tokenUsageUpdates, compactUsage)) addUsage(totals, usageCandidate(event))
    return normalizeUsage(totals)
  }
  const turnEndUsage = events.filter((event) => event?.type === "turn_end" && hasTokenUsage(event.message?.usage))
  if (turnEndUsage.length > 0) {
    for (const event of mergeUsageEvents(turnEndUsage, compactUsage)) addUsage(totals, usageCandidate(event))
    return normalizeUsage(totals)
  }
  const opencodeStepUsage = events.filter((event) => event?.type === "step_finish" && hasTokenUsage(event.part?.tokens))
  if (opencodeStepUsage.length > 0) {
    for (const event of mergeUsageEvents(opencodeStepUsage, compactUsage)) addUsage(totals, usageCandidate(event))
    return normalizeUsage(totals)
  }
  for (const event of events) {
    const usage = usageCandidate(event)
    if (usage) addUsage(totals, usage)
  }
  return normalizeUsage(totals)
}

function compactUsageEvents(events) {
  return (Array.isArray(events) ? events : []).filter((event) => isCompactUsageEvent(event) && hasTokenUsage(usageCandidate(event)))
}

function isCompactUsageEvent(event) {
  if (!event || typeof event !== "object") return false
  const text = [
    event.type,
    event.event,
    event.event_type,
    event.eventType,
    event.kind,
    event.name,
    event.action,
  ].map((value) => String(value || "").toLowerCase()).join(" ")
  return /\b(compact|compaction|summarize|summary)\b/.test(text)
}

function mergeUsageEvents(primary, extra) {
  const merged = []
  const seen = new Set()
  for (const event of [...(Array.isArray(primary) ? primary : []), ...(Array.isArray(extra) ? extra : [])]) {
    if (!event || seen.has(event)) continue
    seen.add(event)
    merged.push(event)
  }
  return merged
}

function usageCandidate(event) {
  if (!event || typeof event !== "object") return null
  return [
    event.usage,
    event.metrics,
    event.runtime_usage,
    event.message?.usage,
    event.result?.usage,
    event.assistantMessageEvent?.usage,
    event.response?.usage,
    event.body?.usage,
    event.part?.tokens,
    event.payload?.info?.last_token_usage,
  ].find(hasTokenUsage) || null
}

function hasTokenUsage(usage) {
  if (!usage || typeof usage !== "object") return false
  return [
    usage.input_tokens,
    usage.inputTokens,
    usage.prompt_tokens,
    usage.input,
    usage.output_tokens,
    usage.outputTokens,
    usage.completion_tokens,
    usage.output,
    usage.total_tokens,
    usage.totalTokens,
    usage.total,
    usage.reasoning_tokens,
    usage.reasoningTokens,
    usage.reasoning_output_tokens,
    usage.cached_input_tokens,
    usage.cacheInputTokens,
    usage.cache_read_input_tokens,
    usage.cacheRead,
    usage.cache?.read,
  ].some((value) => Number(value || 0) > 0)
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

function normalizeUsage(usage) {
  const normalized = { ...emptyUsage(), ...(usage || {}) }
  if (!Number(normalized.total_tokens || 0)) {
    normalized.total_tokens = Number(normalized.input_tokens || 0) + Number(normalized.output_tokens || 0)
  }
  return normalized
}

function firstText(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.trim()) return value
    if (typeof value === "number" || typeof value === "boolean") return String(value)
  }
  return ""
}

function usageFromProviderLogs(logRoot) {
  const totals = emptyUsage()
  const calls = []
  for (const payload of providerLogRecords(logRoot)) {
    if (payload?.type !== "llm_call") continue
    const usage = payload.metrics?.usage || payload.usage
    if (!usage) continue
    addUsage(totals, usage)
    calls.push({
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
  return { totals: normalizeUsage(totals), calls }
}

function usageFromCodexRollouts(agentDir) {
  const totals = emptyUsage()
  const paths = codexRolloutArchivePaths(agentDir)
  for (const rolloutPath of paths) {
    const records = parseJsonl(readText(rolloutPath))
    const seenTotals = new Set()
    for (const record of records) {
      const info = record?.payload?.type === "token_count" ? record.payload.info : null
      const usage = info?.last_token_usage
      if (!hasTokenUsage(usage)) continue
      const totalKey = JSON.stringify(info?.total_token_usage || usage)
      if (seenTotals.has(totalKey)) continue
      seenTotals.add(totalKey)
      addUsage(totals, usage)
    }
  }
  return { totals: normalizeUsage(totals), paths }
}

function codexRolloutArchivePaths(agentDir) {
  const pathsPath = path.join(agentDir, "context-and-calls", "codex-rollout-paths.json")
  const parsed = parseJsonArray(readText(pathsPath))
  if (parsed.length > 0) return parsed.filter((item) => typeof item === "string" && fs.existsSync(item))
  const rolloutDir = path.join(agentDir, "context-and-calls", "codex-rollouts")
  return jsonlFilesUnder(rolloutDir).sort()
}

function providerLogRecords(logRoot) {
  const records = []
  for (const file of jsonFilesUnder(logRoot).sort()) {
    try {
      records.push(JSON.parse(fs.readFileSync(file, "utf8")))
    } catch {}
  }
  return records
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

function jsonlFilesUnder(root) {
  if (!fs.existsSync(root)) return []
  const files = []
  const stack = [root]
  while (stack.length > 0) {
    const current = stack.pop()
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(full)
      else if (entry.isFile() && entry.name.endsWith(".jsonl")) files.push(full)
    }
  }
  return files
}

function readText(file) {
  if (!file || !fs.existsSync(file)) return ""
  try {
    return fs.readFileSync(file, "utf8")
  } catch {
    return ""
  }
}

function parseJsonArray(text) {
  try {
    const parsed = JSON.parse(String(text || ""))
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function defaultTuraExe(repoRoot) {
  return path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
}

function codexServiceTierArgs(serviceTier) {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return ["-c", `service_tier="${tier}"`]
}

function turaServiceTierArgs(serviceTier) {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return tier === "priority" ? ["-p"] : []
}

function escapeConfigPath(value) {
  return String(value).replace(/\\/g, "\\\\").replace(/"/g, "\\\"")
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function shouldUseWindowsShell(command) {
  if (process.platform !== "win32") return false
  const text = String(command || "").toLowerCase()
  if (text.endsWith(".cmd") || text.endsWith(".bat") || text.endsWith(".ps1")) return true
  return !text.includes("/") && !text.includes("\\") && !text.endsWith(".exe")
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text, "utf8")
}

function writeJson(file, value) {
  writeFile(file, `${JSON.stringify(value, null, 2)}\n`)
}
