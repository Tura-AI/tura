#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
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
  "tui-multi-session",
  String(Date.now()),
);
const screenshotsDir = path.join(runRoot, "screenshots");
const summaryPath = path.join(runRoot, "summary.json");
const tuiRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium } = tuiRequire("playwright");

const workspace = path.join(runRoot, "workspace");
const eventClients = new Set();
const sessions = [];
const messagesBySession = new Map();
const promptRecords = [];
let nextSessionNumber = 1;

const config = {
  model: "mock/gpt-test",
  active_model: "mock/gpt-test",
  active_provider: "mock",
  active_agent: "fast",
  model_variant: "medium",
  model_acceleration_enabled: true,
  show_command_instructions: true,
};

const providerList = {
  all: [
    {
      id: "mock",
      name: "Mock Provider",
      source: "mock",
      env: [],
      options: {},
      models: { "gpt-test": { id: "gpt-test", name: "gpt-test" } },
    },
  ],
  default: { mock: "gpt-test" },
  connected: ["mock"],
  enums: {
    domains: [],
    capabilities: [],
    api_styles: [],
    auth_methods: [],
    statuses: [],
  },
};

const agent = {
  summary: {
    id: "fast",
    name: "Fast",
    description: "Mock multi-session agent",
    source: "static",
    path: "agents/src/fast",
    aliases: [],
    capabilities: ["chat"],
    hidden: false,
  },
  config: { agent_name: "fast" },
  prompt: "Mock multi-session prompt",
};

function now() {
  return Date.now();
}

function makeSession() {
  const number = nextSessionNumber++;
  const timestamp = now();
  const id = `daily-session-${number}`;
  const session = {
    id,
    name: `Daily Session ${number}`,
    session_display_name: `Daily Session ${number}`,
    directory: workspace,
    status: "idle",
    model: "mock/gpt-test",
    agent: "fast",
    model_variant: "medium",
    model_acceleration_enabled: true,
    created_at: timestamp,
    updated_at: timestamp,
    message_count: 0,
  };
  sessions.unshift(session);
  messagesBySession.set(id, []);
  emit("session.created", { sessionID: id, info: session });
  return session;
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

function emit(type, properties) {
  const payload = `data: ${JSON.stringify({
    directory: workspace,
    payload: { type, properties },
  })}\n\n`;
  for (const client of eventClients) client.write(payload);
}

function upsertSession(session) {
  const index = sessions.findIndex((item) => item.id === session.id);
  if (index >= 0) sessions[index] = session;
  else sessions.unshift(session);
  sessions.sort((left, right) => (right.updated_at ?? 0) - (left.updated_at ?? 0));
}

function promptText(payload) {
  const parts = Array.isArray(payload?.parts) ? payload.parts : [];
  return (
    parts
      .map((part) => part?.text ?? part?.content ?? "")
      .filter(Boolean)
      .join("\n") ||
    payload?.message ||
    payload?.prompt ||
    ""
  );
}

function appendPrompt(sessionID, payload) {
  const session = sessions.find((item) => item.id === sessionID);
  if (!session) return undefined;
  const text = promptText(payload);
  const list = messagesBySession.get(sessionID) ?? [];
  const index = promptRecords.length + 1;
  const created = now();
  const user = {
    id: `msg-user-${index}`,
    sessionID,
    role: "user",
    parts: [{ id: `part-user-${index}`, type: "text", text }],
    created_at: created,
    updated_at: created,
  };
  const assistant = {
    id: `msg-assistant-${index}`,
    sessionID,
    role: "assistant",
    parts: [{ id: `part-assistant-${index}`, type: "text", text: `Received: ${text}` }],
    created_at: created + 1,
    updated_at: created + 1,
  };
  list.push(user, assistant);
  messagesBySession.set(sessionID, list);
  promptRecords.push({ sessionID, text });
  const updated = { ...session, updated_at: created + 1, message_count: list.length };
  upsertSession(updated);
  emit("message.updated", { sessionID, info: user });
  emit("message.updated", { sessionID, info: assistant });
  emit("session.updated", { sessionID, info: updated });
  return updated;
}

function createGatewayServer() {
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1");
    if (req.method === "GET" && url.pathname === "/global/health") {
      return sendJson(res, { healthy: true, version: "multi-session-mock" });
    }
    if (req.method === "GET" && url.pathname === "/project/current") {
      return sendJson(res, { project: { worktree: workspace } });
    }
    if (req.method === "GET" && url.pathname === "/session/config") return sendJson(res, config);
    if (req.method === "GET" && url.pathname === "/session") return sendJson(res, sessions);
    if (req.method === "POST" && url.pathname === "/session") {
      await readJson(req);
      return sendJson(res, makeSession());
    }
    const sessionMatch = url.pathname.match(/^\/session\/([^/]+)$/);
    if (sessionMatch && req.method === "GET") {
      const sessionID = decodeURIComponent(sessionMatch[1]);
      const session = sessions.find((item) => item.id === sessionID);
      return session
        ? sendJson(res, session)
        : sendJson(res, { error: "missing session", sessionID }, 404);
    }
    const messageMatch = url.pathname.match(/^\/session\/([^/]+)\/message$/);
    if (messageMatch && req.method === "GET") {
      const sessionID = decodeURIComponent(messageMatch[1]);
      return sendJson(res, messagesBySession.get(sessionID) ?? []);
    }
    const promptMatch = url.pathname.match(/^\/session\/([^/]+)\/prompt_async$/);
    if (promptMatch && req.method === "POST") {
      const sessionID = decodeURIComponent(promptMatch[1]);
      const updated = appendPrompt(sessionID, await readJson(req));
      if (!updated) return sendJson(res, { error: "missing session" }, 404);
      return sendJson(res, {});
    }
    const abortMatch = url.pathname.match(/^\/session\/([^/]+)\/abort$/);
    if (abortMatch && req.method === "POST") return sendJson(res, { ok: true });
    if (req.method === "GET" && url.pathname === "/provider") return sendJson(res, providerList);
    if (req.method === "GET" && url.pathname === "/provider/auth") return sendJson(res, {});
    if (req.method === "GET" && url.pathname === "/agent") return sendJson(res, [agent]);
    if (req.method === "GET" && url.pathname === "/persona") return sendJson(res, []);
    if (
      req.method === "GET" &&
      (url.pathname === "/event" || /^\/session\/[^/]+\/events$/.test(url.pathname))
    ) {
      res.writeHead(200, {
        "content-type": "text/event-stream",
        "cache-control": "no-cache",
        connection: "keep-alive",
      });
      eventClients.add(res);
      res.write(
        `data: ${JSON.stringify({
          directory: "global",
          payload: { type: "server.connected", properties: {} },
        })}\n\n`,
      );
      req.on("close", () => eventClients.delete(res));
      return;
    }
    sendJson(res, { error: "not found", path: url.pathname }, 404);
  });
  return server;
}

async function listen(server) {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return server.address().port;
}

function startWebTerminal(gatewayUrl, port) {
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
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let logs = "";
  child.stdout.on("data", (chunk) => {
    logs += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    logs += chunk.toString();
  });
  return { child, logs: () => logs };
}

async function terminalText(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll(".xterm-rows > div")]
      .map((node) => node.textContent ?? "")
      .join("\n"),
  );
}

async function waitForUrl(url, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw lastError || new Error(`timed out waiting for ${url}`);
}

async function waitForCondition(condition, label, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (condition()) return;
    await delay(50);
  }
  throw new Error(label);
}

async function sendInput(page, text) {
  await page.evaluate((input) => globalThis.__turaSendInput?.(input), `\u0015${text}\r`);
}

async function sendPrompt(page, sessionID, text) {
  const previous = promptRecords.length;
  await sendInput(page, text);
  await waitForCondition(
    () => promptRecords.length === previous + 1 && promptRecords.at(-1)?.sessionID === sessionID,
    `timed out waiting for prompt "${text}" in ${sessionID}`,
  );
}

async function selectSession(page, sessionID) {
  await sendInput(page, `/resume ${sessionID}`);
  await delay(500);
}

async function main() {
  await fs.rm(runRoot, { recursive: true, force: true });
  await fs.mkdir(screenshotsDir, { recursive: true });
  await fs.mkdir(workspace, { recursive: true });
  const gateway = createGatewayServer();
  const gatewayPort = await listen(gateway);
  const webServer = http.createServer((_, res) => res.end("placeholder"));
  const webPort = await listen(webServer);
  await new Promise((resolve) => webServer.close(resolve));
  const web = startWebTerminal(`http://127.0.0.1:${gatewayPort}`, webPort);
  let browser;
  const screenshots = [];
  try {
    await waitForUrl(`http://127.0.0.1:${webPort}/`);
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    await page.goto(`http://127.0.0.1:${webPort}/rich?instance=multi-session`, {
      waitUntil: "domcontentloaded",
    });
    await waitForCondition(() => sessions.length === 1, "initial session was not created");
    const session1 = sessions[0].id;
    await page.waitForFunction(() => document.body.innerText.includes("Daily Session 1"), null, {
      timeout: 15_000,
    });

    await sendPrompt(page, session1, "Daily chat 1: organize the todo list first.");
    await sendPrompt(page, session1, "Daily chat 2: remind me to check the build this afternoon.");

    await sendInput(page, "/new");
    await waitForCondition(() => sessions.length === 2, "second session was not created");
    const session2 = sessions[0].id;
    await sendPrompt(page, session2, "Daily chat 3: this session records dinner plans.");

    await sendInput(page, "/new");
    await waitForCondition(() => sessions.length === 3, "third session was not created");
    const session3 = sessions[0].id;
    await sendPrompt(page, session3, "Daily chat 4: the third session records walking plans.");

    await selectSession(page, session1);
    await sendPrompt(
      page,
      session1,
      "Daily chat 5: return to the first session and continue testing.",
    );
    await selectSession(page, session2);
    await sendPrompt(page, session2, "Daily chat 6: add a shopping list to the second session.");
    await selectSession(page, session3);
    await sendPrompt(
      page,
      session3,
      "Daily chat 7: add weather observations to the third session.",
    );
    await selectSession(page, session1);
    await sendPrompt(
      page,
      session1,
      "Daily chat 8: first session confirms session switching works.",
    );
    await selectSession(page, session2);
    await sendPrompt(page, session2, "Daily chat 9: second session confirms history remains.");
    await selectSession(page, session3);
    await sendPrompt(page, session3, "Daily chat 10: third session makes the final confirmation.");

    const finalScreenshot = path.join(screenshotsDir, "multi-session-final.png");
    await page.screenshot({ path: finalScreenshot, fullPage: false });
    screenshots.push(finalScreenshot);

    const counts = Object.fromEntries(
      [session1, session2, session3].map((id) => [
        id,
        promptRecords.filter((record) => record.sessionID === id).length,
      ]),
    );
    assert.equal(promptRecords.length, 10);
    assert.deepEqual(counts, { [session1]: 4, [session2]: 3, [session3]: 3 });
    for (const sessionID of [session1, session2, session3]) {
      const messages = messagesBySession.get(sessionID) ?? [];
      assert.equal(messages.filter((message) => message.role === "user").length, counts[sessionID]);
      assert.equal(
        messages.filter((message) => message.role === "assistant").length,
        counts[sessionID],
      );
    }
    const text = await terminalText(page);
    assert.match(text, /Daily chat 10|third session makes the final confirmation/);

    const summary = {
      ok: true,
      runRoot,
      screenshots,
      sessions: [session1, session2, session3],
      promptRecords,
      counts,
    };
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
  } catch (error) {
    const summary = {
      ok: false,
      runRoot,
      screenshots,
      promptRecords,
      sessions,
      error: error instanceof Error ? error.stack || error.message : String(error),
      webTerminalLog: web.logs(),
    };
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
    process.exitCode = 1;
  } finally {
    await browser?.close().catch(() => undefined);
    web.child.kill();
    gateway.close();
  }
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

await main();
