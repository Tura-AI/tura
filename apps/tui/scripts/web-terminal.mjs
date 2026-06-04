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

const profiles = new Map([
  ["plain", { title: "Tura TUI Plain / Safe", path: "/plain", args: ["--plain"], termName: "dumb", forceColor: "0", clients: new Set(), term: undefined }],
  ["ansi", { title: "Tura TUI ANSI / Default", path: "/ansi", args: [], termName: "vt100", forceColor: "1", clients: new Set(), term: undefined }],
  ["rich", { title: "Tura TUI Rich / Modern", path: "/rich", args: ["--rich"], termName: "xterm-256color", forceColor: "1", clients: new Set(), term: undefined }],
]);

function indexHtml() {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Tura TUI terminal profiles</title>
  <style>
    html, body { height: 100%; margin: 0; background: #111417; color: #edf0f2; font-family: system-ui, sans-serif; }
    main { max-width: 720px; padding: 32px; }
    a { color: #7dd3fc; display: block; margin: 12px 0; font-size: 18px; }
    code { color: #facc15; }
  </style>
</head>
<body>
  <main>
    <h1>Tura TUI terminal profiles</h1>
    <p>Gateway: <code>${escapeHtml(gatewayUrl)}</code></p>
    <p>Workspace: <code>${escapeHtml(workspace)}</code></p>
    <a href="/plain">L1 Plain / Safe</a>
    <a href="/ansi">L2 ANSI / Default</a>
    <a href="/rich">L3 Rich / Modern</a>
  </main>
</body>
</html>`;
}

function html(profileId, profile) {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(profile.title)}</title>
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
    const profile = ${JSON.stringify(profileId)};
    const send = (body) => fetch("/" + profile + "/input", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body)
    }).catch(() => {});
    term.onData((data) => send({ data }));
    addEventListener("resize", () => {
      fit.fit();
      send({ resize: { cols: term.cols, rows: term.rows } });
    });
    const events = new EventSource("/" + profile + "/events");
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

function broadcast(profile, data) {
  const payload = `data: ${JSON.stringify(data)}\n\n`;
  for (const res of profile.clients) res.write(payload);
}

function startTui(profile) {
  if (profile.term) return profile.term;
  profile.term = pty.spawn(nodeBin, [tuiBin, "--gateway-url", gatewayUrl, "--cwd", workspace, ...profile.args], {
    name: profile.termName,
    cols: 120,
    rows: 36,
    cwd: repoRoot,
    env: {
      ...process.env,
      FORCE_COLOR: profile.forceColor,
      TERM: profile.termName,
      TERM_PROGRAM: profile.args.includes("--rich") ? "vscode" : "",
      TURA_GATEWAY_URL: gatewayUrl,
      TURA_CWD: workspace,
    },
    shell,
  });
  profile.term.onData((data) => broadcast(profile, data));
  profile.term.onExit(({ exitCode }) => {
    broadcast(profile, `\r\n[tura tui exited with code ${exitCode}]\r\n`);
    profile.term = undefined;
  });
  return profile.term;
}

function profileFromPath(url) {
  const id = url.pathname.split("/").filter(Boolean)[0] ?? "";
  return profiles.get(id);
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url || "/", "http://127.0.0.1");
  if (req.method === "GET" && url.pathname === "/") return send(res, indexHtml());
  const profile = profileFromPath(url);
  if (!profile) return send(res, { error: "not found" }, 404);
  const leaf = `/${url.pathname.split("/").filter(Boolean).slice(1).join("/")}`;
  if (req.method === "GET" && leaf === "/") return send(res, html(url.pathname.split("/").filter(Boolean)[0], profile));
  if (req.method === "GET" && leaf === "/events") {
    res.writeHead(200, {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
      connection: "keep-alive",
    });
    profile.clients.add(res);
    res.write("event: ready\ndata: ready\n\n");
    startTui(profile);
    req.on("close", () => profile.clients.delete(res));
    return;
  }
  if (req.method === "POST" && leaf === "/input") {
    const body = await readJson(req);
    const active = startTui(profile);
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
