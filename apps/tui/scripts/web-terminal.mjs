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
  [
    "plain",
    {
      title: "Tura TUI Plain / Safe",
      path: "/plain",
      args: ["--plain"],
      termName: "dumb",
      forceColor: "0",
      clients: new Set(),
      term: undefined,
    },
  ],
  [
    "ansi",
    {
      title: "Tura TUI ANSI / Default",
      path: "/ansi",
      args: [],
      termName: "vt100",
      forceColor: "1",
      clients: new Set(),
      term: undefined,
    },
  ],
  [
    "rich",
    {
      title: "Tura TUI Rich / Modern",
      path: "/rich",
      args: ["--rich"],
      termName: "xterm-256color",
      forceColor: "1",
      clients: new Set(),
      term: undefined,
    },
  ],
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
    a { color: #fab283; display: block; margin: 12px 0; font-size: 18px; }
    p { color: #808080; }
    code { color: #808080; }
  </style>
</head>
<body>
  <main>
    <h1>Tura TUI terminal profiles</h1>
    <p>Gateway <code>${escapeHtml(gatewayUrl)}</code></p>
    <a href="/plain">L1 Plain / Safe</a>
    <a href="/ansi">L2 ANSI / Default</a>
    <a href="/rich">L3 Rich / Modern</a>
  </main>
</body>
</html>`;
}

function html(profileId, profile, instance) {
  const instanceQuery = instance ? `?instance=${encodeURIComponent(instance)}` : "";
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(profile.title)}</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/xterm@5.3.0/css/xterm.css">
  <style>
    html, body { height: 100%; margin: 0; background: #0a0a0a; color: #eeeeee; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
    body { overflow: hidden; display: grid; place-items: center; }
    .shell {
      width: min(92vw, 1100px);
      height: min(88vh, 820px);
      border: 2px solid #3a3a3a;
      border-radius: 6px;
      background: #101010;
      box-shadow: 0 24px 70px rgba(0,0,0,.5);
      overflow: hidden;
      display: grid;
      grid-template-rows: 36px 1fr;
    }
    .topbar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 0 12px;
      background: #161618;
      color: #b4b4b4;
      font: 600 13px/1 system-ui, sans-serif;
      border-bottom: 2px solid #3a3a3a;
      box-sizing: border-box;
    }
    .chrome { display: inline-flex; align-items: center; gap: 7px; min-width: 0; }
    .dot { width: 10px; height: 10px; border-radius: 999px; display: inline-block; }
    .red { background: #5c5c5c; }
    .yellow-dot { background: #fab283; }
    .green-dot { background: #5c5c5c; }
    .title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .badge { color: #808080; text-transform: uppercase; font-size: 11px; letter-spacing: .08em; white-space: nowrap; }
    #terminal { width: 100%; max-width: 100%; padding: 12px 14px 10px; box-sizing: border-box; overflow: hidden; background: #101010; min-height: 0; }
    .xterm, .xterm-screen, .xterm-viewport { max-width: 100%; overflow-x: hidden; }
    .xterm-rows, .xterm-rows > div { overflow: visible !important; }
    .xterm { height: 100%; }
    @media (max-width: 640px) {
      .shell { width: 100vw; height: 100vh; border: 0; border-radius: 0; }
      .badge { display: none; }
      #terminal { padding: 8px; }
    }
  </style>
</head>
<body>
  <section class="shell" aria-label="${escapeHtml(profile.title)}">
    <div class="topbar">
      <span class="chrome">
        <span class="dot red"></span>
        <span class="dot yellow-dot"></span>
        <span class="dot green-dot"></span>
        <span class="title">OC | ${escapeHtml(profile.title)}</span>
      </span>
      <span class="badge">terminal ui</span>
    </div>
    <div id="terminal"></div>
  </section>
  <script src="https://cdn.jsdelivr.net/npm/xterm@5.3.0/lib/xterm.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/xterm-addon-fit@0.8.0/lib/xterm-addon-fit.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/xterm-addon-unicode11@0.6.0/lib/xterm-addon-unicode11.min.js"></script>
  <script>
    const term = new Terminal({
      allowProposedApi: true,
      cursorBlink: true,
      fontFamily: "Cascadia Mono, Segoe UI Emoji, Apple Color Emoji, Noto Color Emoji, Consolas, monospace",
      fontSize: 15,
      lineHeight: 1.22,
      theme: { background: "#101010", foreground: "#eeeeee", cursor: "#fab283" },
      convertEol: true
    });
    window.__turaTerminal = term;
    const fit = new FitAddon.FitAddon();
    window.__turaUnicode11Loaded = false;
    try {
      const Unicode11Ctor =
        globalThis.Unicode11Addon?.Unicode11Addon ||
        globalThis.Unicode11Addon ||
        globalThis.XTermAddonUnicode11?.Unicode11Addon;
      if (Unicode11Ctor) {
        term.loadAddon(new Unicode11Ctor());
        if (term.unicode) term.unicode.activeVersion = "11";
        window.__turaUnicode11Loaded = term.unicode?.activeVersion === "11";
      }
    } catch (error) {
      console.warn("Unicode11 addon unavailable", error);
    }
    term.loadAddon(fit);
    term.open(document.getElementById("terminal"));
    fit.fit();
    const profile = ${JSON.stringify(profileId)};
    const instanceQuery = ${JSON.stringify(instanceQuery)};
    const send = (body) => fetch("/" + profile + "/input" + instanceQuery, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body)
    }).catch(() => {});
    const shortDelay = () => new Promise((resolve) => setTimeout(resolve, 30));
    window.__turaSendInput = async (data) => {
      for (const char of Array.from(data)) {
        await send({ data: char });
        await shortDelay();
      }
      await shortDelay();
      await shortDelay();
      await nextFrame();
      await nextFrame();
    };
    const nextFrame = () => new Promise((resolve) => requestAnimationFrame(resolve));
    window.__turaFit = async () => {
      term.options.fontSize = innerWidth <= 640 ? 13 : 15;
      await nextFrame();
      fit.fit();
      await nextFrame();
      fit.fit();
      await send({ resize: { cols: term.cols, rows: term.rows } });
      term.scrollToTop?.();
      return { cols: term.cols, rows: term.rows };
    };
    term.onData((data) => send({ data }));
    const resizeObserver = new ResizeObserver(() => {
      window.__turaFit();
    });
    resizeObserver.observe(document.getElementById("terminal"));
    addEventListener("resize", () => {
      window.__turaFit();
    });
    const events = new EventSource("/" + profile + "/events" + instanceQuery);
    events.onmessage = (event) => {
      const data = JSON.parse(event.data);
      const fullFrame = data.includes("[3J");
      if (fullFrame) {
        term.reset();
        fit.fit();
      }
      term.write(data, () => {
        if (fullFrame) term.scrollToTop?.();
      });
    };
    events.addEventListener("ready", () => window.__turaFit());
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

function runtimeFor(profile, key) {
  profile.runtimes ??= new Map();
  const runtimeKey = key || "default";
  let runtime = profile.runtimes.get(runtimeKey);
  if (!runtime) {
    runtime = { clients: new Set(), term: undefined };
    profile.runtimes.set(runtimeKey, runtime);
  }
  return runtime;
}

function broadcast(runtime, data) {
  const payload = `data: ${JSON.stringify(data)}\n\n`;
  for (const res of runtime.clients) res.write(payload);
}

function startTui(profile, runtime, size = undefined) {
  if (runtime.term) return runtime.term;
  const cols = Number(size?.cols) || 120;
  const rows = Number(size?.rows) || 22;
  const term = pty.spawn(
    nodeBin,
    [tuiBin, "--gateway-url", gatewayUrl, "--cwd", workspace, ...profile.args],
    {
      name: profile.termName,
      cols,
      rows,
      cwd: repoRoot,
      env: {
        ...process.env,
        FORCE_COLOR: profile.forceColor,
        TERM: profile.termName,
        TERM_PROGRAM: profile.args.includes("--rich") ? "vscode" : "",
        TURA_TUI_DISABLE_MOUSE: "1",
        TURA_GATEWAY_URL: gatewayUrl,
        TURA_CWD: workspace,
      },
      shell,
    },
  );
  runtime.term = term;
  term.onData((data) => broadcast(runtime, data));
  term.onExit(({ exitCode }) => {
    broadcast(runtime, `\r\n[tura tui exited with code ${exitCode}]\r\n`);
    if (runtime.term === term) runtime.term = undefined;
  });
  return term;
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
  const instance = url.searchParams.get("instance") ?? "";
  const runtime = runtimeFor(profile, instance);
  const leaf = `/${url.pathname.split("/").filter(Boolean).slice(1).join("/")}`;
  if (req.method === "GET" && leaf === "/") {
    return send(res, html(url.pathname.split("/").filter(Boolean)[0], profile, instance));
  }
  if (req.method === "GET" && leaf === "/events") {
    res.writeHead(200, {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
      connection: "keep-alive",
    });
    runtime.clients.add(res);
    res.write("event: ready\ndata: ready\n\n");
    req.on("close", () => runtime.clients.delete(res));
    return;
  }
  if (req.method === "POST" && leaf === "/input") {
    const body = await readJson(req);
    const active = startTui(profile, runtime, body.resize);
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
