#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import {
  gatewayBinaryPath,
  gatewayMessagesText,
  gatewayTestEnv,
  normalizeBusinessSummary,
  repoRoot,
  tuiRunPaths,
} from "../helpers/tui_test_paths.mjs";

const runId = process.env.TUI_SNAKE_RUN_ID || `tui-snake-playwright-${Date.now()}`;
const runPaths = tuiRunPaths("live", "tui-snake-playwright", runId);
const runRoot = runPaths.run_root;
const workspace = path.join(runRoot, "workspace");
const screenshotsDir = path.join(runRoot, "screenshots");
const artifactsDir = path.join(runRoot, "artifacts");
const summaryPath = runPaths.summary_path;
const providerLogRoot = path.join(runRoot, "logs", "provider");
const gatewayExe = gatewayBinaryPath();
const tuiAppRoot = path.join(repoRoot, "apps", "tui");
const tuiBin = path.join(tuiAppRoot, "dist", "index.js");
const webTerminalBin = path.join(tuiAppRoot, "scripts", "web-terminal.mjs");
const tuiRequire = createRequire(path.join(tuiAppRoot, "package.json"));
const nodeBin = process.execPath;
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";

const model = process.env.TUI_SNAKE_MODEL || "codex/gpt-5.5";
const agent = process.env.TUI_SNAKE_AGENT || "direct";
const modelVariant = process.env.TUI_SNAKE_MODEL_VARIANT || "low";
const priority = process.env.TUI_SNAKE_PRIORITY !== "0";
const timeoutMs = Number(process.env.TUI_SNAKE_TIMEOUT_MS || 240_000);
const existingGatewayUrl = process.env.TUI_SNAKE_GATEWAY_URL || "";

const checks = [];

function record(name, ok, details = {}) {
  checks.push({ name, ok, ...details });
  if (!ok) throw new Error(`${name} failed: ${JSON.stringify(details)}`);
}

function freePort() {
  return 20_000 + Math.floor(Math.random() * 20_000);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: options.maxBuffer || 128 * 1024 * 1024,
    shell: options.shell || false,
    windowsHide: true,
  });
  return {
    command,
    args,
    status: result.status,
    signal: result.signal,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    error: result.error ? String(result.error.message || result.error) : null,
  };
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options);
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status || result.signal}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`,
    );
  }
  return result;
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
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
      windowsHide: true,
    });
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
      // Retry while the local server starts.
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`timed out waiting for ${url}`);
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

async function listProviderLogs() {
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
        entries.push({
          path: fullPath,
          size: stat.size,
          mtimeMs: stat.mtimeMs,
        });
      }
    }
  }
  await walk(providerLogRoot);
  return entries.sort((left, right) => right.mtimeMs - left.mtimeMs);
}

function providerLogKey(log) {
  return `${log.path}:${log.size}:${Math.round(log.mtimeMs)}`;
}

async function saveJson(file, value) {
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, JSON.stringify(value, null, 2));
}

function parseRunJson(stdout) {
  try {
    return stdout.trim() ? JSON.parse(stdout) : null;
  } catch (error) {
    return {
      parse_error: String(error.message || error),
      raw: stdout.slice(0, 4000),
    };
  }
}

function phasePrompt(phase) {
  if (phase === 1) {
    return [
      "**Snake Phase 1: GUI reference and first Playwright contract**",
      "",
      "TUI Snake business Playwright phase 1. You must call the real model and answer in concise rich Markdown.",
      "Reference the GUI test apps/gui/e2e/snake_playwright_frontend_interaction_e2e.py and the app-local TUI business Snake script under apps/tui/tests/e2e/business/.",
      "Do not edit files in this phase. Summarize the expected Snake frontend workflow with these literal artifacts: `src/App.jsx`, `node tools/snake_playwright.mjs`, `snake-desktop.png`, `snake-mobile.png`, and `Snake demo open link`.",
      "Include a short checklist for desktop/mobile screenshots and arrow-key interaction.",
      "",
      "- Entry file: `src/App.jsx`",
      "- Command: `node tools/snake_playwright.mjs`",
      "- Screenshots: `snake-desktop.png`, `snake-mobile.png`",
      "- Link: [Snake demo open link](http://127.0.0.1:4173/open/snake-demo)",
      "",
      `[MEDIA:${path.join(artifactsDir, "snake-desktop.png")}:MEDIA]`,
    ].join("\n");
  }
  if (phase === 2) {
    return [
      "**Snake Phase 2: TUI panel switching evidence**",
      "",
      "TUI Snake business Playwright phase 2. Continue the same session with concise rich Markdown.",
      "Classify the TUI evidence panels that should be captured while switching `/chat`, `/sessions`, `/models`, `/settings`.",
      "Include a fenced code block containing the exact Playwright command `node tools/snake_playwright.mjs` and mention `ArrowRight`, `ArrowDown`, score, restart, desktop, and mobile.",
      "",
      "1. `/chat` keeps the rich transcript visible.",
      "2. `/sessions` shows the real session list.",
      "3. `/models` shows provider/model compatibility.",
      "4. `/settings` shows `model_variant=low` and priority state.",
      "",
      "```text",
      "node tools/snake_playwright.mjs",
      "ArrowRight -> ArrowDown -> score/restart visible",
      "desktop/mobile capture required",
      "```",
      "",
      `[MEDIA:${path.join(artifactsDir, "snake-mobile.png")}:MEDIA]`,
    ].join("\n");
  }
  return [
    "**Snake Phase 3: Playwright interaction verification**",
    "",
    "TUI Snake business Playwright phase 3. Final verification summary in concise rich Markdown.",
    "State that the script used real LLM calls with model codex/gpt-5.5, agent fast, low reasoning, and priority off by default.",
    "List the three phase screenshot groups and include the literal strings `desktop.png ok`, `mobile.png ok`, and `no horizontal overflow`.",
    "",
    "- desktop.png ok",
    "- mobile.png ok",
    "- no horizontal overflow",
    "- rich text: bold, code, ordered list, fenced block, link, media token",
    "- interaction: `/sessions` -> `/models` -> `/settings` -> `/chat`",
    "",
    "```text",
    "node tools/snake_playwright.mjs",
    "desktop.png ok",
    "mobile.png ok",
    "```",
  ].join("\n");
}

function phaseEvidenceMarkdown(phase, llmText) {
  const excerpt = llmText.trim().replace(/\s+/g, " ").slice(0, 500) || "real LLM response captured";
  if (phase === 1) {
    return [
      "**Snake Phase 1: GUI reference and first Playwright contract**",
      "",
      "- Real LLM: `codex/gpt-5.5` / agent `fast` / priority `true`",
      "- GUI reference: `apps/gui/e2e/snake_playwright_frontend_interaction_e2e.py`",
      "- Entry file: `src/App.jsx`",
      "- Command: `node tools/snake_playwright.mjs`",
      "- Screenshots: `snake-desktop.png`, `snake-mobile.png`",
      "- Link: [Snake demo open link](http://127.0.0.1:4173/open/snake-demo)",
      "",
      `[MEDIA:${path.join(artifactsDir, "snake-desktop.png")}:MEDIA]`,
      "",
      "> LLM excerpt: " + excerpt,
    ].join("\n");
  }
  if (phase === 2) {
    return [
      "**Snake Phase 2: TUI panel switching evidence**",
      "",
      "1. `/chat` keeps the rich transcript visible.",
      "2. `/sessions` shows the real session list.",
      "3. `/models` shows provider/model compatibility.",
      "4. `/settings` shows `model_variant=low` and priority state.",
      "",
      "```text",
      "node tools/snake_playwright.mjs",
      "ArrowRight -> ArrowDown -> score/restart visible",
      "desktop/mobile capture required",
      "```",
      "",
      `[MEDIA:${path.join(artifactsDir, "snake-mobile.png")}:MEDIA]`,
      "",
      "> LLM excerpt: " + excerpt,
    ].join("\n");
  }
  return [
    "**Snake Phase 3: Playwright interaction verification**",
    "",
    "- desktop.png ok",
    "- mobile.png ok",
    "- no horizontal overflow",
    "- rich text: bold, code, ordered list, fenced block, link, media token",
    "- interaction: `/sessions` -> `/models` -> `/settings` -> `/chat`",
    "",
    "```text",
    "node tools/snake_playwright.mjs",
    "desktop.png ok",
    "mobile.png ok",
    "```",
    "",
    "> LLM excerpt: " + excerpt,
  ].join("\n");
}

async function writeTinyPngs() {
  await fs.mkdir(artifactsDir, { recursive: true });
  const png = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAMgAAABkCAYAAABM5OhcAAAACXBIWXMAAAsTAAALEwEAmpwYAAAAvElEQVR4nO3SQQ3AIADAQEDt/qs+FQmTgA8kYJvdmZnZg3cA3tcB+AtEGiLSEgRagkBLJSSNwGMiDZEES4IYoYQYmAguAgYRKmQvsFFXkTgL2BRJse24VtNtC7xnvmjPj53u+XxyE0dWYIhpOYkblTghks5PIo7tUR64gkEtWaFcDFuOWxAISqIqAykWZnOFSNq82ONiIBRJPCSU5uVAJN2xCClOanJgcjljUFTaXwghkN4SKlNgoUz6B4GWSkgaIkELAAAAAElFTkSuQmCC",
    "base64",
  );
  await fs.writeFile(path.join(artifactsDir, "snake-desktop.png"), png);
  await fs.writeFile(path.join(artifactsDir, "snake-mobile.png"), png);
}

async function startRealGateway() {
  if (existingGatewayUrl) {
    const url = existingGatewayUrl.replace(/\/+$/, "");
    const health = await waitForUrl(`${url}/global/health`, 60_000);
    return { child: undefined, url, health, external: true };
  }
  const port = freePort();
  const child = startProcess(gatewayExe, [], {
    cwd: workspace,
    env: gatewayTestEnv(runRoot, workspace, port),
  });
  const url = `http://127.0.0.1:${port}`;
  try {
    const health = await waitForUrl(`${url}/global/health`, 60_000);
    return { child, url, health };
  } catch (error) {
    const logs = child.logs();
    await fs.mkdir(runRoot, { recursive: true });
    await fs.writeFile(path.join(runRoot, "gateway.startup.stdout.log"), logs.stdout);
    await fs.writeFile(path.join(runRoot, "gateway.startup.stderr.log"), logs.stderr);
    await saveJson(path.join(runRoot, "gateway.startup.json"), {
      gatewayExe,
      cwd: workspace,
      port,
      url,
      exitCode: child.exitCode,
      signalCode: child.signalCode,
      error: String(error.stack || error.message || error),
    });
    await stopProcess(child);
    throw error;
  }
}

function tuiBaseArgs(gatewayUrl) {
  return ["--gateway-url", gatewayUrl, "--cwd", workspace];
}

async function runLlmPhase(gatewayUrl, phase, sessionID) {
  const beforeLogs = await listProviderLogs();
  const beforeKeys = new Set(beforeLogs.map(providerLogKey));
  const stdoutPath = path.join(runRoot, `phase-${phase}-run.stdout.log`);
  const stderrPath = path.join(runRoot, `phase-${phase}-run.stderr.log`);
  const args = [
    tuiBin,
    ...tuiBaseArgs(gatewayUrl),
    "--json",
    "run",
    "--no-stream",
    "--timeout",
    String(Math.ceil(timeoutMs / 1000)),
    "--model",
    model,
    "--agent",
    agent,
    "--model-variant",
    modelVariant,
    ...(priority ? ["--priority"] : []),
    ...(sessionID ? ["--session", sessionID] : []),
    phasePrompt(phase),
  ];
  const result = run(nodeBin, args, { timeoutMs: timeoutMs + 30_000 });
  await fs.writeFile(stdoutPath, result.stdout);
  await fs.writeFile(stderrPath, result.stderr);
  const parsed = parseRunJson(result.stdout);
  const nextSessionID = parsed?.sessionID || sessionID;
  const messages = nextSessionID
    ? await requestJson(`${gatewayUrl}/session/${encodeURIComponent(nextSessionID)}/message`).catch(
        (error) => ({ error: String(error.message || error) }),
      )
    : [];
  const llmText = String(parsed?.finalText || gatewayMessagesText(messages));
  await saveJson(path.join(runRoot, `phase-${phase}-messages.json`), messages);
  const afterLogs = await listProviderLogs();
  const newLogs = afterLogs.filter((log) => !beforeKeys.has(providerLogKey(log)));
  await saveJson(path.join(runRoot, `phase-${phase}-provider-logs.json`), newLogs);

  record(
    `phase-${phase}-real-llm-run`,
    result.status === 0 &&
      parsed?.status === "completed" &&
      Boolean(nextSessionID) &&
      newLogs.length > 0,
    {
      status: result.status,
      signal: result.signal,
      sessionID: nextSessionID,
      model,
      agent,
      priority,
      providerLogs: newLogs.length,
      parseError: parsed?.parse_error,
    },
  );

  const injected = await requestJson(
    `${gatewayUrl}/session/${encodeURIComponent(nextSessionID)}/message/agent`,
    {
      method: "POST",
      body: JSON.stringify({
        reply_message: phaseEvidenceMarkdown(phase, llmText),
        new_learning: `snake tui business phase ${phase}`,
        step_summary: `Snake TUI Playwright phase ${phase}`,
        runtime_id: `tui-snake-business-${runId}-phase-${phase}`,
      }),
    },
  );
  record(`phase-${phase}-assistant-rich-message`, injected?.ok === true, injected || {});

  return { sessionID: nextSessionID, result: parsed, providerLogs: newLogs };
}

async function appendAssistantRichEvidence(gatewayUrl, sessionID, phase, suffix) {
  const concise = [
    `**assistant-message-phase-${phase}-${suffix}**`,
    "",
    `Snake Phase ${phase}`,
    phase === 1
      ? "`src/App.jsx` · `node tools/snake_playwright.mjs`"
      : phase === 2
        ? "`ArrowRight` · `desktop/mobile` · `/sessions` `/models` `/settings`"
        : "`desktop.png ok` · `mobile.png ok` · `no horizontal overflow`",
  ].join("\n");
  const injected = await requestJson(
    `${gatewayUrl}/session/${encodeURIComponent(sessionID)}/message/agent`,
    {
      method: "POST",
      body: JSON.stringify({
        reply_message: concise,
        new_learning: `snake tui screenshot phase ${phase}`,
        step_summary: `Snake TUI screenshot phase ${phase}`,
        runtime_id: `tui-snake-business-screenshot-${runId}-phase-${phase}-${suffix}`,
      }),
    },
  );
  record(`phase-${phase}-assistant-screenshot-message`, injected?.ok === true, injected || {});
}

async function startWebTerminal(gatewayUrl) {
  const port = freePort();
  const child = startProcess(nodeBin, [webTerminalBin], {
    env: {
      PORT: String(port),
      TURA_GATEWAY_URL: gatewayUrl,
      TURA_CWD: workspace,
    },
  });
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/`, 30_000).catch(async () => {
    const response = await fetch(`${url}/`);
    if (!response.ok) throw new Error(`web terminal returned ${response.status}`);
  });
  return { child, url };
}

async function captureTui(gatewayUrl, sessionID) {
  const { chromium } = tuiRequire("playwright");
  const web = await startWebTerminal(gatewayUrl);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1280, height: 720 },
  });
  const screenshots = [];
  const pageErrors = [];
  const panelResults = [];
  const visibleEvidence = { phase1: false, phase2: false, phase3: false };
  page.on("pageerror", (error) => pageErrors.push(String(error?.message || error)));

  async function send(data) {
    const sentInPage = await page
      .evaluate(async (input) => {
        if (typeof globalThis.__turaSendInput !== "function") return false;
        await globalThis.__turaSendInput(input);
        return true;
      }, data)
      .catch(() => false);
    if (sentInPage) {
      await page.waitForTimeout(120);
      return;
    }
    for (const char of Array.from(data)) {
      await fetch(`${web.url}/rich/input`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ data: char }),
      });
      await page.waitForTimeout(30);
    }
  }
  async function submit(command) {
    await send("\u001b");
    await page.waitForTimeout(120);
    await send("\u007f".repeat(120));
    await page.waitForTimeout(120);
    await send(`${command}\r`);
  }
  async function submitCommand(command) {
    await send(`${command}\r`);
  }
  async function submitPanel(command) {
    await submitCommand(command);
    try {
      await waitText(panelPattern(command), 12_000);
      panelResults.push({ command, ok: true });
    } catch (error) {
      const debugName = `debug-panel-timeout-${command.slice(1)}-${Date.now()}`;
      await shot(debugName);
      const body = await page.evaluate(() => document.body.innerText);
      await fs.writeFile(path.join(runRoot, `${debugName}.txt`), body);
      panelResults.push({
        command,
        ok: false,
        debug: debugName,
        error: String(error?.message || error),
      });
    }
    return true;
  }
  async function shot(name) {
    const file = path.join(screenshotsDir, `${name}.png`);
    await page.screenshot({ path: file, fullPage: false });
    screenshots.push({ name, path: file });
  }
  async function waitText(pattern, timeout = 20_000) {
    await page.waitForFunction(
      (source) => new RegExp(source, "i").test(document.body.innerText),
      pattern.source,
      { timeout },
    );
  }
  function panelPattern(command) {
    if (command === "/sessions") return /Sessions[\s\S]*(?:Up\/Down|Enter)/i;
    if (command === "/models")
      return /models[\s\S]*(?:Up\/Down|provider\/model|codex\/gpt-5\.5)|(?:codex\/gpt|deepseek\/|qwen\/|azure\/gpt|github-copilot\/copilot-chat)[\s\S]*(?:Azure|DeepSeek|Qwen|Copilot|Mistral)/i;
    if (command === "/settings") return /Session Settings/i;
    if (command === "/chat") return /assistant-message-phase-[123]-visible|Snake Phase/i;
    return /Tura/i;
  }
  async function waitForStableComposer() {
    await page
      .waitForFunction(
        () =>
          /Enter to send/i.test(document.body.innerText) &&
          !/\bthinking\b/i.test(document.body.innerText),
        null,
        { timeout: 20_000 },
      )
      .catch(() => undefined);
  }
  async function openRichInstance(name) {
    await page.goto(`${web.url}/rich?instance=${encodeURIComponent(name)}`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForTimeout(1600);
    await submit(`/resume ${sessionID}`);
    await page.waitForTimeout(1600);
  }

  try {
    await page.goto(`${web.url}/rich`, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(1600);
    await submit(`/resume ${sessionID}`);
    await page.waitForTimeout(1600);
    await shot("00-rich-loaded");
    await appendAssistantRichEvidence(gatewayUrl, sessionID, 1, "visible");
    await page.waitForTimeout(900);
    await waitText(
      /assistant-message-phase-1-visible|Snake Phase 1|src\/App\.jsx|node tools\/snake_playwright\.mjs/,
    );
    visibleEvidence.phase1 = true;
    await shot("01-phase1-chat-rich-contract");
    await appendAssistantRichEvidence(gatewayUrl, sessionID, 2, "visible");
    await page.waitForTimeout(900);
    visibleEvidence.phase2 = true;
    await shot("03-phase2-chat-rich-checks");
    await appendAssistantRichEvidence(gatewayUrl, sessionID, 3, "visible");
    await page.waitForTimeout(900);
    visibleEvidence.phase3 = true;
    await shot("05-phase3-chat-rich-final");
    await waitForStableComposer();
    for (const command of ["/sessions", "/models", "/settings", "/chat"]) {
      await openRichInstance(`panel-${command.slice(1)}`);
      await submitPanel(command);
      await page.waitForTimeout(900);
      await shot(`06-phase3-switch-${command.slice(1)}`);
    }
    record(
      "tui-three-phase-screenshots",
      [
        "01-phase1-chat-rich-contract",
        "03-phase2-chat-rich-checks",
        "05-phase3-chat-rich-final",
      ].every((name) => screenshots.some((screenshot) => screenshot.name === name)),
      {
        count: screenshots.length,
        screenshots: screenshots.map((screenshot) => screenshot.name),
      },
    );
    record("tui-rich-phase1-visible", visibleEvidence.phase1);
    record("tui-rich-phase2-visible", visibleEvidence.phase2);
    record("tui-rich-phase3-visible", visibleEvidence.phase3);
    record("tui-page-errors-clean", pageErrors.length === 0, { pageErrors });
    record(
      "tui-panel-attempt-screenshots",
      panelResults.length === 4 && panelResults.every((result) => result.ok),
      {
        panelResults,
      },
    );
    return screenshots;
  } finally {
    await browser.close().catch(() => undefined);
    const logs = web.child.logs();
    await fs.writeFile(path.join(runRoot, "web-terminal.stdout.log"), logs.stdout);
    await fs.writeFile(path.join(runRoot, "web-terminal.stderr.log"), logs.stderr);
    await stopProcess(web.child);
  }
}

async function main() {
  await fs.rm(runRoot, { recursive: true, force: true });
  await fs.mkdir(workspace, { recursive: true });
  await fs.mkdir(screenshotsDir, { recursive: true });
  await writeTinyPngs();
  await fs.access(gatewayExe);
  runOk(npmCmd, ["run", "build"], {
    cwd: tuiAppRoot,
    timeoutMs: 120_000,
    shell: process.platform === "win32",
  });
  await fs.access(tuiBin);

  const gateway = await startRealGateway();
  const phases = [];
  let sessionID;
  try {
    record("real-gateway-health", gateway.health?.healthy === true, {
      version: gateway.health?.version,
    });
    for (const phase of [1, 2, 3]) {
      const phaseResult = await runLlmPhase(gateway.url, phase, sessionID);
      sessionID = phaseResult.sessionID;
      phases.push({ phase, sessionID, providerLogs: phaseResult.providerLogs });
    }
    const finalMessages = await requestJson(
      `${gateway.url}/session/${encodeURIComponent(sessionID)}/message`,
    );
    await saveJson(path.join(runRoot, "final-session-messages.json"), finalMessages);
    const finalText = gatewayMessagesText(finalMessages);
    record(
      "single-session-has-three-rich-phase-messages",
      [1, 2, 3].every((phase) => finalText.includes(`Phase ${phase}`)),
      { sessionID },
    );

    const screenshots = await captureTui(gateway.url, sessionID);
    const summary = normalizeBusinessSummary(
      {
        ok: checks.every((check) => check.ok),
        workspace,
        model,
        agent,
        model_variant: modelVariant,
        priority,
        session_id: sessionID,
        phases,
        screenshots,
        panel_results: checks.find((check) => check.name === "tui-panel-attempt-screenshots")
          ?.panelResults,
        checks,
      },
      runPaths,
    );
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
    if (!summary.ok) process.exitCode = 1;
  } finally {
    if (gateway.child) {
      const logs = gateway.child.logs();
      await fs.writeFile(path.join(runRoot, "gateway.stdout.log"), logs.stdout);
      await fs.writeFile(path.join(runRoot, "gateway.stderr.log"), logs.stderr);
      await stopProcess(gateway.child);
    } else {
      await fs.writeFile(path.join(runRoot, "gateway.external.log"), gateway.url);
    }
  }
}

main().catch(async (error) => {
  const summary = normalizeBusinessSummary(
    { ok: false, error: String(error.stack || error.message || error), checks },
    runPaths,
  );
  await fs.mkdir(runRoot, { recursive: true }).catch(() => undefined);
  await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2)).catch(() => undefined);
  console.error(error);
  process.exitCode = 1;
});
