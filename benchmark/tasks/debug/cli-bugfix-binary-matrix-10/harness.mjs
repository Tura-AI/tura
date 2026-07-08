import assert from "node:assert/strict"
import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import { spawn, spawnSync } from "node:child_process"

const userAgent = "tura-cli-bugfix-binary-harness"

export function loadOracleMatrix(scriptDir) {
  const matrixPath = path.join(scriptDir, "oracle-matrix.json")
  return JSON.parse(fs.readFileSync(matrixPath, "utf8"))
}

export function oracleMatrixForTask(task, matrixConfig) {
  const matrix = matrixConfig.tasks?.[task.id]
  assert(matrix, `missing oracle matrix for ${task.id}`)
  return {
    interfaceAudit: matrix.interfaceAudit || { entrypoints: [] },
    sharedFixtures: matrix.sharedFixtures || [],
    failToPass: matrix.failToPass || [],
    passToPass: matrix.passToPass || [],
  }
}

export function validateOracleMatrix(tasks, matrixConfig) {
  assert.equal(matrixConfig.schema, "tura.debug.cli-bugfix-binary-oracle-matrix.v1")
  const taskIds = new Set(tasks.map((task) => task.id))
  for (const task of tasks) {
    const matrix = oracleMatrixForTask(task, matrixConfig)
    assert(matrix.failToPass.length > 0, `${task.label} missing failToPass matrix`)
    assert(matrix.passToPass.length > 0, `${task.label} missing passToPass matrix`)
    assert(matrix.interfaceAudit?.entrypoints?.length > 0, `${task.label} missing interface audit entrypoints`)
    for (const [group, cases] of Object.entries({ failToPass: matrix.failToPass, passToPass: matrix.passToPass })) {
      const names = new Set()
      for (const item of cases) {
        assert(item.name, `${task.label} ${group} case missing name`)
        assert(!names.has(item.name), `${task.label} duplicate ${group} case: ${item.name}`)
        names.add(item.name)
        assert(item.args || item.steps || item.protocol, `${task.label} ${group}/${item.name} has no executable action`)
        assert(item.compare, `${task.label} ${group}/${item.name} missing compare policy`)
      }
    }
  }
  for (const id of Object.keys(matrixConfig.tasks || {})) {
    assert(taskIds.has(id), `oracle matrix references unknown task ${id}`)
  }
}

export async function runBinaryAudit({ tasks, matrixConfig, runRoot }) {
  const cacheRoot = process.env.COMMAND_RUN_AGENT_BINARY_CACHE_ROOT || path.join(path.dirname(runRoot), "_binary-cache")
  const auditRoot = path.join(runRoot, "binary-audit")
  mkdirp(cacheRoot)
  mkdirp(auditRoot)
  const runPreflight = truthy(process.env.COMMAND_RUN_AGENT_ORACLE_PREFLIGHT || "1")
  const reports = []
  for (const task of tasks) {
    const matrix = oracleMatrixForTask(task, matrixConfig)
    const taskRoot = path.join(auditRoot, safeName(task.id))
    mkdirp(taskRoot)
    const binaries = {
      buggy: await resolveReleaseBinary(task, "buggy", cacheRoot),
      fixed: await resolveReleaseBinary(task, "fixed", cacheRoot),
    }
    const report = {
      task_id: task.id,
      label: task.label,
      buggy: binaryDescriptor(task, "buggy", binaries.buggy),
      fixed: binaryDescriptor(task, "fixed", binaries.fixed),
      interface_audit: await auditInterfaces(task, matrix, binaries, taskRoot),
      oracle_preflight: runPreflight ? await preflightOracleMatrix(task, matrix, binaries, taskRoot) : null,
    }
    reports.push(report)
    writeJson(path.join(taskRoot, "binary-audit.json"), report)
  }
  const summary = {
    schema: "tura.debug.cli-bugfix-binary-audit.v1",
    ok: reports.every((report) => {
      const preflight = report.oracle_preflight
      return !preflight || preflight.ok
    }),
    preflight_enabled: runPreflight,
    reports,
  }
  writeJson(path.join(auditRoot, "binary-audit.json"), summary)
  return summary
}

export async function runCandidateAudit({ task, matrixConfig, runRoot, candidateBinary }) {
  const cacheRoot = process.env.COMMAND_RUN_AGENT_BINARY_CACHE_ROOT || path.join(path.dirname(runRoot), "_binary-cache")
  const matrix = oracleMatrixForTask(task, matrixConfig)
  const auditRoot = path.join(runRoot, "candidate-audit", safeName(task.id))
  mkdirp(auditRoot)
  const fixedBinary = await resolveReleaseBinary(task, "fixed", cacheRoot)
  assertRunnableCommand(candidateBinary, `${task.label} candidate binary`)
  assertRunnableCommand(fixedBinary, `${task.label} fixed oracle binary`)
  const oracle = await candidateOracleMatrix(task, matrix, {
    candidate: candidateBinary,
    fixed: fixedBinary,
  }, auditRoot)
  const report = {
    schema: "tura.debug.cli-bugfix-candidate-audit.v1",
    task_id: task.id,
    label: task.label,
    candidate: {
      command: candidateBinary,
    },
    fixed: binaryDescriptor(task, "fixed", fixedBinary),
    oracle,
  }
  writeJson(path.join(auditRoot, "candidate-audit.json"), report)
  return report
}

function sideVersion(task, side) {
  return side === "buggy" ? task.bug.buggyVersion : task.bug.fixedVersion
}

function sideRef(task, side) {
  return side === "buggy" ? task.bug.buggyRef : task.bug.fixedRef
}

function sideCommit(task, side) {
  return side === "buggy" ? task.bug.buggyCommit : task.bug.fixedCommit
}

function binaryDescriptor(task, side, command) {
  return {
    version: sideVersion(task, side),
    ref: sideRef(task, side),
    commit: sideCommit(task, side),
    command,
  }
}

function assertRunnableCommand(command, label) {
  assert(command, `${label} command is empty`)
  const looksLikePath = path.isAbsolute(command) || /[\\/]/.test(command) || Boolean(path.extname(command))
  if (!looksLikePath) return
  assert(fs.existsSync(command), `${label} does not exist: ${command}`)
  assert(fs.statSync(command).isFile(), `${label} is not a file: ${command}`)
}

async function resolveReleaseBinary(task, side, cacheRoot) {
  const binary = task.binary || {}
  const version = sideVersion(task, side)
  const ref = sideRef(task, side)
  if (binary.kind === "npm_package") return ensureNpmCommand(task, side, cacheRoot)
  if (binary.kind === "pypi_package") return ensurePypiCommand(task, side, cacheRoot)
  if (binary.kind === "github_release_jar") return ensureGithubReleaseJarCommand(task, side, cacheRoot)
  if (binary.kind !== "github_release_asset") throw new Error(`unsupported binary kind for ${task.label}: ${binary.kind}`)
  const binName = process.platform === "win32" ? `${binary.binaryNames[0]}.exe` : binary.binaryNames[0]
  const stable = path.join(cacheRoot, "binaries", task.label, version, binName)
  if (fs.existsSync(stable) && fs.statSync(stable).size > 0) return stable
  const asset = await releaseAsset(task, side, cacheRoot)
  const downloaded = path.join(cacheRoot, "downloads", task.label, version, asset.name)
  await downloadFile(asset.browser_download_url, downloaded)
  mkdirp(path.dirname(stable))
  if (isRawBinaryAsset(downloaded)) {
    fs.copyFileSync(downloaded, stable)
  } else {
    const extractDir = path.join(cacheRoot, "extract", task.label, version)
    cleanExtractDir(extractDir, cacheRoot)
    extractArchive(downloaded, extractDir)
    const found = findBinaryInDir(extractDir, binary.binaryNames)
    fs.copyFileSync(found, stable)
  }
  if (process.platform !== "win32") fs.chmodSync(stable, 0o755)
  return stable
}

async function ensureGithubReleaseJarCommand(task, side, cacheRoot) {
  const version = sideVersion(task, side)
  const ref = sideRef(task, side)
  const versionNoPrefix = version.replace(/^v/i, "").replace(/^checkstyle-/i, "")
  const jarTemplate = task.binary.jarNameTemplate || `${task.label}-{versionNoPrefix}.jar`
  const jarName = jarTemplate.replaceAll("{versionNoPrefix}", versionNoPrefix).replaceAll("{version}", version.replace(/^v/i, ""))
  const stableDir = path.join(cacheRoot, "jars", task.label, version)
  const jarPath = path.join(stableDir, jarName)
  const wrapper = path.join(stableDir, process.platform === "win32" ? `${task.label}.cmd` : task.label)
  if (!fs.existsSync(jarPath)) {
    const asset = await releaseAsset(task, side, cacheRoot)
    const downloaded = path.join(cacheRoot, "downloads", task.label, version, asset.name)
    await downloadFile(asset.browser_download_url, downloaded)
    mkdirp(stableDir)
    fs.copyFileSync(downloaded, jarPath)
  }
  writeCommandWrapper(wrapper, "java", ["-jar", jarPath])
  return wrapper
}

function ensureNpmCommand(task, side, cacheRoot) {
  const version = sideVersion(task, side)
  const pkg = task.binary.package
  const binPath = task.binary.binPath
  assert(pkg && binPath, `${task.label} npm binary requires package and binPath`)
  const stableDir = path.join(cacheRoot, "npm", task.label, version)
  const wrapper = path.join(stableDir, process.platform === "win32" ? `${task.label}.cmd` : task.label)
  const binaryPath = task.binary.installMode === "npm_install"
    ? path.join(stableDir, binPath)
    : path.join(stableDir, "package", binPath)
  if (!fs.existsSync(binaryPath)) {
    resetDir(stableDir, cacheRoot)
    if (task.binary.installMode === "npm_install") {
      runOk("npm", ["install", `${pkg}@${version}`, "--prefix", stableDir, "--no-audit", "--no-fund", "--omit=dev"], { timeoutMs: 10 * 60_000 })
    } else {
      const pack = runOk("npm", ["pack", `${pkg}@${version}`, "--pack-destination", stableDir], { timeoutMs: 5 * 60_000 })
      const tgz = pack.stdout.trim().split(/\r?\n/).pop()
      extractArchive(path.join(stableDir, tgz), stableDir)
    }
  }
  writeCommandWrapper(wrapper, "node", [binaryPath])
  return wrapper
}

function ensurePypiCommand(task, side, cacheRoot) {
  const version = sideVersion(task, side)
  const pkg = task.binary.package
  const command = task.binary.command || task.binary.binaryNames?.[0] || task.label
  assert(pkg, `${task.label} PyPI binary requires package`)
  const stableDir = path.join(cacheRoot, "pypi", task.label, version)
  const venvDir = path.join(stableDir, "venv")
  const script = path.join(venvDir, process.platform === "win32" ? "Scripts" : "bin", process.platform === "win32" ? `${command}.exe` : command)
  if (!fs.existsSync(script)) {
    resetDir(stableDir, cacheRoot)
    runOk(process.env.PYTHON || "python", ["-m", "venv", venvDir], { timeoutMs: 5 * 60_000 })
    const pip = path.join(venvDir, process.platform === "win32" ? "Scripts" : "bin", process.platform === "win32" ? "pip.exe" : "pip")
    runOk(pip, ["install", `${pkg}==${version}`], { timeoutMs: 10 * 60_000 })
  }
  return script
}

async function githubRelease(task, ref, cacheRoot) {
  const cachePath = path.join(cacheRoot, "release-metadata", task.label, `${safeName(ref)}.json`)
  if (fs.existsSync(cachePath)) return JSON.parse(fs.readFileSync(cachePath, "utf8"))
  const url = `https://api.github.com/repos/${task.repo.owner}/${task.repo.name}/releases/tags/${ref}`
  const response = await fetch(url, { headers: { "User-Agent": userAgent } })
  if (!response.ok) throw new Error(`failed to fetch ${url}: ${response.status} ${await response.text()}`)
  const release = await response.json()
  writeJson(cachePath, release)
  return release
}

async function releaseAsset(task, side, cacheRoot) {
  const direct = directReleaseAsset(task, side)
  if (direct) return direct
  const release = await githubRelease(task, sideRef(task, side), cacheRoot)
  return selectAsset(task, release.assets || [], side)
}

function directReleaseAsset(task, side) {
  const info = platformInfo()
  const rules = task.binary.releaseAssetRules || []
  const rule = rules.find((item) => item.os === info.os && (!item.arch || item.arch === info.arch))
  if (!rule || !Array.isArray(rule.includes) || rule.includes.length === 0) return null
  const version = sideVersion(task, side).replace(/^v/i, "")
  const versionNoPrefix = sideVersion(task, side).replace(/^v/i, "").replace(/^checkstyle-/i, "")
  const parts = rule.includes.map((part) => String(part).replaceAll("{version}", version).replaceAll("{versionNoPrefix}", versionNoPrefix))
  const name = parts.join("")
  if (!name || (rule.excludes || []).some((part) => name.includes(part))) return null
  const ref = sideRef(task, side)
  return {
    name,
    browser_download_url: `https://github.com/${task.repo.owner}/${task.repo.name}/releases/download/${ref}/${encodeURIComponent(name)}`,
  }
}

function selectAsset(task, assets, side) {
  const info = platformInfo()
  const rules = task.binary.releaseAssetRules || []
  const candidates = rules.filter((rule) => rule.os === info.os && (!rule.arch || rule.arch === info.arch))
  const version = sideVersion(task, side).replace(/^v/i, "")
  const versionNoPrefix = sideVersion(task, side).replace(/^v/i, "").replace(/^checkstyle-/i, "")
  for (const rule of candidates) {
    const includes = (rule.includes || []).map((part) => String(part).replaceAll("{version}", version).replaceAll("{versionNoPrefix}", versionNoPrefix))
    const excludes = (rule.excludes || []).map((part) => String(part).replaceAll("{version}", version).replaceAll("{versionNoPrefix}", versionNoPrefix))
    const asset = assets.find((item) => {
      const name = item.name || ""
      return includes.every((part) => name.includes(part)) && excludes.every((part) => !name.includes(part))
    })
    if (asset) return asset
  }
  throw new Error(`no release asset for ${task.label} ${sideVersion(task, side)} on ${info.os}/${info.arch}; assets: ${assets.map((item) => item.name).join(", ")}`)
}

async function downloadFile(url, dest) {
  if (fs.existsSync(dest) && fs.statSync(dest).size > 0) return dest
  mkdirp(path.dirname(dest))
  const response = await fetch(url, { headers: { "User-Agent": userAgent } })
  if (!response.ok) throw new Error(`failed to download ${url}: ${response.status} ${await response.text()}`)
  const arrayBuffer = await response.arrayBuffer()
  fs.writeFileSync(dest, Buffer.from(arrayBuffer))
  return dest
}

async function auditInterfaces(task, matrix, binaries, taskRoot) {
  const out = {}
  for (const [side, binary] of Object.entries(binaries)) {
    out[side] = []
    for (const entrypoint of matrix.interfaceAudit.entrypoints || []) {
      const entryRoot = path.join(taskRoot, "interface", side, safeName(entrypoint.name))
      mkdirp(entryRoot)
      const result = entrypoint.protocol
        ? await runProtocolCase(binary, entrypoint, entryRoot)
        : runCommand(binary, entrypoint.args || [], { cwd: entryRoot, timeoutMs: entrypoint.timeoutMs || 20_000 })
      out[side].push({
        name: entrypoint.name,
        commandGroup: entrypoint.commandGroup || entrypoint.name,
        args: entrypoint.args || null,
        protocol: entrypoint.protocol || null,
        result: summarizeResult(result),
      })
    }
  }
  return {
    expected_command_groups: matrix.interfaceAudit.expectedCommandGroups || [],
    entrypoints: out,
  }
}

async function preflightOracleMatrix(task, matrix, binaries, taskRoot) {
  const results = []
  for (const [kind, cases] of Object.entries({ fail_to_pass: matrix.failToPass, pass_to_pass: matrix.passToPass })) {
    for (const item of cases) {
      const buggy = await runMatrixCase(task, matrix, item, binaries.buggy, path.join(taskRoot, "preflight", kind, safeName(item.name), "buggy"))
      const fixed = await runMatrixCase(task, matrix, item, binaries.fixed, path.join(taskRoot, "preflight", kind, safeName(item.name), "fixed"))
      const same = observationsEqual(buggy, fixed, item.compare)
      const pass = kind === "fail_to_pass" ? !same : same
      results.push({
        kind,
        name: item.name,
        command_group: item.commandGroup || null,
        compare: item.compare,
        pass,
        buggy: summarizeResult(buggy),
        fixed: summarizeResult(fixed),
      })
    }
  }
  return {
    ok: results.every((result) => result.pass),
    fail_to_pass_total: results.filter((result) => result.kind === "fail_to_pass").length,
    pass_to_pass_total: results.filter((result) => result.kind === "pass_to_pass").length,
    failed: results.filter((result) => !result.pass),
    results,
  }
}

async function candidateOracleMatrix(task, matrix, binaries, taskRoot) {
  const results = []
  for (const [kind, cases] of Object.entries({ fail_to_pass: matrix.failToPass, pass_to_pass: matrix.passToPass })) {
    for (const item of cases) {
      const candidate = await runMatrixCase(task, matrix, item, binaries.candidate, path.join(taskRoot, "candidate", kind, safeName(item.name)))
      const fixed = await runMatrixCase(task, matrix, item, binaries.fixed, path.join(taskRoot, "fixed", kind, safeName(item.name)))
      const pass = observationsEqual(candidate, fixed, item.compare)
      results.push({
        kind,
        name: item.name,
        command_group: item.commandGroup || null,
        compare: item.compare,
        pass,
        candidate: summarizeResult(candidate),
        fixed: summarizeResult(fixed),
      })
    }
  }
  return {
    ok: results.every((result) => result.pass),
    fail_to_pass_total: results.filter((result) => result.kind === "fail_to_pass").length,
    fail_to_pass_passed: results.filter((result) => result.kind === "fail_to_pass" && result.pass).length,
    pass_to_pass_total: results.filter((result) => result.kind === "pass_to_pass").length,
    pass_to_pass_passed: results.filter((result) => result.kind === "pass_to_pass" && result.pass).length,
    failed: results.filter((result) => !result.pass),
    results,
  }
}

async function runMatrixCase(task, matrix, item, binary, root) {
  resetDir(root, path.dirname(path.dirname(path.dirname(root))))
  const fixtures = path.join(root, "fixtures")
  mkdirp(fixtures)
  writeFixtures(fixtures, matrix.sharedFixtures)
  writeFixtures(fixtures, item.fixtures || [])
  const expanded = expandPlaceholders(item, { fixtures })
  if (expanded.protocol) return runProtocolCase(binary, expanded, root)
  if (Array.isArray(expanded.steps)) return runStepCase(binary, expanded, root)
  return runSingleCommandCase(binary, expanded, root)
}

function runSingleCommandCase(binary, item, root) {
  const cwd = item.cwd ? path.resolve(root, item.cwd) : root
  mkdirp(cwd)
  const result = runCommand(binary, item.args || [], {
    cwd,
    stdin: item.stdin,
    timeoutMs: item.timeoutMs || 30_000,
  })
  result.files = snapshotRequestedFiles(item)
  return result
}

function runStepCase(binary, item, root) {
  const steps = []
  for (const step of item.steps) {
    if (step.readFile) {
      const filePath = path.resolve(root, step.readFile)
      steps.push({
        kind: "readFile",
        path: step.readFile,
        exists: fs.existsSync(filePath),
        content: fs.existsSync(filePath) ? fs.readFileSync(filePath, "utf8") : null,
      })
      continue
    }
    const cwd = step.cwd ? path.resolve(root, step.cwd) : root
    mkdirp(cwd)
    const result = runCommand(binary, step.args || [], {
      cwd,
      stdin: step.stdin,
      timeoutMs: step.timeoutMs || item.timeoutMs || 30_000,
    })
    steps.push({ kind: "command", args: step.args || [], result: summarizeResult(result) })
  }
  return {
    status: steps.some((step) => step.kind === "command" && step.result.status !== 0) ? 1 : 0,
    stdout: "",
    stderr: "",
    timed_out: false,
    steps,
  }
}

async function runProtocolCase(binary, item, root) {
  if (item.protocol === "server-output") return runServerOutput(binary, item, root)
  if (item.protocol === "tsserver-open-close") return runTsserverProtocol(binary, item, root, "open-close")
  if (item.protocol === "tsserver-completions") return runTsserverProtocol(binary, item, root, "completions")
  if (item.protocol === "tsserver-diagnostics") return runTsserverProtocol(binary, item, root, "diagnostics")
  return {
    status: 2,
    stdout: "",
    stderr: `unsupported protocol: ${item.protocol}`,
    timed_out: false,
  }
}

function runServerOutput(binary, item, root) {
  return new Promise((resolve) => {
    const cwd = item.cwd ? path.resolve(root, item.cwd) : root
    mkdirp(cwd)
    const shell = process.platform === "win32" && (/\.cmd$/i.test(binary) || (!/[\\/]/.test(binary) && !path.extname(binary)))
    const child = spawn(binary, (item.args || []).map(String), {
      cwd,
      stdio: ["pipe", "pipe", "pipe"],
      env: stableEnv(),
      shell,
      windowsHide: true,
    })
    if (item.stdin) child.stdin.end(item.stdin)
    else child.stdin.end()
    let stdout = ""
    let stderr = ""
    let finished = false
    let readyTimer = null
    const timeout = setTimeout(() => finish({ timedOut: true }), item.timeoutMs || 15_000)
    const readyPattern = item.readyPattern ? new RegExp(item.readyPattern) : null
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8")
      maybeReady()
    })
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8")
      maybeReady()
    })
    child.on("error", (error) => finish({ error: String(error.stack || error.message || error) }))
    child.on("exit", (code) => finish({ exitCode: code ?? 0 }))
    const settleTimer = setTimeout(() => finish(), item.settleMs || 3000)

    function maybeReady() {
      if (!readyPattern || readyTimer) return
      if (readyPattern.test(`${stdout}\n${stderr}`)) {
        readyTimer = setTimeout(() => finish(), item.readySettleMs || 250)
      }
    }

    function finish(extra = {}) {
      if (finished) return
      finished = true
      clearTimeout(timeout)
      clearTimeout(settleTimer)
      if (readyTimer) clearTimeout(readyTimer)
      killProcessTree(child)
      setTimeout(() => resolve({
        status: extra.timedOut || extra.error ? 1 : extra.exitCode ?? 0,
        stdout,
        stderr: extra.error ? `${stderr}\n${extra.error}` : stderr,
        timed_out: Boolean(extra.timedOut),
      }), 250)
    }
  })
}

function runTsserverProtocol(binary, item, root, mode) {
  return new Promise((resolve) => {
    const file = item.file ? path.resolve(root, item.file) : null
    const child = spawn(binary, [], {
      cwd: root,
      stdio: ["pipe", "pipe", "pipe"],
      env: stableEnv(),
      shell: process.platform === "win32" && /\.cmd$/i.test(binary),
      windowsHide: true,
    })
    let seq = 0
    let stdout = ""
    let stderr = ""
    let buffer = Buffer.alloc(0)
    const messages = []
    let finishing = false
    const timer = setTimeout(() => finish({ timedOut: true }), item.timeoutMs || 10_000)
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8")
      buffer = Buffer.concat([buffer, chunk])
      parseProtocolMessages()
    })
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8")
    })
    child.on("error", (error) => finish({ error: String(error.stack || error.message || error) }))

    function send(commandName, argumentsObject = {}) {
      seq += 1
      const payload = JSON.stringify({ seq, type: "request", command: commandName, arguments: argumentsObject })
      child.stdin.write(`${payload}\n`)
      return seq
    }

    function parseProtocolMessages() {
      while (true) {
        const headerEnd = buffer.indexOf("\r\n\r\n")
        if (headerEnd < 0) return
        const header = buffer.slice(0, headerEnd).toString("utf8")
        const match = header.match(/Content-Length: (\d+)/i)
        if (!match) {
          buffer = buffer.slice(headerEnd + 4)
          continue
        }
        const length = Number(match[1])
        const start = headerEnd + 4
        if (buffer.length < start + length) return
        const body = buffer.slice(start, start + length).toString("utf8")
        buffer = buffer.slice(start + length)
        try {
          messages.push(JSON.parse(body))
        } catch {}
      }
    }

    function finish(extra = {}) {
      if (finishing) return
      finishing = true
      clearTimeout(timer)
      try {
        child.stdin.end()
      } catch {}
      try {
        child.kill()
      } catch {}
      setTimeout(() => {
        parseProtocolMessages()
        const responses = messages.filter((message) => message.type === "response")
        const last = responses[responses.length - 1] || null
        resolve({
          status: extra.timedOut || extra.error ? 1 : 0,
          stdout,
          stderr: extra.error ? `${stderr}\n${extra.error}` : stderr,
          timed_out: Boolean(extra.timedOut),
          protocol: {
            mode,
            ok: Boolean(last && last.success !== false),
            responses: responses.map((message) => ({
              command: message.command,
              success: message.success,
              message: message.message || null,
              bodySummary: protocolBodySummary(message.body),
            })),
            messageCount: messages.length,
          },
        })
      }, 250)
    }

    if (file && fs.existsSync(file)) {
      send("open", { file, fileContent: fs.readFileSync(file, "utf8") })
    }
    if (mode === "completions" && file) {
      const pos = markerPosition(fs.readFileSync(file, "utf8"), item.marker)
      send("completionInfo", {
        file,
        line: pos.line,
        offset: pos.offset,
        includeExternalModuleExports: false,
        includeInsertTextCompletions: true,
        preferences: {
          includeCompletionsWithClassMemberSnippets: true,
          includeCompletionsWithInsertText: true,
        },
      })
    } else if (mode === "diagnostics" && file) {
      send("semanticDiagnosticsSync", { file })
    }
    setTimeout(() => finish(), item.settleMs || 1500)
  })
}

function protocolBodySummary(body) {
  if (!body) return null
  if (Array.isArray(body.entries)) {
    return { entries: body.entries.slice(0, 20).map((entry) => ({ name: entry.name, kind: entry.kind, insertText: entry.insertText || null })) }
  }
  if (Array.isArray(body)) return { items: body.slice(0, 20) }
  if (typeof body === "object") return Object.fromEntries(Object.entries(body).slice(0, 10))
  return body
}

function markerPosition(text, marker) {
  if (marker === "class-body") {
    const lines = text.split(/\r?\n/)
    for (let index = 0; index < lines.length; index += 1) {
      if (/^\s*$/.test(lines[index]) && index > 0 && /class\s+\w+/.test(lines.slice(0, index).join("\n"))) {
        return { line: index + 1, offset: Math.max(1, lines[index].length + 1) }
      }
    }
  }
  const index = text.indexOf("/*1*/")
  if (index >= 0) return offsetToLineOffset(text, index)
  return { line: 1, offset: 1 }
}

function offsetToLineOffset(text, offset) {
  const before = text.slice(0, offset)
  const lines = before.split(/\r?\n/)
  return { line: lines.length, offset: lines[lines.length - 1].length + 1 }
}

function snapshotRequestedFiles(item) {
  if (!Array.isArray(item.readFiles)) return []
  return item.readFiles.map((file) => ({
    path: file,
    exists: fs.existsSync(file),
    content: fs.existsSync(file) ? fs.readFileSync(file, "utf8") : null,
  }))
}

function observationsEqual(a, b, compare) {
  if (compare === "status_only") return a.status === b.status && Boolean(a.timed_out) === Boolean(b.timed_out)
  return JSON.stringify(observationFingerprint(a, compare)) === JSON.stringify(observationFingerprint(b, compare))
}

function observationFingerprint(result, compare) {
  if (compare === "fixed_protocol_result") {
    return {
      status: result.status,
      timed_out: Boolean(result.timed_out),
      protocol: result.protocol ? {
        mode: result.protocol.mode,
        ok: result.protocol.ok,
        responses: result.protocol.responses,
      } : null,
      stderr: normalize(result.stderr),
    }
  }
  if (Array.isArray(result.steps)) {
    return {
      steps: result.steps.map((step) => {
        if (step.kind === "readFile") return { kind: step.kind, exists: step.exists, content: step.content }
        return { kind: step.kind, result: observationFingerprint(step.result || {}, "normalized_streams") }
      }),
    }
  }
  return {
    status: result.status,
    stdout: normalize(result.stdout),
    stderr: normalize(result.stderr),
    timed_out: Boolean(result.timed_out),
    files: result.files || [],
  }
}

function summarizeResult(result) {
  return {
    status: result.status,
    timed_out: Boolean(result.timed_out),
    stdout_head: String(result.stdout || "").slice(0, 2000),
    stderr_head: String(result.stderr || "").slice(0, 2000),
    protocol: result.protocol || null,
    steps: result.steps || null,
    files: result.files || null,
  }
}

function runCommand(command, args = [], options = {}) {
  const shell = process.platform === "win32" && (/\.cmd$/i.test(command) || (!/[\\/]/.test(command) && !path.extname(command)))
  const result = spawnSync(command, args.map(String), {
    cwd: options.cwd || process.cwd(),
    input: options.stdin ?? undefined,
    encoding: "utf8",
    errors: "replace",
    timeout: options.timeoutMs || 30_000,
    maxBuffer: options.maxBuffer || 16 * 1024 * 1024,
    env: stableEnv(),
    shell,
    windowsHide: true,
  })
  return {
    status: result.status === null ? 124 : result.status,
    stdout: result.stdout || "",
    stderr: (result.stderr || "") + (result.error ? `\n${result.error.message || result.error}` : ""),
    timed_out: Boolean(result.error && result.error.code === "ETIMEDOUT"),
  }
}

function killProcessTree(child) {
  if (!child?.pid) return
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
      stdio: "ignore",
      windowsHide: true,
    })
    return
  }
  try {
    child.kill("SIGTERM")
  } catch {}
}

function runOk(command, args, options = {}) {
  const result = runCommand(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`)
  }
  return result
}

function writeFixtures(root, fixtures) {
  for (const fixture of fixtures || []) {
    const file = path.join(root, fixture.path)
    mkdirp(path.dirname(file))
    fs.writeFileSync(file, fixture.content || "", "utf8")
  }
}

function expandPlaceholders(value, replacements) {
  if (typeof value === "string") {
    return Object.entries(replacements).reduce((text, [key, item]) => text.replaceAll(`{{${key}}}`, item), value)
  }
  if (Array.isArray(value)) return value.map((item) => expandPlaceholders(item, replacements))
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, expandPlaceholders(item, replacements)]))
  }
  return value
}

function writeCommandWrapper(wrapper, command, args) {
  mkdirp(path.dirname(wrapper))
  const resolvedCommand = command === "java" ? findJavaCommand() : command
  if (process.platform === "win32") {
    const quoted = [resolvedCommand, ...args].map((item) => `"${String(item).replace(/"/g, '""')}"`).join(" ")
    fs.writeFileSync(wrapper, `@echo off\r\n${quoted} %*\r\n`, "utf8")
  } else {
    const quoted = [resolvedCommand, ...args].map((item) => `'${String(item).replace(/'/g, "'\\''")}'`).join(" ")
    fs.writeFileSync(wrapper, `#!/usr/bin/env sh\nexec ${quoted} "$@"\n`, "utf8")
    fs.chmodSync(wrapper, 0o755)
  }
}

function findJavaCommand() {
  if (process.env.JAVA_EXE && fs.existsSync(process.env.JAVA_EXE)) return process.env.JAVA_EXE
  if (process.env.JAVA_HOME) {
    const homeJava = path.join(process.env.JAVA_HOME, "bin", process.platform === "win32" ? "java.exe" : "java")
    if (fs.existsSync(homeJava)) return homeJava
  }
  if (process.platform === "win32") {
    const home = process.env.USERPROFILE || process.env.HOME || ""
    const candidates = [
      path.join(home, "Documents", "tura_workspace", "tools", "jdk21-extract", "jdk-21.0.11+10", "bin", "java.exe"),
      path.join(home, "jdk-22.0.2+9", "bin", "java.exe"),
      "java",
    ]
    return candidates.find((candidate) => candidate === "java" || fs.existsSync(candidate)) || "java"
  }
  return "java"
}

function platformInfo() {
  const arch = process.arch === "x64" ? "x64" : process.arch === "arm64" ? "arm64" : process.arch
  return { os: process.platform, arch }
}

function isRawBinaryAsset(file) {
  const lower = file.toLowerCase()
  return lower.endsWith(".exe") || (!lower.endsWith(".zip") && !lower.endsWith(".tar.gz") && !lower.endsWith(".tgz") && !lower.endsWith(".tar.xz"))
}

function extractArchive(archive, dest) {
  const lower = archive.toLowerCase()
  if (lower.endsWith(".zip")) {
    runOk("powershell", [
      "-NoProfile",
      "-Command",
      `Expand-Archive -LiteralPath ${JSON.stringify(archive)} -DestinationPath ${JSON.stringify(dest)} -Force`,
    ], { timeoutMs: 5 * 60_000 })
  } else if (lower.endsWith(".tar.gz") || lower.endsWith(".tgz")) {
    runOk("tar", ["-xzf", archive, "-C", dest], { timeoutMs: 5 * 60_000 })
  } else if (lower.endsWith(".tar.xz")) {
    runOk("tar", ["-xJf", archive, "-C", dest], { timeoutMs: 5 * 60_000 })
  } else {
    throw new Error(`unsupported archive type: ${archive}`)
  }
}

function findBinaryInDir(dir, binaryNames) {
  const stack = [dir]
  const names = new Set(binaryNames.flatMap((name) => [name.toLowerCase(), `${name.toLowerCase()}.exe`]))
  const candidates = []
  while (stack.length > 0) {
    const current = stack.pop()
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(full)
      else if (entry.isFile() && names.has(entry.name.toLowerCase())) candidates.push(full)
    }
  }
  if (candidates.length === 0) throw new Error(`could not find ${[...names].join("/")} under ${dir}`)
  candidates.sort((a, b) => a.length - b.length)
  return candidates[0]
}

function normalize(text) {
  return String(text || "")
    .replace(/\r\n/g, "\n")
    .replace(/\x1b\[[0-9;?]*[A-Za-z]/g, "")
    .replace(/\b(VITE|vite)[/ ]v?\d+\.\d+\.\d+(?:[-.\w]*)?/g, "$1/<VERSION>")
    .replace(/(?<![A-Za-z])[A-Z]:[\\/][^\s)]+/gi, "<PATH>")
    .replace(/(?<![:/])\/[^\s)]+/g, "<PATH>")
    .replace(/\t\d{4}-\d\d-\d\d[^\n]*/g, "\t<TIMESTAMP>")
    .replace(/\b\d+(?:\.\d+)?\s*(?:ns|us|µs|ms|s)\b/g, "<DURATION>")
}

function stableEnv() {
  return {
    ...process.env,
    NO_COLOR: "1",
    CLICOLOR: "0",
    TERM: "dumb",
    PYTHONIOENCODING: "utf-8",
    PYTHONUTF8: "1",
  }
}

function resetDir(dir, allowedRoot) {
  assertSafeChildPath(dir, allowedRoot)
  fs.rmSync(dir, { recursive: true, force: true })
  mkdirp(dir)
}

function cleanExtractDir(dir, allowedRoot) {
  resetDir(dir, allowedRoot)
}

function assertSafeChildPath(target, allowedRoot) {
  const resolved = path.resolve(target)
  const root = path.resolve(allowedRoot)
  assert(resolved === root || resolved.startsWith(`${root}${path.sep}`), `refusing to remove path outside ${root}: ${resolved}`)
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeJson(file, value) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8")
}

function safeName(value) {
  return String(value).replace(/[^A-Za-z0-9_.-]+/g, "-").replace(/^-+|-+$/g, "") || "item"
}

function truthy(value) {
  return ["1", "true", "yes", "on", "enabled"].includes(String(value || "").trim().toLowerCase())
}
