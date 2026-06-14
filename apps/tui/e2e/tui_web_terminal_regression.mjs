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

const rgb = {
  textPrimary: "rgb(238, 238, 238)",
  textAgentRich: "rgb(203, 203, 203)",
  textSecondary: "rgb(143, 143, 143)",
  textAuxiliary: "rgb(107, 107, 107)",
  textBackground: "rgb(72, 72, 72)",
  surfaceBackground: "rgb(32, 32, 34)",
  richHighlight: "rgb(64, 224, 208)",
};

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

async function waitForTerminalText(page, value, deadlineMs = 30_000) {
  await page.waitForFunction(
    (needle) => {
      const buffer = window.__turaTerminal?.buffer.active;
      if (!buffer) return false;
      for (let index = 0; index < buffer.length; index += 1) {
        if (buffer.getLine(index)?.translateToString(true).includes(needle)) return true;
      }
      return false;
    },
    value,
    { timeout: deadlineMs },
  );
}

async function waitForCompactTerminalText(page, value, deadlineMs = 30_000) {
  await page.waitForFunction(
    (needle) => {
      const buffer = window.__turaTerminal?.buffer.active;
      if (!buffer) return false;
      const lines = [];
      for (let index = 0; index < buffer.length; index += 1) {
        lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
      }
      return lines.join("").replace(/\s+/g, "").includes(String(needle).replace(/\s+/g, ""));
    },
    value,
    { timeout: deadlineMs },
  );
}

async function screenshot(page, name) {
  const file = path.join(runRoot, name);
  await page.screenshot({ path: file, fullPage: true });
  const stat = await fs.stat(file);
  record(`screenshot-${name}`, stat.size > 10_000, { file, bytes: stat.size });
  return file;
}

async function visibleTerminalStyles(page) {
  return page.$$eval(".xterm-rows span", (spans) =>
    spans.map((span) => {
      const style = getComputedStyle(span);
      return {
        text: span.textContent || "",
        color: style.color,
        backgroundColor: style.backgroundColor,
      };
    }),
  );
}

function recordVisibleColor(name, styles, color) {
  record(
    name,
    styles.some((item) => item.color === color),
    {
      colors: [...new Set(styles.map((item) => item.color).filter(Boolean))],
    },
  );
}

function recordVisibleBackground(name, styles, backgroundColor) {
  record(
    name,
    styles.some((item) => item.backgroundColor === backgroundColor),
    {
      backgrounds: [...new Set(styles.map((item) => item.backgroundColor).filter(Boolean))],
    },
  );
}

function countTextLines(value, pattern) {
  return value.split("\n").filter((line) => pattern.test(line)).length;
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
    env: {
      PORT: String(port),
      TURA_TUI_MOCK: "1",
      TURA_TUI_MOCK_LONG_SESSION: "1",
      TURA_TUI_MOCK_RENDER_REGRESSION: "1",
    },
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
    await waitForTerminalText(page, "Mock history 1000");
    await waitForTerminalText(page, "$ node scripts/check-render-regression.mjs");
    const initialText = await terminalText(page);
    const compactInitialText = initialText.replace(/\s+/g, "");
    record(
      "terminal-history-cleared",
      !initialText.includes("OLD TERMINAL HISTORY SHOULD BE CLEARED"),
    );
    record("latest-session-content-loaded", initialText.includes("Mock history 1000"));
    record(
      "long-user-multiline-visible",
      initialText.includes("REGRESSION_USER_SECOND_LINE_VISIBLE") &&
        initialText.includes("REGRESSION_USER_THIRD_LINE_VISIBLE"),
    );
    record(
      "long-cjk-text-visible-with-tail",
      initialText.includes("滚动中文颜色保持一致") &&
        initialText.includes("REGRESSION_AGENT_RICH_VISIBLE"),
    );
    const commandDetailLines = initialText
      .split("\n")
      .filter((line) =>
        /check-render-regression|REGRESSION_COMMAND_TAIL_VISIBLE_AFTER_WRAP/u.test(line),
      );
    record("command-detail-single-visible-line", commandDetailLines.length === 1, {
      lines: commandDetailLines,
    });
    record(
      "long-command-tail-not-expanded",
      !compactInitialText.includes("REGRESSION_COMMAND_TAIL_VISIBLE_AFTER_WRAP"),
    );
    record(
      "rich-highlight-text-visible",
      initialText.includes("REGRESSION_RICH_HIGHLIGHT_VISIBLE"),
    );
    await page.evaluate(() => window.__turaTerminal.scrollToBottom());
    const bottomStyles = await visibleTerminalStyles(page);
    recordVisibleColor("dom-primary-text-100-visible", bottomStyles, rgb.textPrimary);
    recordVisibleColor("dom-agent-rich-text-85-visible", bottomStyles, rgb.textAgentRich);
    recordVisibleColor("dom-secondary-user-text-60-visible", bottomStyles, rgb.textSecondary);
    recordVisibleColor("dom-command-auxiliary-text-45-visible", bottomStyles, rgb.textAuxiliary);
    recordVisibleColor("dom-background-hint-text-30-visible", bottomStyles, rgb.textBackground);
    recordVisibleColor("dom-rich-highlight-visible", bottomStyles, rgb.richHighlight);
    recordVisibleBackground("dom-surface-background-visible", bottomStyles, rgb.surfaceBackground);
    await screenshot(page, "latest-session.png");

    await page.evaluate(() => window.__turaTerminal.scrollToTop());
    await waitForTerminalText(page, "Mock history 001");
    const scrolledText = await terminalText(page);
    record("earliest-session-content-reachable", scrolledText.includes("Mock history 001"));
    const markerMatches = scrolledText.match(/Mock history \d{3,4}/g) ?? [];
    record(
      "mock-history-has-no-duplicate-lines",
      new Set(markerMatches).size === markerMatches.length,
      {
        markers: markerMatches.length,
        unique: new Set(markerMatches).size,
      },
    );
    record("composer-not-in-history-lines", !/Mock history \d{3,4}.*[>].*mock/i.test(scrolledText));
    await screenshot(page, "earliest-session.png");

    const composerInput = Array.from(
      { length: 8 },
      (_item, index) =>
        `REGRESSION_COMPOSER_WRAP_${String(index + 1).padStart(2, "0")} ${"long-user-input-remains-visible-".repeat(5)}`,
    ).join(" ");
    await page.goto(
      `http://127.0.0.1:${port}/rich?instance=composer&initialComposer=${encodeURIComponent(composerInput)}`,
      { waitUntil: "domcontentloaded" },
    );
    await page.waitForFunction(() => window.__turaTerminal);
    await page.evaluate(() => window.__turaFit());
    await waitForTerminalText(page, "REGRESSION_COMPOSER_WRAP_08");
    const composerText = await terminalText(page);
    record(
      "composer-expands-beyond-four-lines",
      countTextLines(composerText, /REGRESSION_COMPOSER_WRAP_/u) > 4,
      {
        lines: countTextLines(composerText, /REGRESSION_COMPOSER_WRAP_/u),
      },
    );
    record(
      "composer-keeps-first-and-last-line-visible",
      composerText.includes("REGRESSION_COMPOSER_WRAP_01") &&
        composerText.includes("REGRESSION_COMPOSER_WRAP_08"),
    );
    await screenshot(page, "composer-responsive.png");
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
