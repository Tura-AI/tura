#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";
import { referenceSessionID, writeReferenceSession } from "../scripts/opencode-reference-session.mjs";

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..");
const appRoot = path.join(repoRoot, "apps", "tui_new");
const screenshotRoot = path.join(repoRoot, "target", "tui-new-compare", String(Date.now()));
const webBin = path.join(appRoot, "scripts", "web-terminal.mjs");
const reference = "C:\\Users\\liuliu\\Documents\\opencode-dev\\screenshot.png";
const opencodeRoot = process.env.OPENCODE_DEV_ROOT || "C:\\Users\\liuliu\\Documents\\opencode-dev";
const opencodePackageRoot = path.join(opencodeRoot, "packages", "opencode");
const bunBin = process.env.BUN_BIN || "C:\\Users\\liuliu\\.bun\\bin\\bun.exe";
const port = Number(process.env.PORT || "8899");
const tuiReadyPattern = /Homepage button|Find the homepage|Ask anything|Build/i;

async function pngSize(file) {
  const buffer = await fs.readFile(file);
  assert.equal(buffer.toString("ascii", 1, 4), "PNG", `${file} is not a PNG`);
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20), bytes: buffer.length };
}

async function waitForUrl(url, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  throw new Error(`timed out waiting for ${url}`);
}

async function runCommand(command, args, options) {
  const child = spawn(command, args, options);
  let output = "";
  child.stdout?.on("data", (chunk) => (output += chunk.toString()));
  child.stderr?.on("data", (chunk) => (output += chunk.toString()));
  const code = await new Promise((resolve) => child.once("exit", resolve));
  if (code !== 0) throw new Error(`${command} ${args.join(" ")} failed with ${code}\n${output}`);
  return output;
}

async function assertTerminalFits(page, label) {
  const metrics = await page.evaluate(() => {
    const body = document.body;
    const terminal = document.querySelector("#terminal");
    const viewport = document.querySelector(".xterm-viewport");
    return {
      bodyClientWidth: body.clientWidth,
      bodyScrollWidth: body.scrollWidth,
      terminalClientWidth: terminal?.clientWidth ?? 0,
      terminalScrollWidth: terminal?.scrollWidth ?? 0,
      viewportClientWidth: viewport?.clientWidth ?? 0,
      viewportScrollWidth: viewport?.scrollWidth ?? 0,
    };
  });
  assert.ok(
    metrics.bodyScrollWidth <= metrics.bodyClientWidth + 2 &&
      metrics.terminalScrollWidth <= metrics.terminalClientWidth + 2 &&
      metrics.viewportScrollWidth <= metrics.viewportClientWidth + 2,
    `${label} horizontal overflow: ${JSON.stringify(metrics)}`,
  );
}

async function waitForTerminalText(page, pattern, timeout = 25_000) {
  await page.waitForFunction(
    (source) => {
      const terminal = document.querySelector(".xterm-rows")?.textContent ?? "";
      return new RegExp(source, "i").test(terminal);
    },
    pattern.source,
    { timeout },
  );
  await page.waitForTimeout(500);
}

async function prepareReferenceSession(env) {
  const fixture = path.join(screenshotRoot, "opencode-reference-session.json");
  await writeReferenceSession(fixture, opencodeRoot);
  const output = await runCommand(bunBin, ["run", "--conditions=browser", "./src/index.ts", "--pure", "import", fixture], {
    cwd: opencodePackageRoot,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  assert.match(output, new RegExp(`Imported session: ${referenceSessionID}`));
  return fixture;
}

async function main() {
  await fs.access(reference);
  await fs.mkdir(screenshotRoot, { recursive: true });
  const referenceSize = await pngSize(reference);
  const dbPath = path.join(screenshotRoot, "opencode-reference.db");
  const env = { ...process.env, OPENCODE_DB: dbPath, OPENCODE_DISABLE_CHANNEL_DB: "1", TURA_CWD: opencodeRoot };
  const fixture = await prepareReferenceSession(env);
  const child = spawn(process.execPath, [webBin], {
    cwd: repoRoot,
    env: {
      ...env,
      PORT: String(port),
      TURA_TUI_SOURCE: "opencode",
      TURA_OPENCODE_SESSION: referenceSessionID,
      TURA_OPENCODE_MODEL: "opencode/claude-opus-4-5",
      TURA_OPENCODE_AGENT: "build",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let logs = "";
  child.stdout.on("data", (chunk) => (logs += chunk.toString()));
  child.stderr.on("data", (chunk) => (logs += chunk.toString()));
  try {
    await waitForUrl(`http://127.0.0.1:${port}/`);
    const browser = await chromium.launch();
    try {
      const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
      for (const level of ["l1", "l2", "l3"]) {
        await page.goto(`http://127.0.0.1:${port}/${level}?instance=${level}-desktop`, { waitUntil: "domcontentloaded" });
        await waitForTerminalText(page, tuiReadyPattern);
        await page.screenshot({ path: path.join(screenshotRoot, `${level}-desktop.png`) });
        await assertTerminalFits(page, `${level} desktop`);
      }
      await page.goto(`http://127.0.0.1:${port}/compare`, { waitUntil: "domcontentloaded" });
      await page.waitForFunction(
        (pattern) => {
          const terminal = document.querySelector("iframe")?.contentDocument?.querySelector(".xterm-rows")?.textContent ?? "";
          return new RegExp(pattern, "i").test(terminal);
        },
        tuiReadyPattern.source,
        { timeout: 25_000 },
      );
      await page.waitForTimeout(500);
      await page.screenshot({ path: path.join(screenshotRoot, "opencode-side-by-side.png") });
      await page.setViewportSize({ width: referenceSize.width, height: referenceSize.height });
      await page.goto(`http://127.0.0.1:${port}/l3?instance=l3-reference-size`, { waitUntil: "domcontentloaded" });
      await waitForTerminalText(page, tuiReadyPattern);
      const referenceSized = path.join(screenshotRoot, "l3-reference-size.png");
      await page.screenshot({ path: referenceSized });
      await assertTerminalFits(page, "l3 reference size");
      await page.setViewportSize({ width: 390, height: 640 });
      await page.goto(`http://127.0.0.1:${port}/l3?instance=l3-mobile`, { waitUntil: "domcontentloaded" });
      await waitForTerminalText(page, tuiReadyPattern);
      await page.screenshot({ path: path.join(screenshotRoot, "l3-mobile.png") });
      await assertTerminalFits(page, "l3 mobile");
      await fs.writeFile(
        path.join(screenshotRoot, "metadata.json"),
        JSON.stringify({ reference: referenceSize, l3ReferenceSize: await pngSize(referenceSized), source: "opencode", session: referenceSessionID, fixture, viewportChecks: ["l1-desktop", "l2-desktop", "l3-desktop", "l3-reference-size", "l3-mobile"] }, null, 2),
      );
    } finally {
      await browser.close();
    }
  } finally {
    child.kill();
    await new Promise((resolve) => child.once("exit", resolve));
    await fs.writeFile(path.join(screenshotRoot, "web-terminal.log"), logs);
  }
  console.log(`[tui-new-compare] screenshots=${screenshotRoot}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
