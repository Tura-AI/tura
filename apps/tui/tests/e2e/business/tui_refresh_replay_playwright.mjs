#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";

const repoRoot =
  process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..", "..");
const appRoot = path.join(repoRoot, "apps", "tui");
const runRoot = path.join(
  repoRoot,
  "apps",
  "tui",
  "test-results",
  "tui-refresh-replay",
  String(Date.now()),
);
const screenshotsDir = path.join(runRoot, "screenshots");
const summaryPath = path.join(runRoot, "summary.json");
const tuiRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium } = tuiRequire("playwright");

const workspace = runRoot;
const sessionID = "sess-refresh-replay";
const base = Date.now() - 60_000;
const eventClients = new Set();
const requests = [];
let session = {
  id: sessionID,
  name: "Refresh Replay",
  session_display_name: "Refresh Replay",
  directory: workspace,
  status: "idle",
  model: "mock/gpt-refresh",
  agent: "direct",
  created_at: base,
  updated_at: base,
  message_count: 0,
};
const config = {
  model: "mock/gpt-refresh",
  active_model: "mock/gpt-refresh",
  active_provider: "mock",
  active_agent: "direct",
  show_command_instructions: true,
};

const oldMessages = Array.from({ length: 36 }, (_item, index) => {
  const n = index + 1;
  return message(
    `msg-old-${String(n).padStart(2, "0")}`,
    n % 2 ? "user" : "assistant",
    `REFRESH_OLD_${String(n).padStart(2, "0")}`,
    base + n,
  );
});
const userMessage = message("msg-refresh-user", "user", "REFRESH_USER_PROMPT", base + 100);
const historicalRuntimeID = "runtime-refresh-history";
const commandMessage = {
  id: `${historicalRuntimeID}.message`,
  sessionID,
  role: "assistant",
  created_at: base + 101,
  updated_at: base + 101,
  parts: [
    {
      id: `${historicalRuntimeID}.message`,
      sessionID,
      messageID: `${historicalRuntimeID}.message`,
      type: "text",
      text: "HISTORICAL_RUNTIME_TEXT_MARKER",
      content: "HISTORICAL_RUNTIME_TEXT_MARKER",
    },
    {
      id: `${historicalRuntimeID}.tool.command_run`,
      sessionID,
      messageID: `${historicalRuntimeID}.message`,
      type: "tool",
      tool: "command_run",
      callID: `${historicalRuntimeID}.tool.command_run`,
      state: {
        status: "completed",
        input: {
          command_type: "shell_command",
          command_line: "node tools/refresh-order-check.mjs",
        },
      },
    },
  ],
};
const durableMessage = message(
  "msg-refresh-durable",
  "assistant",
  "DURABLE_REFRESH_FINAL_MARKER",
  base + 102,
);
let messages = [...oldMessages, userMessage, commandMessage];
session = { ...session, message_count: messages.length };

function message(id, role, text, createdAt) {
  return {
    id,
    sessionID,
    role,
    created_at: createdAt,
    updated_at: createdAt,
    parts: [{ id: `${id}:text`, sessionID, type: "text", text }],
  };
}

function sendJson(res, value, status = 200) {
  const body = JSON.stringify(value);
  res.writeHead(status, {
    "content-type": "application/json",
    "content-length": Buffer.byteLength(body),
  });
  res.end(body);
}

function readJson(req) {
  return new Promise((resolve) => {
    let body = "";
    req.on("data", (chunk) => {
      body += chunk.toString();
    });
    req.on("end", () => resolve(body.trim() ? JSON.parse(body) : {}));
  });
}

function emit(event) {
  const payload = `data: ${JSON.stringify(event)}\n\n`;
  for (const client of eventClients) client.write(payload);
}

function gatewayEvent(type, properties) {
  emit({
    directory: workspace,
    sessionID,
    payload: {
      type,
      properties:
        type === "session.status"
          ? { ...properties, updatedAt: properties.updatedAt ?? Date.now() }
          : properties,
    },
  });
}

async function delay(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForEventClient(timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (eventClients.size > 0) return;
    await delay(50);
  }
  throw new Error("timed out waiting for TUI event stream subscription");
}

async function waitForUrl(url, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // Retry until the child process finishes binding.
    }
    await delay(100);
  }
  throw new Error(`timed out waiting for ${url}`);
}

function createGatewayServer() {
  const providerList = {
    all: [
      {
        id: "mock",
        name: "Mock",
        source: "test",
        env: [],
        options: { domains: ["llm"] },
        models: { "gpt-refresh": { id: "gpt-refresh", name: "gpt-refresh" } },
      },
    ],
    default: { mock: "gpt-refresh" },
    connected: ["mock"],
    enums: { domains: [], capabilities: [], api_styles: [], auth_methods: [], statuses: [] },
  };
  const agent = {
    summary: {
      id: "direct",
      name: "Direct",
      description: "refresh test agent",
      source: "static",
      path: "mock://agent/direct",
      aliases: [],
      capabilities: ["chat"],
      hidden: false,
    },
    config: { agent_name: "direct" },
    prompt: "refresh test",
  };
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1");
    requests.push({ method: req.method, path: url.pathname, query: url.search });
    if (req.method === "GET" && url.pathname === "/global/health")
      return sendJson(res, { healthy: true, version: "refresh-replay" });
    if (req.method === "GET" && url.pathname === "/project/current")
      return sendJson(res, { project: { worktree: workspace } });
    if (req.method === "GET" && url.pathname === "/session/config") return sendJson(res, config);
    if (req.method === "GET" && url.pathname === "/session") return sendJson(res, [session]);
    if (req.method === "GET" && url.pathname === `/session/${sessionID}`)
      return sendJson(res, session);
    if (req.method === "POST" && url.pathname === "/session") {
      await readJson(req);
      return sendJson(res, session);
    }
    if (req.method === "GET" && url.pathname === `/session/${sessionID}/message`)
      return sendJson(res, messages);
    if (req.method === "POST" && url.pathname === `/session/${sessionID}/prompt_async`) {
      await readJson(req);
      return sendJson(res, {});
    }
    if (req.method === "POST" && url.pathname === `/session/${sessionID}/abort`)
      return sendJson(res, { ok: true });
    if (req.method === "GET" && url.pathname === "/provider") return sendJson(res, providerList);
    if (req.method === "GET" && url.pathname === "/provider/auth") return sendJson(res, {});
    if (req.method === "GET" && url.pathname === "/agent") return sendJson(res, [agent]);
    if (req.method === "GET" && url.pathname === "/persona") return sendJson(res, []);
    if (
      req.method === "GET" &&
      (url.pathname === "/event" || url.pathname === `/session/${sessionID}/events`)
    ) {
      res.writeHead(200, {
        "content-type": "text/event-stream",
        "cache-control": "no-cache",
        connection: "keep-alive",
      });
      eventClients.add(res);
      res.write(
        `data: ${JSON.stringify({ directory: "global", payload: { type: "server.connected", properties: {} } })}\n\n`,
      );
      req.on("close", () => eventClients.delete(res));
      return;
    }
    sendJson(res, { error: "not found", path: url.pathname }, 404);
  });
  server.on("connection", (socket) => socket.unref());
  return server;
}

async function listen(server) {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return server.address().port;
}

function startWebTerminal(gatewayUrl, port) {
  const stdout = fsSync.openSync(path.join(screenshotsDir, "web-terminal.stdout.log"), "a");
  const stderr = fsSync.openSync(path.join(screenshotsDir, "web-terminal.stderr.log"), "a");
  const child = spawn(process.execPath, [path.join(appRoot, "scripts", "web-terminal.mjs")], {
    cwd: appRoot,
    env: {
      ...process.env,
      PORT: String(port),
      TURA_GATEWAY_URL: gatewayUrl,
      TURA_CWD: workspace,
      FORCE_COLOR: "1",
      TURA_LANG: "en",
    },
    detached: true,
    stdio: ["ignore", stdout, stderr],
    windowsHide: true,
  });
  child.unref();
  return { child, logs: () => "" };
}

async function terminalBufferText(page) {
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

async function visibleTerminalText(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll(".xterm-rows > div")]
      .map((node) => node.textContent ?? "")
      .join("\n"),
  );
}

function nonEmptyLineCount(text) {
  return text.split("\n").filter((line) => line.trim()).length;
}

async function capture(page, name) {
  const file = path.join(screenshotsDir, `${name}.png`);
  await page.screenshot({ path: file, fullPage: false });
  return file;
}

async function main() {
  await fs.mkdir(screenshotsDir, { recursive: true });
  const gateway = createGatewayServer();
  const gatewayPort = await listen(gateway);
  gateway.unref();
  const probe = http.createServer((_, res) => res.end("ok"));
  const webPort = await listen(probe);
  await new Promise((resolve) => probe.close(resolve));
  const web = startWebTerminal(`http://127.0.0.1:${gatewayPort}`, webPort);
  const screenshots = [];
  let browser;
  try {
    await waitForUrl(`http://127.0.0.1:${webPort}/`);
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 960, height: 360 } });
    await page.goto(`http://127.0.0.1:${webPort}/rich?instance=refresh-replay`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => window.__turaTerminal);
    await page.evaluate(() => window.__turaFit());
    await page.waitForFunction(
      () => {
        const visible = [...document.querySelectorAll(".xterm-rows > div")]
          .map((row) => row.textContent ?? "")
          .join("\n");
        const buffer = window.__turaTerminal?.buffer.active;
        const lines = [];
        if (buffer) {
          for (let index = 0; index < buffer.length; index += 1) {
            lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
          }
        }
        return `${visible}\n${lines.join("\n")}`.includes("REFRESH_OLD_36");
      },
      null,
      { timeout: 15_000 },
    );
    screenshots.push(await capture(page, "00-initial"));

    const initialBuffer = await terminalBufferText(page);
    const initialVisible = await visibleTerminalText(page);
    const initialText = `${initialBuffer}\n${initialVisible}`;
    const initialLineCount = nonEmptyLineCount(initialText);
    assert.ok(initialLineCount > 0, "initial replay should populate scrollback");
    assert.ok(
      initialText.includes("REFRESH_OLD_36"),
      "initial replay should render the latest historical message",
    );

    await waitForEventClient();
    session = { ...session, status: "busy", updated_at: Date.now() };
    gatewayEvent("session.status", { sessionID, status: "busy" });
    await delay(150);

    messages = [...oldMessages, userMessage, commandMessage, durableMessage];
    session = {
      ...session,
      status: "idle",
      message_count: messages.length,
      updated_at: Date.now(),
    };
    gatewayEvent("message.updated", { sessionID, info: durableMessage });
    gatewayEvent("session.status", { sessionID, status: "idle" });

    await page.waitForFunction(() => Boolean(window.__turaTerminal?.buffer.active), null, {
      timeout: 10_000,
    });
    await delay(500);
    screenshots.push(await capture(page, "02-durable-refresh"));

    const finalBuffer = await terminalBufferText(page);
    const finalText = `${finalBuffer}\n${await visibleTerminalText(page)}`;
    assert.ok(nonEmptyLineCount(await visibleTerminalText(page)) > 0);
    assert.ok(
      finalText.includes("REFRESH_OLD_36"),
      "durable refresh should retain replayed history",
    );
    assert.ok(
      finalText.includes("DURABLE_REFRESH_FINAL_MARKER"),
      "durable refresh should render the latest durable message",
    );
    assert.equal(messages.length, oldMessages.length + 3);
    assert.equal(session.status, "idle");
    assert.ok(
      messages.every(
        (message, index, list) => index === 0 || message.created_at >= list[index - 1].created_at,
      ),
      "durable messages should remain ordered by creation time",
    );

    await page.evaluate(() => window.__turaTerminal.scrollToTop());
    await delay(150);
    await page.evaluate(() => window.__turaTerminal.scrollToBottom());
    await delay(150);
    const visibleAfterScroll = await visibleTerminalText(page);
    assert.ok(
      nonEmptyLineCount(visibleAfterScroll) > 0,
      "scrolling should keep terminal content visible",
    );

    const summary = { ok: true, runRoot, screenshots, requests };
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
  } catch (error) {
    const summary = {
      ok: false,
      runRoot,
      screenshots,
      error: error instanceof Error ? error.stack || error.message : String(error),
      webTerminalLog: web.logs(),
      requests,
    };
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.error(JSON.stringify(summary, null, 2));
    process.exitCode = 1;
  } finally {
    await browser?.close().catch(() => {});
  }
}

await main();
