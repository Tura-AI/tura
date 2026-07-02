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
  const usage = tokenUsage(summary)
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
    roundsDirectory: path.join(paths.run_root, "contracts", "rounds"),
    rounds: [],
    sourceSummaryPath: paths.summary_path,
  }
}

function tokenUsage(summary) {
  const usage = summary.aggregate_usage || summary.token_totals || summary.standard_metrics?.token_usage || summary.usage || {}
  return {
    inputTokens: firstNumber(usage.inputTokens, usage.input_tokens, usage.input, usage.prompt_tokens, 0),
    cacheInputTokens: firstNumber(usage.cacheInputTokens, usage.cached_input_tokens, usage.cached, usage.cache_read_input_tokens, 0),
    outputTokens: firstNumber(usage.outputTokens, usage.output_tokens, usage.output, usage.completion_tokens, 0),
    reasoningTokens: firstNumber(usage.reasoningTokens, usage.reasoning_tokens, usage.reasoning, usage.reasoning_output_tokens, 0),
    totalTokens: firstNumber(usage.totalTokens, usage.total_tokens, usage.total, 0),
  }
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
