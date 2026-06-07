#!/usr/bin/env node
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pty from "node-pty";

const here = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(here, "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const port = Number(process.env.PORT || "8899");
const gatewayUrl = process.env.TURA_GATEWAY_URL || "http://127.0.0.1:4096";
const workspace = process.env.TURA_CWD || repoRoot;
const shell = process.platform === "win32" ? "powershell.exe" : "bash";
const nodeBin = process.execPath;
const bunBin = process.env.BUN_BIN || (process.platform === "win32" ? "C:\\Users\\liuliu\\.bun\\bin\\bun.exe" : "bun");
const oldTuiBin = path.join(repoRoot, "apps", "tui", "dist", "index.js");
const opencodeReference = "C:\\Users\\liuliu\\Documents\\opencode-dev\\screenshot.png";
const opencodeRoot = process.env.OPENCODE_DEV_ROOT || "C:\\Users\\liuliu\\Documents\\opencode-dev";
const opencodePackageRoot = path.join(opencodeRoot, "packages", "opencode");
const tuiSource = process.env.TURA_TUI_SOURCE === "tura" ? "tura" : "opencode";
const opencodeSession = process.env.TURA_OPENCODE_SESSION || "";

const style = {
  background: "#101010",
  chrome: "#303446",
  border: "#586071",
  text: "#eeeeee",
  weak: "#808080",
  accent: "#fab283",
  red: "#ff5f57",
  yellow: "#ffbd2e",
  green: "#28c840",
};

const profiles = new Map([
  ["l1", { title: "Tura TUI New L1 Plain", args: ["--plain"], termName: "dumb", forceColor: "0" }],
  ["l2", { title: "Tura TUI New L2 ANSI", args: [], termName: "vt100", forceColor: "1" }],
  ["l3", { title: "Tura TUI New L3 Rich", args: ["--rich"], termName: "xterm-256color", forceColor: "1" }],
]);

function indexHtml() {
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Tura TUI New</title><style>
html,body{height:100%;margin:0;background:#111417;color:${style.text};font-family:system-ui,sans-serif}
main{max-width:720px;padding:32px}a{color:${style.accent};display:block;margin:12px 0;font-size:18px}p,code{color:${style.weak}}
</style></head><body><main><h1>Tura TUI New terminal profiles</h1><p>Source <code>${escapeHtml(tuiSource)}</code></p><p>Session <code>${escapeHtml(opencodeSession || "new")}</code></p><a href="/l1">L1 Plain / Safe</a><a href="/l2">L2 ANSI / Default</a><a href="/l3">L3 Rich / Modern</a><a href="/compare">Compare with opencode screenshot</a></main></body></html>`;
}

function terminalHtml(profileID, profile, instance) {
  const instanceQuery = instance ? `?instance=${encodeURIComponent(instance)}` : "";
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${escapeHtml(profile.title)}</title><link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/xterm@5.3.0/css/xterm.css"><style>
html,body{height:100%;margin:0;background:#000;color:${style.text};font-family:Cascadia Mono,Consolas,ui-monospace,SFMono-Regular,Menlo,monospace}
body{overflow:hidden;display:grid;place-items:center}.shell{width:min(88vw,1600px);height:min(87vh,1260px);border:2px solid ${style.border};border-radius:28px;background:${style.background};box-shadow:0 24px 70px rgba(0,0,0,.58);overflow:hidden;display:grid;grid-template-rows:68px 1fr}
.topbar{display:flex;align-items:center;justify-content:space-between;gap:20px;padding:0 28px;background:${style.chrome};color:#c9cedb;font:700 28px/1 system-ui,sans-serif;border-bottom:2px solid ${style.border};box-sizing:border-box}
.chrome{display:inline-flex;align-items:center;gap:14px;min-width:0}.dot{width:28px;height:28px;border-radius:999px;display:inline-block}.red{background:${style.red}}.yellow{background:${style.yellow}}.green{background:${style.green}}
.title{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}#terminal{width:100%;max-width:100%;padding:0;box-sizing:border-box;overflow:hidden;background:${style.background};min-height:0}.xterm,.xterm-screen,.xterm-viewport{max-width:100%;overflow-x:hidden}.xterm{height:100%}
@media(max-width:900px){.shell{width:100vw;height:100vh;border:0;border-radius:0;grid-template-rows:40px 1fr}.topbar{padding:0 12px;font-size:14px;gap:8px}.chrome{gap:7px}.dot{width:10px;height:10px}}
</style></head><body><section class="shell" aria-label="${escapeHtml(profile.title)}"><div class="topbar"><span class="chrome"><span class="dot red"></span><span class="dot yellow"></span><span class="dot green"></span><span id="window-title" class="title">opencode</span></span></div><div id="terminal"></div></section>
<script src="https://cdn.jsdelivr.net/npm/xterm@5.3.0/lib/xterm.js"></script><script src="https://cdn.jsdelivr.net/npm/xterm-addon-fit@0.8.0/lib/xterm-addon-fit.js"></script><script>
const term=new Terminal({cursorBlink:true,fontFamily:"Cascadia Mono, Consolas, monospace",fontSize:24,theme:{background:"${style.background}",foreground:"${style.text}",cursor:"${style.accent}"},convertEol:true});
const fit=new FitAddon.FitAddon();term.loadAddon(fit);term.open(document.getElementById("terminal"));fit.fit();
term.onTitleChange?.((title)=>{document.getElementById("window-title").textContent=title||"opencode"});
const send=(body)=>fetch("/${profileID}/input${instanceQuery}",{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(body)}).catch(()=>{});
const nextFrame=()=>new Promise((resolve)=>requestAnimationFrame(resolve));window.__turaFit=async()=>{term.options.fontSize=innerWidth<=900?13:24;await nextFrame();fit.fit();await nextFrame();fit.fit();await send({resize:{cols:term.cols,rows:term.rows}});return{cols:term.cols,rows:term.rows}};
term.onData((data)=>send({data}));new ResizeObserver(()=>window.__turaFit()).observe(document.getElementById("terminal"));addEventListener("resize",()=>window.__turaFit());
const events=new EventSource("/${profileID}/events${instanceQuery}");events.onmessage=(event)=>{const data=JSON.parse(event.data);const full=data.includes("[3J");if(full){term.reset();fit.fit()}term.write(data)};events.addEventListener("ready",()=>window.__turaFit());events.onerror=()=>term.write("\\r\\n[tura-new tui disconnected]\\r\\n");
</script></body></html>`;
}

function compareHtml() {
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>TUI comparison</title><style>
html,body{margin:0;background:#0a0a0a;color:${style.text};font-family:system-ui,sans-serif}.grid{height:100vh;display:grid;grid-template-columns:1fr 1fr}.pane{min-width:0;border-left:1px solid ${style.border};display:grid;grid-template-rows:32px 1fr}.bar{background:${style.chrome};color:${style.weak};font:600 12px/32px system-ui;padding:0 10px}iframe,img{width:100%;height:100%;border:0;object-fit:contain;background:${style.background}}@media(max-width:900px){.grid{grid-template-columns:1fr;grid-template-rows:1fr 1fr}}</style></head><body><main class="grid"><section class="pane"><div class="bar">opencode-dev screenshot.png</div><img src="/opencode-reference.png" alt="opencode reference"></section><section class="pane"><div class="bar">tura tui_new l3 (${escapeHtml(tuiSource)})</div><iframe src="/l3?instance=compare"></iframe></section></main></body></html>`;
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

function startTui(profile, runtime, size) {
  if (runtime.term) return runtime.term;
  const command = tuiSource === "opencode" ? bunBin : nodeBin;
  const opencodeArgs = ["run", "--conditions=browser", "./src/index.ts", "--pure"];
  if (opencodeSession) opencodeArgs.push("--session", opencodeSession);
  if (process.env.TURA_OPENCODE_MODEL) opencodeArgs.push("--model", process.env.TURA_OPENCODE_MODEL);
  if (process.env.TURA_OPENCODE_AGENT) opencodeArgs.push("--agent", process.env.TURA_OPENCODE_AGENT);
  opencodeArgs.push(workspace);
  const args = tuiSource === "opencode" ? opencodeArgs : [oldTuiBin, "--gateway-url", gatewayUrl, "--cwd", workspace, ...profile.args];
  const term = pty.spawn(command, args, {
    name: profile.termName,
    cols: Number(size?.cols) || 120,
    rows: Number(size?.rows) || 22,
    cwd: tuiSource === "opencode" ? opencodePackageRoot : repoRoot,
    env: { ...process.env, FORCE_COLOR: profile.forceColor, TERM: profile.termName, TERM_PROGRAM: profile.args.includes("--rich") ? "vscode" : "", TURA_GATEWAY_URL: gatewayUrl, TURA_CWD: workspace },
    shell,
  });
  runtime.term = term;
  term.onData((data) => broadcast(runtime, data));
  term.onExit(({ exitCode }) => {
    broadcast(runtime, `\r\n[tura-new ${tuiSource} tui exited with code ${exitCode}]\r\n`);
    if (runtime.term === term) runtime.term = undefined;
  });
  return term;
}

function broadcast(runtime, data) {
  const payload = `data: ${JSON.stringify(data)}\n\n`;
  for (const res of runtime.clients) res.write(payload);
}

function send(res, value, status = 200, headers = {}) {
  const body = typeof value === "string" ? value : JSON.stringify(value);
  const type = typeof value === "string" ? "text/html; charset=utf-8" : "application/json";
  res.writeHead(status, { "content-length": Buffer.byteLength(body), "content-type": type, ...headers });
  res.end(body);
}

function readJson(req) {
  return new Promise((resolve) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk.toString()));
    req.on("end", () => resolve(body ? JSON.parse(body) : {}));
  });
}

function escapeHtml(value) {
  return String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url || "/", "http://127.0.0.1");
  if (req.method === "GET" && url.pathname === "/") return send(res, indexHtml());
  if (req.method === "GET" && url.pathname === "/compare") return send(res, compareHtml());
  if (req.method === "GET" && url.pathname === "/opencode-reference.png") {
    return fs.createReadStream(opencodeReference).on("error", () => send(res, { error: "reference screenshot not found" }, 404)).pipe(res.writeHead(200, { "content-type": "image/png" }));
  }
  const id = url.pathname.split("/").filter(Boolean)[0] ?? "";
  const profile = profiles.get(id);
  if (!profile) return send(res, { error: "not found" }, 404);
  const runtime = runtimeFor(profile, url.searchParams.get("instance") ?? "");
  const leaf = `/${url.pathname.split("/").filter(Boolean).slice(1).join("/")}`;
  if (req.method === "GET" && leaf === "/") return send(res, terminalHtml(id, profile, url.searchParams.get("instance") ?? ""));
  if (req.method === "GET" && leaf === "/events") {
    res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache", connection: "keep-alive" });
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
  console.log(`Tura TUI New web terminal: http://127.0.0.1:${port}`);
  console.log(`Compare: http://127.0.0.1:${port}/compare`);
  console.log(`Source: ${tuiSource}`);
  console.log(`Workspace: ${workspace}`);
  console.log(`Session: ${opencodeSession || "new"}`);
});
