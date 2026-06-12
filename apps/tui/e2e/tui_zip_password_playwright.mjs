#!/usr/bin/env node
import { createRequire } from "node:module";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const appRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const runRoot = path.join(repoRoot, "target", "tui-zip-password-playwright", `${Date.now()}`);
const workspace = path.join(runRoot, "workspace");
const screenshotsDir = path.join(runRoot, "screenshots");
const summaryPath = path.join(runRoot, "summary.json");
const providerLogRoot = path.join(runRoot, "provider-log");
const turaHome = path.join(runRoot, "tura-home");
const nodeRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium } = nodeRequire("playwright");
const timeoutMs = Number(process.env.TUI_ZIP_PASSWORD_TIMEOUT_MS || 7 * 60_000);
const sentinel = process.env.TUI_ZIP_PASSWORD_SENTINEL || "TUI_ZIP_PASSWORD_REAL_OK";
const checks = [];
const screenshots = [];
let gateway;
let web;

const exeSuffix = process.platform === "win32" ? ".exe" : "";
const gatewayExe = path.join(repoRoot, "target", "debug", `tura_gateway${exeSuffix}`);

function record(name, ok, details = {}) {
  checks.push({ name, ok, ...details });
  if (!ok) throw new Error(`${name} failed: ${JSON.stringify(details)}`);
}

function freePort() {
  return 24_000 + Math.floor(Math.random() * 20_000);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
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
  if (!child || child.exitCode !== null || child.killed) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
    await delay(100);
    return;
  }
  child.kill("SIGTERM");
  await Promise.race([new Promise((resolve) => child.once("exit", resolve)), delay(2000)]);
  if (child.exitCode === null) child.kill("SIGKILL");
}

async function waitForUrl(url, deadlineMs = 30_000, child, label = url) {
  const deadline = Date.now() + deadlineMs;
  let lastError;
  while (Date.now() < deadline) {
    if (child && child.exitCode !== null) {
      const logs = child.logs?.() ?? { stdout: "", stderr: "" };
      throw new Error(
        `${label} exited before readiness with ${child.exitCode}\nSTDOUT:\n${logs.stdout.slice(-2000)}\nSTDERR:\n${logs.stderr.slice(-2000)}`,
      );
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
  if (child && child.exitCode === null) await stopProcess(child);
  throw lastError ?? new Error(`timed out waiting for ${url}`);
}

async function requestJson(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: {
      "content-type": "application/json",
      "x-opencode-directory": workspace,
      ...(options.headers || {}),
    },
  });
  const text = await response.text();
  if (!response.ok)
    throw new Error(`${options.method || "GET"} ${url} returned ${response.status}: ${text}`);
  return text.trim() ? JSON.parse(text) : undefined;
}

async function terminalLines(page) {
  return page.evaluate(() => {
    const buffer = window.__turaTerminal?.buffer.active;
    if (!buffer) return [];
    const lines = [];
    for (let index = 0; index < buffer.length; index += 1) {
      lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
    }
    return lines;
  });
}

async function terminalText(page) {
  return (await terminalLines(page)).join("\n");
}

async function send(page, data) {
  await page.evaluate(async (value) => window.__turaSendInput(value), data);
  await page.waitForTimeout(180);
}

async function submit(page, text) {
  await send(page, text);
  await send(page, "\r");
}

async function capture(page, name) {
  await page.waitForTimeout(300);
  const file = path.join(screenshotsDir, `${name}.png`);
  await page.screenshot({ path: file, fullPage: false });
  const text = await terminalText(page);
  const lines = text.split("\n");
  const metrics = await page.evaluate(() => {
    const body = document.body;
    const shell = document.querySelector(".shell");
    const terminal = document.querySelector("#terminal");
    const viewport = document.querySelector(".xterm-viewport");
    const rowNodes = [...document.querySelectorAll(".xterm-rows > div")];
    return {
      bodyClientWidth: body.clientWidth,
      bodyScrollWidth: body.scrollWidth,
      shellClientWidth: shell?.clientWidth ?? 0,
      shellScrollWidth: shell?.scrollWidth ?? 0,
      terminalClientWidth: terminal?.clientWidth ?? 0,
      terminalScrollWidth: terminal?.scrollWidth ?? 0,
      viewportClientWidth: viewport?.clientWidth ?? 0,
      viewportScrollWidth: viewport?.scrollWidth ?? 0,
      visibleRows: rowNodes.length,
      nonEmptyRows: rowNodes.filter((node) => (node.textContent ?? "").trim()).length,
    };
  });
  const analysis = {
    name,
    path: file,
    hasComposerHint: /Enter to send/.test(text),
    hasPromptEcho: /zip-password-finder|PYTHON_PORT_TASK|zip password/i.test(text),
    hasSentinel: text.includes(sentinel),
    hasBusyOrThinking: /thinking|busy|working|tokens/i.test(text),
    hasCommandRun: /command_run|Commands:|Get-Content|PYTHON_PORT_TASK/i.test(text),
    hasReconnectNotice: /reconnecting|disconnected|Gateway unavailable|ECONNREFUSED/i.test(text),
    hasRawControlLeak: /\x1b|\[2K|\[K/.test(text),
    overflow:
      metrics.bodyScrollWidth > metrics.bodyClientWidth + 2 ||
      metrics.shellScrollWidth > metrics.shellClientWidth + 2 ||
      metrics.terminalScrollWidth > metrics.terminalClientWidth + 2 ||
      metrics.viewportScrollWidth > metrics.viewportClientWidth + 2,
    tail: lines.slice(-12),
    metrics,
  };
  screenshots.push(file);
  return analysis;
}

async function prepareWorkspace() {
  await fs.rm(runRoot, { recursive: true, force: true });
  await fs.mkdir(screenshotsDir, { recursive: true });
  await fs.mkdir(workspace, { recursive: true });
  await fs.writeFile(
    path.join(workspace, "PYTHON_PORT_TASK.md"),
    [
      "# zip-password-finder CLI refactor task",
      "",
      "You are exercising the TUI real gateway path for a zip-password CLI refactor task.",
      "The target command is zip_password_refactor/bin/zip-password-finder.mjs.",
      "The refactor must support dictionary search, brute-force search, --json output, --help output, and argument validation.",
      "The acceptance command is node acceptance/zip_password_cli_acceptance.mjs.",
      "For this Playwright business smoke, prove the real LLM/gateway/tool loop can inspect the CLI refactor task:",
      "1. inspect this file with a command_run shell command,",
      `2. then include the marker ${sentinel} and the phrase zip-password-finder CLI refactor in the final answer.`,
      "Do not install packages and do not make network calls for this smoke flow.",
      "",
    ].join("\n"),
  );
}

function buildPrompt() {
  return [
    "This is the tui-zip-password-playwright real gateway CLI refactor smoke.",
    "Use command_run once to read PYTHON_PORT_TASK.md in the current workspace.",
    `Then include ${sentinel} and zip-password-finder CLI refactor in the final answer.`,
    "Keep the answer short.",
  ].join(" ");
}

async function startRealGateway() {
  const port = freePort();
  const child = startProcess(gatewayExe, [], {
    env: {
      PORT: String(port),
      TURA_GATEWAY_PORT: String(port),
      TURA_CWD: workspace,
      TURA_HOME: turaHome,
      TURA_PROJECT_ROOT: repoRoot,
      TURA_PROVIDER_CONFIG: path.join(
        repoRoot,
        "crates",
        "provider",
        "config",
        "provider_config.json",
      ),
      LOG_PATH: providerLogRoot,
      TURA_DEBUG_RUNTIME: "1",
      TURA_RUNTIME_WORKER_STDERR_LOG: path.join(runRoot, "runtime-worker.stderr.log"),
      TURA_ROUTER_STDERR_LOG: path.join(runRoot, "router.stderr.log"),
    },
  });
  const url = `http://127.0.0.1:${port}`;
  const healthResponse = await waitForUrl(`${url}/global/health`, 45_000, child, "gateway");
  const health = await healthResponse.json().catch(() => ({}));
  return { child, url, health };
}

async function startWebTerminal(gatewayUrl) {
  const port = freePort();
  const child = startProcess(
    process.execPath,
    [path.join(appRoot, "scripts", "web-terminal.mjs")],
    {
      cwd: appRoot,
      env: {
        PORT: String(port),
        TURA_GATEWAY_URL: gatewayUrl,
        TURA_CWD: workspace,
        FORCE_COLOR: "1",
      },
    },
  );
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/`, 30_000, child, "web terminal");
  return { child, url };
}

async function createSeedSession(gatewayUrl) {
  const config = await requestJson(
    `${gatewayUrl}/session/config?directory=${encodeURIComponent(workspace)}`,
  ).catch(() => ({}));
  const payload = {
    directory: workspace,
    agent:
      process.env.TUI_ZIP_PASSWORD_AGENT ||
      config.active_agent ||
      config.agent ||
      "thinking-planning",
    model: process.env.TUI_ZIP_PASSWORD_MODEL || config.active_model || config.model || undefined,
    model_variant: process.env.TUI_ZIP_PASSWORD_VARIANT || config.model_variant || undefined,
    model_acceleration_enabled:
      process.env.TUI_ZIP_PASSWORD_PRIORITY === "0"
        ? false
        : (config.model_acceleration_enabled ?? true),
  };
  const session = await requestJson(`${gatewayUrl}/session`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
  return { session, config, payload };
}

async function listProviderLogs(root) {
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
  await walk(root);
  return entries.sort((left, right) => right.mtimeMs - left.mtimeMs);
}

function providerLogKey(log) {
  return `${log.path}:${log.size}:${Math.round(log.mtimeMs)}`;
}

function messageRole(message) {
  return message.role || message.info?.role || "";
}

function messageParts(message) {
  return message.parts || message.info?.parts || [];
}

function messageText(message) {
  return messageParts(message)
    .map((part) => part.text ?? part.content ?? "")
    .join("");
}

async function waitForLlmResult(gatewayUrl, sessionID, beforeProviderKeys) {
  const deadline = Date.now() + timeoutMs;
  let lastMessages = [];
  while (Date.now() < deadline) {
    lastMessages = await requestJson(
      `${gatewayUrl}/session/${encodeURIComponent(sessionID)}/message`,
    ).catch(() => []);
    const assistantText = Array.isArray(lastMessages)
      ? lastMessages
          .filter((message) => messageRole(message) === "assistant")
          .map(messageText)
          .join("\n")
      : "";
    const providerLogs = await listProviderLogs(providerLogRoot);
    const newProviderLogs = providerLogs.filter(
      (log) => !beforeProviderKeys.has(providerLogKey(log)),
    );
    if (assistantText.includes(sentinel))
      return { messages: lastMessages, assistantText, newProviderLogs };
    await delay(1500);
  }
  throw new Error(
    `timed out waiting for ${sentinel}; last messages: ${JSON.stringify(lastMessages).slice(-4000)}`,
  );
}

function serverAnalysis(gatewayLogs, webLogs, captures, providerLogs) {
  const combined = `${gatewayLogs.stdout}\n${gatewayLogs.stderr}\n${webLogs.stdout}\n${webLogs.stderr}`;
  const issuePatterns = [
    /panic/i,
    /thread .* panicked/i,
    /address already in use/i,
    /ECONNREFUSED/i,
    /unhandled/i,
    /timed out/i,
    /failed to bind/i,
  ];
  const matchedIssues = issuePatterns
    .filter((pattern) => pattern.test(combined))
    .map((pattern) => pattern.source);
  return {
    gateway_stdout_tail: gatewayLogs.stdout.slice(-3000),
    gateway_stderr_tail: gatewayLogs.stderr.slice(-3000),
    web_stdout_tail: webLogs.stdout.slice(-1500),
    web_stderr_tail: webLogs.stderr.slice(-1500),
    provider_log_count: providerLogs.length,
    ui_reconnect_notices: captures
      .filter((capture) => capture.hasReconnectNotice)
      .map((capture) => capture.name),
    ui_overflow_screens: captures
      .filter((capture) => capture.overflow)
      .map((capture) => capture.name),
    ui_raw_control_leaks: captures
      .filter((capture) => capture.hasRawControlLeak)
      .map((capture) => capture.name),
    matched_issue_patterns: matchedIssues,
    has_potential_server_issue:
      matchedIssues.length > 0 || captures.some((capture) => capture.hasReconnectNotice),
  };
}

async function main() {
  await prepareWorkspace();
  record("gateway-binary-present", existsSync(gatewayExe), { gatewayExe });

  const build = spawnSync(
    process.platform === "win32" ? "cmd.exe" : "npm",
    process.platform === "win32" ? ["/d", "/s", "/c", "npm run build"] : ["run", "build"],
    {
      cwd: appRoot,
      encoding: "utf8",
      timeout: 120_000,
      windowsHide: true,
    },
  );
  record("tui-build", build.status === 0, {
    stdout: String(build.stdout ?? "").slice(-1000),
    stderr: String(build.stderr ?? "").slice(-1000),
    error: build.error?.message,
  });

  gateway = await startRealGateway();
  record("real-gateway-ready", Boolean(gateway.health), {
    url: gateway.url,
    health: gateway.health,
  });
  const seeded = await createSeedSession(gateway.url);
  record("seed-session-created", Boolean(seeded.session?.id), {
    sessionID: seeded.session?.id,
    payload: seeded.payload,
  });
  const beforeProviderLogs = await listProviderLogs(providerLogRoot);
  const beforeProviderKeys = new Set(beforeProviderLogs.map(providerLogKey));

  web = await startWebTerminal(gateway.url);
  record("web-terminal-ready", true, { url: web.url });

  const browser = await chromium.launch({ headless: true });
  const captures = [];
  const page = await browser.newPage({ viewport: { width: 1440, height: 980 } });
  try {
    await page.goto(`${web.url}/rich?instance=zip-password-real-gateway`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => window.__turaTerminal);
    await page.evaluate(() => window.__turaFit());
    await page.waitForFunction(
      () => /Enter to send|OC \| Tura TUI/.test(document.body.innerText),
      null,
      {
        timeout: 20_000,
      },
    );
    captures.push(await capture(page, "01-desktop-initial-real-gateway"));

    await submit(page, "/sessions");
    await page.waitForTimeout(900);
    captures.push(await capture(page, "02-desktop-sessions-panel"));

    await submit(page, "/config get");
    await page.waitForTimeout(900);
    captures.push(await capture(page, "03-desktop-config-panel"));

    await send(page, "\u001b");
    await page.waitForTimeout(400);
    await submit(page, "/chat");
    await page.waitForTimeout(900);
    captures.push(await capture(page, "04-desktop-chat-ready"));

    await submit(page, buildPrompt());
    await page.waitForTimeout(1200);
    captures.push(await capture(page, "05-desktop-prompt-submitted"));

    await page.waitForTimeout(5000);
    captures.push(await capture(page, "06-desktop-llm-in-flight"));

    const llmResult = await waitForLlmResult(gateway.url, seeded.session.id, beforeProviderKeys);
    await page.waitForTimeout(1800);
    captures.push(await capture(page, "07-desktop-llm-final"));

    await send(page, "\u001b[5~\u001b[5~");
    await page.waitForTimeout(700);
    captures.push(await capture(page, "08-desktop-scrollback"));

    await page.setViewportSize({ width: 390, height: 760 });
    await page.evaluate(() => window.__turaFit());
    await page.waitForTimeout(900);
    captures.push(await capture(page, "09-mobile-final"));

    await page.setViewportSize({ width: 900, height: 420 });
    await page.evaluate(() => window.__turaFit());
    await page.waitForTimeout(900);
    captures.push(await capture(page, "10-compact-final"));

    record(
      "llm-final-sentinel-visible",
      captures.some((item) => item.hasSentinel),
      {
        sentinel,
        finalTail: captures.at(-3)?.tail,
      },
    );
    record("provider-log-created", llmResult.newProviderLogs.length > 0, {
      providerLogs: llmResult.newProviderLogs.map((log) => log.path),
    });
    record("assistant-message-persisted", llmResult.assistantText.includes(sentinel), {
      assistantTextTail: llmResult.assistantText.slice(-1000),
    });
    record(
      "command-run-surface-observed",
      captures.some((item) => item.hasCommandRun),
      {
        commandScreens: captures.filter((item) => item.hasCommandRun).map((item) => item.name),
      },
    );
    record("screenshots-captured", screenshots.length >= 8, { screenshots });
    record(
      "ui-no-horizontal-overflow",
      captures.every((item) => !item.overflow),
      {
        overflowScreens: captures.filter((item) => item.overflow).map((item) => item.name),
      },
    );
    record(
      "ui-no-raw-control-leak",
      captures.every((item) => !item.hasRawControlLeak),
      {
        leakingScreens: captures.filter((item) => item.hasRawControlLeak).map((item) => item.name),
      },
    );

    const gatewayLogs = gateway.child.logs();
    const webLogs = web.child.logs();
    const analysis = serverAnalysis(gatewayLogs, webLogs, captures, llmResult.newProviderLogs);
    record("server-no-obvious-runtime-issue", !analysis.has_potential_server_issue, analysis);

    const summary = {
      ok: true,
      runRoot,
      workspace,
      gatewayUrl: gateway.url,
      webUrl: web.url,
      sessionID: seeded.session.id,
      sentinel,
      checks,
      screenshots,
      captures,
      providerLogs: llmResult.newProviderLogs,
      serverAnalysis: analysis,
    };
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
  } finally {
    await browser.close().catch(() => undefined);
  }
}

try {
  await main();
} catch (error) {
  const gatewayLogs = gateway?.child?.logs?.() ?? { stdout: "", stderr: "" };
  const webLogs = web?.child?.logs?.() ?? { stdout: "", stderr: "" };
  const summary = {
    ok: false,
    error: error instanceof Error ? error.stack || error.message : String(error),
    runRoot,
    workspace,
    checks,
    screenshots,
    gatewayLogs,
    webLogs,
  };
  await fs.mkdir(runRoot, { recursive: true }).catch(() => undefined);
  await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2)).catch(() => undefined);
  console.error(JSON.stringify(summary, null, 2));
  process.exitCode = 1;
} finally {
  const gatewayLogs = gateway?.child?.logs?.() ?? { stdout: "", stderr: "" };
  const webLogs = web?.child?.logs?.() ?? { stdout: "", stderr: "" };
  await fs
    .writeFile(path.join(runRoot, "gateway.stdout.log"), gatewayLogs.stdout)
    .catch(() => undefined);
  await fs
    .writeFile(path.join(runRoot, "gateway.stderr.log"), gatewayLogs.stderr)
    .catch(() => undefined);
  await fs
    .writeFile(path.join(runRoot, "web-terminal.stdout.log"), webLogs.stdout)
    .catch(() => undefined);
  await fs
    .writeFile(path.join(runRoot, "web-terminal.stderr.log"), webLogs.stderr)
    .catch(() => undefined);
  await stopProcess(web?.child);
  await stopProcess(gateway?.child);
}
