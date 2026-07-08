#!/usr/bin/env node
import assert from "node:assert/strict"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { spawnSync } from "node:child_process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"

import { businessRunPaths, normalizeBusinessSummary } from "../../../lib/business_paths.mjs"
import { buildMatrix, mapWithConcurrency, parseTaskList, safeName, timestampId } from "../../../lib/debug_suite_matrix.mjs"
import {
  buildGenericAgentRuns,
  ensureGenericAgentExecutables,
  eventsForAgent,
  eventsWithUsageRounds,
  genericAgentKind,
  genericAgentMode,
  modelForGenericAgent,
  parseGenericAgents,
  priorityEnabled,
  runGenericAgentCli,
  usageForAgent,
} from "../../../lib/generic_agent_cli.mjs"
import {
  loadOracleMatrix,
  oracleMatrixForTask,
  runBinaryAudit,
  validateOracleMatrix,
} from "./harness.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const tasksPath = path.join(scriptDir, "tasks.json")
const taskConfig = JSON.parse(fs.readFileSync(tasksPath, "utf8"))
const oracleMatrixConfig = loadOracleMatrix(scriptDir)
const requestedTaskIds = parseTaskList({
  value: process.env.COMMAND_RUN_AGENT_TASKS,
  suiteValue: process.env.COMMAND_RUN_AGENT_CLI_BUGFIX_TASKS,
  fallback: ["all"],
  label: "COMMAND_RUN_AGENT_TASKS/COMMAND_RUN_AGENT_CLI_BUGFIX_TASKS",
})
const selectedTasks = selectTasks(taskConfig.tasks, requestedTaskIds)
const benchmarkTaskName = process.env.COMMAND_RUN_AGENT_BENCHMARK_TASK_NAME || "cli-bugfix-binary-matrix-10"
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `${benchmarkTaskName}-${timestampId()}`
const runPaths = businessRunPaths(benchmarkTaskName, runId)
const selfTest = process.env.COMMAND_RUN_AGENT_SELF_TEST === "1" || process.env.CLI_BUGFIX_SELF_TEST === "1"
const harnessOnly = process.env.COMMAND_RUN_AGENT_HARNESS_ONLY === "1"
const binaryAudit = process.env.COMMAND_RUN_AGENT_BINARY_AUDIT === "1"
const runAgents = process.env.COMMAND_RUN_AGENT_RUN_AGENTS === "1"
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "medium"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "default"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 60 * 60_000)
const agents = parseGenericAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "tura-balanced,tura-direct,codex-main")
const agentRuns = buildGenericAgentRuns(agents)
const routeConcurrency = Number(process.env.COMMAND_RUN_AGENT_ROUTE_CONCURRENCY || process.env.COMMAND_RUN_AGENT_CONCURRENCY || Math.max(1, selectedTasks.length * agentRuns.length))
const workspacePrepConcurrency = Number(process.env.COMMAND_RUN_AGENT_WORKSPACE_PREP_CONCURRENCY || 1)
const promptPolicyVersion = "cli-bugfix-prompt-v3"
const harnessMetadataVersion = "cli-bugfix-binary-harness-metadata-v3"

function selectTasks(tasks, ids) {
  if (ids.some((id) => /^(all|\*)$/i.test(id))) return tasks
  const byId = new Map(tasks.flatMap((task) => [[task.id, task], [task.label, task]]))
  return ids.map((id) => {
    const task = byId.get(id)
    assert(task, `unknown CLI bug-fix task: ${id}`)
    return task
  })
}

function validateTaskConfig(config) {
  assert.equal(config.schema, "tura.debug.cli-bugfix-binary-matrix.v1")
  assert.equal(config.tasks.length, 10)
  assert.deepEqual(countBy(config.tasks, "sourceLanguage"), {
    Go: 2,
    Java: 2,
    Python: 2,
    Rust: 2,
    TypeScript: 2,
  })
  const labels = new Set()
  for (const task of config.tasks) {
    assert(!labels.has(task.label), `duplicate task label: ${task.label}`)
    labels.add(task.label)
    assert(task.id && task.label && task.sourceLanguage, `incomplete task identity: ${task.id || task.label}`)
    assert(task.repo?.url && task.repo?.owner && task.repo?.name, `missing repo metadata for ${task.label}`)
    assert(Number(task.locEstimate) >= 50000, `${task.label} locEstimate is below target range`)
    assert(Number(task.locEstimate) <= 500000, `${task.label} locEstimate is above target range`)
    assert(task.bug?.issue && task.bug?.fix, `missing issue/fix evidence for ${task.label}`)
    assert(task.bug?.buggyVersion && task.bug?.fixedVersion, `missing versions for ${task.label}`)
    assert(task.bug?.issueTitle && task.bug?.issueExcerpt, `missing issue quote metadata for ${task.label}`)
    assert(Number.isInteger(Number(task.bug?.issueNumber)), `missing issue number metadata for ${task.label}`)
    const issueRef = issueReference(task)
    assert.equal(Number(task.bug.issueNumber), issueRef.number, `issue number does not match issue URL for ${task.label}`)
    assert(extractIssueTextForPrompt(task).length > 40, `missing extracted issue text for ${task.label}`)
    assert(isSha(task.bug?.buggyCommit), `missing buggy commit sha for ${task.label}`)
    assert(isSha(task.bug?.fixedCommit), `missing fixed commit sha for ${task.label}`)
    assert(task.binary?.kind && Array.isArray(task.binary.binaryNames), `missing binary metadata for ${task.label}`)
    assert(Array.isArray(task.cases) && task.cases.length > 0, `missing oracle cases for ${task.label}`)
    assert(Array.isArray(task.evidence) && task.evidence.length >= 2, `missing evidence URLs for ${task.label}`)
    const matrix = oracleMatrixForTask(task, oracleMatrixConfig)
    assert(matrix.failToPass.length > 0, `missing fail-to-pass matrix for ${task.label}`)
    assert(matrix.passToPass.length > 0, `missing pass-to-pass matrix for ${task.label}`)
    assertNoIssueReferenceLeak(promptForTask(task), task, `${task.label} prompt`)
    assertNoIssueReferenceLeak(JSON.stringify(redactedTask(task)), task, `${task.label} redacted task`)
  }
  validateOracleMatrix(config.tasks, oracleMatrixConfig)
  return {
    schema: config.schema,
    task_count: config.tasks.length,
    language_counts: countBy(config.tasks, "sourceLanguage"),
    labels: config.tasks.map((task) => task.label),
    oracle_matrix: {
      schema: oracleMatrixConfig.schema,
      fail_to_pass_cases: sum(config.tasks, (task) => oracleMatrixForTask(task, oracleMatrixConfig).failToPass.length),
      pass_to_pass_cases: sum(config.tasks, (task) => oracleMatrixForTask(task, oracleMatrixConfig).passToPass.length),
    },
  }
}

function isSha(value) {
  return /^[0-9a-f]{40}$/i.test(String(value || ""))
}

function countBy(rows, key) {
  return rows.reduce((counts, row) => {
    const value = String(row[key])
    counts[value] = (counts[value] || 0) + 1
    return counts
  }, {})
}

function sum(rows, valueFor) {
  return rows.reduce((total, row) => total + Number(valueFor(row) || 0), 0)
}

function issueReference(task) {
  const issueUrl = String(task.bug?.issue || "")
  const match = issueUrl.match(/^https:\/\/github\.com\/([^/]+)\/([^/]+)\/issues\/(\d+)$/)
  assert(match, `issue must be a GitHub issue URL for ${task.label}`)
  return {
    provider: "github",
    owner: match[1],
    repo: match[2],
    number: Number(match[3]),
    url: issueUrl,
  }
}

function extractIssueTextForPrompt(task) {
  const issueText = task.bug?.issueText
  assert(issueText && typeof issueText === "object", `missing issueText metadata for ${task.label}`)
  const parts = []
  if (issueText.title) parts.push(`Title: ${issueText.title}`)
  if (typeof issueText.body === "string") {
    parts.push(issueText.body)
  } else if (Array.isArray(issueText.body)) {
    parts.push(...issueText.body)
  }
  const rendered = parts.map((part) => String(part).trim()).filter(Boolean).join("\n\n")
  assert(rendered, `empty issueText metadata for ${task.label}`)
  assertNoIssueReferenceLeak(rendered, task, `${task.label} issueText`)
  return rendered
}

function assertNoIssueReferenceLeak(text, task, label) {
  const issueRef = issueReference(task)
  assert(!text.includes(issueRef.url), `${label} leaks issue URL`)
  assert(!text.includes(`/issues/${issueRef.number}`), `${label} leaks issue URL path`)
  assert(!text.includes(`#${issueRef.number}`), `${label} leaks issue number`)
}

function promptForTask(task) {
  const issueText = extractIssueTextForPrompt(task)
  return `You are working on a versioned CLI bug-fix benchmark.

Project: ${task.label}
Repository: ${task.repo.url}
Language: ${task.sourceLanguage}
Buggy version/ref: ${task.bug.buggyVersion} (${task.bug.buggyRef})
Buggy checkout commit: ${task.bug.buggyCommit}

Issue report text:
${issueText}

Bug:
${task.bug.summary}

Goal:
${task.bug.agentInstruction}

Rules:
- Do not search the internet.
- Do not use browser tools, web search, package registry pages, GitHub pages, issue pages, pull request pages, compare views, release pages, or commit pages.
- Do not run git fetch, git pull, git clone, git remote update, git submodule update, or any command that downloads upstream repository history.
- Do not inspect, checkout, merge, cherry-pick, diff, or copy any commit, tag, branch, release archive, source package, or generated file newer than buggy commit ${task.bug.buggyCommit}.
- Do not inspect fixed-version source, fixed binaries, fixed release artifacts, hidden verifier files, oracle fixtures, or reference outputs.
- Do not use the upstream issue, fixing pull request, changelog, or commit history beyond the issue report text included in this prompt.
- Repair the source in this workspace, keeping the change focused on the described bug.
- The benchmark oracle is not the repository test suite. The harness will build your patched CLI and run the real reproducer inputs, then compare observable CLI status/stdout/stderr/side effects with the fixed release binary.
- You may run local focused commands, but passing local tests alone is not enough.
- Allowed git usage is limited to local workspace inspection such as git status and git diff of your own changes.
`
}

function writePlan(tasks) {
  fs.mkdirSync(runPaths.run_root, { recursive: true })
  const taskPlans = tasks.map((task) => {
    const taskDir = path.join(runPaths.run_root, "tasks", safeName(task.id))
    fs.mkdirSync(taskDir, { recursive: true })
    const promptPath = path.join(taskDir, "prompt.md")
    const taskPath = path.join(taskDir, "task.json")
    const redacted = redactedTask(task)
    fs.writeFileSync(promptPath, promptForTask(task), "utf8")
    fs.writeFileSync(taskPath, `${JSON.stringify(redacted, null, 2)}\n`, "utf8")
    return {
      task_id: task.id,
      label: task.label,
      prompt_path: promptPath,
      task_path: taskPath,
      prompt_policy_version: promptPolicyVersion,
      buggy_version: task.bug.buggyVersion,
      buggy_ref: task.bug.buggyRef,
      buggy_commit: task.bug.buggyCommit,
      fixed_oracle_hidden_from_agent: {
        version: task.bug.fixedVersion,
        ref: task.bug.fixedRef,
        commit: task.bug.fixedCommit,
      },
      oracle_cases: task.cases.map((item) => item.name),
      oracle_matrix_counts: {
        fail_to_pass: oracleMatrixForTask(task, oracleMatrixConfig).failToPass.length,
        pass_to_pass: oracleMatrixForTask(task, oracleMatrixConfig).passToPass.length,
      },
    }
  })
  const harnessMetadata = writeHarnessMetadata(tasks)
  const plan = {
    schema: "tura.debug.cli-bugfix-binary-plan.v1",
    run_id: runId,
    run_root: runPaths.run_root,
    prompt_policy_version: promptPolicyVersion,
    harness_metadata_version: harnessMetadataVersion,
    harness_metadata_path: harnessMetadata.path,
    selected_task_ids: tasks.map((task) => task.id),
    tasks: taskPlans,
    note: "This directory defines the binary-verifiable task matrix and agent prompts. Fixed binaries and expected outputs are harness-only oracle material and are not written into task prompts.",
  }
  fs.writeFileSync(path.join(runPaths.run_root, "plan.json"), `${JSON.stringify(plan, null, 2)}\n`, "utf8")
  return plan
}

function writeHarnessMetadata(tasks) {
  const metadataPath = path.join(runPaths.run_root, "harness-metadata.json")
  const metadata = {
    schema: "tura.debug.cli-bugfix-binary-harness-metadata.v3",
    metadata_version: harnessMetadataVersion,
    run_id: runId,
    oracle_policy: {
      source: "fixed-release-binary",
      repo_tests_are_oracle: false,
      preflight: [
        "Run each reproducer against the buggy release binary and verify it exhibits the configured buggy behavior.",
        "Run the same reproducer against the fixed release binary and capture status/stdout/stderr/side effects as the oracle.",
        "Build the candidate from the agent workspace and compare only observable CLI behavior against the fixed binary oracle.",
      ],
    },
    tasks: tasks.map((task) => ({
      task_id: task.id,
      label: task.label,
      repo: task.repo,
      source_language: task.sourceLanguage,
      issue: {
        ...issueReference(task),
        title: task.bug.issueTitle,
        excerpt: task.bug.issueExcerpt,
        prompt_text: extractIssueTextForPrompt(task),
        text_source: task.bug.issueText.source || "task-metadata",
        fix: task.bug.fix,
      },
      buggy: {
        version: task.bug.buggyVersion,
        ref: task.bug.buggyRef,
        commit: task.bug.buggyCommit,
      },
      fixed: {
        version: task.bug.fixedVersion,
        ref: task.bug.fixedRef,
        commit: task.bug.fixedCommit,
      },
      binary: task.binary,
      build: task.build,
      oracle_cases: task.cases,
      oracle_matrix: {
        policy: oracleMatrixConfig.matrixPolicy,
        ...oracleMatrixForTask(task, oracleMatrixConfig),
      },
    })),
  }
  fs.writeFileSync(metadataPath, `${JSON.stringify(metadata, null, 2)}\n`, "utf8")
  return { path: metadataPath, metadata }
}

function redactedTask(task) {
  return {
    id: task.id,
    label: task.label,
    sourceLanguage: task.sourceLanguage,
    repo: task.repo,
    bug: {
      summary: task.bug.summary,
      issueText: extractIssueTextForPrompt(task),
      buggyVersion: task.bug.buggyVersion,
      buggyRef: task.bug.buggyRef,
      buggyCommit: task.bug.buggyCommit,
      agentInstruction: task.bug.agentInstruction,
    },
    promptPolicyVersion,
    build: task.build,
    cases: task.cases.map((item) => ({
      name: item.name,
      buggyBehavior: item.buggyBehavior,
      fixedBehavior: item.fixedBehavior,
      compare: item.compare,
    })),
    oracleMatrix: {
      failToPass: oracleMatrixForTask(task, oracleMatrixConfig).failToPass.map((item) => ({
        name: item.name,
        commandGroup: item.commandGroup || null,
        compare: item.compare,
      })),
      passToPass: oracleMatrixForTask(task, oracleMatrixConfig).passToPass.map((item) => ({
        name: item.name,
        commandGroup: item.commandGroup || null,
        compare: item.compare,
      })),
    },
  }
}

async function runAgentMatrix(tasks) {
  ensureGenericAgentExecutables(agents, { repoRoot: path.resolve(scriptDir, "..", "..", "..", "..") })
  const matrix = buildMatrix(tasks, agentRuns)
  const progress = new Map(matrix.map((job) => [jobKey(job), {
    task: job.task.id,
    label: job.task.label,
    agent: job.agentRun.run_id,
    agent_id: job.agentRun.agent_id,
    phase: "pending",
    in_progress: false,
  }]))
  const writeProgress = () => writeAgentProgress([...progress.values()])
  writeProgress()
  const prepared = await mapWithConcurrency(matrix, workspacePrepConcurrency, async (job, index) => {
    const prepared = prepareAgentJob(job, index)
    progress.set(jobKey(job), prepareProgressResult(job, prepared))
    writeProgress()
    return prepared
  })
  const preparedByJob = new Map(matrix.map((job, index) => [jobKey(job), prepared[index]]))
  return mapWithConcurrency(matrix, routeConcurrency, async (job) => {
    const result = await runAgentOnTask(job, preparedByJob.get(jobKey(job)), (stats) => {
      progress.set(jobKey(job), stats)
      writeProgress()
    })
    progress.set(jobKey(job), result)
    writeProgress()
    return result
  })
}

function prepareAgentJob(job, index) {
  const taskDir = path.join(runPaths.run_root, "agent-runs", safeName(job.task.id), safeName(job.agentRun.run_id))
  const workspace = path.join(taskDir, "workspace")
  const prompt = promptForTask(job.task)
  let error = null
  fs.rmSync(taskDir, { recursive: true, force: true })
  fs.mkdirSync(taskDir, { recursive: true })
  try {
    exportBuggyWorkspace(job.task, workspace)
  } catch (err) {
    error = String(err?.stack || err?.message || err)
  }
  fs.writeFileSync(path.join(taskDir, "prompt.md"), prompt, "utf8")
  fs.writeFileSync(path.join(taskDir, "task.json"), `${JSON.stringify(redactedTask(job.task), null, 2)}\n`, "utf8")
  return {
    index,
    agentDir: taskDir,
    workspace,
    prompt,
    prompt_path: path.join(taskDir, "prompt.md"),
    error,
  }
}

function jobKey(job) {
  return `${job.task.id}:${job.agentRun.run_id}`
}

async function runAgentOnTask(job, prepared, onUpdate) {
  const started = performance.now()
  const agentId = job.agentRun.agent_id
  const base = {
    task: job.task.id,
    task_id: job.task.id,
    label: job.task.label,
    agent: job.agentRun.run_id,
    agent_id: agentId,
    agent_kind: genericAgentKind(agentId),
    agent_mode: genericAgentMode(agentId),
    model: modelForGenericAgent(agentId, { model, turaModel }),
    tura_model: genericAgentKind(agentId) === "tura" ? turaModel : null,
    reasoning,
    service_tier: serviceTier,
    priority_enabled: priorityEnabled(serviceTier),
    workspace: prepared.workspace,
    prompt_path: prepared.prompt_path,
    stdout_path: path.join(prepared.agentDir, "stdout.jsonl"),
    stderr_path: path.join(prepared.agentDir, "stderr.log"),
    provider_log_path: path.join(prepared.agentDir, "provider-log"),
  }
  const writeSummary = (stats) => {
    const merged = { ...base, ...stats, elapsed_ms: Math.round(performance.now() - started) }
    fs.writeFileSync(path.join(prepared.agentDir, "agent-summary.json"), `${JSON.stringify(merged, null, 2)}\n`, "utf8")
    onUpdate?.(merged)
    return merged
  }
  writeSummary({ phase: "agent_started", in_progress: true, exit_code: null, error: prepared.error })
  let liveResult
  if (prepared.error) {
    liveResult = {
      status: 1,
      signal: null,
      stdout: "",
      stderr: "",
      error: prepared.error,
      usage_info: usageForAgent(prepared.agentDir, "", agentId),
      events: eventsWithUsageRounds(eventsForAgent("", agentId), usageForAgent(prepared.agentDir, "", agentId).usage),
    }
  } else {
    try {
      liveResult = await runGenericAgentCli({
        agentId,
        workspace: prepared.workspace,
        agentDir: prepared.agentDir,
        prompt: prepared.prompt,
        repoRoot: path.resolve(scriptDir, "..", "..", "..", ".."),
        model,
        turaModel,
        reasoning,
        serviceTier,
        timeoutMs,
        onProgress: throttle((live) => {
          const usageInfo = usageForAgent(prepared.agentDir, live.stdout || "", agentId)
          writeSummary({
            phase: "agent_running",
            in_progress: true,
            exit_code: live.status,
            signal: live.signal || null,
            error: live.error || null,
            first_output_ms: live.first_output_ms,
            last_progress_ms: live.last_progress_ms,
            usage: usageInfo.usage,
            usage_source: usageInfo.usage_source,
            provider_calls: usageInfo.provider_calls,
            events: eventsWithUsageRounds(eventsForAgent(live.stdout || "", agentId), usageInfo.usage),
          })
        }, 10_000),
      })
    } catch (err) {
      const error = String(err?.stack || err?.message || err)
      liveResult = {
        status: 1,
        signal: null,
        stdout: "",
        stderr: "",
        error,
        usage_info: usageForAgent(prepared.agentDir, "", agentId),
        events: eventsWithUsageRounds(eventsForAgent("", agentId), usageForAgent(prepared.agentDir, "", agentId).usage),
      }
    }
  }
  const patch = collectPatch(prepared.workspace, prepared.agentDir)
  const usageInfo = usageForAgent(prepared.agentDir, liveResult.stdout || "", agentId)
  return writeSummary({
    phase: "agent_completed",
    in_progress: false,
    exit_code: liveResult.status,
    signal: liveResult.signal || null,
    error: liveResult.error || null,
    first_output_ms: liveResult.first_output_ms || null,
    last_progress_ms: liveResult.last_progress_ms || null,
    usage: usageInfo.usage,
    usage_source: usageInfo.usage_source,
    provider_calls: usageInfo.provider_calls,
    events: eventsWithUsageRounds(eventsForAgent(liveResult.stdout || "", agentId), usageInfo.usage),
    context_archive: liveResult.context_archive || null,
    patch,
  })
}

function prepareProgressResult(job, prepared) {
  return {
    task: job.task.id,
    task_id: job.task.id,
    label: job.task.label,
    agent: job.agentRun.run_id,
    agent_id: job.agentRun.agent_id,
    agent_kind: genericAgentKind(job.agentRun.agent_id),
    agent_mode: genericAgentMode(job.agentRun.agent_id),
    phase: prepared.error ? "prepare_failed" : "prepared",
    in_progress: false,
    workspace: prepared.workspace,
    prompt_path: prepared.prompt_path,
    error: prepared.error,
  }
}

function writeAgentProgress(results) {
  const completed = results.filter((item) => item.phase === "agent_completed" || item.phase === "prepare_failed").length
  const running = results.filter((item) => item.in_progress).length
  const payload = {
    schema: "tura.debug.cli-bugfix-agent-progress.v1",
    run_id: runId,
    updated_at: new Date().toISOString(),
    total: results.length,
    completed,
    running,
    pending: Math.max(0, results.length - completed - running),
    results,
  }
  fs.mkdirSync(runPaths.run_root, { recursive: true })
  fs.writeFileSync(path.join(runPaths.run_root, "agent-progress.json"), `${JSON.stringify(payload, null, 2)}\n`, "utf8")
}

function exportBuggyWorkspace(task, workspace) {
  const cacheRoot = path.join(path.dirname(runPaths.run_root), "_source-cache")
  const repoCache = path.join(cacheRoot, "repos", `${safeName(task.repo.owner)}-${safeName(task.repo.name)}`)
  const archiveDir = path.join(cacheRoot, "archives")
  const archivePath = path.join(archiveDir, `${safeName(task.id)}-${task.bug.buggyCommit}.tar`)
  fs.mkdirSync(path.dirname(repoCache), { recursive: true })
  fs.mkdirSync(archiveDir, { recursive: true })
  if (!fs.existsSync(path.join(repoCache, ".git"))) {
    runOk("git", ["clone", "--no-checkout", "--filter=blob:none", task.repo.url, repoCache], { timeoutMs: 30 * 60_000 })
  } else {
    runOk("git", ["-C", repoCache, "remote", "set-url", "origin", task.repo.url], { timeoutMs: 60_000 })
  }
  if (runCommand("git", ["-C", repoCache, "cat-file", "-e", `${task.bug.buggyCommit}^{commit}`], { timeoutMs: 60_000 }).status !== 0) {
    runOk("git", ["-C", repoCache, "fetch", "--tags", "origin"], { timeoutMs: 30 * 60_000 })
  }
  if (!fs.existsSync(archivePath)) {
    runOk("git", ["-C", repoCache, "archive", "--format=tar", "-o", archivePath, task.bug.buggyCommit], { timeoutMs: 10 * 60_000 })
  }
  fs.rmSync(workspace, { recursive: true, force: true })
  fs.mkdirSync(workspace, { recursive: true })
  runOk("tar", ["-xf", archivePath, "-C", workspace], { timeoutMs: 10 * 60_000 })
  runOk("git", ["init"], { cwd: workspace, timeoutMs: 60_000 })
  runOk("git", ["config", "user.email", "benchmark@example.invalid"], { cwd: workspace, timeoutMs: 60_000 })
  runOk("git", ["config", "user.name", "Benchmark"], { cwd: workspace, timeoutMs: 60_000 })
  runOkWithRetry("git", ["add", "-A"], { cwd: workspace, timeoutMs: 10 * 60_000, attempts: 5 })
  runOkWithRetry("git", ["commit", "-m", `baseline ${task.bug.buggyCommit}`], { cwd: workspace, timeoutMs: 10 * 60_000, attempts: 3 })
}

function collectPatch(workspace, agentDir) {
  const diff = runCommand("git", ["diff", "--binary", "HEAD"], { cwd: workspace, timeoutMs: 60_000 })
  const status = runCommand("git", ["status", "--short"], { cwd: workspace, timeoutMs: 60_000 })
  const patchPath = path.join(agentDir, "agent.patch")
  const statusPath = path.join(agentDir, "git-status.txt")
  fs.writeFileSync(patchPath, diff.stdout || "", "utf8")
  fs.writeFileSync(statusPath, status.stdout || "", "utf8")
  return {
    patch_path: patchPath,
    status_path: statusPath,
    changed: Boolean(String(diff.stdout || "").trim() || String(status.stdout || "").trim()),
    status: status.stdout || "",
  }
}

function runCommand(command, args = [], options = {}) {
  const result = spawnSync(command, args.map(String), {
    cwd: options.cwd || process.cwd(),
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 30_000,
    maxBuffer: 256 * 1024 * 1024,
    windowsHide: true,
  })
  return {
    status: result.status === null ? 124 : result.status,
    stdout: result.stdout || "",
    stderr: (result.stderr || "") + (result.error ? `\n${result.error.message || result.error}` : ""),
    signal: result.signal || null,
  }
}

function runOk(command, args = [], options = {}) {
  const result = runCommand(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`)
  }
  return result
}

function runOkWithRetry(command, args = [], options = {}) {
  const attempts = Math.max(1, Number(options.attempts || 1))
  let last
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    last = runCommand(command, args, options)
    if (last.status === 0) return last
    if (attempt < attempts) {
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, Math.min(5000, 250 * attempt * attempt))
    }
  }
  throw new Error(`${command} ${args.join(" ")} failed with ${last.status} after ${attempts} attempts\nSTDOUT:\n${last.stdout}\nSTDERR:\n${last.stderr}`)
}

function throttle(fn, intervalMs) {
  let last = 0
  return (...args) => {
    const now = Date.now()
    if (now - last < intervalMs) return
    last = now
    return fn(...args)
  }
}

async function main() {
  const validation = validateTaskConfig(taskConfig)
  if (selfTest) {
    const summary = normalizeBusinessSummary({ ok: true, self_test: true, validation }, runPaths)
    fs.mkdirSync(runPaths.run_root, { recursive: true })
    fs.writeFileSync(runPaths.summary_path, `${JSON.stringify(summary, null, 2)}\n`, "utf8")
    console.log(JSON.stringify(summary, null, 2))
    return
  }

  const plan = writePlan(selectedTasks)
  const agentResults = runAgents && !harnessOnly ? await runAgentMatrix(selectedTasks) : []
  const audit = binaryAudit ? await runBinaryAudit({
    tasks: selectedTasks,
    matrixConfig: oracleMatrixConfig,
    runRoot: runPaths.run_root,
  }) : null
  const summary = normalizeBusinessSummary({
    ok: agentResults.every((result) => Number(result.exit_code || 0) === 0),
    harness_only: harnessOnly,
    agent_phase: runAgents,
    binary_audit: binaryAudit,
    selected_task_ids: selectedTasks.map((task) => task.id),
    agents: runAgents ? agents : [],
    validation,
    plan,
    agent_results: agentResults,
    harness: {
      ran: binaryAudit,
      metadata_version: harnessMetadataVersion,
      metadata_path: plan.harness_metadata_path,
      binary_audit_path: binaryAudit ? path.join(runPaths.run_root, "binary-audit", "binary-audit.json") : null,
      audit,
      reason: binaryAudit
        ? "Downloaded buggy/fixed release binaries, audited CLI entrypoints, and ran f2p/p2p oracle preflight."
        : "The task matrix and prompt plan are materialized here; set COMMAND_RUN_AGENT_BINARY_AUDIT=1 to download release binaries and run f2p/p2p oracle preflight.",
    },
  }, runPaths)
  fs.writeFileSync(runPaths.summary_path, `${JSON.stringify(summary, null, 2)}\n`, "utf8")
  console.log(JSON.stringify(summary, null, 2))
}

await main()
