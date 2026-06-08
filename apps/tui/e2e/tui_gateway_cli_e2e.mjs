#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..");
const nodeBin = process.execPath;
const tuiBin = path.join(repoRoot, "apps", "tui", "dist", "index.js");
const webTerminalBin = path.join(repoRoot, "apps", "tui", "scripts", "web-terminal.mjs");
const runRoot = path.join(repoRoot, "target", "tui-minimal-e2e", String(Date.now()));
const tuiRequire = createRequire(path.join(repoRoot, "apps", "tui", "package.json"));
const forbiddenGatewayPaths = [
  "/config",
  "/command",
  "/service/status",
  "/file",
  "/file/content",
  "/file/open",
  "/file/open-location",
  "/path",
  "/skill",
  "/plugin",
];
const richFixtureTextOne =
  "# Rich fixture phase 1\n" +
  "<b>Bold</b> <i>Italic</i> <u>Under</u> <s>Gone</s> and inline <code>code_snippet</code>\n" +
  "- checklist item one\n" +
  "- checklist item two with `src/App.tsx:12`\n" +
  "<blockquote>Cited text or summary</blockquote>\n" +
  "```bash\n" +
  "node tools/snake_playwright.mjs\n" +
  "```\n" +
  "Command fixture complete.";
const richFixtureTextTwo =
  "# Rich fixture phase 2\n" +
  "<a href='https://example.com'>Search Link</a> and [README](https://example.com/readme)\n" +
  "Local path C:/repo/apps/tui and media [MEDIA:C:/tmp/conversation-avatar.png:MEDIA]\n" +
  "| Item | Target |\n" +
  "| --- | --- |\n" +
  "| Directory | C:/repo/apps/tui |\n" +
  "| Docs | [README](https://example.com/readme) |\n" +
  "[EMOJI:sticker:😂:EMOJI] [EMOJI:react:👍:EMOJI]\n" +
  "Protocol fixture complete.";

function richFixtureMessages(sessionID) {
  const now = Date.now();
  return [
    {
      id: "msg-rich-web-1",
      sessionID,
      role: "assistant",
      parts: [
        { id: "part-rich-web-1", type: "text", text: richFixtureTextOne },
        {
          id: "tool-rich-web-1",
          type: "tool",
          tool: "command_run",
          state: {
            status: "completed",
            input: { command_line: "node tools/snake_playwright.mjs" },
            output: "desktop.png ok\nmobile.png ok",
          },
        },
        {
          id: "tool-rich-web-2",
          type: "tool",
          tool: "shell",
          state: {
            status: "running",
            input: { command: "pnpm test -- --rich-fixture" },
            output: { text: "collecting rich terminal screenshots" },
          },
        },
      ],
      tokens: { input: 12, output: 18, reasoning: 4, cache: { read: 5, write: 3 } },
      created_at: now,
      updated_at: now,
    },
    {
      id: "msg-rich-web-2",
      sessionID,
      role: "assistant",
      parts: [{ id: "part-rich-web-2", type: "text", text: richFixtureTextTwo }],
      tokens: { prompt_tokens: 10, completion_tokens: 12 },
      created_at: now + 1,
      updated_at: now + 1,
    },
  ];
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

async function waitForUrl(url, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // Retry until deadline.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`timed out waiting for ${url}`);
}

async function waitForCondition(predicate, message, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(message);
}

async function assertTerminalFits(page, label) {
  const metrics = await page.evaluate(() => {
    const body = document.body;
    const terminal = document.querySelector("#terminal");
    const screen = document.querySelector(".xterm-screen");
    const viewport = document.querySelector(".xterm-viewport");
    return {
      bodyClientWidth: body.clientWidth,
      bodyScrollWidth: body.scrollWidth,
      terminalClientWidth: terminal?.clientWidth ?? 0,
      terminalScrollWidth: terminal?.scrollWidth ?? 0,
      screenClientWidth: screen?.clientWidth ?? 0,
      screenScrollWidth: screen?.scrollWidth ?? 0,
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

async function terminalViewportText(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll(".xterm-rows > div")]
      .map((node) => node.textContent ?? "")
      .join("\n"),
  );
}

async function assertTerminalVisualContract(page, profile) {
  const contract = await page.evaluate(() => {
    const row = document.querySelector(".xterm-rows > div");
    const rowStyle = row ? getComputedStyle(row) : undefined;
    const terminal = globalThis.__turaTerminal;
    const syntheticRailRows = document.querySelectorAll(".xterm-rows > div.tura-rail-row").length;
    return {
      rowOverflowY: rowStyle?.overflowY ?? "",
      lineHeight: terminal?.options?.lineHeight,
      unicodeVersion: terminal?.unicode?.activeVersion ?? "",
      fontFamily: terminal?.options?.fontFamily ?? "",
      syntheticRailRows,
      viewportText: [...document.querySelectorAll(".xterm-rows > div")]
        .map((node) => node.textContent ?? "")
        .join("\n"),
    };
  });
  assert.ok(contract.lineHeight >= 1.18, `${profile} should leave vertical room for emoji`);
  assert.equal(contract.unicodeVersion, "11", `${profile} should render emoji as wide cells`);
  assert.equal(contract.rowOverflowY, "visible", `${profile} should not clip emoji vertically`);
  assert.match(contract.fontFamily, /Emoji/i, `${profile} should include emoji fallback fonts`);
  assert.equal(
    contract.syntheticRailRows,
    0,
    `${profile} should not re-render rails in the web UI`,
  );
  if (profile === "plain") {
    assert.doesNotMatch(contract.viewportText, /▏/u);
  } else {
    assert.match(
      contract.viewportText,
      /▏/u,
      `${profile} should render terminal-native split border`,
    );
  }
}

async function startGateway() {
  const records = {
    createSessions: [],
    prompts: [],
    configPatches: [],
    modelConfigPuts: [],
    providerLogouts: [],
    providerAuthSets: [],
    sessionUpdates: [],
    taskManagementUpdates: [],
    aborts: [],
    agentUpserts: [],
    personaUpserts: [],
    requests: [],
  };
  let config = {
    model: "openai/gpt-test",
    active_model: "openai/gpt-test",
    active_provider: "openai",
    active_agent: "fast",
    model_variant: "medium",
    model_acceleration_enabled: true,
  };
  let session = {
    id: "sess-e2e",
    name: "TUI minimal e2e",
    session_display_name: "TUI minimal e2e",
    directory: runRoot,
    status: "idle",
    model: config.model,
    agent: config.active_agent,
    model_variant: config.model_variant,
    model_acceleration_enabled: config.model_acceleration_enabled,
    created_at: Date.now(),
    updated_at: Date.now(),
    message_count: 2,
  };
  const sessions = [session];
  const messages = richFixtureMessages(session.id);
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
  let modelConfig = {
    path: path.join(runRoot, "provider_config.json"),
    tiers: [
      {
        tier: "fast",
        current: { provider: "openai", model: "gpt-test" },
        options: [
          { provider: "openai", model: "gpt-test", model_name: "GPT Test" },
          { provider: "codex", model: "gpt-5.5", model_name: "GPT 5.5" },
        ],
      },
    ],
  };
  let agent = {
    summary: {
      id: "fast",
      name: "Fast",
      description: "Fast existing runtime agent",
      source: "static",
      path: "agents/src/fast",
      aliases: [],
      capabilities: ["chat"],
      hidden: false,
    },
    config: {
      agent_name: "fast",
      agent_persona: [{ persona_name: "direct", persona_directory: "personas/src/direct" }],
    },
    prompt: "Fast prompt",
  };
  const persona = {
    summary: {
      id: "direct",
      source: "dynamic",
      description: "Direct persona",
      path: "personas/src/direct",
    },
    config: {
      persona_name: "direct",
      persona_directory: "personas/src/direct",
      prompt_directory: "personas/src/direct/prompt",
    },
    persona: "Be direct.",
    communication_style: "Concise.",
  };
  const clients = new Set();
  const emit = (event) => {
    const payload = `data: ${JSON.stringify(event)}\n\n`;
    for (const client of clients) client.write(payload);
  };

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1");
    records.requests.push({
      method: req.method,
      path: url.pathname,
      query: Object.fromEntries(url.searchParams),
    });
    if (req.method === "GET" && url.pathname === "/global/health")
      return sendJson(res, { healthy: true, version: "minimal-e2e" });
    if (req.method === "GET" && url.pathname === "/model_config") return sendJson(res, modelConfig);
    if (req.method === "PUT" && url.pathname === "/model_config") {
      const payload = await readJson(req);
      records.modelConfigPuts.push(payload);
      modelConfig = {
        ...modelConfig,
        tiers: modelConfig.tiers.map((tier) =>
          tier.tier === payload.tier
            ? { ...tier, current: { provider: payload.provider, model: payload.model } }
            : tier,
        ),
      };
      return sendJson(res, modelConfig);
    }
    if (req.method === "GET" && url.pathname === "/project/current")
      return sendJson(res, { project: { worktree: runRoot } });
    if (req.method === "GET" && url.pathname === "/session/config") return sendJson(res, config);
    if (req.method === "PATCH" && url.pathname === "/session/config") {
      const patch = await readJson(req);
      records.configPatches.push(patch);
      config = { ...config, ...patch };
      return sendJson(res, config);
    }
    if (req.method === "GET" && url.pathname === "/session") return sendJson(res, sessions);
    if (req.method === "POST" && url.pathname === "/session") {
      const payload = await readJson(req);
      records.createSessions.push(payload);
      session = {
        ...session,
        id: `sess-created-${records.createSessions.length}`,
        name: "Created Session",
        session_display_name: "Created Session",
        directory: payload.directory ?? runRoot,
        model: payload.model ?? config.model,
        agent: payload.agent ?? config.active_agent,
        model_variant: payload.model_variant ?? config.model_variant,
        model_acceleration_enabled:
          payload.model_acceleration_enabled ?? config.model_acceleration_enabled,
        updated_at: Date.now(),
      };
      sessions.unshift(session);
      messages.length = 0;
      return sendJson(res, session);
    }
    const sessionMatch = url.pathname.match(/^\/session\/([^/]+)$/);
    if (sessionMatch && req.method === "PATCH") {
      const patch = await readJson(req);
      records.sessionUpdates.push(patch);
      session = { ...session, ...patch, updated_at: Date.now() };
      sessions[0] = session;
      return sendJson(res, session);
    }
    const taskManagementMatch = url.pathname.match(/^\/session\/([^/]+)\/task-management$/);
    if (taskManagementMatch && req.method === "PATCH") {
      const patch = await readJson(req);
      records.taskManagementUpdates.push(patch);
      session = { ...session, task_management: patch, updated_at: Date.now() };
      sessions[0] = session;
      return sendJson(res, session);
    }
    const messageMatch = url.pathname.match(/^\/session\/([^/]+)\/message$/);
    if (messageMatch && req.method === "GET") return sendJson(res, messages);
    const promptMatch = url.pathname.match(/^\/session\/([^/]+)\/prompt_async$/);
    if (promptMatch && req.method === "POST") {
      const body = await readJson(req);
      records.prompts.push(body);
      const text = body.parts?.[0]?.text ?? "";
      messages.push({
        id: `msg-user-${records.prompts.length}`,
        sessionID: session.id,
        role: "user",
        parts: [{ id: `part-user-${records.prompts.length}`, type: "text", text }],
        created_at: Date.now(),
        updated_at: Date.now(),
      });
      messages.push({
        id: `msg-assistant-${records.prompts.length}`,
        sessionID: session.id,
        role: "assistant",
        parts: [
          { id: `part-assistant-${records.prompts.length}`, type: "text", text: `final: ${text}` },
        ],
        created_at: Date.now() + 1,
        updated_at: Date.now() + 1,
      });
      emit({
        directory: runRoot,
        payload: { type: "message.updated", properties: { info: messages.at(-1) } },
      });
      return sendJson(res, {});
    }
    const abortMatch = url.pathname.match(/^\/session\/([^/]+)\/abort$/);
    if (abortMatch && req.method === "POST") {
      records.aborts.push(decodeURIComponent(abortMatch[1]));
      return sendJson(res, { ok: true });
    }
    if (req.method === "GET" && url.pathname === "/event") {
      res.writeHead(200, {
        "content-type": "text/event-stream",
        "cache-control": "no-cache",
        connection: "keep-alive",
      });
      clients.add(res);
      res.write(
        `data: ${JSON.stringify({ directory: "global", payload: { type: "server.connected", properties: {} } })}\n\n`,
      );
      req.on("close", () => clients.delete(res));
      return;
    }
    if (req.method === "GET" && url.pathname === "/provider") return sendJson(res, providerList);
    if (req.method === "GET" && url.pathname === "/provider/auth") {
      return sendJson(res, {
        openai: [
          {
            type: "oauth",
            kind: "OAuthPkce",
            login: "oauth",
            label: "OpenAI OAuth",
            token_env: "OPENAI_API_KEY",
          },
        ],
      });
    }
    if (req.method === "GET" && url.pathname === "/provider/openai/auth/status") {
      return sendJson(res, {
        provider_id: "openai",
        configured: true,
        authenticated: true,
        auth_state: "authenticated",
        runtime_state: "ready",
      });
    }
    if (req.method === "POST" && url.pathname === "/provider/openai/oauth/authorize") {
      return sendJson(res, {
        url: "https://auth.example.test/openai",
        method: "auto",
        instructions: "OAuth started.",
      });
    }
    if (req.method === "POST" && url.pathname === "/provider/openai/auth/logout") {
      records.providerLogouts.push("openai");
      return sendJson(res, { ok: true, provider_id: "openai", message: "logged out" });
    }
    if (req.method === "PUT" && url.pathname === "/auth/openai") {
      records.providerAuthSets.push(await readJson(req));
      return sendJson(res, true);
    }
    if (req.method === "GET" && url.pathname === "/agent") return sendJson(res, [agent]);
    if (req.method === "GET" && url.pathname === "/agent/fast") return sendJson(res, agent);
    if (req.method === "POST" && url.pathname === "/agent") {
      const payload = await readJson(req);
      records.agentUpserts.push({ method: "POST", payload });
      return sendJson(res, {
        ...agent,
        summary: {
          ...agent.summary,
          id: payload.id ?? payload.config?.agent_name ?? "created",
          path: `agents/src/${payload.id ?? "created"}`,
        },
        config: {
          ...agent.config,
          ...(payload.config ?? {}),
          agent_name: payload.id ?? payload.config?.agent_name ?? "created",
        },
        prompt: payload.prompt ?? agent.prompt,
      });
    }
    const agentMatch = url.pathname.match(/^\/agent\/([^/]+)$/);
    if (agentMatch && (req.method === "PATCH" || req.method === "PUT")) {
      const payload = await readJson(req);
      const id = decodeURIComponent(agentMatch[1]);
      records.agentUpserts.push({ method: req.method, id, payload });
      const updated = {
        ...agent,
        summary: { ...agent.summary, id, path: `agents/src/${id}` },
        config: { ...agent.config, ...(payload.config ?? {}), agent_name: id },
        prompt: payload.prompt ?? agent.prompt,
      };
      if (id === "fast") agent = updated;
      return sendJson(res, updated);
    }
    if (req.method === "GET" && url.pathname === "/persona") return sendJson(res, [persona]);
    if (req.method === "GET" && url.pathname === "/persona/direct") return sendJson(res, persona);
    if (req.method === "POST" && url.pathname === "/persona") {
      const payload = await readJson(req);
      records.personaUpserts.push({ method: "POST", payload });
      return sendJson(res, {
        ...persona,
        summary: {
          ...persona.summary,
          id: payload.id ?? payload.config?.persona_name ?? "created",
          path: `personas/src/${payload.id ?? "created"}`,
        },
        config: {
          ...persona.config,
          ...(payload.config ?? {}),
          persona_name: payload.id ?? payload.config?.persona_name ?? "created",
        },
        persona: payload.persona ?? persona.persona,
        communication_style: payload.communication_style ?? persona.communication_style,
      });
    }
    const personaMatch = url.pathname.match(/^\/persona\/([^/]+)$/);
    if (personaMatch && (req.method === "PATCH" || req.method === "PUT")) {
      const payload = await readJson(req);
      const id = decodeURIComponent(personaMatch[1]);
      records.personaUpserts.push({ method: req.method, id, payload });
      return sendJson(res, {
        ...persona,
        summary: { ...persona.summary, id, path: `personas/src/${id}` },
        config: { ...persona.config, ...(payload.config ?? {}), persona_name: id },
        persona: payload.persona ?? persona.persona,
        communication_style: payload.communication_style ?? persona.communication_style,
      });
    }
    if (req.method === "POST" && url.pathname === "/project/workspace/select-local") {
      return sendJson(res, { id: "local", name: "Local", worktree: runRoot });
    }
    return sendJson(res, { error: "not found", path: url.pathname }, 404);
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  return {
    url: `http://127.0.0.1:${port}`,
    records,
    seedRichFixture: () => {
      session = {
        ...session,
        id: "sess-rich-web",
        name: "Rich Fixture",
        session_display_name: "Rich Fixture",
        status: "idle",
        message_count: 2,
        updated_at: Date.now(),
      };
      sessions.unshift(session);
      messages.length = 0;
      messages.push(...richFixtureMessages(session.id));
    },
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

function runCli(args, options = {}) {
  return new Promise((resolve, reject) => {
    const startedAt = Date.now();
    const child = spawn(nodeBin, [tuiBin, ...args], {
      cwd: repoRoot,
      env: { ...process.env, ...(options.env ?? {}) },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("error", reject);
    child.on("close", (status) =>
      resolve({ status, stdout, stderr, durationMs: Date.now() - startedAt }),
    );
  });
}

function baseArgs(gateway) {
  return ["--gateway-url", gateway.url, "--cwd", runRoot];
}

async function expectCliOk(args) {
  const result = await runCli(args);
  assert.equal(
    result.status,
    0,
    `expected status=0 for ${args.join(" ")}\nstdout=${result.stdout}\nstderr=${result.stderr}`,
  );
  return result;
}

async function expectCliJson(args) {
  const result = await expectCliOk(args);
  return JSON.parse(result.stdout);
}

async function runWebTerminalE2e(gateway) {
  const { chromium } = tuiRequire("playwright");
  const webPort = 18_000 + Math.floor(Math.random() * 1_000);
  const screenshotsDir = path.join(runRoot, "web-terminal-screenshots");
  await fs.mkdir(screenshotsDir, { recursive: true });
  const draggedImage = path.join(runRoot, "dragged-image.png");
  await fs.writeFile(draggedImage, Buffer.from("89504e470d0a1a0a", "hex"));
  const child = spawn(nodeBin, [webTerminalBin], {
    cwd: repoRoot,
    env: {
      ...process.env,
      PORT: String(webPort),
      TURA_GATEWAY_URL: gateway.url,
      TURA_CWD: runRoot,
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
  try {
    await waitForUrl(`http://127.0.0.1:${webPort}/`);
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    try {
      for (const profile of ["plain", "ansi", "rich"]) {
        await page.setViewportSize({ width: 1280, height: 720 });
        await page.goto(`http://127.0.0.1:${webPort}/${profile}?instance=${profile}-desktop`, {
          waitUntil: "domcontentloaded",
        });
        await page.waitForFunction(
          () =>
            /Rich Fixture|Rich fixture ph\s*ase 1|Rich fixture phase 1/.test(
              document.body.innerText,
            ),
          null,
          { timeout: 10_000 },
        );
        await page.screenshot({
          path: path.join(screenshotsDir, `${profile}.png`),
          fullPage: false,
        });
        await assertTerminalFits(page, `${profile} desktop`);
        await assertTerminalVisualContract(page, profile);
        assert.equal(
          await page.title(),
          `Tura TUI ${profile === "plain" ? "Plain / Safe" : profile === "ansi" ? "ANSI / Default" : "Rich / Modern"}`,
        );
        const body = await page.locator("body").innerText();
        assert.match(body, /OC \| Tura TUI/);
        assert.doesNotMatch(body, /^workspace$/im);
        assert.match(body, /Rich fixture p\s*h\s*a\s*s\s*e 1|Bold\s+Italic\s+Under\s+Gone/i);
        assert.match(
          body,
          /Rich fixture phase 2|Local path C:\/repo\/apps\/tui|Directory\s+C:\/repo\/apps\/tui/,
        );
        assert.match(body, /Protocol fixture complete|Search Link|README/);
        assert.match(body, /commands?:[\s\u00a0]*2|命令:[\s\u00a0]*2/i);
        assert.match(body, /👍/u);
        assert.doesNotMatch(body, /\[EMOJI:/);
        if (profile === "plain") {
          assert.doesNotMatch(body, /[│▏─┌┐└┘├┤┬┴┼]/u);
          assert.doesNotMatch(body, /^-{8,}$/m);
        }
        if (profile === "rich") {
          assert.match(body, /Rich Fixture/);
          assert.match(
            body,
            /Enter to send,.*\/help commands \/settings settings|回车输入，.*\/help查看命令行 \/settings设置/,
          );
          assert.doesNotMatch(body, /\[MEDIA:/);
          assert.doesNotMatch(body, /Agent:fast|智能体:fast/);
          assert.doesNotMatch(body, /persona:direct|人格:direct/);
          const chromeColors = await page.evaluate(() =>
            [...document.querySelectorAll(".dot")].map(
              (node) => getComputedStyle(node).backgroundColor,
            ),
          );
          assert.deepEqual(chromeColors, [
            "rgb(92, 92, 92)",
            "rgb(250, 178, 131)",
            "rgb(92, 92, 92)",
          ]);
        }

        await page.setViewportSize({ width: 820, height: 680 });
        await page.goto(`http://127.0.0.1:${webPort}/${profile}?instance=${profile}-medium`, {
          waitUntil: "domcontentloaded",
        });
        await page.waitForFunction(
          () =>
            /Rich Fixture|Rich fixture ph\s*ase 1|Rich fixture phase 1/.test(
              document.body.innerText,
            ),
          null,
          { timeout: 10_000 },
        );
        await page.screenshot({
          path: path.join(screenshotsDir, `${profile}-medium.png`),
          fullPage: false,
        });
        await assertTerminalFits(page, `${profile} medium`);

        await page.setViewportSize({ width: 390, height: 640 });
        await page.goto(`http://127.0.0.1:${webPort}/${profile}?instance=${profile}-mobile`, {
          waitUntil: "domcontentloaded",
        });
        await page.waitForFunction(() => /Rich Fixture/.test(document.body.innerText), null, {
          timeout: 10_000,
        });
        await page.screenshot({
          path: path.join(screenshotsDir, `${profile}-mobile.png`),
          fullPage: false,
        });
        await assertTerminalFits(page, `${profile} mobile`);
        const mobileViewport = await terminalViewportText(page);
        assert.match(
          mobileViewport,
          /Rich Fixture/,
          `${profile} mobile viewport should keep the session title visible`,
        );
        assert.doesNotMatch(mobileViewport, /(?:\\x1b|\\u001b|8;2;128;128;128m)/);
      }
      await page.setViewportSize({ width: 1280, height: 720 });
      const richCommandInstance = "rich-command";
      const richCommandUrl = `http://127.0.0.1:${webPort}/rich?instance=${richCommandInstance}`;
      const sendRichCommandInput = (data) =>
        page.evaluate((input) => globalThis.__turaSendInput?.(input), data);
      await page.goto(richCommandUrl, { waitUntil: "domcontentloaded" });
      await page.waitForFunction(() => /Rich Fixture/.test(document.body.innerText), null, {
        timeout: 10_000,
      });
      await page.evaluate(() => globalThis.__turaFit?.());
      await sendRichCommandInput("/help\r");
      await page.waitForFunction(
        () =>
          /[─-]{3}\s*(Help|帮助)\s*[─-]{9}/i.test(document.body.innerText) &&
          /(^|\n).*\/chat(?:\s|$)/m.test(document.body.innerText),
        null,
        { timeout: 20_000 },
      );
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-help.png"),
        fullPage: false,
      });
      await assertTerminalFits(page, "rich help");
      {
        const body = await page.locator("body").innerText();
        assert.match(body, /[─-]{3}\s*(Help|帮助)\s*[─-]{9}/i);
        assert.doesNotMatch(body, /^\s*[─-]{8,}\s*$/m);
        assert.match(body, /(^|\n).*\/chat(?:\s|$)/m);
        assert.match(body, /(^|\n).*\/commands(?:\s|$)/m);
        assert.doesNotMatch(body, /system|系统|assistant|助手|user|用户/);
        assert.doesNotMatch(body, /Agent:fast|智能体:fast/);
      }
      await sendRichCommandInput("/chat\r");
      await page.waitForFunction(
        () => !/[─-]{3}\s*(Help|帮助)\s*[─-]{9}/i.test(document.body.innerText),
        null,
        { timeout: 10_000 },
      );
      await page.waitForFunction(
        () => document.body.innerText.includes("node tools/snake_playwright.mjs"),
        null,
        { timeout: 10_000 },
      );
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-commands-expanded.png"),
        fullPage: false,
      });
      {
        const body = await page.locator("body").innerText();
        assert.match(body, /node tools\/snake_playwright\.mjs/);
        assert.match(body, /pnpm test -- --rich-fixture|#2\s+shell running/);
        assert.doesNotMatch(body, /◆\s+◇\s+命令|◆\s+◇\s+Commands/);
      }
      await sendRichCommandInput("/models\r");
      await page.waitForTimeout(1200);
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-models.png"),
        fullPage: false,
      });
      await sendRichCommandInput("/chat\r");
      await page.waitForFunction(
        () => document.body.innerText.includes("node tools/snake_playwright.mjs"),
        null,
        { timeout: 10_000 },
      );
      await page.waitForTimeout(300);
      await sendRichCommandInput("\u0015/settings\r");
      await page.waitForFunction(
        () => /[─-]{3}\s*(Session Settings|会话设置)\s*[─-]{9}/.test(document.body.innerText),
        null,
        { timeout: 10_000 },
      );
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-settings.png"),
        fullPage: false,
      });
      {
        const body = await page.locator("body").innerText();
        assert.match(body, /[─-]{3}\s*(Session Settings|会话设置)\s*[─-]{9}/);
        assert.doesNotMatch(body, /^\s*[─-]{8,}\s*$/m);
        assert.doesNotMatch(body, /\/config get|\/config set|\/model provider\/model/);
      }
      await sendRichCommandInput("\u001b");
      await page.waitForFunction(
        () => document.body.innerText.includes("node tools/snake_playwright.mjs"),
        null,
        { timeout: 10_000 },
      );
      await sendRichCommandInput("/auth\r");
      await page.waitForTimeout(1200);
      await page.screenshot({ path: path.join(screenshotsDir, "rich-auth.png"), fullPage: false });
      await sendRichCommandInput("\u001b");
      await page.waitForFunction(
        () => document.body.innerText.includes("node tools/snake_playwright.mjs"),
        null,
        { timeout: 10_000 },
      );
      await sendRichCommandInput("/sessions\r");
      await page.waitForFunction(
        () => /[─-]{3}\s*(Sessions|会话)\s*[─-]{9}/.test(document.body.innerText),
        null,
        { timeout: 10_000 },
      );
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-sessions.png"),
        fullPage: false,
      });
      {
        const body = await page.locator("body").innerText();
        assert.match(body, /[─-]{3}\s*(Sessions|会话)\s*[─-]{9}/);
        assert.doesNotMatch(body, /^\s*[─-]{8,}\s*$/m);
      }
      await sendRichCommandInput("\u001b");
      await page.waitForFunction(
        () => document.body.innerText.includes("node tools/snake_playwright.mjs"),
        null,
        { timeout: 10_000 },
      );
      await sendRichCommandInput("/personas\r");
      await page.waitForFunction(
        () => /Direct persona|Concise|direct/.test(document.body.innerText),
        null,
        { timeout: 10_000 },
      );
      const personaBody = await page.locator("body").innerText();
      assert.match(personaBody, /Direct persona|Concise|direct/);
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-personas.png"),
        fullPage: false,
      });
      const configPatchCount = gateway.records.configPatches.length;
      const agentPatchCount = gateway.records.agentUpserts.length;
      await sendRichCommandInput("/persona direct\r");
      await page.waitForTimeout(1200);
      assert.equal(gateway.records.configPatches.length, configPatchCount);
      const agentPatch = gateway.records.agentUpserts.slice(agentPatchCount).at(-1);
      assert.equal(agentPatch?.id, "fast");
      assert.deepEqual(agentPatch.payload.config.agent_persona, [
        {
          persona_name: "direct",
          persona_directory: "personas/src/direct",
        },
      ]);
      await sendRichCommandInput("/chat\r");
      await page.waitForTimeout(150);
      const promptCountBeforeMedia = gateway.records.prompts.length;
      await sendRichCommandInput(`${draggedImage}\r`);
      await waitForCondition(
        () => gateway.records.prompts.length > promptCountBeforeMedia,
        "timed out waiting for dragged image prompt",
      );
      assert.match(
        gateway.records.prompts.at(-1)?.parts?.[0]?.text ?? "",
        /\[MEDIA:.*dragged-image\.png:MEDIA\]/,
      );
    } finally {
      await browser.close();
    }
    return screenshotsDir;
  } finally {
    child.kill();
    await new Promise((resolve) => child.once("exit", resolve));
    await fs.writeFile(path.join(runRoot, "web-terminal.log"), logs);
  }
}

async function main() {
  await fs.mkdir(runRoot, { recursive: true });
  await fs.access(tuiBin);
  const gateway = await startGateway();
  try {
    const help = await expectCliOk([...baseArgs(gateway), "--lang", "en", "help"]);
    for (const command of ["project", "file", "persona", "command", "inspect", "gateway"]) {
      assert.match(help.stdout, new RegExp(`^  ${command}\\s`, "m"));
    }
    assert.match(help.stdout, /agent\s+list, read, create, update, or tier agents/);
    assert.match(help.stdout, /session\s+list or show sessions/);
    const zhHelp = await expectCliOk([...baseArgs(gateway), "--lang", "zh-CN", "help"]);
    assert.match(zhHelp.stdout, /命令:/);
    assert.match(zhHelp.stdout, /agent\s+列出、读取、创建、更新或配置智能体档位/);

    const config = await expectCliJson([...baseArgs(gateway), "--json", "config", "get"]);
    assert.equal(config.active_agent, "fast");
    const patched = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "config",
      "set",
      "agent=fast",
      "model_variant=low",
    ]);
    assert.equal(patched.active_agent, "fast");
    assert.equal(gateway.records.configPatches[0].model_variant, "low");
    const rejectedTheme = await runCli([...baseArgs(gateway), "config", "set", "theme=dark"]);
    assert.notEqual(rejectedTheme.status, 0);
    assert.match(
      rejectedTheme.stderr + rejectedTheme.stdout,
      /不支持的会话配置键|unsupported session config key/,
    );
    const rejectedPlanning = await runCli([
      ...baseArgs(gateway),
      "--lang",
      "en",
      "config",
      "set",
      "planning=on",
    ]);
    assert.notEqual(rejectedPlanning.status, 0);
    assert.match(
      rejectedPlanning.stderr + rejectedPlanning.stdout,
      /unsupported session config key/,
    );
    const rejectedPlanningZh = await runCli([
      ...baseArgs(gateway),
      "--lang",
      "zh-CN",
      "config",
      "set",
      "planning=on",
    ]);
    assert.notEqual(rejectedPlanningZh.status, 0);
    assert.match(rejectedPlanningZh.stderr + rejectedPlanningZh.stdout, /不支持的会话配置键/);
    const tiers = await expectCliJson([...baseArgs(gateway), "--json", "config", "model-tiers"]);
    assert.equal(tiers.tiers[0].tier, "fast");
    const tierOptions = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "config",
      "model-tier",
      "fast",
    ]);
    assert.equal(tierOptions.tier, "fast");
    const tierUpdated = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "config",
      "model-tier",
      "fast",
      "codex/gpt-5.5",
    ]);
    assert.equal(tierUpdated.tiers[0].current.provider, "codex");
    assert.deepEqual(gateway.records.modelConfigPuts.at(-1), {
      tier: "fast",
      provider: "codex",
      model: "gpt-5.5",
    });

    const sessions = await expectCliJson([...baseArgs(gateway), "--json", "session", "list"]);
    assert.equal(sessions[0].id, "sess-e2e");
    const shown = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "session",
      "show",
      "sess-e2e",
    ]);
    assert.equal(shown.session.id, "sess-e2e");
    assert.match(
      shown.messages
        .map((message) => message.parts.map((part) => part.text ?? "").join("\n"))
        .join("\n"),
      /Rich fixture phase 1[\s\S]*Protocol fixture complete/,
    );
    const updatedSession = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "session",
      "update",
      "sess-e2e",
      "--data",
      '{"agent":"fast"}',
    ]);
    assert.equal(updatedSession.agent, "fast");
    const taskSession = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "session",
      "task-management",
      "sess-e2e",
      "--data",
      '{"status":"doing"}',
    ]);
    assert.equal(taskSession.task_management.status, "doing");

    const providers = await expectCliJson([...baseArgs(gateway), "--json", "provider", "list"]);
    assert.equal(providers.all[0].id, "openai");
    const providerStatus = await expectCliJson([
      ...baseArgs(gateway),
      "provider",
      "status",
      "openai",
    ]);
    assert.equal(providerStatus.authenticated, true);
    const providerLogin = await expectCliOk([
      ...baseArgs(gateway),
      "provider",
      "login",
      "openai",
      "--no-open",
    ]);
    assert.match(providerLogin.stdout, /OAuth started/);
    assert.match(providerLogin.stdout, /authenticated/);
    const logout = await expectCliJson([...baseArgs(gateway), "provider", "logout", "openai"]);
    assert.equal(logout.ok, true);
    const authSet = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "provider",
      "set-auth",
      "openai",
      "--key",
      "sk-test",
    ]);
    assert.equal(authSet.saved, true);
    assert.equal(gateway.records.providerAuthSets.at(-1).key, "sk-test");

    const agents = await expectCliJson([...baseArgs(gateway), "--json", "agent", "list"]);
    assert.equal(agents[0].summary.name, "Fast");
    const agent = await expectCliJson([...baseArgs(gateway), "--json", "agent", "show", "fast"]);
    assert.equal(agent.summary.id, "fast");
    const createdAgent = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "agent",
      "create",
      "dynamic-fast",
      "--config",
      '{"description":"Dynamic fast"}',
      "--prompt",
      "Prompt text",
    ]);
    assert.equal(createdAgent.summary.id, "dynamic-fast");
    const updatedAgent = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "agent",
      "update",
      "dynamic-fast",
      "--config",
      '{"description":"Updated"}',
    ]);
    assert.equal(updatedAgent.config.agent_name, "dynamic-fast");

    const personas = await expectCliJson([...baseArgs(gateway), "--json", "persona", "list"]);
    assert.equal(personas[0].summary.id, "direct");
    const createdPersona = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "persona",
      "create",
      "brief",
      "--persona",
      "Be brief.",
    ]);
    assert.equal(createdPersona.summary.id, "brief");
    const updatedPersona = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "persona",
      "update",
      "brief",
      "--communication-style",
      "Compact.",
    ]);
    assert.equal(updatedPersona.communication_style, "Compact.");

    const localProject = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "project",
      "select-local",
      "--title",
      "Pick workspace",
    ]);
    assert.equal(localProject.worktree, runRoot);

    const resume = await expectCliOk([...baseArgs(gateway), "resume", "sess-e2e"]);
    assert.match(resume.stdout, /Protocol fixture complete/);

    const run = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "run",
      "hello minimal tui",
      "--no-stream",
      "--timeout",
      "5",
    ]);
    assert.equal(run.status, "completed");
    assert.equal(run.finalText, "final: hello minimal tui");
    assert.equal("force_planning" in gateway.records.createSessions.at(-1), false);

    for (const shell of ["bash", "zsh", "fish"]) {
      const completion = await expectCliOk(["completion", shell]);
      assert.match(completion.stdout, /tura|complete|_arguments/);
      assert.match(completion.stdout, /gateway|persona|project/);
    }

    const gatewayClientModule = await import(
      pathToFileURL(path.join(repoRoot, "apps", "tui", "dist", "gateway", "client.js")).href
    );
    const client = new gatewayClientModule.GatewayClient({
      baseUrl: gateway.url,
      directory: runRoot,
      timeoutMs: 5000,
    });
    assert.equal((await client.health()).version, "minimal-e2e");
    await client.syncWorkspace();
    assert.equal((await client.getSessionConfig()).active_agent, "fast");
    assert.equal(
      (await client.patchSessionConfig({ model_variant: "medium" })).model_variant,
      "medium",
    );
    assert.equal(
      (await client.listSessions({ includeChildren: true, limit: 5 }))[0].id,
      "sess-created-1",
    );
    assert.equal((await client.getSession("sess-e2e")).id, "sess-e2e");
    assert.equal((await client.updateSession("sess-e2e", { agent: "fast" })).agent, "fast");
    assert.equal((await client.listMessages("sess-e2e")).at(-1).role, "assistant");
    assert.equal((await client.listProviders()).all[0].id, "openai");
    assert.equal((await client.listProviderAuthMethods()).openai[0].login, "oauth");
    assert.equal((await client.providerAuthStatus("openai")).authenticated, true);
    assert.equal(
      (await client.providerOauthAuthorize("openai", 0)).url,
      "https://auth.example.test/openai",
    );
    assert.equal((await client.providerLogout("openai")).ok, true);
    assert.equal((await client.listAgents())[0].summary.id, "fast");
    assert.equal((await client.getAgent("fast")).summary.id, "fast");
    await client.abort("sess-e2e");
    assert.ok(gateway.records.aborts.includes("sess-e2e"));

    gateway.seedRichFixture();
    const requestCountBeforeWebTerminal = gateway.records.requests.length;
    const screenshotsDir = await runWebTerminalE2e(gateway);
    const forbiddenRequests = gateway.records.requests
      .slice(requestCountBeforeWebTerminal)
      .filter(
        (request) =>
          forbiddenGatewayPaths.some(
            (pathName) => request.path === pathName || request.path.startsWith(`${pathName}/`),
          ) || /\/session\/[^/]+\/task-management/.test(request.path),
      );
    assert.deepEqual(forbiddenRequests, []);
    console.log(`[tui-minimal-e2e] ok=true screenshots=${screenshotsDir}`);
  } finally {
    await gateway.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
