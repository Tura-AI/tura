#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { createRequire } from "node:module";

const appRoot = path.resolve(import.meta.dirname, "..", "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const runRoot = path.join(appRoot, "test-results", "tui-settings-navigation", String(Date.now()));
const nodeRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium } = nodeRequire("playwright");
let web;

function freePort() {
  return 26_000 + Math.floor(Math.random() * 20_000);
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

async function waitForTerminalText(page, pattern, deadlineMs = 10_000) {
  const source =
    pattern instanceof RegExp
      ? pattern.source
      : String(pattern).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const flags = pattern instanceof RegExp ? pattern.flags : "";
  try {
    await page.waitForFunction(
      ({ source, flags }) => {
        const regex = new RegExp(source, flags);
        const buffer = window.__turaTerminal?.buffer.active;
        if (!buffer) return false;
        for (let index = 0; index < buffer.length; index += 1) {
          if (regex.test(buffer.getLine(index)?.translateToString(true) ?? "")) return true;
        }
        return false;
      },
      { source, flags },
      { timeout: deadlineMs },
    );
  } catch (error) {
    const text = await terminalText(page).catch(() => "");
    throw new Error(`timed out waiting for ${pattern}:\n${text}`, { cause: error });
  }
}

async function send(page, data) {
  await page.evaluate((value) => window.__turaSendInput(value), data);
  await page.waitForTimeout(160);
}

async function submit(page, text) {
  await send(page, text);
  await send(page, "\r");
}

async function visibleCursor(page) {
  return page.evaluate(() => {
    const cursor = document.querySelector(".xterm-cursor");
    if (!cursor) return false;
    const style = getComputedStyle(cursor);
    if (style.display === "none" || style.visibility === "hidden" || style.opacity === "0") {
      return false;
    }
    const rect = cursor.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  });
}

async function screenshot(page, name) {
  const file = path.join(runRoot, name);
  await page.screenshot({ path: file, fullPage: false });
  return file;
}

async function main() {
  await fs.mkdir(runRoot, { recursive: true });
  const build = spawnSync(
    process.platform === "win32" ? "cmd.exe" : "npm",
    process.platform === "win32" ? ["/d", "/s", "/c", "npm run build"] : ["run", "build"],
    { cwd: appRoot, encoding: "utf8", timeout: 120_000, windowsHide: true },
  );
  if (build.status !== 0) {
    throw new Error(`npm run build failed\n${build.stdout}\n${build.stderr}`);
  }

  const port = freePort();
  web = startProcess(process.execPath, [path.join(appRoot, "scripts", "web-terminal.mjs")], {
    cwd: appRoot,
    env: {
      PORT: String(port),
      TURA_TUI_MOCK: "1",
      TURA_TUI_MOCK_ABOUT_UPDATE: "1",
    },
  });
  await waitForUrl(`http://127.0.0.1:${port}`);

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 280 } });
  try {
    await page.goto(`http://127.0.0.1:${port}/rich?instance=settings-navigation`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => window.__turaTerminal);
    await page.evaluate(() => window.__turaFit());
    await waitForTerminalText(page, /Enter to send|回车输入/u, 30_000);

    await submit(page, "/settings");
    await waitForTerminalText(page, /settings|设置/iu);
    if (await visibleCursor(page)) {
      throw new Error("settings page should hide the terminal cursor");
    }
    let text = await terminalText(page);
    if (!/\d+\/\d+/u.test(text)) {
      throw new Error(`settings page should show page count in bottom meta:\n${text}`);
    }
    await screenshot(page, "01-settings-root.png");

    for (let index = 0; index < 7; index += 1) {
      await send(page, "\x1b[B");
    }
    await waitForTerminalText(page, />\s+About/u);
    await send(page, "\r");
    await waitForTerminalText(page, /Release version\s+0\.1\.30/u);
    text = await terminalText(page);
    for (const label of ["Add star", "Report bug", "Contribute", "Update", "Contact"]) {
      if (!text.includes(label)) {
        throw new Error(`About should show ${label}:\n${text}`);
      }
    }
    await screenshot(page, "02-about-actions.png");

    for (let index = 0; index < 3; index += 1) {
      await send(page, "\x1b[B");
    }
    await waitForTerminalText(page, />\s+Update/u);
    await send(page, "\r");
    await waitForTerminalText(page, /session will be interrupted/u);
    text = await terminalText(page);
    if (!/Update now/u.test(text) || !/Cancel/u.test(text)) {
      throw new Error(`About update confirmation should use Update/Cancel selection:\n${text}`);
    }
    await screenshot(page, "03-about-update-confirmation.png");

    await send(page, "\x1b");
    await waitForTerminalText(page, /Add star/u);
    text = await terminalText(page);
    if (/session will be interrupted/u.test(text)) {
      throw new Error(`Esc should cancel the update confirmation:\n${text}`);
    }
    await screenshot(page, "04-about-update-cancelled.png");

    await send(page, "\x1b");
    await waitForTerminalText(page, /Enter to send|回车输入/u);
    await submit(page, "/variant");
    await waitForTerminalText(page, />\s+low/u);
    text = await terminalText(page);
    if (!/>\s+low/u.test(text)) {
      throw new Error(`setting detail should start at first option:\n${text}`);
    }
    if (await visibleCursor(page)) {
      throw new Error("setting detail should hide the terminal cursor");
    }
    await screenshot(page, "05-reasoning-first-option.png");

    await send(page, "\x1b[B");
    await waitForTerminalText(page, />\s+medium/u);
    await send(page, "\r");
    await page.waitForTimeout(500);
    text = await terminalText(page);
    if (!/>\s+medium/u.test(text)) {
      throw new Error(`setting detail should keep selected option after apply:\n${text}`);
    }
    if (/settings updated|设置已更新/iu.test(text)) {
      throw new Error(`setting apply should not show settings-updated notice:\n${text}`);
    }

    await send(page, "\x1b[B");
    await waitForTerminalText(page, />\s+high/u);
    text = await terminalText(page);
    if (/>\s+low/u.test(text)) {
      throw new Error(`setting detail should page away from the first page:\n${text}`);
    }
    if (!/>\s+high/u.test(text)) {
      throw new Error(`setting detail should show selected item on the next page:\n${text}`);
    }
    await screenshot(page, "06-reasoning-next-page.png");

    await page.goto(`http://127.0.0.1:${port}/rich?instance=settings-navigation-sessions`, {
      waitUntil: "domcontentloaded",
    });
    await waitForTerminalText(page, /Enter to send|回车输入/u, 30_000);
    await submit(page, "/sessions");
    await waitForTerminalText(page, /New session|新会话/u);
    text = await terminalText(page);
    if (!/1\/1/u.test(text)) {
      throw new Error(`session page should show page count in bottom meta:\n${text}`);
    }
    if (await visibleCursor(page)) {
      throw new Error("session page should hide the terminal cursor");
    }
    await screenshot(page, "07-sessions-page-count.png");
  } finally {
    await browser.close();
  }
}

main()
  .catch(async (error) => {
    await fs.mkdir(runRoot, { recursive: true }).catch(() => {});
    if (web?.logs) {
      const logs = web.logs();
      await fs
        .writeFile(path.join(runRoot, "web-terminal.stdout.log"), logs.stdout)
        .catch(() => {});
      await fs
        .writeFile(path.join(runRoot, "web-terminal.stderr.log"), logs.stderr)
        .catch(() => {});
    }
    console.error(error);
    process.exitCode = 1;
  })
  .finally(() => {
    stopProcess(web);
  });
