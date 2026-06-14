#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import {
  businessRunPaths,
  normalizeBusinessSummary,
} from "../../../../tests/business/business_lib_business_paths.mjs";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..", "..");
const runId = process.env.TUI_BUSINESS_RUN_ID || `tui-real-gateway-${Date.now()}`;
const runPaths = businessRunPaths("tui-real-gateway", runId);
const runRoot = runPaths.run_root;
const workspace = path.join(runRoot, "workspace");
const summaryPath = runPaths.summary_path;
const gatewayExe = path.join(
  repoRoot,
  "bin",
  process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway",
);
const tuiAppRoot = path.join(repoRoot, "apps", "tui");
const tuiBin = path.join(tuiAppRoot, "dist", "index.js");
const webTerminalBin = path.join(tuiAppRoot, "scripts", "web-terminal.mjs");
const tuiRequire = createRequire(path.join(tuiAppRoot, "package.json"));
const nodeBin = process.execPath;
const timeoutMs = Number(process.env.TUI_BUSINESS_TIMEOUT_MS || 120_000);
const livePrompt = process.env.TUI_BUSINESS_ALLOW_SKIP_LIVE_PROMPT !== "1";
const providerLogRoot = path.join(repoRoot, "log", "provider");

const checks = [];

function record(name, ok, details = {}) {
  checks.push({ name, ok, ...details });
  if (!ok) throw new Error(`${name} failed: ${JSON.stringify(details)}`);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: 64 * 1024 * 1024,
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

async function waitForUrl(url, deadlineMs = 30_000) {
  const deadline = Date.now() + deadlineMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return await response.json().catch(() => ({}));
    } catch {
      // Retry until the real gateway is ready.
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`timed out waiting for ${url}`);
}

function freePort() {
  return 19_000 + Math.floor(Math.random() * 20_000);
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
  if (!child || child.killed) return;
  if (child.exitCode !== null) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
      windowsHide: true,
    });
  } else {
    child.kill("SIGTERM");
  }
  await new Promise((resolve) => child.once("exit", resolve));
}

async function startRealGateway() {
  const port = freePort();
  const child = startProcess(gatewayExe, [], {
    env: {
      PORT: String(port),
      TURA_CWD: workspace,
      TURA_PROJECT_ROOT: workspace,
    },
  });
  const url = `http://127.0.0.1:${port}`;
  const health = await waitForUrl(`${url}/global/health`, 45_000);
  return { child, url, health };
}

function tuiArgs(gatewayUrl) {
  return ["--gateway-url", gatewayUrl, "--cwd", workspace];
}

function runTui(gatewayUrl, args, options = {}) {
  return run(nodeBin, [tuiBin, ...tuiArgs(gatewayUrl), ...args], {
    cwd: repoRoot,
    timeoutMs: options.timeoutMs,
    env: { TURA_LANG: "en", ...(options.env || {}) },
  });
}

function runTuiOk(gatewayUrl, args, options = {}) {
  const result = runTui(gatewayUrl, args, options);
  if (result.status !== 0) {
    throw new Error(
      `tui ${args.join(" ")} failed with ${result.status || result.signal}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`,
    );
  }
  return result;
}

function jsonFrom(result) {
  return JSON.parse(result.stdout);
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
      if (item.isDirectory()) {
        await walk(fullPath);
      } else if (item.isFile() && item.name.endsWith(".json")) {
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

async function saveJson(filePath, value) {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, JSON.stringify(value, null, 2));
}

function messageRole(message) {
  return message.role || message.info?.role || "";
}

function messageParts(message) {
  return message.parts || message.info?.parts || [];
}

function messageText(message) {
  return messageParts(message)
    .map((part) => part.text ?? part.content ?? "")
    .join("");
}

function userFacingAssistantMessages(messages) {
  return messages
    .filter((message) => messageRole(message) === "assistant")
    .map(messageText)
    .map((text) => text.trim())
    .filter((text) => text && !/completed without a user-facing message/i.test(text));
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

async function webTerminalBusiness(gatewayUrl) {
  const { chromium } = tuiRequire("playwright");
  const port = freePort();
  const screenshotsDir = path.join(runRoot, "web-terminal-screenshots");
  await fs.mkdir(screenshotsDir, { recursive: true });
  const child = startProcess(nodeBin, [webTerminalBin], {
    env: {
      PORT: String(port),
      TURA_GATEWAY_URL: gatewayUrl,
      TURA_CWD: workspace,
    },
  });
  const url = `http://127.0.0.1:${port}`;
  try {
    await waitForUrl(`${url}/`, 30_000).catch(async () => {
      const response = await fetch(`${url}/`);
      if (!response.ok) throw new Error(`web terminal returned ${response.status}`);
    });
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({
      viewport: { width: 1280, height: 720 },
    });
    async function sendRich(data) {
      await fetch(`${url}/rich/input`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ data }),
      });
    }
    async function submitRich(value) {
      await sendRich("\u001b");
      await page.waitForTimeout(120);
      await sendRich(value);
      await page.waitForTimeout(120);
      await sendRich("\r");
    }
    try {
      await page.goto(`${url}/`, { waitUntil: "domcontentloaded" });
      await page.screenshot({
        path: path.join(screenshotsDir, "00-index.png"),
        fullPage: false,
      });

      for (const profile of ["plain", "ansi", "rich"]) {
        await page.goto(`${url}/${profile}`, { waitUntil: "domcontentloaded" });
        await page.waitForTimeout(1400);
        const title = await page.title();
        assert.equal(
          title,
          `Tura TUI ${profile === "plain" ? "Plain / Safe" : profile === "ansi" ? "ANSI / Default" : "Rich / Modern"}`,
        );
        await page.screenshot({
          path: path.join(
            screenshotsDir,
            `${profile === "plain" ? "01" : profile === "ansi" ? "02" : "03"}-${profile}-initial.png`,
          ),
          fullPage: false,
        });
      }
      const richSteps = [
        { name: "04-rich-sessions.png", data: "/sessions" },
        { name: "05-rich-models.png", data: "/models" },
        { name: "06-rich-auth.png", data: "/auth" },
        { name: "07-rich-settings.png", data: "/settings" },
        { name: "08-rich-config-roundtrip.png", data: "/config get" },
      ];
      for (const step of richSteps) {
        await submitRich(step.data);
        await page.waitForTimeout(1100);
        await page.screenshot({
          path: path.join(screenshotsDir, step.name),
          fullPage: false,
        });
      }
      await sendRich("\u001b");
      await page.waitForTimeout(120);
      await sendRich("TUI web terminal business prompt: include TUI_WEB_OK");
      await page.waitForTimeout(700);
      await page.screenshot({
        path: path.join(screenshotsDir, "09-rich-composer-prompt.png"),
        fullPage: false,
      });
      await sendRich("\r");
      await page
        .waitForFunction(
          () =>
            document.body.innerText.includes("忙碌") ||
            document.body.innerText.toLowerCase().includes("busy"),
          { timeout: 8_000 },
        )
        .catch(() => undefined);
      await page.waitForTimeout(1200);
      await page.screenshot({
        path: path.join(screenshotsDir, "10-rich-live-prompt-sent.png"),
        fullPage: false,
      });
      await page
        .waitForFunction(
          () =>
            document.body.innerText.includes("空闲") ||
            document.body.innerText.toLowerCase().includes("idle"),
          { timeout: 30_000 },
        )
        .catch(() => undefined);
      await page.screenshot({
        path: path.join(screenshotsDir, "11-rich-live-prompt-result.png"),
        fullPage: false,
      });
    } finally {
      await browser.close();
    }
    return screenshotsDir;
  } finally {
    const logs = child.logs();
    await stopProcess(child);
    await fs.writeFile(path.join(runRoot, "web-terminal.stdout.log"), logs.stdout);
    await fs.writeFile(path.join(runRoot, "web-terminal.stderr.log"), logs.stderr);
  }
}

async function main() {
  await fs.rm(runRoot, { recursive: true, force: true });
  await fs.mkdir(workspace, { recursive: true });
  await fs.access(gatewayExe);
  runOk(process.platform === "win32" ? "npm.cmd" : "npm", ["run", "build"], {
    cwd: tuiAppRoot,
    timeoutMs: 120_000,
    shell: process.platform === "win32",
  });
  await fs.access(tuiBin);

  const gateway = await startRealGateway();
  try {
    record("real-gateway-health", gateway.health?.healthy === true, {
      version: gateway.health?.version,
    });

    const help = runTuiOk(gateway.url, ["help"]);
    record(
      "help-covers-current-tui-command-surface",
      [
        "run",
        "resume",
        "session",
        "config",
        "provider",
        "agent",
        "persona",
        "project",
        "file",
        "command",
        "inspect",
        "gateway",
        "completion",
      ].every((command) => new RegExp(`^  ${command}\\s`, "m").test(help.stdout)),
      { help: help.stdout },
    );
    record("help-session-scope-minimal", /session\s+list or show sessions/.test(help.stdout), {
      help: help.stdout,
    });

    const config = jsonFrom(runTuiOk(gateway.url, ["--json", "config", "get"]));
    record("session-config-readable", typeof config === "object" && config !== null, {
      keys: Object.keys(config).slice(0, 12),
    });
    const patch = {};
    if (config.active_agent) patch.agent = config.active_agent;
    if (config.model_variant) patch.model_variant = config.model_variant;
    if (Object.keys(patch).length) {
      const args = Object.entries(patch).map(([key, value]) => `${key}=${value}`);
      const patched = jsonFrom(runTuiOk(gateway.url, ["--json", "config", "set", ...args]));
      record(
        "session-config-write-roundtrip",
        Object.entries(patch).every(([key, value]) => {
          const responseKey = key === "agent" ? "active_agent" : key;
          return String(patched[responseKey]) === String(value);
        }),
        { patch, patched },
      );
    } else {
      record("session-config-write-roundtrip", true, {
        skipped: "gateway returned no active_agent/model_variant to roundtrip",
      });
    }
    const themeAttempt = runTui(gateway.url, ["config", "set", "theme=dark"]);
    record(
      "appearance-config-rejected",
      themeAttempt.status !== 0 &&
        /unsupported session config key/.test(themeAttempt.stderr + themeAttempt.stdout),
    );

    const providerList = jsonFrom(runTuiOk(gateway.url, ["--json", "provider", "list"]));
    record("provider-list-readable", Array.isArray(providerList.all), {
      count: providerList.all?.length ?? 0,
    });
    const providerID = providerList.all?.[0]?.id;
    if (providerID) {
      const status = jsonFrom(runTuiOk(gateway.url, ["provider", "status", providerID]));
      record("provider-status-readable", typeof status === "object" && status !== null, {
        providerID,
        authenticated: status.authenticated,
      });
    } else {
      record("provider-status-readable", true, {
        skipped: "real gateway returned no providers",
      });
    }

    const agents = jsonFrom(runTuiOk(gateway.url, ["--json", "agent", "list"]));
    record("agent-list-readable", Array.isArray(agents), {
      count: agents.length,
    });
    const agentID = agents[0]?.id ?? agents[0]?.name;
    if (agentID) {
      const agent = jsonFrom(runTuiOk(gateway.url, ["--json", "agent", "show", String(agentID)]));
      record("agent-show-readonly", Boolean(agent.summary || agent.name || agent.id), { agentID });
    } else {
      record("agent-show-readonly", true, {
        skipped: "real gateway returned no agents",
      });
    }
    const createAgent = runTui(gateway.url, [
      "--json",
      "agent",
      "create",
      "business-agent",
      "--prompt",
      "Business coverage prompt",
    ]);
    record("agent-create-command-exposed", createAgent.status === 0, {
      status: createAgent.status,
      stderr: createAgent.stderr,
      stdout: createAgent.stdout.slice(0, 500),
    });

    const createdSession = await requestJson(`${gateway.url}/session`, {
      method: "POST",
      body: JSON.stringify({
        directory: workspace,
        agent: agentID,
        model: config.model ?? undefined,
        model_variant: config.model_variant ?? undefined,
        model_acceleration_enabled: config.model_acceleration_enabled ?? undefined,
      }),
    });
    record("real-session-created-for-tui-flow", Boolean(createdSession?.id), {
      sessionID: createdSession?.id,
    });

    const sessions = jsonFrom(runTuiOk(gateway.url, ["--json", "session", "list"]));
    record("session-list-readable", Array.isArray(sessions), {
      count: sessions.length,
    });
    if (createdSession?.id) {
      const shown = jsonFrom(
        runTuiOk(gateway.url, ["--json", "session", "show", createdSession.id]),
      );
      record(
        "session-show-readable",
        shown.session?.id === createdSession.id && Array.isArray(shown.messages),
        { sessionID: createdSession.id },
      );
      const transcript = runTuiOk(gateway.url, ["resume", createdSession.id]);
      record("resume-transcript-readable", transcript.status === 0, {
        stdoutBytes: transcript.stdout.length,
      });
    } else {
      record("session-show-readable", true, {
        skipped: "real gateway returned no sessions",
      });
      record("resume-transcript-readable", true, {
        skipped: "real gateway returned no sessions",
      });
    }

    const screenshotsDir = await webTerminalBusiness(gateway.url);
    record("web-terminal-three-levels-real-gateway", true, { screenshotsDir });

    if (livePrompt) {
      const beforeProviderLogs = await listProviderLogs();
      const beforeKeys = new Set(beforeProviderLogs.map(providerLogKey));
      const beforeSessions = await requestJson(
        `${gateway.url}/session?directory=${encodeURIComponent(workspace)}&includeChildren=true&limit=20`,
      ).catch(() => []);
      const beforeSessionIds = new Set(
        Array.isArray(beforeSessions) ? beforeSessions.map((session) => session.id) : [],
      );
      const liveResult = runTui(
        gateway.url,
        [
          "--json",
          "run",
          "--no-stream",
          "--timeout",
          String(Math.ceil(timeoutMs / 1000)),
          "TUI real business test: include TUI_BUSINESS_OK in the response.",
        ],
        { timeoutMs: timeoutMs + 30_000 },
      );
      await fs.writeFile(path.join(runRoot, "live-run.stdout.log"), liveResult.stdout);
      await fs.writeFile(path.join(runRoot, "live-run.stderr.log"), liveResult.stderr);

      let parsedResult = null;
      try {
        parsedResult = liveResult.stdout.trim() ? JSON.parse(liveResult.stdout) : null;
      } catch (error) {
        parsedResult = { parse_error: String(error.message || error) };
      }
      let sessionID = parsedResult?.sessionID;
      if (!sessionID) {
        const afterSessions = await requestJson(
          `${gateway.url}/session?directory=${encodeURIComponent(workspace)}&includeChildren=true&limit=20`,
        ).catch(() => []);
        if (Array.isArray(afterSessions)) {
          const created = afterSessions.find((session) => !beforeSessionIds.has(session.id));
          sessionID = created?.id ?? afterSessions[0]?.id;
          await saveJson(path.join(runRoot, "live-sessions-after-timeout.json"), afterSessions);
        }
      }
      const messages = sessionID
        ? await requestJson(
            `${gateway.url}/session/${encodeURIComponent(sessionID)}/message`,
          ).catch((error) => ({ error: String(error.message || error) }))
        : [];
      await saveJson(path.join(runRoot, "live-session-messages.json"), messages);

      const afterProviderLogs = await listProviderLogs();
      const newProviderLogs = afterProviderLogs.filter(
        (log) => !beforeKeys.has(providerLogKey(log)),
      );
      await saveJson(path.join(runRoot, "live-provider-logs.json"), newProviderLogs);
      record(
        "live-conversation-run",
        liveResult.status === 0 &&
          parsedResult?.status === "completed" &&
          /TUI_BUSINESS_OK/i.test(parsedResult?.finalText ?? "") &&
          Array.isArray(messages) &&
          userFacingAssistantMessages(messages).some((text) => /TUI_BUSINESS_OK/i.test(text)) &&
          newProviderLogs.length > 0,
        {
          exitStatus: liveResult.status,
          signal: liveResult.signal,
          error: liveResult.error,
          sessionID,
          finalText: parsedResult?.finalText ?? "",
          userFacingAssistantMessages: Array.isArray(messages)
            ? userFacingAssistantMessages(messages)
            : [],
          providerLogs: newProviderLogs.map((log) => log.path),
          stdoutTail: liveResult.stdout.slice(-2000),
          stderrTail: liveResult.stderr.slice(-2000),
        },
      );
    } else {
      record("live-conversation-run", true, {
        skipped:
          "TUI_BUSINESS_ALLOW_SKIP_LIVE_PROMPT=1 was set; default business mode requires a real provider call",
      });
    }
  } finally {
    const logs = gateway.child.logs();
    await fs.writeFile(path.join(runRoot, "gateway.stdout.log"), logs.stdout);
    await fs.writeFile(path.join(runRoot, "gateway.stderr.log"), logs.stderr);
    await stopProcess(gateway.child);
  }

  const failures = checks.filter((check) => !check.ok);
  const summary = normalizeBusinessSummary(
    {
      ok: failures.length === 0,
      workspace,
      live_prompt: livePrompt,
      checks,
      failures,
    },
    runPaths,
  );
  await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  if (failures.length) process.exitCode = 1;
}

main().catch(async (error) => {
  const summary = normalizeBusinessSummary(
    {
      ok: false,
      workspace,
      error: String(error.stack || error.message || error),
      checks,
    },
    runPaths,
  );
  await fs.mkdir(runRoot, { recursive: true }).catch(() => {});
  await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2)).catch(() => {});
  console.error(error);
  process.exitCode = 1;
});
