import http from "node:http";
import os from "node:os";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../../..");
const runId = `oauth-mock-${Date.now()}-${process.pid}`;
const tempRoot = path.join(os.tmpdir(), `tura-${runId}`);
const envPath = path.join(tempRoot, ".env");
const providerConfigPath = path.join(tempRoot, "provider_config.json");

const providers = ["codex", "claude-code", "google", "gemini", "antigravity", "github-copilot"];
const refreshProviders = ["codex", "claude-code", "google", "gemini", "antigravity"];

fs.mkdirSync(tempRoot, { recursive: true });
fs.writeFileSync(envPath, "\n");
fs.writeFileSync(providerConfigPath, JSON.stringify({ provider_auth: {} }, null, 2));

const oauthServer = await startOAuthMockServer();
const port = await freePort();
const gateway = spawn("cargo", ["run", "-p", "gateway", "--bin", "tura_gateway"], {
  cwd: repoRoot,
  stdio: ["ignore", "pipe", "pipe"],
  env: {
    ...process.env,
    PORT: String(port),
    TURA_ENV_PATH: envPath,
    TURA_PROVIDER_CONFIG: providerConfigPath,
    OPENAI_OAUTH_TOKEN_URL: `${oauthServer.origin}/openai/token`,
    ANTHROPIC_OAUTH_TOKEN_URL: `${oauthServer.origin}/anthropic/token`,
    GOOGLE_OAUTH_TOKEN_URL: `${oauthServer.origin}/google/token`,
    GITHUB_DEVICE_CODE_URL: `${oauthServer.origin}/github/device/code`,
    GITHUB_OAUTH_TOKEN_URL: `${oauthServer.origin}/github/oauth/access_token`,
    GOOGLE_OAUTH_CLIENT_ID: "google-client",
    ANTIGRAVITY_OAUTH_CLIENT_ID: "antigravity-client",
    GITHUB_COPILOT_CLIENT_ID: "copilot-client",
    OPENAI_LOGIN: "",
    OPENAI_API_KEY: "",
    OPENAI_REFRESH_TOKEN: "",
    ANTHROPIC_LOGIN: "",
    CLAUDE_CODE_OAUTH_TOKEN: "",
    CLAUDE_CODE_REFRESH_TOKEN: "",
    GOOGLE_LOGIN: "",
    GOOGLE_API_KEY: "",
    GEMINI_LOGIN: "",
    GEMINI_API_KEY: "",
    GOOGLE_REFRESH_TOKEN: "",
    ANTIGRAVITY_LOGIN: "",
    ANTIGRAVITY_API_KEY: "",
    ANTIGRAVITY_REFRESH_TOKEN: "",
    COPILOT_LOGIN: "",
    COPILOT_GITHUB_TOKEN: "",
  },
});

const stdout = [];
const stderr = [];
gateway.stdout.on("data", (chunk) => stdout.push(String(chunk)));
gateway.stderr.on("data", (chunk) => stderr.push(String(chunk)));

try {
  await waitForGateway(port);
  const authSurface = await json("GET", port, "/provider/auth");
  for (const provider of providers) {
    assert(authSurface[provider]?.some((method) => method.type === "oauth"), `${provider} missing oauth method`);
  }

  await runPkceProvider(port, "codex", "openai_access", "openai_refresh");
  await runPkceProvider(port, "claude-code", "anthropic_access", "anthropic_refresh");
  await runPkceProvider(port, "google", "google_access", "google_refresh");
  await refreshProvider(port, "google");
  await runPkceProvider(port, "gemini", "google_access", "google_refresh");
  await runPkceProvider(port, "antigravity", "antigravity_access", "antigravity_refresh");
  await runGithubCopilot(port);

  for (const provider of ["codex", "claude-code", "gemini", "antigravity"]) {
    await refreshProvider(port, provider);
  }

  for (const provider of providers) {
    const status = await json("GET", port, `/provider/${provider}/auth/status`);
    assert(status.authenticated === true, `${provider} final status not authenticated: ${JSON.stringify(status)}`);
  }

  console.log(JSON.stringify({
    ok: true,
    gateway: `http://127.0.0.1:${port}`,
    oauth_mock: oauthServer.origin,
    provider_config: providerConfigPath,
    covered: {
      authorize_callback: providers,
      refresh: refreshProviders,
      status: providers,
    },
    requests: oauthServer.requests,
  }, null, 2));
} finally {
  gateway.kill();
  oauthServer.close();
}

async function runPkceProvider(port, provider, accessPrefix, refreshPrefix) {
  const authorize = await json("POST", port, `/provider/${provider}/oauth/authorize`, { method: 0 });
  assert(authorize.url, `${provider} did not return authorize url`);
  const state = new URL(authorize.url).searchParams.get("state");
  assert(state, `${provider} authorize url missing state`);
  const callbackWait = json("POST", port, `/provider/${provider}/oauth/callback`, { method: 0 });
  await sleep(100);
  const html = await text("GET", port, `/auth/callback?code=${encodeURIComponent(`${provider}-auth-code`)}&state=${encodeURIComponent(state)}`);
  assert(html.includes("OAuth connected"), `${provider} URL callback did not connect: ${html}`);
  const callback = await callbackWait;
  assert(callback === true, `${provider} callback failed`);
  const status = await json("GET", port, `/provider/${provider}/auth/status`);
  assert(status.authenticated === true, `${provider} status after callback failed`);
  assert(oauthServer.requests.some((item) => item.provider === provider || item.access?.startsWith(accessPrefix)), `${provider} token request missing`);
  assert(accessPrefix && refreshPrefix);
}

async function runGithubCopilot(port) {
  const authorize = await json("POST", port, "/provider/github-copilot/oauth/authorize", { method: 0 });
  assert(authorize.method === "code", "github-copilot should use code method");
  assert(authorize.url === `${oauthServer.origin}/github/login/device`, "github-copilot verification url mismatch");
  assert(authorize.instructions.includes("MOCK-CODE"), "github-copilot instructions missing user code");
  const callback = await json("POST", port, "/provider/github-copilot/oauth/callback", { method: 0 });
  assert(callback === true, "github-copilot callback failed");
}

async function refreshProvider(port, provider) {
  const result = await json("POST", port, `/provider/${provider}/auth/refresh`, {});
  assert(result.ok === true, `${provider} refresh failed: ${JSON.stringify(result)}`);
  assert(result.status?.authenticated === true, `${provider} refresh did not authenticate`);
}

function startOAuthMockServer() {
  const requests = [];
  const server = http.createServer(async (req, res) => {
    const body = await readBody(req);
    const params = new URLSearchParams(body);
    let payload;
    if (req.url === "/github/device/code") {
      assert(params.get("client_id") === "copilot-client", "github device client id mismatch");
      payload = {
        device_code: "mock-device-code",
        user_code: "MOCK-CODE",
        verification_uri: `${originOf(req)}/github/login/device`,
        expires_in: 900,
        interval: 1,
      };
      requests.push({ provider: "github-copilot", phase: "device", body });
    } else if (req.url === "/github/oauth/access_token") {
      assert(params.get("device_code") === "mock-device-code", "github device code mismatch");
      payload = { access_token: "gho_mock_copilot_access", token_type: "bearer", scope: "read:user" };
      requests.push({ provider: "github-copilot", phase: "token", body });
    } else if (req.url?.endsWith("/token")) {
      const provider = req.url.split("/")[1];
      const grant = params.get("grant_type");
      const refreshToken = params.get("refresh_token");
      const access = grant === "refresh_token"
        ? `${provider}_refreshed_access`
        : `${provider}_access`;
      const refresh = refreshToken || `${provider}_refresh`;
      payload = { access_token: access, refresh_token: refresh, expires_in: 3600 };
      requests.push({ provider: providerName(provider, body), phase: grant, body, access, refresh });
    } else {
      res.writeHead(404, { "content-type": "application/json" });
      res.end("{}");
      return;
    }
    const text = JSON.stringify(payload);
    res.writeHead(200, { "content-type": "application/json", "content-length": Buffer.byteLength(text) });
    res.end(text);
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const origin = `http://127.0.0.1:${address.port}`;
      server.origin = origin;
      server.requests = requests;
      server.close = server.close.bind(server);
      resolve(server);
    });
  });
}

function providerName(provider, body) {
  if (provider === "openai") return "codex";
  if (provider === "anthropic") return "claude-code";
  if (provider === "google" && body.includes("antigravity")) return "antigravity";
  if (provider === "google" && body.includes("gemini")) return "gemini";
  if (provider === "google") return "google";
  return provider;
}

function originOf(req) {
  return `http://${req.headers.host}`;
}

function readBody(req) {
  return new Promise((resolve) => {
    let body = "";
    req.setEncoding("utf8");
    req.on("data", (chunk) => body += chunk);
    req.on("end", () => resolve(body));
  });
}

async function json(method, port, pathname, body) {
  const init = { method, headers: { "content-type": "application/json" } };
  if (method !== "GET") init.body = JSON.stringify(body ?? {});
  const response = await fetch(`http://127.0.0.1:${port}${pathname}`, init);
  const text = await response.text();
  assert(response.ok, `${method} ${pathname} failed ${response.status}: ${text}`);
  return text ? JSON.parse(text) : null;
}

async function text(method, port, pathname) {
  const response = await fetch(`http://127.0.0.1:${port}${pathname}`, { method });
  const value = await response.text();
  assert(response.ok, `${method} ${pathname} failed ${response.status}: ${value}`);
  return value;
}

async function waitForGateway(port) {
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/global/health`);
      if (response.ok) return;
    } catch {}
    await sleep(500);
  }
  throw new Error(`gateway did not start\nstdout:\n${stdout.join("")}\nstderr:\n${stderr.join("")}`);
}

function freePort() {
  return new Promise((resolve) => {
    const server = http.createServer();
    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      server.close(() => resolve(port));
    });
  });
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
