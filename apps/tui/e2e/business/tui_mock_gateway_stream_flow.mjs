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
let promptCounter = 0;
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

function streamDeltaFor(messageID, partID, delta, sessionPlacement = "properties") {
  const properties = {
    message_id: messageID,
    part_id: partID,
    field: "text",
    delta,
  };
  if (sessionPlacement === "properties") {
    gatewayEvent("message.part.delta", { session_id: sessionID, ...properties });
    return;
  }
  emit({
    directory: workspace,
    sessionID,
    payload: { type: "message.part.delta", properties },
  });
}

function streamDelta(delta) {
  streamDeltaFor("msg-stream-main", "part-stream-main", delta);
}

function upsertMessage(message) {
  const index = messages.findIndex((item) => item.id === message.id);
  if (index >= 0) messages[index] = message;
  else messages.push(message);
  session = { ...session, status: "busy", updated_at: Date.now(), message_count: messages.length };
  gatewayEvent("message.updated", { session_id: sessionID, info: message });
}

function userTextFromPromptPayload(payload) {
  return (payload?.parts ?? [])
    .map((part) => part?.text ?? part?.content ?? "")
    .join("")
    .trim();
}

function handlePromptPayload(payload) {
  promptCounter += 1;
  const index = promptCounter;
  const text = userTextFromPromptPayload(payload) || `TYPED_USER_${index}`;
  upsertMessage({
    id: payload?.messageID || `msg-typed-user-${index}`,
    sessionID,
    role: "user",
    parts: [{ id: `part-typed-user-${index}`, type: "text", text }],
    created_at: Date.now(),
    updated_at: Date.now(),
  });

  const reply =
    index === 1 ? "TYPED_REPLY_1 你好。今天折腾什么？" : "TYPED_REPLY_2 第二轮继续处理。";
  void (async () => {
    const messageID = `msg-typed-reply-${index}`;
    const partID = `part-typed-reply-${index}`;
    await streamShortChunks(reply, messageID, partID, "properties");
    upsertMessage({
      id: messageID,
      sessionID,
      role: "assistant",
      parts: [{ id: partID, type: "text", text: reply }],
      created_at: Date.now(),
      updated_at: Date.now(),
    });
    session = { ...session, status: "idle", updated_at: Date.now() };
    gatewayEvent("session.status", { sessionID, status: "idle" });
  })();
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
      handlePromptPayload(await readJson(req));
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

async function scrollTerminalTo(page, target, marker = undefined) {
  await page.evaluate((nextTarget) => {
    const term = window.__turaTerminal;
    if (!term) return;
    if (nextTarget === "top") {
      if (typeof term.scrollToLine === "function") term.scrollToLine(0);
      else term.scrollToTop();
      return;
    }
    if (typeof term.scrollToBottom === "function") term.scrollToBottom();
  }, target);
  if (!marker) {
    await delay(200);
    return;
  }
  await page.waitForFunction(
    (needle) => {
      const rows = [...document.querySelectorAll(".xterm-rows > div")]
        .map((node) => node.textContent ?? "")
        .join("\n");
      return rows.includes(needle);
    },
    marker,
    { timeout: 5_000 },
  );
}

function markerCount(text, marker) {
  return text.split(marker).length - 1;
}

function regexCount(text, pattern) {
  return Array.from(text.matchAll(pattern)).length;
}

function assertNoDuplicatedFrameText(text, label, markers = []) {
  assert.ok(
    regexCount(text, /回车输入|Enter to send/gu) <= 1,
    `${label} should not retain a duplicated composer/input box`,
  );
  assert.ok(
    markerCount(text, "Mock Stream") <= 1,
    `${label} should not retain duplicated session title chrome`,
  );
  assert.ok(
    regexCount(text, /tokens\s+\d+|tokens\s+-/gu) <= 1,
    `${label} should not retain duplicated token/status chrome`,
  );
  for (const marker of markers) {
    assert.equal(markerCount(text, marker), 1, `${label} should show ${marker} exactly once`);
  }
  assert.doesNotMatch(
    text,
    /置{8,}/u,
    `${label} should not leave repeated composer hint fragments behind`,
  );
}

async function waitForComposer(page, timeoutMs = 5000) {
  await page.waitForFunction(() => /回车输入|Enter to send/.test(document.body.innerText), null, {
    timeout: timeoutMs,
  });
}

async function submitTypedPrompt(page, text) {
  await page.evaluate((value) => window.__turaSendInput(value), text);
  await page.evaluate(() => window.__turaSendInput("\r"));
}

async function seedTerminalScrollback(page, marker) {
  await page.evaluate((staleMarker) => {
    const term = window.__turaTerminal;
    if (!term) return;
    const rows = Number(term.rows) || 24;
    for (let index = 0; index < rows + 12; index += 1) {
      term.write(`\r\n${staleMarker}_${String(index).padStart(2, "0")}`);
    }
    term.scrollToTop?.();
  }, marker);
  await delay(200);
}

async function waitForSessionPicker(page, timeoutMs = 5000) {
  await page.waitForFunction(
    () => /新会话|New session|New Session/.test(document.body.innerText),
    null,
    { timeout: timeoutMs },
  );
}

function assertSessionPickerCleared(text, label, staleMarker) {
  assert.doesNotMatch(
    text,
    new RegExp(staleMarker, "u"),
    `${label} should clear stale terminal scrollback before the session picker renders`,
  );
  assert.doesNotMatch(
    text,
    /回车输入|Enter to send/u,
    `${label} should not carry the chat composer into the session picker`,
  );
  assert.equal(
    markerCount(text, "TYPED_USER_1"),
    0,
    `${label} should not carry older chat rows into the session picker`,
  );
  assert.ok(
    markerCount(text, "TYPED_REPLY_2") <= 1,
    `${label} may show the active session preview once, but not duplicate it`,
  );
}

async function streamShortChunks(
  text,
  messageID = "msg-stream-main",
  partID = "part-stream-main",
  sessionPlacement = "properties",
) {
  for (const char of Array.from(text)) {
    streamDeltaFor(messageID, partID, char, sessionPlacement);
    await delay(12);
  }
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
    composerHintCount: regexCount(text, /回车输入|Enter to send/gu),
    defaultTitleCount: regexCount(text, /^tura$/gmu),
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
  let page;
  const pageErrors = [];
  const consoleMessages = [];
  try {
    await waitForUrl(`http://127.0.0.1:${webPort}/`);
    browser = await chromium.launch({ headless: true });
    page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    page.on("pageerror", (error) =>
      pageErrors.push(String(error?.stack || error?.message || error)),
    );
    page.on("console", (message) => consoleMessages.push(`${message.type()}: ${message.text()}`));
    await page.goto(`http://127.0.0.1:${webPort}/rich?instance=mock-stream`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => /Mock Stream/.test(document.body.innerText), null, {
      timeout: 15_000,
    });
    captures.push(await capture(page, "00-initial"));

    await submitTypedPrompt(page, "TYPED_USER_1 你好啊");
    await page.waitForFunction(() => document.body.innerText.includes("TYPED_REPLY_1"), null, {
      timeout: 15_000,
    });
    await waitForComposer(page);
    captures.push(await capture(page, "00-typed-round-1"));
    assertNoDuplicatedFrameText(captures.at(-1).visibleText, "typed round 1", [
      "TYPED_USER_1",
      "TYPED_REPLY_1",
    ]);
    await scrollTerminalTo(page, "top");
    captures.push(await capture(page, "00-typed-round-1-scrolled-top"));
    assertNoDuplicatedFrameText(captures.at(-1).visibleText, "typed round 1 scrolled top", [
      "TYPED_USER_1",
      "TYPED_REPLY_1",
    ]);

    await submitTypedPrompt(page, "TYPED_USER_2 继续第二轮");
    await page.waitForFunction(() => document.body.innerText.includes("TYPED_REPLY_2"), null, {
      timeout: 15_000,
    });
    await waitForComposer(page);
    captures.push(await capture(page, "00-typed-round-2"));
    assertNoDuplicatedFrameText(captures.at(-1).visibleText, "typed round 2", [
      "TYPED_USER_1",
      "TYPED_REPLY_1",
      "TYPED_USER_2",
      "TYPED_REPLY_2",
    ]);
    await scrollTerminalTo(page, "top");
    captures.push(await capture(page, "00-typed-round-2-scrolled-top"));
    await scrollTerminalTo(page, "bottom");
    captures.push(await capture(page, "00-typed-round-2-scrolled-bottom"));
    for (const typedPhase of captures.filter((item) => item.name.startsWith("00-typed-round-2"))) {
      assertNoDuplicatedFrameText(typedPhase.visibleText, typedPhase.name);
    }

    const staleSessionMarker = "STALE_BEFORE_SESSION_PICKER";
    await seedTerminalScrollback(page, staleSessionMarker);
    assert.match(
      await terminalBufferText(page),
      new RegExp(staleSessionMarker, "u"),
      "test setup must create stale terminal content before opening the session picker",
    );
    await page.evaluate(() => window.__turaSendInput("\t"));
    await waitForSessionPicker(page);
    captures.push(await capture(page, "00-session-picker-cleared"));
    assertSessionPickerCleared(
      captures.at(-1).visibleText,
      "session picker after typed rounds",
      staleSessionMarker,
    );
    assertSessionPickerCleared(
      await terminalBufferText(page),
      "session picker buffer after typed rounds",
      staleSessionMarker,
    );
    await scrollTerminalTo(page, "top");
    captures.push(await capture(page, "00-session-picker-scrolled-top"));
    assertSessionPickerCleared(
      captures.at(-1).visibleText,
      "session picker scrolled top",
      staleSessionMarker,
    );
    await scrollTerminalTo(page, "bottom");
    captures.push(await capture(page, "00-session-picker-scrolled-bottom"));
    assertSessionPickerCleared(
      captures.at(-1).visibleText,
      "session picker scrolled bottom",
      staleSessionMarker,
    );
    await page.evaluate(() => window.__turaSendInput("\x1b"));
    await waitForComposer(page);

    gatewayEvent("session.status", { sessionID, status: "busy" });

    // Phase 1: stream an intro plus a multi-item list. The whole list must stay
    // visible — assistant text used to be capped at 8 lines, hiding the rest.
    const listIntro = "我先给一段 stream 文本，下面是要执行的步骤清单：\n";
    const listItems = Array.from(
      { length: 10 },
      (_item, index) =>
        `- 步骤 ${index + 1}: SHORT_STREAM_MARKER_${String(index + 1).padStart(2, "0")}`,
    );
    await streamShortChunks(listIntro);
    for (const item of listItems) {
      await streamShortChunks(`${item}\n`);
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

    await streamShortChunks("命令运行时继续 stream，文本不应被命令遮挡。\n");
    upsertCommand(commands[1], "running");
    captures.push(await capture(page, "03-command-2-running"));

    upsertCommand(commands[0], "completed");
    upsertCommand(commands[2], "running");
    for (const chunk of [
      "再补充几行说明文字，",
      "用于把面板内容撑高，",
      "以验证滚动与省略行为。\n",
    ]) {
      await streamShortChunks(chunk);
    }
    captures.push(await capture(page, "04-stream-overflow"));

    // Phase 3: resize while the response is still streaming, then scroll the
    // terminal buffer and keep streaming. This is the failure shape: repeated
    // absolute full-frame repaints looked fine in a one-shot test, then moved,
    // duplicated, or failed to refresh once xterm had real scrollback plus a resize.
    await page.setViewportSize({ width: 900, height: 320 });
    await page.evaluate(() => window.__turaFit());
    await streamShortChunks("RESIZE_STREAM_MARKER_A 窗口变小时继续逐字刷新。\n");
    await scrollTerminalTo(page, "top");
    await streamShortChunks("RESIZE_STREAM_MARKER_B 滚动后继续逐字刷新。\n");
    captures.push(await capture(page, "05-stream-resize-compact"));

    await page.setViewportSize({ width: 1280, height: 720 });
    await page.evaluate(() => window.__turaFit());
    await streamShortChunks("RESIZE_STREAM_MARKER_C 窗口恢复后继续逐字刷新。\n");
    captures.push(await capture(page, "06-stream-resize-restored"));

    // Phase 4: finalize. All commands complete and the consolidated reply
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
            "再补充几行说明文字，用于把面板内容撑高，以验证滚动与省略行为。\n" +
            "RESIZE_STREAM_MARKER_A 窗口变小时继续逐字刷新。\n" +
            "RESIZE_STREAM_MARKER_B 滚动后继续逐字刷新。\n" +
            "RESIZE_STREAM_MARKER_C 窗口恢复后继续逐字刷新。",
        },
      ],
      created_at: now + 1,
      updated_at: Date.now(),
    });
    session = { ...session, status: "idle" };
    gatewayEvent("session.status", { sessionID, status: "idle" });
    await waitForComposer(page);
    captures.push(await capture(page, "07-final"));

    const bufferText = await terminalBufferText(page);
    for (const item of listItems) {
      const marker = item.match(/SHORT_STREAM_MARKER_\d+/u)?.[0];
      assert.ok(
        markerCount(bufferText, marker) <= 1,
        `${marker} should not duplicate in the active xterm buffer after many short stream redraws`,
      );
    }
    await scrollTerminalTo(page, "top");
    await scrollTerminalTo(page, "bottom");
    const bufferTextAfterScroll = await terminalBufferText(page);
    for (const item of listItems) {
      const marker = item.match(/SHORT_STREAM_MARKER_\d+/u)?.[0];
      assert.ok(
        markerCount(bufferTextAfterScroll, marker) <= 1,
        `${marker} should remain singular after scrolling the terminal buffer`,
      );
    }

    // Phase 4: shrink the same terminal so the transcript overflows without
    // creating a second isolated runtime. The previous test did that, which was
    // a lovely little false-negative machine.
    await page.setViewportSize({ width: 900, height: 320 });
    await page.evaluate(() => window.__turaFit());
    await delay(600);
    await waitForComposer(page);
    captures.push(await capture(page, "10-compact-overflow"));

    const final = captures.find((item) => item.name === "07-final");
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
      const marker = `SHORT_STREAM_MARKER_${String(index).padStart(2, "0")}`;
      assert.ok(
        markerCount(bufferTextAfterScroll, marker) <= 1,
        `list item ${index} should not duplicate in the active terminal buffer`,
      );
    }
    assert.ok(
      /snake_playwright/.test(final.visibleText),
      "later command output should remain visible",
    );
    for (const marker of [
      "RESIZE_STREAM_MARKER_A",
      "RESIZE_STREAM_MARKER_B",
      "RESIZE_STREAM_MARKER_C",
    ]) {
      assert.ok(
        markerCount(bufferTextAfterScroll, marker) <= 1,
        `${marker} should not duplicate after streaming, scrolling, and resizing`,
      );
    }
    await scrollTerminalTo(page, "top");
    captures.push(await capture(page, "08-final-scrolled-top"));
    await scrollTerminalTo(page, "bottom");
    captures.push(await capture(page, "09-final-scrolled-bottom"));

    const visibleStreamIndex = finalLines.findIndex((line) =>
      /SHORT_STREAM_MARKER_|RESIZE_STREAM_MARKER_/u.test(line),
    );
    const commandSummaryIndex = finalLines.findIndex((line) => /命令:|Commands:/.test(line));
    if (visibleStreamIndex >= 0 && commandSummaryIndex >= 0) {
      assert.ok(
        visibleStreamIndex < commandSummaryIndex,
        "command section must stay below visible streamed text",
      );
    }

    const compact = captures.find((item) => item.name === "10-compact-overflow") ?? captures.at(-1);
    assert.equal(compact.overflow, false, "compact terminal should not overflow horizontally");
    assert.equal(compact.hasComposerHint, true, "compact view should keep composer visible");
    assert.equal(compact.hasBottomMeta, true, "compact view should keep bottom meta visible");
    assert.equal(
      compact.hasRawControlLeak,
      false,
      "compact view should not leak raw terminal controls",
    );

    // Phase 5: a second user/assistant turn while the terminal has real
    // scrollback. This covers the production shape that a single long stream
    // missed: stream, scroll away from the bottom, keep streaming, then replace
    // the live overlay with durable text.
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.evaluate(() => window.__turaFit());
    await waitForComposer(page);

    const round2User = {
      id: "msg-user-2",
      sessionID,
      role: "user",
      parts: [{ id: "part-user-2", type: "text", text: "ROUND2_USER_PROMPT 继续第二轮。" }],
      created_at: now + 20,
      updated_at: Date.now(),
    };
    upsertMessage(round2User);
    gatewayEvent("session.status", { sessionID, status: "busy" });

    const round2MessageID = "msg-stream-round-2";
    const round2PartID = "part-stream-round-2";
    const round2Text =
      "ROUND2_STREAM_MARKER_A 第二轮开始。\n" +
      "ROUND2_STREAM_MARKER_B 滚动到顶部时继续输出。\n" +
      "ROUND2_STREAM_MARKER_C 回到底部后完成。\n";
    await streamShortChunks(
      "ROUND2_STREAM_MARKER_A 第二轮开始。\n",
      round2MessageID,
      round2PartID,
      "envelope",
    );
    await scrollTerminalTo(page, "top");
    await streamShortChunks(
      "ROUND2_STREAM_MARKER_B 滚动到顶部时继续输出。\n",
      round2MessageID,
      round2PartID,
      "envelope",
    );
    await scrollTerminalTo(page, "bottom");
    await streamShortChunks(
      "ROUND2_STREAM_MARKER_C 回到底部后完成。\n",
      round2MessageID,
      round2PartID,
      "envelope",
    );
    captures.push(await capture(page, "11-round2-stream-scroll"));

    upsertMessage({
      id: round2MessageID,
      sessionID,
      role: "assistant",
      parts: [
        {
          id: "part-stream-round-2-final",
          type: "text",
          text: round2Text.trimEnd(),
        },
      ],
      created_at: now + 21,
      updated_at: Date.now(),
    });
    session = { ...session, status: "idle" };
    gatewayEvent("session.status", { sessionID, status: "idle" });
    await waitForComposer(page);
    captures.push(await capture(page, "12-round2-final"));

    await scrollTerminalTo(page, "top");
    await scrollTerminalTo(page, "bottom");
    const round2Visible =
      captures.find((item) => item.name === "12-round2-final")?.visibleText ?? "";
    const round2Buffer = await terminalBufferText(page);
    for (const marker of [
      "ROUND2_USER_PROMPT",
      "ROUND2_STREAM_MARKER_A",
      "ROUND2_STREAM_MARKER_B",
      "ROUND2_STREAM_MARKER_C",
    ]) {
      assert.ok(
        markerCount(round2Visible, marker) <= 1,
        `${marker} should not duplicate in the final visible second turn`,
      );
      assert.ok(
        markerCount(round2Buffer, marker) <= 1,
        `${marker} should not duplicate in the terminal buffer after second-turn scrolling`,
      );
    }

    for (const phase of captures) {
      assert.ok(
        phase.composerHintCount <= 1,
        `${phase.name} should not retain an old composer/input box`,
      );
      assert.equal(
        phase.defaultTitleCount,
        0,
        `${phase.name} should not retain the pre-hydrate default title`,
      );
      assert.doesNotMatch(
        phase.visibleText,
        /置{8,}/u,
        `${phase.name} should not leave repeated composer hint fragments behind`,
      );
    }

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
      pageText: page ? await page.evaluate(() => document.body.innerText).catch(() => "") : "",
      pageErrors,
      consoleMessages: consoleMessages.slice(-20),
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
