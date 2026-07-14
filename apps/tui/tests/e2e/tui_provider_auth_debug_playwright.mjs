#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import {
  terminalBufferText,
  waitForCondition,
  waitForUrl,
} from "./helpers/gateway_cli_fixture.mjs";

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..");
const appRoot = path.join(repoRoot, "apps", "tui");
const nodeBin = process.execPath;
const webTerminalBin = path.join(appRoot, "scripts", "web-terminal.mjs");
const runRoot = path.join(appRoot, "test-results", "tui-provider-auth-debug", String(Date.now()));
const tuiRequire = createRequire(path.join(appRoot, "package.json"));

async function main() {
  await fs.mkdir(runRoot, { recursive: true });
  const gateway = await startAuthGateway(runRoot);
  const webPort = 19_000 + Math.floor(Math.random() * 1_000);
  const webLogs = { stdout: "", stderr: "" };
  const web = spawn(nodeBin, [webTerminalBin], {
    cwd: repoRoot,
    env: {
      ...process.env,
      PORT: String(webPort),
      TURA_CWD: runRoot,
      TURA_GATEWAY_URL: gateway.url,
      TURA_LANG: "en",
      TURA_DISABLE_OPEN_EXTERNAL_URL: "1",
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  web.stdout.on("data", (chunk) => {
    webLogs.stdout += chunk.toString();
  });
  web.stderr.on("data", (chunk) => {
    webLogs.stderr += chunk.toString();
  });

  const { chromium } = tuiRequire("playwright");
  let browser;
  try {
    await waitForUrl(`http://127.0.0.1:${webPort}/`);
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1180, height: 720 } });
    await page.goto(`http://127.0.0.1:${webPort}/rich?instance=provider-auth-debug`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => /Auth Debug Session/.test(document.body.innerText), null, {
      timeout: 15_000,
    });
    await sendInput(page, "/login codex 0\r");
    await page.waitForFunction(
      (url) => document.body.innerText.replace(/[▏\s]/gu, "").includes(url.replace(/\s+/g, "")),
      gateway.oauthUrl,
      { timeout: 15_000 },
    );
    const oauthTerminalText = await terminalBufferText(page);
    assert.match(
      oauthTerminalText.replace(/[▏\s]/gu, ""),
      new RegExp(escapeRegExp(gateway.oauthUrl.replace(/\s+/g, ""))),
    );
    assert.equal(gateway.records.oauthAuthorize.length, 1);

    await clickOAuthTerminalLink(page, gateway.oauthUrl);
    const oauthPage = await browser.newPage();
    await oauthPage.goto(gateway.oauthUrl, { waitUntil: "domcontentloaded" });
    await oauthPage.getByRole("button", { name: "Authorize" }).click();
    await oauthPage.waitForFunction(() => /OAuth connected/.test(document.body.innerText), null, {
      timeout: 10_000,
    });
    await oauthPage.close();
    await page.waitForFunction(() => /connected/i.test(document.body.innerText), null, {
      timeout: 15_000,
    });
    assert.equal(gateway.records.oauthCallback.length, 0);
    assert.equal(gateway.authenticated, true);

    const tokenPage = await browser.newPage({ viewport: { width: 1180, height: 720 } });
    await tokenPage.goto(`http://127.0.0.1:${webPort}/rich?instance=provider-token-debug`, {
      waitUntil: "domcontentloaded",
    });
    await tokenPage.waitForFunction(
      () => /Auth Debug Session/.test(document.body.innerText),
      null,
      {
        timeout: 15_000,
      },
    );
    await sendInput(tokenPage, "/provider set-auth openai --key debug-token\r");
    await waitForCondition(
      () => gateway.records.providerAuthValidate.length > 0,
      "TUI did not validate the token login",
      10_000,
    );
    assert.deepEqual(gateway.records.providerAuthValidate.at(-1), {
      type: "api_key",
      kind: "api_key",
      login: "api",
      key: "debug-token",
      access: "debug-token",
    });
    await waitForCondition(
      () => gateway.records.providerAuthSets.length > 0,
      "TUI did not save the validated token",
      10_000,
    );
    assert.deepEqual(gateway.records.providerAuthSets.at(-1), {
      type: "api_key",
      key: "debug-token",
    });
    await tokenPage.close();

    console.log(
      JSON.stringify(
        {
          ok: true,
          gateway: gateway.url,
          checked: ["tui-oauth-url-clickable", "tui-oauth-status-poll", "tui-token-login"],
          oauthCallbackPayload: gateway.records.oauthCallback.at(-1),
        },
        null,
        2,
      ),
    );
  } finally {
    if (browser) await browser.close();
    web.kill();
    await new Promise((resolve) => web.once("exit", resolve));
    await fs.writeFile(path.join(runRoot, "web-terminal.stdout.log"), webLogs.stdout);
    await fs.writeFile(path.join(runRoot, "web-terminal.stderr.log"), webLogs.stderr);
    await gateway.close();
  }
}

async function sendInput(page, input) {
  await page.evaluate((value) => globalThis.__turaSendInput?.(value), input);
}

async function clickOAuthTerminalLink(page, url) {
  const rows = await page.evaluate(() => {
    const rows = [...document.querySelectorAll(".xterm-rows > div")];
    return rows
      .filter((item) => /debug-oauth\/authorize|client_id=tura-debug/u.test(item.textContent ?? ""))
      .map((row) => {
        const rect = row.getBoundingClientRect();
        return {
          text: row.textContent ?? "",
          left: rect.left,
          right: rect.right,
          top: rect.top,
          height: rect.height,
        };
      });
  });
  assert.ok(rows.length, "OAuth URL row was not visible in the terminal");
  let matchedPoint;
  let lastHovered = "";
  for (const row of rows) {
    const y = row.top + row.height / 2;
    for (let x = row.left + 4; x < row.right - 4; x += 8) {
      await page.mouse.move(x, y);
      await page.waitForTimeout(60);
      const hovered = await page.evaluate(() => globalThis.__turaHoveredLink || "");
      if (hovered) lastHovered = hovered;
      if (hovered === url) {
        matchedPoint = { x, y };
        break;
      }
    }
    if (matchedPoint) break;
  }
  assert.ok(
    matchedPoint,
    `OAuth URL was visible but not exposed as one complete clickable link. lastHovered=${lastHovered} rows=${JSON.stringify(rows.map((row) => row.text))}`,
  );
  await page.mouse.click(matchedPoint.x, matchedPoint.y);
  await page.waitForFunction(
    (expected) => globalThis.__turaActivatedLinks?.includes(expected),
    url,
    { timeout: 5_000 },
  );
}

async function startAuthGateway(runRoot) {
  const records = {
    oauthAuthorize: [],
    oauthCallback: [],
    providerAuthValidate: [],
    providerAuthSets: [],
    requests: [],
  };
  let authenticated = false;
  let oauthCompleted = false;
  let server;
  const oauthState = "debug-state-123";
  const oauthCode = "debug-code-456";
  const config = {
    model: "openai/gpt-debug",
    active_model: "openai/gpt-debug",
    active_provider: "openai",
    active_agent: "direct",
    model_variant: "medium",
    model_acceleration_enabled: false,
  };
  const session = {
    id: "sess-auth-debug",
    name: "Auth Debug Session",
    session_display_name: "Auth Debug Session",
    directory: runRoot,
    status: "idle",
    model: config.model,
    agent: config.active_agent,
    model_variant: config.model_variant,
    model_acceleration_enabled: config.model_acceleration_enabled,
    context_tokens: { input: 0, limit: 200_000 },
    created_at: Date.now(),
    updated_at: Date.now(),
    message_count: 1,
  };
  const messages = [
    {
      id: "msg-auth-debug",
      sessionID: session.id,
      role: "assistant",
      parts: [{ id: "part-auth-debug", type: "text", text: "Auth Debug Session" }],
      created_at: Date.now(),
      updated_at: Date.now(),
    },
  ];
  server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1");
    records.requests.push({ method: req.method, path: url.pathname });
    if (req.method === "GET" && url.pathname === "/global/health")
      return sendJson(res, { healthy: true, version: "auth-debug" });
    if (req.method === "GET" && url.pathname === "/project/current")
      return sendJson(res, { project: { worktree: runRoot } });
    if (req.method === "GET" && url.pathname === "/model_config")
      return sendJson(res, { path: path.join(runRoot, "provider_config.json"), tiers: [] });
    if (req.method === "GET" && url.pathname === "/session/config") return sendJson(res, config);
    if (req.method === "GET" && url.pathname === "/session") return sendJson(res, [session]);
    if (req.method === "GET" && url.pathname === `/session/${session.id}`)
      return sendJson(res, session);
    if (req.method === "GET" && url.pathname === `/session/${session.id}/message`)
      return sendJson(res, messages);
    if (req.method === "GET" && (url.pathname === "/event" || url.pathname.endsWith("/events"))) {
      res.writeHead(200, {
        "content-type": "text/event-stream",
        "cache-control": "no-cache",
        connection: "keep-alive",
      });
      res.write(
        `data: ${JSON.stringify({ directory: "global", payload: { type: "server.connected", properties: {} } })}\n\n`,
      );
      return;
    }
    if (req.method === "GET" && url.pathname === "/provider") {
      return sendJson(res, {
        all: [
          {
            id: "codex",
            name: "Codex Subscription",
            source: "debug",
            env: ["OPENAI_API_KEY"],
            options: { domains: ["llm"] },
            models: { "gpt-debug": { id: "gpt-debug", name: "gpt-debug" } },
          },
          {
            id: "openai",
            name: "OpenAI API",
            source: "debug",
            env: ["OPENAI_API_KEY"],
            options: { domains: ["llm"] },
            models: { "gpt-api-debug": { id: "gpt-api-debug", name: "gpt-api-debug" } },
          },
        ],
        default: { codex: "gpt-debug", openai: "gpt-api-debug" },
        connected: authenticated ? ["codex", "openai"] : [],
        enums: { domains: [], capabilities: [], api_styles: [], auth_methods: [], statuses: [] },
      });
    }
    if (req.method === "GET" && url.pathname === "/provider/auth") {
      return sendJson(res, {
        codex: [
          {
            type: "oauth",
            kind: "OAuthPkce",
            login: "oauth",
            label: "Debug OAuth",
            available: true,
          },
        ],
        openai: [
          {
            type: "api_key",
            kind: "api_key",
            login: "api",
            label: "Debug token",
            available: true,
          },
        ],
      });
    }
    if (req.method === "GET" && url.pathname === "/provider/codex/auth/status")
      return sendJson(res, status());
    if (req.method === "GET" && url.pathname === "/provider/openai/auth/status")
      return sendJson(res, status());
    if (req.method === "POST" && url.pathname === "/provider/codex/oauth/authorize") {
      records.oauthAuthorize.push(await readJson(req));
      return sendJson(res, {
        url: authUrl(server, oauthState),
        method: "auto",
        instructions: "Complete OpenAI authorization in the browser.",
      });
    }
    if (req.method === "GET" && url.pathname === "/debug-oauth/authorize") {
      return sendHtml(
        res,
        `<!doctype html><title>Debug OAuth</title><h1>Debug OAuth</h1><button>Authorize</button><script>document.querySelector("button").onclick=()=>location.href="/auth/callback?code=${oauthCode}&state=${oauthState}"</script>`,
      );
    }
    if (req.method === "GET" && url.pathname === "/auth/callback") {
      assert.equal(url.searchParams.get("code"), oauthCode);
      assert.equal(url.searchParams.get("state"), oauthState);
      authenticated = true;
      oauthCompleted = true;
      return sendHtml(res, "<!doctype html><title>OAuth connected</title><h1>OAuth connected</h1>");
    }
    if (req.method === "POST" && url.pathname === "/provider/codex/oauth/callback") {
      const payload = await readJson(req);
      records.oauthCallback.push(payload);
      const deadline = Date.now() + 10_000;
      while (!oauthCompleted && Date.now() < deadline) await sleep(100);
      authenticated = oauthCompleted;
      return sendJson(res, {
        ok: oauthCompleted,
        provider_id: "codex",
        code: oauthCompleted ? "provider.oauth.completed" : "provider.oauth.code_missing",
        message: oauthCompleted
          ? "provider OAuth completed"
          : "Paste the copied authorization code before submitting",
        level: oauthCompleted ? "valid" : "invalid",
        status: status(),
      });
    }
    if (req.method === "POST" && url.pathname === "/provider/openai/auth/validate") {
      const payload = await readJson(req);
      records.providerAuthValidate.push(payload);
      const ok = payload.key === "debug-token";
      return sendJson(res, {
        ok,
        provider_id: "openai",
        code: ok ? "provider.validation.passed" : "provider.validation.failed",
        message: ok ? "debug token accepted" : "debug token rejected",
        level: ok ? "valid" : "invalid",
        status: { ...status(), configured: ok, authenticated: ok },
      });
    }
    if (req.method === "PUT" && url.pathname === "/auth/openai") {
      records.providerAuthSets.push(await readJson(req));
      authenticated = true;
      return sendJson(res, true);
    }
    if (req.method === "POST" && url.pathname === "/provider/openai/auth/logout") {
      authenticated = false;
      return sendJson(res, { ok: true, provider_id: "openai", message: "logged out" });
    }
    if (req.method === "GET" && url.pathname === "/agent") return sendJson(res, []);
    if (req.method === "GET" && url.pathname === "/persona") return sendJson(res, []);
    return sendJson(res, { error: "not found", path: url.pathname }, 404);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  return {
    url: origin,
    oauthUrl: authUrl(server, oauthState),
    records,
    get authenticated() {
      return authenticated;
    },
    close: () => new Promise((resolve) => server.close(resolve)),
  };

  function status() {
    return {
      provider_id: "openai",
      configured: authenticated,
      authenticated,
      auth_state: authenticated ? "authenticated" : "missing",
      runtime_state: authenticated ? "ready" : "missing",
    };
  }
}

function authUrl(server, state) {
  const origin = `http://127.0.0.1:${server.address().port}`;
  return `${origin}/debug-oauth/authorize?client_id=tura-debug&redirect_uri=${encodeURIComponent(`${origin}/auth/callback`)}&scope=openid%20profile%20email%20offline_access&state=${state}&code_challenge=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~&code_challenge_method=S256`;
}

function sendJson(res, value, status = 200) {
  const body = JSON.stringify(value);
  res.writeHead(status, {
    "content-type": "application/json",
    "content-length": Buffer.byteLength(body),
  });
  res.end(body);
}

function sendHtml(res, value) {
  res.writeHead(200, {
    "content-type": "text/html; charset=utf-8",
    "content-length": Buffer.byteLength(value),
  });
  res.end(value);
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

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
