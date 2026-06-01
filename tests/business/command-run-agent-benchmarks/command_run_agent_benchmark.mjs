#!/usr/bin/env node
import fs from "node:fs/promises"
import { createWriteStream, existsSync, readFileSync, writeFileSync } from "node:fs"
import { spawn } from "node:child_process"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { endStream, isolatedProcessOptions, killProcessTree } from "./process_helpers.mjs"

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `${Date.now()}`
const runRoot =
  process.env.COMMAND_RUN_AGENT_RUN_ROOT ||
  path.join(repoRoot, "target", "command-run-codex-two-way", runId)
const summaryPath =
  process.env.COMMAND_RUN_AGENT_SUMMARY ||
  path.join(repoRoot, "target", "codex-logs", `command-run-codex-two-way-${runId}.json`)
const codexModel = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.4"
const turaModel =
  process.env.COMMAND_RUN_AGENT_TURA_MODEL ||
  (codexModel.includes("/") ? codexModel : `openai/${codexModel}`)
const reasoningEffort = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const codexServiceTier = process.env.COMMAND_RUN_AGENT_CODEX_SERVICE_TIER || "auto"
const turaAccelerationEnabled =
  (process.env.COMMAND_RUN_AGENT_TURA_PRIORITY ||
    (codexServiceTier === "priority" ? "1" : "0")) === "1"
const runtimeTimeoutMs = Math.min(numberEnv("COMMAND_RUN_AGENT_TIMEOUT_MS", 15 * 60_000), 15 * 60_000)
const startupTimeoutMs = numberEnv("COMMAND_RUN_AGENT_STARTUP_TIMEOUT_MS", 180_000)
const firstRoundTimeoutMs = numberEnv("COMMAND_RUN_AGENT_FIRST_ROUND_TIMEOUT_MS", 45_000)
const precompileTura = (process.env.COMMAND_RUN_AGENT_PRECOMPILE_TURA || "0") === "1"
const robustnessPreflight = (process.env.COMMAND_RUN_AGENT_ROBUSTNESS_PREFLIGHT || "1") === "1"
const preflightOnly = (process.env.COMMAND_RUN_AGENT_PREFLIGHT_ONLY || "0") === "1"
const skipStaleProcessCleanup = (process.env.COMMAND_RUN_AGENT_SKIP_STALE_PROCESS_CLEANUP || "0") === "1"
const finalDigestEnabled = (process.env.COMMAND_RUN_AGENT_FINAL_DIGEST || "0") === "1"
const finalDigestTimeoutMs = numberEnv("COMMAND_RUN_AGENT_FINAL_DIGEST_TIMEOUT_MS", 20_000)
const requestedAgents = parseAgentList(process.env.COMMAND_RUN_AGENT_AGENTS || "current-bash,current-shll,tura-bash,tura-shll")
const turaRoot = process.env.COMMAND_RUN_AGENT_TURA_ROOT || repoRoot
const codexCurrentRoot = process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex")
const codexMainRoot = process.env.COMMAND_RUN_AGENT_CODEX_MAIN_ROOT || path.join(homeDir, "Documents", "codex-main")
const codexMainFallbackRoot = process.env.COMMAND_RUN_AGENT_CODEX_MAIN_FALLBACK_ROOT || path.join(homeDir, "codex-main")
const compactStressEnabled = (process.env.COMMAND_RUN_AGENT_COMPACT_STRESS || "0") === "1"
const contextFullModeEnabled = (process.env.COMMAND_RUN_AGENT_CONTEXT_FULL || "0") === "1"
const enterpriseExpansionEnabled =
  (process.env.COMMAND_RUN_AGENT_ENTERPRISE_EXPANSION || (compactStressEnabled ? "1" : "0")) === "1"
const hardEnterpriseExpansionEnabled =
  (process.env.COMMAND_RUN_AGENT_HARD_ENTERPRISE_EXPANSION || (enterpriseExpansionEnabled ? "1" : "0")) === "1"
const hardActiveGeneratedCodeEnabled =
  (process.env.COMMAND_RUN_AGENT_HARD_ACTIVE_GENERATED_CODE || (hardEnterpriseExpansionEnabled ? "1" : "0")) === "1"
const hardActiveScaleMultiplier = hardActiveGeneratedCodeEnabled
  ? numberEnv("COMMAND_RUN_AGENT_HARD_ACTIVE_SCALE_MULTIPLIER", 10)
  : 1
const fixtureScaleMultiplier = numberEnv("COMMAND_RUN_AGENT_FIXTURE_SCALE_MULTIPLIER", compactStressEnabled ? 3 : 1)
const fixedContextTokenBudget = contextFullModeEnabled
  ? numberEnv("COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS", 230_000)
  : 0
const seededBehaviorDefectCount = 120 * fixtureScaleMultiplier

const taskPrompt = taskPromptForShell("shell_command")

function taskPromptForShell(shellSurface) {
  const verifyCommand =
    shellSurface === "bash"
      ? "bash tools/verify.sh"
      : "powershell -NoProfile -ExecutionPolicy Bypass -File tools/verify.ps1"
  const prompt = [
  "You are running an E2E bug-fix benchmark.",
  "",
  "Repository task:",
  "- Fix the full-stack retail operations implementation.",
  "- Do not edit tests to weaken or remove assertions.",
    `- Run \`${verifyCommand}\` until it passes.`,
  "- The bugs are cross-file and some are behind high-level workflows. Follow imports, failing behavior, and data flow instead of assuming failures are local to the first traceback.",
  `- This is intentionally a large full-stack task with at least ${100 * fixtureScaleMultiplier} behavioral defects across backend Python and frontend JavaScript in a ${150 * fixtureScaleMultiplier}+ file, ${30_000 * fixtureScaleMultiplier}+ line repository. Fix behavior, not only the first failing assertion.`,
  ...(enterpriseExpansionEnabled
    ? [
        "- In addition to fixing existing defects, implement the enterprise expansion requirements described by the new acceptance tests and docs. These require new backend and frontend behavior, not just bug fixes.",
      ]
    : []),
  ...(hardEnterpriseExpansionEnabled
    ? [
        "- The hard enterprise acceptance surface includes advanced forecasting, privacy, tax, fulfillment, fraud, SLA, executive brief, operations-console, control-plane, and control-tower requirements. Treat these as production features with exact behavior covered by tests.",
      ]
    : []),
  ...(hardActiveGeneratedCodeEnabled
    ? [
        `- The generated support, integration, policy, frontend view, and frontend shared modules are active business code in this benchmark at ${hardActiveScaleMultiplier}x scale. Their behavior is verified by matrix tests and must be repaired where failing.`,
      ]
    : []),
  "- Keep public APIs stable.",
  "- Finish only after the verification script passes, then summarize the fix.",
  ].join("\n")
  const fixedContext = fixedContextTokenBudget > 0 ? fixedRepoContextBlock(fixedContextTokenBudget) : ""
  return fixedContext ? `${fixedContext}\n\n${prompt}` : prompt
}

function fixedRepoContextBlock(targetTokens) {
  const targetChars = Math.min(Math.round(Math.max(0, targetTokens) * 4.8), 1_000_000)
  if (targetChars <= 0) return ""
  const header = [
    "<SIMULATED_PRIOR_SESSION_CONTEXT>",
    "This deterministic block simulates a long prior conversation about this repository. It includes user/assistant exchanges, tool-call result excerpts, file map notes, verification expectations, and live-repo background. Every tested agent receives the exact same block only when context-full mode is enabled.",
    "Use this as background only. The live workspace remains authoritative. The actual task prompt is below this simulated context.",
  ].join("\n")
  const chunkLines = []
  for (let index = 0; index < 80; index += 1) {
    const file = `src/retail_core/generated/domain_${String(index).padStart(3, "0")}/workflow_${String(index % 17).padStart(2, "0")}.py`
    chunkLines.push(
      `file=${file} responsibility=retail account inventory checkout refunds reporting customer policy fulfillment adapter rule=${index} expected=business-invariant-preserved live-workspace-authoritative`,
    )
    chunkLines.push(
      `tests/generated/test_domain_${String(index).padStart(3, "0")}.py validates decimal money normalization, SKU case folding, month end reconciliation, idempotent exports, stable public APIs, and narrow verification before final summary.`,
    )
    chunkLines.push(
      `debug-note-${index}: failing behavior may be cross-file; prefer rg, directory scan, targeted file reads, apply_patch, then tools/verify.ps1 or tools/verify.sh depending on shell surface.`,
    )
    chunkLines.push(
      `assistant-tool-result-${index}: {"results":[{"step":1,"command":"shell_command","success":true,"output":"${file}\\nclass SimulatedWorkflow${index}: pass\\n# generated repo context excerpt ${index}"}]}`,
    )
    chunkLines.push(
      `user-followup-${index}: keep public APIs stable, preserve Decimal money behavior, avoid weakening tests, and finish only after the repository verification command passes.`,
    )
  }
  const chunk = `${chunkLines.join("\n")}\n`
  let body = ""
  while (header.length + body.length < targetChars) body += chunk
  return `${header}\n${body.slice(0, Math.max(0, targetChars - header.length))}\n</SIMULATED_PRIOR_SESSION_CONTEXT>`
}

function numberEnv(name, fallback) {
  const value = Number(process.env[name])
  return Number.isFinite(value) && value > 0 ? value : fallback
}

function parseAgentList(value) {
  const aliases = new Map([
    ["tura", "tura"],
    ["tura-auto", "tura"],
    ["tura_local", "tura"],
    ["tura-local", "tura"],
    ["tura-shell", "tura-shll"],
    ["tura_shell", "tura-shll"],
    ["tura-shll", "tura-shll"],
    ["tura_shll", "tura-shll"],
    ["tura-shall", "tura-shll"],
    ["tura-multiple-tasks", "tura-multiple-tasks-shll"],
    ["tura_multiple_tasks", "tura-multiple-tasks-shll"],
    ["tura-multiple-tasks-shll", "tura-multiple-tasks-shll"],
    ["tura_multiple_tasks_shll", "tura-multiple-tasks-shll"],
    ["tura-shll-multiple-tasks", "tura-multiple-tasks-shll"],
    ["tura_shll_multiple_tasks", "tura-multiple-tasks-shll"],
    ["tura-fast-multiple-tasks", "tura-fast-multiple-tasks-shll"],
    ["tura_fast_multiple_tasks", "tura-fast-multiple-tasks-shll"],
    ["tura-fast-multiple-tasks-shll", "tura-fast-multiple-tasks-shll"],
    ["tura_fast_multiple_tasks_shll", "tura-fast-multiple-tasks-shll"],
    ["tura-fast-shll-multiple-tasks", "tura-fast-multiple-tasks-shll"],
    ["tura_fast_shll_multiple_tasks", "tura-fast-multiple-tasks-shll"],
    ["tura-bash", "tura-bash"],
    ["tura_bash", "tura-bash"],
    ["tura-bash-nonstrict", "tura-bash"],
    ["tura_bash_nonstrict", "tura-bash"],
    ["tura-bash-strict", "tura-bash-strict"],
    ["tura_bash_strict", "tura-bash-strict"],
    ["tura-fast", "tura-fast-shll"],
    ["tura_fast", "tura-fast-shll"],
    ["tura-fast-shll", "tura-fast-shll"],
    ["tura_fast_shll", "tura-fast-shll"],
    ["tura-simple", "tura-fast-shll"],
    ["tura_simple", "tura-fast-shll"],
    ["tura-simple-shll", "tura-fast-shll"],
    ["tura_simple_shll", "tura-fast-shll"],
    ["codex", "current-shll"],
    ["codex-current", "current-shll"],
    ["codex_current", "current-shll"],
    ["current", "current-shll"],
    ["current-shell", "current-shll"],
    ["current_shell", "current-shll"],
    ["current-shll", "current-shll"],
    ["current_shll", "current-shll"],
    ["current-shall", "current-shll"],
    ["current-bash", "current-bash"],
    ["current_bash", "current-bash"],
    ["codex-main", "codex-main"],
    ["codex_main", "codex-main"],
    ["main", "codex-main"],
  ])
  const agents = String(value || "")
    .split(",")
    .map((item) => aliases.get(item.trim().toLowerCase()))
    .filter(Boolean)
  return agents.length ? Array.from(new Set(agents)) : ["current-bash", "current-shll", "tura-bash", "tura-shll"]
}

function agentShellSurface(id) {
  return id.includes("bash") ? "bash" : "shell_command"
}

function isTuraAgent(id) {
  return id === "tura" || id.startsWith("tura-")
}

function turaCliAgentName(id) {
  return id.includes("fast") ? "coding_agent_fast" : "coding_agent"
}

function turaModelForAgent(id) {
  const envKey = `COMMAND_RUN_AGENT_TURA_MODEL_${id.toUpperCase().replace(/[^A-Z0-9]+/g, "_")}`
  return process.env[envKey] || turaModel
}

function turaStrictJsonDisabled(id) {
  return id.endsWith("-nonstrict")
}

function turaMultipleTasksMode(id) {
  return id.includes("multiple-tasks")
}

function isCurrentAgent(id) {
  return id.startsWith("current-")
}

function quote(value) {
  const text = String(value)
  return text.includes(" ") ? `"${text.replaceAll('"', '\\"')}"` : text
}

function codexBinForRoot(root) {
  return path.join(root, "codex-rs", "target", "debug", process.platform === "win32" ? "codex.exe" : "codex")
}

function codexExecBinForRoot(root) {
  return path.join(root, "codex-rs", "target", "debug", process.platform === "win32" ? "codex-exec.exe" : "codex-exec")
}

function turaBinForRoot(root) {
  return path.join(root, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")
}

function bashBinForHost() {
  if (process.platform !== "win32") return "bash"
  const candidates = [
    "C:\\Program Files\\Git\\bin\\bash.exe",
    "C:\\Program Files\\Git\\usr\\bin\\bash.exe",
    "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
  ]
  return candidates.find((candidate) => existsSync(candidate)) || "bash"
}

function envForShellSurface(shellSurface) {
  if (shellSurface !== "bash" || process.platform !== "win32") return {}
  const bashBin = bashBinForHost()
  const bashDir = path.dirname(bashBin)
  return existsSync(bashBin)
    ? { PATH: `${bashDir}${path.delimiter}${process.env.PATH || ""}` }
    : {}
}

async function writeText(file, content) {
  await fs.mkdir(path.dirname(file), { recursive: true })
  await fs.writeFile(file, content, "utf8")
}

async function timedStep(steps, name, fn) {
  const startedAt = new Date().toISOString()
  const started = performance.now()
  try {
    const result = await fn()
    steps.push({ name, status: "completed", started_at: startedAt, duration_ms: Math.round(performance.now() - started) })
    return result
  } catch (error) {
    steps.push({
      name,
      status: "failed",
      started_at: startedAt,
      duration_ms: Math.round(performance.now() - started),
      error: error.stack || error.message,
    })
    throw error
  }
}

function spawnLogged(command, args, options = {}) {
  return new Promise((resolve) => {
    const started = performance.now()
    if (options.echo !== false) console.log(`$ ${[command, ...args].map(quote).join(" ")}`)
    let settled = false
    let timedOut = false
    let childExitStatus = null
    const timeoutLimitMs = options.timeoutMs
    let closeGraceTimer = null
    let timeoutGraceTimer = null
    const child = spawn(command, args, isolatedProcessOptions({
      cwd: options.cwd,
      env: { ...process.env, ...(options.env || {}) },
      shell: options.shell || false,
      windowsHide: true,
      stdio: [options.input ? "pipe" : "ignore", "pipe", "pipe"],
    }))
    let stdout = ""
    let stderr = ""
    const stdoutStream = options.stdoutPath ? createWriteStream(options.stdoutPath, { flags: "w" }) : null
    const stderrStream = options.stderrPath ? createWriteStream(options.stderrPath, { flags: "w" }) : null
    let lastProgressWriteMs = 0
    const writeProgressSnapshot = (status = "running") => {
      if (!options.progressPath || typeof options.progressSnapshot !== "function") return
      const now = performance.now()
      if (status === "running" && now - lastProgressWriteMs < Number(options.progressIntervalMs || 5_000)) return
      lastProgressWriteMs = now
      try {
        writeFileSync(
          options.progressPath,
          JSON.stringify(options.progressSnapshot({ stdout, stderr, status, elapsed_ms: Math.round(now - started) }), null, 2),
          "utf8",
        )
      } catch {
        // Progress snapshots are best-effort diagnostics and must not change benchmark behavior.
      }
    }
    let firstOutputMs = null
    const markFirstOutput = () => {
      if (firstOutputMs === null) firstOutputMs = Math.round(performance.now() - started)
    }
    function settle(status, error = null) {
      if (settled) return
      settled = true
      if (timer) clearTimeout(timer)
      clearTimeout(closeGraceTimer)
      clearTimeout(timeoutGraceTimer)
      endStream(stdoutStream)
      endStream(stderrStream)
      writeProgressSnapshot(status === 0 ? "completed" : timedOut ? "timeout" : "failed")
      resolve({
        status: status ?? -1,
        stdout,
        stderr: error ? `${stderr}${stderr ? "\n" : ""}${error}` : stderr,
        durationMs: Math.round(performance.now() - started),
        firstOutputMs,
      })
    }

    const timer = options.timeoutMs
      ? setTimeout(() => {
          timedOut = true
          stderr += `\nTimed out after ${options.timeoutMs}ms`
          killProcessTree(child.pid)
          timeoutGraceTimer = setTimeout(() => {
            settle(childExitStatus ?? 1)
          }, Number(options.timeoutCloseGraceMs || 3_000))
        }, options.timeoutMs)
      : null
    child.stdout.on("data", (chunk) => {
      markFirstOutput()
      stdout += chunk.toString()
      if (stdoutStream) stdoutStream.write(chunk)
      if (options.stream) process.stdout.write(chunk)
      if (options.onStdout) options.onStdout(chunk.toString(), stdout, stderr)
      writeProgressSnapshot()
    })
    child.stderr.on("data", (chunk) => {
      markFirstOutput()
      stderr += chunk.toString()
      if (stderrStream) stderrStream.write(chunk)
      if (options.stream) process.stderr.write(chunk)
      if (options.onStderr) options.onStderr(chunk.toString(), stdout, stderr)
      writeProgressSnapshot()
    })
    if (options.input) {
      child.stdin.write(options.input)
      child.stdin.end()
    }
    child.on("error", (error) => {
      settle(-1, error.stack || error.message)
    })
    child.on("exit", (status, signal) => {
      childExitStatus = status
      void signal
      closeGraceTimer = setTimeout(() => {
        settle(timedOut ? (status ?? 1) : status)
      }, Number(options.exitCloseGraceMs || 1_000))
    })
    child.on("close", (status) => {
      settle(timedOut ? (status ?? 1) : status)
    })
  })
}

async function runTuraRobustnessPreflight() {
  const script = path.join(repoRoot, "scripts", "test-command-run-robustness.ps1")
  const command = process.platform === "win32" ? "powershell" : "pwsh"
  const result = await spawnLogged(
    command,
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-NoBuild"],
    {
      cwd: repoRoot,
      echo: true,
      timeoutMs: numberEnv("COMMAND_RUN_AGENT_ROBUSTNESS_TIMEOUT_MS", 180_000),
      stdoutPath: path.join(runRoot, "robustness-preflight.stdout.log"),
      stderrPath: path.join(runRoot, "robustness-preflight.stderr.log"),
    },
  )
  if (result.status !== 0) {
    throw new Error(
      `Tura command_run robustness preflight failed with status ${result.status}. See ${path.join(runRoot, "robustness-preflight.stdout.log")} and ${path.join(runRoot, "robustness-preflight.stderr.log")}`,
    )
  }
  return {
    ok: true,
    status: result.status,
    duration_ms: result.durationMs,
    first_output_ms: result.firstOutputMs,
    stdout_log: path.join(runRoot, "robustness-preflight.stdout.log"),
    stderr_log: path.join(runRoot, "robustness-preflight.stderr.log"),
  }
}

async function clearRunRoot() {
  await fs.rm(runRoot, { recursive: true, force: true })
  await fs.mkdir(runRoot, { recursive: true })
  await fs.mkdir(path.dirname(summaryPath), { recursive: true })
}

function neutralModuleContent(index) {
  const lines = [
    `"""Neutral support module ${index}.`,
    "",
    "These files simulate generated business support code in a large retail",
    "platform. In hard mode this layer is active: matrix tests import every",
    "module and verify behavior, so generated code defects must be repaired too.",
    `"""`,
    "",
    "from __future__ import annotations",
    "",
    "from dataclasses import dataclass",
    "from decimal import Decimal",
    "",
    "",
    "@dataclass(frozen=True)",
    `class SupportRecord${index}:`,
    "    key: str",
    "    amount: Decimal",
    "    active: bool = True",
    "",
    "",
    `def normalize_key_${index}(value: str) -> str:`,
    "    return value.strip().lower().replace(' ', '-')",
    "",
    "",
    `def weighted_total_${index}(records: list[SupportRecord${index}], weight: Decimal = Decimal("1.00")) -> Decimal:`,
    "    total = Decimal('0.00')",
    "    for record in records:",
    "        if record.active:",
    "            total += record.amount * weight",
    "    return total",
    "",
  ]
  for (let item = 0; item < 36; item += 1) {
    lines.push(
      "",
      `def rule_${index}_${item}(value: int) -> int:`,
      `    """Return a deterministic support score for rule ${index}.${item}."""`,
      `    base = value + ${index} + ${item}`,
      "    if base % 5 == 0:",
      "        return base // 5",
      "    if base % 3 == 0:",
      "        return base + 3",
      "    return base - 1",
    )
  }
  return `${lines.join("\n")}\n`
}

async function writeNeutralCodebase(repoPath) {
  const supportDir = path.join(repoPath, "src", "retail_core", "support")
  const integrationsDir = path.join(repoPath, "src", "retail_core", "integrations")
  const supportCount = 70 * fixtureScaleMultiplier * hardActiveScaleMultiplier
  const integrationCount = 20 * fixtureScaleMultiplier * hardActiveScaleMultiplier
  await fs.mkdir(supportDir, { recursive: true })
  await fs.mkdir(integrationsDir, { recursive: true })
  await writeText(path.join(supportDir, "__init__.py"), `"""Generated neutral support modules."""\n`)
  await writeText(path.join(integrationsDir, "__init__.py"), `"""Generated neutral integration modules."""\n`)
  for (let index = 0; index < supportCount; index += 1) {
    await writeText(path.join(supportDir, `support_${String(index).padStart(2, "0")}.py`), neutralModuleContent(index))
  }
  for (let index = 0; index < integrationCount; index += 1) {
    await writeText(path.join(integrationsDir, `adapter_${String(index).padStart(2, "0")}.py`), neutralModuleContent(index + supportCount))
  }
  for (let index = 0; index < 12 * fixtureScaleMultiplier; index += 1) {
    const supportIndex = index % supportCount
    await writeText(
      path.join(repoPath, "tests", `test_neutral_support_${String(index).padStart(2, "0")}.py`),
      `import unittest

from retail_core.support.support_${String(supportIndex).padStart(2, "0")} import rule_${supportIndex}_0


class NeutralSupport${index}Tests(unittest.TestCase):
    def test_rule_is_deterministic(self):
        self.assertEqual(rule_${supportIndex}_0(10), rule_${supportIndex}_0(10))


if __name__ == "__main__":
    unittest.main()
`,
    )
  }
  if (hardActiveGeneratedCodeEnabled) {
    await writeText(
      path.join(repoPath, "tests", "test_generated_active_support_matrix.py"),
      `from decimal import Decimal
import importlib
import unittest


SUPPORT_COUNT = ${supportCount}
INTEGRATION_COUNT = ${integrationCount}


class GeneratedActiveSupportMatrixTests(unittest.TestCase):
    def assert_active_module_contract(self, module, index):
        record_cls = getattr(module, f"SupportRecord{index}")
        self.assertEqual(getattr(module, f"normalize_key_{index}")(f" Order_ID  {index} "), f"order-id-{index}")
        records = [
            record_cls("a", Decimal("1.111"), True),
            record_cls("b", Decimal("2.222"), False),
            record_cls("c", Decimal("3.333"), True),
        ]
        self.assertEqual(getattr(module, f"weighted_total_{index}")(records, Decimal("1.50")), Decimal("6.67"))
        self.assertEqual(getattr(module, f"rule_{index}_0")(10), 10 + index + 1)
        self.assertEqual(getattr(module, f"rule_{index}_7")(10), 10 + index + 8)
        self.assertEqual(getattr(module, f"rule_{index}_19")(10), 10 + index + 20)
        self.assertEqual(getattr(module, f"rule_{index}_35")(10), 10 + index + 36)

    def test_all_generated_support_modules_are_active_business_code(self):
        for index in range(SUPPORT_COUNT):
            module = importlib.import_module(f"retail_core.support.support_{index:02d}")
            with self.subTest(kind="support", index=index):
                self.assert_active_module_contract(module, index)

    def test_all_generated_integration_modules_are_active_business_code(self):
        for offset in range(INTEGRATION_COUNT):
            index = SUPPORT_COUNT + offset
            module = importlib.import_module(f"retail_core.integrations.adapter_{offset:02d}")
            with self.subTest(kind="integration", index=index):
                self.assert_active_module_contract(module, index)


if __name__ == "__main__":
    unittest.main()
`,
    )
  }
}

function backendPolicyModuleContent(index) {
  const name = `policy_${String(index).padStart(2, "0")}`
  return `from __future__ import annotations

from decimal import Decimal


POLICY_ID = "${name}"


def normalize_threshold(value: Decimal | int | str) -> Decimal:
    return Decimal(str(value)).quantize(Decimal("0.01"))


def score_policy(value: int) -> int:
    base = value + ${index}
    return base


def discount_ceiling(subtotal: Decimal) -> Decimal:
    subtotal = normalize_threshold(subtotal)
    return (subtotal * Decimal("${(index % 7) + 3}") / Decimal("1000")).quantize(Decimal("0.01"))


def eligibility_flags(quantity: int, tier: str) -> tuple[str, ...]:
    flags = []
    if quantity > ${index % 5 + 1}:
        flags.append("bulk")
    if tier == "vip":
        flags.append("priority")
    return tuple(flags)
`
}

async function writeBackendPolicyCodebase(repoPath) {
  const policyDir = path.join(repoPath, "src", "retail_core", "policy")
  const policyCount = 40 * fixtureScaleMultiplier * hardActiveScaleMultiplier
  await fs.mkdir(policyDir, { recursive: true })
  await writeText(
    path.join(policyDir, "__init__.py"),
    `"""Generated backend policy modules used by integration tests."""
`,
  )
  for (let index = 0; index < policyCount; index += 1) {
    await writeText(path.join(policyDir, `policy_${String(index).padStart(2, "0")}.py`), backendPolicyModuleContent(index))
  }
  await writeText(
    path.join(repoPath, "tests", "test_backend_policy_matrix.py"),
    `from decimal import Decimal
import importlib
import unittest


class BackendPolicyMatrixTests(unittest.TestCase):
    def test_generated_policy_scores_and_caps(self):
        for index in range(${policyCount}):
            module = importlib.import_module(f"retail_core.policy.policy_{index:02d}")
            with self.subTest(policy=index):
                self.assertEqual(module.score_policy(10), 10 + index + 1)
                expected_cap = (Decimal("250.00") * Decimal(str((index % 7) + 3)) / Decimal("100")).quantize(Decimal("0.01"))
                self.assertEqual(module.discount_ceiling(Decimal("250.00")), expected_cap)
                self.assertIn("priority", module.eligibility_flags(index % 5 + 2, " VIP "))


if __name__ == "__main__":
    unittest.main()
`,
  )
}

function frontendModuleContent(index) {
  return `export const MODULE_ID = "view_${String(index).padStart(2, "0")}";

export function formatCurrency${index}(value) {
  return "$" + Number(value).toFixed(1);
}

export function normalizeRoute${index}(route) {
  return String(route).trim();
}

export function deriveBadge${index}(status, count) {
  if (status === "late") return "danger";
  if (count > ${index % 6 + 1}) return "normal";
  return "quiet";
}

export function reducePanelState${index}(state, event) {
  const next = { ...state };
  if (event.type === "toggle") next.open = !state.open;
  if (event.type === "increment") next.count = state.count + 2;
  if (event.type === "reset") next.count = 1;
  return next;
}
`
}

function frontendNeutralModuleContent(index) {
  const lines = [
    `export const SHARED_ID = "shared_${String(index).padStart(2, "0")}";`,
    "",
    "export function normalizeText(value) {",
    "  return String(value ?? '').trim().toLowerCase();",
    "}",
    "",
  ]
  for (let item = 0; item < 42; item += 1) {
    lines.push(
      `export function sharedRule${index}_${item}(input) {`,
      `  const value = Number(input ?? 0) + ${index} + ${item};`,
      "  if (value % 7 === 0) return value / 7;",
      "  if (value % 2 === 0) return value + 2;",
      "  return value - 1;",
      "}",
      "",
    )
  }
  return `${lines.join("\n")}\n`
}

async function writeFrontendCodebase(repoPath) {
  const srcDir = path.join(repoPath, "frontend", "src", "views")
  const sharedDir = path.join(repoPath, "frontend", "src", "shared")
  const testsDir = path.join(repoPath, "frontend", "tests")
  const viewCount = 40 * fixtureScaleMultiplier * hardActiveScaleMultiplier
  const sharedCount = 50 * fixtureScaleMultiplier * hardActiveScaleMultiplier
  await fs.mkdir(srcDir, { recursive: true })
  await fs.mkdir(sharedDir, { recursive: true })
  await fs.mkdir(testsDir, { recursive: true })
  await writeText(
    path.join(repoPath, "frontend", "package.json"),
    `{
  "name": "retail-ledger-frontend",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "node --test tests/*.test.mjs"
  }
}
`,
  )
  for (let index = 0; index < viewCount; index += 1) {
    await writeText(path.join(srcDir, `view_${String(index).padStart(2, "0")}.mjs`), frontendModuleContent(index))
  }
  for (let index = 0; index < sharedCount; index += 1) {
    await writeText(path.join(sharedDir, `shared_${String(index).padStart(2, "0")}.mjs`), frontendNeutralModuleContent(index))
  }
  await writeText(
    path.join(testsDir, "frontend_behavior.test.mjs"),
    `import test from "node:test";
import assert from "node:assert/strict";

test("generated view modules normalize display, routes, badges, and panel state", async () => {
  for (let index = 0; index < ${viewCount}; index += 1) {
    const id = String(index).padStart(2, "0");
    const mod = await import(\`../src/views/view_\${id}.mjs\`);
    assert.equal(mod[\`formatCurrency\${index}\`](12), "$12.00", \`currency \${index}\`);
    assert.equal(mod[\`normalizeRoute\${index}\`](" Orders / Today "), "/orders/today", \`route \${index}\`);
    assert.equal(mod[\`deriveBadge\${index}\`]("late", 0), "danger", \`late badge \${index}\`);
    assert.equal(mod[\`deriveBadge\${index}\`]("ready", (index % 6) + 2), "strong", \`count badge \${index}\`);
    const start = { open: false, count: 4 };
    assert.deepEqual(mod[\`reducePanelState\${index}\`](start, { type: "toggle" }), { open: true, count: 4 }, \`toggle \${index}\`);
    assert.deepEqual(mod[\`reducePanelState\${index}\`](start, { type: "increment" }), { open: false, count: 5 }, \`increment \${index}\`);
    assert.deepEqual(mod[\`reducePanelState\${index}\`](start, { type: "reset" }), { open: false, count: 0 }, \`reset \${index}\`);
  }
});
`,
  )
  if (hardActiveGeneratedCodeEnabled) {
    await writeText(
      path.join(testsDir, "frontend_shared_active_matrix.test.mjs"),
      `import test from "node:test";
import assert from "node:assert/strict";

test("generated shared modules participate in active behavior", async () => {
  for (let index = 0; index < ${sharedCount}; index += 1) {
    const id = String(index).padStart(2, "0");
    const mod = await import(\`../src/shared/shared_\${id}.mjs\`);
    assert.equal(mod.normalizeText(" Order_ID  " + index + " "), "order-id-" + index, \`normalize shared \${index}\`);
    assert.equal(mod[\`sharedRule\${index}_0\`](10), 10 + index + 1, \`rule0 shared \${index}\`);
    assert.equal(mod[\`sharedRule\${index}_9\`](10), 10 + index + 10, \`rule9 shared \${index}\`);
    assert.equal(mod[\`sharedRule\${index}_21\`](10), 10 + index + 22, \`rule21 shared \${index}\`);
    assert.equal(mod[\`sharedRule\${index}_41\`](10), 10 + index + 42, \`rule41 shared \${index}\`);
  }
});
`,
    )
  }
}

async function writeEnterpriseExpansion(repoPath) {
  await fs.mkdir(path.join(repoPath, "docs"), { recursive: true })
  await writeText(
    path.join(repoPath, "docs", "enterprise_requirements.md"),
    `# Enterprise Expansion Requirements

The enterprise tests describe new product requirements layered on top of the
existing retail operations workflows. Implement these as production code, not
test-specific shortcuts.

Backend requirements:

- Provide \`retail_core.enterprise.build_customer_360\` for account managers.
  It must normalize customer identity, merge duplicate customers, aggregate
  spend and refunds, preserve first-seen customer ids, sort risk flags, compute
  open support ticket counts, and produce deterministic CSV rows.
- Provide \`retail_core.enterprise.forecast_inventory_replenishment\` for
  planners. It must combine on-hand, reserved, target, safety stock, lead-time
  demand, and inbound purchase orders. Recommendations should be sorted by
  urgency and SKU.
- Provide \`retail_core.enterprise.reconcile_promotions_and_returns\` for
  finance. It must combine priced orders, coupon metadata, and refunds into
  promotion liability rows with order-level and category-level net revenue.
- Provide \`retail_core.enterprise.build_audit_timeline\` for operations. It
  must merge order, payment, shipment, and support events into a stable
  chronological timeline with duplicate event ids removed.

Frontend requirements:

- Provide \`frontend/src/views/enterprise_dashboard.mjs\`.
- \`buildDashboardState\` must summarize KPI cards, risk queues, inventory
  alerts, and export metadata from backend-style payloads.
- \`reconcileFilters\` must normalize route/query/filter state into a stable
  object used by bookmarked enterprise views.
- \`exportEnterpriseCsv\` must emit deterministic CSV with escaped values.

Validation:

- \`tests/test_enterprise_requirements.py\` validates backend behavior.
- \`frontend/tests/enterprise_dashboard.test.mjs\` validates frontend behavior.
- The existing \`tools/verify.ps1\` command must pass without weakening tests.
`,
  )
  await writeText(
    path.join(repoPath, "tests", "test_enterprise_requirements.py"),
    `from decimal import Decimal
import unittest

from retail_core.addresses import Address
from retail_core.enterprise import (
    build_audit_timeline,
    build_customer_360,
    forecast_inventory_replenishment,
    reconcile_promotions_and_returns,
)
from retail_core.orders import CheckoutRequest, OrderLine, price_order
from retail_core.returns import ReturnLine, refund_for_lines


class EnterpriseRequirementsTests(unittest.TestCase):
    def test_customer_360_merges_identity_spend_support_and_exports(self):
        orders = [
            {
                "order_id": "o-100",
                "customer_id": "cust-1",
                "email": " Ada@Example.COM ",
                "shipping_state": " wa ",
                "postal_code": " 98101 - 1234 ",
                "total": Decimal("57.77"),
                "refund": Decimal("24.41"),
                "risk_flags": ["late_payment", "address_mismatch"],
            },
            {
                "order_id": "o-101",
                "customer_id": "cust-duplicate",
                "email": "ada@example.com",
                "shipping_state": "WA",
                "postal_code": "98101-1234",
                "total": Decimal("31.00"),
                "refund": Decimal("0.00"),
                "risk_flags": ["late_payment"],
            },
            {
                "order_id": "o-200",
                "customer_id": "cust-2",
                "email": "grace@example.com",
                "shipping_state": "OR",
                "postal_code": "97035",
                "total": Decimal("155.00"),
                "refund": Decimal("45.00"),
                "risk_flags": ["prior_chargeback"],
            },
        ]
        tickets = [
            {"ticket_id": "t-1", "email": "ada@example.com", "status": "open", "severity": "high"},
            {"ticket_id": "t-2", "email": " ADA@example.com ", "status": "closed", "severity": "low"},
            {"ticket_id": "t-3", "email": "grace@example.com", "status": "open", "severity": "medium"},
        ]

        report = build_customer_360(orders, tickets)

        self.assertEqual(report["customer_count"], 2)
        self.assertEqual(
            report["customers"],
            [
                {
                    "customer_id": "cust-1",
                    "identity_key": "ada@example.com|WA|98101-1234",
                    "email": "ada@example.com",
                    "state": "WA",
                    "postal_code": "98101-1234",
                    "gross_spend": Decimal("88.77"),
                    "refunds": Decimal("24.41"),
                    "net_spend": Decimal("64.36"),
                    "order_count": 2,
                    "open_tickets": 1,
                    "risk_flags": ("address_mismatch", "late_payment"),
                },
                {
                    "customer_id": "cust-2",
                    "identity_key": "grace@example.com|OR|97035",
                    "email": "grace@example.com",
                    "state": "OR",
                    "postal_code": "97035",
                    "gross_spend": Decimal("155.00"),
                    "refunds": Decimal("45.00"),
                    "net_spend": Decimal("110.00"),
                    "order_count": 1,
                    "open_tickets": 1,
                    "risk_flags": ("prior_chargeback",),
                },
            ],
        )
        self.assertEqual(
            report["csv_rows"],
            [
                "customer_id,identity_key,net_spend,open_tickets,risk_flags",
                "cust-1,ada@example.com|WA|98101-1234,64.36,1,address_mismatch|late_payment",
                "cust-2,grace@example.com|OR|97035,110.00,1,prior_chargeback",
            ],
        )

    def test_inventory_forecast_accounts_for_inbound_and_lead_time_demand(self):
        forecast = forecast_inventory_replenishment(
            inventory=[
                {"sku": " SKU COFFEE 1 ", "on_hand": 8, "reserved": 3, "target": 10, "safety_stock": 2},
                {"sku": "sku_mug_2", "on_hand": 3, "reserved": 1, "target": 3, "safety_stock": 1},
                {"sku": "sku-sticker-3", "on_hand": 50, "reserved": 0, "target": 10, "safety_stock": 0},
            ],
            demand=[
                {"sku": "sku-coffee-1", "daily_units": 2, "lead_days": 4},
                {"sku": "sku-mug-2", "daily_units": 1, "lead_days": 3},
                {"sku": "sku-sticker-3", "daily_units": 1, "lead_days": 1},
            ],
            inbound=[
                {"sku": "sku-coffee-1", "quantity": 5, "eta_days": 2},
                {"sku": "sku-mug-2", "quantity": 1, "eta_days": 9},
            ],
        )

        self.assertEqual(
            forecast,
            [
                {
                    "sku": "sku-coffee-1",
                    "available": 5,
                    "lead_time_demand": 8,
                    "inbound_before_due": 5,
                    "required": 15,
                    "recommendation": 5,
                    "urgency": "high",
                },
                {
                    "sku": "sku-mug-2",
                    "available": 2,
                    "lead_time_demand": 3,
                    "inbound_before_due": 0,
                    "required": 7,
                    "recommendation": 5,
                    "urgency": "medium",
                },
            ],
        )

    def test_promotion_return_reconciliation_preserves_category_liability(self):
        order_1 = price_order(
            CheckoutRequest(
                lines=[OrderLine("sku-coffee-1", 2), OrderLine("sku-mug-2", 1)],
                destination_state="wa",
                coupon_code="SAVE10",
            )
        )
        order_2 = price_order(
            CheckoutRequest(
                lines=[OrderLine("sku-sticker-3", 4), OrderLine("sku-mug-2", 1)],
                destination_state="or",
                coupon_code="HOME5",
                customer_tier="vip",
            )
        )
        refund = refund_for_lines(order_1, [ReturnLine("sku mug 2", 1)])

        reconciliation = reconcile_promotions_and_returns(
            [
                {"order_id": "order-1", "order": order_1, "coupon_code": "SAVE10", "refund": refund},
                {"order_id": "order-2", "order": order_2, "coupon_code": "HOME5", "refund": Decimal("0.00")},
            ]
        )

        self.assertEqual(reconciliation["gross_sales"], Decimal("89.77"))
        self.assertEqual(reconciliation["refund_total"], Decimal("24.41"))
        self.assertEqual(reconciliation["net_sales"], Decimal("65.36"))
        self.assertEqual(
            reconciliation["coupon_liability"],
            [
                {"coupon_code": "HOME5", "orders": 1, "discount": Decimal("5.00"), "refunds": Decimal("0.00")},
                {"coupon_code": "SAVE10", "orders": 1, "discount": Decimal("5.10"), "refunds": Decimal("24.41")},
            ],
        )
        self.assertEqual(
            reconciliation["category_net_revenue"],
            {"grocery": Decimal("25.37"), "home": Decimal("44.41"), "merch": Decimal("12.00")},
        )

    def test_audit_timeline_deduplicates_and_orders_cross_domain_events(self):
        timeline = build_audit_timeline(
            order_events=[
                {"event_id": "order-created", "at": "2026-05-01T09:00:00Z", "order_id": "order-1", "actor": "checkout"},
                {"event_id": "duplicate", "at": "2026-05-01T09:03:00Z", "order_id": "order-1", "actor": "checkout"},
            ],
            payment_events=[
                {"event_id": "payment-authorized", "at": "2026-05-01T09:01:00Z", "order_id": "order-1", "actor": "payments"},
                {"event_id": "duplicate", "at": "2026-05-01T09:04:00Z", "order_id": "order-1", "actor": "payments"},
            ],
            shipment_events=[
                {"event_id": "shipment-promised", "at": "2026-05-01T09:02:00Z", "order_id": "order-1", "actor": "fulfillment"},
            ],
            support_events=[
                {"event_id": "ticket-opened", "at": "2026-05-01T09:05:00Z", "order_id": "order-1", "actor": "support"},
            ],
        )

        self.assertEqual(
            timeline,
            [
                {"at": "2026-05-01T09:00:00Z", "domain": "order", "event_id": "order-created", "order_id": "order-1", "actor": "checkout"},
                {"at": "2026-05-01T09:01:00Z", "domain": "payment", "event_id": "payment-authorized", "order_id": "order-1", "actor": "payments"},
                {"at": "2026-05-01T09:02:00Z", "domain": "shipment", "event_id": "shipment-promised", "order_id": "order-1", "actor": "fulfillment"},
                {"at": "2026-05-01T09:03:00Z", "domain": "order", "event_id": "duplicate", "order_id": "order-1", "actor": "checkout"},
                {"at": "2026-05-01T09:05:00Z", "domain": "support", "event_id": "ticket-opened", "order_id": "order-1", "actor": "support"},
            ],
        )


if __name__ == "__main__":
    unittest.main()
`,
  )
  await writeText(
    path.join(repoPath, "frontend", "tests", "enterprise_dashboard.test.mjs"),
    `import test from "node:test";
import assert from "node:assert/strict";

import {
  buildDashboardState,
  exportEnterpriseCsv,
  reconcileFilters,
} from "../src/views/enterprise_dashboard.mjs";

test("enterprise dashboard summarizes cross-domain state", () => {
  const state = buildDashboardState({
    customers: [
      { customer_id: "cust-1", net_spend: "64.36", open_tickets: 1, risk_flags: ["late_payment", "address_mismatch"] },
      { customer_id: "cust-2", net_spend: "110.00", open_tickets: 1, risk_flags: ["prior_chargeback"] },
    ],
    inventory: [
      { sku: "sku-coffee-1", recommendation: 5, urgency: "high" },
      { sku: "sku-mug-2", recommendation: 5, urgency: "medium" },
    ],
    reconciliation: {
      net_sales: "65.36",
      refund_total: "24.41",
      coupon_liability: [
        { coupon_code: "HOME5", discount: "5.00" },
        { coupon_code: "SAVE10", discount: "5.10" },
      ],
    },
  });

  assert.deepEqual(state.kpis, [
    { label: "Net sales", value: "$65.36" },
    { label: "Refunds", value: "$24.41" },
    { label: "Open tickets", value: "2" },
    { label: "High urgency SKUs", value: "1" },
  ]);
  assert.deepEqual(state.riskQueue, [
    { customer_id: "cust-1", risk_count: 2, risk_flags: "address_mismatch|late_payment" },
    { customer_id: "cust-2", risk_count: 1, risk_flags: "prior_chargeback" },
  ]);
  assert.deepEqual(state.inventoryAlerts, ["sku-coffee-1:5:high", "sku-mug-2:5:medium"]);
  assert.equal(state.exportName, "enterprise-dashboard-2-customers.csv");
});

test("enterprise dashboard normalizes filters and exports csv", () => {
  assert.deepEqual(
    reconcileFilters({
      route: " Enterprise / Risk ",
      query: " Ada Lovelace ",
      filters: { state: " wa ", urgency: " HIGH ", empty: "" },
      page: "3",
    }),
    {
      route: "/enterprise/risk",
      query: "ada lovelace",
      filters: { state: "WA", urgency: "high" },
      page: 3,
    },
  );

  assert.equal(
    exportEnterpriseCsv([
      { customer_id: "cust-1", note: "needs, review", net_spend: "64.36" },
      { customer_id: "cust-2", note: "clear", net_spend: "110.00" },
    ]),
    [
      "customer_id,net_spend,note",
      "cust-1,64.36,\\"needs, review\\"",
      "cust-2,110.00,clear",
    ].join("\\n"),
  );
});
`,
  )

  if (hardEnterpriseExpansionEnabled) {
    await writeEnterpriseAdvancedExpansion(repoPath)
  }
}

async function writeEnterpriseAdvancedExpansion(repoPath) {
  await writeText(
    path.join(repoPath, "docs", "enterprise_advanced_requirements.md"),
    `# Enterprise Advanced Requirements

These requirements extend the enterprise surface with cross-domain planning,
finance, privacy, operations, and executive reporting behavior. Implement them
as production modules. Do not special-case the tests.

Backend requirements:

- Provide \`retail_core.enterprise_advanced.calculate_customer_health_scores\`.
  It must merge customer 360 rows, support tickets, SLA breaches, fraud signals,
  and revenue trends into deterministic health scores and next actions.
- Provide \`retail_core.enterprise_advanced.allocate_fulfillment_batches\`.
  It must allocate orders to warehouses using stock, distance, priority,
  hazard restrictions, split-shipment rules, and stable tie breakers.
- Provide \`retail_core.enterprise_advanced.reconcile_tax_jurisdictions\`.
  It must aggregate order tax by normalized state/county/city jurisdiction and
  reconcile over/under collection with Decimal-safe rounding.
- Provide \`retail_core.enterprise_advanced.detect_margin_anomalies\`.
  It must combine product cost, discounts, refunds, shipping subsidy, and
  return rates to flag margin problems with explainable reasons.
- Provide \`retail_core.enterprise_advanced.generate_executive_brief\`.
  It must combine customer health, inventory forecasts, promotion liability,
  SLA breaches, margin anomalies, and tax reconciliation into a stable brief.
- Provide \`retail_core.enterprise_advanced.validate_privacy_exports\`.
  It must redact PII, preserve allowed business fields, and report rejected
  fields per row.
- Provide \`retail_core.enterprise_advanced.build_return_fraud_signals\`.
  It must group returns by identity, sku family, address, and time window.
- Provide \`retail_core.enterprise_advanced.compile_sla_breach_report\`.
  It must merge promised dates, actual shipment dates, carrier exceptions, and
  support escalations into prioritized breach rows.

Frontend requirements:

- Provide \`frontend/src/views/enterprise_operations_console.mjs\`.
- \`buildOperationsConsole\` must summarize advanced backend payloads into KPI
  cards, work queues, exception lanes, and executive highlights.
- \`normalizeScenarioState\` must turn route/query/filter/bookmark state into a
  deterministic scenario object.
- \`exportOperationsWorkbook\` must emit multiple deterministic CSV sections.
- \`mergeDrilldownEvents\` must merge mixed-domain events, remove duplicate ids,
  and keep stable chronological ordering.
- \`scoreActionPriority\` must score actions from severity, age, revenue, and
  confidence.

Validation:

- \`tests/test_enterprise_advanced_requirements.py\` validates backend behavior.
- \`frontend/tests/enterprise_operations_console.test.mjs\` validates frontend behavior.
- \`tests/test_enterprise_scenario_matrix.py\` validates edge cases and tie breakers.
- \`frontend/tests/enterprise_operations_edge_cases.test.mjs\` validates frontend edge cases.
- \`tests/test_enterprise_control_plane_generated.py\` validates 24 additional backend control-plane functions.
- \`frontend/tests/enterprise_control_tower_generated.test.mjs\` validates 24 additional frontend control-tower functions.
- The existing \`tools/verify.ps1\` command must pass without weakening tests.
`,
  )

  await writeText(
    path.join(repoPath, "tests", "test_enterprise_advanced_requirements.py"),
    `from decimal import Decimal
import unittest

from retail_core.enterprise_advanced import (
    allocate_fulfillment_batches,
    build_return_fraud_signals,
    calculate_customer_health_scores,
    compile_sla_breach_report,
    detect_margin_anomalies,
    generate_executive_brief,
    reconcile_tax_jurisdictions,
    validate_privacy_exports,
)


class EnterpriseAdvancedRequirementsTests(unittest.TestCase):
    def test_customer_health_scores_prioritize_revenue_risk_and_support(self):
        rows = calculate_customer_health_scores(
            customers=[
                {"customer_id": "cust-1", "net_spend": Decimal("900.00"), "open_tickets": 2, "risk_flags": ("late_payment",)},
                {"customer_id": "cust-2", "net_spend": Decimal("120.00"), "open_tickets": 0, "risk_flags": ()},
                {"customer_id": "cust-3", "net_spend": Decimal("400.00"), "open_tickets": 1, "risk_flags": ("prior_chargeback", "address_mismatch")},
            ],
            sla_breaches=[
                {"customer_id": "cust-1", "days_late": 4, "severity": "high"},
                {"customer_id": "cust-3", "days_late": 2, "severity": "medium"},
            ],
            fraud_signals=[
                {"customer_id": "cust-3", "risk_score": 82, "reason": "return velocity"},
                {"customer_id": "cust-1", "risk_score": 40, "reason": "address churn"},
            ],
            revenue_trends=[
                {"customer_id": "cust-1", "last_30": Decimal("300.00"), "previous_30": Decimal("450.00")},
                {"customer_id": "cust-2", "last_30": Decimal("75.00"), "previous_30": Decimal("50.00")},
                {"customer_id": "cust-3", "last_30": Decimal("100.00"), "previous_30": Decimal("180.00")},
            ],
        )

        self.assertEqual(
            rows,
            [
                {"customer_id": "cust-3", "score": 30, "tier": "critical", "next_action": "fraud_review", "reasons": ("declining_revenue", "fraud_signal", "open_support", "risk_flags", "sla_breach")},
                {"customer_id": "cust-1", "score": 45, "tier": "watch", "next_action": "save_plan", "reasons": ("declining_revenue", "open_support", "risk_flags", "sla_breach")},
                {"customer_id": "cust-2", "score": 96, "tier": "healthy", "next_action": "expand", "reasons": ("growing_revenue",)},
            ],
        )

    def test_allocate_fulfillment_batches_respects_stock_priority_and_hazmat(self):
        allocations = allocate_fulfillment_batches(
            orders=[
                {"order_id": "o-1", "priority": "expedite", "destination": "WA", "lines": [{"sku": "sku-coffee-1", "quantity": 4}, {"sku": "sku-haz-1", "quantity": 1}]},
                {"order_id": "o-2", "priority": "standard", "destination": "OR", "lines": [{"sku": "sku-coffee-1", "quantity": 3}]},
                {"order_id": "o-3", "priority": "vip", "destination": "WA", "lines": [{"sku": "sku-mug-2", "quantity": 2}]},
            ],
            warehouses=[
                {"warehouse_id": "sea", "states": ["WA", "OR"], "hazmat": False, "stock": {"sku-coffee-1": 5, "sku-mug-2": 2}},
                {"warehouse_id": "boi", "states": ["WA", "OR"], "hazmat": True, "stock": {"sku-coffee-1": 4, "sku-haz-1": 2}},
                {"warehouse_id": "reno", "states": ["OR"], "hazmat": False, "stock": {"sku-coffee-1": 10}},
            ],
            distance_rank={"WA": ["sea", "boi", "reno"], "OR": ["reno", "sea", "boi"]},
        )

        self.assertEqual(
            allocations,
            [
                {"order_id": "o-3", "warehouse_id": "sea", "lines": (("sku-mug-2", 2),), "status": "allocated", "split": False},
                {"order_id": "o-1", "warehouse_id": "sea", "lines": (("sku-coffee-1", 4),), "status": "partial", "split": True},
                {"order_id": "o-1", "warehouse_id": "boi", "lines": (("sku-haz-1", 1),), "status": "allocated", "split": True},
                {"order_id": "o-2", "warehouse_id": "reno", "lines": (("sku-coffee-1", 3),), "status": "allocated", "split": False},
            ],
        )

    def test_reconcile_tax_jurisdictions_aggregates_and_flags_variance(self):
        report = reconcile_tax_jurisdictions(
            orders=[
                {"order_id": "o-1", "state": " wa ", "county": " King ", "city": " Seattle ", "taxable": Decimal("100.00"), "collected_tax": Decimal("10.25")},
                {"order_id": "o-2", "state": "WA", "county": "king", "city": "seattle", "taxable": Decimal("50.00"), "collected_tax": Decimal("4.00")},
                {"order_id": "o-3", "state": "OR", "county": " Multnomah ", "city": " Portland ", "taxable": Decimal("80.00"), "collected_tax": Decimal("0.00")},
            ],
            rates={
                "WA|KING|SEATTLE": Decimal("0.1025"),
                "OR|MULTNOMAH|PORTLAND": Decimal("0.0000"),
            },
        )

        self.assertEqual(
            report,
            {
                "jurisdictions": [
                    {"jurisdiction": "OR|MULTNOMAH|PORTLAND", "orders": 1, "taxable": Decimal("80.00"), "expected_tax": Decimal("0.00"), "collected_tax": Decimal("0.00"), "variance": Decimal("0.00"), "status": "balanced"},
                    {"jurisdiction": "WA|KING|SEATTLE", "orders": 2, "taxable": Decimal("150.00"), "expected_tax": Decimal("15.38"), "collected_tax": Decimal("14.25"), "variance": Decimal("-1.13"), "status": "under_collected"},
                ],
                "total_variance": Decimal("-1.13"),
            },
        )

    def test_detect_margin_anomalies_explains_discount_refund_and_shipping_pressure(self):
        anomalies = detect_margin_anomalies(
            products=[
                {"sku": "sku-coffee-1", "category": "grocery", "price": Decimal("12.00"), "cost": Decimal("4.00")},
                {"sku": "sku-mug-2", "category": "home", "price": Decimal("24.00"), "cost": Decimal("18.00")},
                {"sku": "sku-sticker-3", "category": "merch", "price": Decimal("3.00"), "cost": Decimal("0.50")},
            ],
            sales=[
                {"sku": "sku-coffee-1", "units": 10, "discount": Decimal("8.00"), "refund": Decimal("0.00"), "shipping_subsidy": Decimal("4.00")},
                {"sku": "sku-mug-2", "units": 2, "discount": Decimal("10.00"), "refund": Decimal("24.41"), "shipping_subsidy": Decimal("6.00")},
                {"sku": "sku-sticker-3", "units": 50, "discount": Decimal("0.00"), "refund": Decimal("0.00"), "shipping_subsidy": Decimal("0.00")},
            ],
            minimum_margin=Decimal("0.35"),
        )

        self.assertEqual(
            anomalies,
            [
                {"sku": "sku-mug-2", "margin_rate": Decimal("-0.34"), "severity": "critical", "reasons": ("discount_pressure", "refund_pressure", "shipping_subsidy")},
                {"sku": "sku-coffee-1", "margin_rate": Decimal("0.57"), "severity": "ok", "reasons": ()},
                {"sku": "sku-sticker-3", "margin_rate": Decimal("0.83"), "severity": "ok", "reasons": ()},
            ],
        )

    def test_privacy_exports_redact_pii_and_report_rejected_fields(self):
        result = validate_privacy_exports(
            rows=[
                {"customer_id": "cust-1", "email": "Ada@Example.com", "phone": "555-1212", "net_spend": "64.36", "risk_flags": "late_payment", "internal_note": "VIP"},
                {"customer_id": "cust-2", "email": "", "phone": "", "net_spend": "110.00", "risk_flags": "prior_chargeback", "address": "1 Main"},
            ],
            allowed_fields=["customer_id", "net_spend", "risk_flags"],
        )

        self.assertEqual(
            result,
            {
                "rows": [
                    {"customer_id": "cust-1", "net_spend": "64.36", "risk_flags": "late_payment"},
                    {"customer_id": "cust-2", "net_spend": "110.00", "risk_flags": "prior_chargeback"},
                ],
                "rejected_fields": {
                    "cust-1": ("email", "internal_note", "phone"),
                    "cust-2": ("address", "email", "phone"),
                },
            },
        )

    def test_return_fraud_signals_group_velocity_address_and_sku_family(self):
        signals = build_return_fraud_signals(
            returns=[
                {"return_id": "r1", "customer_id": "cust-1", "email": "ada@example.com", "address_key": "WA-98101", "sku": "sku-coffee-1", "returned_at": "2026-05-01"},
                {"return_id": "r2", "customer_id": "cust-1", "email": "ada@example.com", "address_key": "WA-98101", "sku": "sku-coffee-2", "returned_at": "2026-05-03"},
                {"return_id": "r3", "customer_id": "cust-9", "email": "ada@example.com", "address_key": "WA-98101", "sku": "sku-coffee-3", "returned_at": "2026-05-06"},
                {"return_id": "r4", "customer_id": "cust-2", "email": "grace@example.com", "address_key": "OR-97035", "sku": "sku-mug-2", "returned_at": "2026-05-10"},
            ],
            window_days=7,
        )

        self.assertEqual(
            signals,
            [
                {"identity": "ada@example.com|WA-98101", "return_count": 3, "sku_families": ("sku-coffee",), "customer_ids": ("cust-1", "cust-9"), "risk_score": 95, "reason": "velocity+shared_identity"},
                {"identity": "grace@example.com|OR-97035", "return_count": 1, "sku_families": ("sku-mug",), "customer_ids": ("cust-2",), "risk_score": 10, "reason": "baseline"},
            ],
        )

    def test_compile_sla_breach_report_prioritizes_late_high_value_escalations(self):
        report = compile_sla_breach_report(
            promises=[
                {"order_id": "o-1", "customer_id": "cust-1", "promised_at": "2026-05-03", "order_value": Decimal("300.00")},
                {"order_id": "o-2", "customer_id": "cust-2", "promised_at": "2026-05-04", "order_value": Decimal("45.00")},
                {"order_id": "o-3", "customer_id": "cust-3", "promised_at": "2026-05-05", "order_value": Decimal("110.00")},
            ],
            shipments=[
                {"order_id": "o-1", "delivered_at": "2026-05-07", "carrier_exception": "weather"},
                {"order_id": "o-2", "delivered_at": "2026-05-04", "carrier_exception": ""},
                {"order_id": "o-3", "delivered_at": "2026-05-08", "carrier_exception": "missort"},
            ],
            escalations=[
                {"order_id": "o-1", "severity": "high"},
                {"order_id": "o-3", "severity": "medium"},
            ],
        )

        self.assertEqual(
            report,
            [
                {"order_id": "o-1", "customer_id": "cust-1", "days_late": 4, "priority": 98, "reason": "high_value+support_escalation+carrier_exception"},
                {"order_id": "o-3", "customer_id": "cust-3", "days_late": 3, "priority": 73, "reason": "support_escalation+carrier_exception"},
            ],
        )

    def test_generate_executive_brief_combines_all_advanced_surfaces(self):
        brief = generate_executive_brief(
            health_scores=[
                {"customer_id": "cust-3", "tier": "critical", "score": 30},
                {"customer_id": "cust-1", "tier": "watch", "score": 45},
                {"customer_id": "cust-2", "tier": "healthy", "score": 96},
            ],
            inventory_forecast=[
                {"sku": "sku-coffee-1", "urgency": "high", "recommendation": 5},
                {"sku": "sku-mug-2", "urgency": "medium", "recommendation": 5},
            ],
            promotion_reconciliation={"net_sales": Decimal("65.36"), "refund_total": Decimal("24.41")},
            sla_breaches=[
                {"order_id": "o-1", "priority": 98},
                {"order_id": "o-3", "priority": 73},
            ],
            margin_anomalies=[
                {"sku": "sku-mug-2", "severity": "critical"},
                {"sku": "sku-coffee-1", "severity": "ok"},
            ],
            tax_report={"total_variance": Decimal("-1.13")},
        )

        self.assertEqual(
            brief,
            {
                "headline": "2 customers need attention; 1 high urgency SKU; 1 critical margin anomaly",
                "risk": {"critical_customers": 1, "watch_customers": 1, "sla_breaches": 2},
                "finance": {"net_sales": Decimal("65.36"), "refund_total": Decimal("24.41"), "tax_variance": Decimal("-1.13")},
                "actions": (
                    "review critical customer cust-3",
                    "replenish sku-coffee-1 by 5 units",
                    "investigate margin anomaly sku-mug-2",
                    "resolve SLA breach o-1",
                ),
            },
        )


if __name__ == "__main__":
    unittest.main()
`,
  )

  await writeText(
    path.join(repoPath, "frontend", "tests", "enterprise_operations_console.test.mjs"),
    `import test from "node:test";
import assert from "node:assert/strict";

import {
  buildOperationsConsole,
  exportOperationsWorkbook,
  mergeDrilldownEvents,
  normalizeScenarioState,
  scoreActionPriority,
} from "../src/views/enterprise_operations_console.mjs";

test("operations console builds executive KPIs, queues, lanes, and highlights", () => {
  const consoleState = buildOperationsConsole({
    brief: {
      headline: "2 customers need attention; 1 high urgency SKU; 1 critical margin anomaly",
      risk: { critical_customers: 1, watch_customers: 1, sla_breaches: 2 },
      finance: { net_sales: "65.36", refund_total: "24.41", tax_variance: "-1.13" },
      actions: ["review critical customer cust-3", "replenish sku-coffee-1 by 5 units"],
    },
    healthScores: [
      { customer_id: "cust-3", tier: "critical", score: 30, next_action: "fraud_review" },
      { customer_id: "cust-1", tier: "watch", score: 45, next_action: "save_plan" },
    ],
    slaBreaches: [
      { order_id: "o-1", priority: 98, days_late: 4 },
      { order_id: "o-3", priority: 73, days_late: 3 },
    ],
    marginAnomalies: [
      { sku: "sku-mug-2", severity: "critical", margin_rate: "-0.34" },
      { sku: "sku-coffee-1", severity: "ok", margin_rate: "0.57" },
    ],
  });

  assert.deepEqual(consoleState.kpis, [
    { label: "Net sales", value: "$65.36", tone: "neutral" },
    { label: "Refunds", value: "$24.41", tone: "watch" },
    { label: "Tax variance", value: "-$1.13", tone: "critical" },
    { label: "SLA breaches", value: "2", tone: "critical" },
  ]);
  assert.deepEqual(consoleState.workQueue, [
    { id: "cust-3", label: "fraud_review", priority: 90 },
    { id: "o-1", label: "SLA 4 days late", priority: 98 },
    { id: "o-3", label: "SLA 3 days late", priority: 73 },
    { id: "cust-1", label: "save_plan", priority: 65 },
  ]);
  assert.deepEqual(consoleState.exceptionLanes, {
    customer: ["cust-3", "cust-1"],
    fulfillment: ["o-1", "o-3"],
    margin: ["sku-mug-2"],
  });
  assert.equal(consoleState.headline, "2 customers need attention; 1 high urgency SKU; 1 critical margin anomaly");
});

test("scenario state normalizes nested filters, bookmarks, and dates", () => {
  assert.deepEqual(
    normalizeScenarioState({
      route: " Operations / Executive ",
      query: "  High Risk ",
      filters: { state: " wa ", severity: " CRITICAL ", empty: "", tags: [" late ", "VIP", "late"] },
      bookmark: { owner: " Ada ", shared: "TRUE" },
      dateRange: { from: "2026/05/01", to: "2026-05-31" },
      page: "04",
    }),
    {
      route: "/operations/executive",
      query: "high risk",
      filters: { severity: "critical", state: "WA", tags: ["late", "vip"] },
      bookmark: { owner: "ada", shared: true },
      dateRange: { from: "2026-05-01", to: "2026-05-31" },
      page: 4,
    },
  );
});

test("workbook export emits deterministic escaped multi-section csv", () => {
  const workbook = exportOperationsWorkbook({
    kpis: [
      { label: "Net sales", value: "$65.36", tone: "neutral" },
      { label: "Tax variance", value: "-$1.13", tone: "critical" },
    ],
    workQueue: [
      { id: "cust-3", label: "fraud, review", priority: 90 },
      { id: "o-1", label: "SLA 4 days late", priority: 98 },
    ],
  });

  assert.equal(
    workbook,
    [
      "[kpis]",
      "label,value,tone",
      "Net sales,$65.36,neutral",
      "Tax variance,-$1.13,critical",
      "",
      "[work_queue]",
      "id,label,priority",
      "o-1,SLA 4 days late,98",
      "cust-3,\\"fraud, review\\",90",
    ].join("\\n"),
  );
});

test("drilldown events dedupe by id and sort by time then domain", () => {
  assert.deepEqual(
    mergeDrilldownEvents([
      { event_id: "b", at: "2026-05-01T09:02:00Z", domain: "shipment", label: "shipped" },
      { event_id: "a", at: "2026-05-01T09:00:00Z", domain: "order", label: "created" },
      { event_id: "dup", at: "2026-05-01T09:03:00Z", domain: "payment", label: "paid" },
    ], [
      { event_id: "dup", at: "2026-05-01T09:04:00Z", domain: "support", label: "duplicate later" },
      { event_id: "c", at: "2026-05-01T09:02:00Z", domain: "payment", label: "authorized" },
    ]),
    [
      { event_id: "a", at: "2026-05-01T09:00:00Z", domain: "order", label: "created" },
      { event_id: "c", at: "2026-05-01T09:02:00Z", domain: "payment", label: "authorized" },
      { event_id: "b", at: "2026-05-01T09:02:00Z", domain: "shipment", label: "shipped" },
      { event_id: "dup", at: "2026-05-01T09:03:00Z", domain: "payment", label: "paid" },
    ],
  );
});

test("action priority combines severity age revenue and confidence", () => {
  assert.equal(scoreActionPriority({ severity: "critical", ageHours: 50, revenue: "900.00", confidence: 0.95 }), 100);
  assert.equal(scoreActionPriority({ severity: "high", ageHours: 12, revenue: "110.00", confidence: 0.6 }), 76);
  assert.equal(scoreActionPriority({ severity: "low", ageHours: 1, revenue: "10.00", confidence: 0.2 }), 18);
});
`,
  )

  await writeText(
    path.join(repoPath, "tests", "test_enterprise_scenario_matrix.py"),
    `from decimal import Decimal
import unittest

from retail_core.enterprise_advanced import (
    allocate_fulfillment_batches,
    build_return_fraud_signals,
    calculate_customer_health_scores,
    compile_sla_breach_report,
    detect_margin_anomalies,
    generate_executive_brief,
    reconcile_tax_jurisdictions,
    validate_privacy_exports,
)


class EnterpriseScenarioMatrixTests(unittest.TestCase):
    def test_health_scores_handle_missing_trends_as_stable(self):
        rows = calculate_customer_health_scores(
            customers=[{"customer_id": "c1", "net_spend": Decimal("50.00"), "open_tickets": 0, "risk_flags": ()}],
            sla_breaches=[],
            fraud_signals=[],
            revenue_trends=[],
        )
        self.assertEqual(rows, [{"customer_id": "c1", "score": 90, "tier": "healthy", "next_action": "monitor", "reasons": ("stable_revenue",)}])

    def test_health_scores_sort_ties_by_customer_id(self):
        rows = calculate_customer_health_scores(
            customers=[
                {"customer_id": "c2", "net_spend": Decimal("10.00"), "open_tickets": 1, "risk_flags": ()},
                {"customer_id": "c1", "net_spend": Decimal("10.00"), "open_tickets": 1, "risk_flags": ()},
            ],
            sla_breaches=[],
            fraud_signals=[],
            revenue_trends=[],
        )
        self.assertEqual([row["customer_id"] for row in rows], ["c1", "c2"])
        self.assertEqual([row["score"] for row in rows], [82, 82])

    def test_allocation_backorders_when_no_warehouse_can_ship(self):
        rows = allocate_fulfillment_batches(
            orders=[{"order_id": "o-9", "priority": "standard", "destination": "AK", "lines": [{"sku": "sku-x", "quantity": 2}]}],
            warehouses=[{"warehouse_id": "sea", "states": ["WA"], "hazmat": False, "stock": {"sku-x": 1}}],
            distance_rank={"AK": ["sea"]},
        )
        self.assertEqual(rows, [{"order_id": "o-9", "warehouse_id": None, "lines": (("sku-x", 2),), "status": "backorder", "split": False}])

    def test_allocation_prefers_exact_stock_before_split(self):
        rows = allocate_fulfillment_batches(
            orders=[{"order_id": "o-4", "priority": "standard", "destination": "WA", "lines": [{"sku": "sku-coffee-1", "quantity": 5}]}],
            warehouses=[
                {"warehouse_id": "sea", "states": ["WA"], "hazmat": False, "stock": {"sku-coffee-1": 3}},
                {"warehouse_id": "boi", "states": ["WA"], "hazmat": False, "stock": {"sku-coffee-1": 5}},
            ],
            distance_rank={"WA": ["sea", "boi"]},
        )
        self.assertEqual(rows, [{"order_id": "o-4", "warehouse_id": "boi", "lines": (("sku-coffee-1", 5),), "status": "allocated", "split": False}])

    def test_tax_reconciliation_unknown_rate_is_flagged_not_crashed(self):
        report = reconcile_tax_jurisdictions(
            orders=[{"order_id": "o-x", "state": "CA", "county": "LA", "city": "Los Angeles", "taxable": Decimal("10.00"), "collected_tax": Decimal("1.00")}],
            rates={},
        )
        self.assertEqual(report["jurisdictions"], [{"jurisdiction": "CA|LA|LOS ANGELES", "orders": 1, "taxable": Decimal("10.00"), "expected_tax": Decimal("0.00"), "collected_tax": Decimal("1.00"), "variance": Decimal("1.00"), "status": "missing_rate"}])
        self.assertEqual(report["total_variance"], Decimal("1.00"))

    def test_margin_anomalies_round_half_up_and_sort_critical_first(self):
        rows = detect_margin_anomalies(
            products=[
                {"sku": "b", "category": "x", "price": Decimal("10.00"), "cost": Decimal("9.00")},
                {"sku": "a", "category": "x", "price": Decimal("10.00"), "cost": Decimal("3.33")},
            ],
            sales=[
                {"sku": "a", "units": 3, "discount": Decimal("0.01"), "refund": Decimal("0.00"), "shipping_subsidy": Decimal("0.00")},
                {"sku": "b", "units": 1, "discount": Decimal("0.00"), "refund": Decimal("0.00"), "shipping_subsidy": Decimal("0.00")},
            ],
            minimum_margin=Decimal("0.35"),
        )
        self.assertEqual(rows[0]["sku"], "b")
        self.assertEqual(rows[0]["severity"], "critical")
        self.assertEqual(rows[1]["margin_rate"], Decimal("0.67"))

    def test_privacy_exports_empty_allowed_fields_rejects_everything(self):
        result = validate_privacy_exports([{"customer_id": "c1", "email": "a@example.com", "net_spend": "1.00"}], allowed_fields=[])
        self.assertEqual(result, {"rows": [{}], "rejected_fields": {"c1": ("customer_id", "email", "net_spend")}})

    def test_privacy_exports_uses_row_index_when_customer_id_is_missing(self):
        result = validate_privacy_exports([{"email": "a@example.com", "net_spend": "1.00"}], allowed_fields=["net_spend"])
        self.assertEqual(result, {"rows": [{"net_spend": "1.00"}], "rejected_fields": {"row-1": ("email",)}})

    def test_fraud_signals_do_not_group_outside_window(self):
        rows = build_return_fraud_signals(
            returns=[
                {"return_id": "r1", "customer_id": "c1", "email": "a@example.com", "address_key": "WA", "sku": "sku-a-1", "returned_at": "2026-05-01"},
                {"return_id": "r2", "customer_id": "c1", "email": "a@example.com", "address_key": "WA", "sku": "sku-a-2", "returned_at": "2026-05-20"},
            ],
            window_days=7,
        )
        self.assertEqual(rows, [{"identity": "a@example.com|WA", "return_count": 2, "sku_families": ("sku-a",), "customer_ids": ("c1",), "risk_score": 40, "reason": "repeat_returns"}])

    def test_sla_report_excludes_on_time_and_missing_shipments_are_critical(self):
        rows = compile_sla_breach_report(
            promises=[
                {"order_id": "late", "customer_id": "c1", "promised_at": "2026-05-01", "order_value": Decimal("10.00")},
                {"order_id": "ontime", "customer_id": "c2", "promised_at": "2026-05-01", "order_value": Decimal("10.00")},
            ],
            shipments=[{"order_id": "ontime", "delivered_at": "2026-05-01", "carrier_exception": ""}],
            escalations=[],
        )
        self.assertEqual(rows, [{"order_id": "late", "customer_id": "c1", "days_late": None, "priority": 100, "reason": "missing_delivery"}])

    def test_executive_brief_handles_empty_inputs(self):
        brief = generate_executive_brief(
            health_scores=[],
            inventory_forecast=[],
            promotion_reconciliation={"net_sales": Decimal("0.00"), "refund_total": Decimal("0.00")},
            sla_breaches=[],
            margin_anomalies=[],
            tax_report={"total_variance": Decimal("0.00")},
        )
        self.assertEqual(brief, {"headline": "0 customers need attention; 0 high urgency SKUs; 0 critical margin anomalies", "risk": {"critical_customers": 0, "watch_customers": 0, "sla_breaches": 0}, "finance": {"net_sales": Decimal("0.00"), "refund_total": Decimal("0.00"), "tax_variance": Decimal("0.00")}, "actions": ()})

    def test_executive_brief_limits_actions_to_four_highest_value_items(self):
        brief = generate_executive_brief(
            health_scores=[{"customer_id": f"c{i}", "tier": "critical", "score": i} for i in range(6)],
            inventory_forecast=[{"sku": "sku-a", "urgency": "high", "recommendation": 2}],
            promotion_reconciliation={"net_sales": Decimal("1.00"), "refund_total": Decimal("0.00")},
            sla_breaches=[{"order_id": "o-1", "priority": 99}],
            margin_anomalies=[{"sku": "sku-b", "severity": "critical"}],
            tax_report={"total_variance": Decimal("0.00")},
        )
        self.assertEqual(len(brief["actions"]), 4)
        self.assertEqual(brief["actions"][0], "review critical customer c0")


if __name__ == "__main__":
    unittest.main()
`,
  )

  await writeText(
    path.join(repoPath, "frontend", "tests", "enterprise_operations_edge_cases.test.mjs"),
    `import test from "node:test";
import assert from "node:assert/strict";

import {
  buildOperationsConsole,
  exportOperationsWorkbook,
  mergeDrilldownEvents,
  normalizeScenarioState,
  scoreActionPriority,
} from "../src/views/enterprise_operations_console.mjs";

test("console handles empty payloads", () => {
  assert.deepEqual(buildOperationsConsole({}), {
    kpis: [
      { label: "Net sales", value: "$0.00", tone: "neutral" },
      { label: "Refunds", value: "$0.00", tone: "neutral" },
      { label: "Tax variance", value: "$0.00", tone: "neutral" },
      { label: "SLA breaches", value: "0", tone: "neutral" },
    ],
    workQueue: [],
    exceptionLanes: { customer: [], fulfillment: [], margin: [] },
    headline: "No executive brief available",
  });
});

test("console work queue sorts by priority then id", () => {
  const state = buildOperationsConsole({
    brief: { finance: {}, risk: {}, actions: [], headline: "x" },
    healthScores: [
      { customer_id: "b", tier: "critical", score: 10, next_action: "call" },
      { customer_id: "a", tier: "critical", score: 10, next_action: "call" },
    ],
    slaBreaches: [],
    marginAnomalies: [],
  });
  assert.deepEqual(state.workQueue.map((item) => item.id), ["a", "b"]);
});

test("scenario state defaults invalid dates and pages", () => {
  assert.deepEqual(
    normalizeScenarioState({ route: "", query: null, filters: { x: null }, bookmark: {}, dateRange: { from: "bad", to: "" }, page: "-4" }),
    { route: "/operations", query: "", filters: {}, bookmark: { owner: "", shared: false }, dateRange: { from: "", to: "" }, page: 1 },
  );
});

test("scenario state lowercases array filters and removes duplicates", () => {
  assert.deepEqual(normalizeScenarioState({ filters: { tags: ["VIP", " vip ", "Risk"] } }).filters, { tags: ["risk", "vip"] });
});

test("workbook export escapes quotes and newlines", () => {
  assert.equal(
    exportOperationsWorkbook({ kpis: [{ label: 'He said "go"', value: "A\\nB", tone: "critical" }], workQueue: [] }),
    ['[kpis]', 'label,value,tone', '"He said ""go""","A\\nB",critical', '', '[work_queue]', 'id,label,priority'].join("\\n"),
  );
});

test("workbook queue is sorted by descending priority", () => {
  assert.equal(
    exportOperationsWorkbook({ kpis: [], workQueue: [{ id: "low", label: "low", priority: 1 }, { id: "high", label: "high", priority: 99 }] }).split("\\n").slice(-2).join("\\n"),
    "high,high,99\\nlow,low,1",
  );
});

test("merge drilldown keeps first duplicate even when later duplicate is earlier time", () => {
  assert.deepEqual(
    mergeDrilldownEvents([{ event_id: "dup", at: "2026-05-02T00:00:00Z", domain: "order", label: "first" }], [{ event_id: "dup", at: "2026-05-01T00:00:00Z", domain: "payment", label: "second" }]),
    [{ event_id: "dup", at: "2026-05-02T00:00:00Z", domain: "order", label: "first" }],
  );
});

test("merge drilldown accepts nested event arrays", () => {
  assert.deepEqual(
    mergeDrilldownEvents([[{ event_id: "a", at: "2026-05-01T00:00:00Z", domain: "order", label: "a" }]], [{ event_id: "b", at: "2026-05-02T00:00:00Z", domain: "payment", label: "b" }]).map((e) => e.event_id),
    ["a", "b"],
  );
});

test("priority scores cap and floor", () => {
  assert.equal(scoreActionPriority({ severity: "critical", ageHours: 999, revenue: "999999.00", confidence: 2 }), 100);
  assert.equal(scoreActionPriority({ severity: "none", ageHours: -1, revenue: "-10.00", confidence: -1 }), 0);
});

test("priority handles decimal revenue strings", () => {
  assert.equal(scoreActionPriority({ severity: "medium", ageHours: 24, revenue: "249.99", confidence: 0.75 }), 65);
});
`,
  )

  await writeEnterpriseMegaExpansion(repoPath)
}

async function writeEnterpriseMegaExpansion(repoPath) {
  const backendRequirements = [
    ["prioritize_revenue_recovery", "finance"],
    ["score_customer_save_queue", "customer"],
    ["rank_inventory_exceptions", "inventory"],
    ["plan_vendor_recovery", "vendor"],
    ["triage_payment_risk", "payments"],
    ["summarize_channel_leakage", "channels"],
    ["allocate_service_credits", "support"],
    ["rank_store_restocking", "stores"],
    ["prioritize_fraud_reviews", "fraud"],
    ["score_tax_variance_queue", "tax"],
    ["build_carrier_exception_queue", "carrier"],
    ["rank_margin_repair_work", "margin"],
    ["plan_workforce_interventions", "labor"],
    ["prioritize_subscription_saves", "subscriptions"],
    ["score_loyalty_recovery", "loyalty"],
    ["rank_price_audit_items", "pricing"],
    ["triage_return_quality_issues", "returns"],
    ["plan_procurement_risk", "procurement"],
    ["score_marketplace_exceptions", "marketplace"],
    ["rank_export_compliance_work", "compliance"],
    ["prioritize_data_quality_repairs", "data_quality"],
    ["score_promotion_overrides", "promotions"],
    ["rank_reconciliation_backlog", "reconciliation"],
    ["plan_executive_attention", "executive"],
  ]
  const frontendRequirements = [
    ["buildRevenueRecoveryLane", "finance"],
    ["buildCustomerSaveLane", "customer"],
    ["buildInventoryExceptionLane", "inventory"],
    ["buildVendorRecoveryLane", "vendor"],
    ["buildPaymentRiskLane", "payments"],
    ["buildChannelLeakageLane", "channels"],
    ["buildServiceCreditLane", "support"],
    ["buildStoreRestockingLane", "stores"],
    ["buildFraudReviewLane", "fraud"],
    ["buildTaxVarianceLane", "tax"],
    ["buildCarrierExceptionLane", "carrier"],
    ["buildMarginRepairLane", "margin"],
    ["buildWorkforceInterventionLane", "labor"],
    ["buildSubscriptionSaveLane", "subscriptions"],
    ["buildLoyaltyRecoveryLane", "loyalty"],
    ["buildPriceAuditLane", "pricing"],
    ["buildReturnQualityLane", "returns"],
    ["buildProcurementRiskLane", "procurement"],
    ["buildMarketplaceExceptionLane", "marketplace"],
    ["buildExportComplianceLane", "compliance"],
    ["buildDataQualityRepairLane", "data_quality"],
    ["buildPromotionOverrideLane", "promotions"],
    ["buildReconciliationBacklogLane", "reconciliation"],
    ["buildExecutiveAttentionLane", "executive"],
  ]

  await writeText(
    path.join(repoPath, "docs", "enterprise_control_plane_requirements.md"),
    `# Enterprise Control Plane Requirements

This file intentionally expands the long-task benchmark with many independent
but similarly shaped production requirements. The implementation should factor
shared normalization, scoring, sorting, and empty-state behavior instead of
copying brittle one-off shortcuts.

Backend module:

- Provide \`retail_core.enterprise_control\` with these control-plane functions:
${backendRequirements.map(([name, owner]) => `  - \`${name}\`: normalize work items, compute Decimal scores, assign tiers, de-duplicate tags, set owner \`${owner}\`, filter items below threshold, and sort by score descending then id.`).join("\n")}

Frontend module:

- Provide \`frontend/src/views/enterprise_control_tower.mjs\` with these lane builders:
${frontendRequirements.map(([name, owner]) => `  - \`${name}\`: normalize work items, compute numeric scores, assign tones, de-duplicate tags, set owner \`${owner}\`, filter items below threshold, and sort by score descending then id.`).join("\n")}

Validation:

- \`tests/test_enterprise_control_plane_generated.py\` has two acceptance tests
  for every backend function.
- \`frontend/tests/enterprise_control_tower_generated.test.mjs\` has two
  acceptance tests for every frontend function.
`,
  )

  const backendImports = backendRequirements.map(([name]) => `    ${name},`).join("\n")
  const backendTests = backendRequirements
    .map(([name, owner], index) => {
      const threshold = 12 + (index % 5)
      const highScore = 38 + (index % 7)
      const watchScore = 18 + (index % 4)
      return `
    def test_${name}_scores_filters_and_sorts(self):
        rows = ${name}(
            [
                {"id": "low-${index}", "value": Decimal("2.00"), "weight": 1, "severity": "low", "confidence": Decimal("0.20"), "tags": [" skip "]},
                {"id": "b-${index}", "value": Decimal("${10 + (index % 3)}.00"), "weight": 3, "severity": "high", "confidence": Decimal("0.80"), "tags": [" Late ", "VIP", "late"]},
                {"id": "a-${index}", "value": Decimal("${5 + (index % 2)}.00"), "weight": 2, "severity": "medium", "confidence": Decimal("0.50"), "tags": [" New ", "new"]},
            ],
            threshold=Decimal("${threshold}.00"),
        )
        self.assertEqual(
            rows,
            [
                {"id": "b-${index}", "score": Decimal("${highScore}.00"), "tier": "critical", "tags": ("late", "vip"), "owner": "${owner}"},
                {"id": "a-${index}", "score": Decimal("${watchScore}.00"), "tier": "watch", "tags": ("new",), "owner": "${owner}"},
            ],
        )

    def test_${name}_empty_input(self):
        self.assertEqual(${name}([], threshold=Decimal("${threshold}.00")), [])
`
    })
    .join("\n")

  await writeText(
    path.join(repoPath, "tests", "test_enterprise_control_plane_generated.py"),
    `from decimal import Decimal
import unittest

from retail_core.enterprise_control import (
${backendImports}
)


class EnterpriseControlPlaneGeneratedTests(unittest.TestCase):
${backendTests}


if __name__ == "__main__":
    unittest.main()
`,
  )

  const frontendImports = frontendRequirements.map(([name]) => `  ${name},`).join("\n")
  const frontendTests = frontendRequirements
    .map(([name, owner], index) => {
      const threshold = 12 + (index % 5)
      const highScore = 38 + (index % 7)
      const watchScore = 18 + (index % 4)
      return `
test("${name} scores filters and sorts lane items", () => {
  assert.deepEqual(
    ${name}([
      { id: "low-${index}", value: "2.00", weight: 1, severity: "low", confidence: 0.2, tags: [" skip "] },
      { id: "b-${index}", value: "${10 + (index % 3)}.00", weight: 3, severity: "high", confidence: 0.8, tags: [" Late ", "VIP", "late"] },
      { id: "a-${index}", value: "${5 + (index % 2)}.00", weight: 2, severity: "medium", confidence: 0.5, tags: [" New ", "new"] },
    ], { threshold: ${threshold} }),
    [
      { id: "b-${index}", score: ${highScore}, tone: "critical", tags: ["late", "vip"], owner: "${owner}" },
      { id: "a-${index}", score: ${watchScore}, tone: "watch", tags: ["new"], owner: "${owner}" },
    ],
  );
});

test("${name} handles empty lane input", () => {
  assert.deepEqual(${name}([], { threshold: ${threshold} }), []);
});
`
    })
    .join("\n")

  await writeText(
    path.join(repoPath, "frontend", "tests", "enterprise_control_tower_generated.test.mjs"),
    `import test from "node:test";
import assert from "node:assert/strict";

import {
${frontendImports}
} from "../src/views/enterprise_control_tower.mjs";
${frontendTests}
`,
  )
}

async function writeFixture(repoPath) {
  await fs.rm(repoPath, { recursive: true, force: true })
  await fs.mkdir(path.join(repoPath, "src", "retail_core"), { recursive: true })
  await fs.mkdir(path.join(repoPath, "tests"), { recursive: true })
  await fs.mkdir(path.join(repoPath, "tools"), { recursive: true })
  await writeText(
    path.join(repoPath, "README.md"),
    `# Retail Ledger Bug Hunt

This repository is a large, cross-linked full-stack codebase. The Python
backend owns product, inventory, discount, checkout, customer, fulfillment,
accounting, policy, and reporting workflows. The JavaScript frontend owns
formatting, state derivation, dashboard, checkout, and export view logic.

Your task:

1. Fix the implementation so \`tools/verify.ps1\` passes.
2. Keep the public API stable. The tests import from \`retail_core\`.
3. Do not rewrite tests to make them pass.
4. Prefer small, well-reasoned fixes over replacing the whole project.

Useful command:

\`\`\`powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/verify.ps1
\`\`\`

The bugs are intentionally distributed across multiple files. A local fix that
only satisfies one failing assertion will usually expose the next failure.

Difficulty map:

- Some failures are direct unit-level business defects.
- Some failures are integration defects hidden behind a month-end workflow.
- Some supporting modules are intentionally irrelevant, so follow imports and
  behavior rather than scanning the entire tree.

There are at least 100 seeded behavior defects across backend and frontend
code. The repository also contains generated neutral support modules so the
full tree is over 150 files and 30,000 lines. The tests verify public behavior
and reconciliation invariants rather than exact implementation shape.
${enterpriseExpansionEnabled ? `
Enterprise expansion:

- New acceptance tests add backend customer 360, inventory forecasting,
  promotion/return reconciliation, audit timeline, and frontend dashboard
  requirements.
- Read \`docs/enterprise_requirements.md\` and the enterprise test files.
- Implement missing production modules rather than weakening tests.
` : ""}
${hardEnterpriseExpansionEnabled ? `
Hard enterprise expansion:

- Additional acceptance tests add advanced customer health, fulfillment
  allocation, tax reconciliation, margin anomaly detection, privacy exports,
  return fraud signals, SLA breach reporting, executive brief generation, and
  operations-console requirements.
- Mega control-plane acceptance tests add 24 backend workflow functions and 24
  frontend control-tower lane builders.
- Active generated business-code tests scale support, integration, policy,
  frontend view, and frontend shared modules by ${hardActiveScaleMultiplier}x
  and verify every generated module.
- Read \`docs/enterprise_advanced_requirements.md\`,
  \`docs/enterprise_control_plane_requirements.md\`,
  \`tests/test_enterprise_advanced_requirements.py\`, and
  \`frontend/tests/enterprise_operations_console.test.mjs\`.
- These are production feature requirements with many edge-case assertions.
` : ""}
`,
  )
  await writeText(
    path.join(repoPath, "pyproject.toml"),
    `[project]
name = "retail-ledger-bug-hunt"
version = "0.1.0"
requires-python = ">=3.10"

[tool.pytest.ini_options]
pythonpath = ["src"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "__init__.py"),
    `from .orders import CheckoutRequest, OrderLine, price_order
from .reports import build_daily_report

__all__ = [
    "CheckoutRequest",
    "OrderLine",
    "price_order",
    "build_daily_report",
]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "models.py"),
    `from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal, ROUND_HALF_UP


CENT = Decimal("0.01")


def normalize_sku(value: str) -> str:
    """Return the canonical SKU key used by catalog and inventory maps."""
    # sometimes underscores interchangeably.
    return value.strip().lower()


def money(value: Decimal | int | str) -> Decimal:
    return Decimal(value).quantize(CENT, rounding=ROUND_HALF_UP)


@dataclass(frozen=True)
class Product:
    sku: str
    name: str
    category: str
    unit_price: Decimal
    taxable: bool = True


@dataclass(frozen=True)
class InventoryRecord:
    sku: str
    on_hand: int
    reserved: int = 0


@dataclass(frozen=True)
class PricedLine:
    sku: str
    name: str
    category: str
    quantity: int
    unit_price: Decimal
    line_total: Decimal
    taxable: bool
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "data.py"),
    `from __future__ import annotations

from decimal import Decimal

from .models import InventoryRecord, Product


CATALOG = {
    "sku-coffee-1": Product("sku-coffee-1", "Northwind Coffee", "grocery", Decimal("12.99"), True),
    "sku-mug-2": Product("sku-mug-2", "Ceramic Mug", "home", Decimal("25.00"), True),
    "sku-sticker-3": Product("sku-sticker-3", "Logo Sticker", "merch", Decimal("3.00"), False),
}


INVENTORY = {
    "sku-coffee-1": InventoryRecord("sku-coffee-1", on_hand=10, reserved=2),
    "sku-mug-2": InventoryRecord("sku-mug-2", on_hand=3, reserved=1),
    "sku-sticker-3": InventoryRecord("sku-sticker-3", on_hand=50, reserved=0),
}


COUPONS = {
    "SAVE10": {"kind": "percent", "value": 10},
    "HOME5": {"kind": "fixed", "value": "5.00", "category": "home"},
}


TAX_RATES = {
    "WA": "0.085",
    "OR": "0.000",
    "CA": "0.0725",
}
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "catalog.py"),
    `from __future__ import annotations

from .data import CATALOG
from .models import Product, normalize_sku


def get_product(sku: str) -> Product:
    key = normalize_sku(sku)
    try:
        return CATALOG[key]
    except KeyError as exc:
        raise KeyError(f"unknown sku: {sku}") from exc


def products_for_category(category: str) -> list[Product]:
    expected = category.strip().lower()
    return [product for product in CATALOG.values() if product.category == expected]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "inventory.py"),
    `from __future__ import annotations

from .data import INVENTORY
from .models import normalize_sku


class OutOfStockError(ValueError):
    pass


def available_units(sku: str) -> int:
    record = INVENTORY[normalize_sku(sku)]
    return record.on_hand


def ensure_available(sku: str, quantity: int) -> None:
    if quantity <= 0:
        raise ValueError("quantity must be positive")
    available = available_units(sku)
    if quantity > available:
        raise OutOfStockError(f"{sku} requested {quantity}, only {available} available")
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "discounts.py"),
    `from __future__ import annotations

from decimal import Decimal

from .data import COUPONS
from .models import PricedLine, money


def coupon_discount(lines: list[PricedLine], coupon_code: str | None) -> Decimal:
    if not coupon_code:
        return Decimal("0.00")

    coupon = COUPONS.get(coupon_code.strip().upper())
    if coupon is None:
        raise ValueError(f"unknown coupon: {coupon_code}")

    subtotal = sum((line.line_total for line in lines), Decimal("0.00"))
    if coupon["kind"] == "percent":
        return money(subtotal * Decimal(str(coupon["value"])) / Decimal("1000"))

    if coupon["kind"] == "fixed":
        category = coupon.get("category")
        base = subtotal
        if category:
            base = sum((line.line_total for line in lines if line.category == category), Decimal("0.00"))
        return min(money(Decimal(str(coupon["value"]))), money(base))

    raise ValueError(f"unsupported coupon kind: {coupon['kind']}")


def allocate_discount(lines: list[PricedLine], discount: Decimal) -> dict[str, Decimal]:
    subtotal = sum((line.line_total for line in lines), Decimal("0.00"))
    if subtotal == 0 or discount == 0:
        return {line.sku: Decimal("0.00") for line in lines}
    allocated: dict[str, Decimal] = {}
    running = Decimal("0.00")
    for line in lines[:-1]:
        share = money(discount * line.line_total / subtotal)
        allocated[line.sku] = share
        running += share
    allocated[lines[-1].sku] = money(discount - running)
    return allocated
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "taxes.py"),
    `from __future__ import annotations

from decimal import Decimal

from .data import TAX_RATES
from .models import PricedLine, money


def tax_for_state(state: str, taxable_lines: list[PricedLine], discounts_by_sku: dict[str, Decimal]) -> Decimal:
    rate = Decimal(TAX_RATES[state.strip().upper()])
    taxable_base = Decimal("0.00")
    for line in taxable_lines:
        taxable_base += line.line_total
        # The current code records the discount but does not subtract it.
        discounts_by_sku.get(line.sku, Decimal("0.00"))
    return money(taxable_base * rate)
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "orders.py"),
    `from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal

from .catalog import get_product
from .discounts import allocate_discount, coupon_discount
from .inventory import ensure_available
from .models import PricedLine, money, normalize_sku
from .taxes import tax_for_state


@dataclass(frozen=True)
class OrderLine:
    sku: str
    quantity: int


@dataclass(frozen=True)
class CheckoutRequest:
    lines: list[OrderLine]
    destination_state: str
    coupon_code: str | None = None
    customer_tier: str = "standard"


def _shipping_total(after_discount_subtotal: Decimal, customer_tier: str) -> Decimal:
    if customer_tier.strip().lower() == "vip":
        return Decimal("0.00")
    if after_discount_subtotal > Decimal("50.00"):
        return Decimal("0.00")
    return Decimal("7.99")


def price_order(request: CheckoutRequest) -> dict:
    priced_lines: list[PricedLine] = []
    for raw_line in request.lines:
        product = get_product(raw_line.sku)
        ensure_available(raw_line.sku, raw_line.quantity)
        priced_lines.append(
            PricedLine(
                sku=normalize_sku(product.sku),
                name=product.name,
                category=product.category,
                quantity=raw_line.quantity,
                unit_price=product.unit_price,
                line_total=money(product.unit_price * raw_line.quantity),
                taxable=product.taxable,
            )
        )

    subtotal = money(sum((line.line_total for line in priced_lines), Decimal("0.00")))
    discount = coupon_discount(priced_lines, request.coupon_code)
    discounts_by_sku = allocate_discount(priced_lines, discount)
    taxable_lines = [line for line in priced_lines if line.taxable]
    tax = tax_for_state(request.destination_state, taxable_lines, discounts_by_sku)
    after_discount_subtotal = money(subtotal - discount)
    shipping = _shipping_total(subtotal, request.customer_tier)
    total = money(after_discount_subtotal + tax + shipping)

    return {
        "lines": priced_lines,
        "subtotal": subtotal,
        "discount": discount,
        "tax": tax,
        "shipping": shipping,
        "total": total,
        "destination_state": request.destination_state.strip().upper(),
        "customer_tier": request.customer_tier.strip().lower(),
    }
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "returns.py"),
    `from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal

from .models import money, normalize_sku


@dataclass(frozen=True)
class ReturnLine:
    sku: str
    quantity: int


def refund_for_lines(order: dict, returns: list[ReturnLine]) -> Decimal:
    by_sku = {normalize_sku(line.sku): line for line in order["lines"]}
    subtotal = Decimal("0.00")
    for returned in returns:
        line = by_sku[normalize_sku(returned.sku)]
        if returned.quantity > line.quantity:
            raise ValueError("cannot return more units than were ordered")
        subtotal += line.unit_price * returned.quantity
    ratio = subtotal / order["subtotal"] if order["subtotal"] else Decimal("0.00")
    return money(subtotal + order["shipping"] * ratio)
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "reports.py"),
    `from __future__ import annotations

from collections import defaultdict
from decimal import Decimal

from .models import money


def build_daily_report(orders: list[dict], refunds: list[Decimal] | None = None) -> dict:
    refunds = refunds or []
    category_revenue = defaultdict(lambda: Decimal("0.00"))
    gross_sales = Decimal("0.00")

    for order in orders:
        gross_sales += order["total"]
        for line in order["lines"]:
            # and tax, excluding shipping. This raw line sum ignores discounts/tax.
            category_revenue[line.category] += line.line_total

    refund_total = sum(refunds, Decimal("0.00"))
    return {
        "order_count": len(orders),
        "gross_sales": money(gross_sales),
        "refund_total": money(refund_total),
        "net_sales": money(gross_sales + refund_total),
        "category_revenue": {key: money(value) for key, value in sorted(category_revenue.items())},
    }
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "addresses.py"),
    `from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Address:
    name: str
    line1: str
    city: str
    state: str
    postal_code: str


def normalize_state(value: str) -> str:
    return value.strip()


def normalize_postal(value: str) -> str:
    return value.strip()


def is_po_box(address: Address) -> bool:
    compact = address.line1.strip().lower()
    return compact.startswith("po box")


def shipping_zone(address: Address) -> str:
    state = address.state
    if state in {"WA", "OR", "CA"}:
        return "west"
    if state in {"NY", "MA", "PA"}:
        return "east"
    return "central"
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "payments.py"),
    `from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal

from .models import money


@dataclass(frozen=True)
class PaymentMethod:
    card_number: str
    exp_month: int
    exp_year: int
    network: str = "visa"


def mask_card(card_number: str) -> str:
    digits = "".join(ch for ch in card_number if ch.isdigit())
    if len(digits) < 4:
        raise ValueError("card number must contain at least four digits")
    return f"**** **** **** {digits[:4]}"


def is_expired(method: PaymentMethod, *, current_month: int, current_year: int) -> bool:
    return (method.exp_year, method.exp_month) <= (current_year, current_month)


def authorize_payment(order: dict, method: PaymentMethod, *, available_credit: Decimal) -> dict:
    if is_expired(method, current_month=5, current_year=2026):
        return {"approved": False, "reason": "expired", "amount": money("0.00"), "card": mask_card(method.card_number)}
    amount = money(order["subtotal"])
    if amount > money(available_credit):
        return {"approved": False, "reason": "insufficient_credit", "amount": amount, "card": mask_card(method.card_number)}
    return {"approved": True, "reason": "approved", "amount": amount, "card": mask_card(method.card_number)}
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "loyalty.py"),
    `from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal

from .models import money


@dataclass(frozen=True)
class LoyaltyAccount:
    customer_id: str
    tier: str
    points: int = 0


def points_earned(order: dict, account: LoyaltyAccount) -> int:
    base = int(order["subtotal"])
    multiplier = 1
    if account.tier.strip().lower() == "vip":
        multiplier = 1
    return base * multiplier


def apply_points_credit(order: dict, account: LoyaltyAccount, points: int) -> Decimal:
    if points < 0:
        raise ValueError("points must be positive")
    usable = min(points, account.points)
    credit = money(Decimal(usable) / Decimal("10"))
    return min(credit, money(order["total"]))


def next_tier(account: LoyaltyAccount, lifetime_points: int) -> str:
    if lifetime_points >= 1000:
        return "vip"
    if lifetime_points > 500:
        return "gold"
    return account.tier.strip().lower()
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "fulfillment.py"),
    `from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass

from .addresses import Address, is_po_box, shipping_zone
from .models import normalize_sku


@dataclass(frozen=True)
class Shipment:
    carrier: str
    zone: str
    skus: tuple[str, ...]


def carrier_for(address: Address, *, expedited: bool = False) -> str:
    if is_po_box(address) and expedited:
        return "USPS Priority"
    if is_po_box(address):
        return "USPS Ground"
    return "UPS Ground"


def build_pick_list(order: dict) -> dict[str, int]:
    picks: dict[str, int] = {}
    for line in order["lines"]:
        picks[line.sku] = line.quantity
    return picks


def split_shipments(order: dict, address: Address) -> list[Shipment]:
    grouped: dict[str, list[str]] = defaultdict(list)
    for line in order["lines"]:
        grouped[line.category].append(normalize_sku(line.sku))
    return [
        Shipment(carrier=carrier_for(address), zone=zone, skus=tuple(sorted(skus)))
        for zone, skus in sorted(grouped.items())
    ]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "ledger.py"),
    `from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal

from .models import money


@dataclass(frozen=True)
class LedgerEntry:
    account: str
    debit: Decimal
    credit: Decimal


def post_order_entries(order: dict) -> list[LedgerEntry]:
    entries = [
        LedgerEntry("cash", money(order["total"]), Decimal("0.00")),
        LedgerEntry("discounts", Decimal("0.00"), money(order["discount"])),
        LedgerEntry("sales", Decimal("0.00"), money(order["subtotal"])),
        LedgerEntry("tax_payable", Decimal("0.00"), money(order["tax"])),
        LedgerEntry("shipping_income", Decimal("0.00"), money(order["shipping"])),
    ]
    return [entry for entry in entries if entry.debit or entry.credit]


def post_refund_entries(refund: Decimal) -> list[LedgerEntry]:
    amount = money(refund)
    return [
        LedgerEntry("returns", Decimal("0.00"), amount),
        LedgerEntry("cash", amount, Decimal("0.00")),
    ]


def is_balanced(entries: list[LedgerEntry]) -> bool:
    debits = sum((entry.debit for entry in entries), Decimal("0.00"))
    credits = sum((entry.credit for entry in entries), Decimal("0.00"))
    return money(debits) == money(credits)
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "importers.py"),
    `from __future__ import annotations

import csv
from decimal import Decimal
from io import StringIO

from .addresses import Address
from .orders import CheckoutRequest, OrderLine


def parse_order_rows(text: str) -> list[CheckoutRequest]:
    rows = csv.DictReader(StringIO(text))
    requests: list[CheckoutRequest] = []
    for row in rows:
        sku = row["sku"]
        quantity = int(row["quantity"])
        requests.append(
            CheckoutRequest(
                lines=[OrderLine(sku, quantity)],
                destination_state=row["state"],
                coupon_code=row.get("coupon"),
                customer_tier=row.get("tier", "standard"),
            )
        )
    return requests


def parse_address(row: dict[str, str]) -> Address:
    return Address(
        name=row["name"],
        line1=row["line1"],
        city=row["city"],
        state=row["state"],
        postal_code=row["postal_code"],
    )


def parse_money(value: str) -> Decimal:
    return Decimal(value)
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "operations.py"),
    `from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal

from .addresses import normalize_postal, normalize_state
from .models import money, normalize_sku


@dataclass(frozen=True)
class CustomerRecord:
    customer_id: str
    email: str
    state: str
    postal_code: str
    updated_at: int


@dataclass(frozen=True)
class ProductRank:
    sku: str
    units: int
    revenue: Decimal


def normalize_email(value: str) -> str:
    return value.strip()


def customer_key(record: CustomerRecord) -> str:
    return f"{record.email}|{record.state}|{record.postal_code}"


def merge_customer_records(records: list[CustomerRecord]) -> dict[str, CustomerRecord]:
    merged: dict[str, CustomerRecord] = {}
    for record in records:
        key = customer_key(record)
        current = merged.get(key)
        if current is None or record.updated_at < current.updated_at:
            merged[key] = record
    return merged


def batch_net_total(gross: Decimal, refunds: list[Decimal]) -> Decimal:
    return money(gross + sum(refunds, Decimal("0.00")))


def aging_bucket(days_open: int) -> str:
    if days_open < 0:
        raise ValueError("days_open must be non-negative")
    if days_open < 30:
        return "current"
    if days_open < 60:
        return "watch"
    return "late"


def reorder_quantity(on_hand: int, reserved: int, target: int) -> int:
    available = on_hand
    return max(target - available, 0)


def safe_divide(numerator: Decimal, denominator: Decimal) -> Decimal:
    if denominator == 0:
        return Decimal("1.00")
    return numerator / denominator


def rank_products(products: list[ProductRank]) -> list[str]:
    ordered = sorted(products, key=lambda item: (item.revenue, item.units, normalize_sku(item.sku)))
    return [normalize_sku(item.sku) for item in ordered]


def parse_bool(value: str | bool | int) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, int):
        return value != 0
    return value == "true"


def compact_tags(tags: list[str]) -> tuple[str, ...]:
    return tuple(tag for tag in tags if tag)
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "identity", "__init__.py"),
    `from .customers import customer_identity_key

__all__ = ["customer_identity_key"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "identity", "customers.py"),
    `from __future__ import annotations

from retail_core.addresses import normalize_postal, normalize_state


def normalize_email(value: str) -> str:
    return value.strip()


def customer_identity_key(email: str, state: str, postal_code: str) -> str:
    return "|".join([normalize_email(email), state.strip(), postal_code.strip()])
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "customers", "__init__.py"),
    `from .segments import segment_for_ltv

__all__ = ["segment_for_ltv"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "customers", "segments.py"),
    `from __future__ import annotations

from decimal import Decimal


def segment_for_ltv(lifetime_value: Decimal) -> str:
    if lifetime_value > Decimal("1000.00"):
        return "vip"
    if lifetime_value > Decimal("250.00"):
        return "priority"
    return "standard"
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "risk", "__init__.py"),
    `from .scoring import risk_flags_for_order

__all__ = ["risk_flags_for_order"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "risk", "scoring.py"),
    `from __future__ import annotations


def risk_flags_for_order(order: dict) -> list[str]:
    flags: list[str] = []
    if order.get("chargebacks", 0) > 1:
        flags.append("repeat_chargeback")
    if order.get("billing_state") != order.get("shipping_state"):
        flags.append("state_mismatch")
    if order.get("total", 0) > 500:
        flags.append("high_value")
    return flags
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "planning", "__init__.py"),
    `from .replenishment import reorder_units

__all__ = ["reorder_units"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "planning", "replenishment.py"),
    `from __future__ import annotations


def reorder_units(on_hand: int, reserved: int, target: int, safety_stock: int = 0) -> int:
    available = on_hand
    return max(target + safety_stock - available, 0)
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "shipping", "__init__.py"),
    `from .sla import promised_ship_days

__all__ = ["promised_ship_days"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "shipping", "sla.py"),
    `from __future__ import annotations

from retail_core.addresses import is_po_box


def promised_ship_days(order: dict) -> int:
    if order.get("tier") == "vip":
        return 2
    if is_po_box(order["address"]):
        return 5
    if order.get("expedited"):
        return 2
    return 4
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "accounting", "__init__.py"),
    `from .revenue import recognized_net_revenue

__all__ = ["recognized_net_revenue"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "accounting", "revenue.py"),
    `from __future__ import annotations

from decimal import Decimal

from retail_core.models import money


def recognized_net_revenue(orders: list[dict]) -> Decimal:
    total = Decimal("0.00")
    for order in orders:
        total += Decimal(str(order.get("gross", "0.00")))
        total += Decimal(str(order.get("refund", "0.00")))
    return money(total)
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "exports", "__init__.py"),
    `from .monthly import export_month_rows

__all__ = ["export_month_rows"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "exports", "monthly.py"),
    `from __future__ import annotations


def export_month_rows(rows: list[dict]) -> list[str]:
    output = ["customer_id,segment,net_revenue"]
    for row in rows:
        output.append(f"{row['customer_id']},{row['segment']},{row['gross']}")
    return output
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "workflows", "__init__.py"),
    `from .month_end import close_month

__all__ = ["close_month"]
`,
  )
  await writeText(
    path.join(repoPath, "src", "retail_core", "workflows", "month_end.py"),
    `from __future__ import annotations

from collections import defaultdict
from decimal import Decimal

from retail_core.accounting import recognized_net_revenue
from retail_core.customers import segment_for_ltv
from retail_core.exports import export_month_rows
from retail_core.identity import customer_identity_key
from retail_core.planning import reorder_units
from retail_core.risk import risk_flags_for_order
from retail_core.shipping import promised_ship_days


def close_month(orders: list[dict], inventory: list[dict]) -> dict:
    customer_keys: set[str] = set()
    customer_rows: dict[str, dict] = {}
    risk_flags: dict[str, list[str]] = {}
    ship_days: dict[str, int] = {}

    for order in orders:
        key = customer_identity_key(order["email"], order["shipping_state"], order["postal_code"])
        customer_keys.add(key)
        customer_rows[key] = {
            "customer_id": order["customer_id"],
            "segment": segment_for_ltv(Decimal(str(order["lifetime_value"]))),
            "gross": Decimal(str(order["gross"])),
        }
        flags = risk_flags_for_order(order)
        if flags:
            risk_flags[order["order_id"]] = flags
        ship_days[order["order_id"]] = promised_ship_days(order)

    reorder = {
        item["sku"]: reorder_units(item["on_hand"], item["reserved"], item["target"], item.get("safety_stock", 0))
        for item in inventory
    }

    return {
        "unique_customers": len(customer_keys),
        "segments": dict(sorted((row["customer_id"], row["segment"]) for row in customer_rows.values())),
        "net_revenue": recognized_net_revenue(orders),
        "risk_flags": risk_flags,
        "ship_days": ship_days,
        "reorder": {sku: qty for sku, qty in sorted(reorder.items()) if qty},
        "export_rows": export_month_rows(list(customer_rows.values())),
    }
`,
  )
  await writeText(
    path.join(repoPath, "tests", "test_checkout.py"),
    `from decimal import Decimal
import unittest

from retail_core.inventory import OutOfStockError, available_units, ensure_available
from retail_core.orders import CheckoutRequest, OrderLine, price_order


class CheckoutFlowTests(unittest.TestCase):
    def test_sku_aliases_coupon_tax_and_shipping_work_together(self):
        order = price_order(
            CheckoutRequest(
                lines=[
                    OrderLine(" SKU COFFEE 1 ", 2),
                    OrderLine("sku_mug_2", 1),
                ],
                destination_state="wa",
                coupon_code="save10",
            )
        )

        self.assertEqual(order["subtotal"], Decimal("50.98"))
        self.assertEqual(order["discount"], Decimal("5.10"))
        self.assertEqual(order["tax"], Decimal("3.90"))
        self.assertEqual(order["shipping"], Decimal("7.99"))
        self.assertEqual(order["total"], Decimal("57.77"))

    def test_free_shipping_threshold_is_inclusive_after_discount(self):
        order = price_order(
            CheckoutRequest(
                lines=[
                    OrderLine("sku-coffee-1", 2),
                    OrderLine("sku-mug-2", 1),
                ],
                destination_state="or",
                coupon_code=None,
            )
        )

        self.assertEqual(order["subtotal"], Decimal("50.98"))
        self.assertEqual(order["shipping"], Decimal("0.00"))
        self.assertEqual(order["total"], Decimal("50.98"))

    def test_reserved_inventory_is_not_available(self):
        self.assertEqual(available_units("SKU-COFFEE-1"), 8)
        ensure_available("sku coffee 1", 8)
        with self.assertRaises(OutOfStockError):
            ensure_available("sku coffee 1", 9)


if __name__ == "__main__":
    unittest.main()
`,
  )
  await writeText(
    path.join(repoPath, "tests", "test_returns_and_reports.py"),
    `from decimal import Decimal
import unittest

from retail_core.orders import CheckoutRequest, OrderLine, price_order
from retail_core.reports import build_daily_report
from retail_core.returns import ReturnLine, refund_for_lines


class ReturnsAndReportsTests(unittest.TestCase):
    def test_refund_prorates_discount_and_tax_but_not_shipping(self):
        order = price_order(
            CheckoutRequest(
                lines=[
                    OrderLine("sku-coffee-1", 2),
                    OrderLine("sku-mug-2", 1),
                ],
                destination_state="wa",
                coupon_code="SAVE10",
            )
        )

        refund = refund_for_lines(order, [ReturnLine("sku mug 2", 1)])
        self.assertEqual(refund, Decimal("24.41"))

    def test_report_reconciles_net_sales_and_categories(self):
        order_1 = price_order(
            CheckoutRequest(
                lines=[
                    OrderLine("sku-coffee-1", 2),
                    OrderLine("sku-mug-2", 1),
                ],
                destination_state="wa",
                coupon_code="SAVE10",
            )
        )
        order_2 = price_order(
            CheckoutRequest(
                lines=[
                    OrderLine("sku-sticker-3", 4),
                    OrderLine("sku-mug-2", 1),
                ],
                destination_state="or",
                coupon_code="HOME5",
                customer_tier="vip",
            )
        )
        refund = refund_for_lines(order_1, [ReturnLine("sku mug 2", 1)])

        report = build_daily_report([order_1, order_2], refunds=[refund])

        self.assertEqual(report["gross_sales"], Decimal("89.77"))
        self.assertEqual(report["refund_total"], Decimal("24.41"))
        self.assertEqual(report["net_sales"], Decimal("65.36"))
        self.assertEqual(
            report["category_revenue"],
            {
                "grocery": Decimal("25.37"),
                "home": Decimal("44.41"),
                "merch": Decimal("12.00"),
            },
        )


if __name__ == "__main__":
    unittest.main()
`,
  )
  await writeText(
    path.join(repoPath, "tests", "test_customer_fulfillment.py"),
    `from decimal import Decimal
import unittest

from retail_core.addresses import Address, is_po_box, normalize_postal, normalize_state, shipping_zone
from retail_core.fulfillment import build_pick_list, carrier_for, split_shipments
from retail_core.ledger import is_balanced, post_order_entries, post_refund_entries
from retail_core.loyalty import LoyaltyAccount, apply_points_credit, next_tier, points_earned
from retail_core.orders import CheckoutRequest, OrderLine, price_order


class CustomerFulfillmentTests(unittest.TestCase):
    def test_address_normalization_zone_and_po_box_detection(self):
        address = Address("Ada", " P.O. Box 42 ", "Seattle", " wa ", " 98101 - 1234 ")

        self.assertEqual(normalize_state(address.state), "WA")
        self.assertEqual(normalize_postal(address.postal_code), "98101-1234")
        self.assertTrue(is_po_box(address))
        self.assertEqual(shipping_zone(address), "west")

    def test_fulfillment_uses_expedited_carrier_and_aggregates_pick_list(self):
        address = Address("Ada", "100 Market St", "Seattle", "WA", "98101")
        order = price_order(
            CheckoutRequest(
                lines=[
                    OrderLine("sku-coffee-1", 2),
                    OrderLine("sku coffee 1", 3),
                    OrderLine("sku-sticker-3", 2),
                ],
                destination_state="or",
                customer_tier="vip",
            )
        )

        self.assertEqual(carrier_for(address, expedited=True), "UPS Air")
        self.assertEqual(build_pick_list(order), {"sku-coffee-1": 5, "sku-sticker-3": 2})
        shipments = split_shipments(order, address)
        self.assertEqual(len(shipments), 1)
        self.assertEqual(shipments[0].zone, "west")
        self.assertEqual(shipments[0].skus, ("sku-coffee-1", "sku-coffee-1", "sku-sticker-3"))

    def test_loyalty_points_credit_and_next_tier(self):
        order = price_order(
            CheckoutRequest(
                lines=[OrderLine("sku-mug-2", 1), OrderLine("sku-sticker-3", 2)],
                destination_state="or",
                customer_tier="vip",
            )
        )
        account = LoyaltyAccount("cust-1", "vip", points=750)

        self.assertEqual(points_earned(order, account), 62)
        self.assertEqual(apply_points_credit(order, account, 250), Decimal("2.50"))
        self.assertEqual(next_tier(account, 500), "gold")
        self.assertEqual(next_tier(account, 1000), "vip")

    def test_ledger_entries_balance_order_and_refund(self):
        order = price_order(
            CheckoutRequest(
                lines=[OrderLine("sku-coffee-1", 2), OrderLine("sku-mug-2", 1)],
                destination_state="wa",
                coupon_code="SAVE10",
            )
        )

        order_entries = post_order_entries(order)
        self.assertTrue(is_balanced(order_entries))
        self.assertEqual(
            [(entry.account, entry.debit, entry.credit) for entry in order_entries],
            [
                ("cash", Decimal("57.77"), Decimal("0.00")),
                ("discounts", Decimal("5.10"), Decimal("0.00")),
                ("sales", Decimal("0.00"), Decimal("50.98")),
                ("tax_payable", Decimal("0.00"), Decimal("3.90")),
                ("shipping_income", Decimal("0.00"), Decimal("7.99")),
            ],
        )

        refund_entries = post_refund_entries(Decimal("24.41"))
        self.assertTrue(is_balanced(refund_entries))
        self.assertEqual(
            [(entry.account, entry.debit, entry.credit) for entry in refund_entries],
            [
                ("returns", Decimal("24.41"), Decimal("0.00")),
                ("cash", Decimal("0.00"), Decimal("24.41")),
            ],
        )


if __name__ == "__main__":
    unittest.main()
`,
  )
  await writeText(
    path.join(repoPath, "tests", "test_imports_and_payments.py"),
    `from decimal import Decimal
import unittest

from retail_core.addresses import Address
from retail_core.importers import parse_address, parse_money, parse_order_rows
from retail_core.orders import CheckoutRequest, OrderLine, price_order
from retail_core.payments import PaymentMethod, authorize_payment, is_expired, mask_card


class ImportsAndPaymentsTests(unittest.TestCase):
    def test_parse_order_rows_cleans_values_and_skips_blank_rows(self):
        text = """sku,quantity,state,coupon,tier
 SKU COFFEE 1 ,2, wa , save10 , standard
,,,,
sku_mug_2,1,or,,vip
"""

        requests = parse_order_rows(text)

        self.assertEqual(len(requests), 2)
        self.assertEqual(requests[0], CheckoutRequest([OrderLine("SKU COFFEE 1", 2)], "wa", "save10", "standard"))
        self.assertEqual(requests[1], CheckoutRequest([OrderLine("sku_mug_2", 1)], "or", None, "vip"))

    def test_parse_address_and_money_from_exports(self):
        address = parse_address(
            {
                "name": " Grace Hopper ",
                "line1": " 1 Navy Way ",
                "city": " Arlington ",
                "state": " va ",
                "postal_code": " 22201 ",
            }
        )

        self.assertEqual(address, Address("Grace Hopper", "1 Navy Way", "Arlington", "VA", "22201"))
        self.assertEqual(parse_money("$1,234.50"), Decimal("1234.50"))
        self.assertEqual(parse_money(" 19.99 "), Decimal("19.99"))

    def test_payment_masks_last_four_expiry_boundary_and_authorizes_total(self):
        method = PaymentMethod("4111 1111 1111 9876", exp_month=5, exp_year=2026)
        order = price_order(
            CheckoutRequest(
                lines=[OrderLine("sku-coffee-1", 2), OrderLine("sku-mug-2", 1)],
                destination_state="wa",
                coupon_code="SAVE10",
            )
        )

        self.assertEqual(mask_card(method.card_number), "**** **** **** 9876")
        self.assertFalse(is_expired(method, current_month=5, current_year=2026))
        self.assertTrue(is_expired(method, current_month=6, current_year=2026))

        declined = authorize_payment(order, method, available_credit=Decimal("55.00"))
        self.assertFalse(declined["approved"])
        self.assertEqual(declined["reason"], "insufficient_credit")
        self.assertEqual(declined["amount"], Decimal("57.77"))

        approved = authorize_payment(order, method, available_credit=Decimal("60.00"))
        self.assertTrue(approved["approved"])
        self.assertEqual(approved["amount"], Decimal("57.77"))
        self.assertEqual(approved["card"], "**** **** **** 9876")


if __name__ == "__main__":
    unittest.main()
`,
  )
  await writeText(
    path.join(repoPath, "tests", "test_operations.py"),
    `from decimal import Decimal
import unittest

from retail_core.operations import (
    CustomerRecord,
    ProductRank,
    aging_bucket,
    batch_net_total,
    compact_tags,
    customer_key,
    merge_customer_records,
    normalize_email,
    parse_bool,
    rank_products,
    reorder_quantity,
    safe_divide,
)


class OperationsTests(unittest.TestCase):
    def test_customer_identity_and_latest_merge(self):
        older = CustomerRecord("c-1", " Ada@Example.COM ", " wa ", " 98101 - 1234 ", 10)
        newer = CustomerRecord("c-1", "ada@example.com", "WA", "98101-1234", 20)

        self.assertEqual(normalize_email(older.email), "ada@example.com")
        self.assertEqual(customer_key(older), "ada@example.com|WA|98101-1234")
        self.assertEqual(merge_customer_records([older, newer]), {"ada@example.com|WA|98101-1234": newer})

    def test_batch_math_reorder_and_ratios(self):
        self.assertEqual(batch_net_total(Decimal("100.00"), [Decimal("9.99"), Decimal("10.01")]), Decimal("80.00"))
        self.assertEqual(aging_bucket(30), "watch")
        self.assertEqual(aging_bucket(60), "late")
        self.assertEqual(reorder_quantity(on_hand=8, reserved=3, target=10), 5)
        self.assertEqual(safe_divide(Decimal("5.00"), Decimal("0.00")), Decimal("0.00"))

    def test_product_ranking_bool_parsing_and_tag_compaction(self):
        products = [
            ProductRank(" sku mug 2 ", 2, Decimal("50.00")),
            ProductRank("sku-coffee-1", 7, Decimal("50.00")),
            ProductRank("sku-sticker-3", 99, Decimal("12.00")),
        ]

        self.assertEqual(rank_products(products), ["sku-coffee-1", "sku-mug-2", "sku-sticker-3"])
        self.assertTrue(parse_bool(" YES "))
        self.assertTrue(parse_bool("1"))
        self.assertFalse(parse_bool(" no "))
        self.assertEqual(compact_tags(["New", " sale ", "new", "", "Clearance"]), ("clearance", "new", "sale"))


if __name__ == "__main__":
    unittest.main()
`,
  )
  await writeText(
    path.join(repoPath, "tests", "test_month_end_workflow.py"),
    `from decimal import Decimal
import unittest

from retail_core.addresses import Address
from retail_core.workflows import close_month


class MonthEndWorkflowTests(unittest.TestCase):
    def test_month_end_close_reconciles_deep_workflow_outputs(self):
        orders = [
            {
                "order_id": "order-1",
                "customer_id": "cust-1",
                "email": " Ada@Example.COM ",
                "shipping_state": " wa ",
                "billing_state": "WA",
                "postal_code": " 98101 - 1234 ",
                "lifetime_value": "1000.00",
                "gross": "125.00",
                "refund": "25.00",
                "tier": "vip",
                "expedited": False,
                "chargebacks": 0,
                "total": 125,
                "address": Address("Ada", "100 Market", "Seattle", "WA", "98101"),
            },
            {
                "order_id": "order-2",
                "customer_id": "cust-2",
                "email": "grace@example.com",
                "shipping_state": "OR",
                "billing_state": "CA",
                "postal_code": "97035",
                "lifetime_value": "250.00",
                "gross": "200.00",
                "refund": "45.00",
                "tier": "standard",
                "expedited": True,
                "chargebacks": 1,
                "total": 650,
                "address": Address("Grace", "P.O. Box 9", "Lake Oswego", "OR", "97035"),
            },
            {
                "order_id": "order-3",
                "customer_id": "cust-1",
                "email": "ada@example.com",
                "shipping_state": "WA",
                "billing_state": "WA",
                "postal_code": "98101-1234",
                "lifetime_value": "1000.00",
                "gross": "80.00",
                "refund": "10.00",
                "tier": "vip",
                "expedited": False,
                "chargebacks": 0,
                "total": 80,
                "address": Address("Ada", "100 Market", "Seattle", "WA", "98101"),
            },
        ]
        inventory = [
            {"sku": "sku-coffee-1", "on_hand": 8, "reserved": 3, "target": 10, "safety_stock": 0},
            {"sku": "sku-mug-2", "on_hand": 3, "reserved": 1, "target": 3, "safety_stock": 1},
        ]

        summary = close_month(orders, inventory)

        self.assertEqual(summary["unique_customers"], 2)
        self.assertEqual(summary["segments"], {"cust-1": "vip", "cust-2": "priority"})
        self.assertEqual(summary["net_revenue"], Decimal("325.00"))
        self.assertEqual(summary["reorder"], {"sku-coffee-1": 5, "sku-mug-2": 2})
        self.assertEqual(summary["ship_days"], {"order-1": 1, "order-2": 3, "order-3": 1})
        self.assertEqual(
            summary["risk_flags"],
            {
                "order-2": ["state_mismatch", "high_value", "prior_chargeback"],
            },
        )
        self.assertEqual(
            summary["export_rows"],
            [
                "customer_id,segment,net_revenue",
                "cust-1,vip,170.00",
                "cust-2,priority,155.00",
            ],
        )


if __name__ == "__main__":
    unittest.main()
`,
  )
  await writeText(
    path.join(repoPath, "tools", "verify.py"),
    `from __future__ import annotations

import json
import pathlib
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]


def main() -> int:
    python_cmd = [sys.executable, "-m", "unittest", "discover", "-s", "tests", "-v"]
    python_proc = subprocess.run(python_cmd, cwd=ROOT, text=True, capture_output=True)
    node_cmd = ["node", "--test", "frontend/tests/*.test.mjs"]
    node_proc = subprocess.run(node_cmd, cwd=ROOT, text=True, capture_output=True, shell=sys.platform == "win32")
    summary = {
        "ok": python_proc.returncode == 0 and node_proc.returncode == 0,
        "commands": {
            "python": python_cmd,
            "node": node_cmd,
        },
        "returncodes": {
            "python": python_proc.returncode,
            "node": node_proc.returncode,
        },
        "stdout": {
            "python": python_proc.stdout,
            "node": node_proc.stdout,
        },
        "stderr": {
            "python": python_proc.stderr,
            "node": node_proc.stderr,
        },
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
`,
  )
  await writeText(
    path.join(repoPath, "tools", "verify.ps1"),
    `$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Push-Location $root
try {
    $env:PYTHONPATH = (Join-Path $root "src")
    python tools\\verify.py
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
`,
  )
  await writeText(
    path.join(repoPath, "tools", "verify.sh"),
    `#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "\${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
export PYTHONPATH="$root/src"
python tools/verify.py
`,
  )
  await writeBackendPolicyCodebase(repoPath)
  await writeFrontendCodebase(repoPath)
  if (enterpriseExpansionEnabled) {
    await writeEnterpriseExpansion(repoPath)
  }
  await writeNeutralCodebase(repoPath)
  await spawnLogged("git", ["init"], { cwd: repoPath, echo: false, timeoutMs: 20_000 })
  await spawnLogged("git", ["-c", "core.autocrlf=false", "add", "."], { cwd: repoPath, echo: false, timeoutMs: 20_000 })
  await spawnLogged(
    "git",
    ["-c", "core.autocrlf=false", "-c", "user.name=E2E Test", "-c", "user.email=e2e@example.invalid", "commit", "-m", "Initial bug hunt repo"],
    { cwd: repoPath, echo: false, timeoutMs: 20_000 },
  )
}

async function copyBugRepo(source, destination) {
  await fs.rm(destination, { recursive: true, force: true })
  await fs.cp(source, destination, { recursive: true })
}

async function verifyRepo(repoPath, outputPath, shellSurface = "shell_command") {
  const result =
    shellSurface === "bash"
      ? await spawnLogged(bashBinForHost(), ["tools/verify.sh"], {
          cwd: repoPath,
          echo: false,
          timeoutMs: 120_000,
          env: envForShellSurface(shellSurface),
        })
      : await spawnLogged(
          "powershell",
          ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(repoPath, "tools", "verify.ps1")],
          { cwd: repoPath, echo: false, timeoutMs: 120_000 },
        )
  await writeText(outputPath, `${result.stdout}\n--- STDERR ---\n${result.stderr}`)
  return {
    ok: result.status === 0,
    exit_code: result.status,
    wall_ms: result.durationMs,
    output_path: outputPath,
    shell_surface: shellSurface,
    stdout_tail: result.stdout.slice(-2000),
    stderr_tail: result.stderr.slice(-2000),
  }
}

async function digestRepo(repoPath) {
  const files = []
  async function walk(dir) {
    const entries = await fs.readdir(dir, { withFileTypes: true }).catch(() => [])
    for (const entry of entries) {
      if (entry.name === ".git" || entry.name === ".tura" || entry.name === "__pycache__") continue
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) await walk(full)
      else if (entry.isFile()) files.push(full)
    }
  }
  await walk(repoPath)
  const crypto = await import("node:crypto")
  const entries = []
  for (const file of files.sort()) {
    const buffer = await fs.readFile(file)
    entries.push({
      file: path.relative(repoPath, file).replaceAll("\\", "/"),
      sha256: crypto.createHash("sha256").update(buffer).digest("hex"),
      bytes: buffer.length,
    })
  }
  return {
    digest: crypto.createHash("sha256").update(JSON.stringify(entries)).digest("hex"),
    file_count: entries.length,
    sample_entries: entries.slice(0, 20),
  }
}

async function timedOptionalStep(steps, name, timeoutMs, fn) {
  const startedAt = new Date().toISOString()
  const started = performance.now()
  let timer = null
  try {
    const result = await Promise.race([
      fn(),
      new Promise((resolve) => {
        timer = setTimeout(() => resolve({ skipped: true, reason: `${name} timed out after ${timeoutMs}ms` }), timeoutMs)
      }),
    ])
    steps.push({ name, status: "completed", started_at: startedAt, duration_ms: Math.round(performance.now() - started) })
    return result
  } catch (error) {
    steps.push({
      name,
      status: "failed",
      started_at: startedAt,
      duration_ms: Math.round(performance.now() - started),
      error: error.stack || error.message,
    })
    return { skipped: true, reason: error.stack || error.message }
  } finally {
    if (timer) clearTimeout(timer)
  }
}

async function measureRepo(repoPath) {
  let fileCount = 0
  let lineCount = 0
  const byExtension = {}
  async function walk(dir) {
    const entries = await fs.readdir(dir, { withFileTypes: true }).catch(() => [])
    for (const entry of entries) {
      if (entry.name === ".git" || entry.name === ".tura" || entry.name === "__pycache__") continue
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) {
        await walk(full)
      } else if (entry.isFile()) {
        fileCount += 1
        const ext = path.extname(entry.name).toLowerCase() || "<none>"
        byExtension[ext] = (byExtension[ext] || 0) + 1
        const text = await fs.readFile(full, "utf8").catch(() => "")
        lineCount += text ? text.split(/\r?\n/).length : 0
      }
    }
  }
  await walk(repoPath)
  return {
    path: repoPath,
    file_count: fileCount,
    line_count: lineCount,
    seeded_behavior_defects: seededBehaviorDefectCount,
    by_extension: byExtension,
    meets_requested_scale:
      fileCount >= 150 * fixtureScaleMultiplier &&
      lineCount >= 30_000 * fixtureScaleMultiplier &&
      seededBehaviorDefectCount >= 100 * fixtureScaleMultiplier,
  }
}

async function stopStaleTuraRepoProcesses() {
  if (skipStaleProcessCleanup) return { skipped: true, reason: "COMMAND_RUN_AGENT_SKIP_STALE_PROCESS_CLEANUP=1" }
  if (process.platform !== "win32") return { skipped: true, reason: "non-windows" }
  const escapedRoot = repoRoot.replaceAll("'", "''")
  const script = [
    `$root = (Resolve-Path '${escapedRoot}').Path`,
    "$current = @($PID)",
    "$parent = (Get-CimInstance Win32_Process -Filter \"ProcessId=$PID\" -ErrorAction SilentlyContinue).ParentProcessId",
    "while ($parent) {",
    "  $current += $parent",
    "  $proc = Get-CimInstance Win32_Process -Filter \"ProcessId=$parent\" -ErrorAction SilentlyContinue",
    "  if (-not $proc) { break }",
    "  $parent = $proc.ParentProcessId",
    "}",
    "$candidates = Get-CimInstance Win32_Process | Where-Object {",
    "  $_.ExecutablePath -like \"$root*\" -or $_.CommandLine -like \"*$root*\"",
    "}",
    "$stopped = @()",
    "$candidates | Where-Object { $current -notcontains $_.ProcessId } | ForEach-Object {",
    "  $stopped += [pscustomobject]@{ pid=$_.ProcessId; name=$_.Name; command=$_.CommandLine }",
    "  Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue",
    "}",
    "$stopped | ConvertTo-Json -Compress",
  ].join("\n")
  const result = await spawnLogged("powershell", ["-NoProfile", "-Command", script], {
    cwd: repoRoot,
    echo: false,
    timeoutMs: 20_000,
  })
  return {
    status: result.status,
    duration_ms: result.durationMs,
    stdout: result.stdout.trim(),
    stderr_tail: result.stderr.slice(-1000),
  }
}

async function precompileLocalServices() {
  const script = path.join(repoRoot, "scripts", process.platform === "win32" ? "start.ps1" : "start.sh")
  const command = process.platform === "win32" ? "powershell" : "bash"
  const args =
    process.platform === "win32"
      ? ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-BuildOnly", "-ReleaseServices"]
      : [script]
  const result = await spawnLogged(command, args, {
    cwd: repoRoot,
    timeoutMs: startupTimeoutMs,
    env: {
      ...process.env,
      TURA_BUILD_ONLY: "1",
      TURA_BUILD_RELEASE_SERVICES: "1",
    },
  })
  if (result.status !== 0) throw new Error(`Failed to precompile Tura services:\n${result.stderr || result.stdout}`)
  return {
    status: result.status,
    duration_ms: result.durationMs,
    stdout_tail: result.stdout.slice(-4000),
    stderr_tail: result.stderr.slice(-2000),
  }
}

function emptyLlmStats(source) {
  return {
    source,
    llm_turns: 0,
    provider_turns: 0,
    input_tokens: 0,
    output_tokens: 0,
    reasoning_tokens: 0,
    cached_input_tokens: 0,
    cache_write_tokens: 0,
    total_tokens: 0,
    latency_ms: 0,
    output_tps: null,
    turns: [],
  }
}

function addUsage(stats, usage, index, extra = {}) {
  const input = Number(usage.input_tokens ?? usage.inputTokens ?? usage.prompt_tokens ?? 0)
  const output = Number(usage.output_tokens ?? usage.outputTokens ?? usage.completion_tokens ?? 0)
  const reasoning = Number(
    usage.reasoning_output_tokens ??
      usage.reasoning_tokens ??
      usage.reasoningTokens ??
      usage.completion_tokens_details?.reasoning_tokens ??
      0,
  )
  const cached = Number(
    usage.cached_input_tokens ??
      usage.input_token_details?.cached_tokens ??
      usage.input_tokens_details?.cached_tokens ??
      usage.cachedInputTokens ??
      0,
  )
  const cacheWrite = Number(usage.cache_write_tokens ?? usage.input_token_details?.cache_creation_tokens ?? 0)
  const total = Number(usage.total_tokens ?? usage.totalTokens ?? input + output + reasoning)
  const latency = Number(usage.latency_ms ?? usage.latencyMs ?? 0)
  stats.llm_turns += 1
  stats.input_tokens += input
  stats.output_tokens += output
  stats.reasoning_tokens += reasoning
  stats.cached_input_tokens += cached
  stats.cache_write_tokens += cacheWrite
  stats.total_tokens += total
  stats.latency_ms += latency
  stats.turns.push({
    index,
    input_tokens: input,
    cached_input_tokens: cached,
    cache_hit_ratio: input > 0 ? Number((cached / input).toFixed(4)) : null,
    output_tokens: output,
    reasoning_tokens: reasoning,
    total_tokens: total,
    latency_ms: latency,
    ...extra,
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
        return { raw: line }
      }
    })
}

async function buildCodexBin(root, id) {
  if (!existsSync(path.join(root, "codex-rs", "Cargo.toml"))) {
    throw new Error(`Codex root does not look valid for ${id}: ${root}`)
  }
  const primaryBin = codexBinForRoot(root)
  const candidates = [
    { bin: primaryBin, kind: "codex", used_fallback_binary: false },
    { bin: codexExecBinForRoot(root), kind: "codex-exec", used_fallback_binary: false },
    ...(id === "codex-main"
      ? [
          { bin: codexBinForRoot(codexMainFallbackRoot), kind: "codex", used_fallback_binary: true },
          { bin: codexExecBinForRoot(codexMainFallbackRoot), kind: "codex-exec", used_fallback_binary: true },
        ]
      : []),
  ]
  const existing = candidates.find((candidate) => existsSync(candidate.bin))
  if (existing) {
    return { id, root, ...existing, built: false }
  }
  throw new Error(
    `Compiled Codex binary not found for ${id}. Checked: ${candidates.map((candidate) => candidate.bin).join("; ")}`,
  )
}

function codexLlmStats(stdoutPath, stdout) {
  const stats = emptyLlmStats("codex-jsonl")
  const events = parseJsonl(stdout)
  const modelOutputs = []
  let providerTurns = 0
  let commandStartsSinceLastOutput = 0
  let commandCompletionsSinceLastOutput = 0
  let usageEvents = 0
  events.forEach((event, eventIndex) => {
    const item = event.item || event.payload?.item || {}
    const itemType = item.type || ""
    if (event.type === "turn.started") providerTurns += 1
    if (itemType === "command_execution" && event.type === "item.started") commandStartsSinceLastOutput += 1
    if (itemType === "command_execution" && event.type === "item.completed") commandCompletionsSinceLastOutput += 1
    if (itemType === "agent_message") {
      modelOutputs.push({
        index: modelOutputs.length + 1,
        event_index: eventIndex,
        text_excerpt: String(item.text || item.message || "").replace(/\s+/g, " ").slice(0, 500),
        command_starts_since_previous_model_output: commandStartsSinceLastOutput,
        command_completions_since_previous_model_output: commandCompletionsSinceLastOutput,
      })
      commandStartsSinceLastOutput = 0
      commandCompletionsSinceLastOutput = 0
    }
    const usage =
      event.type === "turn.completed"
        ? event.usage
        : event.type === "event_msg" && event.payload?.type === "token_count"
          ? event.payload?.info?.last_token_usage
          : null
    if (!usage) return
    usageEvents += 1
    addUsage(stats, usage, stats.turns.length + 1, { timestamp: event.timestamp })
  })
  stats.provider_usage_events = usageEvents
  stats.provider_usage_turns = stats.turns.length
  stats.provider_turns = providerTurns || stats.turns.length || modelOutputs.length
  stats.model_output_turns = modelOutputs.length
  stats.llm_turns = stats.provider_turns
  stats.codex_turn_accounting_note =
    "provider_turns/llm_turns count turn.started provider cycles. Codex exec can report one aggregated turn.completed usage for multiple provider turns, so provider_usage_turns may be smaller than provider_turns."
  stats.model_outputs = modelOutputs
  stats.trailing_command_starts_after_last_model_output = commandStartsSinceLastOutput
  stats.trailing_command_completions_after_last_model_output = commandCompletionsSinceLastOutput
  stats.stdout_path = stdoutPath
  stats.stdout_jsonl_bytes = Buffer.byteLength(stdout || "", "utf8")
  return stats
}

async function findCodexSessionFileFromStdout(stdout) {
  const threadId = parseJsonl(stdout).find((event) => event.type === "thread.started")?.thread_id
  if (!threadId || !homeDir) return null
  const sessionsRoot = path.join(homeDir, ".codex", "sessions")
  const files = await collectJsonlFiles(sessionsRoot)
  for (const file of files.sort().reverse()) {
    try {
      if ((await fs.readFile(file, "utf8")).includes(threadId)) return file
    } catch {
      // Ignore unreadable session files.
    }
  }
  return null
}

function codexSessionStats(sessionFile) {
  if (!sessionFile || !existsSync(sessionFile)) {
    return { session_path: sessionFile, found: false }
  }
  const text = readFileSync(sessionFile, "utf8")
  const events = parseJsonl(text)
  return {
    session_path: sessionFile,
    found: true,
    event_count: events.length,
    turn_context_count: events.filter((event) => event.type === "turn_context").length,
    task_started_count: events.filter((event) => event.type === "event_msg" && event.payload?.type === "task_started").length,
    response_item_count: events.filter((event) => event.type === "response_item").length,
    token_count_events: events.filter((event) => event.type === "event_msg" && event.payload?.type === "token_count").length,
    context_compaction_items: events.filter((event) => /context_compaction|Compaction|compact/i.test(JSON.stringify(event))).length,
    bytes: Buffer.byteLength(text, "utf8"),
  }
}

function codexToolAnalysis(stdout) {
  const events = parseJsonl(stdout)
  const toolEvents = events.filter((event) => /(function_call|custom_tool_call|tool_call|command_execution|shell_command|apply_patch)/i.test(JSON.stringify(event)))
  const testLike = events.filter((event) => /(tools\\verify|tools\/verify|unittest|pytest|test|check|verify)/i.test(JSON.stringify(event)))
  return {
    event_count: events.length,
    tool_event_count: toolEvents.length,
    test_like_event_count: testLike.length,
  }
}

async function runCodexAgent({ id, root, bin, binKind = "codex", workspace, shellSurface = "shell_command" }) {
  const logs = path.join(runRoot, id)
  await fs.mkdir(logs, { recursive: true })
  const stdoutPath = path.join(logs, "codex.stdout.jsonl")
  const stderrPath = path.join(logs, "codex.stderr.log")
  const progressPath = path.join(logs, "codex.progress.json")
  const lastMessagePath = path.join(logs, "last-message.md")
  const verifyPath = path.join(logs, "verify.txt")
  const started = performance.now()
  const prompt = taskPromptForShell(shellSurface)
  const commonArgs = [
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
  ]
  const args =
    binKind === "codex-exec"
      ? [...commonArgs, prompt]
      : ["exec", ...commonArgs, "--output-last-message", lastMessagePath]
  const result = await spawnLogged(bin, args, {
    cwd: workspace,
    timeoutMs: runtimeTimeoutMs,
    input: binKind === "codex-exec" ? undefined : prompt,
    stdoutPath,
    stderrPath,
    progressPath,
    progressSnapshot: ({ stdout, stderr, status, elapsed_ms }) => ({
      id,
      agent: "codex",
      status,
      elapsed_ms,
      stdout_path: stdoutPath,
      stderr_path: stderrPath,
      llm: codexLlmStats(stdoutPath, stdout),
      tool_analysis: codexToolAnalysis(stdout),
      stderr_tail: String(stderr || "").slice(-3000),
    }),
    env: envForShellSurface(shellSurface),
  })
  await writeText(stdoutPath, result.stdout)
  await writeText(stderrPath, result.stderr)
  const codexSessionPath = await findCodexSessionFileFromStdout(result.stdout)
  const sessionStats = codexSessionStats(codexSessionPath)
  const llmStats = codexLlmStats(stdoutPath, result.stdout)
  if (Number(sessionStats.token_count_events || 0) > Number(llmStats.provider_turns || 0)) {
    llmStats.provider_turns = Number(sessionStats.token_count_events)
    llmStats.llm_turns = llmStats.provider_turns
    llmStats.codex_turn_accounting_note =
      "provider_turns/llm_turns use persisted Codex session token_count events when available; token totals may still be reported as one aggregated turn.completed usage record."
  }
  const verify = await verifyRepo(workspace, verifyPath, shellSurface)
  const toolAnalysis = codexToolAnalysis(result.stdout)
  return {
    id,
    agent: "codex",
    root,
    workspace,
    bin,
    bin_kind: binKind,
    shell_surface: shellSurface,
    ok: result.status === 0 && verify.ok,
    exit_code: result.status,
    verify,
    duration_ms: Math.round(performance.now() - started),
    first_output_ms: result.firstOutputMs,
    stdout_path: stdoutPath,
    stderr_path: stderrPath,
    progress_path: progressPath,
    session_path: codexSessionPath,
    session_stats: sessionStats,
    last_message_path: lastMessagePath,
    stderr_tail: result.stderr.slice(-3000),
    llm: llmStats,
    tool_analysis: toolAnalysis,
    ran_test_like_command: toolAnalysis.test_like_event_count > 0,
    steps: [
      {
        name: "run codex exec",
        status: result.status === 0 ? "completed" : "failed",
        duration_ms: result.durationMs,
        exit_code: result.status,
      },
      {
        name: "verify repaired repo",
        status: verify.ok ? "completed" : "failed",
        duration_ms: verify.wall_ms,
        exit_code: verify.exit_code,
      },
    ],
  }
}

function firstTuraCliRound(stdout, timeoutMs, firstOutputMs) {
  const events = parseJsonl(stdout)
  const firstRuntimeEvent = events.find((event) => {
    const itemType = event.item?.type || ""
    return itemType === "command_execution" || itemType === "agent_message"
  })
  if (!firstRuntimeEvent) {
    return {
      ok: false,
      timeout_ms: timeoutMs,
      elapsed_ms: firstOutputMs,
      error: `no runtime output within ${timeoutMs}ms`,
    }
  }
  return {
    ok: true,
    message_index: events.indexOf(firstRuntimeEvent),
    part_index: 0,
    output_tokens: 0,
    input_tokens: 0,
    reasoning_tokens: 0,
    latency_ms: firstOutputMs || 0,
    tool_names: ["command_run"],
    output_is_null: false,
    error: null,
    timeout_ms: timeoutMs,
    elapsed_ms: firstOutputMs || 0,
  }
}

function turaCliToolAnalysis(stdout) {
  const events = parseJsonl(stdout)
  const commandEvents = events.filter((event) => event.item?.type === "command_execution" && event.type === "item.completed")
  const testLike = commandEvents.filter((event) =>
    /(tools\\verify|tools\/verify|unittest|pytest|test|check|verify)/i.test(
      `${event.item?.command || ""} ${event.item?.aggregated_output || ""}`,
    ),
  )
  return {
    event_count: events.length,
    tool_event_count: commandEvents.length,
    test_like_command_count: testLike.length,
    command_run_calls: commandEvents.length > 0 ? 1 : 0,
    command_runs: [
      {
        ok: commandEvents.every((event) => Number(event.item?.exit_code || 0) === 0),
        mode: "cli",
        completed_commands: commandEvents.filter((event) => Number(event.item?.exit_code || 0) === 0).length,
        failed_commands: commandEvents.filter((event) => Number(event.item?.exit_code || 0) !== 0).length,
        count: commandEvents.length,
        commands: commandEvents.map((event, index) => ({
          step: index + 1,
          command: event.item?.command || "command_run",
          command_line: String(event.item?.command || "").slice(0, 300),
        })),
        results: commandEvents.map((event, index) => ({
          index,
          step: index + 1,
          command: event.item?.command || "command_run",
          ok: Number(event.item?.exit_code || 0) === 0,
          exit_code: event.item?.exit_code,
          stdout_tail: String(event.item?.aggregated_output || "").slice(-1200),
          stderr_tail: "",
        })),
      },
    ],
    batch_sizes: [commandEvents.length],
    failed_subcommands: commandEvents.filter((event) => Number(event.item?.exit_code || 0) !== 0).length,
  }
}

function messageContent(message) {
  const content = message?.content
  if (typeof content === "string") return content
  if (Array.isArray(content)) {
    return content
      .map((item) => {
        if (typeof item === "string") return item
        if (typeof item?.text === "string") return item.text
        if (typeof item?.input_text === "string") return item.input_text
        return JSON.stringify(item)
      })
      .join("\n")
  }
  return content == null ? "" : JSON.stringify(content)
}

function markerSequenceFromProviderMessages(messages) {
  const markerSequence = []
  for (const message of messages || []) {
    const role = message?.role || "unknown"
    const content = messageContent(message)
    if (content.startsWith("You are Codex")) markerSequence.push(`${role}:base_instructions`)
    if (content.includes("<permissions instructions>")) markerSequence.push(`${role}:permissions`)
    if (content.includes("<WORKSPACE_SNAPSHOT>") || content.includes("Initial workspace file snapshot")) {
      markerSequence.push(`${role}:workspace_snapshot`)
    }
    if (content.includes("<environment_context>") || content.includes("Dynamic runtime state:")) {
      markerSequence.push(`${role}:environment_context`)
    }
    if (content.includes("E2E bug-fix benchmark") || content.includes("Repository task:")) {
      markerSequence.push(`${role}:task`)
    }
    if (
      message?.type === "function_call_output" ||
      (content.includes('"results"') && content.includes('"command"') && content.includes('"success"'))
    ) {
      markerSequence.push(`${message?.type || role}:command_run_result`)
    }
  }
  return markerSequence
}

function usageFromProviderLog(log) {
  return log?.response?.usage || log?.metrics?.usage || log?.response?.metrics?.usage || null
}

function providerResponseFunctionCalls(response) {
  const calls = []
  const seen = new Set()
  const addCall = (item) => {
    if (!item || (item.type !== "function_call" && item.type !== "function" && item.name !== "command_run")) return
    const key = item.call_id || item.id || JSON.stringify(item).slice(0, 200)
    if (seen.has(key)) return
    seen.add(key)
    calls.push(item)
  }
  for (const item of Array.isArray(response?.output) ? response.output : []) addCall(item)
  for (const item of Array.isArray(response?.tool_calls) ? response.tool_calls : []) {
    const functionCall = item?.function || {}
    addCall({
      type: "function_call",
      id: item.id,
      name: functionCall.name,
      arguments: functionCall.arguments,
    })
  }

  const partials = new Map()
  const latestByOutputIndex = new Map()
  for (const event of Array.isArray(response?.events) ? response.events : []) {
    if (event?.type === "response.output_item.added" && event.item?.type === "function_call") {
      const key = event.item.id || event.item.call_id || String(event.output_index)
      partials.set(key, { ...event.item })
      if (event.output_index !== undefined) latestByOutputIndex.set(event.output_index, key)
    }
    if (event?.type === "response.output_item.done") {
      const key =
        event.item?.id ||
        event.item?.call_id ||
        latestByOutputIndex.get(event.output_index) ||
        String(event.output_index)
      const existing = partials.get(key) || {}
      partials.set(key, {
        ...existing,
        ...event.item,
        type: event.item?.type || existing.type,
        name: event.item?.name || existing.name,
        arguments: event.item?.arguments || existing.arguments,
      })
    }
    if (event?.type === "response.function_call_arguments.delta") {
      const key =
        event.item_id ||
        latestByOutputIndex.get(event.output_index) ||
        String(event.output_index)
      const existing =
        partials.get(key) || {
          id: event.item_id || key,
          type: "function_call",
          name: "command_run",
          arguments: "",
        }
      existing.arguments = `${existing.arguments || ""}${event.delta || ""}`
      partials.set(key, existing)
    }
    if (event?.type === "response.function_call_arguments.done") {
      const key =
        event.item_id ||
        latestByOutputIndex.get(event.output_index) ||
        String(event.output_index)
      const existing =
        partials.get(key) || { id: event.item_id || key, type: "function_call", name: "command_run" }
      existing.arguments = event.arguments
      partials.set(key, existing)
    }
  }
  for (const item of partials.values()) addCall(item)
  for (const candidate of Array.isArray(response?.candidates) ? response.candidates : []) {
    for (const part of Array.isArray(candidate?.content?.parts) ? candidate.content.parts : []) {
      const functionCall = part?.functionCall
      if (!functionCall?.name) continue
      addCall({
        type: "function_call",
        name: functionCall.name,
        id: functionCall.id,
        arguments: functionCall.args || {},
      })
    }
  }
  return calls
}

function parseFunctionArguments(value) {
  if (!value) return null
  if (typeof value === "object") return value
  if (typeof value !== "string") return null
  try {
    return JSON.parse(value)
  } catch {
    return null
  }
}

function providerToolAnalysisFromDiagnostics(diagnostics) {
  const calls = diagnostics?.calls || []
  const commandRunCalls = calls.reduce((total, call) => total + Number(call.command_run_call_count || 0), 0)
  const batchSizes = calls.flatMap((call) => call.command_run_batch_sizes || [])
  const testLike = calls.reduce((total, call) => total + Number(call.test_like_command_count || 0), 0)
  const failedResults = calls.flatMap((call) => call.failed_command_results || [])
  return {
    source: "tura-provider-json",
    event_count: calls.length,
    tool_event_count: commandRunCalls,
    test_like_command_count: testLike,
    command_run_calls: commandRunCalls,
    command_runs: calls
      .filter((call) => call.command_run_call_count > 0)
      .map((call) => ({
        provider_call_index: call.index,
        ok: call.success,
        mode: "provider",
        count: call.command_count,
        batch_sizes: call.command_run_batch_sizes,
        command_names: call.command_names,
        command_line_excerpts: call.command_line_excerpts,
        incoming_result_count: call.incoming_result_count,
        incoming_failed_result_count: call.incoming_failed_result_count,
      })),
    batch_sizes: batchSizes,
    failed_subcommands: failedResults.length,
    failed_command_results: failedResults.slice(-40),
  }
}

function parseCommandRunResultMessages(messages) {
  const parsed = []
  for (const [message_index, message] of (messages || []).entries()) {
    const content =
      message?.type === "function_call_output"
        ? String(message.output || "")
        : messageContent(message)
    if (!content.includes('"results"') || !content.includes('"command"') || !content.includes('"success"')) continue
    let payload
    try {
      payload = JSON.parse(content)
    } catch {
      continue
    }
    if (!Array.isArray(payload?.results)) continue
    for (const [result_index, result] of payload.results.entries()) {
      const output = typeof result?.output === "string" ? result.output : JSON.stringify(result?.output ?? null)
      parsed.push({
        message_index,
        result_index,
        step: result?.step ?? null,
        command: result?.command ?? null,
        success: result?.success === true,
        exit_code: result?.exit_code ?? result?.response?.exit_code ?? null,
        output_excerpt: String(output || "").slice(0, 1200),
      })
    }
  }
  return parsed
}

async function providerLogFiles(root) {
  const files = []
  async function visit(dir) {
    let entries
    try {
      entries = await fs.readdir(dir, { withFileTypes: true })
    } catch {
      return
    }
    await Promise.all(
      entries.map(async (entry) => {
        const full = path.join(dir, entry.name)
        if (entry.isDirectory()) {
          await visit(full)
        } else if (entry.isFile() && entry.name.endsWith(".json")) {
          files.push(full)
        }
      }),
    )
  }
  await visit(root)
  return files
}

async function collectJsonlFiles(root) {
  const files = []
  async function visit(dir) {
    let entries
    try {
      entries = await fs.readdir(dir, { withFileTypes: true })
    } catch {
      return
    }
    await Promise.all(
      entries.map(async (entry) => {
        const full = path.join(dir, entry.name)
        if (entry.isDirectory()) {
          await visit(full)
        } else if (entry.isFile() && entry.name.endsWith(".jsonl")) {
          files.push(full)
        }
      }),
    )
  }
  await visit(root)
  return files
}

async function collectTuraProviderDiagnostics({ workspace, sinceMs }) {
  const providerLogRoot = path.join(turaRoot, "log", "provider")
  const diagnostics = {
    source: "tura-provider-json",
    provider_log_root: providerLogRoot,
    since_ms: sinceMs,
    workspace,
    matched_files: 0,
    provider_call_count: 0,
    command_run_calls: 0,
    command_run_batch_sizes: [],
    command_count: 0,
    test_like_command_count: 0,
    total_duration_ms: 0,
    max_message_chars: 0,
    huge_message_calls: [],
    first_patch_call_index: null,
    first_verify_call_index: null,
    provider_timeout_messages: 0,
    llm: emptyLlmStats("tura-provider-json"),
    calls: [],
  }
  if (!existsSync(providerLogRoot)) return diagnostics

  const files = await providerLogFiles(providerLogRoot)
  const escapedWorkspace = workspace.replaceAll("\\", "\\\\")
  const runNeedles = [workspace, escapedWorkspace].filter(Boolean)
  const candidates = []
  for (const file of files) {
    let stat
    try {
      stat = await fs.stat(file)
    } catch {
      continue
    }
    if (stat.mtimeMs + 2_000 < sinceMs) continue
    candidates.push({ file, mtimeMs: stat.mtimeMs })
  }
  candidates.sort((left, right) => left.mtimeMs - right.mtimeMs)

  for (const candidate of candidates) {
    let raw
    try {
      raw = await fs.readFile(candidate.file, "utf8")
    } catch {
      continue
    }
    if (!runNeedles.some((needle) => raw.includes(needle))) continue
    let log
    try {
      log = JSON.parse(raw)
    } catch {
      continue
    }
    const messages = Array.isArray(log?.request?.messages) ? log.request.messages : []
    const commandRunTool = (log?.request?.params?.tools || []).find((tool) => tool?.function?.name === "command_run")
    const commandRunDescription = String(commandRunTool?.function?.description || "")
    const messageCharCounts = messages.map((message) => messageContent(message).length)
    const responseCalls = providerResponseFunctionCalls(log?.response)
    const commandRunCalls = responseCalls.filter((call) => call?.name === "command_run")
    const commandArgs = commandRunCalls.map((call) => parseFunctionArguments(call.arguments)).filter(Boolean)
    const commandGroups = commandArgs.map((args) => (Array.isArray(args.commands) ? args.commands : []))
    const commands = commandGroups.flat()
    const commandNames = commands
      .map((command) => command?.command_type || command?.command)
      .filter(Boolean)
    const commandLineExcerpts = commands.map((command) => String(command?.command_line || "").slice(0, 300))
    const incomingResults = parseCommandRunResultMessages(messages)
    const failedIncomingResults = incomingResults
      .filter((result) => result.success !== true)
      .map((result) => ({
        provider_call_index: diagnostics.calls.length + 1,
        provider_file: candidate.file,
        ...result,
      }))
    const joinedCommands = `${commandNames.join(" ")} ${commandLineExcerpts.join(" ")}`
    const testLikeCommandCount = commands.filter((command) =>
      /(tools\\verify|tools\/verify|unittest|pytest|test|check|verify)/i.test(
        `${command?.command_type || command?.command || ""} ${command?.command_line || ""}`,
      ),
    ).length
    const usage = usageFromProviderLog(log)
    if (usage) {
      addUsage(diagnostics.llm, { ...usage, latency_ms: Number(log.duration_ms || 0) }, diagnostics.llm.turns.length + 1, {
        provider_file: candidate.file,
        success: log.success,
      })
    }
    const call = {
      index: diagnostics.calls.length + 1,
      file: candidate.file,
      success: log.success !== false,
      provider: log.provider,
      model: log.model,
      started_at: log.started_at,
      finished_at: log.finished_at,
      duration_ms: Math.round(Number(log.duration_ms || 0)),
      message_count: messages.length,
      message_char_counts: messageCharCounts,
      total_message_chars: messageCharCounts.reduce((total, count) => total + count, 0),
      max_message_chars: messageCharCounts.reduce((max, count) => Math.max(max, count), 0),
      marker_sequence: markerSequenceFromProviderMessages(messages),
      function_call_count: responseCalls.length,
      command_run_call_count: commandRunCalls.length,
      command_run_batch_sizes: commandGroups.map((group) => group.length),
      command_count: commands.length,
      command_names: commandNames,
      command_line_excerpts: commandLineExcerpts,
      command_run_description_has_bash: /\bbash\b/.test(commandRunDescription),
      command_run_description_has_shell_command: /\bshell_command\b/.test(commandRunDescription),
      command_run_description_excerpt: commandRunDescription.slice(0, 1200),
      incoming_result_count: incomingResults.length,
      incoming_failed_result_count: failedIncomingResults.length,
      failed_command_results: failedIncomingResults,
      test_like_command_count: testLikeCommandCount,
      has_apply_patch: commandNames.includes("apply_patch") || /apply_patch/i.test(joinedCommands),
      has_verify: /(tools\\verify|tools\/verify|verify\.ps1)/i.test(joinedCommands),
      usage: usage
        ? {
            input_tokens: Number(usage.input_tokens || usage.prompt_tokens || 0),
            cached_input_tokens: Number(
              usage.input_tokens_details?.cached_tokens ||
                usage.prompt_tokens_details?.cached_tokens ||
                usage.cached_input_tokens ||
                0,
            ),
            output_tokens: Number(usage.output_tokens || usage.completion_tokens || 0),
            reasoning_tokens: Number(
              usage.output_tokens_details?.reasoning_tokens ||
                usage.completion_tokens_details?.reasoning_tokens ||
                usage.reasoning_output_tokens ||
                0,
            ),
            total_tokens: Number(usage.total_tokens || 0),
          }
        : null,
    }
    diagnostics.calls.push(call)
    diagnostics.matched_files += 1
    diagnostics.provider_call_count += 1
    diagnostics.command_run_calls += commandRunCalls.length
    diagnostics.command_run_batch_sizes.push(...call.command_run_batch_sizes)
    diagnostics.command_count += commands.length
    diagnostics.test_like_command_count += testLikeCommandCount
    diagnostics.failed_command_results = [
      ...(diagnostics.failed_command_results || []),
      ...failedIncomingResults,
    ].slice(-80)
    diagnostics.total_duration_ms += call.duration_ms
    diagnostics.max_message_chars = Math.max(diagnostics.max_message_chars, call.max_message_chars)
    if (call.max_message_chars > 100_000) {
      diagnostics.huge_message_calls.push({
        index: call.index,
        file: call.file,
        max_message_chars: call.max_message_chars,
        input_tokens: call.usage?.input_tokens ?? null,
      })
    }
    if (diagnostics.first_patch_call_index === null && call.has_apply_patch) diagnostics.first_patch_call_index = call.index
    if (diagnostics.first_verify_call_index === null && call.has_verify) diagnostics.first_verify_call_index = call.index
    if (/(timed out|timeout after|request timeout|provider timeout)/i.test(raw)) diagnostics.provider_timeout_messages += 1
  }
  diagnostics.tool_analysis = providerToolAnalysisFromDiagnostics(diagnostics)
  diagnostics.llm.provider_turns = diagnostics.provider_call_count
  diagnostics.llm.llm_turns = diagnostics.provider_call_count
  return diagnostics
}

function firstTuraProviderRound(diagnostics, timeoutMs) {
  const first = diagnostics?.calls?.[0]
  if (!first) {
    return {
      ok: false,
      timeout_ms: timeoutMs,
      elapsed_ms: null,
      error: "no provider call log matched this tura workspace",
    }
  }
  return {
    ok: first.command_run_call_count > 0,
    provider_file: first.file,
    message_count: first.message_count,
    marker_sequence: first.marker_sequence,
    input_tokens: first.usage?.input_tokens ?? 0,
    output_tokens: first.usage?.output_tokens ?? 0,
    reasoning_tokens: first.usage?.reasoning_tokens ?? 0,
    latency_ms: first.duration_ms,
    tool_names: first.command_run_call_count > 0 ? ["command_run"] : [],
    command_run_batch_sizes: first.command_run_batch_sizes,
    output_is_null: first.command_run_call_count === 0,
    error: first.command_run_call_count > 0 ? null : "first provider call did not request command_run",
    timeout_ms: timeoutMs,
    elapsed_ms: first.duration_ms,
  }
}

function turaShellSurfaceContract(diagnostics, expectedSurface) {
  const first = diagnostics?.calls?.find((call) => call.command_run_description_excerpt)
  const hasBash = first?.command_run_description_has_bash === true
  const hasShellCommand = first?.command_run_description_has_shell_command === true
  return {
    ok: expectedSurface === "bash" ? hasBash && !hasShellCommand : hasShellCommand && !hasBash,
    expected_surface: expectedSurface,
    provider_file: first?.file || null,
    command_run_description_has_bash: hasBash,
    command_run_description_has_shell_command: hasShellCommand,
    description_excerpt: first?.command_run_description_excerpt || "",
  }
}

async function runTuraAgent({ id = "tura", workspace, shellSurface = "shell_command" }) {
  const logs = path.join(runRoot, id)
  const steps = []
  await fs.mkdir(logs, { recursive: true })
  const bin = turaBinForRoot(turaRoot)
  const started = performance.now()
  const providerSinceMs = Date.now() - 2_000
  const prompt = taskPromptForShell(shellSurface)
  try {
    if (!existsSync(bin)) throw new Error(`Compiled Tura CLI not found: ${bin}`)
    const stdoutPath = path.join(logs, "tura.stdout.jsonl")
    const stderrPath = path.join(logs, "tura.stderr.log")
    const progressPath = path.join(logs, "tura.progress.json")
    const lastMessagePath = path.join(logs, "last-message.md")
    const messagePath = path.join(logs, "messages.json")
    const verifyPath = path.join(logs, "verify.txt")

    const commonArgs = [
      "exec",
      "--skip-git-repo-check",
      "--json",
      ...(turaMultipleTasksMode(id) ? ["--multiple-tasks-mode"] : []),
      "-C",
      workspace,
      "-m",
      turaModelForAgent(id),
      "--agent-id",
      turaCliAgentName(id),
      ...(turaAccelerationEnabled ? ["-p"] : []),
      "--dangerously-bypass-approvals-and-sandbox",
      "--model-reasoning-effort",
      reasoningEffort,
      "--output-last-message",
      lastMessagePath,
    ]
    const result = await timedStep(steps, "run tura exec", () =>
      spawnLogged(bin, commonArgs, {
        cwd: workspace,
        timeoutMs: runtimeTimeoutMs,
        input: prompt,
        stdoutPath,
        stderrPath,
        progressPath,
        progressSnapshot: ({ stdout, stderr, status, elapsed_ms }) => ({
          id,
          agent: "tura",
          status,
          elapsed_ms,
          stdout_path: stdoutPath,
          stderr_path: stderrPath,
          llm: codexLlmStats(stdoutPath, stdout),
          tool_analysis: turaCliToolAnalysis(stdout),
          stderr_tail: String(stderr || "").slice(-3000),
        }),
        env: {
          ...envForShellSurface(shellSurface),
          TURA_COMMAND_RUN_SHELL: shellSurface,
          TURA_COMMAND_RUN_DISABLE_STRICT_JSON: turaStrictJsonDisabled(id) ? "1" : "0",
        },
      }),
    )
    await writeText(stdoutPath, result.stdout)
    await writeText(stderrPath, result.stderr)
    await writeText(messagePath, JSON.stringify(parseJsonl(result.stdout), null, 2))
    const verify = await verifyRepo(workspace, verifyPath, shellSurface)
    const providerDiagnostics = await collectTuraProviderDiagnostics({ workspace, sinceMs: providerSinceMs })
    const shellSurfaceContract = turaShellSurfaceContract(providerDiagnostics, shellSurface)
    const cliToolAnalysis = turaCliToolAnalysis(result.stdout)
    const providerToolAnalysis = providerDiagnostics.tool_analysis || providerToolAnalysisFromDiagnostics(providerDiagnostics)
    const toolAnalysis =
      cliToolAnalysis.command_run_calls > 0
        ? { ...cliToolAnalysis, provider: providerToolAnalysis }
        : { ...providerToolAnalysis, cli: cliToolAnalysis }
    const firstRound = firstTuraCliRound(result.stdout, firstRoundTimeoutMs, result.firstOutputMs)
    const providerFirstRound = firstTuraProviderRound(providerDiagnostics, firstRoundTimeoutMs)
    const stdoutLlm = codexLlmStats(stdoutPath, result.stdout)
    return {
      id,
      agent: "tura",
      workspace,
      bin,
      bin_kind: "tura",
      shell_surface: shellSurface,
      ok: result.status === 0 && providerFirstRound.ok && verify.ok && shellSurfaceContract.ok,
      wait_error: providerFirstRound.ok ? null : firstRound.error,
      exit_code: result.status,
      verify,
      duration_ms: Math.round(performance.now() - started),
      first_round: firstRound,
      provider_first_round: providerFirstRound,
      session_id: null,
      session_path: null,
      message_path: messagePath,
      message_count: parseJsonl(result.stdout).length,
      stdout_path: stdoutPath,
      stderr_path: stderrPath,
      progress_path: progressPath,
      last_message_path: lastMessagePath,
      stderr_tail: result.stderr.slice(-3000),
      llm: providerDiagnostics.llm.turns.length > 0 ? providerDiagnostics.llm : stdoutLlm,
      stdout_llm: stdoutLlm,
      provider_diagnostics: providerDiagnostics,
      shell_surface_contract: shellSurfaceContract,
      tool_analysis: toolAnalysis,
      ran_test_like_command: toolAnalysis.test_like_command_count > 0 || providerDiagnostics.test_like_command_count > 0,
      steps,
    }
  } catch (error) {
    const providerDiagnostics = await collectTuraProviderDiagnostics({ workspace, sinceMs: providerSinceMs })
    const shellSurfaceContract = turaShellSurfaceContract(providerDiagnostics, shellSurface)
    return {
      id,
      agent: "tura",
      workspace,
      shell_surface: shellSurface,
      ok: false,
      error: error.stack || error.message,
      wait_error: error.stack || error.message,
      duration_ms: Math.round(performance.now() - started),
      first_round: { ok: false, timeout_ms: firstRoundTimeoutMs, error: `agent failed before first round: ${error.message}` },
      provider_first_round: firstTuraProviderRound(providerDiagnostics, firstRoundTimeoutMs),
      session_id: null,
      llm: providerDiagnostics.llm.turns.length > 0 ? providerDiagnostics.llm : emptyLlmStats("failed_agent_run"),
      provider_diagnostics: providerDiagnostics,
      shell_surface_contract: shellSurfaceContract,
      tool_analysis: providerDiagnostics.tool_analysis || { command_run_calls: 0, command_runs: [], batch_sizes: [], test_like_command_count: 0, failed_subcommands: 0 },
      ran_test_like_command: providerDiagnostics.test_like_command_count > 0,
      steps,
    }
  }
}

function aggregateLlm(runs) {
  return runs.reduce(
    (total, run) => {
      const llm = run.llm || {}
      total.llm_turns += Number(llm.llm_turns || 0)
      total.provider_turns += Number(llm.provider_turns || llm.llm_turns || 0)
      total.input_tokens += Number(llm.input_tokens || 0)
      total.output_tokens += Number(llm.output_tokens || 0)
      total.reasoning_tokens += Number(llm.reasoning_tokens || 0)
      total.cached_input_tokens += Number(llm.cached_input_tokens || 0)
      total.cache_write_tokens += Number(llm.cache_write_tokens || 0)
      total.total_tokens += Number(llm.total_tokens || 0)
      total.latency_ms += Number(llm.latency_ms || 0)
      return total
    },
    {
      llm_turns: 0,
      provider_turns: 0,
      input_tokens: 0,
      output_tokens: 0,
      reasoning_tokens: 0,
      cached_input_tokens: 0,
      cache_write_tokens: 0,
      total_tokens: 0,
      latency_ms: 0,
    },
  )
}

function compactContextStats(runs) {
  return Object.fromEntries(
    runs.map((run) => {
      const providerCalls = run.provider_diagnostics?.calls || []
      const providerCompactCalls = providerCalls.filter((call) =>
        (call.command_names || []).includes("compact_context") ||
        (call.command_line_excerpts || []).some((text) => /compact_context/i.test(text)),
      )
      const stdoutEvents = parseJsonl(run.stdout_path ? "" : "")
      return [
        run.id,
        {
          provider_compact_context_calls: providerCompactCalls.length,
          first_provider_compact_call_index: providerCompactCalls[0]?.index ?? null,
          provider_compact_files: providerCompactCalls.map((call) => call.file),
          compact_context_observed:
            providerCompactCalls.length > 0 ||
            /compact_context/i.test(JSON.stringify(run.tool_analysis || {})) ||
            /compact_context/i.test(run.stderr_tail || ""),
          stdout_event_count_hint: stdoutEvents.length,
        },
      ]
    }),
  )
}

function contextCompressionStats(runs) {
  return Object.fromEntries(
    runs.map((run) => {
      const providerCalls = run.provider_diagnostics?.calls || []
      const providerSamples = providerCalls
        .filter((call) => Number(call.max_message_chars || 0) > 0)
        .map((call) => ({
          source: "provider_message_chars",
          index: call.index,
          max_context_chars: Number(call.max_message_chars || 0),
          total_message_chars: Number(call.total_message_chars || 0),
          input_tokens: Number(call.usage?.input_tokens || 0),
        }))
      const usageSamples = (run.llm?.turns || [])
        .filter((turn) => Number(turn.input_tokens || 0) > 0)
        .map((turn) => ({
          source: "llm_input_tokens",
          index: turn.index,
          max_context_chars: null,
          input_tokens: Number(turn.input_tokens || 0),
        }))
      const samples = providerSamples.length > 1 ? providerSamples : usageSamples
      const metric = providerSamples.length > 1 ? "max_context_chars" : "input_tokens"
      const drop = firstCompressionDrop(samples, metric)
      const stdout = readRunStdout(run)
      const textSignal = /(compacted state|compact(?:ed|ion)|handoff summary|context checkpoint|compressed context)/i.test(stdout)
      return [
        run.id,
        {
          observed_by_context_drop: drop.observed,
          metric,
          reason: drop.reason,
          first_drop: drop.first_drop,
          sample_count: samples.length,
          samples: samples.slice(0, 20),
          compact_text_signal_observed: textSignal,
          compact_text_signal_is_size_evidence: false,
        },
      ]
    }),
  )
}

function readRunStdout(run) {
  if (!run?.stdout_path || !existsSync(run.stdout_path)) return ""
  try {
    return readFileSync(run.stdout_path, "utf8")
  } catch {
    return ""
  }
}

function firstCompressionDrop(samples, metric) {
  if (!Array.isArray(samples) || samples.length < 2) {
    return {
      observed: false,
      reason: "insufficient_context_size_samples",
      first_drop: null,
    }
  }
  let previousPeak = Number(samples[0]?.[metric] || 0)
  let previousPeakIndex = samples[0]?.index ?? 1
  for (let offset = 1; offset < samples.length; offset += 1) {
    const current = Number(samples[offset]?.[metric] || 0)
    const threshold = metric === "max_context_chars" ? 200_000 : 100_000
    const absoluteDrop = previousPeak - current
    const ratio = previousPeak > 0 ? current / previousPeak : 1
    if (previousPeak >= threshold && absoluteDrop >= threshold * 0.5 && ratio <= 0.55) {
      return {
        observed: true,
        reason: "large_context_metric_drop",
        first_drop: {
          from_index: previousPeakIndex,
          to_index: samples[offset]?.index ?? offset + 1,
          from: previousPeak,
          to: current,
          drop: absoluteDrop,
          ratio: Number(ratio.toFixed(4)),
        },
      }
    }
    if (current > previousPeak) {
      previousPeak = current
      previousPeakIndex = samples[offset]?.index ?? offset + 1
    }
  }
  return {
    observed: false,
    reason: "no_large_context_metric_drop",
    first_drop: null,
  }
}

function buildSummary({
  started,
  steps,
  workspaces,
  precompile,
  robustness,
  codexBuilds,
  baselineVerify,
  baselineMeasure,
  beforeDigests,
  afterDigests,
  identicalInitialRepos,
  runtimeDurationMs,
  runs,
  summaryStage,
}) {
  const successfulRuns = runs.filter((run) => run.ok).length
  const successfulRepairs = runs.filter((run) => run.verify?.ok).length
  const runsWithTestLikeCommand = runs.filter((run) => run.ran_test_like_command).length
  return {
    ok: identicalInitialRepos && successfulRuns === runs.length && successfulRepairs === runs.length,
    run_id: runId,
    run_root: runRoot,
    summary_path: summaryPath,
    summary_stage: summaryStage,
    prompt: taskPrompt,
    task_prompts: Object.fromEntries(
      requestedAgents.map((id) => [id, taskPromptForShell(agentShellSurface(id))]),
    ),
    phases: [
      "phase 1: generate one failing Python retail bug repo and copy it into requested identical workspaces",
      "phase 1 preflight: run tura command_run robustness checks before long E2E",
      "phase 2: run requested agents on the same repair task",
      "phase 3: verify each repaired repo and collect token/cache/tool logs",
    ],
    model_config: {
      requested_agents: requestedAgents,
      shell_surfaces: Object.fromEntries(requestedAgents.map((id) => [id, agentShellSurface(id)])),
      tura_model: turaModel,
      codex_model: codexModel,
      reasoning_effort: reasoningEffort,
      codex_service_tier: codexServiceTier,
      tura_model_acceleration_enabled: turaAccelerationEnabled,
      priority_mode: false,
    },
    roots: {
      tura_root: turaRoot,
      codex_current_root: codexCurrentRoot,
      codex_main_root: codexMainRoot,
    },
    workspaces,
    timeout_ms: runtimeTimeoutMs,
    first_round_timeout_ms: firstRoundTimeoutMs,
    duration_ms: Math.round(performance.now() - started),
    runtime_duration_ms: runtimeDurationMs,
    startup: { precompile, codex_builds: codexBuilds, gateway_steps: [], gateway_started: false },
    observations: {
      baseline_verify: baselineVerify,
      baseline_scale: baselineMeasure,
      identical_initial_repos: identicalInitialRepos,
      robustness_preflight: robustness,
      successful_runs: successfulRuns,
      successful_repairs: successfulRepairs,
      runs_with_test_like_command: runsWithTestLikeCommand,
      aggregate_llm: aggregateLlm(runs),
      aggregate_command_run_calls: runs.reduce((total, run) => total + Number(run.tool_analysis?.command_run_calls || 0), 0),
      aggregate_codex_tool_events: runs.reduce((total, run) => total + Number(run.tool_analysis?.tool_event_count || 0), 0),
      aggregate_failed_subcommands: runs.reduce((total, run) => total + Number(run.tool_analysis?.failed_subcommands || 0), 0),
      compact_stress: {
        enabled: compactStressEnabled,
        context_full_mode: contextFullModeEnabled,
        fixed_context_token_budget: fixedContextTokenBudget,
        fixture_scale_multiplier: fixtureScaleMultiplier,
        hard_active_generated_code: hardActiveGeneratedCodeEnabled,
        hard_active_scale_multiplier: hardActiveScaleMultiplier,
        compact_context: compactContextStats(runs),
        context_compression: contextCompressionStats(runs),
      },
    },
    before: beforeDigests,
    after: afterDigests,
    steps,
    runs,
  }
}

async function main() {
  const started = performance.now()
  const steps = []
  await timedStep(steps, "phase 1: clear run workspace", clearRunRoot)
  const turaAgents = requestedAgents.filter(isTuraAgent)
  const currentAgents = requestedAgents.filter(isCurrentAgent)
  const wantsTura = turaAgents.length > 0
  const wantsCodexCurrent = currentAgents.length > 0
  const wantsCodexMain = requestedAgents.includes("codex-main")
  const requiredRoots = [
    ...(wantsTura ? [turaRoot] : []),
    ...(wantsCodexCurrent ? [codexCurrentRoot] : []),
    ...(wantsCodexMain ? [codexMainRoot] : []),
  ]
  for (const root of requiredRoots) {
    if (!existsSync(root)) throw new Error(`Required root does not exist: ${root}`)
  }
  await timedStep(steps, "phase 1: stop stale tura repo processes", stopStaleTuraRepoProcesses)
  const robustness =
    wantsTura && robustnessPreflight
      ? await timedStep(steps, "phase 1: run tura command_run robustness preflight", runTuraRobustnessPreflight)
      : { skipped: true, reason: "robustness preflight disabled or no tura agent requested" }
  if (preflightOnly) {
    await writeText(summaryPath, JSON.stringify({
      ok: true,
      run_id: runId,
      run_root: runRoot,
      summary_path: summaryPath,
      summary_stage: "preflight-only",
      duration_ms: Math.round(performance.now() - started),
      observations: {
        robustness_preflight: robustness,
      },
      steps,
    }, null, 2))
    return
  }
  const precompile =
    wantsTura && precompileTura
      ? await timedStep(steps, "phase 1: precompile tura services", precompileLocalServices)
      : { skipped: true, reason: "using already built single Tura CLI exe; set COMMAND_RUN_AGENT_PRECOMPILE_TURA=1 to precompile" }
  const codexBuildTasks = [
    ...(wantsCodexCurrent ? [buildCodexBin(codexCurrentRoot, "codex-current")] : []),
    ...(wantsCodexMain ? [buildCodexBin(codexMainRoot, "codex-main")] : []),
  ]
  const codexBuilds = codexBuildTasks.length
    ? await timedStep(steps, "phase 1: prepare requested codex binaries", () => Promise.all(codexBuildTasks))
    : []
  const baseline = path.join(runRoot, "baseline-repo")
  await timedStep(steps, "phase 1: create baseline python bug repo", () => writeFixture(baseline))
  const baselineVerify = await timedStep(steps, "phase 1: verify baseline fails", () =>
    verifyRepo(baseline, path.join(runRoot, "baseline.verify.txt")),
  )
  if (baselineVerify.ok) throw new Error(`Baseline bug repo unexpectedly passes verification: ${baselineVerify.output_path}`)
  const baselineMeasure = await timedStep(steps, "phase 1: measure baseline repo scale", () => measureRepo(baseline))
  if (!baselineMeasure.meets_requested_scale) {
    throw new Error(
      `Baseline bug repo is smaller than requested scale: ${baselineMeasure.file_count} files, ${baselineMeasure.line_count} lines, ${baselineMeasure.seeded_behavior_defects} seeded behavior defects`,
    )
  }
  const workspaces = Object.fromEntries(
    [
      ...turaAgents.map((id) => [id, path.join(runRoot, `repo-${id}`)]),
      ...currentAgents.map((id) => [id, path.join(runRoot, `repo-${id}`)]),
      wantsCodexMain ? ["codex_main", path.join(runRoot, "repo-codex-main")] : null,
    ].filter(Boolean),
  )
  await timedStep(steps, "phase 1: copy identical challenge repos", () =>
    Promise.all(Object.values(workspaces).map((workspace) => copyBugRepo(baseline, workspace))),
  )
  const beforeDigests = await timedStep(steps, "phase 1: digest initial challenge repos", () =>
    Promise.all(Object.values(workspaces).map((workspace) => digestRepo(workspace))),
  )
  const identicalInitialRepos = beforeDigests.every((digest) => digest.digest === beforeDigests[0]?.digest)
  let runs
  const runtimeStarted = performance.now()
  try {
    const codexCurrent = codexBuilds.find((item) => item.id === "codex-current")
    const codexMain = codexBuilds.find((item) => item.id === "codex-main")
    const runTasks = [
      ...turaAgents.map((id) =>
        runTuraAgent({
          id,
          workspace: workspaces[id],
          shellSurface: agentShellSurface(id),
        }),
      ),
      ...currentAgents.map((id) =>
            runCodexAgent({
              id,
              root: codexCurrentRoot,
              workspace: workspaces[id],
              bin: codexCurrent.bin,
              binKind: codexCurrent.kind,
              shellSurface: agentShellSurface(id),
            }),
      ),
      ...(wantsCodexMain
        ? [
            runCodexAgent({
          id: "codex-main",
          root: codexMainRoot,
          workspace: workspaces.codex_main,
          bin: codexMain.bin,
          binKind: codexMain.kind,
            }),
          ]
        : []),
    ]
    runs = await timedStep(steps, "phase 2: run requested agents concurrently", () => Promise.all(runTasks))
  } finally {
    await timedStep(steps, "phase 2 cleanup: stop stale tura repo processes", stopStaleTuraRepoProcesses)
  }
  const runtimeDurationMs = Math.round(performance.now() - runtimeStarted)
  let summary = buildSummary({
    started,
    steps,
    workspaces,
    precompile,
    robustness,
    codexBuilds,
    baselineVerify,
    baselineMeasure,
    beforeDigests,
    afterDigests: { pending: true, reason: "agent verification summary written before optional final digest" },
    identicalInitialRepos,
    runtimeDurationMs,
    runs,
    summaryStage: "post-agent-verification",
  })
  await writeText(summaryPath, JSON.stringify(summary, null, 2))

  const afterDigests = finalDigestEnabled
    ? await timedOptionalStep(steps, "phase 3: digest final challenge repos", finalDigestTimeoutMs, () =>
        Promise.all(Object.values(workspaces).map((workspace) => digestRepo(workspace))),
      )
    : { skipped: true, reason: "final digest disabled by default; set COMMAND_RUN_AGENT_FINAL_DIGEST=1 to collect it" }
  summary = buildSummary({
    started,
    steps,
    workspaces,
    precompile,
    robustness,
    codexBuilds,
    baselineVerify,
    baselineMeasure,
    beforeDigests,
    afterDigests,
    identicalInitialRepos,
    runtimeDurationMs,
    runs,
    summaryStage: "complete",
  })
  await writeText(summaryPath, JSON.stringify(summary, null, 2))
  console.log(`[command-run-agent-three-way] summary: ${summaryPath}`)
  console.log(
    `[command-run-agent-three-way] ok=${summary.ok} successful_repairs=${summary.observations.successful_repairs}/${runs.length} duration_ms=${summary.duration_ms}`,
  )
  if (!summary.ok) process.exitCode = 1
}

main().catch(async (error) => {
  await fs.mkdir(path.dirname(summaryPath), { recursive: true })
  const summary = {
    ok: false,
    run_id: runId,
    run_root: runRoot,
    summary_path: summaryPath,
    error: error.stack || error.message,
  }
  await writeText(summaryPath, JSON.stringify(summary, null, 2))
  console.error(error.stack || error.message)
  process.exit(1)
})

