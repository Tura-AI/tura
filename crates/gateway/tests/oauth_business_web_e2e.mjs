import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../../..");
const page = path.join(__dirname, "oauth_business_page.html");
const port = Number(process.env.TURA_OAUTH_BUSINESS_PORT || await freePort());
const pageServer = await startPageServer();

const gateway = spawn("cargo", ["run", "-p", "gateway", "--bin", "gateway"], {
  cwd: repoRoot,
  stdio: ["ignore", "pipe", "pipe"],
  env: {
    ...process.env,
    PORT: String(port),
    OPENAI_LOGIN: process.env.OPENAI_LOGIN || "oauth",
  },
});

gateway.stdout.on("data", (chunk) => process.stdout.write(String(chunk)));
gateway.stderr.on("data", (chunk) => process.stderr.write(String(chunk)));

await waitForGateway(port);
const url =
  `${pageServer.origin}/oauth_business_page.html` +
  `?gatewayUrl=${encodeURIComponent(`http://127.0.0.1:${port}`)}` +
  `&openerUrl=${encodeURIComponent(`${pageServer.origin}/open`)}`;
console.log(`\nOAuth business test page:\n${url}\n`);
openBrowser(url);
console.log("Leave this process running while you test. Press Ctrl+C when done.");

process.on("SIGINT", () => {
  gateway.kill();
  pageServer.close();
  process.exit(0);
});

function startPageServer() {
  const server = http.createServer(async (req, res) => {
    if (req.method === "OPTIONS") {
      writeCors(res, 204);
      res.end();
      return;
    }
    if (req.method === "GET" && (req.url === "/" || req.url?.startsWith("/oauth_business_page.html"))) {
      const html = fs.readFileSync(page);
      writeCors(res, 200, {
        "content-type": "text/html; charset=utf-8",
        "content-length": html.length,
      });
      res.end(html);
      return;
    }
    if (req.method === "POST" && req.url === "/open") {
      const body = await readBody(req);
      const payload = JSON.parse(body || "{}");
      if (typeof payload.url === "string" && /^https?:\/\//.test(payload.url)) {
        console.log(`Opening OAuth URL in system browser: ${payload.url}`);
        openBrowser(payload.url);
        writeCors(res, 204);
        res.end();
        return;
      }
      writeCors(res, 400, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "invalid url" }));
      return;
    }
    writeCors(res, 404);
    res.end();
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.origin = `http://127.0.0.1:${address.port}`;
      server.close = server.close.bind(server);
      resolve(server);
    });
  });
}

function writeCors(res, status, headers = {}) {
  res.writeHead(status, {
    "access-control-allow-origin": "*",
    "access-control-allow-methods": "GET,POST,OPTIONS",
    "access-control-allow-headers": "content-type",
    ...headers,
  });
}

function openBrowser(url) {
  if (process.env.TURA_OAUTH_BUSINESS_NO_OPEN === "1") return;
  if (process.platform === "win32") {
    spawn("powershell", [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-Command",
      "Start-Process -FilePath $args[0]",
      url,
    ], {
      stdio: "ignore",
      detached: true,
      windowsHide: true,
    });
  } else if (process.platform === "darwin") {
    spawn("open", [url], { stdio: "ignore", detached: true });
  } else {
    spawn("xdg-open", [url], { stdio: "ignore", detached: true });
  }
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
  gateway.kill();
  pageServer.close();
  throw new Error("gateway did not start");
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

function readBody(req) {
  return new Promise((resolve) => {
    let body = "";
    req.setEncoding("utf8");
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => resolve(body));
  });
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
