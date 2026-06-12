#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..");
const runRoot = path.join(repoRoot, "target", "tui-mock-stream", String(Date.now()));
const screenshotsDir = path.join(runRoot, "screenshots");
const summaryPath = path.join(runRoot, "summary.json");
const tuiRequire = createRequire(path.join(repoRoot, "apps", "tui", "package.json"));
const { chromium } = tuiRequire("playwright");

const sessionID = "sess-mock-stream";
const workspace = runRoot;
const now = Date.now();
let session = {
  id: sessionID,
  name: "Mock Stream",
  session_display_name: "Mock Stream",
  directory: workspace,
  status: "idle",
  model: "openai/gpt-test",
  agent: "fast",
  model_variant: "medium",
  model_acceleration_enabled: true,
  created_at: now,
  updated_at: now,
  message_count: 1,
};
const config = {
  model: "openai/gpt-test",
  active_model: "openai/gpt-test",
  active_provider: "openai",
  active_agent: "fast",
  model_variant: "medium",
  model_acceleration_enabled: true,
  show_command_instructions: true,
};
const messages = [
  {
    id: "msg-user-1",
    sessionID,
    role: "user",
    parts: [{ id: "part-user-1", type: "text", text: "触发 mock gateway stream 文本和命令。" }],
    created_at: now,
    updated_at: now,
  },
];
const eventClients = new Set();

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
  emit({ directory: workspace, payload: { type, properties } });
}

function streamDelta(delta) {
  gatewayEvent("message.part.delta", {
    session_id: sessionID,
    message_id: "msg-stream-main",
    part_id: "part-stream-main",
    field: "text",
    delta,
  });
}

function upsertMessage(message) {
  const index = messages.findIndex((item) => item.id === message.id);
  if (index >= 0) messages[index] = message;
  else messages.push(message);
  session = { ...session, status: "busy", updated_at: Date.now(), message_count: messages.length };
  gatewayEvent("message.updated", { session_id: sessionID, info: message });
}

async function delay(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForUrl(url, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // Retry.
    }
    await delay(100);
  }
  throw new Error(`timed out waiting for ${url}`);
}

function createGatewayServer() {
  const providerList = {
    all: [
      {
        id: "openai",
        name: "OpenAI",
        source: "config",
        env: ["OPENAI_API_KEY"],
        options: {},
        models: { "gpt-test": { id: "gpt-test", name: "gpt-test" } },
      },
    ],
    default: { openai: "gpt-test" },
    connected: ["openai"],
    enums: { domains: [], capabilities: [], api_styles: [], auth_methods: [], statuses: [] },
  };
  const agent = {
    summary: {
      id: "fast",
      name: "Fast",
      description: "Mock stream agent",
      source: "static",
      path: "agents/src/fast",
      aliases: [],
      capabilities: ["chat"],
      hidden: false,
    },
    config: { agent_name: "fast", agent_persona: [] },
    prompt: "Mock stream prompt",
  };
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1");
    if (req.method === "GET" && url.pathname === "/global/health")
      return sendJson(res, { healthy: true, version: "mock-stream" });
    if (req.method === "GET" && url.pathname === "/project/current")
      return sendJson(res, { project: { worktree: workspace } });
    if (req.method === "GET" && url.pathname === "/session/config") return sendJson(res, config);
    if (req.method === "GET" && url.pathname === "/session") return sendJson(res, [session]);
    if (req.method === "POST" && url.pathname === "/session") {
      await readJson(req);
      return sendJson(res, session);
    }
    if (req.method === "GET" && url.pathname === `/session/${sessionID}/message`)
      return sendJson(
        res,
        [...messages].sort((left, right) => (left.created_at ?? 0) - (right.created_at ?? 0)),
      );
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
    if (req.method === "GET" && url.pathname === "/event") {
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
  return server;
}

async function listen(server) {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return server.address().port;
}

function startWebTerminal(gatewayUrl, port) {
  const child = spawn(
    process.execPath,
    [path.join(repoRoot, "apps", "tui", "scripts", "web-terminal.mjs")],
    {
      cwd: path.join(repoRoot, "apps", "tui"),
      env: {
        ...process.env,
        PORT: String(port),
        TURA_GATEWAY_URL: gatewayUrl,
        TURA_CWD: workspace,
        FORCE_COLOR: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    },
  );
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

async function capture(page, name) {
  await delay(300);
  const file = path.join(screenshotsDir, `${name}.png`);
  await page.screenshot({ path: file, fullPage: false });
  const text = await terminalText(page);
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
  return {
    name,
    path: file,
    hasComposerHint: /回车输入|Enter to send/.test(text),
    hasBottomMeta: /tokens|openai\/gpt-test/.test(text),
    hasCommand: /command_run|命令:|Commands:/.test(text),
    hasRawControlLeak: /\x1b|\[2K|\[K/.test(text),
    overflow:
      metrics.bodyScrollWidth > metrics.bodyClientWidth + 2 ||
      metrics.terminalScrollWidth > metrics.terminalClientWidth + 2 ||
      metrics.viewportScrollWidth > metrics.viewportClientWidth + 2,
    visibleText: text,
  };
}

async function main() {
  await fs.mkdir(screenshotsDir, { recursive: true });
  const gateway = createGatewayServer();
  const gatewayPort = await listen(gateway);
  const webServer = http.createServer((_, res) => res.end("placeholder"));
  const webPort = await listen(webServer);
  await new Promise((resolve) => webServer.close(resolve));
  const web = startWebTerminal(`http://127.0.0.1:${gatewayPort}`, webPort);
  const captures = [];
  let browser;
  try {
    await waitForUrl(`http://127.0.0.1:${webPort}/`);
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    await page.goto(`http://127.0.0.1:${webPort}/rich?instance=mock-stream`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => /Mock Stream/.test(document.body.innerText), null, {
      timeout: 15_000,
    });
    captures.push(await capture(page, "00-initial"));

    gatewayEvent("session.status", { sessionID, status: "busy" });

    // Phase 1: stream an intro plus a multi-item list. The whole list must stay
    // visible — assistant text used to be capped at 8 lines, hiding the rest.
    const listIntro = "我先给一段 stream 文本，下面是要执行的步骤清单：\n";
    const listItems = Array.from(
      { length: 6 },
      (_item, index) => `- 步骤 ${index + 1}: stream list item ${index + 1}`,
    );
    streamDelta(listIntro);
    await delay(60);
    for (const item of listItems) {
      streamDelta(`${item}\n`);
      await delay(60);
    }
    captures.push(await capture(page, "01-stream-list"));

    // Phase 2: commands arrive while text keeps streaming. They must not collide
    // with the streaming text nor reorder past it (the streamed reply belongs to
    // this turn and stays above the command output).
    const commands = [
      { id: "msg-command-1", cmd: "Get-Content -Raw .tura/config.conf", at: now + 2 },
      { id: "msg-command-2", cmd: "npm run build -- --watch", at: now + 3 },
      { id: "msg-command-3", cmd: "node tools/snake_playwright.mjs --steps 40", at: now + 4 },
    ];
    const commandPart = (entry, status) => ({
      id: `part-${entry.id}`,
      type: "tool",
      tool: "command_run",
      state: {
        status,
        input: { command_line: entry.cmd },
        output: status === "running" ? "working\r\x1b[2Kstill working" : "working\ncompleted",
      },
    });
    const upsertCommand = (entry, status) =>
      upsertMessage({
        id: entry.id,
        sessionID,
        role: "assistant",
        parts: [commandPart(entry, status)],
        created_at: entry.at,
        updated_at: Date.now(),
      });

    upsertCommand(commands[0], "running");
    captures.push(await capture(page, "02-command-1-running"));

    streamDelta("命令运行时继续 stream，文本不应被命令遮挡。\n");
    await delay(60);
    upsertCommand(commands[1], "running");
    captures.push(await capture(page, "03-command-2-running"));

    upsertCommand(commands[0], "completed");
    upsertCommand(commands[2], "running");
    for (const chunk of [
      "再补充几行说明文字，",
      "用于把面板内容撑高，",
      "以验证滚动与省略行为。\n",
    ]) {
      streamDelta(chunk);
      await delay(60);
    }
    captures.push(await capture(page, "04-stream-overflow"));

    // Phase 3: finalize. All commands complete and the consolidated reply
    // replaces the streamed message; ordering must remain stable.
    for (const entry of commands) upsertCommand(entry, "completed");
    upsertMessage({
      id: "msg-stream-main",
      sessionID,
      role: "assistant",
      parts: [
        {
          id: "part-stream-main",
          type: "text",
          text:
            listIntro +
            listItems.join("\n") +
            "\n命令运行时继续 stream，文本不应被命令遮挡。\n" +
            "再补充几行说明文字，用于把面板内容撑高，以验证滚动与省略行为。",
        },
      ],
      created_at: now + 1,
      updated_at: Date.now(),
    });
    session = { ...session, status: "idle" };
    gatewayEvent("session.status", { sessionID, status: "idle" });
    captures.push(await capture(page, "05-final"));

    // Phase 4: open the same session in a short panel so the transcript
    // overflows, proving the renderer keeps the latest content and marks the
    // rest as hidden rather than splicing the middle of the list together with
    // the command section.
    await page.setViewportSize({ width: 900, height: 320 });
    await page.goto(`http://127.0.0.1:${webPort}/rich?instance=mock-stream-compact`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => /Mock Stream/.test(document.body.innerText), null, {
      timeout: 15_000,
    });
    await delay(600);
    captures.push(await capture(page, "06-compact-overflow"));

    const final = captures.find((item) => item.name === "05-final");
    const finalLines = final.visibleText.split("\n").map((line) => line.trim());
    assert.equal(final.overflow, false, "terminal should not overflow horizontally");
    assert.equal(final.hasComposerHint, true, "composer hint should remain visible");
    assert.equal(final.hasBottomMeta, true, "bottom meta should remain visible");
    assert.equal(final.hasCommand, true, "command should remain visible after later stream text");
    assert.equal(
      final.hasRawControlLeak,
      false,
      "raw terminal controls should not leak into UI text",
    );

    for (let index = 1; index <= listItems.length; index += 1) {
      assert.ok(
        final.visibleText.includes(`stream list item ${index}`),
        `list item ${index} should stay visible (no assistant line cap)`,
      );
    }
    assert.ok(
      /snake_playwright/.test(final.visibleText),
      "later command output should remain visible",
    );
    const firstListIndex = finalLines.findIndex((line) => line.includes("stream list item 1"));
    const commandSummaryIndex = finalLines.findIndex((line) => /命令:|Commands:/.test(line));
    assert.ok(firstListIndex >= 0, "streamed list should be visible at final");
    assert.ok(
      commandSummaryIndex > firstListIndex,
      "command section must stay below the streamed text (stable ordering, no jump)",
    );

    const compact = captures.at(-1);
    assert.equal(compact.overflow, false, "compact terminal should not overflow horizontally");
    assert.ok(
      /snake_playwright|命令:|Commands:/.test(compact.visibleText),
      "compact view should keep the most recent content (the command section)",
    );
    // Scroll-based design: when the transcript overflows a short panel the
    // renderer keeps the latest content pinned and trims the earliest lines off
    // the top (reachable by scrolling) instead of drawing an "earlier output
    // hidden" marker or splicing the middle of the list together.
    assert.ok(
      !/触发 mock gateway stream/.test(compact.visibleText),
      "overflowing transcript should trim the earliest lines from the top",
    );
    assert.equal(
      compact.hasRawControlLeak,
      false,
      "compact view should not leak raw terminal controls",
    );

    const summary = {
      ok: true,
      runRoot,
      screenshotsDir,
      screenshots: captures.map((item) => item.path),
      phases: captures.map(({ visibleText, ...item }) => item),
    };
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
  } catch (error) {
    const summary = {
      ok: false,
      runRoot,
      screenshotsDir,
      screenshots: captures.map((item) => item.path),
      phases: captures.map(({ visibleText, ...item }) => item),
      error: error instanceof Error ? error.message : String(error),
      webTerminalLog: web.logs(),
    };
    await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
    process.exitCode = 1;
  } finally {
    await browser?.close().catch(() => {});
    web.child.kill();
    gateway.close();
  }
}

await main();
