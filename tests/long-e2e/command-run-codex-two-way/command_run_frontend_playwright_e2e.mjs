#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `frontend-playwright-${Date.now()}`
const runRoot = path.join(repoRoot, "target", "command-run-frontend-playwright", runId)
const summaryPath = path.join(runRoot, "summary.json")
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const claudeModel = process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 20 * 60_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "current-shll,codex-main,tura-fast-shll")
const prepOnly = (process.env.COMMAND_RUN_AGENT_PREP_ONLY || "0") === "1"
const smokeOnly = (process.env.COMMAND_RUN_AGENT_SMOKE_ONLY || "0") === "1"
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm"
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx"

const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")
const codexCurrentExe = path.join(
  process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex"),
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
)
const codexMainExe = findCodexMainExe()
const claudeExe = findClaudeExe()

function findCodexMainExe() {
  const exeName = process.platform === "win32" ? "codex.exe" : "codex"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_CODEX_MAIN_ROOT,
    path.join(homeDir, "Documents", "codex-main"),
    path.join(homeDir, "codex-main"),
    path.join(homeDir, "RustroverProjects", "codex-main"),
  ]
    .filter(Boolean)
    .map((root) => path.join(root, "codex-rs", "target", "debug", exeName))
  return candidates.find((candidate) => fs.existsSync(candidate)) || candidates[0]
}

function findClaudeExe() {
  const exeName = process.platform === "win32" ? "claude.exe" : "claude"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_CLAUDE_EXE,
    process.platform === "win32"
      ? path.join(homeDir, "AppData", "Local", "Packages", "Claude_pzs8sxrjxfjjc", "LocalCache", "Roaming", "Claude", "claude-code", "2.1.128", exeName)
      : null,
    "claude",
  ].filter(Boolean)
  return candidates.find((candidate) => candidate === "claude" || fs.existsSync(candidate)) || candidates[0]
}

function parseAgents(value) {
  const alias = new Map([
    ["current", "current-shll"],
    ["current-shll", "current-shll"],
    ["codex-current", "current-shll"],
    ["main", "codex-main"],
    ["codex-main", "codex-main"],
    ["tura", "tura-fast-shll"],
    ["tura-shll", "tura-shll"],
    ["tura-coding", "tura-shll"],
    ["tura-coding-agent", "tura-shll"],
    ["tura-fast", "tura-fast-shll"],
    ["tura-fast-shll", "tura-fast-shll"],
    ["claude", "claude-code"],
    ["claude-code", "claude-code"],
    ["claude-opus", "claude-code"],
  ])
  return String(value)
    .split(",")
    .map((item) => alias.get(item.trim().toLowerCase()))
    .filter(Boolean)
}

function run(command, args, options = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: options.maxBuffer || 256 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    shell: options.shell || false,
    windowsHide: true,
  })
  return {
    command,
    args,
    status: result.status,
    signal: result.signal,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    duration_ms: Math.round(performance.now() - started),
    error: result.error ? String(result.error.stack || result.error.message || result.error) : null,
  }
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
  }
  return result
}

function emptyRun(reason) {
  return {
    command: "",
    args: [],
    status: null,
    signal: null,
    stdout: "",
    stderr: "",
    duration_ms: 0,
    error: reason,
  }
}

function runLive(command, args, options = {}) {
  const started = performance.now()
  const stdoutPath = options.stdoutPath
  const stderrPath = options.stderrPath
  const statusPath = options.statusPath
  if (stdoutPath) mkdirp(path.dirname(stdoutPath))
  if (stderrPath) mkdirp(path.dirname(stderrPath))
  const stdoutStream = stdoutPath ? fs.createWriteStream(stdoutPath, { flags: "w" }) : null
  const stderrStream = stderrPath ? fs.createWriteStream(stderrPath, { flags: "w" }) : null
  let stdout = ""
  let stderr = ""
  let timedOut = false

  function writeStatus(extra) {
    if (!statusPath) return
    writeFile(statusPath, JSON.stringify({
      command,
      args,
      cwd: options.cwd || repoRoot,
      started_at: new Date(Date.now() - Math.round(performance.now() - started)).toISOString(),
      elapsed_ms: Math.round(performance.now() - started),
      ...extra,
    }, null, 2))
  }

  writeStatus({ status: "running" })
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["ignore", "pipe", "pipe"],
      shell: options.shell || false,
      windowsHide: true,
    })

    const timer = setTimeout(() => {
      timedOut = true
      try {
        if (process.platform === "win32" && child.pid) {
          spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true })
        } else {
          child.kill("SIGKILL")
        }
      } catch {}
    }, options.timeoutMs || timeoutMs)

    child.stdout?.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stdout += text
      stdoutStream?.write(text)
      writeStatus({ status: "running", stdout_bytes: Buffer.byteLength(stdout), stderr_bytes: Buffer.byteLength(stderr) })
    })
    child.stderr?.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stderr += text
      stderrStream?.write(text)
      writeStatus({ status: "running", stdout_bytes: Buffer.byteLength(stdout), stderr_bytes: Buffer.byteLength(stderr) })
    })

    child.on("error", (error) => {
      clearTimeout(timer)
      stdoutStream?.end()
      stderrStream?.end()
      const result = {
        command,
        args,
        status: null,
        signal: null,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        error: String(error.stack || error.message || error),
      }
      writeStatus({ status: "error", result })
      resolve(result)
    })

    child.on("close", (status, signal) => {
      clearTimeout(timer)
      stdoutStream?.end()
      stderrStream?.end()
      const result = {
        command,
        args,
        status,
        signal,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        error: timedOut ? `timed out after ${options.timeoutMs || timeoutMs}ms` : null,
      }
      writeStatus({ status: timedOut ? "timeout" : "closed", result })
      resolve(result)
    })
  })
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text)
}

function copyDir(src, dst) {
  mkdirp(dst)
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name)
    const to = path.join(dst, entry.name)
    if (entry.isDirectory()) copyDir(from, to)
    else fs.copyFileSync(from, to)
  }
}

function createFixtureTemplate() {
  const template = path.join(runRoot, "template")
  mkdirp(template)
  writeFile(path.join(template, "package.json"), JSON.stringify({
    scripts: {
      start: "vite --host 127.0.0.1",
      "screenshot:desktop": "node tools/capture.mjs desktop",
      "screenshot:mobile": "node tools/capture.mjs mobile",
      "screenshot:modal": "node tools/capture.mjs modal",
      inspect: "node tools/inspect.mjs",
      "check:a11y": "node tools/a11y_check.mjs",
      "probe:flow": "node tools/probe_flow.mjs",
      "smoke:playwright": "node tools/playwright_smoke.mjs"
    },
    dependencies: {
      "@vitejs/plugin-react": "latest",
      vite: "latest",
      react: "latest",
      "react-dom": "latest",
      playwright: "latest"
    },
    devDependencies: {}
  }, null, 2))
  writeFile(path.join(template, "index.html"), `<div id="root"></div><script type="module" src="/src/App.jsx"></script>\n`)
  writeFile(path.join(template, "src", "App.jsx"), buggyApp())
  writeFile(path.join(template, "src", "styles.css"), buggyCss())
  writeFile(path.join(template, "tools", "capture.mjs"), captureScript())
  writeFile(path.join(template, "tools", "inspect.mjs"), inspectScript())
  writeFile(path.join(template, "tools", "a11y_check.mjs"), a11yScript())
  writeFile(path.join(template, "tools", "probe_flow.mjs"), probeScript())
  writeFile(path.join(template, "tools", "playwright_smoke.mjs"), playwrightSmokeScript())
  writeFile(path.join(template, "REFERENCE_NOTES.md"), referenceNotes())
  return template
}

function buggyApp() {
  return `import React, { useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

const seedTasks = [
  { id: "A-17", title: "Sync checkout rates", owner: "Mira", status: "Blocked", priority: "High", due: "Today" },
  { id: "B-22", title: "Audit catalog feed", owner: "Jon", status: "Queued", priority: "Med", due: "Tomorrow" },
  { id: "C-09", title: "Repair refund export", owner: "Nia", status: "Active", priority: "High", due: "Fri" },
  { id: "D-31", title: "Reconcile carrier SLA", owner: "Pax", status: "Active", priority: "Low", due: "Mon" },
  { id: "E-04", title: "Review fraud rules", owner: "Ivo", status: "Queued", priority: "Med", due: "Wed" },
];

function App() {
  const [filter, setFilter] = useState("all");
  const [modalOpen, setModalOpen] = useState(false);
  const [toast, setToast] = useState("");
  const [tasks, setTasks] = useState(seedTasks);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(null);
  const [audit, setAudit] = useState([]);

  const filtered = useMemo(() => {
    const byFilter = filter === "all" ? tasks : tasks.filter((task) => task.status.toLowerCase() === filter);
    return byFilter.filter((task) => task.title.toLowerCase().includes(query.toLowerCase()));
  }, [filter, query, tasks]);

  function addTask() {
    const next = { id: "N-" + Math.floor(Math.random() * 90 + 10), title: "New launch task", owner: "Ops", status: "Queued", priority: "Low", due: "Next" };
    setTasks([...tasks, next]);
    setToast("Task added");
    setAudit([...audit, "created " + next.id]);
  }

  function completeTask(id) {
    setTasks(tasks.map((task) => task.id === id ? { ...task, status: "Complete" } : task));
    setToast("Completed " + id);
    setAudit([...audit, "completed " + id]);
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">Tura Ops</div>
        <button>Overview</button>
        <button>Queues</button>
        <button>Reports</button>
        <button>Settings</button>
      </aside>
      <section className="content">
        <header className="hero">
          <div>
            <p className="eyebrow">Retail command center</p>
            <h1>Daily Operations Board</h1>
            <p className="subtitle">Track fulfillment incidents, ownership, and release readiness.</p>
          </div>
          <div className="summary-grid">
            <div><strong>{tasks.length}</strong><span>Total</span></div>
            <div><strong>{tasks.filter((t) => t.status === "Active").length}</strong><span>Active</span></div>
            <div><strong>{tasks.filter((t) => t.status === "Blocked").length}</strong><span>Blocked</span></div>
          </div>
        </header>
        <div className="toolbar">
          <input aria-label="Search tasks" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search tasks" />
          {["all", "active", "queued", "blocked"].map((item) => (
            <button key={item} className={filter === item ? "active" : ""} onClick={() => setFilter(item)}>{item}</button>
          ))}
          <button className="primary" onClick={() => setModalOpen(true)}>New task</button>
        </div>
        {toast && <div className="toast" role="status">{toast}</div>}
        <section className="board" aria-label="Task board">
          {filtered.map((task) => (
            <article className={"task-card priority-" + task.priority.toLowerCase()} key={task.id}>
              <div className="task-head">
                <span>{task.id}</span>
                <b>{task.status}</b>
              </div>
              <h2>{task.title}</h2>
              <p>Owner {task.owner} - due {task.due}</p>
              <div className="task-actions">
                <button onClick={() => setSelected(task)}>Details</button>
                <button onClick={() => completeTask(task.id)}>Complete</button>
              </div>
            </article>
          ))}
        </section>
        <section className="analytics">
          <h2>Readiness</h2>
          <div className="bars">
            <span style={{"--v": "42%"}}>Payments</span>
            <span style={{"--v": "71%"}}>Inventory</span>
            <span style={{"--v": "58%"}}>Shipping</span>
          </div>
        </section>
      </section>
      {modalOpen && (
        <div className="modal-backdrop" onClick={() => setModalOpen(false)}>
          <div className="modal" role="dialog" aria-modal="true" aria-label="Create task" onClick={(event) => event.stopPropagation()}>
            <h2>Create work item</h2>
            <label>Title<input defaultValue="New launch task" /></label>
            <label>Owner<input defaultValue="Ops" /></label>
            <div className="modal-actions">
              <button onClick={() => setModalOpen(false)}>Cancel</button>
              <button onClick={() => { addTask(); setModalOpen(false); }}>Create</button>
            </div>
          </div>
        </div>
      )}
      {selected && (
        <div className="drawer">
          <button aria-label="Close details" onClick={() => setSelected(null)}>x</button>
          <h2>{selected.title}</h2>
          <p>Status: {selected.status}</p>
          <p>Priority: {selected.priority}</p>
        </div>
      )}
      <div className="audit-log" aria-label="Audit log">{audit.join(" | ")}</div>
    </main>
  );
}

createRoot(document.getElementById("root")).render(<App />);
`
}

function buggyCss() {
  return `:root {
  font-family: Arial, sans-serif;
  color: #14213d;
  background: #f6f8fb;
}
* { box-sizing: border-box; }
body { margin: 0; }
.app-shell { min-height: 100vh; display: grid; grid-template-columns: 280px 1fr; }
.sidebar {
  background: #111827;
  color: white;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  position: sticky;
  top: 0;
  height: 100vh;
}
.brand { font-size: 28px; font-weight: 800; margin-bottom: 30px; }
.sidebar button { color: #cbd5e1; background: transparent; border: 0; text-align: left; padding: 12px; font-size: 16px; }
.content { padding: 30px 42px 60px; overflow: hidden; }
.hero {
  min-height: 250px;
  background: linear-gradient(135deg, #0f766e, #2563eb);
  color: white;
  border-radius: 34px;
  padding: 46px;
  display: flex;
  justify-content: space-between;
  align-items: end;
  box-shadow: 0 20px 50px rgba(15, 23, 42, 0.22);
}
.eyebrow { text-transform: uppercase; letter-spacing: .18em; opacity: .7; }
h1 { font-size: 66px; line-height: .9; margin: 0; }
.subtitle { font-size: 20px; max-width: 500px; }
.summary-grid { display: grid; grid-template-columns: repeat(3, 120px); gap: 14px; }
.summary-grid div { background: rgba(255,255,255,.18); padding: 18px; border-radius: 26px; display: grid; }
.summary-grid strong { font-size: 36px; }
.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: -20px 24px 26px;
  padding: 16px;
  background: white;
  border-radius: 24px;
  box-shadow: 0 16px 30px rgba(15, 23, 42, .18);
  position: relative;
  z-index: 2;
}
.toolbar input { flex: 1; padding: 14px 16px; border: 1px solid #d7dee8; border-radius: 16px; font-size: 16px; }
.toolbar button { border: 1px solid #d7dee8; background: white; padding: 13px 18px; border-radius: 16px; text-transform: capitalize; }
.toolbar button.active { background: #111827; color: white; }
.toolbar .primary { background: #f97316; color: white; border-color: #f97316; }
.toast { background: #16a34a; color: white; margin: 0 0 18px; padding: 10px 14px; border-radius: 10px; width: fit-content; }
.board { display: grid; grid-template-columns: repeat(5, minmax(210px, 1fr)); gap: 18px; }
.task-card {
  background: white;
  border: 1px solid #d7dee8;
  border-radius: 28px;
  padding: 20px;
  min-height: 230px;
  box-shadow: 0 16px 34px rgba(15, 23, 42, .08);
}
.task-head { display: flex; justify-content: space-between; color: #64748b; }
.task-head b { color: #dc2626; }
.task-card h2 { font-size: 26px; line-height: 1; min-height: 70px; }
.task-actions { display: flex; gap: 8px; }
.task-actions button { flex: 1; padding: 12px; border-radius: 14px; border: 1px solid #cbd5e1; background: #f8fafc; }
.analytics { margin-top: 22px; background: white; padding: 22px; border-radius: 28px; }
.bars { display: grid; grid-template-columns: repeat(3, 1fr); gap: 18px; }
.bars span { padding-top: 70px; background: linear-gradient(to top, #2563eb var(--v), #dbeafe var(--v)); border-radius: 18px; text-align: center; min-height: 120px; }
.modal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,.18); display: flex; justify-content: flex-end; align-items: flex-start; padding: 50px; z-index: 8; }
.modal { background: white; padding: 26px; border-radius: 28px; width: 420px; box-shadow: 0 30px 60px rgba(0,0,0,.2); }
.modal label { display: grid; gap: 8px; margin: 16px 0; }
.modal input { padding: 12px; border: 1px solid #cbd5e1; border-radius: 12px; }
.modal-actions { display: flex; justify-content: end; gap: 10px; }
.drawer { position: fixed; right: 0; top: 0; width: 380px; height: 100vh; background: #fff; box-shadow: -20px 0 40px rgba(0,0,0,.15); padding: 30px; z-index: 7; }
.drawer button { float: right; }
.audit-log { position: fixed; left: 290px; bottom: -12px; background: #111827; color: white; padding: 12px; max-width: 50vw; overflow: hidden; white-space: nowrap; }

@media (max-width: 800px) {
  .app-shell { grid-template-columns: 1fr; }
  .sidebar { height: 96px; position: relative; flex-direction: row; overflow: hidden; align-items: center; padding: 18px; }
  .brand { margin: 0 20px 0 0; white-space: nowrap; }
  .content { padding: 18px; }
  .hero { padding: 24px; display: block; min-height: 310px; }
  h1 { font-size: 58px; }
  .summary-grid { grid-template-columns: repeat(3, 1fr); margin-top: 20px; }
  .toolbar { margin: -18px 8px 16px; flex-wrap: nowrap; overflow: hidden; }
  .toolbar input { min-width: 250px; }
  .board { grid-template-columns: repeat(2, minmax(220px, 1fr)); }
  .bars { grid-template-columns: 1fr; }
  .modal-backdrop { padding: 20px; justify-content: center; }
  .drawer { width: 90vw; }
  .audit-log { left: 0; max-width: 100vw; }
}
`
}

function referenceNotes() {
  return `# Operator notes

This is a front-end repair benchmark. Use the visible screenshots in \`reference/\` and the scripts in \`tools/\`.

Recommended workflow:
- Run \`npm install\` if dependencies are missing.
- Run \`npm run screenshot:desktop\`, \`npm run screenshot:mobile\`, and \`npm run screenshot:modal\`.
- Inspect generated screenshots in \`artifacts/\` with the available image-reading tool.
- Fix visual and interaction issues in \`src/App.jsx\` and \`src/styles.css\`.
- Use Playwright scripts to verify interactions. The hidden evaluator checks behavior after the task is complete.
`
}

function captureScript() {
  return `import { chromium } from "playwright";
import fs from "node:fs";
import { fileURLToPath } from "node:url";

const mode = process.argv[2] || "desktop";
const port = Number(process.env.PORT || 4173);
const outDir = new URL("../artifacts/", import.meta.url);
fs.mkdirSync(fileURLToPath(outDir), { recursive: true });
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({
  viewport: mode === "mobile" ? { width: 390, height: 844 } : { width: 1440, height: 980 },
  deviceScaleFactor: 1,
});
await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
if (mode === "modal") {
  await page.getByRole("button", { name: /new task/i }).click();
  await page.waitForTimeout(300);
}
await page.screenshot({ path: fileURLToPath(new URL(\`\${mode}.png\`, outDir)), fullPage: true });
await browser.close();
console.log(\`wrote artifacts/\${mode}.png\`);
`
}

function playwrightSmokeScript() {
  return `import { spawn } from "node:child_process";
import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const port = Number(process.env.PORT || process.argv[2] || 4173);
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const outDir = fileURLToPath(new URL("../artifacts/", import.meta.url));
fs.mkdirSync(outDir, { recursive: true });

function startServer() {
  return spawn(npmCmd, ["run", "start", "--", "--port", String(port), "--strictPort"], {
    cwd: process.cwd(),
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    shell: process.platform === "win32",
  });
}

async function waitForServer() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(\`http://127.0.0.1:\${port}\`);
      if (response.ok) return;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(\`Vite server did not become ready on port \${port}\`);
}

function stopServer(child) {
  if (!child || child.killed) return;
  try {
    if (process.platform === "win32" && child.pid) {
      spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
    } else {
      child.kill("SIGTERM");
    }
  } catch {}
}

const server = startServer();
try {
  await waitForServer();
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 980 }, deviceScaleFactor: 1 });
  await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
  const title = await page.locator("h1").textContent();
  const cardCount = await page.locator(".task-card").count();
  const screenshot = path.join(outDir, "desktop.png");
  await page.screenshot({ path: screenshot, fullPage: true });
  await browser.close();
  console.log(JSON.stringify({ ok: true, port, screenshot, title, cardCount }, null, 2));
} finally {
  stopServer(server);
}
`
}

function inspectScript() {
  return `import { chromium } from "playwright";
const port = Number(process.env.PORT || 4173);
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 980 } });
await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
const data = await page.evaluate(() => ({
  title: document.querySelector("h1")?.textContent,
  cards: [...document.querySelectorAll(".task-card")].map((node) => node.textContent),
  buttons: [...document.querySelectorAll("button")].map((node) => node.textContent || node.getAttribute("aria-label")),
  bodyWidth: document.body.scrollWidth,
  viewportWidth: innerWidth,
  audit: document.querySelector(".audit-log")?.textContent,
}));
console.log(JSON.stringify(data, null, 2));
await browser.close();
`
}

function a11yScript() {
  return `import { chromium } from "playwright";
const port = Number(process.env.PORT || 4173);
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
const result = await page.evaluate(() => ({
  horizontalOverflow: document.documentElement.scrollWidth > window.innerWidth + 1,
  clippedCards: [...document.querySelectorAll(".task-card")].filter((el) => el.scrollHeight > el.clientHeight + 2).length,
  dialogName: document.querySelector('[role="dialog"]')?.getAttribute("aria-label") || null,
}));
console.log(JSON.stringify(result, null, 2));
if (result.horizontalOverflow || result.clippedCards > 0) process.exit(1);
await browser.close();
`
}

function probeScript() {
  return `import { chromium } from "playwright";
const port = Number(process.env.PORT || 4173);
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
await page.getByRole("button", { name: /active/i }).click();
const activeCount = await page.locator(".task-card").count();
await page.getByRole("button", { name: /new task/i }).click();
await page.getByRole("button", { name: /^create$/i }).click();
const toast = await page.getByRole("status").textContent();
await page.getByRole("button", { name: /all/i }).click();
const afterCreate = await page.locator(".task-card").count();
await page.locator(".task-card").first().getByRole("button", { name: /complete/i }).click();
const audit = await page.locator(".audit-log").textContent();
console.log(JSON.stringify({ activeCount, toast, afterCreate, audit }, null, 2));
if (!toast?.toLowerCase().includes("task added")) process.exit(1);
if (!audit?.toLowerCase().includes("completed")) process.exit(1);
await browser.close();
`
}

function hiddenEvaluator() {
  return `import { createRequire } from "node:module";
import fs from "node:fs";
const workspace = process.argv[2];
const port = Number(process.argv[3]);
const require = createRequire(workspace + "/package.json");
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: true });
const failures = [];
const score = { visual: 0, interaction: 0, phase2: 0 };

async function pageFor(viewport) {
  const page = await browser.newPage({ viewport });
  await page.goto(\`http://127.0.0.1:\${port}\`, { waitUntil: "networkidle" });
  return page;
}

let page = await pageFor({ width: 1440, height: 980 });
const desktop = await page.evaluate(() => {
  const hero = document.querySelector(".hero")?.getBoundingClientRect();
  const cards = [...document.querySelectorAll(".task-card")].map((el) => el.getBoundingClientRect());
  const toolbar = document.querySelector(".toolbar")?.getBoundingClientRect();
  return {
    h1: document.querySelector("h1")?.textContent || "",
    heroHeight: hero?.height || 0,
    cardCount: cards.length,
    minCardWidth: Math.min(...cards.map((r) => r.width)),
    toolbarTop: toolbar?.top || 0,
    overflow: document.documentElement.scrollWidth > innerWidth + 1,
    bodyText: document.body.textContent || "",
  };
});
if (/daily operations board/i.test(desktop.h1)) score.visual += 1; else failures.push("desktop heading missing");
if (desktop.heroHeight >= 150 && desktop.heroHeight <= 340) score.visual += 1; else failures.push("desktop hero height wrong");
if (desktop.cardCount >= 5 && desktop.minCardWidth >= 180) score.visual += 1; else failures.push("desktop cards cramped/missing");
if (!desktop.overflow) score.visual += 1; else failures.push("desktop horizontal overflow");
await page.close();

page = await pageFor({ width: 390, height: 844 });
const mobile = await page.evaluate(() => ({
  overflow: document.documentElement.scrollWidth > innerWidth + 1,
  cardCount: document.querySelectorAll(".task-card").length,
  firstCardTop: document.querySelector(".task-card")?.getBoundingClientRect().top || 0,
  navHeight: document.querySelector(".sidebar")?.getBoundingClientRect().height || 0,
  h1Size: parseFloat(getComputedStyle(document.querySelector("h1")).fontSize),
}));
if (!mobile.overflow) score.visual += 1; else failures.push("mobile horizontal overflow");
if (mobile.navHeight <= 130) score.visual += 1; else failures.push("mobile nav too tall");
if (mobile.h1Size <= 46) score.visual += 1; else failures.push("mobile heading too large");
await page.close();

page = await pageFor({ width: 1280, height: 900 });
await page.getByRole("button", { name: /blocked/i }).click();
const blockedCount = await page.locator(".task-card").count();
if (blockedCount === 1) score.interaction += 1; else failures.push("blocked filter wrong");
await page.getByLabel(/search tasks/i).fill("refund");
const refundCount = await page.locator(".task-card").count();
if (refundCount === 0) score.interaction += 1; else failures.push("search/filter intersection wrong");
await page.getByRole("button", { name: /all/i }).click();
await page.getByRole("button", { name: /new task/i }).click();
if (await page.getByRole("dialog", { name: /create (task|work item)/i }).count()) score.interaction += 1; else failures.push("dialog inaccessible");
await page.getByRole("button", { name: /^create$/i }).click();
const toast = await page.getByRole("status").textContent().catch(() => "");
if (/task added/i.test(toast || "")) score.interaction += 1; else failures.push("create toast missing");
await page.locator(".task-card").first().getByRole("button", { name: /complete/i }).click();
const audit = await page.locator(".audit-log").textContent().catch(() => "");
if (/completed/i.test(audit || "")) score.interaction += 1; else failures.push("complete audit missing");
await page.close();

const source = fs.readFileSync(\`\${workspace}/src/App.jsx\`, "utf8") + "\\n" + fs.readFileSync(\`\${workspace}/src/styles.css\`, "utf8");
if (/bulk|select all|selected/i.test(source)) score.phase2 += 1; else failures.push("phase2 bulk selection not implemented");
if (/export|download|csv/i.test(source)) score.phase2 += 1; else failures.push("phase2 export not implemented");
if (/prefers-reduced-motion|animation/i.test(source)) score.phase2 += 1; else failures.push("phase2 animation/reduced motion missing");

page = await pageFor({ width: 1280, height: 900 });
const body = await page.textContent("body");
if (/bulk|selected|export|csv/i.test(body || "")) score.phase2 += 1; else failures.push("phase2 UI text missing");
await page.close();
await browser.close();

const total = score.visual + score.interaction + score.phase2;
const result = { pass: failures.length === 0, total, max: 16, score, failures };
console.log(JSON.stringify(result, null, 2));
if (!result.pass) process.exit(1);
`
}

function referenceScreenshotHtml(kind) {
  const mobile = kind === "mobile"
  const modal = kind === "modal"
  return `<!doctype html><html><head><meta charset="utf-8"><style>
body{margin:0;font-family:Segoe UI,Arial,sans-serif;background:#f4f7fb;color:#182033}
.wrap{width:${mobile ? 390 : 1440}px;min-height:${mobile ? 844 : 980}px;padding:${mobile ? 16 : 32}px;box-sizing:border-box}
.shell{display:grid;grid-template-columns:${mobile ? "1fr" : "230px 1fr"};gap:24px}
.nav{background:#111827;color:white;border-radius:0;padding:20px;display:${mobile ? "flex" : "block"};gap:14px;height:${mobile ? "76px" : "900px"};box-sizing:border-box;overflow:hidden}.brand{font-size:25px;font-weight:800;margin-right:20px}.nav span{display:inline-block;color:#cbd5e1;margin:${mobile ? "8px 8px 0 0" : "22px 0"}}
.hero{background:linear-gradient(135deg,#0f766e,#2563eb);color:white;border-radius:18px;padding:${mobile ? "22px" : "34px"};display:flex;justify-content:space-between;gap:24px;align-items:flex-end;min-height:${mobile ? "230px" : "210px"};box-shadow:0 18px 40px rgba(15,23,42,.18)}
h1{font-size:${mobile ? "40px" : "54px"};line-height:1;margin:0}.sub{font-size:18px;max-width:560px}.stats{display:grid;grid-template-columns:repeat(3,1fr);gap:10px}.stats div{background:rgba(255,255,255,.18);border-radius:12px;padding:14px;min-width:${mobile ? "76px" : "100px"}}.stats b{font-size:32px;display:block}
.toolbar{margin:18px 0;display:flex;gap:10px;flex-wrap:wrap;background:white;padding:14px;border:1px solid #d7dee8;border-radius:14px}.toolbar input{flex:1;min-width:${mobile ? "100%" : "280px"};padding:12px;border:1px solid #cbd5e1;border-radius:10px}.toolbar button{padding:11px 14px;border:1px solid #cbd5e1;border-radius:10px;background:#fff}.toolbar .primary{background:#f97316;color:white}
.board{display:grid;grid-template-columns:${mobile ? "1fr" : "repeat(3,1fr)"};gap:16px}.card{background:white;border:1px solid #d7dee8;border-left:5px solid #2563eb;border-radius:14px;padding:18px;min-height:170px;box-shadow:0 14px 28px rgba(15,23,42,.07)}.card h2{font-size:22px;margin:18px 0 8px}.actions{display:flex;gap:8px;margin-top:18px}.actions button{flex:1;padding:10px;border-radius:10px;border:1px solid #cbd5e1;background:#f8fafc}
.bars{display:grid;grid-template-columns:${mobile ? "1fr" : "repeat(3,1fr)"};gap:14px;margin-top:18px}.bar{background:white;border-radius:14px;padding:18px}.bar i{display:block;height:16px;border-radius:999px;background:linear-gradient(90deg,#2563eb 70%,#dbeafe 70%)}
.modalBg{position:fixed;inset:0;background:rgba(15,23,42,.35);display:${modal ? "grid" : "none"};place-items:center}.modal{background:white;border-radius:18px;padding:26px;width:420px;box-shadow:0 30px 80px rgba(0,0,0,.3)}.modal input{display:block;width:100%;padding:12px;margin:8px 0 16px;border:1px solid #cbd5e1;border-radius:10px}
</style></head><body><div class="wrap"><div class="shell"><aside class="nav"><div class="brand">Tura Ops</div><span>Overview</span><span>Queues</span><span>Reports</span><span>Settings</span></aside><main><section class="hero"><div><p>RETAIL COMMAND CENTER</p><h1>Daily Operations Board</h1><p class="sub">Track fulfillment incidents, ownership, and release readiness.</p></div><div class="stats"><div><b>5</b>Total</div><div><b>2</b>Active</div><div><b>1</b>Blocked</div></div></section><section class="toolbar"><input placeholder="Search tasks"><button>All</button><button>Active</button><button>Queued</button><button>Blocked</button><button class="primary">New task</button></section><section class="board">${["Sync checkout rates","Audit catalog feed","Repair refund export","Reconcile carrier SLA","Review fraud rules"].map((t,i)=>`<article class="card"><small>${["A-17","B-22","C-09","D-31","E-04"][i]} - ${["Blocked","Queued","Active","Active","Queued"][i]}</small><h2>${t}</h2><p>Owner ${["Mira","Jon","Nia","Pax","Ivo"][i]} - due ${["Today","Tomorrow","Fri","Mon","Wed"][i]}</p><div class="actions"><button>Details</button><button>Complete</button></div></article>`).join("")}</section><section class="bars"><div class="bar">Payments<i></i></div><div class="bar">Inventory<i></i></div><div class="bar">Shipping<i></i></div></section></main></div></div><div class="modalBg"><div class="modal"><h2>Create work item</h2><label>Title<input value="New launch task"></label><label>Owner<input value="Ops"></label><button>Create</button></div></div></body></html>`
}

function createReferenceScreenshots(template) {
  const ref = path.join(template, "reference")
  mkdirp(ref)
  for (const kind of ["desktop", "mobile", "modal"]) {
    writeFile(path.join(ref, `${kind}.html`), referenceScreenshotHtml(kind))
  }
  writeFile(path.join(template, "tools", "make_reference.mjs"), `import { chromium } from "playwright";\nimport path from "node:path";\nconst browser = await chromium.launch({ headless: true });\nfor (const kind of ["desktop","mobile","modal"]) {\n  const page = await browser.newPage({ viewport: kind === "mobile" ? { width: 390, height: 844 } : { width: 1440, height: 980 } });\n  await page.goto("file://" + path.resolve("reference", kind + ".html"));\n  await page.screenshot({ path: path.resolve("reference", kind + ".png"), fullPage: true });\n  await page.close();\n}\nawait browser.close();\n`)
  runOk(npmCmd, ["install"], { cwd: template, timeoutMs: 180_000, shell: process.platform === "win32" })
  runOk(npxCmd, ["playwright", "install", "chromium"], { cwd: template, timeoutMs: 240_000, shell: process.platform === "win32" })
  runOk("node", ["tools/make_reference.mjs"], { cwd: template, timeoutMs: 120_000 })
  fs.rmSync(path.join(template, "tools", "make_reference.mjs"))
  for (const kind of ["desktop", "mobile", "modal"]) fs.rmSync(path.join(ref, `${kind}.html`))
}

function createHiddenEvaluator() {
  const hiddenDir = path.join(runRoot, "hidden")
  mkdirp(hiddenDir)
  writeFile(path.join(hiddenDir, "evaluate.mjs"), hiddenEvaluator())
  return path.join(hiddenDir, "evaluate.mjs")
}

function promptPhase1(port) {
  return `You are in a front-end repair benchmark. The workspace contains a half-finished React page and three reference screenshots in reference/desktop.png, reference/mobile.png, and reference/modal.png. The screenshots show the intended visual quality; the current app has many display and interaction bugs.

Do not edit files under reference/. Do not ask for hidden tests. Run npm install if dependencies are missing. Start the Vite app before Playwright probes, for example npm run start -- --port ${port} --strictPort, then run the Playwright npm scripts with PORT=${port}. Use Playwright from the provided npm scripts to inspect, screenshot, interact with the page, and compare against the reference screenshots. Use your image-reading capability on generated screenshots and the reference screenshots when helpful.

Fix the page so the desktop, mobile, and modal states match the references in layout intent and quality. Also fix obvious interaction problems around filtering, search, create, complete, details, accessibility names, overflow, and responsive behavior. Use the provided scripts and screenshots to verify. Finish with a concise summary and mention the screenshots or probes you ran.`
}

function promptSmoke(port) {
  return `Smoke test only. Do not repair the app. Confirm the local frontend and Playwright path works.

Run npm install if dependencies are missing. Then run PORT=${port} npm run smoke:playwright. Finish by reporting the screenshot path and one observed page detail from that script output.`
}

function promptPhase2(port) {
  return `Follow-up feature task. Keep the visual repairs from phase 1. Add these product features and verify them with Playwright interaction and screenshots:

- Add row selection with individual checkboxes and a select-all control.
- Add a bulk complete action that completes selected tasks and records the action in the audit log.
- Add an export CSV action that produces a downloadable CSV or visible CSV preview containing the current filtered tasks.
- Add a lightweight two-stage animation: cards should enter smoothly on load/filter changes, and the create-task success state should animate without breaking prefers-reduced-motion.
- Preserve keyboard-accessible labels and avoid horizontal overflow on mobile.

You may read and use the visible tools/probe scripts. Start the Vite app before Playwright probes on port ${port}, then use Playwright to demonstrate the new interactions and take updated screenshots.`
}

function parseJsonl(text) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try { return JSON.parse(line) } catch { return null }
    })
    .filter(Boolean)
}

function usageFromEvents(events) {
  const usage = { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 }
  for (const event of events) {
    const u = event.usage || event.message?.usage || event.payload?.info?.last_token_usage
    if (!u) continue
    usage.input += Number(u.input_tokens || u.prompt_tokens || 0)
    usage.cached += Number(u.cached_input_tokens || u.input_tokens_details?.cached_tokens || u.prompt_tokens_details?.cached_tokens || 0)
    usage.cached += Number(u.cache_read_input_tokens || 0)
    usage.output += Number(u.output_tokens || u.completion_tokens || 0)
    usage.reasoning += Number(u.reasoning_output_tokens || u.reasoning_tokens || u.output_tokens_details?.reasoning_tokens || u.completion_tokens_details?.reasoning_tokens || 0)
    usage.total += Number(u.total_tokens || 0)
  }
  return usage
}

function usageFromRuns(agentId, result) {
  const first = usageFromEvents(parseJsonl(result.first.stdout))
  const second = usageFromEvents(parseJsonl(result.second.stdout))
  if (
    agentId.startsWith("tura-") &&
    second.input >= first.input &&
    second.cached >= first.cached &&
    second.output >= first.output &&
    second.reasoning >= first.reasoning
  ) {
    return second
  }
  return {
    input: first.input + second.input,
    cached: first.cached + second.cached,
    output: first.output + second.output,
    reasoning: first.reasoning + second.reasoning,
    total: first.total + second.total,
  }
}

function countEvents(events) {
  let commands = 0
  let failures = 0
  let turns = 0
  for (const event of events) {
    if (event.type === "turn.started") turns += 1
    if (event.type === "system" && event.subtype === "init") turns += 1
    if (Array.isArray(event.message?.content) && event.message.content.some((part) => part?.type === "tool_use")) commands += 1
    if (event.item?.type === "command_execution" && event.item.status === "completed") {
      commands += 1
      if (event.item.exit_code && event.item.exit_code !== 0) failures += 1
    }
  }
  return { turns, commands, failures }
}

function startServer(workspace, port) {
  return spawn(npmCmd, ["run", "start", "--", "--port", String(port), "--strictPort"], {
    cwd: workspace,
    stdio: ["ignore", "pipe", "pipe"],
    env: process.env,
    shell: process.platform === "win32",
    windowsHide: true,
  })
}

function stopServer(child) {
  if (!child || child.killed) return
  try {
    if (process.platform === "win32" && child.pid) {
      spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true })
    } else {
      child.kill("SIGTERM")
    }
  } catch {}
}

async function waitForServer(port) {
  const deadline = Date.now() + 30_000
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}`)
      if (response.ok) return true
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
  return false
}

async function runCurrentLike(agentId, exe, workspace, agentDir, agentPort) {
  const common = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "-C",
    workspace,
    "-m",
    model,
    "--dangerously-bypass-approvals-and-sandbox",
    "-c",
    `model_reasoning_effort="${reasoning}"`,
    "-c",
    `service_tier="priority"`,
  ]
  const first = await runLive(exe, [...common, smokeOnly ? promptSmoke(agentPort) : promptPhase1(agentPort)], {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "phase1.stdout.jsonl"),
    stderrPath: path.join(agentDir, "phase1.stderr.log"),
    statusPath: path.join(agentDir, "phase1.status.json"),
  })
  const threadId = parseJsonl(first.stdout).find((event) => event.type === "thread.started")?.thread_id
  const second = threadId && !smokeOnly
    ? await runLive(
        exe,
        [
          "exec",
          "resume",
          "--json",
          "--skip-git-repo-check",
          "-m",
          model,
          "--dangerously-bypass-approvals-and-sandbox",
          "-c",
          `model_reasoning_effort="${reasoning}"`,
          "-c",
          `service_tier="priority"`,
          threadId,
          promptPhase2(agentPort),
        ],
        {
          cwd: workspace,
          timeoutMs,
          stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
          stderrPath: path.join(agentDir, "phase2.stderr.log"),
          statusPath: path.join(agentDir, "phase2.status.json"),
        },
      )
    : emptyRun(smokeOnly ? "smoke mode skipped phase2" : `${agentId} did not emit thread.started`)
  return { first, second, threadId, error: first.error || second.error || null }
}

async function runTura(workspace, agentDir, agentPort, agentPrompt = "coding_agent_fast") {
  runOk("cargo", ["build", "-p", "gateway", "--bin", "tura"], { cwd: repoRoot, timeoutMs: 240_000 })
  const sessionId = `frontend-${Date.now()}`
  const common = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    sessionId,
    "--agent",
    agentPrompt,
    "-m",
    turaModel,
    "-c",
    `model_reasoning_effort=${reasoning}`,
    "-c",
    "service_tier=priority",
    "--cwd",
    workspace,
  ]
  const env = {
    TURA_COMMAND_RUN_SHELL: "shell_command",
    TURA_COMMAND_RUN_STRICT_JSON: "0",
    COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
  }
  const first = await runLive(turaExe, [...common, smokeOnly ? promptSmoke(agentPort) : promptPhase1(agentPort)], {
    cwd: workspace,
    env,
    timeoutMs,
    stdoutPath: path.join(agentDir, "phase1.stdout.jsonl"),
    stderrPath: path.join(agentDir, "phase1.stderr.log"),
    statusPath: path.join(agentDir, "phase1.status.json"),
  })
  const second = smokeOnly
    ? emptyRun("smoke mode skipped phase2")
    : await runLive(turaExe, [...common, promptPhase2(agentPort)], {
        cwd: workspace,
        env,
        timeoutMs,
        stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
        stderrPath: path.join(agentDir, "phase2.stderr.log"),
        statusPath: path.join(agentDir, "phase2.status.json"),
      })
  return { first, second, threadId: sessionId, error: first.error || second.error || null }
}

async function runClaudeCode(workspace, agentDir, agentPort) {
  const common = [
    "--print",
    "--model",
    claudeModel,
    "--output-format",
    "stream-json",
    "--verbose",
    "--dangerously-skip-permissions",
  ]
  const first = await runLive(claudeExe, [...common, smokeOnly ? promptSmoke(agentPort) : promptPhase1(agentPort)], {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "phase1.stdout.jsonl"),
    stderrPath: path.join(agentDir, "phase1.stderr.log"),
    statusPath: path.join(agentDir, "phase1.status.json"),
  })
  const firstEvents = parseJsonl(first.stdout)
  const threadId = firstEvents.find((event) => event.type === "result")?.session_id
    || firstEvents.find((event) => event.session_id)?.session_id
  const second = threadId && !smokeOnly
    ? await runLive(
        claudeExe,
        [
          "--print",
          "--resume",
          threadId,
          "--model",
          claudeModel,
          "--output-format",
          "stream-json",
          "--verbose",
          "--dangerously-skip-permissions",
          promptPhase2(agentPort),
        ],
        {
          cwd: workspace,
          timeoutMs,
          stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
          stderrPath: path.join(agentDir, "phase2.stderr.log"),
          statusPath: path.join(agentDir, "phase2.status.json"),
        },
      )
    : emptyRun(smokeOnly ? "smoke mode skipped phase2" : "claude-code did not emit session_id")
  return { first, second, threadId, error: first.error || second.error || null }
}

function writeRunLogs(agentDir, result) {
  writeFile(path.join(agentDir, "phase1.stdout.jsonl"), result.first.stdout)
  writeFile(path.join(agentDir, "phase1.stderr.log"), result.first.stderr)
  writeFile(path.join(agentDir, "phase2.stdout.jsonl"), result.second.stdout)
  writeFile(path.join(agentDir, "phase2.stderr.log"), result.second.stderr)
}

async function evaluate(workspace, evaluator, port) {
  const server = startServer(workspace, port)
  try {
    const ready = await waitForServer(port)
    if (!ready) {
      return { pass: false, error: "dev server did not become ready" }
    }
    const result = run("node", [evaluator, workspace, String(port)], { cwd: workspace, timeoutMs: 120_000 })
    try {
      return JSON.parse(result.stdout)
    } catch {
      return { pass: false, status: result.status, stdout: result.stdout, stderr: result.stderr }
    }
  } finally {
    stopServer(server)
  }
}

function evaluateSmoke(workspace) {
  const artifactsDir = path.join(workspace, "artifacts")
  const desktop = path.join(artifactsDir, "desktop.png")
  const hasScreenshot = fs.existsSync(desktop) && fs.statSync(desktop).size > 1_000
  const packageInstalled = fs.existsSync(path.join(workspace, "node_modules", "playwright"))
  return {
    pass: hasScreenshot && packageInstalled,
    has_screenshot: hasScreenshot,
    screenshot: desktop,
    playwright_installed: packageInstalled,
  }
}

async function runAgent(agentId, template, evaluator, index) {
  const agentDir = path.join(runRoot, agentId)
  const workspace = path.join(agentDir, "workspace")
  const agentPort = 4173 + index
  copyDir(template, workspace)
  const started = performance.now()
  let result
  let runError = null
  try {
    if (agentId === "current-shll") {
      result = await runCurrentLike(agentId, codexCurrentExe, workspace, agentDir, agentPort)
    } else if (agentId === "codex-main") {
      result = await runCurrentLike(agentId, codexMainExe, workspace, agentDir, agentPort)
    } else if (agentId === "tura-fast-shll") {
      result = await runTura(workspace, agentDir, agentPort, "coding_agent_fast")
    } else if (agentId === "tura-shll") {
      result = await runTura(workspace, agentDir, agentPort, "coding_agent")
    } else if (agentId === "claude-code") {
      result = await runClaudeCode(workspace, agentDir, agentPort)
    } else {
      throw new Error(`unsupported agent ${agentId}`)
    }
  } catch (error) {
    runError = String(error?.stack || error?.message || error)
    result = { first: emptyRun(runError), second: emptyRun(runError), threadId: null, error: runError }
  }
  writeRunLogs(agentDir, result)
  const events = [...parseJsonl(result.first.stdout), ...parseJsonl(result.second.stdout)]
  const validation = smokeOnly ? evaluateSmoke(workspace) : await evaluate(workspace, evaluator, 43100 + index)
  const stats = {
    id: agentId,
    workspace,
    agent_port: agentPort,
    thread_id: result.threadId,
    elapsed_ms: Math.round(performance.now() - started),
    phase1_ms: result.first.duration_ms,
    phase2_ms: result.second.duration_ms,
    phase1_status: result.first.status,
    phase2_status: result.second.status,
    error: runError || result.error || null,
    usage: usageFromRuns(agentId, result),
    events: countEvents(events),
    validation,
  }
  writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
  return stats
}

async function main() {
  mkdirp(runRoot)
  const template = createFixtureTemplate()
  const evaluator = createHiddenEvaluator()
  createReferenceScreenshots(template)
  if (prepOnly) {
    const summary = {
      ok: true,
      prep_only: true,
      run_id: runId,
      run_root: runRoot,
      template,
      evaluator,
    }
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  assert(fs.existsSync(codexCurrentExe), `missing current exe ${codexCurrentExe}`)
  assert(fs.existsSync(codexMainExe), `missing main exe ${codexMainExe}`)
  const results = await Promise.all(agents.map((agent, index) => {
    console.log(`[frontend-playwright-e2e] running ${agent}`)
    return runAgent(agent, template, evaluator, index)
  }))
  const summary = {
    ok: results.every((result) => result.validation?.pass),
    run_id: runId,
    run_root: runRoot,
    model,
    claude_model: claudeModel,
    reasoning,
    timeout_ms: timeoutMs,
    smoke_only: smokeOnly,
    agents,
    results,
  }
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") {
    process.exitCode = 1
  }
}

await main()
