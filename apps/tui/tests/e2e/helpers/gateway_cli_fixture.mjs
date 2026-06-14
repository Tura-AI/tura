#!/usr/bin/env node
import assert from "node:assert/strict";
import http from "node:http";
import path from "node:path";

export const forbiddenGatewayPaths = [
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

export async function waitForUrl(url, timeoutMs = 10_000) {
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

export async function waitForCondition(predicate, message, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(message);
}

export async function assertTerminalFits(page, label) {
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

export async function terminalViewportText(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll(".xterm-rows > div")]
      .map((node) => node.textContent ?? "")
      .join("\n"),
  );
}

export async function assertTerminalVisualContract(page, profile) {
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

export async function startGateway(runRoot) {
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
