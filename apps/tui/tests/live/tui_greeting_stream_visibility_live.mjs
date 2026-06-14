#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import fsp from "node:fs/promises";
import { createRequire } from "node:module";
import net from "node:net";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const tuiAppRoot = path.resolve(here, "..", "..");
const repoRoot = path.resolve(tuiAppRoot, "..", "..");
const tuiRequire = createRequire(path.join(tuiAppRoot, "package.json"));
const { chromium } = tuiRequire("playwright");
const exeSuffix = process.platform === "win32" ? ".exe" : "";
const debugDir = path.join(repoRoot, "target", "debug");
const gatewayExe = path.join(debugDir, `tura_gateway${exeSuffix}`);
const webTerminalBin = path.join(tuiAppRoot, "scripts", "web-terminal.mjs");
const runId = process.env.TURA_TUI_LIVE_RUN_ID || `greeting-stream-${timestamp()}`;
const runRoot = path.join(
  repoRoot,
  "apps",
  "tui",
  "test-results",
  "live",
  "greeting-stream",
  runId,
);
const workspace = path.join(runRoot, "workspace");
const logsDir = path.join(runRoot, "logs");
const screenshotsDir = path.join(runRoot, "screenshots");
const turaHome = path.join(runRoot, "tura-home");
const summaryPath = path.join(runRoot, "summary.json");
const timeoutMs = Number(process.env.TURA_TUI_LIVE_TIMEOUT_MS || 60_000);
const input = process.env.TURA_TUI_LIVE_GREETING_PROMPT || "你好，简单打个招呼。";
const instance = `greeting-${runId.replace(/[^\w-]/g, "-")}`;

await main();

async function main() {
  ensureLocalTuiBuild();
  ensureDebugGateway();
  await fsp.rm(runRoot, { recursive: true, force: true });
  await fsp.mkdir(workspace, { recursive: true });
  await fsp.mkdir(logsDir, { recursive: true });
  await fsp.mkdir(screenshotsDir, { recursive: true });
  await writeWorkspaceConfig();

  const gateway = await startGateway();
  const web = await startWebTerminal(gateway.url);
  let browser;
  let summary = {
    ok: false,
    input,
    observed_response: "",
    candidate_responses: [],
    final_screen_tail: "",
    screenshot: "",
    run_root: runRoot,
    workspace,
  };
  let page;
  try {
    browser = await chromium.launch({ headless: true });
    page = await browser.newPage({ viewport: { width: 1280, height: 760 } });
    const terminalUrl = `${web.url}/rich?instance=${encodeURIComponent(instance)}`;
    await page.goto(terminalUrl, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(
      () => /Enter to send|回车输入|Tura/i.test(document.body.innerText),
      null,
      { timeout: 45_000 },
    );

    await startRichTerminal(page);
    await submitRich(page, input);
    await waitForScreenText(page, input, 5_000);
    const observed = await waitForVisibleGreetingResponse(page);
    const screenshot = path.join(screenshotsDir, "final-visible-screen.png");
    await page.screenshot({ path: screenshot, fullPage: false });
    const finalScreen = await terminalText(page);
    await fsp.writeFile(path.join(logsDir, "final-screen.txt"), finalScreen);

    assert.ok(
      finalScreen.includes(input),
      `final TUI screen should still show the user input so the comparison is meaningful: ${input}`,
    );
    assert.ok(
      observed.response,
      `no visible assistant/runtime response found after input: ${input}`,
    );
    assert.notEqual(
      observed.response,
      input,
      "visible assistant/runtime response must not be replaced by the user input",
    );
    assert.doesNotMatch(observed.response, /^(doing|done|question)\s*:/iu);
    assert.doesNotMatch(observed.response, /task_status|command_type/iu);

    summary = {
      ok: true,
      input,
      observed_response: observed.response,
      candidate_responses: observed.candidates,
      final_screen_tail: tail(finalScreen),
      screenshot,
      run_root: runRoot,
      workspace,
    };
  } catch (error) {
    if (page) {
      try {
        const failureScreen = await terminalText(page);
        const screenshot = path.join(screenshotsDir, "failure-visible-screen.png");
        await page.screenshot({ path: screenshot, fullPage: false });
        await fsp.writeFile(path.join(logsDir, "failure-screen.txt"), failureScreen);
        summary = {
          ...summary,
          candidate_responses: extractVisibleResponses(failureScreen, input).candidates,
          final_screen_tail: tail(failureScreen),
          screenshot,
        };
      } catch {
        // Keep the original assertion or timeout error as the useful failure.
      }
    }
    summary = {
      ...summary,
      error: String(error?.stack || error?.message || error),
    };
    throw error;
  } finally {
    if (browser) await browser.close();
    await stopProcess(web.child);
    await stopProcess(gateway.child);
    await shutdownBackendDaemons();
    await fsp.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
  }
}

function ensureLocalTuiBuild() {
  runChecked(process.platform === "win32" ? "npm.cmd" : "npm", ["run", "build"], {
    cwd: tuiAppRoot,
    timeoutMs: 120_000,
    shell: process.platform === "win32",
  });
}

function ensureDebugGateway() {
  if (fs.existsSync(gatewayExe)) return;
  runChecked("cargo", ["build", "-p", "gateway", "--bin", "tura_gateway"], {
    cwd: repoRoot,
    timeoutMs: 180_000,
  });
}

function runChecked(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 120_000,
    windowsHide: true,
    shell: Boolean(options.shell),
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status || result.signal || result.error?.message || "unknown"}\nSTDOUT:\n${result.stdout || ""}\nSTDERR:\n${result.stderr || ""}`,
    );
  }
}

async function writeWorkspaceConfig() {
  const configDir = path.join(workspace, ".tura");
  await fsp.mkdir(configDir, { recursive: true });
  await fsp.writeFile(
    path.join(configDir, "config.conf"),
    [
      `model=${process.env.TURA_TUI_LIVE_MODEL || "codex/gpt-5.5"}`,
      "active_provider=codex",
      "active_model=gpt-5.5",
      `active_agent=${process.env.TURA_TUI_LIVE_AGENT || "fast"}`,
      "session_type=coding",
      `model_variant=${process.env.TURA_TUI_LIVE_MODEL_VARIANT || "medium"}`,
      "model_acceleration_enabled=true",
      "kill_processes_on_start=false",
      "validator_enabled=false",
      "force_planning=false",
      "",
    ].join("\n"),
  );
}

async function startGateway() {
  const port = await freePort();
  const child = spawn(gatewayExe, [], {
    cwd: workspace,
    env: {
      ...process.env,
      PORT: String(port),
      TURA_GATEWAY_PORT: String(port),
      TURA_GATEWAY_URL: `http://127.0.0.1:${port}`,
      TURA_CWD: workspace,
      TURA_HOME: turaHome,
      TURA_PROJECT_ROOT: repoRoot,
      TURA_PROVIDER_CONFIG:
        process.env.TURA_PROVIDER_CONFIG ||
        path.join(repoRoot, "crates", "provider", "config", "provider_config.json"),
      FORCE_COLOR: "0",
      NO_COLOR: "1",
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout?.pipe(fs.createWriteStream(path.join(logsDir, "gateway.stdout.log")));
  child.stderr?.pipe(fs.createWriteStream(path.join(logsDir, "gateway.stderr.log")));
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/global/health`, child, 60_000);
  return { child, url };
}

async function startWebTerminal(gatewayUrl) {
  const port = await freePort();
  const child = spawn(process.execPath, [webTerminalBin], {
    cwd: tuiAppRoot,
    env: {
      ...process.env,
      PORT: String(port),
      TURA_GATEWAY_URL: gatewayUrl,
      TURA_CWD: workspace,
      TURA_HOME: turaHome,
      TURA_PROJECT_ROOT: repoRoot,
      TURA_TUI_DISABLE_MOUSE: "1",
      FORCE_COLOR: "1",
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout?.pipe(fs.createWriteStream(path.join(logsDir, "web-terminal.stdout.log")));
  child.stderr?.pipe(fs.createWriteStream(path.join(logsDir, "web-terminal.stderr.log")));
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/`, child, 60_000);
  return { child, url };
}

async function startRichTerminal(page) {
  await page.waitForFunction(() => typeof window.__turaFit === "function", null, {
    timeout: 10_000,
  });
  await page.evaluate(() => window.__turaFit());
  await focusTerminal(page);
  await waitForScreenText(page, /Enter to send|回车输入/u, 20_000);
}

async function submitRich(page, value) {
  await focusTerminal(page);
  await page.keyboard.type(value, { delay: 10 });
  await delay(150);
  await page.keyboard.press("Enter");
}

async function focusTerminal(page) {
  const textarea = page.locator(".xterm-helper-textarea").first();
  if ((await textarea.count()) > 0) {
    await textarea.focus({ timeout: 5_000 });
    return;
  }
  await page.locator("#terminal").click({ timeout: 5_000 });
}

async function waitForScreenText(page, expected, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const screen = await terminalText(page);
    if (typeof expected === "string" ? screen.includes(expected) : expected.test(screen)) {
      return screen;
    }
    await delay(250);
  }
  throw new Error(`timed out waiting for TUI screen text ${String(expected)}`);
}

async function waitForVisibleGreetingResponse(page) {
  const deadline = Date.now() + Math.min(timeoutMs, 45_000);
  let latest = { response: "", candidates: [], screen: "" };
  let sawBusy = false;
  while (Date.now() < deadline) {
    const screen = await terminalText(page);
    sawBusy ||= /忙碌|busy/i.test(screen);
    const extracted = extractVisibleResponses(screen, input);
    if (extracted.candidates.length) {
      latest = {
        response: extracted.candidates[0],
        candidates: extracted.candidates,
        screen,
      };
    }
    if (latest.response) {
      await delay(1_000);
      const stableScreen = await terminalText(page);
      const stable = extractVisibleResponses(stableScreen, input);
      return {
        response: stable.candidates[0] ?? latest.response,
        candidates: stable.candidates.length ? stable.candidates : latest.candidates,
        screen: stableScreen,
      };
    }
    if (sawBusy && /空闲|idle/i.test(screen)) break;
    await delay(500);
  }
  throw new Error(
    `timed out waiting for visible greeting response after ${Math.min(timeoutMs, 45_000)}ms`,
  );
}

async function terminalText(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll(".xterm-rows > div")]
      .map((node) => node.textContent ?? "")
      .join("\n"),
  );
}

function extractVisibleResponses(screen, userInput) {
  const lines = screen.split(/\r?\n/u).map(cleanLine).filter(Boolean);
  const inputIndex = lines.findIndex((line) => line.includes(userInput));
  const afterInput = inputIndex >= 0 ? lines.slice(inputIndex + 1) : lines;
  const candidates = afterInput.filter((line) => isVisibleResponseLine(line, userInput));
  return { lines, inputIndex, candidates };
}

function cleanLine(value) {
  return value
    .replace(/\u00a0/g, " ")
    .replace(/[│┃]/gu, " ")
    .replace(/\s+/gu, " ")
    .trim()
    .replace(/^[▏>◆◇└├┌┐┘┬┴┼─✓✕x #\d:.-]+/u, "")
    .trim();
}

function isVisibleResponseLine(line, userInput) {
  if (!line || line === userInput || line.includes(userInput)) return false;
  if (/^session-\d+/iu.test(line)) return false;
  if (/^(dev\s+模式|provider\s+llm|llm\s+log)/iu.test(line)) return false;
  if (/^(enter to send|回车输入|tura|oc \||gateway|workspace|directory|目录)/iu.test(line)) {
    return false;
  }
  if (/(tokens|codex\/|openai\/|anthropic\/|优先|priority|空闲|忙碌|idle|busy)/iu.test(line)) {
    return false;
  }
  if (/thinking\s+\d+s?/iu.test(line)) return false;
  if (/(command_run|commands?:|命令:|事件流|unable to connect|mano failed)/iu.test(line)) {
    return false;
  }
  if (/^(doing|done|question)\s*:/iu.test(line)) return false;
  if (/(task_status|command_type)/iu.test(line)) return false;
  if (/^\{.*"status"\s*:/iu.test(line)) return false;
  return true;
}

async function waitForUrl(url, child, timeout) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    if (child?.exitCode !== null) {
      throw new Error(`${url} exited before readiness with ${child.exitCode}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) return response;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw lastError || new Error(`Timed out waiting for ${url}`);
}

async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close(() => resolve(port));
    });
    server.on("error", reject);
  });
}

async function stopProcess(child) {
  if (!child || child.killed || child.exitCode !== null) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
    return;
  }
  try {
    child.kill("SIGTERM");
  } catch {
    // Best-effort process cleanup.
  }
  await Promise.race([new Promise((resolve) => child.once("exit", resolve)), delay(2_000)]);
  if (child.exitCode === null) {
    try {
      child.kill("SIGKILL");
    } catch {
      // Best-effort process cleanup.
    }
  }
}

async function shutdownBackendDaemons() {
  const routerAddrPath = path.join(turaHome, "db", "session_log", "router.addr");
  const endpoint = await readJson(routerAddrPath);
  if (!endpoint?.addr) return;
  try {
    await callRouter(endpoint.addr, {
      request_id: "tui-greeting-live-cleanup",
      kind: "call",
      method: "execution.shutdown",
      payload: {},
    });
  } catch {
    // The parent process cleanup above is still authoritative for this live run.
  }
}

async function callRouter(addr, payload) {
  const parsed = parseHostPort(addr);
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(parsed.port, parsed.host);
    let raw = "";
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("router cleanup timeout"));
    }, 10_000);
    socket.on("connect", () => socket.write(`${JSON.stringify(payload)}\n`));
    socket.on("data", (chunk) => {
      raw += chunk.toString();
      if (raw.includes("\n")) {
        clearTimeout(timer);
        socket.end();
        resolve(JSON.parse(raw.trim()));
      }
    });
    socket.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

async function readJson(file) {
  try {
    return JSON.parse(await fsp.readFile(file, "utf8"));
  } catch {
    return undefined;
  }
}

function parseHostPort(addr) {
  const parsed = new URL(`tcp://${addr}`);
  return { host: parsed.hostname, port: Number(parsed.port) };
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function tail(value) {
  return value.slice(-4000);
}

function timestamp() {
  return new Date().toISOString().replace(/[-:]/g, "").replace(/\..+$/u, "").replace("T", "-");
}
