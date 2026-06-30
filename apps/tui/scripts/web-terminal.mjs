#!/usr/bin/env node
import fs from "node:fs";
import fsp from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pty from "node-pty";
const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const port = Number(process.env.PORT || "8799");
const gatewayUrl = process.env.TURA_GATEWAY_URL || readActiveGatewayUrl() || defaultGatewayUrl();
const workspace = process.env.TURA_CWD || repoRoot;
const mockMode = process.env.TURA_TUI_MOCK === "1";
const shell = process.env.TURA_WEB_TERMINAL_SHELL || defaultShell();
const nodeBin = process.execPath;
const tuiBin = path.join(appRoot, "dist", "index.js");
const tuiCommand = process.env.TURA_TUI_BIN || nodeBin;
const tuiBaseArgs = process.env.TURA_TUI_BIN ? [] : [tuiBin];

function defaultGatewayUrl() {
  return `http://127.0.0.1:${defaultGatewayPort()}`;
}

function defaultGatewayPort() {
  if (process.env.TURA_BUILD_KIND === "release") return "4126";
  return process.execPath.replace(/\\/g, "/").toLowerCase().includes("/target/release/")
    ? "4126"
    : "4125";
}

function readActiveGatewayUrl() {
  try {
    const raw = fs.readFileSync(path.join(instanceHome(), ".tura", "gateway-active.env"), "utf8");
    for (const line of raw.split(/\r?\n/u)) {
      const trimmed = line.trim();
      if (!trimmed.startsWith("TURA_GATEWAY_URL=")) continue;
      const value = trimmed
        .slice("TURA_GATEWAY_URL=".length)
        .trim()
        .replace(/^["']|["']$/gu, "");
      if (value) return value.replace(/\/+$/u, "");
    }
  } catch {
    return undefined;
  }
  return undefined;
}

function instanceHome() {
  const fromEnv = process.env.TURA_HOME?.trim();
  return path.resolve(fromEnv || repoRoot);
}

function defaultShell() {
  if (process.platform === "win32") return "powershell.exe";
  const userShell = process.env.SHELL?.trim();
  if (userShell && shellPathUsable(userShell)) return userShell;
  if (process.platform === "darwin" && fs.existsSync("/bin/zsh")) return "/bin/zsh";
  for (const candidate of ["/bin/bash", "/usr/bin/bash", "/bin/sh", "/usr/bin/sh"]) {
    if (fs.existsSync(candidate)) return candidate;
  }
  return "bash";
}
function shellPathUsable(value) {
  return !path.isAbsolute(value) || fs.existsSync(value);
}
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
    a { color: #40e0d0; display: block; margin: 12px 0; font-size: 18px; }
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
    .shell.dragging {
      border-color: #40e0d0;
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
    .yellow-dot { background: #40e0d0; }
    .green-dot { background: #5c5c5c; }
    .title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .badge { color: #808080; text-transform: uppercase; font-size: 11px; letter-spacing: .08em; white-space: nowrap; }
    #terminal { width: 100%; max-width: 100%; padding: 12px 14px 10px; box-sizing: border-box; overflow: hidden; background: #101010; min-height: 0; }
    .xterm { height: 100%; background: #101010; }
    .xterm-screen, .xterm-viewport, .xterm-helpers, .xterm-helper-textarea { background: #101010 !important; }
    .xterm, .xterm-screen, .xterm-viewport { max-width: 100%; overflow-x: hidden; }
    .xterm-rows, .xterm-rows > div { overflow: visible !important; }
    .xterm-rows > div { line-height: 1.22 !important; }
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
    const terminalHost = document.getElementById("terminal");
    const shellHost = document.querySelector(".shell");
    let term;
    let fit;
    window.__turaUnicode11Loaded = false;
    window.__turaHoveredLink = "";
    window.__turaActivatedLinks = [];
    const linkHandler = {
      allowNonHttpProtocols: true,
      activate: (event, text) => {
        event?.preventDefault?.();
        window.__turaActivatedLinks.push(text);
        try {
          window.open(text, "_blank", "noopener,noreferrer");
        } catch (error) {
          console.warn("link open failed", error);
        }
      },
      hover: (_event, text) => {
        window.__turaHoveredLink = text;
      },
      leave: (_event, text) => {
        if (window.__turaHoveredLink === text) window.__turaHoveredLink = "";
      },
    };
    const createTerminal = () => {
      const nextTerm = new Terminal({
        allowProposedApi: true,
        cursorBlink: true,
        fontFamily: "Cascadia Mono, Segoe UI Emoji, Apple Color Emoji, Noto Color Emoji, Consolas, monospace",
        fontSize: innerWidth <= 640 ? 13 : 15,
        linkHandler,
        lineHeight: 1.22,
        scrollback: 20000,
        theme: { background: "#101010", foreground: "#eeeeee", cursor: "#40e0d0" },
        convertEol: true
      });
      const nextFit = new FitAddon.FitAddon();
      try {
        const Unicode11Ctor =
          globalThis.Unicode11Addon?.Unicode11Addon ||
          globalThis.Unicode11Addon ||
          globalThis.XTermAddonUnicode11?.Unicode11Addon;
        if (Unicode11Ctor) {
          nextTerm.loadAddon(new Unicode11Ctor());
          if (nextTerm.unicode) nextTerm.unicode.activeVersion = "11";
          window.__turaUnicode11Loaded = nextTerm.unicode?.activeVersion === "11";
        }
      } catch (error) {
        console.warn("Unicode11 addon unavailable", error);
      }
      nextTerm.loadAddon(nextFit);
      nextTerm.open(terminalHost);
      nextFit.fit();
      nextTerm.onData((data) => send({ data }));
      term = nextTerm;
      fit = nextFit;
      window.__turaTerminal = term;
    };
    createTerminal();
    const profile = ${JSON.stringify(profileId)};
    const instanceQuery = ${JSON.stringify(instanceQuery)};
    const send = (body) => fetch("/" + profile + "/input" + instanceQuery, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body)
    }).catch(() => {});
    const sendDroppedText = async (text) => {
      if (!text) return;
      await send({ data: text });
      await shortDelay();
      await nextFrame();
    };
    const fileUrl = (filePath) => {
      const normalized = String(filePath).replaceAll(String.fromCharCode(92), "/");
      const withSlash = isWindowsFilePath(normalized) ? "/" + normalized : normalized;
      return "file://" + encodeURI(withSlash);
    };
    const isWorkspaceInputPath = (filePath) => String(filePath).replaceAll(String.fromCharCode(92), "/").startsWith(".tura/media/input/");
    const isWindowsFilePath = (value) =>
      value.length >= 3 && /^[A-Za-z]$/u.test(value[0]) && value[1] === ":" && value[2] === "/";
    const isMediaPath = (filePath) => {
      const lower = String(filePath).toLowerCase();
      return [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp", ".mp4", ".mov", ".webm", ".mp3", ".wav", ".ogg"].some((suffix) =>
        lower.endsWith(suffix),
      );
    };
    const richTokenForPath = (filePath) => {
      if (isMediaPath(filePath)) return "[MEDIA:" + filePath + ":MEDIA]";
      let clean = String(filePath);
      while (clean.endsWith("/") || clean.endsWith(String.fromCharCode(92))) clean = clean.slice(0, -1);
      const slashIndex = Math.max(clean.lastIndexOf("/"), clean.lastIndexOf(String.fromCharCode(92)));
      const label = slashIndex >= 0 ? clean.slice(slashIndex + 1) || filePath : clean || filePath;
      return "[" + label + "](" + (isWorkspaceInputPath(filePath) ? filePath : fileUrl(filePath)) + ")";
    };
    const normalizeDroppedUri = (value) => {
      const trimmed = String(value || "").trim().replace(/^['\"]|['\"]$/gu, "");
      if (!trimmed) return undefined;
      if (/^file:/iu.test(trimmed)) {
        try {
          const url = new URL(trimmed);
          const pathname = decodeURIComponent(url.pathname || "");
          if (url.hostname && url.hostname !== "localhost") return undefined;
          return pathname.length >= 4 && pathname[0] === "/" && isWindowsFilePath(pathname.slice(1))
            ? pathname.slice(1)
            : pathname;
        } catch {
          return undefined;
        }
      }
      const backslash = String.fromCharCode(92);
      const isAbsolute =
        isWindowsFilePath(trimmed.replaceAll(backslash, "/")) ||
        trimmed.startsWith(backslash + backslash) ||
        trimmed.startsWith("/");
      if (isAbsolute)
        return trimmed;
      return undefined;
    };
    const droppedTextPaths = (dataTransfer) => {
      const paths = [];
      const appendText = (text) => {
        const newline = String.fromCharCode(10);
        const carriage = String.fromCharCode(13);
        const lines = String(text || "")
          .replaceAll(carriage + newline, newline)
          .replaceAll(carriage, newline)
          .split(newline);
        for (const line of lines) {
          if (!line || line.startsWith("#")) continue;
          const normalized = normalizeDroppedUri(line);
          if (normalized) paths.push(normalized);
        }
      };
      for (const type of ["text/uri-list", "text/plain"]) {
        if ([...dataTransfer.types].includes(type)) appendText(dataTransfer.getData(type));
      }
      for (const file of dataTransfer.files || []) {
        const nativePath = file.path || file.webkitRelativePath;
        const normalized = normalizeDroppedUri(nativePath);
        if (normalized) paths.push(normalized);
      }
      return [...new Set(paths)];
    };
    const base64FromFile = async (file) => {
      const bytes = new Uint8Array(await file.arrayBuffer());
      let binary = "";
      const chunkSize = 0x8000;
      for (let index = 0; index < bytes.length; index += chunkSize) {
        binary += String.fromCharCode(...bytes.slice(index, index + chunkSize));
      }
      return btoa(binary);
    };
    const uploadDroppedFile = async (file) => {
      const response = await fetch("/" + profile + "/drop-file" + instanceQuery, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name: file.name, type: file.type, data: await base64FromFile(file) })
      });
      if (!response.ok) throw new Error("drop upload failed: " + response.status);
      return (await response.json()).path;
    };
    const uploadDroppedPath = async (filePath) => {
      const response = await fetch("/" + profile + "/drop-path" + instanceQuery, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ path: filePath })
      });
      if (!response.ok) throw new Error("drop path copy failed: " + response.status);
      return (await response.json()).path;
    };
    const handleDroppedData = async (dataTransfer) => {
      const paths = [];
      for (const filePath of droppedTextPaths(dataTransfer)) paths.push(await uploadDroppedPath(filePath));
      const filesToUpload = [...(dataTransfer.files || [])].filter((file) => {
        const nativePath = file.path || file.webkitRelativePath;
        return !normalizeDroppedUri(nativePath);
      });
      for (const file of filesToUpload) paths.push(await uploadDroppedFile(file));
      const text = paths.map(richTokenForPath).join(" ");
      await sendDroppedText(text);
      return text;
    };
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
      term.scrollToBottom?.();
      return { cols: term.cols, rows: term.rows };
    };
    window.__turaHandleDroppedData = handleDroppedData;
    for (const eventName of ["dragenter", "dragover"]) {
      shellHost.addEventListener(eventName, (event) => {
        event.preventDefault();
        shellHost.classList.add("dragging");
        if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
      });
    }
    for (const eventName of ["dragleave", "dragend"]) {
      shellHost.addEventListener(eventName, () => shellHost.classList.remove("dragging"));
    }
    shellHost.addEventListener("drop", (event) => {
      event.preventDefault();
      shellHost.classList.remove("dragging");
      handleDroppedData(event.dataTransfer).catch((error) => {
        console.warn("drop failed", error);
        term.write("\\r\\n[drop failed: " + String(error?.message || error) + "]\\r\\n");
      });
    });
    shellHost.addEventListener("paste", (event) => {
      const files = [...(event.clipboardData?.files || [])];
      if (!files.length) return;
      event.preventDefault();
      handleDroppedData(event.clipboardData).catch((error) => {
        console.warn("paste failed", error);
        term.write("\\r\\n[paste failed: " + String(error?.message || error) + "]\\r\\n");
      });
    });
    const resizeObserver = new ResizeObserver(() => {
      window.__turaFit();
    });
    resizeObserver.observe(document.getElementById("terminal"));
    addEventListener("resize", () => {
      window.__turaFit();
    });
    const events = new EventSource("/" + profile + "/events" + instanceQuery);
    let writeQueue = Promise.resolve();
    const repaintSequences = [
      String.fromCharCode(27) + "c",
      String.fromCharCode(27) + "[3J",
      String.fromCharCode(27) + "[2J",
      String.fromCharCode(27) + "[H",
    ];
    const isRepaintFrame = (data) =>
      repaintSequences.some((sequence) => data.includes(sequence)) ||
      data.includes("[3J") ||
      data.includes("[2J");
    const writeTerminal = (data) => {
      const repaintFrame = isRepaintFrame(data);
      writeQueue = writeQueue
        .then(() =>
          new Promise((resolve) => {
            term.write(data, () => {
              if (repaintFrame) term.scrollToBottom?.();
              resolve();
            });
          }),
        )
        .catch((error) => console.warn("terminal write failed", error));
    };
    events.onmessage = (event) => {
      writeTerminal(JSON.parse(event.data));
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
async function saveDroppedFile(body) {
  const name = sanitizeDropFileName(body?.name || "attachment.bin");
  const data = typeof body?.data === "string" ? body.data : "";
  if (!data) throw new Error("drop file payload is empty");
  return saveInputBytes(name, Buffer.from(data, "base64"));
}
async function saveDroppedPath(body) {
  const rawPath = typeof body?.path === "string" ? body.path.trim() : "";
  if (!rawPath) throw new Error("drop path is empty");
  const sourcePath = path.resolve(rawPath);
  if (!path.isAbsolute(rawPath)) throw new Error("drop path must be absolute");
  const stat = await fsp.stat(sourcePath);
  if (!stat.isFile()) throw new Error("drop path is not a file");
  return saveInputBytes(path.basename(sourcePath), await fsp.readFile(sourcePath));
}
async function saveInputBytes(name, bytes) {
  const attachmentsDir = path.resolve(workspace, ".tura", "media", "input");
  await fsp.mkdir(attachmentsDir, { recursive: true });
  const safeName = sanitizeDropFileName(name || "attachment.bin");
  const prefix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const filePath = path.join(attachmentsDir, `${prefix}-${safeName}`);
  await fsp.writeFile(filePath, bytes);
  return path.relative(workspace, filePath).replaceAll(path.sep, "/");
}
function sanitizeDropFileName(value) {
  const cleaned = path
    .basename(String(value))
    .replace(/[<>:"/\\|?*\x00-\x1f]/gu, "-")
    .trim();
  return cleaned || "attachment.bin";
}
function runtimeFor(profile, key) {
  profile.runtimes ??= new Map();
  const runtimeKey = key || "default";
  let runtime = profile.runtimes.get(runtimeKey);
  if (!runtime) {
    runtime = {
      clients: new Set(),
      term: undefined,
      initialComposer: "",
      initialSessionId: "",
      outputBuffer: "",
      outputFlush: undefined,
    };
    profile.runtimes.set(runtimeKey, runtime);
  }
  return runtime;
}
function broadcast(runtime, data) {
  const payload = `data: ${JSON.stringify(data)}\n\n`;
  for (const res of runtime.clients) res.write(payload);
}
function queueOutput(runtime, data) {
  runtime.outputBuffer += data;
  const lastFrameStart = lastRepaintFrameStart(runtime.outputBuffer);
  if (lastFrameStart > 0) runtime.outputBuffer = runtime.outputBuffer.slice(lastFrameStart);
  if (runtime.outputFlush) return;
  runtime.outputFlush = setTimeout(() => {
    runtime.outputFlush = undefined;
    const raw = runtime.outputBuffer;
    runtime.outputBuffer = "";
    const output = isClearFrame(raw)
      ? normalizePanelRailStarts(`\x1b[3J\x1b[2J\x1b[H${raw}`)
      : normalizePanelRailStarts(raw);
    if (output) broadcast(runtime, output);
  }, 16);
}
function lastRepaintFrameStart(data) {
  const resetTerminal = data.lastIndexOf("\x1bc");
  if (resetTerminal >= 0) return resetTerminal;
  const clearScrollback = data.lastIndexOf("\x1b[3J");
  if (clearScrollback >= 0) return clearScrollback;
  const clearScreen = data.lastIndexOf("\x1b[2J");
  if (clearScreen >= 0) return clearScreen;
  const home = data.lastIndexOf("\x1b[H");
  if (home >= 0) return home;
  return -1;
}
function isClearFrame(data) {
  return (
    data.includes("\x1bc") ||
    data.includes("\x1b[H") ||
    data.includes("\x1b[2J") ||
    data.includes("\x1b[3J")
  );
}
const panelBackground = "\x1b[48;2;32;32;34m";
const panelAssistantRail = "\x1b[38;2;107;107;107m";
const panelUserRail = "\x1b[38;2;238;238;238m";
function normalizePanelRailStarts(data) {
  return data.replace(/(^|\r\n|\x1b\[H)([▏|])\x1b\[39m/gu, (match, lineStart, rail, offset) => {
    const recent = data.slice(Math.max(0, offset - 48), offset);
    const railColor = recent.includes(panelUserRail) ? panelUserRail : panelAssistantRail;
    return `${lineStart}${panelBackground}${railColor}${rail}\x1b[0m${panelBackground}`;
  });
}
function startTui(profile, runtime, size = undefined) {
  if (runtime.term) return runtime.term;
  const cols = Number(size?.cols) || 120;
  const rows = Number(size?.rows) || 22;
  const initialSessionArgs = runtime.initialSessionId
    ? ["--initial-session", runtime.initialSessionId]
    : [];
  const term = pty.spawn(
    tuiCommand,
    [
      ...tuiBaseArgs,
      ...(mockMode ? ["--mock"] : ["--gateway-url", gatewayUrl]),
      "--cwd",
      workspace,
      ...initialSessionArgs,
      ...profile.args,
    ],
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
        TURA_TUI_MOCK_INITIAL_COMPOSER: runtime.initialComposer || "",
      },
      shell,
    },
  );
  runtime.term = term;
  term.onData((data) => queueOutput(runtime, data));
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
  const initialComposer = url.searchParams.get("initialComposer");
  if (initialComposer !== null && !runtime.term) runtime.initialComposer = initialComposer;
  const initialSessionId =
    url.searchParams.get("sessionId") ?? url.searchParams.get("initialSession");
  if (initialSessionId !== null && !runtime.term) runtime.initialSessionId = initialSessionId;
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
    startTui(profile, runtime);
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
  if (req.method === "POST" && leaf === "/drop-file") {
    try {
      const filePath = await saveDroppedFile(await readJson(req));
      return send(res, { ok: true, path: filePath });
    } catch (error) {
      return send(res, { error: error instanceof Error ? error.message : String(error) }, 400);
    }
  }
  if (req.method === "POST" && leaf === "/drop-path") {
    try {
      const filePath = await saveDroppedPath(await readJson(req));
      return send(res, { ok: true, path: filePath });
    } catch (error) {
      return send(res, { error: error instanceof Error ? error.message : String(error) }, 400);
    }
  }
  return send(res, { error: "not found" }, 404);
});
server.listen(port, "127.0.0.1", () => {
  console.log(`Tura TUI web terminal: http://127.0.0.1:${port}`);
  console.log(`Gateway: ${gatewayUrl}`);
  console.log(`Workspace: ${workspace}`);
});
