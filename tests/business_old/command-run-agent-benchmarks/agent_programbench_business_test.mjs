#!/usr/bin/env node
import assert from "node:assert/strict"
import crypto from "node:crypto"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { spawn, spawnSync } from "node:child_process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { endStream, isolatedProcessOptions, killProcessTree } from "./process_helpers.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const programbenchRoot = process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_ROOT || path.join(homeDir, "Documents", "programbench")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `agent-programbench-test-${Date.now()}`
const runRoot = path.join(repoRoot, "target", "agent-programbench-test", runId)
const summaryPath = path.join(runRoot, "summary.json")

const instanceId = process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_INSTANCE_ID || "agourlay__zip-password-finder.704700d"
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 4 * 60_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "tura-planning-shll")
const prepOnly = (process.env.COMMAND_RUN_AGENT_PREP_ONLY || "0") === "1"
const selfTest = (process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_SELF_TEST || "0") === "1"
const runEval = (process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_RUN_EVAL || "0") === "1"
const allowLocalFixture = (process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_ALLOW_LOCAL_FIXTURE || "0") === "1"
const imageTag = process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_IMAGE_TAG || "task_cleanroom"
const dockerOrg = process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_DOCKER_ORG || "programbench"
const planningOverride = parsePlanningOverride(process.env.COMMAND_RUN_AGENT_TURA_PLANNING || "auto")
const codexGoalsEnabled = ["1", "true", "yes", "on", "enabled"].includes(
  String(process.env.COMMAND_RUN_AGENT_CODEX_GOALS || "").trim().toLowerCase()
)
const dockerCpus = process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_DOCKER_CPUS || "8"

const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")
const codexMainExe = findCodexMainExe()

function findCodexMainExe() {
  const exeName = process.platform === "win32" ? "codex.exe" : "codex"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_CODEX_MAIN_ROOT,
    path.join(homeDir, "Documents", "codex-main"),
    path.join(homeDir, "codex-main"),
  ].filter(Boolean).map((root) => path.join(root, "codex-rs", "target", "debug", exeName))
  return candidates.find((candidate) => fs.existsSync(candidate)) || candidates[0]
}

function parseAgents(value) {
  const alias = new Map([
    ["tura", "tura-planning-shll"],
    ["tura-planning", "tura-planning-shll"],
    ["tura-planning-shll", "tura-planning-shll"],
    ["tura-fast", "tura-fast-shll"],
    ["tura-fast-shll", "tura-fast-shll"],
    ["tura-fast-planning", "tura-fast-planning-shll"],
    ["tura-fast-planning-shll", "tura-fast-planning-shll"],
    ["codex-main", "codex-main"],
    ["main", "codex-main"],
  ])
  return String(value).split(",").map((item) => alias.get(item.trim().toLowerCase())).filter(Boolean)
}

function parsePlanningOverride(value) {
  const normalized = String(value || "auto").trim().toLowerCase()
  if (["auto", "default", "agent"].includes(normalized)) return null
  if (["on", "true", "1", "yes", "enabled"].includes(normalized)) return true
  if (["off", "false", "0", "no", "disabled"].includes(normalized)) return false
  throw new Error(`COMMAND_RUN_AGENT_TURA_PLANNING must be auto, on, or off; got ${value}`)
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text)
}

function run(command, args, options = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: options.maxBuffer || 512 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    windowsHide: true,
  })
  return {
    command,
    args,
    status: result.status,
    signal: result.signal,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    duration_ms: Math.round(performance.now() - started),
    error: result.error ? String(result.error.stack || result.error.message || result.error) : null,
  }
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
  }
  return result
}

function runLive(command, args, options = {}) {
  const started = performance.now()
  const stdoutPath = options.stdoutPath
  const stderrPath = options.stderrPath
  const statusPath = options.statusPath
  if (stdoutPath) mkdirp(path.dirname(stdoutPath))
  const stdoutStream = stdoutPath ? fs.createWriteStream(stdoutPath) : null
  const stderrStream = stderrPath ? fs.createWriteStream(stderrPath) : null
  return new Promise((resolve) => {
    let stdout = ""
    let stderr = ""
    let firstOutputMs = null
    let settled = false
    let timedOut = false
    const child = spawn(command, args, isolatedProcessOptions({
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    }))
    if (options.input) {
      child.stdin.write(options.input)
      child.stdin.end()
    }
    const timer = setTimeout(() => {
      timedOut = true
      killProcessTree(child.pid)
      settle(1, null, `timed out after ${options.timeoutMs || timeoutMs}ms`)
    }, options.timeoutMs || timeoutMs)
    function record(kind, chunk) {
      if (firstOutputMs == null) firstOutputMs = Math.round(performance.now() - started)
      const text = chunk.toString()
      if (kind === "stdout") {
        stdout += text
        stdoutStream?.write(text)
      } else {
        stderr += text
        stderrStream?.write(text)
      }
    }
    function settle(status, signal, error = null) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      endStream(stdoutStream)
      endStream(stderrStream)
      const summary = {
        status,
        signal,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        first_output_ms: firstOutputMs,
        timed_out: timedOut,
        error,
        pid: child.pid,
      }
      if (statusPath) writeFile(statusPath, JSON.stringify(summary, null, 2))
      resolve(summary)
    }
    child.stdout?.on("data", (chunk) => record("stdout", chunk))
    child.stderr?.on("data", (chunk) => record("stderr", chunk))
    child.on("error", (error) => settle(null, null, String(error.stack || error.message || error)))
    child.on("close", (status, signal) => settle(status, signal, timedOut ? `timed out after ${options.timeoutMs || timeoutMs}ms` : null))
  })
}

function dockerImageName(id = instanceId) {
  return `${dockerOrg}/${id.replace("__", "_1776_")}:${imageTag}`
}

function loadTaskMetadata(id = instanceId) {
  const taskDir = path.join(programbenchRoot, "src", "programbench", "data", "tasks", id)
  const yamlPath = path.join(taskDir, "task.yaml")
  const testsPath = path.join(taskDir, "tests.json")
  assert(fs.existsSync(yamlPath), `missing ProgramBench task: ${yamlPath}`)
  const taskYaml = fs.readFileSync(yamlPath, "utf8")
  const tests = fs.existsSync(testsPath) ? JSON.parse(fs.readFileSync(testsPath, "utf8")) : { branches: {} }
  const metadata = Object.fromEntries(taskYaml.split(/\r?\n/).map((line) => {
    const index = line.indexOf(":")
    if (index < 0) return null
    return [line.slice(0, index).trim(), line.slice(index + 1).trim()]
  }).filter(Boolean))
  return { id, taskDir, taskYaml, tests, ...metadata, image: dockerImageName(id) }
}

function prepareWorkspace(agentDir, task) {
  const workspace = path.join(agentDir, "workspace")
  mkdirp(agentDir)
  const prepLog = []
  const dockerVersion = run("docker", ["version", "--format", "{{json .}}"], { timeoutMs: 30_000 })
  prepLog.push({ step: "docker_version", ...briefRun(dockerVersion) })
  if (dockerVersion.status === 0) {
    const pull = run("docker", ["pull", task.image], { timeoutMs: Number(process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_PULL_TIMEOUT_MS || 20 * 60_000) })
    prepLog.push({ step: "docker_pull_cleanroom", image: task.image, ...briefRun(pull) })
    if (pull.status === 0) {
      const containerName = `programbench-${process.pid}-${crypto.randomBytes(4).toString("hex")}`
      const create = run("docker", ["create", "--name", containerName, task.image], { timeoutMs: 120_000 })
      prepLog.push({ step: "docker_create", container: containerName, ...briefRun(create) })
      try {
        if (create.status !== 0) throw new Error("docker create failed")
        fs.rmSync(workspace, { recursive: true, force: true })
        mkdirp(workspace)
        const cp = run("docker", ["cp", `${containerName}:/workspace/.`, workspace], { timeoutMs: 5 * 60_000 })
        prepLog.push({ step: "docker_cp_workspace", ...briefRun(cp) })
        if (cp.status !== 0) throw new Error("docker cp failed")
      } finally {
        const rm = run("docker", ["rm", "-f", containerName], { timeoutMs: 60_000 })
        prepLog.push({ step: "docker_rm", container: containerName, ...briefRun(rm) })
      }
      writeProgrambenchNotes(workspace, task)
      initGit(workspace, prepLog)
      writeFile(path.join(agentDir, "prep-log.json"), JSON.stringify(prepLog, null, 2))
      return { workspace, source: "docker_cleanroom", image: task.image, prep_log: prepLog }
    }
  }
  if (!allowLocalFixture) {
    writeFile(path.join(agentDir, "prep-log.json"), JSON.stringify(prepLog, null, 2))
    return { workspace, source: "unavailable", image: task.image, prep_log: prepLog, error: "Docker cleanroom image could not be prepared and local fixture fallback is disabled" }
  }
  fs.rmSync(workspace, { recursive: true, force: true })
  mkdirp(workspace)
  writeFile(path.join(workspace, "README.md"), localFixtureReadme(task))
  writeFile(path.join(workspace, "src", "main.sh"), "#!/usr/bin/env bash\nprintf 'ProgramBench local fixture placeholder\\n'\n")
  writeProgrambenchNotes(workspace, task)
  initGit(workspace, prepLog)
  writeFile(path.join(agentDir, "prep-log.json"), JSON.stringify(prepLog, null, 2))
  return { workspace, source: "local_fixture", image: task.image, prep_log: prepLog, warning: "Used local fixture only because Docker cleanroom image was unavailable" }
}

function briefRun(result) {
  return {
    status: result.status,
    signal: result.signal,
    duration_ms: result.duration_ms,
    stdout_tail: result.stdout.slice(-4000),
    stderr_tail: result.stderr.slice(-4000),
    error: result.error,
  }
}

function initGit(workspace, log) {
  run("git", ["init"], { cwd: workspace, timeoutMs: 60_000 })
  run("git", ["config", "user.email", "programbench-runner@example.invalid"], { cwd: workspace, timeoutMs: 60_000 })
  run("git", ["config", "user.name", "ProgramBench Runner"], { cwd: workspace, timeoutMs: 60_000 })
  run("git", ["add", "."], { cwd: workspace, timeoutMs: 120_000 })
  const commit = run("git", ["commit", "-m", "initial cleanroom workspace"], { cwd: workspace, timeoutMs: 120_000 })
  log.push({ step: "git_initial_commit", ...briefRun(commit) })
}

function writeProgrambenchNotes(workspace, task) {
  writeFile(path.join(workspace, "PROGRAMBENCH_TASK.md"), [
    `# ProgramBench Task ${task.id}`,
    "",
    `Repository: ${task.repository || ""}`,
    `Commit: ${task.commit || ""}`,
    `Language: ${task.language || ""}`,
    `Difficulty: ${task.difficulty || ""}`,
    `Cleanroom image: ${task.image}`,
    "",
    "You are given only this cleanroom binary/documentation workspace.",
    "Do not use the internet. Do not fetch source code, package answers, repository history, or external implementation references.",
    "Do not ask the user for clarification, credentials, permissions, environment changes, or missing information during this benchmark. Do not request credentials, permissions, shell changes, Docker setup, API keys, or missing context. Exhaust local docs, fixtures, binary inspection, WSL/Git Bash/MSYS/Docker routes, record the exact failed route and continue, and record unresolved gaps as unknowns in CLI_BEHAVIOR_INVENTORY.md; do not stop to ask the user.",
    "Rebuild the full application source code from scratch so the rebuilt app has identical behavior to the original binary.",
    "For the same inputs, the rebuilt app must produce the same outputs, exit codes, files, and errors required by ProgramBench.",
    ...programbenchExactBehaviorLines(),
    "Keep all source code, build scripts, and documentation needed for evaluation in this workspace.",
    "",
    "## Required evaluation output contract",
    ...programbenchOutputContractLines(),
    "",
    "## How to discover the CLI behavior to rebuild",
    ...programbenchCliDiscoveryLines(),
    "",
    "## Command environment routes",
    ...programbenchEnvironmentRouteLines(),
    "",
    "Read `CLI_RECONSTRUCTION_PLAYBOOK.md` before implementation. Create `CLI_BEHAVIOR_INVENTORY.md` with your probes and findings before writing the rebuilt implementation.",
    "",
    "## Raw task.yaml",
    "```yaml",
    task.taskYaml.trim(),
    "```",
    "",
    "## Active test branches",
    ...Object.entries(task.tests.branches || {}).map(([branch, info]) => `- ${branch}: ${info.ignored ? "ignored" : `${(info.tests || []).length} tests`}`),
    "",
  ].join("\n"))
  writeFile(path.join(workspace, "CLI_RECONSTRUCTION_PLAYBOOK.md"), programbenchCliReconstructionPlaybook(task))
}

function programbenchOutputContractLines() {
  return [
    "- The final submission archive is built from this workspace root.",
    "- Create a root-level `compile.sh` file. ProgramBench will run `chmod +x ./compile.sh && ./compile.sh` after extracting the submission.",
    "- `compile.sh` must be self-contained for the cleanroom container, fail on errors, and compile or prepare the rebuilt application without internet access.",
    "- After `compile.sh` completes successfully, the runnable program must exist at `./executable` in the workspace root.",
    "- `./executable` must be executable and must implement the reconstructed application behavior. ProgramBench test branches invoke this file and compare stdout, stderr, exit code, file outputs, and other observable behavior.",
    "- Include any source files, manifests, lockfiles, assets, scripts, and documentation needed for `compile.sh` to produce `./executable`.",
  ]
}

function programbenchExactBehaviorLines() {
  return [
    "- Output compatibility is strict. Reproduce the oracle's observable behavior one-for-one, not just approximately.",
    "- Match exact stdout and stderr routing, text, capitalization, punctuation, spacing, ordering, and trailing newlines.",
    "- Match exact process exit codes for success, help/version, parser errors, validation errors, missing files, not-found results, and functional failures.",
    "- Match help and version output exactly, including usage line, option names, aliases, descriptions, default values, and whether text goes to stdout or stderr.",
    "- Match parser and validation behavior exactly: required argument errors, missing flag value errors, unknown flags, invalid enum/integer/path errors, duplicate flags, defaults, and flag ordering.",
    "- Do not replace precise oracle errors with generic or more idiomatic errors. If the oracle says `'inputFile' does not exist`, the rebuilt app should say that exact text on the same stream with the same exit code.",
    "- Treat return code, stdout, stderr, created/modified files, and timing-sensitive early-exit behavior as part of the API.",
  ]
}

function programbenchCliDiscoveryLines() {
  return [
    "- Treat the provided cleanroom `./executable` as the behavioral oracle for the CLI surface.",
    "- First inspect local documentation and files, then run safe local probes against `./executable` only through a command environment that can actually execute it.",
    "- Do not begin implementing the replacement until you have recorded at least one real source of input/output behavior: oracle probe results are preferred; if oracle execution is impossible after all routes below, record exact docs/fixture/binary-string evidence and every execution route attempted.",
    "- Never ask the user what to do next during this benchmark. If a route, credential, shell, or environment feature is missing, record the exact limitation and continue with the next local route or the best local evidence.",
    "- Probe common help/version/error entry points such as `./executable --help`, `./executable -h`, `./executable help`, `./executable --version`, and `./executable --unknown-flag` when they are safe.",
    "- Exercise representative valid and invalid inputs using only local fixture files/assets in the workspace. Record what stdout, stderr, exit code, and file outputs should look like.",
    "- Re-run the same probes against your rebuilt `./executable` and iterate until the observed behavior matches the cleanroom executable.",
  ]
}

function programbenchEnvironmentRouteLines() {
  return [
    "- There are two possible command environments. Use the environment exposed by the current agent; do not assume both are available, and do not force a Bash-only approach through a Windows PowerShell route.",
    "- Route A, `bash` / POSIX route: when a real POSIX shell is exposed, use it for `file`, `ldd`, `strings`, `chmod`, fixture probes, `./executable` oracle probes, `./compile.sh`, and rebuilt-vs-oracle comparison.",
    "- Route B, `shell_command` / host route: on Windows this is PowerShell-oriented. Use it for listing files, reading docs, editing files, copying/renaming artifacts, git status, Python/Node checks, Docker checks, and other host-safe workspace operations.",
    "- If Route B is Windows and `./executable` is a Linux ELF, PowerShell cannot execute it directly. Do not repeatedly run `./executable` in PowerShell. Instead, actively look for a Linux execution route before giving up.",
    "- Under Windows `shell_command`, try these oracle routes in order when safe: `wsl sh -lc` from the workspace, a usable `bash`/Git Bash/MSYS if present, and Docker using the cleanroom image named in PROGRAMBENCH_TASK.md. Docker cleanroom is a valid oracle route because it runs the same ProgramBench image locally and offline.",
    "- If a route exists but fails, record the exact command, stdout, stderr, and exit code in CLI_BEHAVIOR_INVENTORY.md, then try the next route. For example, `/bin/bash` missing does not prove `sh`, WSL, Git Bash, or Docker are unavailable.",
    "- Only after all safe oracle execution routes fail may you rely on local docs, fixtures, ZIP metadata, and binary strings. In that case, clearly mark the implementation as inferred rather than oracle-confirmed.",
    "- For rebuilt implementation checks, run through the same route ProgramBench will use when possible; otherwise clearly separate host-validation results from oracle-comparison results.",
  ]
}

function programbenchCliReconstructionPlaybook(task) {
  return [
    `# CLI Reconstruction Playbook for ${task.id}`,
    "",
    "This workspace is a cleanroom ProgramBench task. The original source and hidden tests are not available. Your job is to discover the CLI behavior from the local oracle executable and local files, then rebuild it.",
    "This file is `CLI_RECONSTRUCTION_PLAYBOOK.md`; use it to create `CLI_BEHAVIOR_INVENTORY.md` before implementation.",
    "",
    "Do not use the internet. Do not fetch source code, examples, repository history, package answers, or external references. All discovery and rebuilding must work without internet access.",
    "Do not ask the user questions during this benchmark. Do not request credentials, permissions, shell changes, Docker setup, API keys, or missing context. If something is unavailable, record the exact failed route and continue with the next local route or best local evidence; do not stop to ask the user.",
    "",
    "## Command environment routes",
    "",
    ...programbenchEnvironmentRouteLines(),
    "",
    "## Exact behavior compatibility",
    "",
    ...programbenchExactBehaviorLines(),
    "",
    "## Discovery gate before implementation",
    "",
    ...programbenchCliDiscoveryLines(),
    "",
    "## Required workflow",
    "",
    "1. Inspect local files.",
    "   - List the workspace tree.",
    "   - Read every local README, docs file, manifest, script, and obvious fixture description.",
    "   - Identify sample inputs under directories such as `assets`, `test-files`, `fixtures`, `examples`, or similar names.",
    "",
    "2. Identify the oracle command.",
    "   - The provided cleanroom executable is usually `./executable`.",
    "   - If another local binary or wrapper is present, record why you think it matters.",
    "",
    "3. Capture baseline invocation behavior.",
    "   Run these probes when safe and record stdout, stderr, exit code, and whether output order matters:",
    "   - `./executable`",
    "   - `./executable --help`",
    "   - `./executable -h`",
    "   - `./executable help`",
    "   - `./executable --version`",
    "   - `./executable -V`",
    "   - `./executable --`",
    "   - `./executable --unknown-flag`",
    "",
    "4. Build a CLI surface inventory from help output.",
    "   For every command, subcommand, option, and positional argument mentioned by help output, record:",
    "   - long flag",
    "   - short flag",
    "   - whether it is required",
    "   - value name and type",
    "   - default value",
    "   - allowed enum/preset values",
    "   - whether the flag may repeat",
    "   - whether both `--flag value` and `--flag=value` work",
    "   - stdout/stderr behavior for valid and invalid values",
    "   - exit code for valid and invalid values",
    "",
    "5. Probe parser and validation behavior.",
    "   For each option, test valid and invalid inputs, including at least one valid value and one invalid value. Include:",
    "   - missing required arguments",
    "   - missing flag values",
    "   - unknown flags",
    "   - non-integer values for integer options",
    "   - zero, negative, and very large integer values when relevant",
    "   - paths that do not exist",
    "   - paths that are directories",
    "   - duplicate flags",
    "   - flags before and after other arguments",
    "",
    "6. Probe functional behavior with local fixtures.",
    "   Use only local files in this workspace. For each fixture-driven behavior, record:",
    "   - exact command line",
    "   - input fixture path",
    "   - expected stdout and stderr snippets",
    "   - exact exit code",
    "   - output files created or modified",
    "   - timing-sensitive behavior or early exit behavior",
    "",
    "7. Probe output format exactly.",
    "   Hidden tests often compare strings. Preserve:",
    "   - exact stdout vs stderr routing",
    "   - exact exit code for every success and error path",
    "   - first line and description text in help",
    "   - executable name shown in usage",
    "   - capitalization",
    "   - punctuation",
    "   - spaces and indentation",
    "   - output ordering",
    "   - singular/plural words",
    "   - trailing newlines",
    "   - whether normal messages go to stdout or stderr",
    "   - whether errors use stdout or stderr",
    "   - exact error message text, including quoted argument names",
    "",
    "8. Create `CLI_BEHAVIOR_INVENTORY.md` before implementation.",
    "   It must contain:",
    "   - commands/probes you ran",
    "   - observed stdout/stderr/exit code for baseline probes",
    "   - every discovered option and default",
    "   - every local fixture you used",
    "   - behavior examples for success, not-found/no-op, and invalid-input cases",
    "   - unknowns you could not resolve and what probe you tried",
    "",
    "9. Implement and compare.",
    "   - Keep the oracle executable available if possible by renaming or copying it before building.",
    "   - Build your replacement as `./executable` through `compile.sh`.",
    "   - Re-run the same inventory probes against your rebuilt `./executable`.",
    "   - Diff the observed stdout, stderr, exit code, and file outputs against the oracle observations.",
    "   - Iterate until the replacement matches the oracle for the inventory.",
    "",
    "10. Final ProgramBench artifacts.",
    "   - The final submission archive is built from the workspace root.",
    "   - Root-level `compile.sh` must exist.",
    "   - ProgramBench will run `chmod +x ./compile.sh && ./compile.sh`.",
    "   - After compile, root-level `./executable` must exist and be executable.",
    "   - Include all source, scripts, assets, manifests, and lockfiles needed to build offline.",
    "",
    "## Suggested inventory template",
    "",
    "```markdown",
    "# CLI Behavior Inventory",
    "",
    "## Oracle",
    "- executable path:",
    "- files/docs inspected:",
    "",
    "## Baseline probes",
    "| command | exit | stdout summary | stderr summary | notes |",
    "| --- | ---: | --- | --- | --- |",
    "",
    "## CLI surface",
    "| option/subcommand | alias | required | value/default | valid values | invalid behavior |",
    "| --- | --- | --- | --- | --- | --- |",
    "",
    "## Functional probes",
    "| behavior | command | fixture | exit | stdout/stderr expectation |",
    "| --- | --- | --- | ---: | --- |",
    "",
    "## Rebuilt comparison",
    "| probe | oracle result | rebuilt result | match |",
    "| --- | --- | --- | --- |",
    "```",
    "",
  ].join("\n")
}

function localFixtureReadme(task) {
  return [
    `# Local fixture for ${task.id}`,
    "",
    "Docker is not available, so this is only a runner plumbing fixture.",
    "It is not a valid ProgramBench cleanroom workspace and should not be used for scoring.",
    "",
  ].join("\n")
}

function programbenchPrompt(task) {
  return `You are solving ProgramBench instance ${task.id}.

This is a long-running planning and reconstruction task, not a quick single-pass coding task. You should expect to spend most of the work on discovery, inventory, implementation, oracle comparison, and repeated revision. Do not jump straight to writing the final implementation before you have mapped the CLI behavior.

Rules:
- You MUST NOT use the internet.
- Do not fetch the original repository, source code, issue discussions, examples, package answers, or external implementation references.
- Do not ask the user any questions during this benchmark. Do not request credentials, permissions, shell changes, Docker setup, API keys, or missing context. If something is unavailable, record the exact failed route and continue with the next local route or best local evidence.
- You are given a cleanroom workspace containing only the compiled binary/documentation made available by ProgramBench.
- Recreate the complete application source implementation from scratch.
- The rebuilt app must match the original app for the same inputs: stdout, stderr, exit code, file outputs, and observable behavior.
${programbenchExactBehaviorLines().join("\n")}
- Produce the exact ProgramBench evaluation entry points:
${programbenchOutputContractLines().map((line) => `  ${line}`).join("\n")}
- Discover the CLI behavior before implementing:
${programbenchCliDiscoveryLines().map((line) => `  ${line}`).join("\n")}
- Command environment routes:
${programbenchEnvironmentRouteLines().map((line) => `  ${line}`).join("\n")}
- Read CLI_RECONSTRUCTION_PLAYBOOK.md first. Before writing the rebuilt implementation, create CLI_BEHAVIOR_INVENTORY.md documenting the probes you ran, the discovered CLI surface, exact output/exit-code behavior, local fixtures used, and rebuilt-vs-oracle comparison results. This inventory is a gate: do not start implementation until it contains enough input/output behavior to justify the design.
- Work inside this workspace only.
- Infer the required behavior from the cleanroom materials and from safe local probes of the provided executable.
- If you cannot execute the oracle after exhausting local routes, continue with an inferred implementation from docs, fixtures, binary strings, and metadata; do not stop to ask the user.
- If your first implementation does not build or does not match the executable, keep revising it until it is ready for evaluation.

Required phased workflow:
1. Discovery phase: inspect local files, read CLI_RECONSTRUCTION_PLAYBOOK.md, run oracle probes through the appropriate route, and identify all visible commands, flags, defaults, aliases, parser errors, output channels, exit codes, and local fixtures.
2. Inventory phase: create CLI_BEHAVIOR_INVENTORY.md before implementation. Record exact commands, stdout, stderr, exit code, file outputs, timing-sensitive behavior, unknowns, and every local fixture used.
3. Design phase: choose a minimal implementation strategy that can reproduce the observed behavior offline and produce ./executable through compile.sh.
4. Implementation phase: write source code, build scripts, and required assets. Keep the work scoped to this workspace.
5. Comparison phase: run the same inventory probes against the rebuilt ./executable and compare against the oracle observations.
6. Revision phase: fix mismatches in behavior, formatting, exit codes, stdout/stderr routing, and performance. Repeat comparison until the rebuilt executable is ready for ProgramBench evaluation.

Task metadata:
- repository: ${task.repository || ""}
- commit: ${task.commit || ""}
- language: ${task.language || ""}
- difficulty: ${task.difficulty || ""}

Start by inspecting the workspace and identifying how the original binary is invoked. Then reconstruct, build, run, compare against the provided executable, and iterate until the rebuilt application is ready for ProgramBench evaluation.
`
}

function assertProgrambenchOutputContract(text, label) {
  for (const expected of [
    "compile.sh",
    "chmod +x ./compile.sh && ./compile.sh",
    "./executable",
    "submission archive",
    "without internet access",
    "stdout, stderr, exit code",
    "one-for-one",
    "same stream with the same exit code",
    "trailing newlines",
    "parser errors",
  ]) {
    assert(text.includes(expected), `${label} is missing ProgramBench output contract text: ${expected}`)
  }
}

function assertProgrambenchCliDiscovery(text, label) {
  for (const expected of [
    "CLI_RECONSTRUCTION_PLAYBOOK.md",
    "CLI_BEHAVIOR_INVENTORY.md",
    "./executable --help",
    "./executable -h",
    "./executable --version",
    "./executable --unknown-flag",
    "valid and invalid inputs",
    "stdout, stderr, exit code",
    "rebuilt `./executable`",
    "Do not begin implementing the replacement until you have recorded at least one real source of input/output behavior",
    "Never ask the user what to do next during this benchmark",
  ]) {
    assert(text.includes(expected), `${label} is missing ProgramBench CLI discovery text: ${expected}`)
  }
}

function assertProgrambenchNoUserQuestions(text, label) {
  for (const expected of [
    "Do not ask the user",
    "Do not request credentials, permissions, shell changes, Docker setup, API keys, or missing context",
    "record the exact failed route and continue",
    "do not stop to ask the user",
  ]) {
    assert(text.includes(expected), `${label} is missing ProgramBench no-user-question text: ${expected}`)
  }
}

function assertProgrambenchEnvironmentRoutes(text, label) {
  for (const expected of [
    "Command environment routes",
    "Route A, `bash` / POSIX route",
    "Route B, `shell_command` / host route",
    "do not force a Bash-only approach through a Windows PowerShell route",
    "PowerShell cannot execute it directly",
    "wsl sh -lc",
    "Docker using the cleanroom image",
    "Docker cleanroom is a valid oracle route",
    "/bin/bash` missing does not prove `sh`, WSL, Git Bash, or Docker are unavailable",
    "Only after all safe oracle execution routes fail",
  ]) {
    assert(text.includes(expected), `${label} is missing ProgramBench environment route text: ${expected}`)
  }
}

function assertProgrambenchPlanningWorkflow(text, label) {
  for (const expected of [
    "long-running planning and reconstruction task",
    "Do not jump straight to writing the final implementation",
    "This inventory is a gate",
    "Required phased workflow",
    "Discovery phase",
    "Inventory phase",
    "Design phase",
    "Implementation phase",
    "Comparison phase",
    "Revision phase",
  ]) {
    assert(text.includes(expected), `${label} is missing ProgramBench planning workflow text: ${expected}`)
  }
}

function runSelfTest() {
  const task = {
    id: "testorg__calculator.abc1234",
    repository: "testorg/calculator",
    commit: "abc1234",
    language: "rs",
    difficulty: "mini",
    image: "programbench/testorg_calculator.abc1234:task_cleanroom",
    taskYaml: "repository: testorg/calculator\ncommit: abc1234\nlanguage: rs",
    tests: { branches: { branch1: { tests: ["addition"] } } },
  }
  const prompt = programbenchPrompt(task)
  assertProgrambenchOutputContract(prompt, "programbenchPrompt")
  assertProgrambenchCliDiscovery(prompt, "programbenchPrompt")
  assertProgrambenchEnvironmentRoutes(prompt, "programbenchPrompt")
  assertProgrambenchPlanningWorkflow(prompt, "programbenchPrompt")
  assertProgrambenchNoUserQuestions(prompt, "programbenchPrompt")

  const tmp = fs.mkdtempSync(path.join(runRoot, "self-test-"))
  writeProgrambenchNotes(tmp, task)
  const notes = fs.readFileSync(path.join(tmp, "PROGRAMBENCH_TASK.md"), "utf8")
  assertProgrambenchOutputContract(notes, "PROGRAMBENCH_TASK.md")
  assertProgrambenchCliDiscovery(notes, "PROGRAMBENCH_TASK.md")
  assertProgrambenchEnvironmentRoutes(notes, "PROGRAMBENCH_TASK.md")
  assertProgrambenchNoUserQuestions(notes, "PROGRAMBENCH_TASK.md")
  const playbook = fs.readFileSync(path.join(tmp, "CLI_RECONSTRUCTION_PLAYBOOK.md"), "utf8")
  assertProgrambenchOutputContract(playbook, "CLI_RECONSTRUCTION_PLAYBOOK.md")
  assertProgrambenchCliDiscovery(playbook, "CLI_RECONSTRUCTION_PLAYBOOK.md")
  assertProgrambenchEnvironmentRoutes(playbook, "CLI_RECONSTRUCTION_PLAYBOOK.md")
  assertProgrambenchNoUserQuestions(playbook, "CLI_RECONSTRUCTION_PLAYBOOK.md")
  for (const expected of [
    "Required workflow",
    "Build a CLI surface inventory",
    "Probe parser and validation behavior",
    "Probe functional behavior with local fixtures",
    "Probe output format exactly",
    "Suggested inventory template",
  ]) {
    assert(playbook.includes(expected), `CLI_RECONSTRUCTION_PLAYBOOK.md is missing manual section: ${expected}`)
  }
  console.log(JSON.stringify({ ok: true, self_test: "programbench output contract and CLI reconstruction playbook" }, null, 2))
}

function serviceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return ["-c", `service_tier="${tier}"`]
}

function turaServiceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return tier === "priority" ? ["-p"] : []
}

async function runCodexMain(workspace, agentDir, prompt) {
  assert(fs.existsSync(codexMainExe), `missing codex-main exe: ${codexMainExe}`)
  return runLive(codexMainExe, [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "-C",
    workspace,
    "-m",
    model,
    "--dangerously-bypass-approvals-and-sandbox",
    "-c",
    `model_reasoning_effort="${reasoning}"`,
    ...(codexGoalsEnabled ? ["-c", "goals=true"] : []),
    ...serviceTierConfigArgs(),
  ], {
    cwd: workspace,
    input: prompt,
    timeoutMs,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
  })
}

async function runTuraPlanning(workspace, agentDir, prompt, agentPrompt) {
  assert(fs.existsSync(turaExe), `missing Tura exe: ${turaExe}`)
  const sessionId = `agent-programbench-${agentPrompt}-${process.pid}-${Date.now()}`
  snapshotTuraInternalPrompt(agentDir, agentPrompt)
  snapshotTuraAgentConfig(agentDir, agentPrompt)
  const planningMode = planningOverride ?? path.basename(agentDir).includes("planning")
  return runLive(turaExe, [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    sessionId,
    "--agent-id",
    agentPrompt,
    "-m",
    turaModel,
    ...turaServiceTierConfigArgs(),
    ...(planningOverride !== null || path.basename(agentDir).includes("planning") ? ["--planning", planningMode ? "on" : "off"] : []),
    "--model-reasoning-effort",
    reasoning,
    "--cwd",
    workspace,
  ], {
    cwd: workspace,
    input: prompt,
    timeoutMs,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath: path.join(agentDir, "status.json"),
    env: {
      TURA_PROJECT_ROOT: repoRoot,
      TURA_COMMAND_RUN_SHELL: "shell_command",
      TURA_COMMAND_RUN_STRICT_JSON: "0",
      TURA_SESSION_REASONING_EFFORT: reasoning,
      ...(planningMode ? { TURA_FORCE_EXECUTE_TOOLS_PLANNING: "1" } : {}),
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    },
  })
}

function snapshotTuraInternalPrompt(agentDir, agentPrompt) {
  const promptPath = path.join(repoRoot, "agents", "src", agentPrompt, "prompt.md")
  if (!fs.existsSync(promptPath)) return null
  const content = fs.readFileSync(promptPath, "utf8")
  const snapshotPath = path.join(agentDir, "tura-internal-prompt.md")
  writeFile(snapshotPath, content)
  return { prompt_path: promptPath, snapshot_path: snapshotPath, sha256: crypto.createHash("sha256").update(content).digest("hex") }
}

function snapshotTuraAgentConfig(agentDir, agentPrompt) {
  const configPath = path.join(repoRoot, "agents", "src", agentPrompt, "agent_config.json")
  if (!fs.existsSync(configPath)) return null
  const content = fs.readFileSync(configPath, "utf8")
  const snapshotPath = path.join(agentDir, "tura-agent-config.json")
  writeFile(snapshotPath, content)
  return { config_path: configPath, snapshot_path: snapshotPath, sha256: crypto.createHash("sha256").update(content).digest("hex") }
}

function turaCapabilityInfo(agentPrompt, agentDir) {
  if (!agentPrompt) return null
  const configPath = path.join(repoRoot, "agents", "src", agentPrompt, "agent_config.json")
  let capabilities = []
  if (fs.existsSync(configPath)) {
    const config = JSON.parse(fs.readFileSync(configPath, "utf8"))
    capabilities = (config.agent_capabilities || []).map((item) => item.capability_name).filter(Boolean)
  }
  const planningMode = planningOverride ?? path.basename(agentDir).includes("planning")
  return {
    agent_prompt: agentPrompt,
    config_path: configPath,
    configured_capabilities: capabilities,
    config_has_task_status: capabilities.includes("task_status"),
    config_has_planning: capabilities.includes("planning"),
    planning_override: planningOverride === null ? "auto" : (planningOverride ? "on" : "off"),
    planning_cli_override_effective: planningMode,
    effective_planning_available: planningMode || capabilities.includes("planning"),
  }
}

function parseJsonl(text) {
  return String(text || "").split(/\r?\n/).map((line) => line.trim()).filter(Boolean).map((line) => {
    try { return JSON.parse(line) } catch { return null }
  }).filter(Boolean)
}

function addUsage(totals, usage) {
  totals.usage_events += 1
  totals.input_tokens += Number(usage.input_tokens || usage.prompt_tokens || 0)
  totals.output_tokens += Number(usage.output_tokens || usage.completion_tokens || 0)
  totals.reasoning_tokens += Number(usage.reasoning_tokens || usage.reasoning_output_tokens || usage.output_tokens_details?.reasoning_tokens || 0)
  totals.cached_input_tokens += Number(usage.cached_input_tokens || usage.input_tokens_details?.cached_tokens || usage.prompt_tokens_details?.cached_tokens || 0)
  totals.total_tokens += Number(usage.total_tokens || 0)
}

function usageFromJsonl(stdout) {
  const totals = { usage_events: 0, input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, cached_input_tokens: 0, total_tokens: 0 }
  for (const event of parseJsonl(stdout)) {
    const usage = event.usage || event.message?.usage || event.payload?.info?.last_token_usage
    if (usage) addUsage(totals, usage)
  }
  return totals
}

function eventStats(stdout) {
  const events = parseJsonl(stdout)
  const stats = {
    events: events.length,
    thread_started: 0,
    turn_started: 0,
    turn_completed: 0,
    agent_messages: 0,
    command_executions: 0,
    commands_completed: 0,
    commands_failed: 0,
    file_changes: 0,
    task_status_callbacks: 0,
    planning_mentions: 0,
    planning_command_executions: 0,
    runtime_usage_events: 0,
  }
  for (const event of events) {
    const text = JSON.stringify(event)
    if (event.type === "thread.started") stats.thread_started += 1
    if (event.type === "turn.started") stats.turn_started += 1
    if (event.type === "turn.completed") stats.turn_completed += 1
    if (event.type === "runtime_usage") stats.runtime_usage_events += 1
    if (event.item?.type === "agent_message") stats.agent_messages += 1
    if (event.item?.type === "file_change") stats.file_changes += 1
    if (event.item?.type === "command_execution") {
      stats.command_executions += 1
      if (event.item.status === "completed") stats.commands_completed += 1
      if (event.item.status === "failed") stats.commands_failed += 1
      if (event.item.command === "task_status" || /"task_status"\s*:/.test(text)) stats.task_status_callbacks += 1
      if (event.item.command === "planning" || /"planning"\s*:/.test(text)) stats.planning_command_executions += 1
    }
    if (text.includes("planning")) stats.planning_mentions += 1
  }
  stats.dispatch_ok = stats.command_executions > 0 || stats.file_changes > 0 || stats.planning_mentions > 0
  stats.planning_dispatch_ok = stats.planning_command_executions > 0
  stats.callback_ok = stats.task_status_callbacks > 0 || stats.turn_completed > 0
  return stats
}

function collectPatch(workspace, agentDir) {
  const patchPath = path.join(agentDir, "model.patch")
  const statusPath = path.join(agentDir, "git-status.txt")
  if (!fs.existsSync(path.join(workspace, ".git"))) {
    writeFile(patchPath, "")
    writeFile(statusPath, "")
    return { patch_path: patchPath, patch_bytes: 0, changed_files: 0, git_status: "" }
  }
  const diff = run("git", ["diff", "--binary"], { cwd: workspace, timeoutMs: 120_000 })
  const status = run("git", ["status", "--short"], { cwd: workspace, timeoutMs: 120_000 })
  writeFile(patchPath, diff.stdout || "")
  writeFile(statusPath, status.stdout || "")
  return {
    patch_path: patchPath,
    patch_bytes: Buffer.byteLength(diff.stdout || "", "utf8"),
    changed_files: status.stdout.split(/\r?\n/).filter(Boolean).length,
    git_status: status.stdout,
  }
}

function packageSubmission(workspace, agentDir, task) {
  const submissionsRoot = path.join(runRoot, "submissions")
  const submissionDir = path.join(submissionsRoot, path.basename(agentDir), task.id)
  mkdirp(submissionDir)
  const archive = path.join(submissionDir, "submission.tar.gz")
  const result = run("tar", ["-czf", archive, "--exclude=.git", "."], { cwd: workspace, timeoutMs: 5 * 60_000 })
  return { archive, status: result.status, stderr: result.stderr.slice(-2000), stdout: result.stdout.slice(-2000) }
}

function removeDirWithRetries(dir) {
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try {
      fs.rmSync(dir, { recursive: true, force: true })
      return true
    } catch {
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250)
    }
  }
  return false
}

function cleanupProgrambenchEvalArtifacts(evalTempDir) {
  const cleanup = []
  const containers = run("docker", ["ps", "-aq", "--filter", "name=programbench-"], { timeoutMs: 60_000 })
  cleanup.push({ step: "docker_ps_programbench", ...briefRun(containers) })
  const ids = containers.stdout.split(/\s+/).map((item) => item.trim()).filter(Boolean)
  if (ids.length > 0) {
    const rm = run("docker", ["rm", "-f", ...ids], { timeoutMs: 120_000 })
    cleanup.push({ step: "docker_rm_programbench", containers: ids, ...briefRun(rm) })
  }
  cleanup.push({ step: "eval_temp_cleanup", directory: evalTempDir, removed: removeDirWithRetries(evalTempDir) })
  writeFile(path.join(runRoot, "programbench-eval-cleanup.json"), JSON.stringify(cleanup, null, 2))
  return cleanup
}

async function maybeEvaluate(submissionRoot) {
  if (!runEval) return { ran: false, reason: "COMMAND_RUN_AGENT_PROGRAMBENCH_RUN_EVAL is not 1" }
  const evalTempDir = path.join(runRoot, "programbench-eval-temp")
  fs.rmSync(evalTempDir, { recursive: true, force: true })
  mkdirp(evalTempDir)
  const result = run("uv", [
    "run",
    "programbench",
    "eval",
    submissionRoot,
    "--workers",
    "1",
    "--branch-workers",
    "1",
    "--docker-cpus",
    dockerCpus,
  ], {
    cwd: programbenchRoot,
    timeoutMs: Number(process.env.COMMAND_RUN_AGENT_PROGRAMBENCH_EVAL_TIMEOUT_MS || 60 * 60_000),
    env: {
      TMP: evalTempDir,
      TEMP: evalTempDir,
      TMPDIR: evalTempDir,
      PYTHONIOENCODING: "utf-8",
      PYTHONUTF8: "1",
    },
  })
  const cleanup = cleanupProgrambenchEvalArtifacts(evalTempDir)
  writeFile(path.join(runRoot, "programbench-eval.stdout.log"), result.stdout)
  writeFile(path.join(runRoot, "programbench-eval.stderr.log"), result.stderr)
  return { ran: true, exit_code: result.status, stdout_path: path.join(runRoot, "programbench-eval.stdout.log"), stderr_path: path.join(runRoot, "programbench-eval.stderr.log"), cleanup_path: path.join(runRoot, "programbench-eval-cleanup.json"), cleanup, error: result.error }
}

async function runAgent(agentId, task, index) {
  const agentDir = path.join(runRoot, task.id, `${agentId}-${index + 1}`)
  const prep = prepareWorkspace(agentDir, task)
  let result = { status: null, signal: null, stdout: "", stderr: "", duration_ms: 0, first_output_ms: null, error: prep.error || null }
  const started = performance.now()
  if (!prep.error) {
    const prompt = programbenchPrompt(task)
    if (agentId === "codex-main") result = await runCodexMain(prep.workspace, agentDir, prompt)
    else if (agentId === "tura-fast-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "fast")
    else if (agentId === "tura-fast-planning-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "fast")
    else if (agentId === "tura-planning-shll") result = await runTuraPlanning(prep.workspace, agentDir, prompt, "thinking-planning")
    else throw new Error(`unsupported agent ${agentId}`)
  }
  const agentPrompt =
    agentId === "tura-fast-shll" || agentId === "tura-fast-planning-shll" ? "fast" :
    agentId === "tura-planning-shll" ? "thinking-planning" :
    null
  const patch = prep.error ? { patch_path: "", patch_bytes: 0, changed_files: 0, git_status: "" } : collectPatch(prep.workspace, agentDir)
  const submission = prep.error ? null : packageSubmission(prep.workspace, agentDir, task)
  const stats = {
    agent: agentId,
    instance_id: task.id,
    workspace: prep.workspace,
    prep,
    elapsed_ms: Math.round(performance.now() - started),
    exit_code: result.status,
    timed_out: result.timed_out || false,
    first_output_ms: result.first_output_ms,
    error: result.error || null,
    stdout_path: path.join(agentDir, "stdout.jsonl"),
    stderr_path: path.join(agentDir, "stderr.log"),
    tura_capability_info: turaCapabilityInfo(agentPrompt, agentDir),
    usage: usageFromJsonl(result.stdout),
    events: eventStats(result.stdout),
    patch,
    submission,
  }
  writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
  return stats
}

async function main() {
  mkdirp(runRoot)
  if (selfTest) {
    runSelfTest()
    return
  }
  assert(fs.existsSync(programbenchRoot), `missing ProgramBench root: ${programbenchRoot}`)
  const task = loadTaskMetadata()
  if (prepOnly) {
    const prepDir = path.join(runRoot, task.id, "prep-only")
    const prep = prepareWorkspace(prepDir, task)
    const summary = { ok: !prep.error, prep_only: true, run_id: runId, run_root: runRoot, programbench_root: programbenchRoot, task, prep }
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  const results = []
  for (let i = 0; i < agents.length; i += 1) {
    console.log(`[programbench] running ${agents[i]} on ${task.id} for ${Math.round(timeoutMs / 1000)}s`)
    results.push(await runAgent(agents[i], task, i))
  }
  const evalResult = await maybeEvaluate(path.join(runRoot, "submissions"))
  const summary = {
    ok: results.every((result) => !result.error && result.events.callback_ok),
    run_id: runId,
    run_root: runRoot,
    programbench_root: programbenchRoot,
    instance_id: task.id,
    cleanroom_image: task.image,
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
    agents,
    results,
    eval: evalResult,
  }
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
}

await main()

