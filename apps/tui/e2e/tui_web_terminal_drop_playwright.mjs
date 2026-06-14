#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";

const appRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const runRoot = path.join(repoRoot, "target", "tui-web-terminal-drop", String(Date.now()));
const nodeRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium } = nodeRequire("playwright");

function freePort() {
  return 24_000 + Math.floor(Math.random() * 10_000);
}

function startProcess(command, args, options = {}) {
  const child = spawn(command, args, {
    cwd: options.cwd || appRoot,
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

async function waitForUrl(url, child, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      const logs = child.logs?.() ?? {};
      throw new Error(
        `web terminal exited before readiness: ${child.exitCode}\n${logs.stderr || ""}\n${logs.stdout || ""}`,
      );
    }
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw lastError ?? new Error(`timed out waiting for ${url}`);
}

async function waitForTerminalText(page, pattern, timeoutMs = 20_000) {
  await page.waitForFunction(
    (source) => {
      const regex = new RegExp(source, "u");
      return regex.test(document.body.innerText || "");
    },
    pattern.source,
    { timeout: timeoutMs },
  );
}

async function main() {
  await fs.mkdir(runRoot, { recursive: true });
  const port = freePort();
  const web = startProcess(process.execPath, [path.join(appRoot, "scripts", "web-terminal.mjs")], {
    env: {
      PORT: String(port),
      TURA_TUI_MOCK: "1",
      TURA_CWD: runRoot,
    },
  });
  try {
    await waitForUrl(`http://127.0.0.1:${port}/`, web);
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    const pageErrors = [];
    page.on("pageerror", (error) =>
      pageErrors.push(String(error?.stack || error?.message || error)),
    );
    try {
      await page.goto(`http://127.0.0.1:${port}/rich?instance=drop`, {
        waitUntil: "domcontentloaded",
      });
      await page.waitForFunction(() => Boolean(globalThis.__turaTerminal), null, {
        timeout: 10_000,
      });
      await waitForTerminalText(page, /Enter to send|回车输入/u);

      await page.evaluate(async () => {
        const dataTransfer = new DataTransfer();
        dataTransfer.items.add(
          new File([new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10])], "drop-image.png", {
            type: "image/png",
          }),
        );
        dataTransfer.items.add(new File(["note"], "drop-note.txt", { type: "text/plain" }));
        const shell = document.querySelector(".shell");
        shell.dispatchEvent(
          new DragEvent("dragover", { bubbles: true, cancelable: true, dataTransfer }),
        );
        if (!shell.classList.contains("dragging"))
          throw new Error("dragover did not set dragging state");
        shell.dispatchEvent(
          new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer }),
        );
      });

      await waitForTerminalText(page, /drop-image\.png/u);
      await waitForTerminalText(page, /drop-note\.txt/u);
      const directPathToken = await page.evaluate(async () => {
        const dataTransfer = new DataTransfer();
        dataTransfer.setData("text/uri-list", "file:///C:/tmp/direct-drop.txt\n");
        return await globalThis.__turaHandleDroppedData(dataTransfer);
      });
      assert.equal(directPathToken, "[direct-drop.txt](file:///C:/tmp/direct-drop.txt)");

      const attachmentFiles = await fs.readdir(path.join(runRoot, ".tura", "attachments"));
      assert.ok(attachmentFiles.some((file) => file.endsWith("drop-image.png")));
      assert.ok(attachmentFiles.some((file) => file.endsWith("drop-note.txt")));
      await page.screenshot({ path: path.join(runRoot, "drop-composer.png"), fullPage: false });
      assert.deepEqual(pageErrors, []);
      const summary = { ok: true, runRoot, attachmentFiles, directPathToken };
      await fs.writeFile(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2));
      console.log(JSON.stringify(summary, null, 2));
    } finally {
      await browser.close();
    }
  } finally {
    stopProcess(web);
  }
}

main().catch(async (error) => {
  await fs.mkdir(runRoot, { recursive: true });
  const summary = {
    ok: false,
    error: error instanceof Error ? error.stack || error.message : String(error),
    runRoot,
  };
  await fs.writeFile(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2));
  console.error(JSON.stringify(summary, null, 2));
  process.exitCode = 1;
});
