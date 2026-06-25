#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import { gatewayBinaryPath, gatewayTestEnv } from "./helpers/tui_test_paths.mjs";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..", "..");
const appRoot = path.join(repoRoot, "apps", "tui");
const runId = process.env.TUI_REAL_SNAKE_RUN_ID || `tui-real-snake-${Date.now()}`;
const runRoot = path.join(repoRoot, "apps", "tui", "test-results", "tui-real-gateway-snake", runId);
const workspace = path.join(runRoot, "workspace");
const screenshotsDir = path.join(runRoot, "screenshots");
const providerLogRoot =
  process.env.TUI_REAL_SNAKE_PROVIDER_LOG_ROOT || path.join(runRoot, "logs", "provider");
const summaryPath = path.join(runRoot, "summary.json");
const nodeBin = process.execPath;
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const gatewayExe = gatewayBinaryPath();
const tuiBin = path.join(appRoot, "dist", "index.js");
const webTerminalBin = path.join(appRoot, "scripts", "web-terminal.mjs");
const tuiRequire = createRequire(path.join(appRoot, "package.json"));

const model = process.env.TUI_REAL_SNAKE_MODEL || "codex/gpt-5.5";
const agent = process.env.TUI_REAL_SNAKE_AGENT || "fast";
const timeoutMs = Number(process.env.TUI_REAL_SNAKE_TIMEOUT_MS || 600_000);
const checks = [];

function record(name, ok, details = {}) {
  checks.push({ name, ok, ...details });
  if (!ok) throw new Error(`${name} failed: ${JSON.stringify(details)}`);
}

function freePort() {
  return 20_000 + Math.floor(Math.random() * 20_000);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: 128 * 1024 * 1024,
    shell: Boolean(options.shell),
    windowsHide: true,
  });
  return {
    status: result.status,
    signal: result.signal,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    error: result.error ? String(result.error.message || result.error) : null,
  };
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options);
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status || result.signal}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`,
    );
  }
  return result;
}

function startProcess(command, args, options = {}) {
  const child = spawn(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let stdout = "";
  let stderr = "";
  child.stdout?.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr?.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  child.logs = () => ({ stdout, stderr });
  return child;
}

async function stopProcess(child) {
  if (!child || child.killed || child.exitCode !== null) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
  } else {
    child.kill("SIGTERM");
  }
  await new Promise((resolve) => child.once("exit", resolve));
}

async function waitForUrl(url, deadlineMs = 45_000) {
  const deadline = Date.now() + deadlineMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return await response.json().catch(() => ({}));
    } catch {
      // Retry while the server starts.
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`timed out waiting for ${url}`);
}

async function listProviderLogs() {
  const entries = [];
  async function walk(dir) {
    let items = [];
    try {
      items = await fs.readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const item of items) {
      const fullPath = path.join(dir, item.name);
      if (item.isDirectory()) await walk(fullPath);
      else if (item.isFile() && item.name.endsWith(".json")) {
        const stat = await fs.stat(fullPath);
        entries.push({ path: fullPath, size: stat.size, mtimeMs: stat.mtimeMs });
      }
    }
  }
  await walk(providerLogRoot);
  return entries.sort((left, right) => right.mtimeMs - left.mtimeMs);
}

function providerLogKey(log) {
  return `${log.path}:${log.size}:${Math.round(log.mtimeMs)}`;
}

async function writeSnakeFixture() {
  await fs.mkdir(path.join(workspace, "tools"), { recursive: true });
  await fs.writeFile(
    path.join(workspace, "snake.html"),
    `<!doctype html><html><head><meta charset="utf-8"><title>Networked Snake</title><style>
html,body{margin:0;height:100%;background:#0e1111;color:#f3f6f2;font-family:system-ui,sans-serif;overflow:hidden}.wrap{min-height:100%;display:grid;place-items:center;padding:16px;box-sizing:border-box}.hud{display:flex;justify-content:space-between;gap:12px;align-items:center;margin-bottom:12px}.game{width:min(82vmin,520px);max-width:100%;}.board{display:grid;grid-template-columns:repeat(16,1fr);aspect-ratio:1;border:2px solid #778071;background:#141a17}.cell{border:1px solid #202820}.snake{background:#9fd36f}.food{background:#f2c15b}.head{background:#e7f7d0}.status{font-size:14px;color:#b7c0b0}button{font:inherit;background:#f3f6f2;color:#111;border:0;padding:8px 12px}</style></head><body><main class="wrap"><section class="game"><div class="hud"><strong>Snake demo</strong><span id="score">Score: 0</span><button id="restart">Restart</button></div><div id="board" class="board" aria-label="snake board"></div><p class="status">Use ArrowRight, ArrowDown, ArrowLeft, ArrowUp. Restart is always visible.</p></section></main><script>
const size=16,board=document.getElementById('board'),scoreEl=document.getElementById('score');let snake,dir,food,score,timer;function draw(){board.innerHTML='';for(let i=0;i<size*size;i++){const c=document.createElement('div');c.className='cell';const x=i%size,y=Math.floor(i/size);if(food.x===x&&food.y===y)c.classList.add('food');const s=snake.findIndex(p=>p.x===x&&p.y===y);if(s>=0)c.classList.add(s===0?'head':'snake');board.append(c)}scoreEl.textContent='Score: '+score}function reset(){snake=[{x:7,y:7},{x:6,y:7},{x:5,y:7}];dir={x:1,y:0};food={x:11,y:7};score=0;clearInterval(timer);timer=setInterval(step,240);draw()}function step(){const h={x:(snake[0].x+dir.x+size)%size,y:(snake[0].y+dir.y+size)%size};snake.unshift(h);if(h.x===food.x&&h.y===food.y){score++;food={x:(food.x+5)%size,y:(food.y+3)%size}}else snake.pop();draw()}addEventListener('keydown',e=>{if(e.key==='ArrowRight')dir={x:1,y:0};if(e.key==='ArrowLeft')dir={x:-1,y:0};if(e.key==='ArrowDown')dir={x:0,y:1};if(e.key==='ArrowUp')dir={x:0,y:-1}});document.getElementById('restart').onclick=reset;reset();</script></body></html>`,
  );
  await fs.writeFile(
    path.join(workspace, "tools", "snake_playwright.mjs"),
    `import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const require = createRequire(${JSON.stringify(path.join(appRoot, "package.json"))});
const { chromium } = require('playwright');
const out=path.resolve('playwright-screenshots');
await import('node:fs/promises').then(fs=>fs.mkdir(out,{recursive:true}));
const browser=await chromium.launch({headless:true});
for (const [name, viewport] of [['desktop',{width:1280,height:720}], ['mobile',{width:390,height:720}]]) {
  const page=await browser.newPage({viewport});
  await page.goto(pathToFileURL(path.resolve('snake.html')).href);
  await page.keyboard.press('ArrowRight');
  await page.waitForTimeout(260);
  await page.keyboard.press('ArrowDown');
  await page.waitForTimeout(260);
  await page.screenshot({path:path.join(out, name+'.png')});
  assert.match(await page.locator('#score').innerText(), /Score: /);
  await page.locator('#restart').click();
  const overflow=await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1);
  assert.equal(overflow, false, name+' has no horizontal overflow');
  await page.close();
}
await browser.close();
console.log(JSON.stringify({ok:true, screenshots:['playwright-screenshots/desktop.png','playwright-screenshots/mobile.png']}));
`,
  );
}

async function startGateway() {
  const external = process.env.TUI_REAL_SNAKE_GATEWAY_URL;
  if (external) {
    const url = external.replace(/\/+$/u, "");
    return { url, child: undefined, health: await waitForUrl(`${url}/global/health`, 60_000) };
  }
  const port = freePort();
  const child = startProcess(gatewayExe, [], {
    env: {
      ...gatewayTestEnv(runRoot, workspace, port),
      LOG_PATH: providerLogRoot,
      TURA_DEBUG_RUNTIME: "1",
      TURA_RUNTIME_WORKER_STDERR_LOG: path.join(runRoot, "runtime-worker.stderr.log"),
      TURA_ROUTER_STDERR_LOG: path.join(runRoot, "router.stderr.log"),
    },
  });
  const url = `http://127.0.0.1:${port}`;
  return { url, child, health: await waitForUrl(`${url}/global/health`, 60_000) };
}

async function runNetworkedSnakePrompt(gatewayUrl) {
  const before = await listProviderLogs();
  const beforeKeys = new Set(before.map(providerLogKey));
  const prompt = [
    "Networked TUI snake Playwright verification.",
    "This is a real gateway/provider task, not mock mode.",
    "Confirm the local Snake app contract in concise Markdown.",
    "Mention `snake.html`, `node tools/snake_playwright.mjs`, `desktop.png`, `mobile.png`, ArrowRight, ArrowDown, score, restart, and no horizontal overflow.",
  ].join("\n");
  const result = run(
    nodeBin,
    [
      tuiBin,
      "--gateway-url",
      gatewayUrl,
      "--cwd",
      workspace,
      "--json",
      "run",
      "--no-stream",
      "--timeout",
      String(Math.ceil(timeoutMs / 1000)),
      "--model",
      model,
      "--agent",
      agent,
      "--model-variant",
      "low",
      "--priority",
      prompt,
    ],
    { timeoutMs: timeoutMs + 30_000 },
  );
  await fs.writeFile(path.join(runRoot, "tui-run.stdout.log"), result.stdout);
  await fs.writeFile(path.join(runRoot, "tui-run.stderr.log"), result.stderr);
  const parsed = result.stdout.trim() ? JSON.parse(result.stdout) : {};
  const after = await listProviderLogs();
  const newLogs = after.filter((log) => !beforeKeys.has(providerLogKey(log)));
  await fs.writeFile(path.join(runRoot, "provider-logs.json"), JSON.stringify(newLogs, null, 2));
  record("real-networked-tui-run", result.status === 0 && parsed.status === "completed", {
    status: result.status,
    sessionID: parsed.sessionID,
    finalText: String(parsed.finalText || "").slice(0, 500),
  });
  record("provider-log-created", newLogs.length > 0, { count: newLogs.length });
  return parsed.sessionID;
}

async function startWebTerminal(gatewayUrl) {
  const port = freePort();
  const child = startProcess(nodeBin, [webTerminalBin], {
    env: { PORT: String(port), TURA_GATEWAY_URL: gatewayUrl, TURA_CWD: workspace },
  });
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/`, 30_000).catch(async () => {
    const response = await fetch(`${url}/`);
    if (!response.ok) throw new Error(`web terminal returned ${response.status}`);
  });
  return { child, url };
}

async function captureTui(gatewayUrl, sessionID) {
  const { chromium } = tuiRequire("playwright");
  const web = await startWebTerminal(gatewayUrl);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  const screenshots = [];
  async function shot(name) {
    const file = path.join(screenshotsDir, `${name}.png`);
    await page.screenshot({ path: file, fullPage: false });
    screenshots.push(file);
  }
  async function send(input) {
    await page.evaluate(async (value) => globalThis.__turaSendInput(value), input);
    await page.waitForTimeout(350);
  }
  async function command(input) {
    await send("\u001b");
    await send(`${input}\r`);
    await page.waitForTimeout(1300);
  }
  try {
    await page.goto(`${web.url}/`, { waitUntil: "domcontentloaded" });
    await shot("00-web-terminal-index");
    await page.goto(`${web.url}/rich`, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(1600);
    await command(`/resume ${sessionID}`);
    await page.waitForFunction(
      () =>
        /snake\.html|snake_playwright\.mjs|desktop\.png|mobile\.png|Snake/i.test(
          document.body.innerText,
        ),
      null,
      { timeout: 30_000 },
    );
    await shot("01-rich-resumed-real-snake-task");
    for (const panel of ["/sessions", "/models", "/settings", "/chat"]) {
      await command(panel);
      await shot(`02-panel-${panel.slice(1)}`);
    }
    await page.setViewportSize({ width: 390, height: 720 });
    await page.waitForTimeout(800);
    await shot("03-mobile-chat-real-snake-task");
    record("many-tui-screenshots", screenshots.length >= 7, { count: screenshots.length });
    return screenshots;
  } finally {
    await browser.close().catch(() => undefined);
    const logs = web.child.logs();
    await fs.writeFile(path.join(runRoot, "web-terminal.stdout.log"), logs.stdout);
    await fs.writeFile(path.join(runRoot, "web-terminal.stderr.log"), logs.stderr);
    await stopProcess(web.child);
  }
}

async function main() {
  await fs.rm(runRoot, { recursive: true, force: true });
  await fs.mkdir(screenshotsDir, { recursive: true });
  await writeSnakeFixture();
  runOk(npmCmd, ["run", "build"], {
    cwd: appRoot,
    timeoutMs: 120_000,
    shell: process.platform === "win32",
  });
  // The gateway binary is only required when we have to spawn one ourselves.
  if (!process.env.TUI_REAL_SNAKE_GATEWAY_URL) {
    await fs.access(gatewayExe);
  }

  const snakeResult = runOk(nodeBin, [path.join("tools", "snake_playwright.mjs")], {
    cwd: workspace,
    timeoutMs: 120_000,
  });
  await fs.writeFile(path.join(runRoot, "snake-playwright.stdout.log"), snakeResult.stdout);
  record("local-snake-playwright-passes", /"ok":true/.test(snakeResult.stdout), {
    stdout: snakeResult.stdout,
  });

  const gateway = await startGateway();
  try {
    record("real-gateway-health", gateway.health?.healthy === true, { health: gateway.health });
    const sessionID = await runNetworkedSnakePrompt(gateway.url);
    record("session-created", Boolean(sessionID), { sessionID });
    const tuiScreenshots = await captureTui(gateway.url, sessionID);
    const summary = {
      ok: checks.every((check) => check.ok),
      runRoot,
      workspace,
      sessionID,
      model,
      agent,
      checks,
      tuiScreenshots,
    };
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
    if (!summary.ok) process.exitCode = 1;
  } finally {
    if (gateway.child) {
      const logs = gateway.child.logs();
      await fs.writeFile(path.join(runRoot, "gateway.stdout.log"), logs.stdout);
      await fs.writeFile(path.join(runRoot, "gateway.stderr.log"), logs.stderr);
      await stopProcess(gateway.child);
    }
  }
}

main().catch(async (error) => {
  const summary = {
    ok: false,
    runRoot,
    workspace,
    error: String(error.stack || error.message || error),
    checks,
  };
  await fs.mkdir(runRoot, { recursive: true }).catch(() => undefined);
  await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2)).catch(() => undefined);
  console.error(error);
  process.exitCode = 1;
});
