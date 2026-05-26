#!/usr/bin/env node
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pty from "node-pty";

const here = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(here, "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const port = Number(process.env.PORT || "8799");
const gatewayUrl = process.env.TURA_GATEWAY_URL || "http://127.0.0.1:4096";
const workspace = process.env.TURA_CWD || repoRoot;
const shell = process.platform === "win32" ? "powershell.exe" : "bash";
const nodeBin = process.execPath;
const tuiBin = path.join(appRoot, "dist", "index.js");

let term;
const clients = new Set();

function html() {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Tura TUI</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/xterm@5.3.0/css/xterm.css">
  <style>
    html, body { height: 100%; margin: 0; background: #111417; }
    body { overflow: hidden; }
    #terminal { height: 100vh; width: 100vw; padding: 8px; box-sizing: border-box; }
    .xterm { height: 100%; }
  </style>
</head>
<body>
  <div id="terminal"></div>
  <script src="https://cdn.jsdelivr.net/npm/xterm@5.3.0/lib/xterm.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/xterm-addon-fit@0.8.0/lib/xterm-addon-fit.js"></script>
  <script>
    const term = new Terminal({
      cursorBlink: true,
      fontFamily: "Cascadia Mono, Consolas, monospace",
      fontSize: 15,
      theme: { background: "#111417", foreground: "#edf0f2" },
      convertEol: true
    });
    const fit = new FitAddon.FitAddon();
    term.loadAddon(fit);
    term.open(document.getElementById("terminal"));
    fit.fit();
    const send = (body) => fetch("/input", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body)
    }).catch(() => {});
    term.onData((data) => send({ data }));
    addEventListener("resize", () => {
      fit.fit();
      send({ resize: { cols: term.cols, rows: term.rows } });
    });
    const events = new EventSource("/events");
    events.onmessage = (event) => term.write(JSON.parse(event.data));
    events.addEventListener("ready", () => send({ resize: { cols: term.cols, rows: term.rows } }));
    events.onerror = () => term.write("\\r\\n[web terminal disconnected]\\r\\n");
  </script>
</body>
</html>`;
}

function send(res, value, status = 200, headers = {}) {
  const body = typeof value === "string" ? value : JSON.stringify(value);
  res.writeHead(status, {
    "content-length": Buffer.byteLength(body),
    "content-type": typeof value === "string" ? "text/html; charset=utf-8" : "application/json",
    ...headers,
  });
  res.end(body);
}

function readJson(req) {
  return new Promise((resolve) => {
    let body = "";
    req.on("data", (chunk) => {
      body += chunk.toString();
    });
    req.on("end", () => resolve(body ? JSON.parse(body) : {}));
  });
}

function broadcast(data) {
  const payload = `data: ${JSON.stringify(data)}\n\n`;
  for (const res of clients) res.write(payload);
}

function startTui() {
  if (term) return term;
  term = pty.spawn(nodeBin, [tuiBin, "--gateway-url", gatewayUrl, "--cwd", workspace], {
    name: "xterm-256color",
    cols: 120,
    rows: 36,
    cwd: repoRoot,
    env: {
      ...process.env,
      FORCE_COLOR: "1",
      TURA_GATEWAY_URL: gatewayUrl,
      TURA_CWD: workspace,
    },
    shell,
  });
  term.onData((data) => broadcast(data));
  term.onExit(({ exitCode }) => {
    broadcast(`\r\n[tura tui exited with code ${exitCode}]\r\n`);
    term = undefined;
  });
  return term;
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url || "/", "http://127.0.0.1");
  if (req.method === "GET" && url.pathname === "/") return send(res, html());
  if (req.method === "GET" && url.pathname === "/events") {
    res.writeHead(200, {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
      connection: "keep-alive",
    });
    clients.add(res);
    res.write("event: ready\ndata: ready\n\n");
    startTui();
    req.on("close", () => clients.delete(res));
    return;
  }
  if (req.method === "POST" && url.pathname === "/input") {
    const body = await readJson(req);
    const active = startTui();
    if (typeof body.data === "string") active.write(body.data);
    if (body.resize) active.resize(Number(body.resize.cols) || 120, Number(body.resize.rows) || 36);
    return send(res, { ok: true });
  }
  return send(res, { error: "not found" }, 404);
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Tura TUI web terminal: http://127.0.0.1:${port}`);
  console.log(`Gateway: ${gatewayUrl}`);
  console.log(`Workspace: ${workspace}`);
});
