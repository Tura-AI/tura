import fs from "node:fs"
import path from "node:path"
import process from "node:process"

const DEFAULT_WORKSPACE_NAME = "tura_workspace"
const DOCUMENTS_DIRECTORY_NAMES = ["Documents", "文档"]

export function userHome() {
  return process.env.USERPROFILE || process.env.HOME || ""
}

export function defaultUserWorkspace() {
  const home = userHome()
  if (!home) return process.cwd()
  return path.join(documentsDirectory(home), DEFAULT_WORKSPACE_NAME)
}

function documentsDirectory(home) {
  const leaf = home.replace(/[\\/]+$/, "").split(/[\\/]/).at(-1)
  if (leaf && DOCUMENTS_DIRECTORY_NAMES.some((name) => name.toLowerCase() === leaf.toLowerCase())) {
    return home
  }
  return (
    DOCUMENTS_DIRECTORY_NAMES.map((name) => path.join(home, name)).find((candidate) =>
      fs.existsSync(candidate),
    ) || path.join(home, DOCUMENTS_DIRECTORY_NAMES[0])
  )
}

export function businessTargetRoot() {
  return (
    process.env.TURA_BUSINESS_TARGET_ROOT ||
    process.env.COMMAND_RUN_BUSINESS_TARGET_ROOT ||
    path.join(defaultUserWorkspace(), "target")
  )
}

export function businessRunPaths(testName, runId, options = {}) {
  const targetRoot = options.targetRoot || businessTargetRoot()
  const runRoot = options.runRoot || path.join(targetRoot, testName, String(runId))
  return {
    test_name: testName,
    run_id: String(runId),
    user_workspace: defaultUserWorkspace(),
    target_root: targetRoot,
    run_root: runRoot,
    summary_path: path.join(runRoot, "summary.json"),
  }
}

export function normalizeBusinessSummary(summary, paths, extras = {}) {
  const merged = { ...summary, ...extras }
  return {
    ...merged,
    schema: "tura.business-test.summary.v1",
    test_name: paths.test_name,
    run_id: paths.run_id,
    user_workspace: paths.user_workspace,
    target_root: paths.target_root,
    run_root: paths.run_root,
    summary_path: paths.summary_path,
    ok: Boolean(summary?.ok),
    standard_metrics: {
      duration_ms: firstFiniteNumber(
        merged.duration_ms,
        merged.runtime_duration_ms,
        merged.elapsed_ms,
        sumResultField(merged.results, "duration_ms"),
        sumResultField(merged.results, "elapsed_ms"),
      ),
      timeout_ms: firstFiniteNumber(merged.timeout_ms, merged.runtime_timeout_ms),
      token_usage:
        merged.aggregate_usage ||
        merged.token_totals ||
        merged.by_agent_tokens ||
        merged.observations?.aggregate_llm ||
        null,
      time_windows: {
        first_round_timeout_ms: firstFiniteNumber(merged.first_round_timeout_ms),
        timeout_ms: firstFiniteNumber(merged.timeout_ms, merged.runtime_timeout_ms),
      },
      harness: merged.harness || merged.eval || merged.harness_plan || null,
      scores: merged.comparison || merged.validation || merged.observations || null,
    },
  }
}

function firstFiniteNumber(...values) {
  for (const value of values) {
    const number = Number(value)
    if (Number.isFinite(number)) return number
  }
  return null
}

function sumResultField(results, field) {
  if (!Array.isArray(results)) return null
  let seen = false
  let total = 0
  for (const result of results) {
    const value = Number(result?.[field])
    if (Number.isFinite(value)) {
      seen = true
      total += value
    }
  }
  return seen ? total : null
}
