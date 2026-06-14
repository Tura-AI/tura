#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import {
  assertNoDuplicatedFrameText,
  assertSessionPickerCleared,
  delay,
  listen,
  markerCount,
  regexCount,
  scrollTerminalTo,
  seedTerminalScrollback,
  startWebTerminal,
  submitTypedPrompt,
  terminalBufferText,
  terminalText,
  waitForTerminalBufferText,
  waitForComposer,
  waitForSessionPicker,
  waitForUrl,
} from "../helpers/mock_stream_terminal.mjs";

const repoRoot =
  process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..", "..");
const runRoot = path.join(
  repoRoot,
  "apps",
  "tui",
  "test-results",
  "tui-mock-stream",
  String(Date.now()),
);
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
  model: "mock/gpt-test",
  agent: "fast",
  model_variant: "medium",
  model_acceleration_enabled: true,
  created_at: now,
  updated_at: now,
  message_count: 1,
};
let promptCounter = 0;
const config = {
  model: "mock/gpt-test",
  active_model: "mock/gpt-test",
  active_provider: "mock",
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
    parts: [
      { id: "part-user-1", type: "text", text: "Trigger mock gateway stream text and commands." },
    ],
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
    index === 1
      ? "TYPED_REPLY_1 Hello. What are we working on today?"
      : "TYPED_REPLY_2 Continue the second round.";
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

function createGatewayServer() {
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
    hasComposerHint: /Enter to send/.test(text),
    composerHintCount: regexCount(text, /Enter to send/gu),
    defaultTitleCount: regexCount(text, /^tura$/gmu),
    hasBottomMeta: /tokens|mock\/gpt-test/.test(text),
    hasCommand: /\bCommands\b|shell_command/.test(text),
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
  const web = startWebTerminal({
    repoRoot,
    workspace,
    gatewayUrl: `http://127.0.0.1:${gatewayPort}`,
    port: webPort,
  });
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

    await submitTypedPrompt(page, "TYPED_USER_1 hello there");
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

    await submitTypedPrompt(page, "TYPED_USER_2 continue the second round");
    await waitForTerminalBufferText(page, "TYPED_REPLY_2 Continue the second round.", 15_000);
    await waitForComposer(page);
    await scrollTerminalTo(page, "bottom", "TYPED_REPLY_2");
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
    const streamStartedAt = Date.now();

    // Phase 1: stream an intro plus a multi-item list. The whole list must stay
    // visible — assistant text used to be capped at 8 lines, hiding the rest.
    const listIntro = "First stream text, then a checklist of steps to execute:\n";
    const listItems = Array.from(
      { length: 10 },
      (_item, index) =>
        `- Step ${index + 1}: SHORT_STREAM_MARKER_${String(index + 1).padStart(2, "0")}`,
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
      { id: "msg-command-1", cmd: "Get-Content -Raw .tura/config.conf", at: streamStartedAt + 2 },
      { id: "msg-command-2", cmd: "npm run build -- --watch", at: streamStartedAt + 3 },
      {
        id: "msg-command-3",
        cmd: "node tools/snake_playwright.mjs --steps 40",
        at: streamStartedAt + 4,
      },
    ];
    const commandPart = (entry, status) => ({
      id: `part-${entry.id}`,
      type: "tool",
      tool: "command_run",
      state: {
        status,
        input: { command_type: "shell_command", command_line: entry.cmd },
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

    await streamShortChunks(
      "Keep streaming while commands run; text must not be covered by command output.\n",
    );
    upsertCommand(commands[1], "running");
    captures.push(await capture(page, "03-command-2-running"));

    upsertCommand(commands[0], "completed");
    upsertCommand(commands[2], "running");
    for (const chunk of [
      "Add a few more explanation lines, ",
      "to make the panel taller, ",
      "so scrolling and truncation behavior is covered.\n",
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
    await streamShortChunks("RESIZE_STREAM_MARKER_A keeps streaming while the viewport shrinks.\n");
    await scrollTerminalTo(page, "top");
    await streamShortChunks("RESIZE_STREAM_MARKER_B keeps streaming after scrolling.\n");
    captures.push(await capture(page, "05-stream-resize-compact"));

    await page.setViewportSize({ width: 1280, height: 720 });
    await page.evaluate(() => window.__turaFit());
    await streamShortChunks(
      "RESIZE_STREAM_MARKER_C keeps streaming after restoring the viewport.\n",
    );
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
            "\nKeep streaming while commands run; text must not be covered by command output.\n" +
            "Add a few more explanation lines to make the panel taller and cover scrolling/truncation.\n" +
            "RESIZE_STREAM_MARKER_A keeps streaming while the viewport shrinks.\n" +
            "RESIZE_STREAM_MARKER_B keeps streaming after scrolling.\n" +
            "RESIZE_STREAM_MARKER_C keeps streaming after restoring the viewport.",
        },
      ],
      created_at: streamStartedAt + 1,
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
      /\bCommands\b|shell_command/.test(bufferTextAfterScroll),
      "command should remain in terminal buffer after later stream text",
    );
    assert.ok(
      /snake_playwright/.test(bufferTextAfterScroll),
      "later command output should remain in terminal buffer",
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
    const commandSummaryIndex = finalLines.findIndex((line) => /\bCommands\b/.test(line));
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
      parts: [
        { id: "part-user-2", type: "text", text: "ROUND2_USER_PROMPT continue the second round." },
      ],
      created_at: now + 20,
      updated_at: Date.now(),
    };
    upsertMessage(round2User);
    gatewayEvent("session.status", { sessionID, status: "busy" });

    const round2MessageID = "msg-stream-round-2";
    const round2PartID = "part-stream-round-2";
    const round2Text =
      "ROUND2_STREAM_MARKER_A second round starts.\n" +
      "ROUND2_STREAM_MARKER_B keeps streaming while scrolled to top.\n" +
      "ROUND2_STREAM_MARKER_C finishes after returning to bottom.\n";
    await streamShortChunks(
      "ROUND2_STREAM_MARKER_A second round starts.\n",
      round2MessageID,
      round2PartID,
      "envelope",
    );
    await scrollTerminalTo(page, "top");
    await streamShortChunks(
      "ROUND2_STREAM_MARKER_B keeps streaming while scrolled to top.\n",
      round2MessageID,
      round2PartID,
      "envelope",
    );
    await scrollTerminalTo(page, "bottom");
    await streamShortChunks(
      "ROUND2_STREAM_MARKER_C finishes after returning to bottom.\n",
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
    await scrollTerminalTo(page, "bottom");
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
    await browser?.close().catch(() => undefined);
    web.child.kill();
    gateway.close();
  }
}

await main();
