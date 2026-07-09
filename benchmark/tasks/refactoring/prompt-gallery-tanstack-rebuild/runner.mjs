#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { agentEventStats, agentUsageFromJsonl, claudeCodeArgs, findClaudeExe, findPiExe, piAgentArgs } from "../../../lib/agent_cli.mjs"
import { businessRunPaths, normalizeBusinessSummary } from "../../../lib/business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `makeup-tanstack-${Date.now()}`
const taskVersion = process.env.COMMAND_RUN_MAKEUP_TANSTACK_VERSION || "fullstack"
const isFullStackVersion = taskVersion !== "frontend"
const runPaths = businessRunPaths(isFullStackVersion ? "project-rebuild-makeup-tanstack-fullstack" : "project-rebuild-makeup-tanstack-frontend", runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path
const desktopDir = process.env.COMMAND_RUN_MAKEUP_DESKTOP || path.join(runRoot, "projects")
const sourceHtml = resolveSourceHtml()
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 25 * 60_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "codex,tura")
const prepOnly = (process.env.COMMAND_RUN_AGENT_PREP_ONLY || "0") === "1"
const evaluateOnly = (process.env.COMMAND_RUN_AGENT_EVALUATE_ONLY || "0") === "1"
const skipEval = (process.env.COMMAND_RUN_AGENT_SKIP_EVAL || "0") === "1"
const skipTuraBuild = (process.env.COMMAND_RUN_AGENT_SKIP_TURA_BUILD || "0") === "1"
const allowFailure = (process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE || "0") === "1"
const turaEmbedded = (process.env.COMMAND_RUN_AGENT_TURA_EMBEDDED || "0") === "1"
const turaExplicitSessionId = (process.env.COMMAND_RUN_AGENT_TURA_EXPLICIT_SESSION_ID || "0") === "1"
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm"
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx"

const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const claudeExe = findClaudeExe()
const piExe = findPiExe()
const codexExe = path.join(
  process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex"),
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
)
const codexMainExe = path.join(
  process.env.COMMAND_RUN_AGENT_CODEX_MAIN_ROOT || path.join(homeDir, "Documents", "codex-main"),
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
)

function resolveSourceHtml() {
  const candidates = [
    process.env.COMMAND_RUN_MAKEUP_HTML,
    "/Users/jayden/Downloads/makeup.html",
    path.join(homeDir, "Downloads", "makeup.html"),
    path.join(scriptDir, "makeup.html"),
  ].filter(Boolean)
  return candidates.find((candidate) => fs.existsSync(candidate)) || candidates[candidates.length - 1]
}

function parseAgents(value) {
  const alias = new Map([
    ["codex", "codex"],
    ["codex-current", "codex"],
    ["current", "codex"],
    ["codex-main", "codex-main"],
    ["main", "codex-main"],
    ["tura", "tura-fast"],
    ["tura-fast", "tura-fast"],
    ["tura-fast-shll", "tura-fast"],
    ["tura-balanced", "tura-balanced"],
    ["balanced", "tura-balanced"],
    ["tura-direct", "tura-direct"],
    ["direct", "tura-direct"],
    ["tura-thinking", "tura-thinking"],
    ["tura-think", "tura-thinking"],
    ["thinking", "tura-thinking"],
    ["tura-coding", "tura-thinking"],
    ["tura-shll", "tura-thinking"],
    ["claude", "claude-code"],
    ["claude-code", "claude-code"],
    ["claude-opus", "claude-code"],
    ["pi", "pi-agent"],
    ["pi-agent", "pi-agent"],
    ["pi-coding-agent", "pi-agent"],
  ])
  return String(value)
    .split(",")
    .map((item) => alias.get(item.trim().toLowerCase()))
    .filter(Boolean)
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
    maxBuffer: options.maxBuffer || 256 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    shell: options.shell || false,
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

async function runLive(command, args, options = {}) {
  const started = performance.now()
  const stdoutChunks = []
  const stderrChunks = []
  mkdirp(path.dirname(options.stdoutPath))
  const stdoutStream = fs.createWriteStream(options.stdoutPath)
  const stderrStream = fs.createWriteStream(options.stderrPath)
  return await new Promise((resolve) => {
    let progressQueued = false
    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: [options.input ? "pipe" : "ignore", "pipe", "pipe"],
      shell: options.shell || false,
      windowsHide: true,
    })
    if (options.input) {
      child.stdin.write(options.input)
      child.stdin.end()
    }
    let settled = false
    const timeout = setTimeout(() => {
      if (settled) return
      settled = true
      killProcessTree(child)
      finish(null, "timeout")
    }, options.timeoutMs || timeoutMs)
    child.stdout.on("data", (chunk) => {
      stdoutChunks.push(chunk)
      stdoutStream.write(chunk)
      queueProgress()
    })
    child.stderr.on("data", (chunk) => {
      stderrChunks.push(chunk)
      stderrStream.write(chunk)
      queueProgress()
    })
    child.on("error", (error) => {
      if (settled) return
      settled = true
      clearTimeout(timeout)
      finish(null, null, error)
    })
    child.on("close", (status, signal) => {
      if (settled) return
      settled = true
      clearTimeout(timeout)
      finish(status, signal)
    })
    function finish(status, signal, error = null) {
      stdoutStream.end()
      stderrStream.end()
      const stdout = Buffer.concat(stdoutChunks).toString("utf8")
      const stderr = Buffer.concat(stderrChunks).toString("utf8")
      const result = {
        command,
        args,
        status,
        signal,
        stdout,
        stderr,
        stdout_path: options.stdoutPath,
        stderr_path: options.stderrPath,
        status_path: options.statusPath,
        duration_ms: Math.round(performance.now() - started),
        error: error ? String(error.stack || error.message || error) : signal === "timeout" ? "timeout" : null,
      }
      writeFile(options.statusPath, JSON.stringify(result, null, 2))
      options.onProgress?.(result)
      resolve(result)
    }
    function queueProgress() {
      if (!options.onProgress || progressQueued || settled) return
      progressQueued = true
      setTimeout(() => {
        progressQueued = false
        if (settled) return
        options.onProgress({
          command,
          args,
          status: null,
          signal: null,
          stdout: Buffer.concat(stdoutChunks).toString("utf8"),
          stderr: Buffer.concat(stderrChunks).toString("utf8"),
          stdout_path: options.stdoutPath,
          stderr_path: options.stderrPath,
          status_path: options.statusPath,
          duration_ms: Math.round(performance.now() - started),
          error: null,
        })
      }, 1000)
    }
  })
}

function killProcessTree(child) {
  try {
    if (process.platform === "win32" && child.pid) {
      spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true })
    } else {
      child.kill("SIGTERM")
    }
  } catch {}
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

function reasoningConfigArgs() {
  return ["-c", `model_reasoning_effort="${reasoning}"`]
}

function turaReasoningArgs() {
  return ["--model-reasoning-effort", reasoning]
}

function parseJsonl(text) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try { return JSON.parse(line) } catch { return null }
    })
    .filter(Boolean)
}

function usageFromEvents(events) {
  const usage = { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 }
  for (const event of events) {
    const u = event.usage || event.message?.usage || event.payload?.info?.last_token_usage
    if (!u) continue
    const input = Number(u.input_tokens || u.prompt_tokens || 0)
    const output = Number(u.output_tokens || u.completion_tokens || 0)
    usage.input += input
    usage.cached += Number(u.cached_input_tokens || u.input_tokens_details?.cached_tokens || u.prompt_tokens_details?.cached_tokens || 0)
    usage.cached += Number(u.cache_read_input_tokens || 0)
    usage.output += output
    usage.reasoning += Number(u.reasoning_output_tokens || u.reasoning_tokens || u.output_tokens_details?.reasoning_tokens || u.completion_tokens_details?.reasoning_tokens || 0)
    usage.total += Number(u.total_tokens || 0) || input + output
  }
  return usage
}

function countEvents(events) {
  let commands = 0
  let failures = 0
  let turns = 0
  for (const event of events) {
    if (event.type === "turn.started" || event.type === "thread.started") turns += 1
    if (Array.isArray(event.message?.content) && event.message.content.some((part) => part?.type === "tool_use")) commands += 1
    if (event.item?.type === "command_execution" && event.item.status === "completed") {
      commands += 1
      if (event.item.exit_code && event.item.exit_code !== 0) failures += 1
    }
  }
  return { turns, commands, failures }
}

function contextArchiveDir(agentDir) {
  return path.join(agentDir, "context-and-calls")
}

function emptyAgentUsage() {
  return {
    input: 0,
    cached: 0,
    output: 0,
    reasoning: 0,
    total: 0,
    usage_events: 0,
    latency_ms: 0,
  }
}

function addAgentUsage(totals, usage) {
  if (!usage || typeof usage !== "object") return
  totals.usage_events += 1
  const input = Number(usage.input || usage.input_tokens || usage.prompt_tokens || 0)
  const cached = Number(usage.cached || usage.cached_input_tokens || usage.cache_read_input_tokens || usage.input_tokens_details?.cached_tokens || usage.prompt_tokens_details?.cached_tokens || 0)
  const output = Number(usage.output || usage.output_tokens || usage.completion_tokens || 0)
  const reasoning = Number(usage.reasoning || usage.reasoning_tokens || usage.reasoning_output_tokens || usage.output_tokens_details?.reasoning_tokens || usage.completion_tokens_details?.reasoning_tokens || 0)
  totals.input += input
  totals.cached += cached
  totals.output += output
  totals.reasoning += reasoning
  totals.total += Number(usage.total || usage.total_tokens || 0) || input + output
  totals.latency_ms += Number(usage.latency_ms || 0)
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

function usageFromProviderLogs(logRoot) {
  const totals = emptyAgentUsage()
  const calls = []
  for (const file of jsonFilesUnder(logRoot)) {
    let payload
    try { payload = JSON.parse(fs.readFileSync(file, "utf8")) } catch { continue }
    if (payload?.type !== "llm_call") continue
    const usage = payload.metrics?.usage || payload.response?.usage
    if (usage) addAgentUsage(totals, usage)
    calls.push({
      file,
      call_id: payload.call_id,
      success: payload.success,
      provider: payload.provider,
      model: payload.model,
      started_at: payload.started_at,
      finished_at: payload.finished_at,
      duration_ms: payload.duration_ms,
      usage: usage || null,
    })
  }
  calls.sort((a, b) => String(a.started_at || "").localeCompare(String(b.started_at || "")))
  return { totals, calls }
}

function refreshContextAndCallArchive(agentDir, stdout = "") {
  const archive = contextArchiveDir(agentDir)
  mkdirp(archive)
  const providerLogRoot = path.join(agentDir, "provider-log")
  const fullCalls = []
  for (const file of jsonFilesUnder(providerLogRoot).sort()) {
    let payload = null
    try { payload = JSON.parse(fs.readFileSync(file, "utf8")) } catch {}
    if (payload?.type !== "llm_call") continue
    fullCalls.push({ source_file: file, ...payload })
  }
  writeFile(
    path.join(archive, "provider-calls-full.jsonl"),
    fullCalls.map((call) => JSON.stringify(call)).join("\n") + (fullCalls.length ? "\n" : ""),
  )
  writeFile(path.join(archive, "visible-agent-events.jsonl"), stdout || "")
  writeFile(path.join(archive, "provider-calls-index.json"), JSON.stringify(fullCalls.map((call, index) => ({
    index,
    source_file: call.source_file,
    call_id: call.call_id,
    success: call.success,
    provider: call.provider,
    model: call.model,
    started_at: call.started_at,
    finished_at: call.finished_at,
    duration_ms: call.duration_ms,
    usage: call.metrics?.usage || call.response?.usage || null,
    request_messages_count: Array.isArray(call.request?.messages) ? call.request.messages.length : null,
    has_request_messages: Array.isArray(call.request?.messages),
    has_response: Boolean(call.response),
  })), null, 2))
  writeFile(path.join(archive, "archive-summary.json"), JSON.stringify({
    provider_log_root: providerLogRoot,
    provider_call_count: fullCalls.length,
    provider_calls_full_path: path.join(archive, "provider-calls-full.jsonl"),
    provider_calls_include_full_request_messages: fullCalls.some((call) => Array.isArray(call.request?.messages)),
    visible_events_path: path.join(archive, "visible-agent-events.jsonl"),
  }, null, 2))
  return {
    archive_dir: archive,
    provider_call_count: fullCalls.length,
    provider_calls_full_path: path.join(archive, "provider-calls-full.jsonl"),
  }
}

function usageForAgent(agentDir, stdout) {
  const provider = usageFromProviderLogs(path.join(agentDir, "provider-log"))
  if (provider.totals.usage_events > 0) return { usage: provider.totals, usage_source: "provider_log", provider_calls: provider.calls }
  return { usage: usageFromEvents(parseJsonl(stdout || "")), usage_source: "stdout_jsonl", provider_calls: [] }
}

function prepareProject(agent) {
  const dir = existingProject(agent)
  fs.rmSync(dir, { recursive: true, force: true })
  mkdirp(dir)
  fs.copyFileSync(sourceHtml, path.join(dir, "makeup.html"))
  writeFile(path.join(dir, "README-task.md"), taskReadme(agent))
  return dir
}

function existingProject(agent) {
  return path.join(desktopDir, projectDirectoryName(agent))
}

function projectDirectoryName(agent) {
  if (agent === "codex") return "makeup-codex"
  if (agent === "codex-main") return "makeup-codex-main"
  if (agent === "tura-fast") return "makeup-tura-fast"
  if (agent === "tura-thinking") return "makeup-tura-thinking"
  if (agent === "tura-balanced") return "makeup-tura-balanced"
  if (agent === "tura-direct") return "makeup-tura-direct"
  return `makeup-${agent.replace(/[^a-z0-9-]/gi, "-").toLowerCase()}`
}

function turaAgentPrompt(agent) {
  if (agent === "tura-balanced") return "balanced"
  if (agent === "tura-direct") return "direct"
  return agent === "tura-thinking" ? "thinking" : "fast"
}

function taskReadme(agent) {
  if (!isFullStackVersion) {
    return `# Prompt Gallery TanStack frontend rebuild task

Agent: ${agent}
Source HTML: ${path.basename(sourceHtml)}

Convert makeup.html into a production-quality TanStack Start React frontend in this directory.

Requirements:
- Use a real TanStack Start application structure, including the appropriate app/vite/start configuration and file based routing under src/routes. Do not make a plain Vite-only React app with a decorative TanStack dependency.
- Preserve the original page's visual identity, typography, responsive layout, and interactions from makeup.html.
- Recreate the complete frontend product experience from the source page: the POWERPROMPT brand, sticky sidebar/navigation, top model filter bar, sort controls, search reveal, masonry-style prompt gallery, varied media card aspect ratios, image-based prompt previews, hover overlays, save/favorite behavior, cart/dock actions, toast feedback, lightbox/detail preview, and mobile drawer/dock layout.
- Keep the source domain vocabulary visible where it belongs, including the original model/filter/sort concepts such as GPT-4o, Claude, Midjourney, Flux, Featured, Newest, Popular, Favorites, and Cart.
- Split meaningful UI into React components instead of leaving one giant HTML string.
- Keep styles maintainable in source files that belong to the app.
- Provide npm scripts for dev, build, start or preview, and a Playwright browser smoke/e2e test.
- Install Playwright by default in package.json, preferably both @playwright/test and playwright or whichever your test script imports.
- Verify with npm install, npm run build, and at least one Playwright browser smoke check.
- Do not ask the user questions or stop early while the task can still be completed locally. Keep setting up the environment, fixing failures, running the required tests, and iterating until the app is complete and the tests pass. Only ask a question if the current environment truly cannot run the required validation after reasonable setup effort.
- Do not read or compare against sibling benchmark projects or previous outputs.
`
  }
  return `# Prompt Gallery Full-Stack TanStack rebuild task

Agent: ${agent}
Source HTML: ${path.basename(sourceHtml)}

Convert makeup.html into a production full-stack TanStack Start product in this directory.

Requirements:
- Use a real TanStack Start application structure, including the appropriate app/vite/start configuration and file based routing under src/routes. Do not make a plain Vite-only React app with a decorative TanStack dependency.
- Preserve the original page's visual identity, typography, responsive layout, and interactions from makeup.html.
- Recreate the complete frontend product experience from the source page: the POWERPROMPT brand, sticky sidebar/navigation, top model filter bar, sort controls, search reveal, masonry-style prompt gallery, varied media card aspect ratios, image-based prompt previews, hover overlays, save/favorite behavior, cart/dock actions, toast feedback, lightbox/detail preview, and mobile drawer/dock layout.
- Keep the source domain vocabulary visible where it belongs, including the original model/filter/sort concepts such as GPT-4o, Claude, Midjourney, Flux, Featured, Newest, Popular, Favorites, and Cart.
- Split meaningful UI into React components instead of leaving one giant HTML string.
- Keep styles maintainable in source files that belong to the app.
- Add a real backend layer for the prompt marketplace. Use TanStack Start server functions, API routes, or server-side route loaders/actions for catalog reads, search/filtering, favorites/cart mutations, checkout simulation, and creator/admin analytics.
- Add a local database layer with seed data derived from the source page. SQLite is preferred. Keep schema, seed data, and query helpers in source-controlled files. If you choose a file database, it must be created under the project workspace only.
- Implement database-side calculations, not just frontend array math. Required computed data includes prompt ranking, featured/free filters, cart totals, creator revenue, conversion rate, average price, category totals, and daily sales or trend summaries.
- Surface those backend/database values in the UI through routes, loaders, API calls, or server functions. Include at least a storefront route, prompt detail route, cart or checkout route, and creator/admin analytics route.
- Provide npm scripts for dev, build, and start or preview.
- Provide more than one test command. Include unit tests for database/query calculations, API/server-function tests for backend behavior, and a browser/component/e2e smoke test for the main user flows.
- Install Playwright by default in package.json, preferably both @playwright/test and playwright or whichever your browser test imports.
- Install any required dependencies in package.json.
- Verify with npm install, npm run build, the database/API/unit test scripts, and at least one browser smoke check.
- Do not ask the user questions or stop early while the task can still be completed locally. Keep setting up the environment, fixing failures, running the required tests, and iterating until the app is complete and the tests pass. Only ask a question if the current environment truly cannot run the required validation after reasonable setup effort.
- Do not read or compare against the sibling makeup-codex or makeup-tura project.
`
}

function conversionPrompt() {
  if (!isFullStackVersion) {
    return `You are in a directory containing makeup.html and README-task.md. Turn this HTML into a production-quality TanStack Start React frontend in the current directory.

Follow README-task.md exactly. Preserve the page as a real app, not a screenshot or iframe. Use gpt-5.5 ${reasoning} reasoning style: act decisively, keep the implementation compact, and verify locally.

What matters most:
- Match the source page as a complete app, not just the general theme: POWERPROMPT branding, left sidebar, sticky top filters, search reveal, sort controls, masonry gallery, varied image cards, hover overlays, save/favorite state, cart/dock actions, toast feedback, lightbox/detail preview, and the mobile drawer/dock experience.
- Preserve the source model/filter/sort vocabulary where it belongs, including GPT-4o, Claude, Midjourney, Flux, Featured, Newest, Popular, Favorites, and Cart.
- Use real image/media presentation for the prompt cards and varied card proportions like the source. Avoid replacing the gallery with equal-height generic cards or purely CSS placeholder objects.
- Recreate the experience in idiomatic React and TanStack Start architecture, with real Start configuration, file routes, reusable components, and maintainable styling.
- Make the app robust in a browser: accessible controls, working media, no broken layout on desktop or mobile, and no console-visible shortcuts like iframes or raw HTML dumps.
- Keep runtime and code performance healthy: avoid unnecessary dependencies, oversized DOMs, duplicated markup, and heavy assets when lighter implementation works.
- Install Playwright by default in package.json and provide a runnable Playwright smoke/e2e test script that opens the app and checks desktop/mobile rendering.

Important constraints:
- Work only inside the current directory.
- Do not inspect sibling benchmark projects or any previous benchmark outputs.
- The result must be runnable by npm install and npm run build.
- Prefer npm run dev for development and npm run start or npm run preview for serving the built app.
- Do not make a plain Vite-only React app with TanStack listed only as a dependency; wire TanStack Start through the project configuration and routes.
- Keep code performance in mind: avoid unnecessary client state, huge duplicated markup, unused dependencies, and layout thrash.
- Do not ask the user questions or stop early while the task can still be completed locally. Keep setting up the environment, fixing failures, running the required tests, and iterating until the app is complete and the tests pass. Only ask a question if the current environment truly cannot run the required validation after reasonable setup effort.
- If you start a local server for verification, stop only the exact process id you started. Never run broad cleanup such as killing every node, npm, powershell, pwsh, codex, or tura process.
`
  }
  return `You are in a directory containing makeup.html and README-task.md. Turn this HTML into a production-quality full-stack TanStack Start prompt marketplace in the current directory.

Follow README-task.md exactly. Preserve the page as a real app, not a screenshot or iframe. Use gpt-5.5 ${reasoning} reasoning style: act decisively, keep the implementation compact, and verify locally.

What matters most:
- Match the source page as a complete frontend app, not just the general theme: POWERPROMPT branding, left sidebar, sticky top filters, search reveal, sort controls, masonry gallery, varied image cards, hover overlays, save/favorite state, cart/dock actions, toast feedback, lightbox/detail preview, and the mobile drawer/dock experience.
- Preserve the source model/filter/sort vocabulary where it belongs, including GPT-4o, Claude, Midjourney, Flux, Featured, Newest, Popular, Favorites, and Cart.
- Use real image/media presentation for the prompt cards and varied card proportions like the source. Avoid replacing the gallery with equal-height generic cards or purely CSS placeholder objects.
- Recreate the experience in idiomatic React and TanStack Start architecture, with real Start configuration, file routes, reusable components, and maintainable styling.
- Turn the static gallery into a complete product flow: storefront, prompt detail, search/filter/sort, favorites or cart, checkout simulation, creator/admin analytics, and persistent seed data.
- Build a backend boundary using TanStack Start server functions, route APIs, loaders/actions, or server-only modules. The UI must not be just a static in-memory clone.
- Use a local database layer. SQLite is preferred; a well-structured file database or embedded SQL adapter is acceptable if it is local to this workspace. Seed at least 12 prompts, 4 creators, orders/sales rows, category rows, and user/cart/favorite rows.
- Put important business calculations in database/query code: ranked prompts, free/paid filter counts, cart subtotal/fees/total, creator revenue, conversion rate, average order value, category revenue, and daily sales/trend summaries.
- Add route/API/server-function tests for those calculations and flows. A stronger answer has separate scripts such as test:db, test:api, test:e2e or equivalent.
- Make the app robust in a browser: accessible controls, working media, no broken layout on desktop or mobile, and no console-visible shortcuts like iframes or raw HTML dumps.
- Keep runtime and code performance healthy: avoid unnecessary dependencies, oversized DOMs, duplicated markup, and heavy assets when lighter implementation works.

Important constraints:
- Work only inside the current directory.
- Do not inspect sibling Desktop projects or any previous benchmark outputs.
- The result must be runnable by npm install and npm run build.
- Prefer npm run dev for development and npm run start or npm run preview for serving the built app.
- Do not make a plain Vite-only React app with TanStack listed only as a dependency; wire TanStack Start through the project configuration and routes.
- The result must include runnable tests for database calculations and backend/API behavior, not just snapshot-style frontend checks.
- Keep code performance in mind: avoid unnecessary client state, huge duplicated markup, unused dependencies, and layout thrash.
- Do not ask the user questions or stop early while the task can still be completed locally. Keep setting up the environment, fixing failures, running the required tests, and iterating until the app is complete and the tests pass. Only ask a question if the current environment truly cannot run the required validation after reasonable setup effort.
- If you start a local server for verification, stop only the exact process id you started. Never run broad cleanup such as killing every node, npm, powershell, pwsh, codex, or tura process.
`
}

async function runCodex(agent, workspace, agentDir, onProgress = null) {
  const exe = agent === "codex-main" ? codexMainExe : codexExe
  const args = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "-C",
    workspace,
    "-m",
    model,
    "--dangerously-bypass-approvals-and-sandbox",
    ...reasoningConfigArgs(),
    ...serviceTierConfigArgs(),
    conversionPrompt(),
  ]
  return await runLive(exe, args, {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "codex.stdout.jsonl"),
    stderrPath: path.join(agentDir, "codex.stderr.log"),
    statusPath: path.join(agentDir, "codex.status.json"),
    onProgress,
  })
}

async function runTura(agent, workspace, agentDir, onProgress = null) {
  const sessionId = `makeup-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  const sessionCwd = prepareTuraSessionCwd(sessionId)
  const providerLogPath = path.join(agentDir, "provider-log")
  mkdirp(providerLogPath)
  const args = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    ...(turaEmbedded ? ["--embedded"] : []),
    ...(turaExplicitSessionId ? ["--session-id", sessionId] : []),
    "--sandbox",
    "--agent-id",
    turaAgentPrompt(agent),
    "-m",
    turaModel,
    ...turaServiceTierConfigArgs(),
    ...turaReasoningArgs(),
    "--cwd",
    workspace,
  ]
  return await runLive(turaExe, args, {
    cwd: sessionCwd,
    input: conversionPrompt(),
    timeoutMs,
    resolveOnTurnCompleted: true,
    cleanupWorkspaceOnSettle: true,
    env: {
      ...(process.env.OPENAI_LOGIN ? { OPENAI_LOGIN: process.env.OPENAI_LOGIN } : {}),
      TURA_ENV_PATH: process.env.TURA_ENV_PATH || path.join(repoRoot, ".env"),
      TURA_PROJECT_ROOT: repoRoot,
      LOG_PATH: providerLogPath,
      TURA_COMMAND_RUN_STRICT_JSON: "0",
      TURA_SESSION_REASONING_EFFORT: reasoning,
      COMMAND_RUN_AGENT_CONTEXT_ARCHIVE: "1",
    },
    stdoutPath: path.join(agentDir, "tura.stdout.jsonl"),
    stderrPath: path.join(agentDir, "tura.stderr.log"),
    statusPath: path.join(agentDir, "tura.status.json"),
    onProgress,
  })
}

function prepareTuraSessionCwd(sessionId) {
  const safe = sessionId.replace(/[^A-Za-z0-9_.-]/g, "_").slice(0, 80)
  const dir = path.join(runRoot, "tura-session-cwd", safe)
  mkdirp(path.join(dir, "crates", "session_log"))
  writeFile(path.join(dir, "Cargo.toml"), "[workspace]\n")
  return dir
}

async function runClaudeCode(workspace, agentDir, onProgress = null) {
  return await runLive(claudeExe, claudeCodeArgs(conversionPrompt(), { model: process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus" }), {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "claude-code.stdout.jsonl"),
    stderrPath: path.join(agentDir, "claude-code.stderr.log"),
    statusPath: path.join(agentDir, "claude-code.status.json"),
    onProgress,
  })
}

async function runPiAgent(workspace, agentDir, onProgress = null) {
  return await runLive(piExe, piAgentArgs(conversionPrompt()), {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "pi-agent.stdout.jsonl"),
    stderrPath: path.join(agentDir, "pi-agent.stderr.log"),
    statusPath: path.join(agentDir, "pi-agent.status.json"),
    onProgress,
  })
}

function packageJson(workspace) {
  const file = path.join(workspace, "package.json")
  if (!fs.existsSync(file)) return null
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"))
  } catch {
    return null
  }
}

function listSourceFiles(workspace) {
  const roots = ["src", "app", "routes"].map((name) => path.join(workspace, name)).filter((dir) => fs.existsSync(dir))
  const out = []
  for (const root of roots) walk(root, out)
  const rootConfigs = [
    "package.json",
    "app.config.ts",
    "app.config.mjs",
    "vite.config.ts",
    "vite.config.mjs",
    "vinxi.config.ts",
    "tanstack.config.ts",
  ].map((name) => path.join(workspace, name)).filter((file) => fs.existsSync(file))
  return [
    ...out.filter((file) => /\.(tsx?|jsx?|css|scss)$/.test(file)),
    ...rootConfigs,
  ]
}

function walk(dir, out) {
  for (const item of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, item.name)
    if (item.isDirectory()) {
      if (!["node_modules", ".git", "dist", "build", ".vinxi"].includes(item.name)) walk(full, out)
    } else {
      out.push(full)
    }
  }
}

function byteSize(dir) {
  if (!fs.existsSync(dir)) return 0
  let total = 0
  for (const file of listAllFiles(dir)) total += fs.statSync(file).size
  return total
}

function listAllFiles(dir) {
  const out = []
  for (const item of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, item.name)
    if (item.isDirectory()) listAllFiles(full).forEach((file) => out.push(file))
    else out.push(full)
  }
  return out
}

function listProjectFiles(dir) {
  const out = []
  function visit(current) {
    if (!fs.existsSync(current)) return
    for (const item of fs.readdirSync(current, { withFileTypes: true })) {
      if (item.isDirectory() && ["node_modules", ".git", "dist", "build", ".output", ".vinxi", ".tanstack"].includes(item.name)) continue
      const full = path.join(current, item.name)
      if (item.isDirectory()) visit(full)
      else out.push(full)
    }
  }
  visit(dir)
  return out
}

function codeMetrics(workspace) {
  const files = listSourceFiles(workspace)
  let lines = 0
  let bytes = 0
  let componentSignals = 0
  let tanstackSignals = 0
  let rawHtmlSignals = 0
  let cssSignals = 0
  let responsiveSignals = 0
  let accessibilitySignals = 0
  let assetSignals = 0
  let duplicateClassSignals = 0
  let longestFileLines = 0
  let backendSignals = 0
  let databaseSignals = 0
  let databaseCalculationSignals = 0
  let routeSignals = 0
  let routeFileSignals = 0
  let routerProviderSignals = 0
  let tanstackStartConfigSignals = 0
  let tanstackStartConfigFileSignals = 0
  let cssTokenSignals = 0
  let testSignals = 0
  let seedDataSignals = 0
  const classNames = new Map()
  for (const file of files) {
    const rel = path.relative(workspace, file).replace(/\\/g, "/")
    const text = fs.readFileSync(file, "utf8")
    const fileLines = text.split(/\r?\n/).length
    lines += fileLines
    longestFileLines = Math.max(longestFileLines, fileLines)
    bytes += Buffer.byteLength(text)
    componentSignals += (text.match(/function\s+[A-Z]\w+|const\s+[A-Z]\w+\s*=/g) || []).length
    tanstackSignals += (text.match(/createFileRoute|@tanstack\/react-start|@tanstack\/react-router/g) || []).length
    routeFileSignals += /^src\/routes\/.+\.(tsx?|jsx?)$/i.test(rel) ? 1 : 0
    routerProviderSignals += (text.match(/RouterProvider|createRouter|createRootRoute|createFileRoute/g) || []).length
    tanstackStartConfigSignals += (text.match(/@tanstack\/react-start|@tanstack\/start|TanStackStart|tanstackStart|createStartHandler|StartClient|StartServer/gi) || []).length
    if (/^(app|vite|vinxi|tanstack)\.config\.(tsx?|jsx?|mjs|cjs)$/i.test(path.basename(file))) {
      const configSignals = (text.match(/@tanstack\/react-start|@tanstack\/start|TanStackStart|tanstackStart|createStartHandler|StartClient|StartServer|vinxi/gi) || []).length
      tanstackStartConfigSignals += configSignals
      tanstackStartConfigFileSignals += configSignals
    }
    rawHtmlSignals += (text.match(/dangerouslySetInnerHTML|<iframe|document\.write/g) || []).length
    cssSignals += (text.match(/className=|\.module\.css|import\s+["'][^"']+\.css|style\s*=/g) || []).length
    responsiveSignals += (text.match(/@media|clamp\(|minmax\(|grid-template|flex-wrap|container-type/g) || []).length
    accessibilitySignals += (text.match(/aria-|role=|alt=|label|htmlFor|sr-only/g) || []).length
    assetSignals += (text.match(/<img|picture|source|background-image|url\(/g) || []).length
    cssTokenSignals += (text.match(/--[a-z0-9-]+\s*:|box-shadow|border-radius|backdrop-filter|linear-gradient|radial-gradient|clamp\(|aspect-ratio|object-fit/gi) || []).length
    backendSignals += (text.match(/createServerFn|serverOnly|api\/|loader\s*:|beforeLoad|action\s*:|useServer|Request\(|Response\.json|json\(/g) || []).length
    databaseSignals += (text.match(/\bsqlite\b|better-sqlite3|libsql|drizzle|kysely|prisma|CREATE TABLE|INSERT INTO|SELECT\b|db\.(prepare|query|execute|select|insert|update)/gi) || []).length
    databaseCalculationSignals += (text.match(/SUM\s*\(|COUNT\s*\(|AVG\s*\(|GROUP BY|ORDER BY|CASE WHEN|conversion|revenue|subtotal|fee|total|ranking|ranked|trend|analytics/gi) || []).length
    routeSignals += (text.match(/createFileRoute|\/cart|\/checkout|\/admin|\/analytics|\/prompts\/\$|promptId|creator|storefront/gi) || []).length
    testSignals += (text.match(/\bdescribe\s*\(|\bit\s*\(|\btest\s*\(|expect\s*\(|playwright|vitest|node:test|supertest|fetch\(/gi) || []).length
    seedDataSignals += (text.match(/seed|fixture|prompts\s*[:=]|creators\s*[:=]|orders\s*[:=]|sales\s*[:=]|categories\s*[:=]/gi) || []).length
    for (const match of text.matchAll(/className=["'`]([^"'`]+)["'`]/g)) {
      for (const name of match[1].split(/\s+/).filter(Boolean)) classNames.set(name, (classNames.get(name) || 0) + 1)
    }
  }
  for (const count of classNames.values()) {
    if (count >= 4) duplicateClassSignals += 1
  }
  return {
    source_files: files.length,
    source_lines: lines,
    source_bytes: bytes,
    longest_file_lines: longestFileLines,
    component_signals: componentSignals,
    tanstack_signals: tanstackSignals,
    raw_html_shortcuts: rawHtmlSignals,
    css_signals: cssSignals,
    responsive_signals: responsiveSignals,
    accessibility_signals: accessibilitySignals,
    asset_signals: assetSignals,
    repeated_class_signals: duplicateClassSignals,
    backend_signals: backendSignals,
    database_signals: databaseSignals,
    database_calculation_signals: databaseCalculationSignals,
    route_signals: routeSignals,
    route_file_signals: routeFileSignals,
    router_provider_signals: routerProviderSignals,
    tanstack_start_config_signals: tanstackStartConfigSignals,
    tanstack_start_config_file_signals: tanstackStartConfigFileSignals,
    css_token_signals: cssTokenSignals,
    test_signals: testSignals,
    seed_data_signals: seedDataSignals,
  }
}

function testProfile(workspace, pkg) {
  const scripts = pkg?.scripts || {}
  const scriptEntries = Object.entries(scripts)
  const testScripts = scriptEntries.filter(([name, value]) => /test|spec|check|verify|e2e|api|db|unit/i.test(`${name} ${value}`))
  const files = listProjectFiles(workspace)
    .filter((file) => !file.includes(`${path.sep}node_modules${path.sep}`))
    .filter((file) => /\.(test|spec)\.(tsx?|jsx?)$|tests?[\\/].*\.(tsx?|jsx?)$|tools[\\/].*(test|verify|e2e|smoke|api|db).*\.mjs$/i.test(file))
  const text = files.map((file) => {
    try {
      return fs.readFileSync(file, "utf8")
    } catch {
      return ""
    }
  }).join("\n")
  return {
    scripts: Object.fromEntries(testScripts),
    script_count: testScripts.length,
    file_count: files.length,
    db_test_mentions: (text.match(/\b(sqlite|database|db|query|revenue|subtotal|conversion|analytics|GROUP BY|SUM\()/gi) || []).length,
    api_test_mentions: (text.match(/\b(fetch|Request|Response|api|server function|serverFn|loader|action|checkout|cart)\b/gi) || []).length,
    browser_test_mentions: (text.match(/\b(playwright|chromium|page\.|locator|goto|click|expect)\b/gi) || []).length,
  }
}

function runAdditionalTests(workspace, pkg) {
  const scripts = pkg?.scripts || {}
  const names = Object.keys(scripts)
  const prioritized = [
    ...names.filter((name) => /db|database|query|unit/i.test(name)),
    ...names.filter((name) => /api|server|route/i.test(name)),
    ...names.filter((name) => /e2e|browser|playwright|smoke/i.test(name)),
    ...names.filter((name) => /^test($|[:_-])|^verify($|[:_-])|^check($|[:_-])/i.test(name)),
  ]
  const candidates = [...new Set(prioritized)]
    .filter((name) => name !== "build")
    .slice(0, 4)
  return candidates.map((name) => ({
    script: name,
    ...summarizeRun(run(npmCmd, ["run", name], { cwd: workspace, timeoutMs: 180_000, shell: process.platform === "win32" })),
  }))
}

function sourceHtmlProfile() {
  const html = fs.readFileSync(sourceHtml, "utf8")
  const text = html
    .replace(/<script\b[\s\S]*?<\/script>/gi, " ")
    .replace(/<style\b[\s\S]*?<\/style>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim()
  const colors = new Set((html.match(/#[0-9a-f]{3,8}\b|rgba?\([^)]+\)|hsla?\([^)]+\)/gi) || []).map((item) => item.toLowerCase()))
  const identityTerms = ["POWERPROMPT", "GPT-4o", "Claude", "Midjourney", "Flux", "Featured", "Newest", "Popular", "Favorites", "Cart"]
  const identityTermCount = identityTerms.filter((term) => html.toLowerCase().includes(term.toLowerCase())).length
  return {
    bytes: Buffer.byteLength(html),
    text_chars: text.length,
    heading_count: (html.match(/<h[1-3]\b/gi) || []).length,
    interactive_count: (html.match(/<(button|a|input|select|textarea)\b/gi) || []).length,
    button_count: (html.match(/<button\b/gi) || []).length,
    link_count: (html.match(/<a\b/gi) || []).length,
    form_field_count: (html.match(/<(input|select|textarea)\b/gi) || []).length,
    article_count: (html.match(/<article\b/gi) || []).length,
    nav_count: (html.match(/<(nav|aside|header)\b/gi) || []).length,
    image_tag_count: (html.match(/<img\b/gi) || []).length,
    image_like_count: (html.match(/<(img|picture|source|video|canvas|svg)\b|background-image|url\(/gi) || []).length,
    masonry_signal: /masonry|tile|aspect-ratio|data-model|data-cat/i.test(html),
    identity_term_count: identityTermCount,
    color_count: colors.size,
    gradient_count: (html.match(/gradient\(/gi) || []).length,
    media_query_count: (html.match(/@media/gi) || []).length,
    shadow_count: (html.match(/box-shadow/gi) || []).length,
    radius_count: (html.match(/border-radius/gi) || []).length,
    css_token_count: (html.match(/--[a-z0-9-]+\s*:|box-shadow|border-radius|backdrop-filter|linear-gradient|radial-gradient|clamp\(|aspect-ratio|object-fit/gi) || []).length,
  }
}

function pickServeScript(pkg) {
  return pickServeScripts(pkg)[0] || null
}

function pickServeScripts(pkg, workspace = null) {
  const scripts = pkg?.scripts || {}
  const names = ["start", "preview", "dev"].filter((name) => scripts[name])
  if (!workspace) return names
  const hasOutputServer = fs.existsSync(path.join(workspace, ".output", "server", "index.mjs"))
  const hasDist = fs.existsSync(path.join(workspace, "dist"))
  return names.sort((a, b) => serveScriptRank(a, scripts[a], { hasOutputServer, hasDist }) - serveScriptRank(b, scripts[b], { hasOutputServer, hasDist }))
}

function serveScriptRank(name, command, { hasOutputServer, hasDist }) {
  const text = String(command || "")
  if (name === "start" && /\.output[\\/]server[\\/]index\.mjs|\.output\/server\/index\.mjs/.test(text) && !hasOutputServer) return 30
  if (name === "preview" && hasDist) return 0
  if (name === "start" && hasOutputServer) return 1
  if (name === "preview") return 2
  if (name === "dev") return 3
  return 10
}

function tailFile(filePath, maxChars = 4000) {
  try {
    const text = fs.readFileSync(filePath, "utf8")
    return text.length > maxChars ? text.slice(-maxChars) : text
  } catch {
    return ""
  }
}

function startServer(workspace, script, port, agentDir) {
  const safeScript = script.replace(/[^a-z0-9_-]/gi, "-")
  const stdoutPath = path.join(agentDir, `runtime-server-${safeScript}.stdout.log`)
  const stderrPath = path.join(agentDir, `runtime-server-${safeScript}.stderr.log`)
  mkdirp(agentDir)
  const server = spawn(npmCmd, ["run", script, "--", "--port", String(port), "--host", "127.0.0.1"], {
    cwd: workspace,
    stdio: ["ignore", "pipe", "pipe"],
    env: {
      ...process.env,
      HOST: "127.0.0.1",
      HOSTNAME: "127.0.0.1",
      PORT: String(port),
      NITRO_HOST: "127.0.0.1",
      NITRO_PORT: String(port),
      SERVER_HOST: "127.0.0.1",
      SERVER_PORT: String(port),
    },
    shell: process.platform === "win32",
    windowsHide: true,
  })
  const stdoutStream = fs.createWriteStream(stdoutPath)
  const stderrStream = fs.createWriteStream(stderrPath)
  server.stdout?.pipe(stdoutStream)
  server.stderr?.pipe(stderrStream)
  server.on("close", () => {
    stdoutStream.end()
    stderrStream.end()
  })
  server.stdoutPath = stdoutPath
  server.stderrPath = stderrPath
  server.exitStatus = null
  server.exitSignal = null
  server.exitPromise = new Promise((resolve) => {
    server.on("close", (status, signal) => {
      server.exitStatus = status
      server.exitSignal = signal
      resolve({ status, signal })
    })
  })
  return server
}

async function waitForServer(port, server = null) {
  const deadline = Date.now() + 45_000
  let exited = null
  server?.exitPromise?.then((result) => {
    exited = result
  })
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}`)
      if (response.status > 0 && response.status < 600) return { ready: true, status: response.status }
    } catch {}
    if (exited) return { ready: false, status: null, exited: true, exit_status: exited.status, exit_signal: exited.signal }
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
  return {
    ready: false,
    status: null,
    exited: Boolean(exited),
    exit_status: exited?.status ?? server?.exitStatus ?? null,
    exit_signal: exited?.signal ?? server?.exitSignal ?? null,
  }
}

async function evaluateRuntime(workspace, port, agentDir) {
  const evalScript = path.join(agentDir, "runtime-evaluate.mjs")
  const screenshotsDir = path.join(agentDir, "screenshots")
  mkdirp(screenshotsDir)
  writeFile(evalScript, runtimeEvaluator())
  const result = run("node", [evalScript, String(port), path.join(screenshotsDir, "desktop.png")], {
    cwd: workspace,
    timeoutMs: 120_000,
  })
  try {
    return JSON.parse(result.stdout)
  } catch {
    return { pass: false, status: result.status, stdout: result.stdout, stderr: result.stderr }
  }
}

function runtimeEvaluator() {
  return `import { chromium } from "playwright";
const port = Number(process.argv[2]);
const screenshot = process.argv[3];
const browser = await chromium.launch({ headless: true });
async function scan(name, viewport, screenshotPath) {
  const page = await browser.newPage({ viewport });
  const started = performance.now();
  await page.goto("http://127.0.0.1:" + port, { waitUntil: "networkidle", timeout: 60_000 });
  const loadMs = Math.round(performance.now() - started);
  await page.screenshot({ path: screenshotPath, fullPage: true });
  const data = await page.evaluate(() => {
    const bodyText = document.body.innerText || "";
    const overflow = document.documentElement.scrollWidth > window.innerWidth + 2;
    const controls = [...document.querySelectorAll("button,a,input,select,textarea")];
    const images = [...document.images].map((img) => ({ src: img.currentSrc || img.src, complete: img.complete, width: img.naturalWidth, height: img.naturalHeight }));
    const perf = performance.getEntriesByType("navigation")[0];
    const styles = [...document.querySelectorAll("*")].slice(0, 400).map((el) => {
      const cs = getComputedStyle(el);
      const rect = el.getBoundingClientRect();
      return {
        bg: cs.backgroundColor,
        bgImage: cs.backgroundImage,
        color: cs.color,
        font: cs.fontFamily,
        radius: cs.borderRadius,
        shadow: cs.boxShadow,
        border: cs.borderColor,
        display: cs.display,
        position: cs.position,
        opacity: cs.opacity,
        transform: cs.transform,
        width: Math.round(rect.width),
        height: Math.round(rect.height),
        top: Math.round(rect.top),
      };
    });
    const colorSet = new Set(styles.flatMap((item) => [item.bg, item.color]).filter((value) => value && value !== "rgba(0, 0, 0, 0)"));
    const fontSet = new Set(styles.map((item) => item.font).filter(Boolean));
    const h1 = document.querySelector("h1");
    const hero = h1?.closest("section,main,header,div");
    const heroRect = hero?.getBoundingClientRect();
    const articles = [...document.querySelectorAll("article")].filter((el) => {
      const rect = el.getBoundingClientRect();
      return rect.width > 40 && rect.height > 40;
    });
    const cards = articles.map((el) => {
      const rect = el.getBoundingClientRect();
      const cs = getComputedStyle(el);
      return { width: Math.round(rect.width), height: Math.round(rect.height), radius: cs.borderRadius, shadow: cs.boxShadow };
    });
    const firstViewportElements = styles.filter((item) => item.top < window.innerHeight && item.width > 8 && item.height > 8);
    const richBackgrounds = styles.filter((item) => item.bgImage && item.bgImage !== "none").length;
    const decorativeSurfaces = styles.filter((item) =>
      (item.bg && item.bg !== "rgba(0, 0, 0, 0)") ||
      (item.bgImage && item.bgImage !== "none") ||
      (item.shadow && item.shadow !== "none") ||
      (item.radius && item.radius !== "0px")
    ).length;
    const h1Style = h1 ? getComputedStyle(h1) : null;
    const identityTerms = ["POWERPROMPT", "GPT-4o", "Claude", "Midjourney", "Flux", "Featured", "Newest", "Popular", "Favorites", "Cart"];
    const lowerText = bodyText.toLowerCase();
    const unlabeledControls = controls.filter((el) => {
      const text = (el.innerText || el.value || el.getAttribute("aria-label") || el.getAttribute("title") || "").trim();
      return !text && !el.closest("label");
    }).length;
    return {
      title: document.title,
      body_chars: bodyText.length,
      heading_count: document.querySelectorAll("h1,h2,h3").length,
      h1_text_chars: (h1?.innerText || "").trim().length,
      interactive_count: controls.length,
      unlabeled_controls: unlabeledControls,
      image_count: images.length,
      broken_images: images.filter((img) => !img.complete || img.width === 0).length,
      horizontal_overflow: overflow,
      dom_nodes: document.querySelectorAll("*").length,
      section_count: document.querySelectorAll("section,article,aside,header,footer,main").length,
      article_count: articles.length,
      nav_count: document.querySelectorAll("nav,aside,header").length,
      button_count: document.querySelectorAll("button").length,
      link_count: document.querySelectorAll("a[href]").length,
      form_field_count: document.querySelectorAll("input,select,textarea").length,
      visible_svg_count: [...document.querySelectorAll("svg")].filter((el) => el.getBoundingClientRect().width > 4).length,
      color_count: colorSet.size,
      font_count: fontSet.size,
      shadow_count: styles.filter((item) => item.shadow && item.shadow !== "none").length,
      radius_count: styles.filter((item) => item.radius && item.radius !== "0px").length,
      layout_count: styles.filter((item) => item.display === "grid" || item.display === "flex").length,
      rich_background_count: richBackgrounds,
      decorative_surface_count: decorativeSurfaces,
      first_viewport_surface_count: firstViewportElements.filter((item) =>
        (item.bg && item.bg !== "rgba(0, 0, 0, 0)") ||
        (item.bgImage && item.bgImage !== "none") ||
        (item.shadow && item.shadow !== "none")
      ).length,
      card_count: cards.length,
      card_min_height: cards.length ? Math.min(...cards.map((item) => item.height)) : 0,
      card_max_height: cards.length ? Math.max(...cards.map((item) => item.height)) : 0,
      card_height_delta: cards.length ? Math.max(...cards.map((item) => item.height)) - Math.min(...cards.map((item) => item.height)) : 0,
      card_min_width: cards.length ? Math.min(...cards.map((item) => item.width)) : 0,
      h1_font_px: h1Style ? Number.parseFloat(h1Style.fontSize) : 0,
      identity_term_count: identityTerms.filter((term) => lowerText.includes(term.toLowerCase())).length,
      hero_height: heroRect ? Math.round(heroRect.height) : 0,
      hero_top: heroRect ? Math.round(heroRect.top) : null,
      viewport_width: window.innerWidth,
      scroll_height: document.documentElement.scrollHeight,
      transfer_size: Math.round((performance.getEntriesByType("resource") || []).reduce((sum, item) => sum + (item.transferSize || 0), 0)),
      resource_count: performance.getEntriesByType("resource").length,
      dom_content_loaded_ms: perf ? Math.round(perf.domContentLoadedEventEnd) : null,
    };
  });
  await page.close();
  return { name, load_ms: loadMs, screenshot: screenshotPath, ...data };
}
const mobileScreenshot = screenshot.replace(/[^/\\\\]+$/, "mobile.png");
const desktop = await scan("desktop", { width: 1440, height: 980 }, screenshot);
const mobile = await scan("mobile", { width: 390, height: 844 }, mobileScreenshot);
await browser.close();
console.log(JSON.stringify({
  pass: desktop.body_chars > 500 && mobile.body_chars > 500 && !desktop.horizontal_overflow && !mobile.horizontal_overflow,
  load_ms: desktop.load_ms,
  screenshot,
  screenshots_dir: screenshot.replace(/[/\\\\][^/\\\\]+$/, ""),
  desktop,
  mobile,
  body_chars: desktop.body_chars,
  heading_count: desktop.heading_count,
  interactive_count: desktop.interactive_count,
  image_count: desktop.image_count,
  broken_images: desktop.broken_images + mobile.broken_images,
  horizontal_overflow: desktop.horizontal_overflow || mobile.horizontal_overflow,
  dom_nodes: desktop.dom_nodes,
  transfer_size: desktop.transfer_size,
  dom_content_loaded_ms: desktop.dom_content_loaded_ms,
}, null, 2));
`
}

function dependencyCount(pkg) {
  return Object.keys({ ...(pkg?.dependencies || {}), ...(pkg?.devDependencies || {}) }).length
}

function standard(id, category, pass, detail) {
  return { id, category, pass: Boolean(pass), detail }
}

function buildStandards({ pkg, install, build, tests, metrics, runtime, distBytes, source }) {
  const scripts = pkg?.scripts || {}
  const deps = dependencyCount(pkg)
  const desktop = runtime.desktop || runtime
  const mobile = runtime.mobile || runtime
  const sourceTextFloor = Math.max(500, Math.round(source.text_chars * 0.45))
  const sourceHeadingFloor = Math.max(1, Math.min(source.heading_count, 4))
  const sourceInteractiveFloor = Math.min(Math.max(2, Math.floor(source.interactive_count * 0.5)), 12)
  const sourceButtonFloor = Math.min(Math.max(2, Math.floor((source.button_count || source.interactive_count) * 0.45)), 18)
  const sourceFormFieldFloor = Math.min(Math.max(1, Math.floor((source.form_field_count || 1) * 0.5)), 6)
  const sourceCardFloor = Math.min(Math.max(4, Math.floor((source.article_count || 8) * 0.65)), 12)
  const sourceMediaFloor = source.image_like_count > 0 ? 1 : 0
  const sourceImageFloor = source.image_like_count >= 8 || source.image_tag_count >= 1 ? Math.min(8, Math.max(2, Math.floor(source.image_like_count * 0.2))) : 0
  const sourceIdentityFloor = Math.min(Math.max(4, source.identity_term_count || 0), 8)
  const sourceStyleFloor = Math.min(Math.max(8, Math.floor((source.css_token_count || 16) * 0.35)), 40)
  const sourceShadowFloor = Math.min(Math.max(2, Math.floor((source.shadow_count || 4) * 0.35)), 16)
  const sourceRadiusFloor = Math.min(Math.max(6, Math.floor((source.radius_count || 12) * 0.35)), 36)
  const baseStandards = [
    standard("pkg-present", "architecture", Boolean(pkg), "project declares package metadata"),
    standard("tanstack-dependency", "architecture", /@tanstack\/react-start|@tanstack\/start/.test(JSON.stringify(pkg || {})), "TanStack Start dependency is present"),
    standard("route-source", "architecture", metrics.tanstack_signals > 0, "TanStack route source exists"),
    standard("route-files", "architecture", metrics.route_file_signals >= 2, "file-based routes live under src/routes"),
    standard("router-wiring", "architecture", metrics.router_provider_signals >= 3, "TanStack router is wired through route definitions and provider"),
    standard("tanstack-start-wiring", "architecture", metrics.tanstack_start_config_signals >= 2, "TanStack Start is used beyond a decorative dependency"),
    standard("tanstack-start-config-file", "architecture", metrics.tanstack_start_config_file_signals >= 1, "TanStack Start is wired through an app/vite/vinxi config file"),
    standard("dev-script", "architecture", Boolean(scripts.dev), "development script exists"),
    standard("build-script", "architecture", Boolean(scripts.build), "build script exists"),
    standard("serve-script", "architecture", Boolean(scripts.start || scripts.preview), "built app can be served"),
    standard("playwright-dependency", "tests", /@playwright\/test|playwright/i.test(JSON.stringify(pkg || {})), "Playwright is installed by default in package.json"),
    standard("playwright-script", "tests", Object.entries(scripts).some(([name, value]) => /playwright|e2e|browser|smoke/i.test(`${name} ${value}`)), "Playwright browser smoke/e2e script exists"),
  ]
  const fullstackStandards = isFullStackVersion ? [
    standard("backend-boundary", "fullstack", metrics.backend_signals >= 2, "server/API/loader/action backend boundary exists"),
    standard("multi-route-product", "fullstack", metrics.route_signals >= 5, "storefront, detail, cart/checkout, and analytics route signals exist"),
    standard("database-dependency", "database", /sqlite|better-sqlite3|libsql|drizzle|kysely|prisma/i.test(JSON.stringify(pkg || {})) || metrics.database_signals >= 6, "local database dependency or SQL layer exists"),
    standard("database-schema", "database", metrics.database_signals >= 8, "database schema/query source is present"),
    standard("database-calculations", "database", metrics.database_calculation_signals >= 8, "business calculations are implemented in query/database code"),
    standard("seed-data", "database", metrics.seed_data_signals >= 6, "seed data for prompts, creators, orders, categories, and cart/favorites exists"),
    standard("test-scripts", "tests", tests.script_count >= 3, "multiple test scripts are declared"),
    standard("test-files", "tests", tests.file_count >= 3, "multiple test/spec/evaluator files are present"),
    standard("db-tests", "tests", tests.db_test_mentions >= 4, "database calculation tests are present"),
    standard("api-tests", "tests", tests.api_test_mentions >= 4, "backend/API/server-function tests are present"),
    standard("browser-tests", "tests", tests.browser_test_mentions >= 4, "browser/component/e2e smoke tests are present"),
  ] : [
    standard("browser-tests", "tests", tests.browser_test_mentions >= 4, "Playwright browser smoke/e2e test is present"),
  ]
  const standards = [
    ...baseStandards,
    ...fullstackStandards,
    standard("components", "maintainability", metrics.component_signals >= 4, "UI is decomposed into components"),
    standard("no-raw-html", "maintainability", metrics.raw_html_shortcuts === 0, "no iframe/raw HTML shortcut"),
    standard("style-system", "maintainability", metrics.css_signals >= 4, "styling is represented in app source"),
    standard("style-token-depth", "maintainability", metrics.css_token_signals >= sourceStyleFloor, "source contains enough real visual styling primitives"),
    standard("responsive-source", "maintainability", metrics.responsive_signals >= 2, "responsive layout techniques are present"),
    standard("a11y-source", "accessibility", metrics.accessibility_signals >= 2, "accessibility-oriented source signals exist"),
    standard("dependency-budget", "performance", deps > 0 && deps <= 18, "dependency count stays restrained"),
    standard("source-budget", "performance", metrics.source_bytes > 0 && metrics.source_bytes < 300_000, "source size is reasonable"),
    standard("install", "build", install?.status === 0, "npm install succeeds"),
    standard("build", "build", build?.status === 0, "npm run build succeeds"),
    standard("build-time", "performance", build?.duration_ms > 0 && build.duration_ms < 90_000, "build completes in a practical time"),
    standard("artifact-budget", "performance", distBytes === 0 || distBytes < 6_000_000, "build output is not excessive"),
    standard("runtime-ready", "runtime", runtime.pass, "served app renders in browser"),
    standard("desktop-load", "performance", desktop.load_ms > 0 && desktop.load_ms < 5_500, "desktop load is responsive"),
    standard("mobile-load", "performance", mobile.load_ms > 0 && mobile.load_ms < 5_500, "mobile load is responsive"),
    standard("dom-budget", "performance", desktop.dom_nodes > 20 && desktop.dom_nodes < 1_500, "DOM size is practical"),
    standard("desktop-content-depth", "effect", desktop.body_chars >= sourceTextFloor, "desktop preserves substantial source content"),
    standard("mobile-content-depth", "effect", mobile.body_chars >= sourceTextFloor, "mobile preserves substantial source content"),
    standard("heading-depth", "effect", desktop.heading_count >= sourceHeadingFloor, "heading hierarchy is preserved"),
    standard("interactive-depth", "effect", desktop.interactive_count >= sourceInteractiveFloor, "interactive affordances are preserved"),
    standard("button-depth", "appearance", desktop.button_count >= sourceButtonFloor, "primary action/control density is preserved"),
    standard("form-control-depth", "appearance", desktop.form_field_count >= sourceFormFieldFloor, "search/filter form controls are preserved"),
    standard("card-grid-depth", "appearance", desktop.card_count >= sourceCardFloor && desktop.card_min_height >= 120 && desktop.card_min_width >= 120, "gallery/card grid has visible product-card depth"),
    standard("navigation-chrome", "appearance", desktop.nav_count >= Math.min(Math.max(1, source.nav_count || 2), 4), "navigation/header/sidebar chrome is represented"),
    standard("media-depth", "effect", desktop.image_count + desktop.visible_svg_count >= sourceMediaFloor, "media/iconography is preserved when relevant"),
    standard("image-media-depth", "appearance", sourceImageFloor === 0 || desktop.image_count >= sourceImageFloor, "image-based gallery media is preserved when the source uses it"),
    standard("masonry-card-variety", "appearance", !source.masonry_signal || desktop.card_height_delta >= 80, "masonry-style card height variety is preserved"),
    standard("source-identity-terms", "appearance", desktop.identity_term_count >= sourceIdentityFloor, "source model/filter/sort identity terms are preserved"),
    standard("color-depth", "effect", desktop.color_count >= Math.min(8, Math.max(4, Math.floor(source.color_count * 0.25))), "visual color/material depth exists"),
    standard("layout-depth", "effect", desktop.layout_count >= 6, "layout uses structured flex/grid composition"),
    standard("surface-richness", "appearance", desktop.decorative_surface_count >= 18 && desktop.first_viewport_surface_count >= 8, "first screen has layered visual surfaces instead of plain text"),
    standard("shadow-depth", "appearance", desktop.shadow_count >= sourceShadowFloor, "elevation/shadow styling is materially represented"),
    standard("radius-depth", "appearance", desktop.radius_count >= sourceRadiusFloor, "rounded/card styling depth is materially represented"),
    standard("background-depth", "appearance", desktop.rich_background_count >= Math.min(2, Math.max(1, source.gradient_count)), "background gradients or image layers are represented"),
    standard("mobile-gallery-density", "appearance", mobile.card_count >= Math.min(sourceCardFloor, desktop.card_count), "mobile keeps readable gallery density"),
    standard("desktop-overflow", "responsive", !desktop.horizontal_overflow, "desktop has no horizontal overflow"),
    standard("mobile-overflow", "responsive", !mobile.horizontal_overflow, "mobile has no horizontal overflow"),
    standard("mobile-content", "responsive", mobile.scroll_height > 700 && mobile.body_chars >= Math.round(desktop.body_chars * 0.65), "mobile retains the page experience"),
    standard("broken-images", "runtime", runtime.broken_images === 0, "no broken image elements"),
    standard("control-labels", "accessibility", (desktop.unlabeled_controls || 0) <= 1, "controls are mostly labeled"),
  ]
  return standards
}

async function evaluateProject(agent, workspace, agentDir, index) {
  const pkg = packageJson(workspace)
  const install = pkg ? run(npmCmd, ["install"], { cwd: workspace, timeoutMs: 240_000, shell: process.platform === "win32" }) : null
  const build = pkg ? run(npmCmd, ["run", "build"], { cwd: workspace, timeoutMs: 240_000, shell: process.platform === "win32" }) : null
  const metrics = codeMetrics(workspace)
  const tests = testProfile(workspace, pkg)
  tests.runs = []
  const source = sourceHtmlProfile()
  const distBytes = byteSize(path.join(workspace, "dist")) || byteSize(path.join(workspace, ".output")) || byteSize(path.join(workspace, "build"))
  let runtime = { pass: false, error: "no package.json or serve script" }
  const serveScripts = pickServeScripts(pkg, workspace)
  if (pkg && serveScripts.length > 0 && install?.status === 0) {
    const port = 45200 + index
    const attempts = []
    for (const serveScript of serveScripts) {
      const server = startServer(workspace, serveScript, port, agentDir)
      try {
        const ready = await waitForServer(port, server)
        attempts.push({
          script: serveScript,
          ready: ready.ready,
          status: ready.status,
          exited: Boolean(ready.exited),
          exit_status: ready.exit_status ?? null,
          exit_signal: ready.exit_signal ?? null,
          stdout_path: server.stdoutPath,
          stderr_path: server.stderrPath,
          stdout_tail: tailFile(server.stdoutPath),
          stderr_tail: tailFile(server.stderrPath),
        })
        if (ready.ready) {
          runtime = await evaluateRuntime(workspace, port, agentDir)
          runtime.serve_script = serveScript
          runtime.server_status = ready.status
          runtime.server_attempts = attempts
          break
        }
        runtime = {
          pass: false,
          error: `${serveScript} server did not become ready`,
          server_stdout_path: server.stdoutPath,
          server_stderr_path: server.stderrPath,
          server_exit_status: ready.exit_status ?? null,
          server_exit_signal: ready.exit_signal ?? null,
          server_stdout_tail: tailFile(server.stdoutPath),
          server_stderr_tail: tailFile(server.stderrPath),
          server_attempts: attempts,
        }
      } finally {
        killProcessTree(server)
      }
    }
  }
  const standards = buildStandards({ pkg, install, build, tests, metrics, runtime, distBytes, source })
  const passedStandards = standards.filter((item) => item.pass).length
  const standardsByCategory = {}
  for (const item of standards) {
    standardsByCategory[item.category] ||= { passed: 0, total: 0 }
    standardsByCategory[item.category].total += 1
    if (item.pass) standardsByCategory[item.category].passed += 1
  }
  const qualityScore = standards.filter((item) => item.pass && !["performance"].includes(item.category)).length
  const performanceScore = standards.filter((item) => item.pass && item.category === "performance").length
  return {
    agent,
    workspace,
    package_name: pkg?.name || null,
    scripts: pkg?.scripts || null,
    install: summarizeRun(install),
    build: summarizeRun(build),
    metrics,
    tests,
    source_profile: source,
    dist_bytes: distBytes,
    runtime,
    standards,
    standards_by_category: standardsByCategory,
    standards_passed: passedStandards,
    standards_total: standards.length,
    quality_score: qualityScore,
    performance_score: performanceScore,
    total_score: passedStandards,
  }
}

function summarizeRun(result) {
  if (!result) return null
  return {
    status: result.status,
    signal: result.signal,
    duration_ms: result.duration_ms,
    error: result.error,
    stdout_path: result.stdout_path || null,
    stderr_path: result.stderr_path || null,
    status_path: result.status_path || null,
    stderr_tail: String(result.stderr || "").split(/\r?\n/).filter(Boolean).slice(-20).join("\n"),
  }
}

function statsFromLiveResult(agent, workspace, agentDir, started, result, validation = null, contextArchive = null) {
  const stdout = result?.stdout || ""
  const events = parseJsonl(stdout)
  const isExternal = agent === "claude-code" || agent === "pi-agent"
  const usageInfo = isExternal
    ? { usage: agentUsageFromJsonl(stdout), usage_source: `${agent}-jsonl`, provider_calls: [] }
    : usageForAgent(agentDir, stdout)
  return {
    id: agent,
    agent,
    task: `prompt-gallery-tanstack-${taskVersion}-rebuild`,
    workspace,
    in_progress: result?.status === null && !result?.error,
    elapsed_ms: Math.round(performance.now() - started),
    exit_code: result?.status ?? null,
    error: result?.error || null,
    stdout_path: result?.stdout_path || null,
    stderr_path: result?.stderr_path || null,
    status_path: result?.status_path || null,
    run: summarizeRun(result),
    usage: usageInfo.usage,
    usage_source: usageInfo.usage_source,
    provider_calls: usageInfo.provider_calls,
    provider_calls_path: contextArchive?.provider_calls_full_path || null,
    context_archive: contextArchive,
    events: isExternal ? agentEventStats(stdout) : countEvents(events),
    validation,
  }
}

function aggregateUsage(results) {
  const usage = { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 }
  for (const result of results) {
    const u = result?.usage || {}
    usage.input += Number(u.input || u.input_tokens || u.inputTokens || 0)
    usage.cached += Number(u.cached || u.cached_input_tokens || u.cacheInputTokens || 0)
    usage.output += Number(u.output || u.output_tokens || u.outputTokens || 0)
    usage.reasoning += Number(u.reasoning || u.reasoning_tokens || u.reasoningTokens || 0)
    usage.total += Number(u.total || u.total_tokens || u.totalTokens || 0)
  }
  return {
    ...usage,
    inputTokens: usage.input,
    cacheInputTokens: usage.cached,
    outputTokens: usage.output,
    reasoningTokens: usage.reasoning,
    totalTokens: usage.total,
  }
}

function buildRunSummary(results, extra = {}) {
  const finalResults = !extra.in_progress && results.length > 0
  const validations = results.map((result) => result.validation).filter(Boolean)
  const comparison = validations.length > 0 ? compareResults(validations) : null
  return normalizeBusinessSummary({
    ok: finalResults && results.every((result) =>
      result.run?.status === 0 &&
      (skipEval || (result.validation && validationAccepted(result.validation))),
    ),
    source_html: sourceHtml,
    task_version: taskVersion,
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    desktop_dir: desktopDir,
    agents,
    aggregate_usage: aggregateUsage(results),
    comparison,
    results,
    skip_eval: skipEval,
    ...extra,
  }, runPaths)
}

function compareResults(results) {
  const sorted = [...results].sort((a, b) => {
    const scoreDelta = b.total_score - a.total_score
    if (scoreDelta) return scoreDelta
    return Number(a.runtime.load_ms || Number.MAX_SAFE_INTEGER) - Number(b.runtime.load_ms || Number.MAX_SAFE_INTEGER)
  })
  const [winner, runnerUp] = sorted
  return {
    winner: winner?.agent || null,
    reason: winner && runnerUp
      ? `${winner.agent} passed ${winner.standards_passed}/${winner.standards_total} standards vs ${runnerUp.standards_passed}/${runnerUp.standards_total}; runtime load ${winner.runtime.load_ms ?? "n/a"}ms vs ${runnerUp.runtime.load_ms ?? "n/a"}ms`
      : null,
    scores: Object.fromEntries(results.map((result) => [result.agent, {
      quality: result.quality_score,
      performance: result.performance_score,
      total: result.total_score,
      standards: `${result.standards_passed}/${result.standards_total}`,
      load_ms: result.runtime.load_ms ?? null,
      build_ms: result.build?.duration_ms ?? null,
      source_lines: result.metrics.source_lines,
      database_calculation_signals: result.metrics.database_calculation_signals,
      test_scripts: result.tests?.script_count ?? 0,
      passing_test_runs: result.tests?.runs?.filter((item) => item.status === 0).length ?? 0,
      dist_bytes: result.dist_bytes,
    }])),
  }
}

function validationAccepted(validation) {
  const category = (name) => validation.standards_by_category?.[name] || { passed: 0, total: 0 }
  const architecture = category("architecture")
  const appearance = category("appearance")
  const tests = category("tests")
  const totalOk = validation.standards_passed >= Math.ceil(validation.standards_total * 0.72)
  const architectureOk = architecture.total === 0 || architecture.passed === architecture.total
  const appearanceOk = appearance.total === 0 || appearance.passed >= Math.ceil(appearance.total * 0.8)
  const testsOk = tests.total === 0 || tests.passed >= Math.ceil(tests.total * 0.75)
  return totalOk && architectureOk && appearanceOk && testsOk
}

function ensureHarnessRuntime() {
  writeFile(path.join(runRoot, "package.json"), JSON.stringify({
    private: true,
    type: "module",
    dependencies: { playwright: "latest" },
  }, null, 2))
  runOk(npmCmd, ["install"], { cwd: runRoot, timeoutMs: 240_000, shell: process.platform === "win32" })
  runOk(npxCmd, ["playwright", "install", "chromium"], { cwd: runRoot, timeoutMs: 240_000, shell: process.platform === "win32" })
}

async function runAgent(agent, index, onAgentUpdate = null) {
  const agentDir = path.join(runRoot, agent)
  mkdirp(agentDir)
  const workspace = prepareProject(agent)
  const started = performance.now()
  let result
  let lastContextArchive = null
  let lastContextArchiveRefreshMs = 0
  const publishProgress = (liveResult) => {
    const now = performance.now()
    if (!lastContextArchive || liveResult.status !== null || now - lastContextArchiveRefreshMs > 10_000) {
      lastContextArchive = refreshContextAndCallArchive(agentDir, liveResult.stdout || "")
      lastContextArchiveRefreshMs = now
    }
    const stats = statsFromLiveResult(agent, workspace, agentDir, started, liveResult, null, lastContextArchive)
    writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
    onAgentUpdate?.(stats)
  }
  if (agent === "codex" || agent === "codex-main") result = await runCodex(agent, workspace, agentDir, publishProgress)
  else if (agent === "claude-code") result = await runClaudeCode(workspace, agentDir, publishProgress)
  else if (agent === "pi-agent") result = await runPiAgent(workspace, agentDir, publishProgress)
  else result = await runTura(agent, workspace, agentDir, publishProgress)
  const validation = skipEval ? null : await evaluateProject(agent, workspace, agentDir, index)
  lastContextArchive = refreshContextAndCallArchive(agentDir, result.stdout || "")
  const stats = statsFromLiveResult(agent, workspace, agentDir, started, result, validation, lastContextArchive)
  stats.in_progress = false
  writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
  onAgentUpdate?.(stats)
  return stats
}

async function main() {
  mkdirp(runRoot)
  assert(fs.existsSync(sourceHtml), `missing source HTML: ${sourceHtml}`)
  if (prepOnly) {
    const prepared = agents.map((agent) => ({ agent, workspace: prepareProject(agent) }))
    const summary = normalizeBusinessSummary({ ok: true, prep_only: true, task_version: taskVersion, source_html: sourceHtml, prepared }, runPaths)
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  if (evaluateOnly) {
    ensureHarnessRuntime()
    const results = []
    for (const [index, agent] of agents.entries()) {
      const workspace = existingProject(agent)
      assert(fs.existsSync(workspace), `missing existing project: ${workspace}`)
      const agentDir = path.join(runRoot, agent)
      mkdirp(agentDir)
      const summaryFile = path.join(agentDir, "agent-summary.json")
      const prior = fs.existsSync(summaryFile) ? JSON.parse(fs.readFileSync(summaryFile, "utf8")) : {}
      const validation = await evaluateProject(agent, workspace, agentDir, index)
      const stats = {
        ...prior,
        id: agent,
        agent,
        task: `prompt-gallery-tanstack-${taskVersion}-rebuild`,
        workspace,
        elapsed_ms: validation.install?.duration_ms + validation.build?.duration_ms,
        run: { ...(prior.run || {}), status: prior.run?.status || null, evaluate_only: true },
        usage: prior.usage || { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 },
        events: prior.events || { turns: 0, commands: 0, failures: 0 },
        validation,
      }
      writeFile(summaryFile, JSON.stringify(stats, null, 2))
      results.push(stats)
    }
    const validations = results.map((result) => result.validation)
    const comparison = compareResults(validations)
    const summary = normalizeBusinessSummary({
      ok: results.every(
        (result) =>
          validationAccepted(result.validation),
      ),
      evaluate_only: true,
      task_version: taskVersion,
      source_html: sourceHtml,
      desktop_dir: desktopDir,
      agents,
      aggregate_usage: aggregateUsage(results),
      comparison,
      results,
    }, runPaths)
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    if (!summary.ok && !allowFailure) process.exitCode = 1
    return
  }
  if (agents.includes("codex")) assert(fs.existsSync(codexExe), `missing codex exe ${codexExe}`)
  if (agents.includes("codex-main")) assert(fs.existsSync(codexMainExe), `missing codex-main exe ${codexMainExe}`)
  if (agents.some((agent) => agent.startsWith("tura-"))) {
    if (!skipTuraBuild || !fs.existsSync(turaExe)) {
      runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_exec"], { cwd: repoRoot, timeoutMs: 5 * 60_000 })
    }
    assert(fs.existsSync(turaExe), `missing tura exe ${turaExe}`)
  }
  ensureHarnessRuntime()
  const partialResults = new Map()
  let finalSummaryWritten = false
  const writeProgressSummary = () => {
    if (finalSummaryWritten) return
    const results = [...partialResults.values()].sort((a, b) => String(a.agent || a.id).localeCompare(String(b.agent || b.id)))
    const summary = buildRunSummary(results, { in_progress: true })
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
  }
  const results = await Promise.all(agents.map((agent, index) => {
    console.log(`[makeup-tanstack] running ${agent}`)
    return runAgent(agent, index, (stats) => {
      partialResults.set(agent, stats)
      writeProgressSummary()
    })
  }))
  finalSummaryWritten = true
  const summary = buildRunSummary(results, { in_progress: false })
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && !allowFailure) process.exitCode = 1
}

main().catch((error) => {
  const summary = normalizeBusinessSummary({
    ok: false,
    source_html: sourceHtml,
    error: String(error?.stack || error?.message || error),
  }, runPaths)
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.error(JSON.stringify(summary, null, 2))
  process.exitCode = 1
})
