#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";

const appRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(appRoot, "..", "..", "..");
const runId = process.env.GUI_PLAN_PIPELINE_E2E_RUN_ID || `plan-pipeline-${Date.now()}`;
const runRoot = path.join(repoRoot, "target", "gui-plan-pipeline-e2e", runId);
const workspace = path.join(runRoot, "workspace");
const appWorkspace = workspace;
const summaryPath = path.join(runRoot, "summary.json");
const gatewayExe = path.join(
  repoRoot,
  "target",
  "debug",
  process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway",
);
const nodeRequire = createRequire(path.join(repoRoot, "apps", "tui", "package.json"));
const { chromium } = nodeRequire("playwright");

const checks = [];
let gateway;
let vite;
const browserEvents = [];

function record(name, ok, details = {}) {
  checks.push({ name, ok, ...details });
  if (!ok) {
    throw new Error(`${name} failed: ${JSON.stringify(details)}`);
  }
}

function freePort() {
  return 21_000 + Math.floor(Math.random() * 20_000);
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
  if (!child || child.exitCode !== null || child.killed) {
    return;
  }
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
      if (response.ok) {
        return response;
      }
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw lastError ?? new Error(`timed out waiting for ${url}`);
}

async function requestJson(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: {
      "content-type": "application/json",
      ...(options.headers || {}),
    },
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${url} returned ${response.status}: ${text}`);
  }
  return text ? JSON.parse(text) : null;
}

async function createPlanSession(gatewayUrl) {
  const taskManagement = {
    tasks: [
      {
        task_id: "pipeline-task-alpha",
        step: 1,
        status: "todo",
        start_condition: "user_action",
        task_summary: `Alpha queued task ${runId}`,
      },
      {
        task_id: "pipeline-task-scheduled",
        step: 2,
        status: "todo",
        start_condition: "scheduled_task",
        start_at: "2099-06-08T10:00:00Z",
        poll_interval: { m: 0, d: 0, h: 0, s: 0 },
        task_summary: `Scheduled task ${runId}`,
      },
      {
        task_id: "pipeline-task-bravo",
        step: 3,
        status: "todo",
        start_condition: "user_action",
        task_summary: `Bravo queued task ${runId}`,
      },
      {
        task_id: "pipeline-task-polling",
        step: 4,
        status: "todo",
        start_condition: "polling_task",
        start_at: "2099-06-08T11:00:00Z",
        poll_interval: { m: 30, d: 0, h: 0, s: 0 },
        task_summary: `Polling task ${runId}`,
      },
      {
        task_id: "pipeline-task-charlie",
        step: 5,
        status: "todo",
        start_condition: "user_action",
        task_summary: `Charlie queued task ${runId}`,
      },
    ],
  };
  return requestJson(`${gatewayUrl}/session`, {
    method: "POST",
    body: JSON.stringify({
      directory: appWorkspace,
      auto_session_name: false,
      task_management: taskManagement,
    }),
  });
}

async function dragTaskBefore(page, sourceText, targetText) {
  const source = page.locator(".plan-pipeline-task", { hasText: sourceText });
  const target = page.locator(".plan-pipeline-task", { hasText: targetText });
  const track = target.locator("xpath=ancestor::*[contains(@class, 'plan-pipeline-track')][1]");
  const sourceBox = await source.boundingBox();
  const targetBox = await target.boundingBox();
  const trackBox = await track.boundingBox();
  assert(sourceBox, `missing source box for ${sourceText}`);
  assert(targetBox, `missing target box for ${targetText}`);
  assert(trackBox, `missing track box for ${targetText}`);

  const stepCount = await page.locator(".plan-pipeline-step").count();
  const stepWidth = trackBox.width / stepCount;
  await page.mouse.move(sourceBox.x + sourceBox.width / 2, sourceBox.y + sourceBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(trackBox.x + stepWidth / 2, targetBox.y + targetBox.height / 2, {
    steps: 12,
  });
  await page.mouse.up();
}

async function openAppPlanPage(page, appUrl) {
  let lastError;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      await page.goto(appUrl, { waitUntil: "domcontentloaded", timeout: 30_000 });
      await page.waitForSelector(".plan-board", { timeout: 20_000 });
      return;
    } catch (error) {
      lastError = error;
      await page.waitForTimeout(500);
    }
  }
  throw lastError;
}

async function main() {
  await fs.mkdir(workspace, { recursive: true });
  const build = spawnSync("cargo", ["build", "-p", "gateway", "--bin", "tura_gateway"], {
    cwd: repoRoot,
    encoding: "utf8",
    timeout: 240_000,
    windowsHide: true,
  });
  if (build.status !== 0 || !existsSync(gatewayExe)) {
    throw new Error(`gateway build failed\n${build.stdout}\n${build.stderr}`);
  }

  const gatewayPort = freePort();
  const vitePort = freePort();
  const gatewayUrl = `http://127.0.0.1:${gatewayPort}`;
  gateway = startProcess(gatewayExe, [], {
    cwd: workspace,
    env: { PORT: String(gatewayPort), TURA_HOME: path.join(runRoot, ".tura") },
  });
  await waitForUrl(`${gatewayUrl}/global/health`, 45_000);
  record("real-gateway-health", true, { gatewayUrl });

  const session = await createPlanSession(gatewayUrl);
  record("fixture-session-created", Boolean(session?.id), { sessionId: session?.id });

  vite = startProcess(
    process.execPath,
    [
      path.join(appRoot, "node_modules", "vite", "bin", "vite.js"),
      "--host",
      "127.0.0.1",
      "--port",
      String(vitePort),
      "--strictPort",
    ],
    { cwd: appRoot, env: { VITE_TURA_GATEWAY_URL: gatewayUrl } },
  );
  await waitForUrl(`http://127.0.0.1:${vitePort}`, 45_000);
  record("vite-ready", true, { vitePort });

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1360, height: 820 } });
  await page.addInitScript(() => {
    window.addEventListener("unhandledrejection", (event) => {
      console.error(
        `[unhandledrejection] ${event.reason?.stack || event.reason?.message || event.reason}`,
      );
    });
  });
  page.on("console", (message) => {
    browserEvents.push({ type: "console", level: message.type(), text: message.text() });
  });
  page.on("pageerror", (error) => {
    browserEvents.push({ type: "pageerror", text: error.stack || error.message });
  });
  page.on("response", async (response) => {
    if (!response.url().includes("/task-management")) {
      return;
    }
    browserEvents.push({
      type: "response",
      url: response.url(),
      status: response.status(),
      text: await response.text().catch(() => ""),
    });
  });
  const appUrl = `http://127.0.0.1:${vitePort}/?tab=plan&gatewayUrl=${encodeURIComponent(gatewayUrl)}`;
  try {
    await openAppPlanPage(page, appUrl);
    await page.locator(".plan-mode-actions button").first().click();
    await page.waitForSelector(".plan-pipeline-task", { timeout: 10_000 });

    const visibleText = await page.locator(".plan-pipeline").innerText();
    const filterDetails = {
      hasAlpha: visibleText.includes(`Alpha queued task ${runId}`),
      hasBravo: visibleText.includes(`Bravo queued task ${runId}`),
      hasCharlie: visibleText.includes(`Charlie queued task ${runId}`),
      hasScheduled: visibleText.includes(`Scheduled task ${runId}`),
      hasPolling: visibleText.includes(`Polling task ${runId}`),
    };
    record(
      "pipeline-shows-all-task-types",
      Object.values(filterDetails).every(Boolean),
      filterDetails,
    );
    assert.match(visibleText, new RegExp(`Alpha queued task ${runId}`));
    assert.match(visibleText, new RegExp(`Bravo queued task ${runId}`));
    assert.match(visibleText, new RegExp(`Charlie queued task ${runId}`));
    assert.match(visibleText, new RegExp(`Scheduled task ${runId}`));
    assert.match(visibleText, new RegExp(`Polling task ${runId}`));

    await dragTaskBefore(page, `Charlie queued task ${runId}`, `Alpha queued task ${runId}`);
    await page.waitForTimeout(1_000);

    const updated = await requestJson(
      `${gatewayUrl}/session?directory=${encodeURIComponent(appWorkspace)}&includeChildren=true`,
    );
    const updatedSession = updated.find((item) => item.id === session.id);
    const ordered = updatedSession?.task_management?.tasks?.map((task) => [
      task.task_id,
      task.step,
    ]);
    record("drag-persists-step-order", true, { ordered });
    assert.deepEqual(ordered, [
      ["pipeline-task-charlie", 1],
      ["pipeline-task-alpha", 2],
      ["pipeline-task-scheduled", 3],
      ["pipeline-task-bravo", 4],
      ["pipeline-task-polling", 5],
    ]);
    assert.equal(ordered?.length, 5);
    const visibleDomOrder = await page
      .locator(".plan-pipeline-task")
      .evaluateAll((items) => items.map((item) => item.getAttribute("data-task-nonce")));
    assert.deepEqual(visibleDomOrder, [
      "pipeline-task-charlie",
      "pipeline-task-alpha",
      "pipeline-task-scheduled",
      "pipeline-task-bravo",
      "pipeline-task-polling",
    ]);

    const screenshotPath = path.join(runRoot, "plan-pipeline.png");
    await page.screenshot({ path: screenshotPath, fullPage: true });
    record("playwright-screenshot", true, { screenshotPath });
  } catch (error) {
    await fs.mkdir(runRoot, { recursive: true });
    await fs.writeFile(
      path.join(runRoot, "browser-events.json"),
      JSON.stringify(browserEvents, null, 2),
    );
    await fs.writeFile(
      path.join(runRoot, "page-body.html"),
      await page
        .locator("body")
        .evaluate((body) => body.innerHTML)
        .catch(() => ""),
    );
    await page
      .screenshot({ path: path.join(runRoot, "failure-page.png"), fullPage: true })
      .catch(() => {});
    throw error;
  } finally {
    await browser.close();
  }
}

const startedAt = Date.now();
try {
  await main();
  await fs.mkdir(runRoot, { recursive: true });
  const summary = { ok: true, duration_ms: Date.now() - startedAt, checks };
  await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
} catch (error) {
  await fs.mkdir(runRoot, { recursive: true });
  const logs = {
    gateway: gateway?.logs?.(),
    vite: vite?.logs?.(),
    browser: browserEvents,
  };
  await fs.writeFile(path.join(runRoot, "failure-logs.json"), JSON.stringify(logs, null, 2));
  const summary = {
    ok: false,
    duration_ms: Date.now() - startedAt,
    error: error instanceof Error ? error.stack || error.message : String(error),
    checks,
    logs_path: path.join(runRoot, "failure-logs.json"),
  };
  await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
  console.error(JSON.stringify(summary, null, 2));
  process.exitCode = 1;
} finally {
  stopProcess(vite);
  stopProcess(gateway);
}
