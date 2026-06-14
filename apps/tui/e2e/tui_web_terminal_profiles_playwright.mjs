#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { createRequire } from "node:module";

const appRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const runRoot = path.join(repoRoot, "target", "tui-web-terminal-profiles", `${Date.now()}`);
const nodeRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium, devices } = nodeRequire("playwright");
let web;
const visibleTerminalPattern = /tura|Enter to send|回车输入|OC \| Tura TUI/u;

function freePort() {
  return 24_000 + Math.floor(Math.random() * 20_000);
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
      if (response.ok) return;
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

async function waitForText(page, pattern, timeout = 30_000) {
  await page.waitForFunction(
    (source) => {
      const regex = new RegExp(source, "iu");
      const buffer = window.__turaTerminal?.buffer.active;
      if (!buffer) return false;
      for (let index = 0; index < buffer.length; index += 1) {
        if (regex.test(buffer.getLine(index)?.translateToString(true) ?? "")) return true;
      }
      return false;
    },
    pattern.source,
    { timeout },
  );
}

async function screenshot(page, name) {
  const file = path.join(runRoot, name);
  await page.screenshot({ path: file, fullPage: true });
  const stat = await fs.stat(file);
  assert.ok(stat.size > 8_000, `${name} screenshot should not be blank`);
  return { file, bytes: stat.size };
}

async function assertNoHorizontalOverflow(page) {
  const overflow = await page.evaluate(() => ({
    body: document.body.scrollWidth - document.documentElement.clientWidth,
    terminal:
      document.querySelector("#terminal")?.scrollWidth -
      document.querySelector("#terminal")?.clientWidth,
  }));
  assert.ok(overflow.body <= 1, `body horizontal overflow: ${JSON.stringify(overflow)}`);
  assert.ok(
    (overflow.terminal ?? 0) <= 1,
    `terminal horizontal overflow: ${JSON.stringify(overflow)}`,
  );
}

async function assertTerminalVerticalSpacing(page, profile) {
  const spacing = await page.evaluate(() => {
    const terminal = document.querySelector("#terminal");
    const xterm = document.querySelector(".xterm");
    const screen = document.querySelector(".xterm-screen");
    const rowNodes = [...document.querySelectorAll(".xterm-rows > div")];
    const rowTexts = rowNodes.map((node) => node.textContent ?? "");
    const visibleRows = rowNodes.filter((node) => (node.textContent ?? "").trim().length > 0);
    const terminalRect = terminal?.getBoundingClientRect();
    const screenRect = screen?.getBoundingClientRect();
    const firstVisibleRect = visibleRows[0]?.getBoundingClientRect();
    const lastVisibleRect = visibleRows.at(-1)?.getBoundingClientRect();
    const style = terminal ? getComputedStyle(terminal) : undefined;
    const xtermStyle = xterm ? getComputedStyle(xterm) : undefined;
    const screenStyle = screen ? getComputedStyle(screen) : undefined;
    const rowStyle = rowNodes[0] ? getComputedStyle(rowNodes[0]) : undefined;
    const fontSize = Number.parseFloat(rowStyle?.fontSize ?? "0");
    const blankRows = rowTexts.filter((text) => text.trim().length === 0).length;
    const maxBlankRun = rowTexts.reduce(
      (state, text) => {
        const nextRun = text.trim() ? 0 : state.current + 1;
        return { current: nextRun, max: Math.max(state.max, nextRun) };
      },
      { current: 0, max: 0 },
    ).max;
    const hasBackgroundSpan = (node, minimumWidth = 0) => {
      const rect = node.getBoundingClientRect();
      return [...node.querySelectorAll("span")].some((span) => {
        const spanRect = span.getBoundingClientRect();
        const background = getComputedStyle(span).backgroundColor;
        return (
          background !== "rgba(0, 0, 0, 0)" &&
          background !== "transparent" &&
          spanRect.width > minimumWidth &&
          spanRect.height > rect.height * 0.6
        );
      });
    };
    const backgroundBandWidth = (node) => {
      return [...node.querySelectorAll("span")]
        .filter((span) => {
          const background = getComputedStyle(span).backgroundColor;
          return /rgb\(32, 32, 34\)/u.test(background);
        })
        .reduce((sum, span) => sum + span.getBoundingClientRect().width, 0);
    };
    const panelBlankRows = rowNodes.filter((node) => {
      const text = (node.textContent ?? "").replace(/[|│┃▏▕]/gu, "").trim();
      if (text) return false;
      return hasBackgroundSpan(node, 8);
    }).length;
    const textBandRows = rowNodes.filter((node) => {
      if (!(node.textContent ?? "").trim()) return false;
      return hasBackgroundSpan(node, 8);
    }).length;
    const mockRowIndex = rowTexts.findIndex((text) => text.includes("Mock TUI"));
    const mockRow = rowNodes[mockRowIndex];
    const mockRowWidth = mockRow?.getBoundingClientRect().width ?? 0;
    const mockBandWidth = mockRow ? backgroundBandWidth(mockRow) : 0;
    const panelPaddingWidths = rowNodes
      .map((node, index) => {
        const text = (node.textContent ?? "").replace(/[|│┃▏▕]/gu, "").trim();
        return { index, text, width: backgroundBandWidth(node) };
      })
      .filter((row) => row.width > 0 && !row.text);
    const topBandWidth =
      panelPaddingWidths
        .filter((row) => mockRowIndex >= 0 && row.index < mockRowIndex)
        .at(-1)?.width ?? 0;
    const bottomBandWidth =
      panelPaddingWidths.find((row) => mockRowIndex >= 0 && row.index > mockRowIndex)?.width ?? 0;
    return {
      paddingTop: Number.parseFloat(style?.paddingTop ?? "0"),
      paddingBottom: Number.parseFloat(style?.paddingBottom ?? "0"),
      xtermBackground: xtermStyle?.backgroundColor ?? "",
      screenBackground: screenStyle?.backgroundColor ?? "",
      rowHeight: rowNodes[0]?.getBoundingClientRect().height ?? 0,
      lineHeight: Number.parseFloat(rowStyle?.lineHeight ?? "0"),
      fontSize,
      topContentGap: firstVisibleRect && terminalRect ? firstVisibleRect.top - terminalRect.top : 0,
      bottomContentGap:
        lastVisibleRect && terminalRect ? terminalRect.bottom - lastVisibleRect.bottom : 0,
      screenTopGap: screenRect && terminalRect ? screenRect.top - terminalRect.top : 0,
      blankRows,
      maxBlankRun,
      panelBlankRows,
      textBandRows,
      mockBandWidth,
      mockRowWidth,
      topBandWidth,
      bottomBandWidth,
      rowCount: rowNodes.length,
      visibleRowCount: visibleRows.length,
      viewportText: rowTexts.join("\n"),
    };
  });
  assert.ok(
    spacing.paddingTop >= 8,
    `${profile} terminal top padding collapsed: ${JSON.stringify(spacing)}`,
  );
  assert.ok(
    spacing.paddingBottom >= 8,
    `${profile} terminal bottom padding collapsed: ${JSON.stringify(spacing)}`,
  );
  assert.ok(
    /rgb\(16, 16, 16\)/u.test(spacing.xtermBackground) &&
      /rgb\(16, 16, 16\)/u.test(spacing.screenBackground),
    `${profile} xterm background should cover terminal gutters: ${JSON.stringify(spacing)}`,
  );
  assert.ok(spacing.rowHeight >= 16, `${profile} row height collapsed: ${JSON.stringify(spacing)}`);
  assert.ok(
    spacing.lineHeight >= spacing.fontSize * 1.18,
    `${profile} line box should preserve vertical room: ${JSON.stringify(spacing)}`,
  );
  assert.ok(
    spacing.topContentGap >= spacing.paddingTop - 1,
    `${profile} missing terminal top gutter: ${JSON.stringify(spacing)}`,
  );
  assert.ok(
    spacing.bottomContentGap >= spacing.paddingBottom + spacing.rowHeight,
    `${profile} missing bottom text breathing room: ${JSON.stringify(spacing)}`,
  );
  assert.ok(
    spacing.blankRows >= 2,
    `${profile} missing blank rows between blocks: ${JSON.stringify(spacing)}`,
  );
  assert.ok(
    spacing.maxBlankRun >= 1,
    `${profile} blank row run collapsed: ${JSON.stringify(spacing)}`,
  );
  if (profile !== "plain") {
    assert.ok(
      spacing.panelBlankRows >= 2,
      `${profile} missing colored vertical panel padding rows: ${JSON.stringify(spacing)}`,
    );
    assert.ok(
      spacing.textBandRows >= 2,
      `${profile} missing colored text bands: ${JSON.stringify(spacing)}`,
    );
    if (!profile.includes("mobile")) {
      assert.ok(
        spacing.mockBandWidth >= spacing.mockRowWidth * 0.9,
        `${profile} mock message band should cover the full row width: ${JSON.stringify(spacing)}`,
      );
    }
    assert.ok(
      spacing.bottomBandWidth > 0 && Math.abs(spacing.bottomBandWidth - spacing.mockBandWidth) <= 24,
      `${profile} mock message lower padding band should match body width: ${JSON.stringify(spacing)}`,
    );
  }
}

async function main() {
  await fs.mkdir(runRoot, { recursive: true });
  const build = spawnSync(
    process.platform === "win32" ? "cmd.exe" : "npm",
    process.platform === "win32" ? ["/d", "/s", "/c", "npm run build"] : ["run", "build"],
    { cwd: appRoot, encoding: "utf8", timeout: 120_000, windowsHide: true },
  );
  assert.equal(build.status, 0, String(build.stderr || build.stdout));

  const port = freePort();
  web = startProcess(process.execPath, [path.join(appRoot, "scripts", "web-terminal.mjs")], {
    cwd: appRoot,
    env: { PORT: String(port), TURA_TUI_MOCK: "1" },
  });
  await waitForUrl(`http://127.0.0.1:${port}`);

  const browser = await chromium.launch({ headless: true });
  const artifacts = [];
  try {
    for (const profile of ["plain", "ansi", "rich"]) {
      const page = await browser.newPage({ viewport: { width: 1100, height: 720 } });
      await page.goto(`http://127.0.0.1:${port}/${profile}?instance=${profile}`, {
        waitUntil: "domcontentloaded",
      });
      await page.waitForFunction(() => window.__turaTerminal);
      await page.evaluate(() => window.__turaFit());
      await waitForText(page, visibleTerminalPattern);
      const text = await terminalText(page);
      assert.match(text, visibleTerminalPattern);
      assert.doesNotMatch(text, /\x1b\[[0-9;]/u, `${profile} leaked raw ANSI controls`);
      await assertNoHorizontalOverflow(page);
      await assertTerminalVerticalSpacing(page, profile);
      artifacts.push(await screenshot(page, `${profile}.png`));
      await page.close();
    }

    const mobile = await browser.newPage({ ...devices["Pixel 5"] });
    await mobile.goto(`http://127.0.0.1:${port}/rich?instance=mobile-user-agent`, {
      waitUntil: "domcontentloaded",
    });
    await mobile.waitForFunction(() => window.__turaTerminal);
    await mobile.evaluate(() => window.__turaFit());
    await waitForText(mobile, visibleTerminalPattern);
    const userAgent = await mobile.evaluate(() => navigator.userAgent);
    assert.match(userAgent, /Mobile|Android|Pixel/i);
    await assertNoHorizontalOverflow(mobile);
    await assertTerminalVerticalSpacing(mobile, "mobile-user-agent");
    artifacts.push(await screenshot(mobile, "mobile-user-agent.png"));
    await mobile.close();
  } finally {
    await browser.close();
  }

  const summary = { ok: true, port, runRoot, artifacts };
  await fs.writeFile(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
}

try {
  await main();
} catch (error) {
  const summary = {
    ok: false,
    error: error instanceof Error ? error.stack || error.message : String(error),
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
