#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { createRequire } from "node:module";

const appRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const runRoot = path.join(repoRoot, "target", "tui-web-terminal-regression", `${Date.now()}`);
const nodeRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium } = nodeRequire("playwright");
const checks = [];
let web;

function record(name, ok, details = {}) {
  checks.push({ name, ok, ...details });
  if (!ok) throw new Error(`${name} failed: ${JSON.stringify(details)}`);
}

function freePort() {
  return 23_000 + Math.floor(Math.random() * 20_000);
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
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
    return;
  }
  child.kill("SIGTERM");
}

async function waitForUrl(url, deadlineMs = 30_000) {
  const deadline = Date.now() + deadlineMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return response;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw lastError ?? new Error(`timed out waiting for ${url}`);
}

async function terminalText(page) {
  return page.evaluate(() => {
    const buffer = window.__turaTerminal?.buffer.active;
    if (!buffer) return "";
    const lines = [];
    for (let index = 0; index < buffer.length; index += 1) {
      lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
    }
    return lines.join("\n");
  });
}

async function main() {
  await fs.mkdir(runRoot, { recursive: true });
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

  const port = freePort();
  web = startProcess(process.execPath, [path.join(appRoot, "scripts", "web-terminal.mjs")], {
    cwd: appRoot,
    env: { PORT: String(port), TURA_TUI_MOCK: "1", TURA_TUI_MOCK_LONG_SESSION: "1" },
  });
  await waitForUrl(`http://127.0.0.1:${port}`, 30_000);
  record("web-terminal-ready", true, { port });

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1360, height: 820 } });
  try {
    await page.goto(`http://127.0.0.1:${port}/rich?instance=regression`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => window.__turaTerminal);
    await page.evaluate(() =>
      window.__turaTerminal.write("OLD TERMINAL HISTORY SHOULD BE CLEARED\r\n"),
    );
    await page.evaluate(() => window.__turaFit());
    await page.waitForFunction(() => {
      const buffer = window.__turaTerminal?.buffer.active;
      if (!buffer) return false;
      for (let index = 0; index < buffer.length; index += 1) {
        if (buffer.getLine(index)?.translateToString(true).includes("Mock history 080"))
          return true;
      }
      return false;
    });
    const initialText = await terminalText(page);
    record(
      "terminal-history-cleared",
      !initialText.includes("OLD TERMINAL HISTORY SHOULD BE CLEARED"),
    );
    record("latest-session-content-loaded", initialText.includes("Mock history 080"));
    await page.screenshot({ path: path.join(runRoot, "latest-session.png"), fullPage: true });

    for (let index = 0; index < 12; index += 1)
      await page.evaluate(() => window.__turaSendInput("\u001b[5~"));
    await page.waitForFunction(() => {
      const buffer = window.__turaTerminal?.buffer.active;
      if (!buffer) return false;
      for (let index = 0; index < buffer.length; index += 1) {
        if (buffer.getLine(index)?.translateToString(true).includes("Mock history 001"))
          return true;
      }
      return false;
    });
    const scrolledText = await terminalText(page);
    record("earliest-session-content-reachable", scrolledText.includes("Mock history 001"));
    await page.screenshot({ path: path.join(runRoot, "earliest-session.png"), fullPage: true });

    await page.evaluate(() => window.__turaSendInput("\u001b[6~\u001b[6~\u001b[6~"));
    const started = Date.now();
    await page.evaluate(() => window.__turaSendInput("responsive composer latency check"));
    await page.waitForFunction(() => {
      const buffer = window.__turaTerminal?.buffer.active;
      if (!buffer) return false;
      for (let index = 0; index < buffer.length; index += 1) {
        if (
          buffer
            .getLine(index)
            ?.translateToString(true)
            .includes("responsive composer latency check")
        )
          return true;
      }
      return false;
    });
    const elapsed = Date.now() - started;
    record("composer-input-responsive", elapsed < 1200, { elapsed });
    await page.screenshot({ path: path.join(runRoot, "composer-responsive.png"), fullPage: true });
  } finally {
    await browser.close();
  }
}

try {
  await main();
  const summary = { ok: true, checks, runRoot };
  await fs.writeFile(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
} catch (error) {
  const summary = {
    ok: false,
    error: error instanceof Error ? error.stack || error.message : String(error),
    checks,
    logs: web?.logs?.(),
    runRoot,
  };
  await fs.mkdir(runRoot, { recursive: true });
  await fs.writeFile(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2));
  console.error(JSON.stringify(summary, null, 2));
  process.exitCode = 1;
} finally {
  stopProcess(web);
}
