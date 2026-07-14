#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..", "..");
const appRoot = path.join(repoRoot, "apps", "tui");
const runRoot = path.join(
  repoRoot,
  "apps",
  "tui",
  "test-results",
  "tui-session-name-update",
  String(Date.now()),
);
const workspace = path.join(runRoot, "workspace");
const webTerminalBin = path.join(appRoot, "scripts", "web-terminal.mjs");
const tuiRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium } = tuiRequire("playwright");
const initialName = "Initial TUI Session";
const updatedName = "Updated TUI Session";

function sendJson(res, value, status = 200) {
  const body = JSON.stringify(value);
  res.writeHead(status, {
    "content-type": "application/json",
    "content-length": Buffer.byteLength(body),
  });
  res.end(body);
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

function stopProcess(child) {
  if (!child || child.exitCode !== null || child.killed) return;
  if (process.platform === "win32" && child.pid)
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
  else child.kill("SIGTERM");
}

async function waitForUrl(url, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      if ((await fetch(url)).ok) return;
    } catch {
      // Retry until the web terminal is ready or the deadline expires.
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`timed out waiting for ${url}`);
}

async function startGateway() {
  await fs.mkdir(workspace, { recursive: true });
  const clients = new Set();
  const session = {
    id: "sess-tui-name",
    name: initialName,
    session_display_name: initialName,
    directory: workspace,
    status: "idle",
    model: "openai/gpt-test",
    agent: "direct",
    created_at: Date.now(),
    updated_at: Date.now(),
    message_count: 0,
    task_management: {},
  };
  const emitUpdate = () => {
    const info = {
      ...session,
      name: updatedName,
      session_display_name: updatedName,
      updated_at: Date.now(),
    };
    const event = {
      directory: workspace,
      payload: { type: "session.updated", properties: { sessionID: info.id, info } },
    };
    for (const client of clients) client.write(`data: ${JSON.stringify(event)}\n\n`);
  };
  const server = http.createServer((req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1");
    if (url.pathname === "/global/health") return sendJson(res, { healthy: true, version: "mock" });
    if (url.pathname === "/project/workspace/sync") return sendJson(res, { ok: true });
    if (url.pathname === "/session") return sendJson(res, [session]);
    if (url.pathname === `/session/${session.id}/message`) return sendJson(res, []);
    if (url.pathname === "/session/config")
      return sendJson(res, { model: "openai/gpt-test", active_agent: "direct" });
    if (url.pathname === "/provider")
      return sendJson(res, { all: [], connected: [], default: {}, enums: {} });
    if (url.pathname === "/agent" || url.pathname === "/persona") return sendJson(res, []);
    if (url.pathname === "/event") {
      res.writeHead(200, {
        "content-type": "text/event-stream",
        "cache-control": "no-cache",
        connection: "keep-alive",
      });
      clients.add(res);
      res.write(
        `data: ${JSON.stringify({ directory: "global", payload: { type: "server.connected", properties: {} } })}\n\n`,
      );
      setTimeout(emitUpdate, 900);
      req.on("close", () => clients.delete(res));
      return;
    }
    return sendJson(res, {}, 404);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    url: `http://127.0.0.1:${server.address().port}`,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

async function main() {
  const gateway = await startGateway();
  let web;
  try {
    const webPort = 34_000 + Math.floor(Math.random() * 10_000);
    web = startProcess(process.execPath, [webTerminalBin], {
      cwd: appRoot,
      env: { PORT: String(webPort), TURA_GATEWAY_URL: gateway.url, TURA_CWD: workspace },
    });
    await waitForUrl(`http://127.0.0.1:${webPort}/`);
    const browser = await chromium.launch({ headless: true });
    try {
      const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
      await page.goto(`http://127.0.0.1:${webPort}/rich?instance=session-name-update`, {
        waitUntil: "domcontentloaded",
      });
      await page.waitForFunction((name) => document.body.innerText.includes(name), updatedName, {
        timeout: 20_000,
      });
      const text = await page.locator("body").innerText();
      assert.match(text, new RegExp(updatedName));
      const screenshotPath = path.join(runRoot, "tui-session-name-updated.png");
      await fs.mkdir(runRoot, { recursive: true });
      await page.screenshot({ path: screenshotPath, fullPage: false });
      console.log(`TUI_SESSION_NAME_UPDATE_OK ${JSON.stringify({ screenshotPath, updatedName })}`);
    } finally {
      await browser.close();
    }
  } finally {
    stopProcess(web);
    await gateway.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
