import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import process from "node:process"
import { spawnSync } from "node:child_process"
import {
  businessRunPaths,
  businessTargetRoot,
  defaultUserWorkspace,
  normalizeBusinessSummary as normalizeBaseBusinessSummary,
  userHome,
} from "../../tests/business/business_lib_business_paths.mjs"

export { businessRunPaths, businessTargetRoot, defaultUserWorkspace, userHome }

export function normalizeBusinessSummary(summary, paths, extras = {}) {
  const normalized = normalizeBaseBusinessSummary(summary, paths, extras)
  writeBenchmarkContracts(normalized, paths)
  return normalized
}

function writeBenchmarkContracts(summary, paths) {
  try {
    const contractsRoot = path.join(paths.run_root, "contracts")
    fs.mkdirSync(contractsRoot, { recursive: true })
    const cliMetadataPath = path.join(contractsRoot, "cli-metadata.json")
    const harnessReportPath = path.join(contractsRoot, "harness-report.json")
    const taskReportPath = path.join(contractsRoot, "task-report.json")
    const gitDiffPath = path.join(contractsRoot, "git-diff.patch")
    const gitDiff = readGit(["diff", "--binary"]) || ""
    fs.writeFileSync(gitDiffPath, gitDiff, "utf8")

    const cliMetadata = buildCliMetadata(summary)
    const harnessReport = buildHarnessReport(summary, paths)
    const taskReport = buildTaskReport(summary, paths, {
      cliMetadataPath,
      gitDiff,
      gitDiffPath,
      harnessScore: harnessReport.finalScore,
    })

    writeJson(cliMetadataPath, cliMetadata)
    writeJson(harnessReportPath, harnessReport)
    writeJson(taskReportPath, taskReport)
    summary.benchmark_contracts = {
      cli_metadata_path: cliMetadataPath,
      task_report_path: taskReportPath,
      harness_report_path: harnessReportPath,
    }
  } catch (error) {
    summary.benchmark_contract_error = String(error?.stack || error?.message || error)
  }
}

function buildCliMetadata(summary) {
  const agentId = firstText(summary.agent_id, summary.agent, summary.provider, process.env.COMMAND_RUN_AGENT_AGENTS, "benchmark")
  const cliCommand = process.argv.map((arg) => quoteArg(arg)).join(" ")
  return {
    schema: "tura.benchmark.cli-metadata.v1",
    software: {
      platform: process.platform,
      arch: process.arch,
      nodeVersion: process.version,
      systemSoftwareVersion: `${process.platform}/${process.arch} ${os.release()} node ${process.version}`,
      packageName: readRootPackage()?.name ?? null,
      packageVersion: readRootPackage()?.version ?? null,
      gitHead: readGit(["rev-parse", "HEAD"]),
    },
    agent: {
      agentId,
      agentName: firstText(summary.agent_name, agentId),
      agentVersion: firstText(summary.agent_version, summary.model, process.env.COMMAND_RUN_AGENT_CODEX_MODEL, "unknown"),
      agentApplicationVersion: firstText(summary.agent_application_version, summary.agent_version, summary.model, "unknown"),
      cliLaunchCommandName: path.basename(process.argv[1] || process.argv[0] || "node"),
      cliCommand,
      pluginSkillGithubUrls: parseUrlList(summary.plugin_skill_github_urls || process.env.COMMAND_RUN_AGENT_PLUGIN_SKILL_GITHUB_URLS),
      releaseDownloadUrl: firstTextOrNull(summary.release_download_url, process.env.COMMAND_RUN_AGENT_RELEASE_URL),
      releaseSha256: firstTextOrNull(summary.release_sha256, process.env.COMMAND_RUN_AGENT_RELEASE_SHA256),
    },
    createdAt: new Date().toISOString(),
  }
}

function buildHarnessReport(summary, paths) {
  const finalScore = numericScore(summary)
  return {
    schema: "tura.benchmark.harness-report.v1",
    runId: paths.run_id,
    taskId: paths.test_name,
    harnessDirectory: firstText(summary.harness_directory, summary.harness_dir, paths.run_root),
    scores: finalScore === null ? [] : [{ harnessId: paths.test_name, score: finalScore, passed: Boolean(summary.ok) }],
    finalScore,
    createdAt: new Date().toISOString(),
  }
}

function buildTaskReport(summary, paths, artifacts) {
  const roundsDirectory = path.join(paths.run_root, "contracts", "rounds")
  const rounds = collectRounds(summary, roundsDirectory)
  const usage = tokenUsage(summary, rounds)
  const now = new Date().toISOString()
  return {
    schema: "tura.benchmark.task-report.v1",
    runId: paths.run_id,
    taskId: paths.test_name,
    agentId: firstText(summary.agent_id, summary.agent, summary.provider, process.env.COMMAND_RUN_AGENT_AGENTS, "benchmark"),
    metadata: {
      startedAt: firstText(summary.started_at, summary.start_time, summary.startedAt, now),
      endedAt: firstText(summary.ended_at, summary.end_time, summary.endedAt, now),
      agentVersion: firstText(summary.agent_version, summary.model, process.env.COMMAND_RUN_AGENT_CODEX_MODEL, "unknown"),
      agentCliCommand: process.argv.map((arg) => quoteArg(arg)).join(" "),
    },
    usage: {
      ...usage,
      providerDurationMs: firstNumber(summary.provider_duration_ms_sum, summary.provider_duration_ms, summary.standard_metrics?.duration_ms, summary.duration_ms, 0),
      llmRoundCount: firstNumber(summary.llm_round_count, summary.provider_call_count, summary.turns, summary.standard_metrics?.turns, 0),
    },
    harnessScore: artifacts.harnessScore,
    gitDiff: artifacts.gitDiff,
    gitDiffPath: artifacts.gitDiffPath,
    harnessDirectory: firstText(summary.harness_directory, summary.harness_dir, paths.run_root),
    startRepoSnapshot: {
      repoRoot: repoRoot(),
      gitHead: readGit(["rev-parse", "HEAD"]),
      gitStatusShort: readGit(["status", "--short"]),
      capturedAt: now,
      snapshotPath: path.join(paths.run_root, "contracts", "start-repo-snapshot.json"),
    },
    cliMetadataPath: artifacts.cliMetadataPath,
    roundsDirectory,
    rounds,
    sourceSummaryPath: paths.summary_path,
  }
}

function tokenUsage(summary, rounds = []) {
  const usage = summary.aggregate_usage || summary.token_totals || summary.standard_metrics?.token_usage || summary.usage || {}
  const roundUsage = sumRoundUsage(rounds)
  return {
    inputTokens: firstNumber(usage.inputTokens, usage.input_tokens, usage.input, usage.prompt_tokens, roundUsage.inputTokens, 0),
    cacheInputTokens: firstNumber(usage.cacheInputTokens, usage.cached_input_tokens, usage.cached, usage.cache_read_input_tokens, roundUsage.cacheInputTokens, 0),
    outputTokens: firstNumber(usage.outputTokens, usage.output_tokens, usage.output, usage.completion_tokens, roundUsage.outputTokens, 0),
    reasoningTokens: firstNumber(usage.reasoningTokens, usage.reasoning_tokens, usage.reasoning, usage.reasoning_output_tokens, roundUsage.reasoningTokens, 0),
    totalTokens: firstNumber(usage.totalTokens, usage.total_tokens, usage.total, roundUsage.totalTokens, 0),
  }
}

function collectRounds(summary, roundsDirectory) {
  const callbacks = []
  pushRoundCallbacks(callbacks, summary.rounds)
  pushRoundCallbacks(callbacks, summary.agent_rounds)
  pushRoundCallbacks(callbacks, summary.callbacks)
  for (const result of Array.isArray(summary.results) ? summary.results : []) {
    pushJsonlCallbacks(callbacks, result?.stdout)
    pushJsonlCallbacks(callbacks, readOptionalText(result?.stdout_path || result?.stdoutPath))
    pushRoundCallbacks(callbacks, result?.rounds)
    pushRoundCallbacks(callbacks, result?.callbacks)
  }

  const rounds = callbacks.map((callback, index) => normalizeRound(callback, index))
  if (rounds.length === 0) return []
  fs.mkdirSync(roundsDirectory, { recursive: true })
  for (const round of rounds) {
    const file = path.join(roundsDirectory, `${String(round.roundIndex + 1).padStart(4, "0")}-${safeFileName(round.roundId)}.json`)
    round.rawCallbackPath = file
    writeJson(file, round)
  }
  return rounds
}

function normalizeRound(callback, index) {
  const record = objectOrEmpty(callback)
  const startedAt = firstTextOrNull(record.startedAt, record.startTimestamp, record.started_at) || new Date().toISOString()
  const endedAt = firstTextOrNull(record.endedAt, record.endTimestamp, record.ended_at) || startedAt
  const usage = usageFromRecord(record)
  return {
    schema: "tura.benchmark.agent-round.v1",
    roundId: firstText(record.roundId, record.id, record.turnId, record.turn_id, record.session_id, record.sessionId, `round-${index + 1}`),
    roundIndex: index,
    startedAt,
    endedAt,
    input: { fullContext: fullContext(record) },
    output: { fullOutput: fullOutput(record), assistantMessage: assistantMessage(record) },
    usage,
    providerDurationMs: firstNumber(record.providerDurationMs, record.provider_duration_ms, record.duration_ms, record.metrics?.durationMs, record.runtime_usage?.latency_ms, 0),
    toolCalls: toolCallsFromRecord(record),
  }
}

function pushRoundCallbacks(callbacks, value) {
  if (!Array.isArray(value)) return
  for (const item of value) callbacks.push(item)
}

function pushJsonlCallbacks(callbacks, text) {
  if (typeof text !== "string" || !text.trim()) return
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue
    try {
      callbacks.push(JSON.parse(line))
    } catch {
      // Ignore non-JSON progress lines from CLIs that mix human output and JSONL.
    }
  }
}

function readOptionalText(file) {
  if (typeof file !== "string" || !file || !fs.existsSync(file)) return ""
  return fs.readFileSync(file, "utf8")
}

function usageFromRecord(record) {
  const usage = { inputTokens: 0, cacheInputTokens: 0, outputTokens: 0, reasoningTokens: 0, totalTokens: 0 }
  for (const item of [record.usage, record.metrics, record.runtime_usage, record.message?.usage, record.result?.usage, record.assistantMessageEvent?.usage, record.response?.usage, record.body?.usage]) {
    if (!item || typeof item !== "object") continue
    usage.inputTokens += firstNumber(item.inputTokens, item.input_tokens, item.prompt_tokens, 0)
    usage.cacheInputTokens += firstNumber(item.cacheInputTokens, item.cached_input_tokens, item.cache_read_input_tokens, item.input_tokens_details?.cached_tokens, 0)
    usage.outputTokens += firstNumber(item.outputTokens, item.output_tokens, item.completion_tokens, 0)
    usage.reasoningTokens += firstNumber(item.reasoningTokens, item.reasoning_tokens, item.reasoning_output_tokens, item.output_tokens_details?.reasoning_tokens, 0)
    usage.totalTokens += firstNumber(item.totalTokens, item.total_tokens, 0)
  }
  if (usage.totalTokens === 0) usage.totalTokens = usage.inputTokens + usage.cacheInputTokens + usage.outputTokens + usage.reasoningTokens
  return usage
}

function toolCallsFromRecord(record) {
  const candidates = []
  pushArray(candidates, record.toolCalls)
  pushArray(candidates, record.tool_calls)
  pushObject(candidates, record.tool)
  pushObject(candidates, record.tool_call)
  pushObject(candidates, record.tool_result)
  pushArray(candidates, record.message?.tool_calls)
  pushArray(candidates, record.assistantMessage?.tool_calls)
  pushArray(candidates, record.assistant_message?.tool_calls)
  pushContentTools(candidates, record.message?.content)
  pushContentTools(candidates, record.assistantMessage?.content)
  pushContentTools(candidates, record.assistant_message?.content)
  const body = record.body && typeof record.body === "object" ? record.body : (record.response && typeof record.response === "object" ? record.response : record)
  pushOpenAiOutput(candidates, body.output)
  pushArray(candidates, body.choices?.[0]?.message?.tool_calls)
  pushContentTools(candidates, body.choices?.[0]?.message?.content)
  if (isToolCall(record)) candidates.push(record)
  return candidates.flatMap((call, index) => normalizeToolCall(call, index))
}

function normalizeToolCall(call, index) {
  const name = firstTextOrNull(call.name, call.tool_name, call.function?.name) || "tool"
  const id = firstTextOrNull(call.id, call.call_id, call.tool_call_id) || `${name}-${index + 1}`
  const args = toolArguments(call)
  const parallelGroupId = firstTextOrNull(call.parallelGroupId, call.parallel_group_id, call.step)
  if (name === "command_run" && Array.isArray(args?.commands) && args.commands.length > 0) {
    return args.commands.map((command, commandIndex) => {
      const commandRecord = objectOrEmpty(command)
      return {
        id: `${id}:${commandIndex + 1}`,
        kind: "command",
        name: firstTextOrNull(commandRecord.command_type, commandRecord.commandType, commandRecord.name) || inferCommandName(commandRecord.command || commandRecord.command_line || commandRecord.commandLine || "command"),
        commandLine: commandLineFromValue(commandRecord),
        arguments: jsonValue(command) ?? {},
        parentToolName: name,
        parentToolCallId: id,
        parallelGroupId: firstTextOrNull(commandRecord.parallelGroupId, commandRecord.parallel_group_id, commandRecord.step) || parallelGroupId,
        raw: jsonValue(command),
      }
    })
  }
  return [{ id, kind: "tool", name, commandLine: commandLineFromValue(args), arguments: jsonValue(args) ?? {}, parallelGroupId, raw: jsonValue(call) }]
}

function toolArguments(call) {
  const raw = call.arguments ?? call.function?.arguments ?? call.args ?? call.input ?? {}
  if (typeof raw !== "string") return raw
  try {
    return JSON.parse(raw)
  } catch {
    return raw
  }
}

function fullContext(record) {
  return firstTextOrNull(record.fullContext, record.full_context, record.inputContext, record.input_context, record.context) || stringifyFirst(record.messages, record.input?.messages, record.request?.input, record.body?.input)
}

function fullOutput(record) {
  return firstTextOrNull(record.fullOutput, record.full_output, record.output, record.content, record.text) || stringifyFirst(record.output?.message, record.response?.output, record.body?.output, record.message)
}

function assistantMessage(record) {
  for (const value of [record.assistantMessage, record.assistantmessage, record.assistant_message, record.message?.content, record.output?.message?.content, record.response?.output_text, record.body?.output_text]) {
    if (typeof value === "string") return value
    const text = textFromContent(value)
    if (text) return text
  }
  return textFromOutput(record.response?.output ?? record.body?.output ?? record.output)
}

function textFromOutput(value) {
  const pieces = []
  for (const item of Array.isArray(value) ? value : []) {
    if (typeof item?.text === "string") pieces.push(item.text)
    for (const content of Array.isArray(item?.content) ? item.content : []) {
      if (typeof content?.text === "string") pieces.push(content.text)
    }
  }
  return pieces.join("\n")
}

function textFromContent(value) {
  const pieces = []
  for (const item of Array.isArray(value) ? value : []) {
    if (item?.type === "text" && typeof item.text === "string") pieces.push(item.text)
    if (typeof item?.content === "string") pieces.push(item.content)
  }
  return pieces.join("\n")
}

function sumRoundUsage(rounds) {
  return rounds.reduce((total, round) => ({
    inputTokens: total.inputTokens + round.usage.inputTokens,
    cacheInputTokens: total.cacheInputTokens + round.usage.cacheInputTokens,
    outputTokens: total.outputTokens + round.usage.outputTokens,
    reasoningTokens: total.reasoningTokens + round.usage.reasoningTokens,
    totalTokens: total.totalTokens + round.usage.totalTokens,
  }), { inputTokens: 0, cacheInputTokens: 0, outputTokens: 0, reasoningTokens: 0, totalTokens: 0 })
}

function numericScore(summary) {
  return firstFinite(
    summary.harness_score,
    summary.score,
    summary.final_score,
    summary.validation?.score,
    summary.eval?.score,
    summary.standard_metrics?.scores?.score,
    summary.standard_metrics?.scores?.harness_score,
  )
}

function firstNumber(...values) {
  return firstFinite(...values) ?? 0
}

function firstFinite(...values) {
  for (const value of values) {
    const number = Number(value)
    if (Number.isFinite(number)) return number
  }
  return null
}

function firstText(...values) {
  return firstTextOrNull(...values) ?? ""
}

function firstTextOrNull(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.trim()) return value
    if (typeof value === "number" && Number.isFinite(value)) return String(value)
  }
  return null
}

function parseUrlList(value) {
  if (Array.isArray(value)) return value.filter((item) => typeof item === "string" && item.trim())
  return String(value || "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)
}

function pushArray(target, value) {
  if (!Array.isArray(value)) return
  for (const item of value) if (item && typeof item === "object") target.push(item)
}

function pushObject(target, value) {
  if (value && typeof value === "object" && !Array.isArray(value)) target.push(value)
}

function pushContentTools(target, value) {
  if (!Array.isArray(value)) return
  for (const item of value) if (item && typeof item === "object" && isToolCall(item)) target.push(item)
}

function pushOpenAiOutput(target, value) {
  if (!Array.isArray(value)) return
  for (const item of value) if (item && typeof item === "object" && isToolCall(item)) target.push(item)
}

function isToolCall(value) {
  return value?.type === "function_call" || value?.type === "tool_use" || Boolean(value?.function) || Boolean((value?.arguments || value?.input) && (value?.name || value?.tool_name))
}

function commandLineFromValue(value) {
  if (typeof value === "string") return value
  if (value && typeof value === "object") {
    const direct = firstTextOrNull(value.commandLine, value.command_line, value.command, value.cmd)
    if (direct) return direct
  }
  return JSON.stringify(jsonValue(value) ?? {})
}

function inferCommandName(commandLine) {
  return String(commandLine || "command").trim().split(/\s+/)[0] || "command"
}

function stringifyFirst(...values) {
  for (const value of values) {
    if (typeof value === "string") return value
    if (value !== undefined && value !== null) return JSON.stringify(jsonValue(value) ?? value)
  }
  return ""
}

function objectOrEmpty(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {}
}

function jsonValue(value) {
  if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") return value
  if (Array.isArray(value)) return value.map((item) => jsonValue(item) ?? null)
  if (!value || typeof value !== "object") return undefined
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, jsonValue(item) ?? null]))
}

function safeFileName(value) {
  return String(value || "round").replace(/[^A-Za-z0-9._-]+/g, "_").slice(0, 120) || "round"
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true })
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8")
}

function readRootPackage() {
  try {
    return JSON.parse(fs.readFileSync(path.join(repoRoot(), "package.json"), "utf8"))
  } catch {
    return null
  }
}

function readGit(args) {
  const result = spawnSync("git", args, { cwd: repoRoot(), encoding: "utf8", windowsHide: true })
  return result.status === 0 ? result.stdout.trim() : null
}

function repoRoot() {
  return path.resolve(import.meta.dirname, "..", "..")
}

function quoteArg(arg) {
  return /\s/.test(arg) ? JSON.stringify(arg) : arg
}
