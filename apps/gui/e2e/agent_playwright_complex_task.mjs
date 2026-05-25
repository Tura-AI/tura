#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..", "..");
const nonce = process.argv[2] || `manual-${Date.now()}`;
const safeNonce = nonce.replace(/[^A-Za-z0-9_.-]/g, "-");
const runRoot = path.join(
  repoRoot,
  "target",
  "gui-agent-playwright",
  safeNonce,
);
const artifacts = path.join(runRoot, "artifacts");
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx";
let port = Number(process.env.TURA_AGENT_PLAYWRIGHT_PORT || 5277);

function marker(step, detail) {
  console.log(`TURA_PLAYWRIGHT_STEP ${nonce} ${step} ${detail}`);
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function write(file, text) {
  mkdirp(path.dirname(file));
  fs.writeFileSync(file, text);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || runRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 180_000,
    maxBuffer: 64 * 1024 * 1024,
    windowsHide: true,
    shell: process.platform === "win32",
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status}`,
    );
  }
  return result;
}

function findOpenPort(preferred) {
  return new Promise((resolve, reject) => {
    const candidates = [preferred, 0];
    const tryNext = () => {
      const candidate = candidates.shift();
      const server = net.createServer();
      server.once("error", (error) => {
        server.close();
        if (candidates.length > 0) {
          tryNext();
          return;
        }
        reject(error);
      });
      server.listen(candidate, "127.0.0.1", () => {
        const address = server.address();
        const selected =
          typeof address === "object" && address ? address.port : candidate;
        server.close(() => resolve(selected));
      });
    };
    tryNext();
  });
}

function createFixture() {
  mkdirp(artifacts);
  write(
    path.join(runRoot, "package.json"),
    JSON.stringify(
      {
        type: "module",
        scripts: {
          dev: "vite --host 127.0.0.1",
          probe: "node tools/probe.mjs",
        },
        dependencies: {
          "@vitejs/plugin-react": "latest",
          vite: "latest",
          playwright: "latest",
        },
        devDependencies: {},
      },
      null,
      2,
    ),
  );
  write(
    path.join(runRoot, "index.html"),
    `<div id="app"></div><script type="module" src="/src/main.js"></script>`,
  );
  write(
    path.join(runRoot, "src", "main.js"),
    [
      'const app = document.querySelector("#app");',
      'const cards = ["Desktop capture", "Mobile capture", "Streaming check", "Error state"];',
      'let stream = "Preparing checks";',
      'let error = "";',
      "let modal = false;",
      "",
      "function cardHtml() {",
      "  return cards",
      "    .map((title, index) => '<article class=\"card\"><small>STEP ' + (index + 1) + '</small><h2>' + title + '</h2><p>Playwright verified state.</p></article>')",
      '    .join("");',
      "}",
      "",
      "function render() {",
      "  app.innerHTML = '<main class=\"shell\">' +",
      '    \'<header><div><p>Retail command center</p><h1>Daily Operations Board</h1></div><div class="actions"><button id="modal">Open run</button><button id="error">Error</button></div></header>\' +',
      '    \'<section class="grid">\' + cardHtml() + "</section>" +',
      '    \'<section class="stream" aria-label="Streaming output"><p>\' + stream + "</p></section>" +',
      '    (error ? \'<div role="alert">\' + error + "</div>" : "") +',
      '    (modal ? \'<div class="modal" role="dialog" aria-label="Run details"><h2>Run details</h2><p>Modal screenshot ready.</p><button id="close">Close</button></div>\' : "") +',
      '    "</main>";',
      '  document.querySelector("#modal")?.addEventListener("click", () => { modal = true; render(); });',
      '  document.querySelector("#error")?.addEventListener("click", () => { error = "Visible error state for screenshot"; render(); });',
      '  document.querySelector("#close")?.addEventListener("click", () => { modal = false; render(); });',
      "}",
      "",
      "render();",
      'setTimeout(() => { stream = "Streaming chunk one"; render(); }, 300);',
      'setTimeout(() => { stream = "Streaming chunk one, chunk two"; render(); }, 700);',
      'setTimeout(() => { stream = "Streaming chunk one, chunk two, done"; render(); }, 1200);',
    ].join("\n"),
  );
  write(path.join(runRoot, "src", "style.css"), "");
  write(
    path.join(runRoot, "src", "main.js"),
    fs
      .readFileSync(path.join(runRoot, "src", "main.js"), "utf8")
      .replace(
        'const app = document.querySelector("#app");',
        'import "./styles.css";\nconst app = document.querySelector("#app");',
      ),
  );
  write(
    path.join(runRoot, "src", "styles.css"),
    `
* { box-sizing: border-box; }
body { margin: 0; font-family: Inter, ui-sans-serif, system-ui, sans-serif; background: #f8f8f7; color: #111; }
.shell { min-height: 100vh; padding: 42px; display: grid; gap: 24px; }
header { display: flex; align-items: end; justify-content: space-between; gap: 16px; border-bottom: 1px solid #ddd; padding-bottom: 18px; }
header p { margin: 0 0 8px; font-size: 13px; color: #747474; }
h1 { margin: 0; font-size: clamp(34px, 6vw, 72px); line-height: 1; letter-spacing: 0; }
button { min-height: 38px; border: 1px solid #111; border-radius: 6px; background: transparent; padding: 0 14px; }
.grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 14px; }
.card { min-height: 170px; border: 1px solid #ddd; border-radius: 8px; padding: 18px; background: #fff; animation: enter 420ms ease both; }
.card small { color: #747474; }
.card h2 { font-size: 22px; line-height: 1.12; }
.stream, [role="alert"] { border: 1px solid #ddd; border-radius: 8px; padding: 18px; background: #fff; }
[role="alert"] { border-color: #111; }
.modal { position: fixed; inset: 15% auto auto 50%; transform: translateX(-50%); width: min(420px, calc(100vw - 32px)); border: 1px solid #111; border-radius: 8px; padding: 24px; background: #fff; }
@keyframes enter { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
@media (max-width: 640px) { .shell { padding: 20px; } header { display: grid; } .grid { grid-template-columns: 1fr; } }
`,
  );
  write(
    path.join(runRoot, "tools", "probe.mjs"),
    `
import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";

const out = path.resolve("artifacts");
fs.mkdirSync(out, { recursive: true });
const baseURL = process.env.BASE_URL;
const nonce = ${JSON.stringify(nonce)};
function marker(step, detail) {
  console.log("TURA_PLAYWRIGHT_STEP " + nonce + " " + step + " " + JSON.stringify(detail));
}

const browser = await chromium.launch({ headless: true });
try {
  const desktop = await browser.newPage({ viewport: { width: 1440, height: 980 } });
  await desktop.goto(baseURL);
  await desktop.screenshot({ path: path.join(out, "desktop.png"), fullPage: true });
  marker("desktop", { screenshot: "artifacts/desktop.png", title: await desktop.locator("h1").innerText() });

  const mobile = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await mobile.goto(baseURL);
  await mobile.screenshot({ path: path.join(out, "mobile.png"), fullPage: true });
  marker("mobile", { screenshot: "artifacts/mobile.png", overflow: await mobile.evaluate(() => document.documentElement.scrollWidth > window.innerWidth) });

  await desktop.getByRole("button", { name: "Open run" }).click();
  await desktop.getByRole("dialog", { name: "Run details" }).waitFor();
  await desktop.screenshot({ path: path.join(out, "modal.png"), fullPage: true });
  marker("modal", { screenshot: "artifacts/modal.png" });

  await desktop.getByRole("button", { name: "Close" }).click();
  await desktop.waitForTimeout(1400);
  const streamText = await desktop.locator(".stream p").innerText();
  await desktop.screenshot({ path: path.join(out, "streaming.png"), fullPage: true });
  marker("streaming", { screenshot: "artifacts/streaming.png", stable: streamText.includes("done"), text: streamText });

  await desktop.getByRole("button", { name: "Error" }).click();
  await desktop.getByRole("alert").waitFor();
  await desktop.screenshot({ path: path.join(out, "error-state.png"), fullPage: true });
  marker("error-state", { screenshot: "artifacts/error-state.png", alert: await desktop.getByRole("alert").innerText() });
} finally {
  await browser.close();
  marker("cleanup", { browser: "closed" });
}
`,
  );
  marker("setup", `fixture-created ${runRoot}`);
}

function startServer() {
  const out = fs.openSync(path.join(artifacts, "vite.log"), "w");
  const err = fs.openSync(path.join(artifacts, "vite.err.log"), "w");
  const child = spawn(
    npmCmd,
    ["run", "dev", "--", "--port", String(port), "--strictPort"],
    {
      cwd: runRoot,
      stdio: ["ignore", out, err],
      shell: process.platform === "win32",
      windowsHide: true,
    },
  );
  fs.writeFileSync(path.join(artifacts, "vite.pid"), String(child.pid));
  return child;
}

async function waitForServer() {
  const deadline = Date.now() + 45_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}`);
      if (response.ok) return true;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return false;
}

function stopServer(child) {
  if (!child || child.killed) return;
  try {
    if (process.platform === "win32" && child.pid) {
      spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
        windowsHide: true,
      });
    } else {
      child.kill("SIGTERM");
    }
  } catch {}
}

createFixture();
run(npmCmd, ["install"], { timeoutMs: 180_000 });
run(npxCmd, ["playwright", "install", "chromium"], { timeoutMs: 240_000 });
port = await findOpenPort(port);
const server = startServer();
try {
  if (!(await waitForServer())) throw new Error("vite did not become ready");
  marker("setup", `vite-ready port=${port}`);
  run(npmCmd, ["run", "probe"], {
    env: { BASE_URL: `http://127.0.0.1:${port}` },
    timeoutMs: 120_000,
  });
  const files = fs
    .readdirSync(artifacts)
    .filter((name) => name.endsWith(".png"));
  const summary = { nonce, runRoot, artifacts, files };
  fs.writeFileSync(
    path.join(artifacts, "summary.json"),
    JSON.stringify(summary, null, 2),
  );
  fs.writeFileSync(
    path.join(runRoot, "summary.json"),
    JSON.stringify(summary, null, 2),
  );
  marker("cleanup", `artifacts=${files.join(",")}`);
} finally {
  stopServer(server);
}
