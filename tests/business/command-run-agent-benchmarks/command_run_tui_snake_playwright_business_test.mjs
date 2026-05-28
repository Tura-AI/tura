#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `arcade-two-step-${Date.now()}`
const runRoot = path.join(repoRoot, "target", "command-run-arcade-two-step", runId)
const summaryPath = path.join(runRoot, "summary.json")
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 180_000)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm"
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx"
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "codex-gpt55,deepseek-coder,qwen-coder,google-flash-lite")

const defaultModels = {
  "codex-gpt55": process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "openai/gpt-5.5",
  "deepseek-coder": process.env.COMMAND_RUN_AGENT_DEEPSEEK_MODEL || "deepseek/deepseek-v4-pro",
  "qwen-coder": process.env.COMMAND_RUN_AGENT_QWEN_MODEL || "qwen/qwen3.6-max-preview",
  "google-flash-lite": process.env.COMMAND_RUN_AGENT_GOOGLE_MODEL || "google/gemini-3.1-flash-lite",
}

function parseAgents(value) {
  const alias = new Map([
    ["codex", "codex-gpt55"],
    ["codex-gpt55", "codex-gpt55"],
    ["openai", "codex-gpt55"],
    ["deepseek", "deepseek-coder"],
    ["deepseek-coder", "deepseek-coder"],
    ["qwen", "qwen-coder"],
    ["qwen-coder", "qwen-coder"],
    ["google", "google-flash-lite"],
    ["google-flash-lite", "google-flash-lite"],
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

function copyDir(src, dst) {
  mkdirp(dst)
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name)
    const to = path.join(dst, entry.name)
    if (entry.isDirectory()) copyDir(from, to)
    else fs.copyFileSync(from, to)
  }
}

function run(command, args, options = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
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

function runLive(command, args, options = {}) {
  const started = performance.now()
  mkdirp(path.dirname(options.stdoutPath))
  const stdoutStream = fs.createWriteStream(options.stdoutPath, { flags: "w" })
  const stderrStream = fs.createWriteStream(options.stderrPath, { flags: "w" })
  let stdout = ""
  let stderr = ""
  let settled = false
  return new Promise((resolve) => {
    function settle(status, signal, error) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      stdoutStream.end()
      stderrStream.end()
      resolve({
        command,
        args,
        status,
        signal,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        error,
      })
    }

    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["ignore", "pipe", "pipe"],
      shell: options.shell || false,
      windowsHide: true,
    })
    const timer = setTimeout(() => {
      try {
        if (process.platform === "win32" && child.pid) spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true })
        else child.kill("SIGKILL")
      } catch {}
      settle(1, null, `timed out after ${options.timeoutMs || timeoutMs}ms`)
    }, options.timeoutMs || timeoutMs)
    child.stdout?.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stdout += text
      stdoutStream.write(text)
      if (text.includes("\"type\":\"turn.completed\"")) settle(0, null, null)
    })
    child.stderr?.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stderr += text
      stderrStream.write(text)
    })
    child.on("error", (error) => settle(null, null, String(error.stack || error.message || error)))
    child.on("close", (status, signal) => settle(status, signal, null))
  })
}

function serviceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || ["default", "none", "off"].includes(tier)) return []
  return ["-c", `service_tier=${tier}`]
}

function createFixtureTemplate() {
  const template = path.join(runRoot, "template")
  mkdirp(template)
  writeFile(path.join(template, "package.json"), JSON.stringify({
    scripts: {
      start: "vite --host 127.0.0.1",
      "verify:arcade": "node tools/with_vite.mjs -- node tools/evaluate_arcade.mjs",
      "smoke:playwright": "node tools/with_vite.mjs -- node tools/smoke.mjs",
    },
    dependencies: {
      "@vitejs/plugin-react": "latest",
      vite: "latest",
      react: "latest",
      "react-dom": "latest",
      playwright: "latest",
    },
    devDependencies: {},
  }, null, 2))
  writeFile(path.join(template, "index.html"), `<div id="root"></div><script type="module" src="/src/App.jsx"></script>\n`)
  writeFile(path.join(template, "src", "App.jsx"), `import React from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

function App() {
  return (
    <main className="shell">
      <section className="panel">
        <p className="eyebrow">Arcade repair task</p>
        <h1>Snake prototype</h1>
        <p>This placeholder has no board, keyboard control, food, score, pause, or game-over state yet.</p>
        <button>Start</button>
      </section>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<App />);
`)
  writeFile(path.join(template, "src", "styles.css"), `:root { font-family: Inter, Segoe UI, Arial, sans-serif; color: #152238; background: #f5f7fb; }
* { box-sizing: border-box; }
body { margin: 0; }
.shell { min-height: 100vh; display: grid; place-items: center; padding: 24px; }
.panel { width: min(720px, 100%); background: white; border: 1px solid #d8e1ef; border-radius: 12px; padding: 28px; }
.eyebrow { text-transform: uppercase; letter-spacing: .12em; color: #4f6b91; }
h1 { margin: 0 0 12px; font-size: 48px; }
button { border: 0; border-radius: 8px; padding: 12px 18px; background: #2563eb; color: white; font-weight: 700; }
`)
  writeFile(path.join(template, "tools", "with_vite.mjs"), withViteScript())
  writeFile(path.join(template, "tools", "smoke.mjs"), smokeScript())
  writeFile(path.join(template, "tools", "evaluate_arcade.mjs"), evaluatorScript())
  return template
}

function withViteScript() {
  return `import { spawn } from "node:child_process";
import fs from "node:fs";
const separator = process.argv.indexOf("--");
const commandArgs = separator >= 0 ? process.argv.slice(separator + 1) : [];
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const port = Number(process.env.PORT || 4173);
fs.mkdirSync("artifacts", { recursive: true });
const server = spawn(npmCmd, ["run", "start", "--", "--port", String(port), "--strictPort"], {
  stdio: ["ignore", fs.openSync("artifacts/vite.stdout.log", "w"), fs.openSync("artifacts/vite.stderr.log", "w")],
  shell: process.platform === "win32",
  detached: process.platform !== "win32",
  windowsHide: true,
});
function kill() {
  if (process.platform === "win32" && server.pid) spawn("taskkill", ["/pid", String(server.pid), "/t", "/f"], { stdio: "ignore", windowsHide: true });
  else { try { process.kill(-server.pid, "SIGTERM"); } catch { try { server.kill("SIGTERM"); } catch {} } }
}
const deadline = Date.now() + 30000;
while (Date.now() < deadline) {
  try { if ((await fetch(\`http://127.0.0.1:\${port}\`)).ok) break; } catch {}
  await new Promise((resolve) => setTimeout(resolve, 400));
}
if (Date.now() >= deadline) { kill(); console.error("vite readiness timeout"); process.exit(1); }
try {
  const child = spawn(commandArgs[0], commandArgs.slice(1), { stdio: "inherit", shell: process.platform === "win32", env: { ...process.env, PORT: String(port) }, windowsHide: true });
  process.exitCode = await new Promise((resolve) => {
    child.on("exit", (status) => resolve(status ?? 1));
    child.on("error", () => resolve(1));
  });
} finally { kill(); }
`
}

function smokeScript() {
  return `import { chromium } from "playwright";
import fs from "node:fs";
const port = Number(process.env.PORT || 4173);
fs.mkdirSync("artifacts", { recursive: true });
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
await page.screenshot({ path: "artifacts/arcade-smoke.png", fullPage: true });
console.log(await page.locator("body").textContent());
await browser.close();
`
}

function evaluatorScript() {
  return `import { chromium } from "playwright";
import fs from "node:fs";
const port = Number(process.env.PORT || 4173);
fs.mkdirSync("artifacts", { recursive: true });
const failures = [];
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 980, height: 760 } });
await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
const homeText = await page.locator("body").textContent();
for (const word of ["arcade", "snake", "tetris"]) if (!String(homeText || "").toLowerCase().includes(word)) failures.push(word + " missing");
const snakeEntry = page.getByRole("button", { name: /snake|贪吃蛇/i }).first();
const tetrisEntry = page.getByRole("button", { name: /tetris|俄罗斯方块/i }).first();
if (await snakeEntry.count()) {} else failures.push("snake entry missing");
if (await tetrisEntry.count()) {} else failures.push("tetris entry missing");
if (await snakeEntry.count()) await snakeEntry.click();
else await page.getByText(/snake|贪吃蛇/i).first().click().catch(() => {});
await page.waitForTimeout(200);
if (await page.getByText(/score|得分/i).count()) {} else failures.push("snake score missing");
const snakeBoardCount = await page.locator("canvas, [data-testid*=board], .board, .game-board, [aria-label*=Snake], [aria-label*=snake]").count();
if (snakeBoardCount > 0) {} else failures.push("snake board missing");
const before = await page.locator("body").textContent();
await page.getByRole("button", { name: /start|restart|play|开始|重新/i }).first().click().catch(() => {});
await page.keyboard.press("ArrowRight");
await page.waitForTimeout(250);
await page.keyboard.press("ArrowDown");
await page.waitForTimeout(500);
const after = await page.locator("body").textContent();
if (after !== before || /score\\s*[:：]?\\s*[1-9]/i.test(after || "")) {} else failures.push("snake keyboard interaction did not change visible state");
const metrics = await page.evaluate(() => ({ overflow: document.documentElement.scrollWidth > innerWidth + 1, text: document.body.textContent || "" }));
if (!metrics.overflow) {} else failures.push("horizontal overflow");
await page.getByRole("button", { name: /back|home|arcade|返回|主页/i }).first().click().catch(() => {});
await page.waitForTimeout(200);
await page.getByRole("button", { name: /tetris|俄罗斯方块/i }).first().click().catch(() => {});
await page.waitForTimeout(200);
if (await page.getByText(/score|lines|level|得分|行/i).count()) {} else failures.push("tetris score/lines missing");
const tetrisBoardCount = await page.locator("canvas, [data-testid*=board], .board, .game-board, [aria-label*=Tetris], [aria-label*=tetris]").count();
if (tetrisBoardCount > 0) {} else failures.push("tetris board missing");
await page.keyboard.press("ArrowLeft");
await page.keyboard.press("ArrowUp");
await page.waitForTimeout(400);
await page.screenshot({ path: "artifacts/arcade-evaluator.png", fullPage: true });
await browser.close();
const source = fs.readFileSync("src/App.jsx", "utf8") + "\\n" + fs.readFileSync("src/styles.css", "utf8");
if (/keydown|Arrow|requestAnimationFrame|setInterval|canvas|grid/i.test(source)) {} else failures.push("source lacks game loop/keyboard implementation");
if (/tetris|tetromino|俄罗斯方块|piece|rotate/i.test(source)) {} else failures.push("source lacks tetris implementation");
const result = { pass: failures.length === 0, failures, screenshot: "artifacts/arcade-evaluator.png" };
console.log(JSON.stringify(result, null, 2));
if (!result.pass) process.exit(1);
`
}

function promptPhase1(port) {
  return `Phase 1: Build a polished playable Snake game in this React workspace. You must edit files with command_run/apply_patch or shell commands; do not answer with a description only. For large file writes, prefer command_run apply_patch with valid Begin Patch/Delete File/Add File hunks instead of long inline node -e or raw PowerShell source. Edit src/App.jsx and src/styles.css. It must show Snake or 贪吃蛇, score, start/restart, responsive board, food, moving snake, keyboard arrows, collision/game-over, and no mobile horizontal overflow. Run npm install if needed, then run PORT=${port} npm run verify:arcade with command timeout_ms at least 120000. In phase 1 it is acceptable if the evaluator still complains about missing Arcade/Tetris; make Snake itself real and playable.`
}

function promptPhase2(port) {
  return `Phase 2: Continue by editing this workspace with command_run/apply_patch or shell commands; do not answer with a description only. For large file writes, prefer command_run apply_patch with valid Begin Patch/Delete File/Add File hunks instead of long inline node -e or raw PowerShell source. Keep the Snake game and add an Arcade entrance/home screen with two games: Snake and Tetris/俄罗斯方块. The Arcade entry must have visible buttons or tabs for Snake and Tetris. Implement a simple playable Tetris board with falling blocks or at least keyboard-controlled pieces, score/lines, restart, and responsive layout. Run PORT=${port} npm run verify:arcade with command timeout_ms at least 120000 and fix all failures.`
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
    usage.input += Number(u.input_tokens || u.prompt_tokens || 0)
    usage.cached += Number(u.cached_input_tokens || u.input_tokens_details?.cached_tokens || u.prompt_tokens_details?.cached_tokens || u.cache_read_input_tokens || 0)
    usage.output += Number(u.output_tokens || u.completion_tokens || 0)
    usage.reasoning += Number(u.reasoning_output_tokens || u.reasoning_tokens || u.output_tokens_details?.reasoning_tokens || u.completion_tokens_details?.reasoning_tokens || 0)
    usage.total += Number(u.total_tokens || 0)
  }
  return usage
}

function readSessionLog(workspace, sessionId) {
  const sessionPath = path.join(workspace, ".tura", "sessions", `${sessionId}.json`)
  if (!fs.existsSync(sessionPath)) return { path: sessionPath, state: "missing", entries: [] }
  try {
    const value = JSON.parse(fs.readFileSync(sessionPath, "utf8"))
    const entries = value?.info?.management?.session_log
      ?.map((entry) => {
        try { return JSON.parse(entry) } catch { return null }
      })
      .filter(Boolean) || []
    return {
      path: sessionPath,
      state: value?.info?.management?.state || value?.info?.status || "unknown",
      entries,
    }
  } catch {
    return { path: sessionPath, state: "unreadable", entries: [] }
  }
}

function usageFromSessionLog(entries) {
  const usage = { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 }
  for (const entry of entries) {
    if (entry?.type !== "runtime_usage") continue
    const u = entry.usage || {}
    usage.input += Number(u.input_tokens || 0)
    usage.cached += Number(u.cached_input_tokens || 0)
    usage.output += Number(u.output_tokens || 0)
    usage.reasoning += Number(u.reasoning_tokens || u.reasoning_output_tokens || 0)
    usage.total += Number(u.total_tokens || 0)
  }
  return usage
}

function countSessionLog(entries) {
  let commands = 0
  let failures = 0
  let turns = 0
  for (const entry of entries) {
    if (entry?.type === "runtime_usage") turns += 1
    if (entry?.type !== "tool_result" || entry?.tool_name !== "command_run") continue
    const results = entry?.output?.results
    const resultCount = Array.isArray(results) ? results.length : 1
    commands += resultCount
    if (entry.success === false) failures += 1
    if (Array.isArray(results)) {
      failures += results.filter((result) => result?.success === false).length
    }
  }
  return { turns, commands, failures }
}

function countEvents(events) {
  let commands = 0
  let failures = 0
  let turns = 0
  for (const event of events) {
    if (event.type === "turn.started") turns += 1
    if (event.type === "turn.completed") turns += 1
    if (event.item?.type === "command_execution") {
      commands += 1
      if (event.item.exit_code && event.item.exit_code !== 0) failures += 1
    }
  }
  return { turns, commands, failures }
}

function latestProviderLogs(sinceMs) {
  const root = path.join(repoRoot, "log", "provider")
  const out = []
  if (!fs.existsSync(root)) return out
  for (const day of fs.readdirSync(root)) {
    const dir = path.join(root, day)
    if (!fs.statSync(dir).isDirectory()) continue
    for (const file of fs.readdirSync(dir)) {
      const full = path.join(dir, file)
      const stat = fs.statSync(full)
      if (stat.mtimeMs >= sinceMs) out.push(full)
    }
  }
  return out
}

function analyzeProviderLogs(paths, model) {
  const hits = []
  const usage = { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 }
  const modelName = model.split("/").slice(1).join("/")
  for (const file of paths) {
    try {
      const value = JSON.parse(fs.readFileSync(file, "utf8"))
      if (value?.request?.model !== modelName && value?.model !== modelName) continue
      hits.push(file)
      const u = value?.metrics?.usage || value?.usage || value?.response?.usage
      if (!u) continue
      usage.input += Number(u.input_tokens || u.prompt_tokens || 0)
      usage.cached += Number(u.cached_input_tokens || u.prompt_tokens_details?.cached_tokens || 0)
      usage.output += Number(u.output_tokens || u.completion_tokens || 0)
      usage.reasoning += Number(u.reasoning_tokens || u.completion_tokens_details?.reasoning_tokens || 0)
      usage.total += Number(u.total_tokens || 0)
    } catch {}
  }
  return { files: hits, usage }
}

async function runAgent(agent, template, index) {
  const startedAt = Date.now()
  const agentDir = path.join(runRoot, agent)
  const workspace = path.join(agentDir, "workspace")
  const port = 4273 + index
  const model = defaultModels[agent]
  const sessionId = `arcade-${agent}-${Date.now()}`
  const agentDeadline = performance.now() + timeoutMs
  const remainingAgentMs = () => Math.max(1_000, Math.ceil(agentDeadline - performance.now()))
  copyDir(template, workspace)
  const common = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--dangerously-bypass-approvals-and-sandbox",
    "--session-id",
    sessionId,
    "--agent",
    "coding_agent_fast",
    "-m",
    model,
    "-c",
    `model_reasoning_effort=${reasoning}`,
    ...serviceTierConfigArgs(),
    "--cwd",
    workspace,
  ]
  const env = {
    TURA_ENV_PATH: process.env.TURA_ENV_PATH || path.join(repoRoot, ".env"),
    TURA_COMMAND_RUN_SHELL: "shell_command",
    TURA_COMMAND_RUN_STRICT_JSON: "0",
    COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
  }
  const phase1 = await runLive(turaExe, [...common, promptPhase1(port)], {
    cwd: workspace,
    env,
    timeoutMs: remainingAgentMs(),
    stdoutPath: path.join(agentDir, "phase1.stdout.jsonl"),
    stderrPath: path.join(agentDir, "phase1.stderr.log"),
  })
  const phase2 = await runLive(turaExe, [...common, promptPhase2(port)], {
    cwd: workspace,
    env,
    timeoutMs: remainingAgentMs(),
    stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
    stderrPath: path.join(agentDir, "phase2.stderr.log"),
  })
  const validation = run(npmCmd, ["run", "verify:arcade"], { cwd: workspace, timeoutMs: 180_000, shell: process.platform === "win32", env: { PORT: String(port) } })
  let parsedValidation = { pass: false, stdout: validation.stdout, stderr: validation.stderr, status: validation.status }
  try {
    const match = validation.stdout.match(/\{[\s\S]*\}\s*$/)
    if (match) parsedValidation = JSON.parse(match[0])
  } catch {}
  const events = [...parseJsonl(phase1.stdout), ...parseJsonl(phase2.stdout)]
  const sessionLog = readSessionLog(workspace, sessionId)
  const eventCounts = countEvents(events)
  const sessionCounts = countSessionLog(sessionLog.entries)
  const eventUsage = usageFromEvents(events)
  const sessionUsage = usageFromSessionLog(sessionLog.entries)
  const providerLogs = analyzeProviderLogs(latestProviderLogs(startedAt), model)
  const usage = eventUsage.total > 0
    ? eventUsage
    : sessionUsage.total > 0
      ? sessionUsage
      : providerLogs.usage
  const stats = {
    id: agent,
    model,
    workspace,
    session_id: sessionId,
    session_log: {
      path: sessionLog.path,
      state: sessionLog.state,
      entries: sessionLog.entries.length,
    },
    timeout_ms: timeoutMs,
    phase1_status: phase1.status,
    phase2_status: phase2.status,
    phase1_error: phase1.error,
    phase2_error: phase2.error,
    phase1_ms: phase1.duration_ms,
    phase2_ms: phase2.duration_ms,
    events: {
      turns: Math.max(eventCounts.turns, sessionCounts.turns),
      commands: Math.max(eventCounts.commands, sessionCounts.commands),
      failures: Math.max(eventCounts.failures, sessionCounts.failures),
      stdout: eventCounts,
      session_log: sessionCounts,
    },
    event_usage: usage,
    provider_logs: providerLogs,
    validation: parsedValidation,
    ok: phase1.status === 0 && phase2.status === 0 && parsedValidation.pass,
  }
  writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
  return stats
}

async function main() {
  mkdirp(runRoot)
  const template = createFixtureTemplate()
  runOk(npmCmd, ["install"], { cwd: template, timeoutMs: 180_000, shell: process.platform === "win32" })
  runOk(npxCmd, ["playwright", "install", "chromium"], { cwd: template, timeoutMs: 240_000, shell: process.platform === "win32" })
  runOk("cargo", ["build", "-p", "gateway", "--bin", "tura"], { cwd: repoRoot, timeoutMs: 300_000 })
  assert(fs.existsSync(turaExe), `missing tura executable: ${turaExe}`)
  const results = await Promise.all(agents.map((agent, index) => runAgent(agent, template, index)))
  const summary = {
    ok: results.every((result) => result.ok),
    run_id: runId,
    run_root: runRoot,
    timeout_ms: timeoutMs,
    reasoning,
    service_tier: serviceTier,
    agents,
    models: defaultModels,
    results,
  }
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
}

await main()
