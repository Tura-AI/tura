#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..", "..", "..");
const appRoot = path.join(repoRoot, "apps", "tui");
const nodeBin = process.execPath;
const exeSuffix = process.platform === "win32" ? ".exe" : "";
const gatewayExe = path.join(repoRoot, "target", "debug", `tura_gateway${exeSuffix}`);
const webTerminalBin = path.join(appRoot, "scripts", "web-terminal.mjs");
const tuiRequire = createRequire(path.join(appRoot, "package.json"));
const runId = process.env.TURA_TUI_SESSION_NAME_RUN_ID || `tui-live-session-name-${Date.now()}`;
const runRoot = path.join(repoRoot, "apps", "tui", "test-results", "live-session-name", runId);
const workspace = path.join(runRoot, "workspace");
const screenshotsDir = path.join(runRoot, "screenshots");
const expectedName = process.env.TURA_TUI_SESSION_NAME_EXPECTED || `TuraName${Date.now()}`;
const prompt =
  process.env.TURA_TUI_SESSION_NAME_PROMPT ||
  `Reply with exactly this name and nothing else: ${expectedName}`;
const model =
  process.env.TURA_TUI_SESSION_NAME_MODEL || process.env.TURA_E2E_MODEL || "openai/gpt-5.5";
const agent =
  process.env.TURA_TUI_SESSION_NAME_AGENT || process.env.TURA_E2E_AGENT || "direct-text-only";
const timeoutMs = Number(process.env.TURA_TUI_SESSION_NAME_TIMEOUT_MS || 180_000);

function freePort() {
  return 20_000 + Math.floor(Math.random() * 20_000);
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
      // Retry until the service is actually up, not just spiritually present.
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`timed out waiting for ${url}`);
}

async function writeProcessLogs(child, prefix) {
  if (!child?.logs) return;
  const logs = child.logs();
  await fs.mkdir(runRoot, { recursive: true });
  await fs.writeFile(path.join(runRoot, `${prefix}.stdout.log`), logs.stdout);
  await fs.writeFile(path.join(runRoot, `${prefix}.stderr.log`), logs.stderr);
}

async function startGateway() {
  const external = process.env.TURA_TUI_SESSION_NAME_GATEWAY_URL || process.env.TURA_GATEWAY_URL;
  if (external) {
    const url = external.replace(/\/+$/u, "");
    return { url, child: undefined, health: await waitForUrl(`${url}/global/health`, 60_000) };
  }
  for (const port of [4126, 4156]) {
    const url = `http://127.0.0.1:${port}`;
    try {
      return { url, child: undefined, health: await waitForUrl(`${url}/global/health`, 1500) };
    } catch {
      // No reusable gateway on this conventional port.
    }
  }
  await fs.access(gatewayExe);
  const port = freePort();
  const child = startProcess(gatewayExe, [], {
    env: {
      PORT: String(port),
      TURA_HOME: path.join(runRoot, "tura-home"),
      TURA_PROJECT_ROOT: repoRoot,
      TURA_CWD: workspace,
      LOG_PATH: path.join(runRoot, "provider.log"),
    },
  });
  const url = `http://127.0.0.1:${port}`;
  try {
    return { url, child, health: await waitForUrl(`${url}/global/health`, 90_000) };
  } catch (error) {
    await writeProcessLogs(child, "gateway");
    await stopProcess(child);
    throw error;
  }
}

async function startWebTerminal(gatewayUrl) {
  const port = freePort();
  const child = startProcess(nodeBin, [webTerminalBin], {
    cwd: appRoot,
    env: { PORT: String(port), TURA_GATEWAY_URL: gatewayUrl, TURA_CWD: workspace },
  });
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/`, 30_000).catch(async () => {
    const response = await fetch(`${url}/`);
    if (!response.ok) throw new Error(`web terminal returned ${response.status}`);
  });
  return { url, child };
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

async function sendRich(webUrl, data) {
  await fetch(`${webUrl}/rich/input`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ data }),
  });
}

async function submitPrompt(webUrl, page) {
  await sendRich(webUrl, "\u001b");
  await page.waitForTimeout(150);
  await sendRich(webUrl, prompt);
  await page.waitForTimeout(300);
  await page.screenshot({
    path: path.join(screenshotsDir, "02-composer-filled.png"),
    fullPage: false,
  });
  await sendRich(webUrl, "\r");
}

function messageText(message) {
  return (message.parts || message.info?.parts || [])
    .map((part) => part.text || part.content || "")
    .join("");
}

function messageRole(message) {
  return message.role || message.info?.role || "";
}

async function waitForName(gatewayUrl, page) {
  const deadline = Date.now() + timeoutMs;
  let last = {};
  while (Date.now() < deadline) {
    const sessions = await requestJson(
      `${gatewayUrl}/session?directory=${encodeURIComponent(workspace)}&includeChildren=true&limit=20`,
    ).catch(() => []);
    const session = Array.isArray(sessions) ? sessions[0] : undefined;
    const messages = session?.id
      ? await requestJson(`${gatewayUrl}/session/${encodeURIComponent(session.id)}/message`).catch(
          () => [],
        )
      : [];
    const assistantText = Array.isArray(messages)
      ? messages
          .filter((message) => messageRole(message) === "assistant")
          .map(messageText)
          .find((text) => text.includes(expectedName)) || ""
      : "";
    const terminalText = await page
      .locator("body")
      .innerText()
      .catch(() => "");
    last = { session, assistantText, terminalTail: terminalText.slice(-1000) };
    if (
      /all providers failed|rate_limit|insufficient_quota|model call failed/i.test(
        `${assistantText}\n${terminalText}`,
      )
    ) {
      throw new Error(`provider error: ${JSON.stringify(last)}`);
    }
    if (assistantText.includes(expectedName)) return last;
    await page.waitForTimeout(1500);
  }
  throw new Error(`timed out waiting for TUI live name: ${JSON.stringify(last)}`);
}

async function main() {
  await fs.rm(runRoot, { recursive: true, force: true });
  await fs.mkdir(screenshotsDir, { recursive: true });
  await fs.mkdir(workspace, { recursive: true });

  const gateway = await startGateway();
  let web;
  try {
    assert.equal(gateway.health?.healthy, true);
    web = await startWebTerminal(gateway.url);
    const { chromium } = tuiRequire("playwright");
    const browser = await chromium.launch({ headless: true });
    try {
      const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
      await page.goto(`${web.url}/rich?instance=live-session-name`, {
        waitUntil: "domcontentloaded",
      });
      await page.waitForTimeout(1200);
      await page.screenshot({ path: path.join(screenshotsDir, "01-open.png"), fullPage: false });
      await submitPrompt(web.url, page);
      await page.screenshot({
        path: path.join(screenshotsDir, "03-after-submit.png"),
        fullPage: false,
      });
      const result = await waitForName(gateway.url, page);
      await page.screenshot({
        path: path.join(screenshotsDir, "04-after-name.png"),
        fullPage: false,
      });
      const report = {
        ok: true,
        gatewayUrl: gateway.url,
        webUrl: web.url,
        model,
        agent,
        expectedName,
        result,
      };
      await fs.writeFile(path.join(runRoot, "report.json"), JSON.stringify(report, null, 2));
      console.log(`TUI_LIVE_SESSION_NAME_OK ${JSON.stringify(report)}`);
    } finally {
      await browser.close();
    }
  } finally {
    if (web?.child) {
      await writeProcessLogs(web.child, "web");
      await stopProcess(web.child);
    }
    if (gateway.child) {
      await writeProcessLogs(gateway.child, "gateway");
      await stopProcess(gateway.child);
    }
  }
}

main().catch(async (error) => {
  await fs.mkdir(runRoot, { recursive: true });
  await fs.writeFile(path.join(runRoot, "exception.txt"), error.stack || String(error));
  console.error(error);
  process.exit(1);
});
