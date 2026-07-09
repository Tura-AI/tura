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
  normalized.ok = normalizeSummaryOk(normalized)
  writeBenchmarkContracts(normalized, paths)
  return normalized
}

function normalizeSummaryOk(summary) {
  const results = Array.isArray(summary.results) ? summary.results : []
  const evalResults = results.filter((result) => result?.eval?.ran)
  if (evalResults.length === 0) return summary.ok
  return Boolean(summary.ok) && evalResults.every(resultEvaluationPassed)
}

function resultEvaluationPassed(result) {
  if (result?.error) return false
  const reports = Array.isArray(result?.eval?.report?.reports) ? result.eval.report.reports : []
  const failed = reports.reduce((total, report) => total + Number(report?.failed || 0), 0)
  return Number(result?.eval?.exit_code) === 0 && failed === 0
}

function writeBenchmarkContracts(summary, paths) {
  try {
    const contractsRoot = path.join(paths.run_root, "contracts")
    fs.mkdirSync(contractsRoot, { recursive: true })
    const cliMetadataPath = path.join(contractsRoot, "cli-metadata.json")
    const harnessReportPath = path.join(contractsRoot, "harness-report.json")
    const taskReportPath = path.join(contractsRoot, "task-report.json")
    const roundsJsonlPath = path.join(contractsRoot, "agent-rounds.jsonl")
    const webRunPath = path.join(contractsRoot, "benchmark-web-run.json")
    const gitDiffPath = path.join(contractsRoot, "git-diff.patch")
    const gitDiff = modelPatchDiff(summary) || readGit(["diff", "--binary"]) || ""
    fs.writeFileSync(gitDiffPath, gitDiff, "utf8")

    const cliMetadata = buildCliMetadata(summary)
    const harnessReport = buildHarnessReport(summary, paths)
    const taskReport = buildTaskReport(summary, paths, {
      cliMetadataPath,
      gitDiff,
      gitDiffPath,
      harnessScore: harnessReport.finalScore,
    })
    const webRun = buildBenchmarkWebRun(summary, paths, taskReport, harnessReport, gitDiff)

    writeJson(cliMetadataPath, cliMetadata)
    writeJson(harnessReportPath, harnessReport)
    writeJson(taskReportPath, taskReport)
    fs.writeFileSync(roundsJsonlPath, taskReport.rounds.map((round) => JSON.stringify(round)).join("\n") + (taskReport.rounds.length ? "\n" : ""), "utf8")
    writeJson(webRunPath, webRun)
    summary.benchmark_contracts = {
      cli_metadata_path: cliMetadataPath,
      task_report_path: taskReportPath,
      harness_report_path: harnessReportPath,
      rounds_jsonl_path: roundsJsonlPath,
      web_run_path: webRunPath,
      git_diff_path: gitDiffPath,
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
  const scores = harnessScores(summary, paths)
  const finalScore = numericScore(summary) ?? aggregateScores(scores)
  return {
    schema: "tura.benchmark.harness-report.v1",
    runId: paths.run_id,
    taskId: paths.test_name,
    harnessDirectory: firstText(summary.harness_directory, summary.harness_dir, paths.run_root),
    scores: scores.length > 0 ? scores : (finalScore === null ? [] : [{ harnessId: paths.test_name, score: finalScore, passed: Boolean(summary.ok) }]),
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
      llmRoundCount: firstNumber(summary.llm_round_count, summary.provider_call_count, summary.turns, summary.standard_metrics?.turns, rounds.length, 0),
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

function buildBenchmarkWebRun(summary, paths, taskReport, harnessReport, gitDiff) {
  const selectedTask = summary.selected_task || {}
  const metadata = selectedTask.metadata || {}
  const usage = taskReport.usage || {}
  const rounds = Array.isArray(taskReport.rounds) ? taskReport.rounds : []
  const startedAt = firstText(taskReport.metadata?.startedAt, summary.started_at, summary.startedAt, new Date().toISOString())
  const endedAt = firstText(taskReport.metadata?.endedAt, summary.ended_at, summary.endedAt, startedAt)
  const durationSeconds = Math.round(firstNumber(summary.elapsed_ms, summary.duration_ms, usage.providerDurationMs, 0) / 1000)
  const commandCount = commandCountFromRounds(rounds)
  const peakContext = firstNumber(summary.peak_context, summary.peakContext, summary.standard_metrics?.peak_context, peakContextFromRounds(rounds), 0)
  const taskName = firstText(selectedTask.id, selectedTask.label, metadata.task_id, paths.test_name)
  const model = firstText(summary.model, summary.tura_model, taskReport.metadata?.agentVersion, "unknown")
  return {
    schema: "tura.benchmark.web-run.v1",
    taskName,
    sessionName: paths.run_id,
    title: firstText(metadata.display_title, selectedTask.label, taskName),
    subtitle: firstText(metadata.display_description, summary.suite, "Benchmark run generated from Tura benchmark contracts."),
    source: {
      url: firstTextOrNull(summary.upstream_repository, metadata.repository_url),
      model,
      steps: rounds.length,
      costUsd: firstFinite(summary.cost_usd, summary.costUsd) ?? 0,
      inputTokens: firstNumber(usage.inputTokens, 0),
      outputTokens: firstNumber(usage.outputTokens, 0),
      reasoningTokens: firstNumber(usage.reasoningTokens, 0),
      cacheInputTokens: firstNumber(usage.cacheInputTokens, 0),
      totalTokens: firstNumber(usage.totalTokens, 0),
      peakContext,
      durationSeconds,
      commands: commandCount,
      rawArtifactStatus: "Generated from tura benchmark contract artifacts.",
    },
    run: {
      status: summary.ok ? "pass" : "fail",
      agent: firstText(summary.agent_id, summary.agent, Array.isArray(summary.agents) ? summary.agents.join(",") : "", taskReport.agentId),
      provider: firstText(summary.provider, "benchmark"),
      runtimeModel: model,
      startedAt,
      completedAt: endedAt,
      repository: firstText(metadata.repository_url, summary.repository, ""),
      branch: firstText(summary.branch, paths.run_id),
    },
    rounds: rounds.map(webRoundFromContract),
    harness: webHarnessFromContract(harnessReport),
    repoDiff: parseGitDiffForWeb(gitDiff),
    metadata: {
      benchmark_version: firstText(summary.suite, paths.test_name),
      task_id: firstText(metadata.task_id, selectedTask.id, paths.test_name),
      run_id: paths.run_id,
      run_page_schema: "tura.benchmark.web-run.v1",
      contract_schemas: {
        cliMetadata: "tura.benchmark.cli-metadata.v1",
        round: "tura.benchmark.agent-round.v1",
        taskReport: "tura.benchmark.task-report.v1",
        harnessReport: "tura.benchmark.harness-report.v1",
      },
      contract_paths: {
        cliMetadataPath: taskReport.cliMetadataPath,
        taskReportPath: paths.summary_path ? path.join(paths.run_root, "contracts", "task-report.json") : null,
        harnessReportPath: path.join(paths.run_root, "contracts", "harness-report.json"),
        roundsDirectory: taskReport.roundsDirectory,
        gitDiffPath: taskReport.gitDiffPath,
      },
      visible_source_metrics: {
        steps: rounds.length,
        input_tokens: firstNumber(usage.inputTokens, 0),
        output_tokens: firstNumber(usage.outputTokens, 0),
        reasoning_tokens: firstNumber(usage.reasoningTokens, 0),
        cached_input_tokens: firstNumber(usage.cacheInputTokens, 0),
        total_tokens: firstNumber(usage.totalTokens, 0),
        peak_context_tokens: peakContext,
        duration_seconds: durationSeconds,
        command_count: commandCount,
      },
      artifacts: {
        source_summary_path: paths.summary_path,
        harness_directory: taskReport.harnessDirectory,
        model_patch: taskReport.gitDiffPath,
        model_patch_paths: modelPatchPaths(summary),
      },
      raw_summary: compactSummaryForWeb(summary, paths, taskReport),
    },
  }
}

function compactSummaryForWeb(summary, paths, taskReport) {
  return {
    ok: Boolean(summary.ok),
    in_progress: Boolean(summary.in_progress),
    suite: firstTextOrNull(summary.suite),
    run_id: paths.run_id,
    selected_task: jsonValue(summary.selected_task) ?? {},
    agents: jsonValue(summary.agents) ?? [],
    aggregate_usage: jsonValue(summary.aggregate_usage) ?? {},
    results: compactResultsForWeb(summary.results),
    summary_path: paths.summary_path,
    contracts_root: path.join(paths.run_root, "contracts"),
    task_report_path: path.join(paths.run_root, "contracts", "task-report.json"),
    rounds_directory: taskReport.roundsDirectory,
  }
}

function compactResultsForWeb(results) {
  if (!Array.isArray(results)) return []
  return results.map((result) => ({
    agent_id: firstTextOrNull(result.agent_id, result.agentId, result.id),
    ok: Boolean(result.ok),
    exit_code: firstNumber(result.exit_code, result.exitCode, 0),
    usage: jsonValue(result.usage) ?? {},
    events: jsonValue(result.events) ?? {},
    harness: jsonValue(result.harness) ?? {},
    output_path: firstTextOrNull(result.output_path, result.outputPath),
    stdout_path: firstTextOrNull(result.stdout_path, result.stdoutPath),
    stderr_path: firstTextOrNull(result.stderr_path, result.stderrPath),
    provider_calls_path: firstTextOrNull(result.provider_calls_path, result.providerCallsPath),
    fixture_backend: firstTextOrNull(result.fixture_backend, result.fixtureBackend),
    fixture_source_path: firstTextOrNull(result.fixture_source_path, result.fixtureSourcePath),
  }))
}

function webRoundFromContract(round) {
  const usage = round.usage || {}
  return {
    id: String(round.roundId || `round-${Number(round.roundIndex || 0) + 1}`),
    index: Number(round.roundIndex || 0) + 1,
    title: `${firstText(round.metadata?.agentId, "agent")} round ${Number(round.roundIndex || 0) + 1}`,
    intent: firstText(round.metadata?.roundSource, round.metadata?.eventType, "agent round"),
    durationSeconds: Math.round(firstNumber(round.providerDurationMs, 0) / 1000),
    inputTokens: firstNumber(usage.inputTokens, 0),
    cacheInputTokens: firstNumber(usage.cacheInputTokens, 0),
    outputTokens: firstNumber(usage.outputTokens, 0),
    reasoningTokens: firstNumber(usage.reasoningTokens, 0),
    totalTokens: firstNumber(usage.totalTokens, 0),
    messages: webMessagesFromRound(round),
    commands: webCommandsFromRound(round),
    metadata: jsonValue(round.metadata) ?? {},
    sources: jsonValue(round.sources) ?? {},
  }
}

function webMessagesFromRound(round) {
  const messages = Array.isArray(round.messages) ? round.messages : []
  if (messages.length > 0) {
    return messages.map((message) => ({
      role: firstText(message.role, "unknown"),
      text: firstText(message.text, message.content, ""),
    })).filter((message) => message.text)
  }
  const output = firstTextOrNull(round.output?.assistantMessage, round.output?.fullOutput)
  return output ? [{ role: "assistant", text: output }] : []
}

function webCommandsFromRound(round) {
  return (Array.isArray(round.toolCalls) ? round.toolCalls : []).map((tool, index) => {
    const args = objectOrEmpty(tool.arguments)
    return {
      id: firstText(tool.id, `cmd-${index + 1}`),
      type: firstText(tool.name, tool.kind, "tool"),
      step: firstNumber(tool.parallelGroupId, index + 1),
      status: firstText(args.status, args.exit_code === 0 ? "done" : "", "unknown"),
      durationSeconds: Math.round(firstNumber(tool.durationMs, tool.duration_ms, args.duration_ms, 0) / 1000),
      commandLine: firstText(tool.commandLine, ""),
      receipt: firstText(args.receipt, args.aggregated_output, args.stdout, ""),
      stdout: firstText(args.stdout, args.aggregated_output, ""),
      stderr: firstText(args.stderr, ""),
    }
  })
}

function webHarnessFromContract(harnessReport) {
  return (Array.isArray(harnessReport?.scores) ? harnessReport.scores : []).map((score, index) => ({
    id: firstText(score.harnessId, `h-${index + 1}`),
    status: score.passed ? "pass" : "fail",
    assertion: firstText(score.harnessId, "harness assertion"),
    evidence: JSON.stringify(jsonValue(score.details) ?? {}),
    artifacts: Array.isArray(score.artifacts) ? score.artifacts : [],
  }))
}

function parseGitDiffForWeb(diffText) {
  const files = []
  let current = null
  let hunk = null
  for (const line of String(diffText || "").split(/\r?\n/)) {
    if (line.startsWith("diff --git ")) {
      current = { path: line.split(" b/")[1] || line.split(" a/")[1] || "unknown", status: "modified", hunks: [] }
      hunk = null
      files.push(current)
      continue
    }
    if (!current) continue
    if (line.startsWith("new file mode")) current.status = "added"
    if (line.startsWith("deleted file mode")) current.status = "deleted"
    if (line.startsWith("+++ b/")) current.path = line.slice(6)
    if (line.startsWith("@@")) {
      hunk = { header: line, lines: [] }
      current.hunks.push(hunk)
      continue
    }
    if (!hunk || line.startsWith("\\ No newline")) continue
    if (line.startsWith("+") && !line.startsWith("+++")) hunk.lines.push({ type: "add", text: line.slice(1) })
    else if (line.startsWith("-") && !line.startsWith("---")) hunk.lines.push({ type: "remove", text: line.slice(1) })
    else if (line.startsWith(" ")) hunk.lines.push({ type: "context", text: line.slice(1) })
  }
  return files
}

function commandCountFromRounds(rounds) {
  return (Array.isArray(rounds) ? rounds : []).reduce((total, round) => {
    return total + (Array.isArray(round?.toolCalls) ? round.toolCalls.length : 0)
  }, 0)
}

function peakContextFromRounds(rounds) {
  return (Array.isArray(rounds) ? rounds : []).reduce((max, round) => {
    const usage = round?.usage || {}
    const contextTokens = Number(usage.inputTokens || 0)
    return Number.isFinite(contextTokens) ? Math.max(max, contextTokens) : max
  }, 0)
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

  const rounds = callbacks.map((callback, index) => normalizeRound(enrichRoundRecord(callback, summary), index))
  for (const result of Array.isArray(summary.results) ? summary.results : []) {
    const resultCallbacks = []
    pushRoundCallbacks(resultCallbacks, result?.rounds)
    pushRoundCallbacks(resultCallbacks, result?.callbacks)

    const stdoutText = firstTextOrNull(result?.stdout) || readOptionalText(result?.stdout_path || result?.stdoutPath)
    const stdoutRecords = parseJsonlRecords(stdoutText)
    if (resultCallbacks.length > 0) {
      for (const callback of resultCallbacks) rounds.push(normalizeRound(enrichRoundRecord(callback, summary, result), rounds.length))
      continue
    }
    if (stdoutRecords.length > 0 && stdoutRecords.every(isExplicitRoundRecord)) {
      for (const callback of stdoutRecords) rounds.push(normalizeRound(enrichRoundRecord(callback, summary, result), rounds.length))
      continue
    }
    for (const aggregate of aggregateResultRounds(result, summary, stdoutText, stdoutRecords)) {
      rounds.push(normalizeRound(aggregate, rounds.length))
    }
  }
  if (rounds.length === 0) return []
  fs.mkdirSync(roundsDirectory, { recursive: true })
  const writtenFiles = new Set()
  for (const round of rounds) {
    const file = path.join(roundsDirectory, `${String(round.roundIndex + 1).padStart(4, "0")}-${safeFileName(round.roundId)}.json`)
    round.rawCallbackPath = file
    writeJson(file, round)
    writtenFiles.add(path.resolve(file))
  }
  for (const entry of fs.readdirSync(roundsDirectory, { withFileTypes: true })) {
    if (!entry.isFile() || !entry.name.endsWith(".json")) continue
    const file = path.resolve(path.join(roundsDirectory, entry.name))
    if (!writtenFiles.has(file)) fs.rmSync(file, { force: true })
  }
  return rounds
}

function normalizeRound(callback, index) {
  const record = objectOrEmpty(callback)
  const startedAt = firstTextOrNull(record.startedAt, record.startTimestamp, record.started_at) || new Date().toISOString()
  const endedAt = firstTextOrNull(record.endedAt, record.endTimestamp, record.ended_at) || startedAt
  const usage = usageFromRecord(record)
  const roundId = firstText(record.roundId, record.id, record.turnId, record.turn_id, record.session_id, record.sessionId, `round-${index + 1}`)
  return {
    schema: "tura.benchmark.agent-round.v1",
    roundId,
    roundIndex: index,
    startedAt,
    endedAt,
    input: { fullContext: fullContext(record), messages: inputMessages(record) },
    output: { fullOutput: fullOutput(record), assistantMessage: assistantMessage(record), messages: outputMessages(record) },
    messages: messagesFromRecord(record),
    usage,
    providerDurationMs: firstNumber(record.providerDurationMs, record.provider_duration_ms, record.duration_ms, record.metrics?.durationMs, record.runtime_usage?.latency_ms, 0),
    toolCalls: toolCallsFromRecord(record),
    sources: roundSources(record),
    metadata: roundMetadata(record, roundId),
  }
}

function enrichRoundRecord(callback, summary, result = {}) {
  const record = objectOrEmpty(callback)
  const agentId = firstTextOrNull(record.agentId, record.agent_id, record.agent, record.provider, result.agent, summary.agent_id, summary.agent, summary.provider)
  const taskId = firstTextOrNull(record.taskId, record.task_id, record.task, result.task_id, result.task, summary.selected_task?.id, summary.task_id)
  return {
    ...record,
    agent_id: agentId || record.agent_id,
    task_id: taskId || record.task_id,
    task: taskId || record.task,
    agent_kind: firstTextOrNull(record.agentKind, record.agent_kind) || inferAgentKind(agentId),
    agent_mode: firstTextOrNull(record.agentMode, record.agent_mode, record.mode, record.tura_agent) || inferAgentMode(agentId),
    model: firstTextOrNull(record.model, record.model_id, record.provider_model) || modelForAgent(agentId, summary, result),
    reasoning: firstTextOrNull(record.reasoning, record.reasoning_effort, record.reasoningEffort) || firstTextOrNull(summary.reasoning, summary.reasoning_effort),
    service_tier: firstTextOrNull(record.serviceTier, record.service_tier, record.tier) || firstTextOrNull(summary.service_tier, summary.serviceTier),
    priority_enabled: firstBoolean(record.priorityEnabled, record.priority_enabled, record.priority, record.is_priority) ?? isPriority(summary),
    round_source: firstTextOrNull(record.roundSource, record.round_source, record.source) || "callback",
  }
}

function aggregateResultRounds(result, summary, stdoutText, stdoutRecords) {
  if (!result || typeof result !== "object") return []
  const agentId = firstTextOrNull(result.agent, result.agent_id, result.provider)
  if (!agentId && stdoutRecords.length === 0) return []
  const providerRecords = providerRoundRecords(result)
  if (stdoutRecords.length === 0 && providerRecords.length === 0 && !hasRoundUsageEvidence(result)) return []
  if (providerRecords.length > 0) {
    return providerRecords.map((record, index) => aggregateResultRound(result, summary, stdoutText, providerRecords.length === 1 ? stdoutRecords : recordsForProviderRecord(stdoutRecords, record), [record], {
      turnId: firstTextOrNull(record.call_id, record.id, record.response?.id) || `${firstText(agentId, "agent")}-provider-${index + 1}`,
      startedAt: firstTextOrNull(record.started_at, record.startedAt),
      endedAt: firstTextOrNull(record.finished_at, record.finishedAt, record.ended_at, record.endedAt),
      roundSource: firstTextOrNull(record.round_source, record.roundSource) || "provider-log",
      records: [],
    }, index, providerRecords.length))
  }
  const codexRolloutGroups = codexRolloutRoundGroups(result)
  if (codexRolloutGroups.length > 0) {
    return codexRolloutGroups.map((group, index) => aggregateResultRound(result, summary, stdoutText, [], [group.providerRecord], group, index, codexRolloutGroups.length))
  }
  const tokenUsageGroups = tokenUsageRoundGroups(stdoutRecords)
  if (tokenUsageGroups.length > 1) {
    return tokenUsageGroups.map((group, index) => aggregateResultRound(result, summary, stdoutText, group.records, [], group, index, tokenUsageGroups.length))
  }
  const lifecycleGroups = lifecycleTurnGroups(stdoutRecords)
  if (lifecycleGroups.length > 1) {
    return lifecycleGroups.map((group, index) => aggregateResultRound(result, summary, stdoutText, group.records, [], group, index, lifecycleGroups.length))
  }
  const visibleGroups = visibleAgentRoundGroups(stdoutRecords)
  if (visibleGroups.length > 1) {
    return visibleGroups.map((group, index) => aggregateResultRound(result, summary, stdoutText, group.records, [], group, index, visibleGroups.length))
  }
  if (tokenUsageGroups.length > 0) {
    return tokenUsageGroups.map((group, index) => aggregateResultRound(result, summary, stdoutText, group.records, [], group, index, tokenUsageGroups.length))
  }
  const groups = lifecycleGroups
  return groups.map((group, index) => aggregateResultRound(result, summary, stdoutText, group.records, providerRecordsForTurn(providerRecords, groups, index), group, index, groups.length))
}

function hasRoundUsageEvidence(result) {
  return hasUsage(result?.usage) ||
    Number(result?.usage?.usage_events || 0) > 0 ||
    Number(result?.events?.llm_rounds || 0) > 0 ||
    Number(result?.events?.usage_events || 0) > 0
}

function aggregateResultRound(result, summary, stdoutText, stdoutRecords, providerRecords, turnGroup, turnIndex, turnCount = 1) {
  const agentId = firstTextOrNull(result.agent, result.agent_id, result.provider)
  const usage = turnGroup?.usageUnavailable ? emptyContractUsage() : usageFromAggregateResult(result, providerRecords, stdoutRecords)
  const messages = mergeMessages(messagesFromEvents(stdoutRecords), providerRecords.flatMap((record) => messagesFromProviderRecord(record)))
  const allMessages = messages.filter((message) => message.role === "assistant").map((message) => message.text).filter(Boolean)
  const startedAt = firstTimestampOrNull(turnGroup?.startedAt, result.started_at, result.startedAt, providerRecords[0]?.started_at) || new Date().toISOString()
  const endedAt = firstTimestampOrNull(turnGroup?.endedAt, result.ended_at, result.endedAt, providerRecords.at(-1)?.finished_at) || startedAt
  const roundDuration = aggregateRoundDuration(result, providerRecords, startedAt, endedAt, turnIndex, turnCount)
  const compactSummaryCount = compactSummaryTextCount(stdoutRecords)
  const toolCalls = mergeToolCalls(
    providerToolCalls(providerRecords),
    commandExecutionToolCalls(stdoutRecords),
    piToolExecutionToolCalls(stdoutRecords),
    opencodeToolUseToolCalls(stdoutRecords),
  )
  return {
    type: "benchmark.agent.round.completed",
    roundId: firstTextOrNull(turnGroup?.turnId, result.round_id, result.roundId, result.turn_id, result.session_id) || `${firstText(result.task, "task")}-${firstText(agentId, "agent")}-round-${turnIndex + 1}`,
    started_at: startedAt,
    ended_at: endedAt,
    agent_id: agentId,
    agent_kind: inferAgentKind(agentId),
    agent_mode: inferAgentMode(agentId),
    model: modelForAgent(agentId, summary, result),
    reasoning: firstTextOrNull(result.reasoning, summary.reasoning, summary.reasoning_effort),
    service_tier: firstTextOrNull(result.service_tier, result.serviceTier, summary.service_tier, summary.serviceTier),
    priority_enabled: isPriority(summary),
    round_source: firstTextOrNull(turnGroup?.roundSource) || "agent-result",
    task: result.task,
    full_context: firstTextOrNull(readOptionalText(result.prep?.prompt_path), readOptionalText(result.context_archive?.input_prompt_path)),
    full_output: allMessages.join("\n\n"),
    assistant_message: allMessages.at(-1) || "",
    messages,
    usage,
    usage_event_source: usageEventSource(stdoutRecords, providerRecords),
    compact_summary_count: compactSummaryCount,
    compact_summary_token_included: compactSummaryCount > 0 && usage.totalTokens > 0,
    provider_duration_ms: roundDuration.durationMs,
    provider_duration_source: roundDuration.source,
    toolCalls,
    eval: result.eval,
    source_stdout_path: result.stdout_path || result.stdoutPath || null,
    source_provider_calls_path: result.context_archive?.provider_calls_full_path || null,
    source_codex_rollout_path: firstTextOrNull(turnGroup?.rolloutPath, providerRecords[0]?.rollout_path),
  }
}

function aggregateRoundDuration(result, providerRecords, startedAt, endedAt, turnIndex, turnCount) {
  const providerDurationRecord = providerRecords.find((record) => firstPositiveNumberOrNull(record.duration_ms, record.provider_duration_ms, record.metrics?.durationMs, record.runtime_usage?.latency_ms) !== null)
  const providerDuration = providerDurationRecord ? firstPositiveNumberOrNull(providerDurationRecord.duration_ms, providerDurationRecord.provider_duration_ms, providerDurationRecord.metrics?.durationMs, providerDurationRecord.runtime_usage?.latency_ms) : null
  if (providerDuration !== null) {
    return { durationMs: providerDuration, source: firstTextOrNull(providerDurationRecord.duration_source, providerDurationRecord.provider_duration_source, providerDurationRecord.durationSource) || "provider-log" }
  }

  const timestampDuration = durationMsBetweenOrNull(startedAt, endedAt)
  if (timestampDuration !== null && timestampDuration > 0) return { durationMs: timestampDuration, source: "event-timestamps" }

  const fallbackDuration = fallbackRoundDurationMs(result, turnIndex, turnCount)
  if (fallbackDuration !== null && fallbackDuration > 0) return { durationMs: fallbackDuration, source: "result-elapsed-fallback" }

  return { durationMs: firstNumber(providerDuration, timestampDuration, fallbackDuration, 0), source: "unavailable" }
}

function usageEventSource(records, providerRecords = []) {
  const values = Array.isArray(records) ? records : []
  const hasCompactUsage = compactUsageRecords(values).length > 0
  if (values.some((record) => record?.type === "thread.token_usage.updated" && hasUsage(record.usage))) return "token-usage-jsonl"
  if (values.some((record) => (record?.type === "turn.completed" || record?.type === "turn_end") && hasUsage(record.usage || record.message?.usage))) return hasCompactUsage ? "turn-end+compact-summary" : "turn-end"
  if (values.some((record) => record?.type === "step_finish" && hasUsage(record.part?.tokens))) return hasCompactUsage ? "step-finish+compact-summary" : "step-finish"
  if (hasCompactUsage) return "compact-summary"
  const providerUsageRecord = providerRecords.find((record) => hasUsage(record?.usage || record?.response?.usage))
  if (providerUsageRecord) return firstTextOrNull(providerUsageRecord.usage_event_source, providerUsageRecord.usageEventSource) || "provider-log"
  if (values.some((record) => hasUsage(usageCandidate(record)))) return "stdout-event"
  return "result-summary"
}

function compactSummaryTextCount(records) {
  const texts = new Set()
  for (const record of Array.isArray(records) ? records : []) {
    for (const text of compactSummaryTexts(record)) texts.add(text)
  }
  return texts.size
}

function compactSummaryTexts(value, seen = new Set()) {
  if (!value || typeof value !== "object") return []
  if (seen.has(value)) return []
  seen.add(value)
  const texts = []
  if (value.type === "summary_text" && typeof value.text === "string" && value.text.trim()) {
    texts.push(value.text.trim())
  }
  if (typeof value.thinkingSignature === "string") {
    const parsed = parseJsonObject(value.thinkingSignature)
    if (parsed) texts.push(...compactSummaryTexts(parsed, seen))
  }
  for (const item of Object.values(value)) {
    if (item && typeof item === "object") {
      texts.push(...compactSummaryTexts(item, seen))
    } else if (typeof item === "string" && item.includes("summary_text")) {
      const parsed = parseJsonObject(item)
      if (parsed) texts.push(...compactSummaryTexts(parsed, seen))
    }
  }
  return texts
}

function parseJsonObject(value) {
  try {
    const parsed = JSON.parse(value)
    return parsed && typeof parsed === "object" ? parsed : null
  } catch {
    return null
  }
}

function roundMetadata(record, roundId) {
  const agentId = firstTextOrNull(record.agentId, record.agent_id, record.agent, record.provider, record.source_agent) || inferAgentId(record)
  const serviceTier = firstTextOrNull(record.serviceTier, record.service_tier, record.tier, record.metadata?.serviceTier, record.metadata?.service_tier) || "unknown"
  return {
    agentId,
    taskId: firstTextOrNull(record.taskId, record.task_id, record.task, record.metadata?.taskId, record.metadata?.task_id),
    agentKind: firstTextOrNull(record.agentKind, record.agent_kind, record.kind) || inferAgentKind(agentId),
    agentMode: firstTextOrNull(record.agentMode, record.agent_mode, record.mode, record.tura_agent) || inferAgentMode(agentId),
    model: firstTextOrNull(record.model, record.model_id, record.provider_model, record.metadata?.model) || "unknown",
    reasoning: firstTextOrNull(record.reasoning, record.reasoning_effort, record.reasoningEffort, record.metadata?.reasoning) || "unknown",
    serviceTier,
    priorityEnabled: firstBoolean(record.priorityEnabled, record.priority_enabled, record.priority, record.is_priority) ?? serviceTier.toLowerCase() === "priority",
    roundSource: firstTextOrNull(record.roundSource, record.round_source, record.source) || "callback",
    eventType: firstTextOrNull(record.type, record.event, record.event_type, record.eventType) || "unknown",
    sessionOrTurnId: firstTextOrNull(record.sessionOrTurnId, record.session_or_turn_id, record.turnId, record.turn_id, record.session_id, record.sessionId, record.id) || roundId,
    fixtureBackend: firstTextOrNull(record.fixtureBackend, record.fixture_backend),
    fixtureSourcePath: firstTextOrNull(record.fixtureSourcePath, record.fixture_source_path),
    sourceAgentId: firstTextOrNull(record.sourceAgentId, record.source_agent_id),
    sourceEventType: firstTextOrNull(record.sourceEventType, record.source_event_type),
    sourceRoundIndex: firstNumberOrNull(record.sourceRoundIndex, record.source_round_index),
    durationSource: firstTextOrNull(record.providerDurationSource, record.provider_duration_source, record.durationSource, record.duration_source),
    usageEventSource: firstTextOrNull(record.usageEventSource, record.usage_event_source),
    compactSummaryCount: firstNumber(record.compactSummaryCount, record.compact_summary_count, 0),
    compactSummaryTokenIncluded: firstBoolean(record.compactSummaryTokenIncluded, record.compact_summary_token_included) ?? false,
  }
}

function roundSources(record) {
  return {
    stdoutPath: firstTextOrNull(record.source_stdout_path, record.stdout_path, record.stdoutPath),
    providerCallsPath: firstTextOrNull(record.source_provider_calls_path, record.provider_calls_path, record.providerCallsPath),
    codexRolloutPath: firstTextOrNull(record.source_codex_rollout_path, record.codex_rollout_path, record.rollout_path),
    providerLogPath: firstTextOrNull(record.provider_log_path, record.providerLogPath),
    summaryPath: firstTextOrNull(record.source_summary_path, record.summary_path, record.summaryPath),
  }
}

function lifecycleTurnGroups(records) {
  if (!Array.isArray(records) || records.length === 0) return [{ records: [] }]
  const groups = []
  let current = null
  let pending = []
  for (const record of records) {
    if (record?.type === "turn.started" || record?.type === "turn_start" || record?.type === "step_start") {
      if (current) groups.push(current)
      pending = []
      current = {
        turnId: firstTextOrNull(record.turn_id, record.turnId, record.id),
        startedAt: firstTimestampOrNull(record.started_at, record.startedAt, record.timestamp, record.time),
        records: [...pending, record],
      }
      continue
    }
    if (current) {
      current.records.push(record)
      if (record?.type === "turn.completed" || record?.type === "turn_end" || record?.type === "step_finish") {
        current.endedAt = firstTimestampOrNull(record.ended_at, record.endedAt, record.timestamp, record.time)
        groups.push(current)
        current = null
      }
    } else {
      pending.push(record)
    }
  }
  if (current) groups.push(current)
  else if (hasRoundContent(pending)) groups.push({ records: pending })
  const materialGroups = groups.filter((group) => hasRoundContent(group.records))
  return materialGroups.length > 0 ? materialGroups : [{ records }]
}

function tokenUsageRoundGroups(records) {
  const groups = []
  let pending = []
  let lastMessageRecords = []
  for (const [index, record] of (Array.isArray(records) ? records : []).entries()) {
    if (isMessageRecord(record)) lastMessageRecords = [record]
    pending.push(record)
    if (!isTokenUsageUpdateRecord(record)) continue
    const groupRecords = messagesFromEvents(pending).length > 0 ? pending : [...lastMessageRecords, ...pending]
    pending = []
    groups.push({
      turnId: firstTextOrNull(record.turn_id, record.turnId, record.id) || `token-usage-${index + 1}`,
      startedAt: firstTimestampOrNull(groupRecords[0]?.started_at, groupRecords[0]?.startedAt, groupRecords[0]?.timestamp, groupRecords[0]?.time, record.started_at, record.startedAt, record.timestamp, record.time),
      endedAt: firstTimestampOrNull(record.ended_at, record.endedAt, record.timestamp, record.time, groupRecords.at(-1)?.ended_at, groupRecords.at(-1)?.endedAt, groupRecords.at(-1)?.timestamp, groupRecords.at(-1)?.time),
      roundSource: "token-usage-jsonl",
      records: groupRecords,
    })
  }
  return groups
}

function isMessageRecord(record) {
  const itemType = firstTextOrNull(record?.item?.type)
  return itemType === "agent_message" ||
    itemType === "assistant_message" ||
    itemType === "user_message" ||
    itemType === "system_message" ||
    (record?.type === "message_end" && record.message) ||
    (record?.type === "text" && record.part?.text)
}

function visibleAgentRoundGroups(records) {
  const values = Array.isArray(records) ? records : []
  const groups = []
  let current = null
  let pending = []
  for (const record of values) {
    if (isVisibleAssistantBoundary(record)) {
      if (current) groups.push(current)
      current = {
        turnId: firstTextOrNull(record.item?.id, record.id) || `visible-round-${groups.length + 1}`,
        startedAt: firstTimestampOrNull(record.started_at, record.startedAt, record.timestamp, record.time),
        roundSource: "stdout-visible-round",
        usageUnavailable: true,
        records: [...pending, record],
      }
      pending = []
      continue
    }
    if (current) {
      current.records.push(record)
      current.endedAt = firstTimestampOrNull(record.ended_at, record.endedAt, record.timestamp, record.time, current.endedAt)
    } else {
      pending.push(record)
    }
  }
  if (current) groups.push(current)
  return groups.filter((group) => hasRoundContent(group.records))
}

function isVisibleAssistantBoundary(record) {
  const itemType = firstTextOrNull(record?.item?.type)
  return itemType === "agent_message" || itemType === "assistant_message" || (record?.type === "message_end" && record.message?.role === "assistant") || (record?.type === "text" && record.part?.text)
}

function codexRolloutRoundGroups(result) {
  const groups = []
  for (const { rolloutPath, records } of codexRolloutRecordSets(result)) {
    groups.push(...codexRolloutGroupsForRecords(records, rolloutPath))
  }
  return groups
}

function codexRolloutRecordSets(result) {
  const paths = codexRolloutPaths(result)
  return paths.map((rolloutPath) => ({
    rolloutPath,
    records: parseJsonlRecords(readOptionalText(rolloutPath)),
  })).filter((set) => set.records.length > 0)
}

function codexRolloutPaths(result) {
  const values = []
  for (const item of Array.isArray(result?.context_archive?.codex_rollout_paths) ? result.context_archive.codex_rollout_paths : []) {
    if (typeof item === "string") values.push(item)
  }
  const singlePath = firstTextOrNull(result?.context_archive?.codex_rollout_path)
  if (singlePath) values.push(singlePath)
  const pathsPath = firstTextOrNull(result?.context_archive?.codex_rollout_paths_path)
  if (pathsPath) {
    const parsed = parseJsonArray(readOptionalText(pathsPath))
    for (const item of parsed) if (typeof item === "string") values.push(item)
  }
  return [...new Set(values.map((value) => firstTextOrNull(value)).filter((value) => value && fs.existsSync(value)))]
}

function codexRolloutGroupsForRecords(records, rolloutPath) {
  const groups = []
  let previousUsageKey = ""
  for (const [index, record] of records.entries()) {
    if (!isCodexRolloutTokenRecord(record)) continue
    const usage = record.payload.info.last_token_usage
    const usageKey = JSON.stringify(record.payload.info.total_token_usage || usage)
    if (usageKey === previousUsageKey) continue
    previousUsageKey = usageKey
    const startIndex = codexRolloutGroupStart(records, index)
    const endIndex = codexRolloutGroupEnd(records, index)
    const groupRecords = records.slice(startIndex, endIndex + 1)
    const startedAt = firstTimestampOrNull(groupRecords[0]?.timestamp, record.timestamp)
    const endedAt = firstTimestampOrNull(groupRecords.at(-1)?.timestamp, record.timestamp, startedAt)
    const callId = firstTextOrNull(...groupRecords.map((item) => item?.payload?.call_id))
    const providerRecord = codexRolloutProviderRecord(groupRecords, rolloutPath, usage, startedAt, endedAt, groups.length, callId)
    groups.push({
      turnId: callId || `codex-rollout-${groups.length + 1}`,
      startedAt,
      endedAt,
      roundSource: "codex-rollout",
      rolloutPath,
      records: groupRecords,
      providerRecord,
    })
  }
  return groups
}

function isCodexRolloutTokenRecord(record) {
  return record?.type === "event_msg" && record.payload?.type === "token_count" && hasUsage(record.payload?.info?.last_token_usage)
}

function codexRolloutGroupStart(records, tokenIndex) {
  let index = tokenIndex
  while (index > 0 && isCodexRolloutModelOutput(records[index - 1])) index -= 1
  return index
}

function codexRolloutGroupEnd(records, tokenIndex) {
  let index = tokenIndex
  while (index + 1 < records.length && !isCodexRolloutModelOutput(records[index + 1])) index += 1
  return index
}

function isCodexRolloutModelOutput(record) {
  const payload = record?.payload
  if (!payload || typeof payload !== "object") return false
  if (record.type === "event_msg" && payload.type === "agent_message") return true
  if (record.type !== "response_item") return false
  return payload.type === "message" || payload.type === "function_call"
}

function codexRolloutProviderRecord(records, rolloutPath, usage, startedAt, endedAt, index, callId) {
  const messages = codexRolloutMessages(records)
  const outputText = messages.map((message) => message.text).filter(Boolean).join("\n\n")
  const output = records
    .filter((record) => record?.type === "response_item" && record.payload?.type === "function_call")
    .map((record) => ({ ...record.payload }))
  return {
    call_id: callId || `codex-rollout-${index + 1}`,
    started_at: startedAt,
    finished_at: endedAt,
    duration_ms: durationMsBetween(startedAt, endedAt),
    duration_source: "codex-rollout",
    round_source: "codex-rollout",
    usage_event_source: "codex-rollout-token-count",
    rollout_path: rolloutPath,
    response: {
      output_text: outputText || undefined,
      usage,
      output,
    },
    tool_calls: codexRolloutToolCalls(records),
  }
}

function codexRolloutMessages(records) {
  const messages = []
  for (const [index, record] of records.entries()) {
    const payload = record?.payload
    if (record?.type === "event_msg" && payload?.type === "agent_message") {
      messages.push(normalizedMessage({
        id: `codex-rollout-message-${index + 1}`,
        role: "assistant",
        type: "agent_message",
        content: payload.message,
        raw: payload,
      }, "codex-rollout.agent_message", index, "assistant"))
    }
  }
  return messages
}

function codexRolloutToolCalls(records) {
  const outputsByCallId = codexRolloutFunctionOutputs(records)
  const calls = []
  for (const [index, record] of records.entries()) {
    const payload = record?.payload
    if (record?.type !== "response_item" || payload?.type !== "function_call") continue
    const normalized = normalizeToolCall(payload, index)
    for (const [callIndex, call] of normalized.entries()) {
      const output = outputsByCallId.get(firstTextOrNull(call.parentToolCallId, payload.call_id, payload.id))?.[callIndex]
      calls.push(enrichToolCallWithCodexOutput(call, output))
    }
  }
  return calls
}

function codexRolloutFunctionOutputs(records) {
  const outputs = new Map()
  for (const record of records) {
    const payload = record?.payload
    if (record?.type !== "response_item" || payload?.type !== "function_call_output") continue
    const parsed = parseJsonObject(payload.output)
    const results = Array.isArray(parsed?.results) ? parsed.results : []
    outputs.set(firstTextOrNull(payload.call_id), results)
  }
  return outputs
}

function enrichToolCallWithCodexOutput(call, output) {
  if (!output || typeof output !== "object") return call
  const args = objectOrEmpty(call.arguments)
  const success = firstBoolean(output.success)
  return {
    ...call,
    arguments: jsonValue({
      ...args,
      status: success === false ? "failed" : "completed",
      exit_code: success === false ? 1 : 0,
      stdout: typeof output.output === "string" ? output.output : JSON.stringify(jsonValue(output.output) ?? output.output ?? ""),
      stderr: firstTextOrNull(output.stderr, output.error) || "",
    }) ?? args,
  }
}

function recordsForProviderRecord(records, providerRecord) {
  const values = Array.isArray(records) ? records : []
  const start = timestampToMillis(firstTextOrNull(providerRecord?.started_at, providerRecord?.startedAt))
  const end = timestampToMillis(firstTextOrNull(providerRecord?.finished_at, providerRecord?.finishedAt, providerRecord?.ended_at, providerRecord?.endedAt))
  const toolIds = new Set(providerToolCalls([providerRecord]).flatMap((call) => [call.id, call.parentToolCallId, call.parent_tool_call_id]).filter(Boolean).map(String))
  return values.filter((record) => {
    const toolId = firstTextOrNull(record?.item?.provider_tool_call_id, record?.item?.id)
    if (toolId && toolIds.has(String(toolId))) return true
    const ts = recordTimestampMillis(record)
    if (ts === null || start === null || end === null) return false
    return ts >= start && ts <= end
  })
}

function recordTimestampMillis(record) {
  return timestampToMillis(firstTextOrNull(
    record?.timestamp,
    record?.time,
    record?.started_at,
    record?.startedAt,
    record?.ended_at,
    record?.endedAt,
    record?.item?.timestamp,
    record?.item?.started_at,
    record?.item?.startedAt,
    record?.item?.ended_at,
    record?.item?.endedAt,
  ))
}

function hasRoundContent(records) {
  return Array.isArray(records) && records.some((record) => {
    const itemType = firstTextOrNull(record?.item?.type)
    return Boolean(
      itemType === "agent_message"
        || itemType === "assistant_message"
        || itemType === "user_message"
        || itemType === "system_message"
        || itemType === "command_execution"
        || ((record?.type === "turn.completed" || record?.type === "turn_end") && (record.usage || record.message?.usage))
        || (record?.type === "step_finish" && record.part?.tokens)
        || (record?.type === "text" && record.part?.text)
        || (record?.type === "message_end" && record.message)
        || isTokenUsageUpdateRecord(record),
    )
  })
}

function providerRecordsForTurn(providerRecords, groups, index) {
  if (!Array.isArray(providerRecords) || providerRecords.length === 0) return []
  if (groups.length === 1) return providerRecords
  if (providerRecords.length === groups.length) return [providerRecords[index]].filter(Boolean)
  return []
}

function inferAgentId(record) {
  const type = firstTextOrNull(record.type, record.event, record.event_type, record.eventType) || ""
  const prefix = type.split(".")[0]
  if (!prefix) return "unknown"
  return prefix === "claude" ? "claudecode" : prefix
}

function inferAgentKind(agentId) {
  const text = String(agentId || "unknown").replace(/-\d+$/, "")
  if (text.startsWith("tura-")) return "tura"
  if (text.startsWith("codex-")) return "codex"
  if (text === "claude-code") return "claudecode"
  if (text === "pi-agent") return "pi"
  if (text === "opencode") return "opencode"
  return text || "unknown"
}

function inferAgentMode(agentId) {
  const text = String(agentId || "")
  if (text.startsWith("tura-")) return text.slice("tura-".length).replace(/-shll$/, "")
  if (text.startsWith("codex-")) return text.slice("codex-".length)
  if (text === "claude-code" || text === "pi-agent" || text === "opencode") return "cli"
  return "unknown"
}

function pushRoundCallbacks(callbacks, value) {
  if (!Array.isArray(value)) return
  for (const item of value) callbacks.push(item)
}

function pushJsonlCallbacks(callbacks, text) {
  for (const record of parseJsonlRecords(text)) callbacks.push(record)
}

function parseJsonlRecords(text) {
  if (typeof text !== "string" || !text.trim()) return []
  const records = []
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue
    try {
      records.push(JSON.parse(line))
    } catch {
      // Ignore non-JSON progress lines from CLIs that mix human output and JSONL.
    }
  }
  return records
}

function parseJsonArray(text) {
  try {
    const parsed = JSON.parse(String(text || ""))
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function isExplicitRoundRecord(record) {
  if (!record || typeof record !== "object" || Array.isArray(record)) return false
  const type = firstTextOrNull(record.type, record.event, record.event_type, record.eventType)
  if (type && /(^|\.)round\./.test(type)) return true
  return Boolean(record.roundId || record.round_id || record.turnId || record.turn_id || record.session_id || record.sessionId)
}

function readOptionalText(file) {
  if (typeof file !== "string" || !file || !fs.existsSync(file)) return ""
  return fs.readFileSync(file, "utf8")
}

function modelPatchDiff(summary) {
  const chunks = []
  for (const result of Array.isArray(summary?.results) ? summary.results : []) {
    const patchText = modelPatchText(result)
    if (!patchText.trim()) continue
    const agentId = firstText(result?.agent_id, result?.agent, result?.id, "agent")
    chunks.push(`# ---- ${agentId} model.patch ----\n${patchText.trimEnd()}`)
  }
  return chunks.length > 0 ? `${chunks.join("\n\n")}\n` : ""
}

function modelPatchText(result) {
  const patch = result?.patch || {}
  if (typeof patch.patch_text === "string") return patch.patch_text
  if (typeof patch.diff === "string") return patch.diff
  if (typeof patch.patch === "string") return patch.patch
  return readOptionalText(patch.patch_path || patch.patchPath)
}

function modelPatchPaths(summary) {
  return (Array.isArray(summary?.results) ? summary.results : [])
    .map((result) => result?.patch?.patch_path || result?.patch?.patchPath)
    .filter((item) => typeof item === "string" && item.length > 0)
}

function usageFromRecord(record) {
  const usage = { inputTokens: 0, cacheInputTokens: 0, outputTokens: 0, reasoningTokens: 0, totalTokens: 0 }
  for (const item of [record.usage, record.metrics, record.runtime_usage, record.message?.usage, record.result?.usage, record.assistantMessageEvent?.usage, record.response?.usage, record.body?.usage, record.part?.tokens]) {
    if (!item || typeof item !== "object") continue
    usage.inputTokens += usageNumber(item, "input")
    usage.cacheInputTokens += usageNumber(item, "cached")
    usage.outputTokens += usageNumber(item, "output")
    usage.reasoningTokens += usageNumber(item, "reasoning")
    usage.totalTokens += usageNumber(item, "total")
  }
  if (usage.totalTokens === 0) usage.totalTokens = usage.inputTokens + usage.outputTokens
  return usage
}

function usageNumber(item, kind) {
  if (!item || typeof item !== "object") return 0
  if (kind === "input") {
    const standardInput = firstFinite(item.inputTokens, item.input_tokens, item.prompt_tokens)
    if (standardInput !== null) return standardInput
    return firstNumber(item.input, 0) + usageNumber(item, "cached") + usageNumber(item, "cacheWrite")
  }
  if (kind === "cached") return firstNumber(item.cacheInputTokens, item.cached_input_tokens, item.cached, item.cache_read_input_tokens, item.cacheRead, item.cache?.read, item.input_tokens_details?.cached_tokens, item.prompt_tokens_details?.cached_tokens, 0)
  if (kind === "cacheWrite") return firstNumber(item.cacheWriteTokens, item.cache_write_tokens, item.cacheWrite, item.cache?.write, item.input_tokens_details?.cache_write_tokens, item.prompt_tokens_details?.cache_creation_tokens, 0)
  if (kind === "output") return firstNumber(item.outputTokens, item.output_tokens, item.completion_tokens, item.output, 0)
  if (kind === "reasoning") return firstNumber(item.reasoningTokens, item.reasoning_tokens, item.reasoning_output_tokens, item.reasoning, item.output_tokens_details?.reasoning_tokens, item.completion_tokens_details?.reasoning_tokens, 0)
  if (kind === "total") return firstNumber(item.totalTokens, item.total_tokens, item.total, 0)
  return 0
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
  if ((call.kind === "command" || call.kind === "tool") && firstTextOrNull(call.commandLine, call.command_line)) {
    return [{
      id: firstTextOrNull(call.id, call.call_id, call.tool_call_id) || `${call.name || call.kind}-${index + 1}`,
      kind: call.kind,
      name: firstTextOrNull(call.name, call.tool_name) || call.kind,
      commandLine: firstText(call.commandLine, call.command_line),
      arguments: jsonValue(call.arguments ?? call.args ?? call.input ?? {}) ?? {},
      parentToolName: firstTextOrNull(call.parentToolName, call.parent_tool_name),
      parentToolCallId: firstTextOrNull(call.parentToolCallId, call.parent_tool_call_id),
      parallelGroupId: firstTextOrNull(call.parallelGroupId, call.parallel_group_id, call.step),
      startedAt: firstTextOrNull(call.startedAt, call.started_at),
      endedAt: firstTextOrNull(call.endedAt, call.ended_at),
      raw: jsonValue(call.raw ?? call),
    }]
  }
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

function messagesFromRecord(record) {
  return mergeMessages(
    inputMessages(record),
    outputMessages(record),
    messagesFromProviderRecord(record),
  )
}

function inputMessages(record) {
  const messages = []
  pushNormalizedMessages(messages, record.messages, "messages")
  pushNormalizedMessages(messages, record.input?.messages, "input.messages")
  pushNormalizedMessages(messages, record.request?.messages, "request.messages")
  pushNormalizedMessages(messages, record.body?.messages, "body.messages")
  pushNormalizedMessages(messages, record.request?.input, "request.input")
  pushNormalizedMessages(messages, record.body?.input, "body.input")
  if (messages.length === 0) {
    const context = firstTextOrNull(record.fullContext, record.full_context, record.inputContext, record.input_context, record.context)
    if (context) messages.push(normalizedMessage({ role: "user", content: context }, "full_context", messages.length))
  }
  return messages.filter((message) => message.role !== "assistant")
}

function outputMessages(record) {
  const explicitAssistantMessages = normalizedMessageArray(record.messages, "messages").filter((message) => message.role === "assistant")
  if (explicitAssistantMessages.length > 0) return explicitAssistantMessages
  return mergeMessages(
    normalizedMessageArray(record.message, "message", "assistant"),
    normalizedMessageArray(record.assistantMessage, "assistantMessage", "assistant"),
    normalizedMessageArray(record.assistant_message, "assistant_message", "assistant"),
    normalizedMessageArray(record.output?.message, "output.message", "assistant"),
    normalizedMessageArray(record.response?.output_text, "response.output_text", "assistant"),
    normalizedMessageArray(record.body?.output_text, "body.output_text", "assistant"),
    messagesFromOpenAiOutput(record.response?.output ?? record.body?.output ?? record.output),
  )
}

function messagesFromProviderRecord(record) {
  return mergeMessages(
    normalizedMessageArray(record?.response?.output_text, "provider.response.output_text", "assistant"),
    messagesFromOpenAiOutput(record?.response?.output),
    normalizedMessageArray(record?.output_text, "provider.output_text", "assistant"),
  )
}

function messagesFromEvents(records) {
  const messages = []
  for (const [index, record] of records.entries()) {
    const item = record?.item
    if (item && typeof item === "object" && ["agent_message", "assistant_message", "user_message", "system_message"].includes(item.type)) {
      messages.push(normalizedMessage({
        id: item.id,
        role: item.type === "user_message" ? "user" : item.type === "system_message" ? "system" : "assistant",
        type: item.type,
        content: item.text ?? item.message ?? item.content,
        usage: item.usage,
        raw: item,
      }, `stdout.${record.type || "event"}`, index))
      continue
    }
    if (record?.type === "message_end" && record.message) {
      messages.push(normalizedMessage({
        id: record.message.id,
        role: record.message.role,
        type: "message",
        content: record.message.content,
        usage: record.message.usage,
        raw: record.message,
      }, "stdout.message_end", index))
      continue
    }
    if (record?.type === "text" && record.part?.text) {
      messages.push(normalizedMessage({
        id: record.part.id,
        role: "assistant",
        type: "message",
        content: record.part.text,
        raw: record.part,
      }, "stdout.text", index, "assistant"))
    }
  }
  return messages
}

function pushNormalizedMessages(target, value, source) {
  for (const message of normalizedMessageArray(value, source)) target.push(message)
}

function normalizedMessageArray(value, source, fallbackRole = "unknown") {
  if (value === undefined || value === null) return []
  const values = Array.isArray(value) ? value : [value]
  return values
    .map((item, index) => normalizedMessage(item, source, index, fallbackRole))
    .filter((message) => message.text || (message.content !== null && message.content !== ""))
}

function normalizedMessage(value, source, index = 0, fallbackRole = "unknown") {
  const object = value && typeof value === "object" && !Array.isArray(value) ? value : { content: value }
  const content = object.content ?? object.text ?? object.message ?? object.output_text ?? object.value ?? value
  const text = typeof content === "string" ? content : textFromContent(content) || textFromOutput(content) || (content === undefined || content === null ? "" : stringifyFirst(content))
  return {
    id: firstTextOrNull(object.id, object.message_id, object.messageId, object.item_id) || `${safeFileName(source)}-${index + 1}`,
    role: firstTextOrNull(object.role) || fallbackRole,
    type: firstTextOrNull(object.type) || "message",
    text,
    content: jsonValue(content) ?? null,
    usage: usageFromRecord(object),
    source,
    raw: jsonValue(object) ?? {},
  }
}

function messagesFromOpenAiOutput(value) {
  const messages = []
  for (const [index, item] of (Array.isArray(value) ? value : []).entries()) {
    if (!item || typeof item !== "object" || isToolCall(item)) continue
    const text = firstTextOrNull(item.text, item.output_text) || textFromContent(item.content)
    if (text) messages.push(normalizedMessage({ ...item, role: item.role || "assistant", content: text }, "response.output", index, "assistant"))
  }
  return messages
}

function mergeMessages(...groups) {
  const merged = []
  const seen = new Set()
  for (const group of groups) {
    for (const message of Array.isArray(group) ? group : []) {
      const key = `${message.role}\u0000${message.type}\u0000${message.text}\u0000${message.source}`
      if (seen.has(key)) continue
      seen.add(key)
      merged.push(message)
    }
  }
  return merged
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

function providerRoundRecords(result) {
  const records = []
  const archivePath = firstTextOrNull(result?.context_archive?.provider_calls_full_path)
  if (archivePath) records.push(...parseJsonlRecords(readOptionalText(archivePath)))
  if (Array.isArray(result?.provider_calls)) records.push(...result.provider_calls)
  return dedupeProviderRecords(records.filter((record) => record && typeof record === "object" && !Array.isArray(record)))
}

function dedupeProviderRecords(records) {
  const unique = []
  const seen = new Set()
  for (const record of records) {
    const usage = record?.response?.usage || record?.metrics?.usage || record?.usage || {}
    const key = [
      firstTextOrNull(record.call_id, record.id),
      firstTextOrNull(record.started_at, record.startedAt),
      firstTextOrNull(record.finished_at, record.finishedAt),
      firstTextOrNull(record.model, record.provider_model),
      usageNumber(usage, "input"),
      usageNumber(usage, "output"),
    ].join("\u0000")
    if (seen.has(key)) continue
    seen.add(key)
    unique.push(record)
  }
  return unique
}

function usageFromAggregateResult(result, providerRecords, stdoutRecords) {
  if (Array.isArray(providerRecords) && providerRecords.length > 0) {
    const providerUsage = { inputTokens: 0, cacheInputTokens: 0, outputTokens: 0, reasoningTokens: 0, totalTokens: 0 }
    for (const record of providerRecords) addUsageInto(providerUsage, usageFromRecord(record.response || record.metrics || record))
    if (providerUsage.inputTokens > 0 || providerUsage.outputTokens > 0 || providerUsage.totalTokens > 0) {
      if (providerUsage.totalTokens === 0) providerUsage.totalTokens = providerUsage.inputTokens + providerUsage.outputTokens
      return providerUsage
    }
  }
  const stdoutUsage = sumRecordUsage(stdoutRecords)
  if (stdoutUsage.inputTokens > 0 || stdoutUsage.outputTokens > 0 || stdoutUsage.totalTokens > 0) return stdoutUsage
  const resultUsage = objectOrEmpty(result?.usage)
  const usage = {
    inputTokens: usageNumber(resultUsage, "input"),
    cacheInputTokens: usageNumber(resultUsage, "cached"),
    outputTokens: usageNumber(resultUsage, "output"),
    reasoningTokens: usageNumber(resultUsage, "reasoning"),
    totalTokens: usageNumber(resultUsage, "total"),
  }
  if (usage.inputTokens === 0 && usage.outputTokens === 0) {
    for (const record of stdoutRecords) addUsageInto(usage, usageFromRecord(record))
  }
  if (usage.totalTokens === 0) usage.totalTokens = usage.inputTokens + usage.outputTokens
  return usage
}

function emptyContractUsage() {
  return { inputTokens: 0, cacheInputTokens: 0, outputTokens: 0, reasoningTokens: 0, totalTokens: 0 }
}

function sumRecordUsage(records) {
  const usage = { inputTokens: 0, cacheInputTokens: 0, outputTokens: 0, reasoningTokens: 0, totalTokens: 0 }
  const sourceRecords = tokenUsageRecords(records)
  for (const record of sourceRecords) addUsageInto(usage, usageFromRecord(record))
  if (usage.totalTokens === 0) usage.totalTokens = usage.inputTokens + usage.outputTokens
  return usage
}

function tokenUsageRecords(records) {
  const values = Array.isArray(records) ? records : []
  const compactRecords = compactUsageRecords(values)
  const updates = values.filter(isTokenUsageUpdateRecord)
  if (updates.length > 0) return mergeUsageRecords(updates, compactRecords)
  const piTurnEnds = values.filter((record) => record?.type === "turn_end" && hasUsage(record.message?.usage))
  if (piTurnEnds.length > 0) return mergeUsageRecords(piTurnEnds, compactRecords)
  const opencodeStepFinish = values.filter((record) => record?.type === "step_finish" && hasUsage(record.part?.tokens))
  if (opencodeStepFinish.length > 0) return mergeUsageRecords(opencodeStepFinish, compactRecords)
  return values
}

function isTokenUsageUpdateRecord(record) {
  return record?.type === "thread.token_usage.updated" && hasUsage(record.usage)
}

function compactUsageRecords(records) {
  return (Array.isArray(records) ? records : []).filter((record) => isCompactUsageRecord(record) && hasUsage(usageCandidate(record)))
}

function isCompactUsageRecord(record) {
  if (!record || typeof record !== "object") return false
  const text = [
    record.type,
    record.event,
    record.event_type,
    record.eventType,
    record.kind,
    record.name,
    record.action,
  ].map((value) => String(value || "").toLowerCase()).join(" ")
  return /\b(compact|compaction|summarize|summary)\b/.test(text)
}

function mergeUsageRecords(primary, extra) {
  const merged = []
  const seen = new Set()
  for (const record of [...(Array.isArray(primary) ? primary : []), ...(Array.isArray(extra) ? extra : [])]) {
    if (!record || seen.has(record)) continue
    seen.add(record)
    merged.push(record)
  }
  return merged
}

function usageCandidate(record) {
  if (!record || typeof record !== "object") return null
  return [
    record.usage,
    record.metrics,
    record.runtime_usage,
    record.message?.usage,
    record.result?.usage,
    record.assistantMessageEvent?.usage,
    record.response?.usage,
    record.body?.usage,
    record.part?.tokens,
    record.payload?.info?.last_token_usage,
  ].find(hasUsage) || null
}

function hasUsage(value) {
  if (!value || typeof value !== "object") return false
  return [
    value.inputTokens,
    value.input_tokens,
    value.prompt_tokens,
    value.input,
    value.outputTokens,
    value.output_tokens,
    value.completion_tokens,
    value.output,
    value.totalTokens,
    value.total_tokens,
    value.reasoningTokens,
    value.reasoning_tokens,
    value.reasoning_output_tokens,
    value.cacheInputTokens,
    value.cached_input_tokens,
    value.cache_read_input_tokens,
    value.cacheRead,
    value.cache?.read,
  ].some((item) => Number(item || 0) > 0)
}

function addUsageInto(total, usage) {
  total.inputTokens += usage.inputTokens
  total.cacheInputTokens += usage.cacheInputTokens
  total.outputTokens += usage.outputTokens
  total.reasoningTokens += usage.reasoningTokens
  total.totalTokens += usage.totalTokens
}

function assistantMessagesFromEvents(records) {
  const messages = []
  for (const record of records) {
    const item = record?.item
    if (item?.type === "agent_message" || item?.type === "assistant_message") {
      const text = firstTextOrNull(item.text, item.message, item.content) || textFromContent(item.content)
      if (text) messages.push(text)
    }
  }
  return messages
}

function providerToolCalls(providerRecords) {
  const calls = []
  for (const record of providerRecords) {
    if (Array.isArray(record?.tool_calls) && record.tool_calls.length > 0) {
      calls.push(...record.tool_calls)
      continue
    }
    if (Array.isArray(record?.toolCalls) && record.toolCalls.length > 0) {
      calls.push(...record.toolCalls)
      continue
    }
    pushOpenAiOutput(calls, record?.response?.output)
    const events = Array.isArray(record?.response?.events) ? record.response.events : []
    const completedItemIds = new Set(
      events
        .filter((event) => event?.type === "response.output_item.done" && isToolCall(event.item))
        .map((event) => firstTextOrNull(event.item?.id, event.item?.call_id))
        .filter(Boolean),
    )
    for (const event of events) {
      if (event?.type === "response.output_item.done" && isToolCall(event.item)) calls.push(event.item)
      if (event?.type === "response.function_call_arguments.done") {
        const itemId = firstTextOrNull(event.item_id, event.call_id)
        if (itemId && completedItemIds.has(itemId)) continue
        calls.push({
          id: event.item_id,
          call_id: event.call_id,
          name: "command_run",
          arguments: event.arguments,
          type: "function_call",
        })
      }
    }
  }
  return calls
}

function commandExecutionToolCalls(records) {
  const byId = new Map()
  for (const record of records) {
    const item = record?.item
    if (item?.type !== "command_execution") continue
    const id = firstTextOrNull(item.id, item.provider_tool_call_id) || `command-${byId.size + 1}`
    const existing = byId.get(id)
    if (!existing || record.type === "item.completed") byId.set(id, { event: record, item })
  }
  const commands = []
  for (const { event, item } of byId.values()) {
    const commandLine = firstTextOrNull(item.command, item.commandLine, item.command_line)
    if (!commandLine) continue
    commands.push({
      id: firstTextOrNull(item.id, item.provider_tool_call_id),
      kind: "command",
      name: firstTextOrNull(item.command_type, item.commandType) || inferCommandName(commandLine),
      commandLine,
      arguments: jsonValue({ status: item.status, exit_code: item.exit_code, aggregated_output: item.aggregated_output }) ?? {},
      parentToolName: "command_execution",
      parentToolCallId: firstTextOrNull(item.provider_tool_call_id),
      parallelGroupId: firstTextOrNull(item.parallel_group_id, item.parallelGroupId, item.command_index),
      raw: jsonValue(event),
    })
  }
  return commands
}

function piToolExecutionToolCalls(records) {
  const byId = new Map()
  for (const record of records) {
    if (!String(record?.type || "").startsWith("tool_execution_")) continue
    const id = firstTextOrNull(record.toolCallId, record.tool_call_id, record.id) || `pi-tool-${byId.size + 1}`
    const existing = byId.get(id) || {}
    byId.set(id, {
      ...existing,
      id,
      toolName: firstTextOrNull(record.toolName, record.tool_name, existing.toolName),
      args: objectOrEmpty(record.args ?? existing.args),
      result: record.result ?? record.partialResult ?? existing.result,
      startedAt: firstTimestampOrNull(existing.startedAt, record.started_at, record.startedAt, record.timestamp, record.time),
      endedAt: record.type === "tool_execution_end"
        ? firstTimestampOrNull(record.ended_at, record.endedAt, record.timestamp, record.time, existing.endedAt)
        : existing.endedAt,
      isError: firstBoolean(record.isError, record.is_error, existing.isError) ?? false,
      raw: record,
    })
  }
  return [...byId.values()].map((call) => {
    const output = toolResultText(call.result)
    const commandLine = firstTextOrNull(call.args.command, call.args.commandLine, call.args.command_line, call.args.filePath, call.args.file_path) || compactJson(call.args) || firstText(call.toolName, "tool")
    return {
      id: call.id,
      kind: inferToolKind(call.toolName, commandLine),
      name: firstText(call.toolName, "tool"),
      commandLine,
      arguments: jsonValue({
        ...call.args,
        status: call.isError ? "failed" : "done",
        stdout: output,
        stderr: call.isError ? output : "",
        duration_ms: durationMsBetween(call.startedAt, call.endedAt),
      }) ?? {},
      startedAt: call.startedAt,
      endedAt: call.endedAt,
      raw: jsonValue(call.raw) ?? {},
    }
  })
}

function opencodeToolUseToolCalls(records) {
  const calls = []
  for (const record of records) {
    if (record?.type !== "tool_use" || !record.part) continue
    const part = record.part
    const state = objectOrEmpty(part.state)
    const input = objectOrEmpty(state.input)
    const output = firstTextOrNull(state.output, state.metadata?.output, state.metadata?.preview) || ""
    const startedAt = firstTimestampOrNull(state.time?.start, record.started_at, record.startedAt, record.timestamp, record.time)
    const endedAt = firstTimestampOrNull(state.time?.end, record.ended_at, record.endedAt, record.timestamp, record.time)
    const name = firstText(part.tool, state.tool, "tool")
    const commandLine = firstTextOrNull(input.command, state.title, input.filePath, input.file_path, input.pattern) || compactJson(input) || name
    calls.push({
      id: firstText(part.callID, part.callId, part.id, `opencode-tool-${calls.length + 1}`),
      kind: inferToolKind(name, commandLine),
      name,
      commandLine,
      arguments: jsonValue({
        ...input,
        status: firstText(state.status, "unknown"),
        stdout: output,
        stderr: firstTextOrNull(state.metadata?.stderr, state.metadata?.error) || "",
        duration_ms: durationMsBetween(startedAt, endedAt),
      }) ?? {},
      startedAt,
      endedAt,
      raw: jsonValue(record) ?? {},
    })
  }
  return calls
}

function inferToolKind(name, commandLine) {
  const text = String(name || commandLine || "").toLowerCase()
  return /bash|shell|cmd|powershell|pwsh|command/.test(text) ? "command" : "tool"
}

function toolResultText(result) {
  if (typeof result === "string") return result
  if (!result || typeof result !== "object") return ""
  const content = Array.isArray(result.content) ? result.content : []
  const pieces = []
  for (const item of content) {
    if (typeof item === "string") pieces.push(item)
    else if (typeof item?.text === "string") pieces.push(item.text)
    else if (item !== null && item !== undefined) pieces.push(compactJson(item))
  }
  return pieces.filter(Boolean).join("\n")
}

function compactJson(value) {
  const json = JSON.stringify(jsonValue(value) ?? {})
  return json === "{}" ? "" : json
}

function mergeToolCalls(...groups) {
  const merged = []
  const seen = new Set()
  for (const group of groups) {
    for (const call of Array.isArray(group) ? group : []) {
      const key = `${firstTextOrNull(call.call_id, call.id, call.name)}\u0000${firstTextOrNull(call.arguments, call.commandLine, call.command_line) || JSON.stringify(jsonValue(call.arguments ?? call.input ?? call) ?? {})}`
      if (seen.has(key)) continue
      seen.add(key)
      merged.push(call)
    }
  }
  return merged
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

function harnessScores(summary, paths) {
  const scores = []
  for (const result of Array.isArray(summary.results) ? summary.results : []) {
    const reports = Array.isArray(result?.eval?.report?.reports) ? result.eval.report.reports : []
    let passed = 0
    let failed = 0
    for (const report of reports) {
      passed += firstNumber(report?.passed, 0)
      failed += firstNumber(report?.failed, 0)
    }
    if (!result?.eval?.ran && reports.length === 0) continue
    const total = passed + failed
    scores.push({
      harnessId: `${paths.test_name}:${firstText(result?.task, "task")}:${firstText(result?.agent, "agent")}`,
      score: total > 0 ? passed / total : (Number(result?.eval?.exit_code) === 0 ? 1 : 0),
      maxScore: 1,
      passed: Number(result?.eval?.exit_code) === 0 && failed === 0,
      details: jsonValue({
        agent: result?.agent ?? null,
        task: result?.task ?? null,
        evalExitCode: result?.eval?.exit_code ?? null,
        passed,
        failed,
        stdoutPath: result?.eval?.stdout_path ?? null,
        stderrPath: result?.eval?.stderr_path ?? null,
      }) ?? {},
      artifacts: [result?.eval?.stdout_path, result?.eval?.stderr_path].filter((value) => typeof value === "string" && value),
    })
  }
  return scores
}

function aggregateScores(scores) {
  if (!Array.isArray(scores) || scores.length === 0) return null
  return scores.reduce((total, score) => total + Number(score.score || 0), 0) / scores.length
}

function modelForAgent(agentId, summary, result = {}) {
  const text = String(agentId || "")
  if (text.startsWith("tura-")) return firstTextOrNull(result.tura_model, summary.tura_model, result.model, summary.model) || "unknown"
  return firstTextOrNull(result.model, summary.model, summary.tura_model) || "unknown"
}

function isPriority(summary) {
  return firstBoolean(summary.priority_enabled, summary.priorityEnabled, summary.priority, summary.is_priority) ??
    String(firstTextOrNull(summary.service_tier, summary.serviceTier) || "").toLowerCase() === "priority"
}

function firstNumber(...values) {
  return firstFinite(...values) ?? 0
}

function firstNumberOrNull(...values) {
  return firstFinite(...values)
}

function firstTimestampOrNull(...values) {
  for (const value of values) {
    const normalized = timestampToIso(value)
    if (normalized) return normalized
  }
  return null
}

function timestampToIso(value) {
  if (value === undefined || value === null || value === "") return null
  if (typeof value === "string" && value.trim() && !/^-?\d+(\.\d+)?$/.test(value.trim())) return value
  const number = Number(value)
  if (!Number.isFinite(number) || number <= 0) return null
  const millis = number > 1_000_000_000_000 ? number : number * 1000
  const date = new Date(millis)
  return Number.isFinite(date.getTime()) ? date.toISOString() : null
}

function durationMsBetween(startedAt, endedAt) {
  return durationMsBetweenOrNull(startedAt, endedAt) ?? 0
}

function durationMsBetweenOrNull(startedAt, endedAt) {
  const start = timestampToMillis(startedAt)
  const end = timestampToMillis(endedAt)
  if (start === null || end === null || end < start) return null
  return end - start
}

function fallbackRoundDurationMs(result, turnIndex, turnCount) {
  const elapsed = firstFinite(result?.elapsed_ms, result?.duration_ms)
  const count = Math.max(1, Number(turnCount || result?.events?.llm_rounds || result?.usage?.usage_events || 1))
  if (elapsed === null || elapsed <= 0) return null
  const base = Math.floor(elapsed / count)
  const remainder = Math.round(elapsed - base * count)
  return base + (turnIndex < remainder ? 1 : 0)
}

function timestampToMillis(value) {
  const normalized = timestampToIso(value)
  if (!normalized) return null
  const millis = Date.parse(normalized)
  return Number.isFinite(millis) ? millis : null
}

function firstFinite(...values) {
  for (const value of values) {
    if (value === undefined || value === null || value === "") continue
    const number = Number(value)
    if (Number.isFinite(number)) return number
  }
  return null
}

function firstPositiveNumberOrNull(...values) {
  for (const value of values) {
    if (value === undefined || value === null || value === "") continue
    const number = Number(value)
    if (Number.isFinite(number) && number > 0) return number
  }
  return null
}

function firstBoolean(...values) {
  for (const value of values) {
    if (typeof value === "boolean") return value
    if (typeof value === "string") {
      const normalized = value.trim().toLowerCase()
      if (["1", "true", "yes", "on", "enabled"].includes(normalized)) return true
      if (["0", "false", "no", "off", "disabled"].includes(normalized)) return false
    }
    if (typeof value === "number" && Number.isFinite(value)) return value !== 0
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
