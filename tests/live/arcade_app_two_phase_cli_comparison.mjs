#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { agentEventStats, agentUsageFromJsonl, claudeCodeArgs, findClaudeExe, findPiExe, piAgentArgs } from "./live_lib_agent_cli.mjs"
import { businessRunPaths, normalizeBusinessSummary } from "../business/business_lib_business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `arcade-two-step-${Date.now()}`
const runPaths = businessRunPaths("tui-command-run-arcade-two-step", runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 180_000)
const phaseTimeoutMs = Number(process.env.COMMAND_RUN_AGENT_PHASE_TIMEOUT_MS || timeoutMs)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm"
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx"
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const gatewayExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway")
const claudeExe = findClaudeExe()
const piExe = findPiExe()
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

function isExternalCliAgent(agent) {
  return agent === "claude-code" || agent === "pi-agent"
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
      if (
        stdout.includes("\"type\":\"turn.completed\"") ||
        /"task_status"\s*:\s*\{[\s\S]*?"status"\s*:\s*"done"/u.test(stdout)
      ) {
        settle(0, null, null)
      }
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
  return tier === "priority" ? ["-p"] : []
}

function createFixtureTemplate() {
  const template = path.join(runRoot, "template")
  mkdirp(template)
  writeFile(path.join(template, "package.json"), JSON.stringify({
    scripts: {
      start: "vite --host 127.0.0.1",
      "capture:arcade": "node tools/with_vite.mjs -- node tools/capture_arcade.mjs",
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
  writeFile(path.join(template, "tools", "capture_arcade.mjs"), captureArcadeScript())
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

function captureArcadeScript() {
  return `import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";
const port = Number(process.env.PORT || 4173);
const outDir = path.resolve("artifacts", "arcade-captures");
fs.mkdirSync(outDir, { recursive: true });
const screenshots = [];
async function shot(page, name, fullPage = true) {
  const file = path.join(outDir, name + ".png");
  await page.screenshot({ path: file, fullPage });
  screenshots.push({ name, path: path.relative(process.cwd(), file).replace(/\\\\/g, "/") });
}
async function clickFirst(page, matcher) {
  const button = page.getByRole("button", { name: matcher }).first();
  if (await button.count()) {
    await button.click();
    return true;
  }
  const text = page.getByText(matcher).first();
  if (await text.count()) {
    await text.click();
    return true;
  }
  return false;
}
const browser = await chromium.launch({ headless: true });
const desktop = await browser.newPage({ viewport: { width: 1100, height: 820 } });
const diagnostics = [];
desktop.on("pageerror", (error) => diagnostics.push({ type: "desktop-pageerror", message: String(error?.message || error) }));
desktop.on("console", (message) => {
  if (["error", "warning"].includes(message.type())) diagnostics.push({ type: "desktop-console-" + message.type(), message: message.text() });
});
await desktop.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
await shot(desktop, "01-home-desktop");
const homeText = await desktop.locator("body").textContent();
await clickFirst(desktop, /snake|贪吃蛇/i);
await desktop.waitForTimeout(250);
await shot(desktop, "02-snake-open");
await clickFirst(desktop, /start|restart|play|开始|重新/i);
for (const key of ["ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp"]) {
  await desktop.keyboard.press(key);
  await desktop.waitForTimeout(160);
}
await shot(desktop, "03-snake-after-input");
await clickFirst(desktop, /back|home|arcade|返回|主页/i);
await desktop.waitForTimeout(200);
await clickFirst(desktop, /tetris|俄罗斯方块/i);
await desktop.waitForTimeout(300);
await shot(desktop, "04-tetris-open");
const mobile = await browser.newPage({ viewport: { width: 390, height: 844 } });
mobile.on("pageerror", (error) => diagnostics.push({ type: "mobile-pageerror", message: String(error?.message || error) }));
await mobile.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
await shot(mobile, "05-home-mobile");
await clickFirst(mobile, /snake|贪吃蛇/i);
await mobile.waitForTimeout(250);
await shot(mobile, "06-snake-mobile");
const overflow = await mobile.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
await browser.close();
const missingHomeText = !/arcade|snake|贪吃蛇/i.test(String(homeText || ""));
const result = { ok: !overflow && !missingHomeText && diagnostics.length === 0, overflow, missingHomeText, diagnostics, screenshots };
fs.writeFileSync(path.join(outDir, "manifest.json"), JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
if (!result.ok) process.exit(1);
`
}

function evaluatorScript() {
  return `import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";
const port = Number(process.env.PORT || 4173);
fs.mkdirSync("artifacts", { recursive: true });
const outDir = path.resolve("artifacts", "arcade-evaluator");
fs.mkdirSync(outDir, { recursive: true });
const failures = [];
const screenshots = [];
const diagnostics = [];
async function shot(page, name) {
  const file = path.join(outDir, name + ".png");
  await page.screenshot({ path: file, fullPage: true });
  screenshots.push({ name, path: path.relative(process.cwd(), file).replace(/\\\\/g, "/") });
}
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 980, height: 760 } });
page.on("pageerror", (error) => diagnostics.push({ type: "pageerror", message: String(error?.message || error) }));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type())) diagnostics.push({ type: "console-" + message.type(), message: message.text() });
});
await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
await shot(page, "01-home");
const homeText = await page.locator("body").textContent();
if (!String(homeText || "").trim()) failures.push("page rendered no visible text");
for (const word of ["arcade", "snake", "tetris"]) if (!String(homeText || "").toLowerCase().includes(word)) failures.push(word + " missing");
const snakeEntry = page.getByRole("button", { name: /snake|贪吃蛇/i }).first();
const tetrisEntry = page.getByRole("button", { name: /tetris|俄罗斯方块/i }).first();
if (await snakeEntry.count()) {} else failures.push("snake entry missing");
if (await tetrisEntry.count()) {} else failures.push("tetris entry missing");
if (await snakeEntry.count()) await snakeEntry.click();
else await page.getByText(/snake|贪吃蛇/i).first().click().catch(() => {});
await page.waitForTimeout(200);
await shot(page, "02-snake-open");
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
await shot(page, "03-snake-after-input");
if (after !== before || /score\\s*[:：]?\\s*[1-9]/i.test(after || "")) {} else failures.push("snake keyboard interaction did not change visible state");
const metrics = await page.evaluate(() => ({ overflow: document.documentElement.scrollWidth > innerWidth + 1, text: document.body.textContent || "" }));
if (!metrics.overflow) {} else failures.push("horizontal overflow");
await page.getByRole("button", { name: /back|home|arcade|返回|主页/i }).first().click().catch(() => {});
await page.waitForTimeout(200);
await page.getByRole("button", { name: /tetris|俄罗斯方块/i }).first().click().catch(() => {});
await page.waitForTimeout(200);
await shot(page, "04-tetris-open");
if (await page.getByText(/score|lines|level|得分|行/i).count()) {} else failures.push("tetris score/lines missing");
const tetrisBoardCount = await page.locator("canvas, [data-testid*=board], .board, .game-board, [aria-label*=Tetris], [aria-label*=tetris]").count();
if (tetrisBoardCount > 0) {} else failures.push("tetris board missing");
await page.keyboard.press("ArrowLeft");
await page.keyboard.press("ArrowUp");
await page.waitForTimeout(400);
await shot(page, "05-tetris-after-input");
await page.screenshot({ path: "artifacts/arcade-evaluator.png", fullPage: true });
screenshots.push({ name: "legacy-final", path: "artifacts/arcade-evaluator.png" });
await browser.close();
const source = fs.readFileSync("src/App.jsx", "utf8") + "\\n" + fs.readFileSync("src/styles.css", "utf8");
if (/keydown|Arrow|requestAnimationFrame|setInterval|canvas|grid/i.test(source)) {} else failures.push("source lacks game loop/keyboard implementation");
if (/tetris|tetromino|俄罗斯方块|piece|rotate/i.test(source)) {} else failures.push("source lacks tetris implementation");
if (/createRoot/i.test(source) && /\\.render\\s*\\(/i.test(source)) {} else failures.push("source does not mount React with createRoot(...).render(...)");
const result = { pass: failures.length === 0 && diagnostics.length === 0, failures, diagnostics, screenshot: "artifacts/arcade-evaluator.png", screenshots };
console.log(JSON.stringify(result, null, 2));
if (!result.pass) process.exit(1);
`
}

function promptPhase1(port) {
  return `Phase 1: Build a polished playable Snake game in this current React workspace only. Do not read from, copy from, diff against, or inspect any other benchmark run directory, sibling agent workspace, old target artifact, previous solution, or path outside this workspace; all implementation work must be based only on the files and assets already present under the current working directory. You must edit files with command_run/apply_patch or shell commands; do not answer with a description only. For large file writes, prefer command_run apply_patch with valid Begin Patch/Delete File/Add File hunks instead of long inline node -e or raw PowerShell source. Edit src/App.jsx and src/styles.css. Keep a working React mount in src/App.jsx, for example createRoot(document.getElementById("root")).render(<App />); the page must not be blank. It must show Snake or 贪吃蛇, score, start/restart, responsive board, food, moving snake, keyboard arrows, collision/game-over, and no mobile horizontal overflow. Run npm install if needed. After the first visual implementation, run PORT=${port} npm run capture:arcade with command timeout_ms at least 120000, inspect artifacts/arcade-captures/manifest.json and the screenshots it lists, then fix visible layout or interaction bugs. Finally run PORT=${port} npm run verify:arcade with command timeout_ms at least 120000. In phase 1 it is acceptable if the evaluator still complains about missing Arcade/Tetris; make Snake itself real and playable.`
}

function promptPhase2(port) {
  return `Phase 2: Continue by editing this current workspace only with command_run/apply_patch or shell commands; do not answer with a description only. Do not read from, copy from, diff against, or inspect any other benchmark run directory, sibling agent workspace, old target artifact, previous solution, or path outside this workspace; all implementation work must be based only on the files and assets already present under the current working directory. For large file writes, prefer command_run apply_patch with valid Begin Patch/Delete File/Add File hunks instead of long inline node -e or raw PowerShell source. Keep a working React mount in src/App.jsx, for example createRoot(document.getElementById("root")).render(<App />); the page must not be blank. Keep the Snake game and add an Arcade entrance/home screen with two games: Snake and Tetris/俄罗斯方块. The Arcade entry must have visible buttons or tabs for Snake and Tetris. Implement a simple playable Tetris board with falling blocks or at least keyboard-controlled pieces, score/lines, restart, and responsive layout. Run PORT=${port} npm run capture:arcade with command timeout_ms at least 120000, inspect artifacts/arcade-captures/manifest.json and its screenshots, fix visible bugs, then run PORT=${port} npm run verify:arcade with command timeout_ms at least 120000 and fix all failures.`
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
  const sessionPath = `session_log:${sessionId}`
  try {
    const result = run(gatewayExe, ["session-log"], {
      input: JSON.stringify({ command: "get_session", session_id: sessionId }),
      cwd: repoRoot,
    })
    if (result.status !== 0) return { path: sessionPath, state: "missing", entries: [] }
    const response = JSON.parse(result.stdout)
    const management = response?.session?.management
    const entries = management?.session_log
      ?.map((entry) => {
        try { return JSON.parse(entry) } catch { return null }
      })
      .filter(Boolean) || []
    return {
      path: sessionPath,
      state: management?.state || response?.session?.status || "unknown",
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
  const model = isExternalCliAgent(agent)
    ? (agent === "claude-code" ? process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus" : "pi")
    : defaultModels[agent]
  const sessionId = `arcade-${agent}-${Date.now()}`
  copyDir(template, workspace)
  let phase1
  let phase2
  let externalSessionId = null
  if (isExternalCliAgent(agent)) {
    const isClaude = agent === "claude-code"
    phase1 = await runLive(isClaude ? claudeExe : piExe, isClaude
      ? claudeCodeArgs(promptPhase1(port), { model })
      : piAgentArgs(promptPhase1(port)), {
      cwd: workspace,
      timeoutMs: phaseTimeoutMs,
      stdoutPath: path.join(agentDir, "phase1.stdout.jsonl"),
      stderrPath: path.join(agentDir, "phase1.stderr.log"),
    })
    const phase1Events = parseJsonl(phase1.stdout)
    externalSessionId = isClaude
      ? phase1Events.find((event) => event.type === "result")?.session_id || phase1Events.find((event) => event.session_id)?.session_id || null
      : null
    const phase2Args = isClaude && externalSessionId
      ? [
          "--print",
          "--resume",
          externalSessionId,
          "--model",
          model,
          "--output-format",
          "stream-json",
          "--verbose",
          "--dangerously-skip-permissions",
          promptPhase2(port),
        ]
      : (isClaude ? claudeCodeArgs(promptPhase2(port), { model }) : piAgentArgs(promptPhase2(port)))
    phase2 = await runLive(isClaude ? claudeExe : piExe, phase2Args, {
      cwd: workspace,
      timeoutMs: phaseTimeoutMs,
      stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
      stderrPath: path.join(agentDir, "phase2.stderr.log"),
    })
  } else {
    const common = [
      "exec",
      "--json",
      "--skip-git-repo-check",
      "--dangerously-bypass-approvals-and-sandbox",
      "--session-id",
      sessionId,
      "--agent-id",
      "fast",
      "-m",
      model,
      ...serviceTierConfigArgs(),
      "--model-reasoning-effort",
      reasoning,
      "--cwd",
      workspace,
    ]
    const env = {
      TURA_ENV_PATH: process.env.TURA_ENV_PATH || path.join(repoRoot, ".env"),
      TURA_COMMAND_RUN_SHELL: "shell_command",
      TURA_COMMAND_RUN_STRICT_JSON: "0",
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    }
    phase1 = await runLive(turaExe, [...common, promptPhase1(port)], {
      cwd: workspace,
      env,
      timeoutMs: phaseTimeoutMs,
      stdoutPath: path.join(agentDir, "phase1.stdout.jsonl"),
      stderrPath: path.join(agentDir, "phase1.stderr.log"),
    })
    phase2 = await runLive(turaExe, [...common, promptPhase2(port)], {
      cwd: workspace,
      env,
      timeoutMs: phaseTimeoutMs,
      stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
      stderrPath: path.join(agentDir, "phase2.stderr.log"),
    })
  }
  const validation = run(npmCmd, ["run", "verify:arcade"], { cwd: workspace, timeoutMs: 180_000, shell: process.platform === "win32", env: { PORT: String(port) } })
  let parsedValidation = { pass: false, stdout: validation.stdout, stderr: validation.stderr, status: validation.status }
  try {
    const match = validation.stdout.match(/\{[\s\S]*\}\s*$/)
    if (match) parsedValidation = JSON.parse(match[0])
  } catch {}
  const events = [...parseJsonl(phase1.stdout), ...parseJsonl(phase2.stdout)]
  const sessionLog = isExternalCliAgent(agent) ? { path: null, state: "external-cli", entries: [] } : readSessionLog(workspace, sessionId)
  const eventCounts = countEvents(events)
  const sessionCounts = countSessionLog(sessionLog.entries)
  const eventUsage = usageFromEvents(events)
  const sessionUsage = usageFromSessionLog(sessionLog.entries)
  const providerLogs = isExternalCliAgent(agent) ? { files: [], usage: agentUsageFromJsonl(`${phase1.stdout}\n${phase2.stdout}`) } : analyzeProviderLogs(latestProviderLogs(startedAt), model)
  const usage = isExternalCliAgent(agent)
    ? providerLogs.usage
    : eventUsage.total > 0
      ? eventUsage
      : sessionUsage.total > 0
        ? sessionUsage
        : providerLogs.usage
  const stats = {
    id: agent,
    model,
    workspace,
    session_id: isExternalCliAgent(agent) ? externalSessionId : sessionId,
    invocation: isExternalCliAgent(agent) ? `${agent}-cli` : "tura-cli",
    session_log: {
      path: sessionLog.path,
      state: sessionLog.state,
      entries: sessionLog.entries.length,
    },
    timeout_ms: timeoutMs,
    phase_timeout_ms: phaseTimeoutMs,
    phase1_status: phase1.status,
    phase2_status: phase2.status,
    phase1_error: phase1.error,
    phase2_error: phase2.error,
    phase1_ms: phase1.duration_ms,
    phase2_ms: phase2.duration_ms,
    events: isExternalCliAgent(agent) ? {
      stdout: agentEventStats(`${phase1.stdout}\n${phase2.stdout}`),
    } : {
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
  if (agents.some((agent) => !isExternalCliAgent(agent))) {
    runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_exec"], { cwd: repoRoot, timeoutMs: 300_000 })
    assert(fs.existsSync(turaExe), `missing cli executable: ${turaExe}`)
  }
  const results = await Promise.all(agents.map((agent, index) => runAgent(agent, template, index)))
  const summary = normalizeBusinessSummary({
    ok: results.every((result) => result.ok),
    timeout_ms: timeoutMs,
    phase_timeout_ms: phaseTimeoutMs,
    reasoning,
    service_tier: serviceTier,
    agents,
    models: defaultModels,
    results,
  }, runPaths)
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
}

await main()
