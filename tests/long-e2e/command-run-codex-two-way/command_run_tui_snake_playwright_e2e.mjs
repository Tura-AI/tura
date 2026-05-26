#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import http from "node:http"
import { createRequire } from "node:module"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { isolatedProcessOptions, killProcessTree } from "./process_helpers.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `tui-snake-${Date.now()}`
const runRoot = path.join(repoRoot, "target", "command-run-tui-snake", runId)
const summaryPath = path.join(runRoot, "summary.json")
const model = process.env.COMMAND_RUN_AGENT_TURA_MODEL || "openai/gpt-5.5"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 20 * 60_000)
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm"
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx"
const tuiBin = path.join(repoRoot, "apps", "tui", "dist", "index.js")
const tuiRequire = createRequire(path.join(repoRoot, "apps", "tui", "package.json"))

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text)
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: 128 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    shell: options.shell || false,
    windowsHide: true,
  })
  return {
    status: result.status,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    error: result.error ? String(result.error.stack || result.error.message || result.error) : null,
  }
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
  }
  return result
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

function createFixture() {
  const template = path.join(runRoot, "template")
  mkdirp(template)
  writeFile(path.join(template, "package.json"), JSON.stringify({
    scripts: {
      start: "vite --host 127.0.0.1",
      "verify:snake": "node tools/with_vite.mjs -- node tools/evaluate_snake.mjs",
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
  writeFile(path.join(template, "tools", "evaluate_snake.mjs"), evaluatorScript())
  writeFile(path.join(template, "TASK.md"), `Build a polished, playable Snake game in this React app. The hidden evaluator uses Playwright and checks real keyboard interaction, responsive layout, score, restart/game-over state, and a screenshot artifact.\n`)
  return template
}

function withViteScript() {
  return `import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
const separator = process.argv.indexOf("--");
const commandArgs = separator >= 0 ? process.argv.slice(separator + 1) : [];
if (!commandArgs.length) process.exit(2);
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const port = Number(process.env.PORT || 4173);
fs.mkdirSync("artifacts", { recursive: true });
const out = path.resolve("artifacts/vite.stdout.log");
const err = path.resolve("artifacts/vite.stderr.log");
function tail(file) { try { return fs.existsSync(file) ? fs.readFileSync(file, "utf8").slice(-3000) : ""; } catch { return ""; } }
function kill(pid) {
  if (!pid) return;
  if (process.platform === "win32") spawn("taskkill", ["/pid", String(pid), "/t", "/f"], { stdio: "ignore", windowsHide: true });
  else { try { process.kill(-pid, "SIGTERM"); } catch { try { process.kill(pid, "SIGTERM"); } catch {} } }
}
const server = spawn(npmCmd, ["run", "start", "--", "--port", String(port), "--strictPort"], {
  stdio: ["ignore", fs.openSync(out, "w"), fs.openSync(err, "w")],
  shell: process.platform === "win32",
  detached: process.platform !== "win32",
  windowsHide: true,
});
const deadline = Date.now() + 30000;
while (Date.now() < deadline) {
  if (server.exitCode !== null || server.signalCode !== null) {
    console.error("Vite exited before ready\\n" + tail(err) + "\\n" + tail(out));
    kill(server.pid);
    process.exit(1);
  }
  try {
    const response = await fetch(\`http://127.0.0.1:\${port}\`);
    if (response.ok) break;
  } catch {}
  await new Promise((resolve) => setTimeout(resolve, 400));
}
if (Date.now() >= deadline) {
  console.error("Vite readiness timeout\\n" + tail(err) + "\\n" + tail(out));
  kill(server.pid);
  process.exit(1);
}
try {
  const child = spawn(commandArgs[0], commandArgs.slice(1), {
    stdio: "inherit",
    shell: process.platform === "win32",
    env: { ...process.env, PORT: String(port) },
    windowsHide: true,
  });
  const code = await new Promise((resolve) => {
    child.on("exit", (status) => resolve(status ?? 1));
    child.on("error", () => resolve(1));
  });
  process.exitCode = code;
} finally {
  kill(server.pid);
}
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
await page.screenshot({ path: "artifacts/snake-smoke.png", fullPage: true });
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
const body = await page.locator("body").textContent();
if (/snake|贪吃蛇/i.test(body || "")) {} else failures.push("title missing snake");
if (await page.getByText(/score|得分/i).count()) {} else failures.push("score missing");
if (await page.getByRole("button", { name: /start|restart|play|开始|重新/i }).count()) {} else failures.push("start/restart button missing");
const boardCount = await page.locator("canvas, [data-testid*=board], .board, .game-board, [aria-label*=Snake], [aria-label*=snake]").count();
if (boardCount > 0) {} else failures.push("game board missing");
const before = await page.locator("body").textContent();
await page.getByRole("button", { name: /start|restart|play|开始|重新/i }).first().click().catch(() => {});
await page.keyboard.press("ArrowRight");
await page.waitForTimeout(250);
await page.keyboard.press("ArrowDown");
await page.waitForTimeout(600);
const after = await page.locator("body").textContent();
if (after !== before || /score\\s*[:：]?\\s*[1-9]/i.test(after || "")) {} else failures.push("keyboard interaction did not change visible state");
const metrics = await page.evaluate(() => ({
  overflow: document.documentElement.scrollWidth > innerWidth + 1,
  buttons: document.querySelectorAll("button").length,
  text: document.body.textContent || "",
}));
if (!metrics.overflow) {} else failures.push("horizontal overflow");
if (metrics.buttons >= 1) {} else failures.push("no controls");
await page.screenshot({ path: "artifacts/snake-evaluator.png", fullPage: true });
await browser.close();
const source = fs.readFileSync("src/App.jsx", "utf8") + "\\n" + fs.readFileSync("src/styles.css", "utf8");
if (/keydown|Arrow|requestAnimationFrame|setInterval|canvas|grid/i.test(source)) {} else failures.push("source lacks game loop/keyboard implementation");
const result = { pass: failures.length === 0, failures, screenshot: "artifacts/snake-evaluator.png" };
console.log(JSON.stringify(result, null, 2));
if (!result.pass) process.exit(1);
`
}

function startGatewayProcess(port, agentDir, env) {
  const gatewayExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "gateway.exe" : "gateway")
  const stdoutPath = path.join(agentDir, "gateway.stdout.log")
  const stderrPath = path.join(agentDir, "gateway.stderr.log")
  mkdirp(agentDir)
  const child = spawn(gatewayExe, [], isolatedProcessOptions({
    cwd: repoRoot,
    env: { ...process.env, ...env, PORT: String(port) },
    stdio: ["ignore", fs.openSync(stdoutPath, "w"), fs.openSync(stderrPath, "w")],
    windowsHide: true,
  }))
  child.once("error", () => {})
  return child
}

async function waitForGateway(port, child) {
  const deadline = Date.now() + 60_000
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) return false
    try {
      const response = await fetch(`http://127.0.0.1:${port}/global/health`)
      if (response.ok) return true
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
  return false
}

function realTuiBridgeHtml() {
  return `<!doctype html><html><head><meta charset="utf-8"><style>
body{margin:0;background:#0b1020;color:#e5edf7;font-family:Consolas,Menlo,monospace}
.wrap{height:100vh;display:grid;grid-template-rows:1fr auto}
#screen{white-space:pre-wrap;margin:0;padding:18px;overflow:auto;font-size:14px;line-height:1.28}
.bar{display:flex;gap:10px;padding:12px;background:#121a2f;border-top:1px solid #26324f}
textarea{flex:1;min-height:54px;background:#070b16;color:white;border:1px solid #33415f;border-radius:8px;padding:10px;font:inherit}
button{background:#4f8cff;color:white;border:0;border-radius:8px;padding:0 18px;font-weight:700}
</style></head><body><div class="wrap"><pre id="screen" aria-label="TUI screen"></pre><div class="bar"><textarea id="input" aria-label="TUI input"></textarea><button id="send">Send</button></div></div><script>
const screenEl=document.querySelector('#screen');const inputEl=document.querySelector('#input');const sendEl=document.querySelector('#send');
async function refresh(){const r=await fetch('/screen');const j=await r.json();screenEl.textContent=j.screen || '';screenEl.scrollTop=screenEl.scrollHeight}
setInterval(refresh,200);refresh();
sendEl.onclick=async()=>{await fetch('/input',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({text:inputEl.value,submit:true,slow:true})});inputEl.value='';await refresh()}
</script></body></html>`
}

function stripAnsi(text) {
  return text
    .replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, "")
    .replace(/\x1b\][^\x07]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b[PX^_][\s\S]*?(?:\x07|\x1b\\)/g, "")
}

function latestTuiScreen(raw) {
  const cleaned = stripAnsi(raw)
  const marker = cleaned.lastIndexOf("Tura")
  return marker >= 0 ? cleaned.slice(marker).slice(0, 12000) : cleaned.slice(-12000)
}

async function startRealTuiBridge(workspace, gatewayUrl, agentDir, env) {
  const pty = tuiRequire("node-pty")
  let raw = ""
  let alive = true
  const term = pty.spawn(process.execPath, [tuiBin, "--gateway-url", gatewayUrl, "--cwd", workspace, "--color", "always"], {
    name: "xterm-256color",
    cols: 118,
    rows: 34,
    cwd: workspace,
    env: { ...process.env, ...env },
  })
  term.onData((data) => {
    raw += data
    if (raw.length > 2_000_000) raw = raw.slice(-1_000_000)
    writeFile(path.join(agentDir, "tui.raw.log"), raw)
    writeFile(path.join(agentDir, "tui.screen.txt"), latestTuiScreen(raw))
  })
  term.onExit(() => { alive = false })
  const writePty = async (text, slow = false) => {
    if (!slow) return term.write(text)
    for (const char of text) {
      term.write(char)
      await new Promise((resolve) => setTimeout(resolve, 1))
    }
  }
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1")
    if (req.method === "GET" && url.pathname === "/") {
      const body = realTuiBridgeHtml()
      res.writeHead(200, { "content-type": "text/html; charset=utf-8", "content-length": Buffer.byteLength(body) })
      return res.end(body)
    }
    if (req.method === "GET" && url.pathname === "/screen") {
      res.writeHead(200, { "content-type": "application/json" })
      return res.end(JSON.stringify({ screen: latestTuiScreen(raw), alive }))
    }
    if (req.method === "POST" && url.pathname === "/input") {
      const payload = await new Promise((resolve) => {
        let body = ""
        req.on("data", (chunk) => { body += chunk.toString() })
        req.on("end", () => resolve(body.trim() ? JSON.parse(body) : {}))
      })
      await writePty(String(payload.text || ""), Boolean(payload.slow))
      if (payload.submit) term.write("\r")
      res.writeHead(200, { "content-type": "application/json" })
      return res.end(JSON.stringify({ ok: true }))
    }
    res.writeHead(404)
    res.end()
  })
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      resolve({ server, term, isAlive: () => alive, url: `http://127.0.0.1:${address.port}` })
    })
  })
}

async function sendAndScreenshot(page, text, file) {
  mkdirp(path.dirname(file))
  const ext = path.extname(file)
  const stem = file.slice(0, -ext.length)
  const typed = String(text).replace(/\r?\n/g, " ")
  await page.screenshot({ path: `${stem}-01-before-input${ext}` })
  await page.getByLabel("TUI input").fill(typed)
  await page.screenshot({ path: `${stem}-02-after-fill${ext}` })
  await page.evaluate(async (value) => {
    const response = await fetch("/input", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ text: value, submit: true, slow: true }),
    })
    if (!response.ok) throw new Error(await response.text())
    document.querySelector("#input").value = ""
  }, typed)
  await page.waitForTimeout(900)
  await page.screenshot({ path: `${stem}-03-after-send${ext}` })
}

async function waitForTuiCompletion(page, expected, file) {
  const started = performance.now()
  await page.waitForTimeout(2500)
  await page.screenshot({ path: file.replace(/\.png$/, "-04-running.png") })
  const failure = await page.waitForFunction(
    ({ items, failurePatterns }) => {
      const text = document.querySelector("#screen")?.textContent || ""
      const lower = text.toLowerCase()
      const firstLine = text.split("\n").find((line) => line.includes("Tura")) || ""
      if (failurePatterns.some((pattern) => lower.includes(pattern))) return { done: true, ok: false, text }
      if (firstLine.includes(" idle ") && items.every((item) => lower.includes(String(item).toLowerCase()))) {
        return { done: true, ok: true, text }
      }
      return false
    },
    {
      items: expected,
      failurePatterns: [
        "provider runtime failed",
        "all providers failed",
        "you didn't provide an api key",
        "http status 401",
        "模型调用失败",
        "runtime failed",
      ],
    },
    { timeout: timeoutMs + 30_000 },
  ).then((handle) => handle.jsonValue())
  await page.waitForTimeout(800)
  await page.screenshot({ path: file.replace(/\.png$/, "-05-completed.png") })
  await page.screenshot({ path: file })
  if (!failure.ok) {
    throw new Error(`TUI task failed before completion:\n${String(failure.text || "").slice(-3000)}`)
  }
  return Math.round(performance.now() - started)
}

function snakePrompt(port) {
  return `Build a polished playable Snake game in this React workspace. Edit src/App.jsx and src/styles.css. It must show Snake or 贪吃蛇, score, start/restart, responsive board, food, moving snake, keyboard arrows, collision/game-over, and no mobile horizontal overflow. Run npm install if needed, then PORT=${port} npm run verify:snake. Fix failures until it passes and mention artifacts/snake-evaluator.png.`
}

async function evaluate(workspace, port) {
  const result = run(npmCmd, ["run", "verify:snake"], {
    cwd: workspace,
    timeoutMs: 180_000,
    shell: process.platform === "win32",
    env: { PORT: String(port) },
  })
  try {
    const match = result.stdout.match(/\{[\s\S]*\}\s*$/)
    return match ? JSON.parse(match[0]) : { pass: false, stdout: result.stdout, stderr: result.stderr, status: result.status }
  } catch {
    return { pass: false, stdout: result.stdout, stderr: result.stderr, status: result.status }
  }
}

async function main() {
  mkdirp(runRoot)
  const template = createFixture()
  runOk(npmCmd, ["install"], { cwd: template, timeoutMs: 180_000, shell: process.platform === "win32" })
  runOk(npxCmd, ["playwright", "install", "chromium"], { cwd: template, timeoutMs: 240_000, shell: process.platform === "win32" })
  const workspace = path.join(runRoot, "tui-fast-shll", "workspace")
  const agentDir = path.dirname(workspace)
  copyDir(template, workspace)
  const gatewayExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "gateway.exe" : "gateway")
  if (!fs.existsSync(gatewayExe)) runOk("cargo", ["build", "-p", "gateway", "--bin", "gateway"], { cwd: repoRoot, timeoutMs: 240_000 })
  runOk(npmCmd, ["run", "build"], { cwd: path.join(repoRoot, "apps", "tui"), timeoutMs: 120_000, shell: process.platform === "win32" })

  const gatewayPort = 46800 + Math.floor(Math.random() * 500)
  const taskPort = 4273
  const gatewayUrl = `http://127.0.0.1:${gatewayPort}`
  const env = {
    TURA_COMMAND_RUN_SHELL: "shell_command",
    TURA_COMMAND_RUN_STRICT_JSON: "0",
    COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
  }
  const gateway = startGatewayProcess(gatewayPort, agentDir, env)
  const ready = await waitForGateway(gatewayPort, gateway)
  assert(ready, `gateway did not become ready on ${gatewayPort}`)
  const bridge = await startRealTuiBridge(workspace, gatewayUrl, agentDir, env)
  const { chromium } = tuiRequire("playwright")
  const browser = await chromium.launch({ headless: true })
  const page = await browser.newPage({ viewport: { width: 1200, height: 860 } })
  let runError = null
  let durationMs = 0
  try {
    await page.goto(bridge.url, { waitUntil: "domcontentloaded" })
    await page.waitForFunction(() => (document.querySelector("#screen")?.textContent || "").includes("Tura"), undefined, { timeout: 60_000 })
    await page.screenshot({ path: path.join(agentDir, "tui-00-loaded.png") })
    await sendAndScreenshot(page, "/auth", path.join(agentDir, "tui-01-auth.png"))
    await sendAndScreenshot(page, "/login openai 0", path.join(agentDir, "tui-02-login-openai.png"))
    await sendAndScreenshot(page, "/logout openai", path.join(agentDir, "tui-03-logout-openai.png"))
    await sendAndScreenshot(page, "/models", path.join(agentDir, "tui-04-models.png"))
    await sendAndScreenshot(page, "/settings", path.join(agentDir, "tui-05-settings.png"))
    await sendAndScreenshot(page, "/sessions", path.join(agentDir, "tui-06-sessions.png"))
    await sendAndScreenshot(page, "/new", path.join(agentDir, "tui-07-new-session.png"))
    await sendAndScreenshot(page, "/sessions", path.join(agentDir, "tui-08-sessions-after-new.png"))
    await sendAndScreenshot(page, "/agent coding_agent_fast", path.join(agentDir, "tui-09-agent.png"))
    await sendAndScreenshot(page, `/model ${model}`, path.join(agentDir, "tui-10-model.png"))
    await sendAndScreenshot(page, `/config set model_reasoning_effort=${reasoning} service_tier=${serviceTier}`, path.join(agentDir, "tui-11-config.png"))
    await sendAndScreenshot(page, "/permissions", path.join(agentDir, "tui-12-permissions.png"))
    await sendAndScreenshot(page, "/diff", path.join(agentDir, "tui-13-diff.png"))
    await sendAndScreenshot(page, "/status", path.join(agentDir, "tui-14-status.png"))
    await sendAndScreenshot(page, "/chat", path.join(agentDir, "tui-15-chat.png"))
    const started = performance.now()
    await sendAndScreenshot(page, snakePrompt(taskPort), path.join(agentDir, "tui-16-snake-task.png"))
    durationMs = await waitForTuiCompletion(page, ["snake-evaluator.png"], path.join(agentDir, "tui-16-snake-task.png"))
    if (!durationMs) durationMs = Math.round(performance.now() - started)
    const screen = await page.getByLabel("TUI screen").innerText()
    writeFile(path.join(agentDir, "tui.final-screen.txt"), screen)
    await sendAndScreenshot(page, "/abort", path.join(agentDir, "tui-17-abort.png"))
    await sendAndScreenshot(page, "/quit", path.join(agentDir, "tui-18-quit.png"))
  } catch (error) {
    runError = String(error?.stack || error?.message || error)
  } finally {
    await browser.close()
    try { if (bridge.isAlive()) bridge.term.kill() } catch {}
    await new Promise((resolve) => bridge.server.close(resolve))
    try { killProcessTree(gateway.pid) } catch {}
  }
  const validation = await evaluate(workspace, taskPort)
  const summary = {
    ok: !runError && validation.pass,
    run_id: runId,
    run_root: runRoot,
    workspace,
    gateway_url: gatewayUrl,
    model,
    reasoning,
    duration_ms: durationMs,
    error: runError,
    validation,
  }
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") process.exitCode = 1
}

await main()
process.exit(process.exitCode ?? 0)
