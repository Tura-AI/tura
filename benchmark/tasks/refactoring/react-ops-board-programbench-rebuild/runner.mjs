#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import http from "node:http"
import { createRequire } from "node:module"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { businessRunPaths, normalizeBusinessSummary } from "../../../lib/business_paths.mjs"
import { endStream, isolatedProcessOptions, killProcessTree } from "../../../lib/process_helpers.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `frontend-playwright-${Date.now()}`
const runPaths = businessRunPaths("frontend-playwright-full", runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const claudeModel = process.env.COMMAND_RUN_AGENT_CLAUDE_MODEL || "opus"
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 20 * 60_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "tura-fast-shll")
const prepOnly = (process.env.COMMAND_RUN_AGENT_PREP_ONLY || "0") === "1"
const smokeOnly = (process.env.COMMAND_RUN_AGENT_SMOKE_ONLY || "0") === "1"
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm"
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx"
const tuiRequire = createRequire(path.join(repoRoot, "apps", "tui", "package.json"))

const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const codexCurrentExe = path.join(
  process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex"),
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
)
const codexMainExe = findCodexMainExe()
const claudeExe = findClaudeExe()
const piExe = findPiExe()

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

function findPiExe() {
  const exeName = process.platform === "win32" ? "pi.cmd" : "pi"
  const candidates = [
    process.env.COMMAND_RUN_AGENT_PI_EXE,
    exeName,
    "pi",
  ].filter(Boolean)
  return candidates.find((candidate) => candidate === exeName || candidate === "pi" || fs.existsSync(candidate)) || candidates[0]
}

function parseAgents(value) {
  const alias = new Map([
    ["current", "current-shll"],
    ["current-shll", "current-shll"],
    ["current-bash", "current-bash"],
    ["codex-current", "current-shll"],
    ["main", "codex-main"],
    ["codex-main", "codex-main"],
    ["codex-main-shll", "codex-main"],
    ["codex-main-bash", "codex-main-bash"],
    ["tura", "tura-fast-shll"],
    ["tura-shll", "tura-shll"],
    ["tura-bash", "tura-bash"],
    ["tura-coding", "tura-shll"],
    ["tura-coding-agent", "tura-shll"],
    ["tura-fast", "tura-fast-shll"],
    ["tura-fast-shll", "tura-fast-shll"],
    ["tura-fast-bash", "tura-fast-bash"],
    ["tui", "tui-fast-shll"],
    ["tui-shll", "tui-shll"],
    ["tui-fast", "tui-fast-shll"],
    ["tui-fast-shll", "tui-fast-shll"],
    ["claude", "claude-code"],
    ["claude-code", "claude-code"],
    ["claude-opus", "claude-code"],
    ["pi", "pi-agent"],
    ["pi-agent", "pi-agent"],
    ["pi-coding-agent", "pi-agent"],
  ])
  const parsed = String(value)
    .split(",")
    .map((item) => alias.get(item.trim().toLowerCase()))
    .filter(Boolean)
  const counts = new Map()
  return parsed.map((agent) => {
    const next = (counts.get(agent) || 0) + 1
    counts.set(agent, next)
    return next === 1 ? agent : `${agent}-${next}`
  })
}

function agentKind(agentId) {
  return String(agentId).replace(/-\d+$/, "")
}

function requireCodexCurrentExe() {
  return agents.some((agent) => ["current-shll", "current-bash"].includes(agentKind(agent)))
}

function requireCodexMainExe() {
  return agents.some((agent) => ["codex-main", "codex-main-bash"].includes(agentKind(agent)))
}

function requirePiExe() {
  return agents.some((agent) => agentKind(agent) === "pi-agent")
}

function shellSurfaceForAgent(agentId) {
  return agentKind(agentId).endsWith("-bash") ? "bash" : "shell_command"
}

function bashBinForHost() {
  if (process.platform !== "win32") return "bash"
  const candidates = [
    "C:\\Program Files\\Git\\bin\\bash.exe",
    "C:\\Program Files\\Git\\usr\\bin\\bash.exe",
    "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
  ]
  return candidates.find((candidate) => fs.existsSync(candidate)) || "bash"
}

function envForShellSurface(shellSurface) {
  if (shellSurface !== "bash" || process.platform !== "win32") return {}
  const bashBin = bashBinForHost()
  const bashDir = path.dirname(bashBin)
  return fs.existsSync(bashBin)
    ? { PATH: `${bashDir}${path.delimiter}${process.env.PATH || ""}` }
    : {}
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

function serviceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return ["-c", `service_tier="${tier}"`]
}

function turaServiceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return tier === "priority" ? ["-p"] : []
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
  let settled = false
  let childExitStatus = null
  let childExitSignal = null

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
    const timeoutLimitMs = options.timeoutMs || timeoutMs
    let closeGraceTimer = null
    let timeoutGraceTimer = null

    function settle(statusLabel, status, signal, error) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      clearTimeout(closeGraceTimer)
      clearTimeout(timeoutGraceTimer)
      endStream(stdoutStream)
      endStream(stderrStream)
      const result = {
        command,
        args,
        status,
        signal,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        error,
      }
      writeStatus({ status: statusLabel, result })
      resolve(result)
    }

    const child = spawn(command, args, isolatedProcessOptions({
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["ignore", "pipe", "pipe"],
      shell: options.shell || false,
      windowsHide: true,
    }))

    const timer = setTimeout(() => {
      timedOut = true
      writeStatus({ status: "timeout_killing", stdout_bytes: Buffer.byteLength(stdout), stderr_bytes: Buffer.byteLength(stderr) })
      try {
        killProcessTree(child.pid)
      } catch {}
      timeoutGraceTimer = setTimeout(() => {
        settle("timeout", childExitStatus ?? 1, childExitSignal, `timed out after ${timeoutLimitMs}ms`)
      }, Number(options.timeoutCloseGraceMs || 3_000))
    }, timeoutLimitMs)

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
      settle("error", null, null, String(error.stack || error.message || error))
    })

    child.on("exit", (status, signal) => {
      childExitStatus = status
      childExitSignal = signal
      closeGraceTimer = setTimeout(() => {
        settle(
          timedOut ? "timeout" : "closed",
          timedOut ? (status ?? 1) : status,
          signal,
          timedOut ? `timed out after ${timeoutLimitMs}ms` : null,
        )
      }, Number(options.exitCloseGraceMs || 1_000))
    })

    child.on("close", (status, signal) => {
      settle(
        timedOut ? "timeout" : "closed",
        timedOut ? (status ?? 1) : status,
        signal,
        timedOut ? `timed out after ${timeoutLimitMs}ms` : null,
      )
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
      "verify:all": "node tools/with_vite.mjs -- npm run verify:all:inner",
      "verify:all:inner": "npm run screenshot:desktop && npm run screenshot:mobile && npm run screenshot:modal && npm run probe:flow && npm run check:a11y && npm run smoke:playwright",
      "smoke:playwright": "node tools/playwright_smoke.mjs",
      "verify:programbench": "node tools/programbench_verify.mjs"
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
  writeFile(path.join(template, "tools", "with_vite.mjs"), withViteScript())
  writeFile(path.join(template, "tools", "playwright_smoke.mjs"), playwrightSmokeScript())
  writeFile(path.join(template, "tools", "programbench_verify.mjs"), programbenchVerifyScript())
  writeFile(path.join(template, "REFERENCE_NOTES.md"), referenceNotes())
  createProgramBenchMini(template)
  return template
}

function createProgramBenchMini(template) {
  const root = path.join(template, "programbench-mini")
  writeFile(path.join(root, "Cargo.toml"), [
    "[package]",
    'name = "pb-rebuild"',
    'version = "0.1.0"',
    'edition = "2021"',
    "",
    "[profile.release]",
    "lto = false",
    "codegen-units = 1",
    "",
    "[workspace]",
  ].join("\n"))
  writeFile(path.join(root, "SPEC.md"), [
    "# ProgramBench Mini Reconstruction Spec",
    "",
    "This fixture is based on the real ProgramBench sample instance `testorg__calculator.abc1234`: rebuild a small program from documentation, tests, and expected black-box behavior.",
    "",
    "## Required CLI",
    "",
    "- The executable name must be `pb-rebuild`.",
    "- `pb-rebuild --self-check` must print `PB_REBUILD_SELF_CHECK ok`.",
    "- `pb-rebuild 2 + 3` must print `5`; `pb-rebuild 10 - 3` must print `7`; `pb-rebuild 4 * 3` must print `12`.",
    "- `pb-rebuild --manifest benches/programbench-mini.manifest --out artifacts/cli-report.md` must parse all `case:<id>=<description>` lines.",
    "- It must fail if fewer than four cases are present.",
    "- The generated report must contain `# ProgramBench Mini Report`, `facebookresearch/programbench`, `testorg__calculator.abc1234`, each case id, and the phrase `step-2 barrier`.",
    "- Package reconstructed source as `programbench-run/testorg__calculator.abc1234/submission.tar.gz`.",
    "- Write `programbench-run/testorg__calculator.abc1234/testorg__calculator.abc1234.eval.json` with passed addition/subtraction/multiplication test results for branch `33128f6b8600`.",
    "",
    "## Required Docs",
    "",
    "- Write `docs/REBUILD.md` with build/test/run instructions and the release executable path.",
    "- Write `docs/ARCHITECTURE.md` describing derived tasks: fixture, cli, docs, verify.",
    "- The docs must mention ordered barriers and command_run queue/file-lock reuse.",
  ].join("\n"))
  writeFile(path.join(root, "benches", "programbench-mini.manifest"), [
    "instance=testorg__calculator.abc1234",
    "repository=testorg/calculator",
    "commit=abc1234567890abcdef1234567890abcdef123456",
    "source=facebookresearch/programbench",
    "branch=33128f6b8600",
    "case:addition=./executable 2 + 3 prints 5",
    "case:subtraction=./executable 10 - 3 prints 7",
    "case:multiplication=./executable 4 * 3 prints 12",
    "case:submission=Package reconstructed source as submission.tar.gz",
  ].join("\n"))
  writeFile(path.join(root, "src", "main.rs"), [
    "fn main() {",
    "    // TODO: rebuild the CLI described in SPEC.md.",
    "    // Hidden tests expect --self-check, --manifest, --out, docs, and report generation.",
    "    println!(\"PB_REBUILD_TODO\");",
    "}",
  ].join("\n"))
  writeFile(path.join(root, "docs", ".gitkeep"), "")
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

Repair the task board so the desktop, mobile, and modal experiences match the reference intent and behave like a polished operations tool. Keep the desktop hero as a medium-height band, keep the mobile navigation compact, keep the mobile heading modest, and avoid horizontal overflow. Complete the ProgramBench mini rebuild artifacts as part of the same workspace task.
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
const browser = await chromium.launch({ headless: false });
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

function withViteScript() {
  return `import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const separator = process.argv.indexOf("--");
const commandArgs = separator >= 0 ? process.argv.slice(separator + 1) : [];
if (commandArgs.length === 0) {
  console.error("Usage: node tools/with_vite.mjs [port] -- <command...>");
  process.exit(2);
}

let port = Number(process.env.PORT || 4173);
if (separator > 2 && /^\\d+$/.test(process.argv[2] || "")) {
  port = Number(process.argv[2]);
}

const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const outDir = fileURLToPath(new URL("../artifacts/", import.meta.url));
fs.mkdirSync(outDir, { recursive: true });
const serverOut = path.join(outDir, "vite.stdout.log");
const serverErr = path.join(outDir, "vite.stderr.log");

function tail(file) {
  try {
    return fs.existsSync(file) ? fs.readFileSync(file, "utf8").slice(-4000) : "";
  } catch {
    return "";
  }
}

function killProcessTree(pid) {
  if (!pid) return;
  if (process.platform === "win32") {
    spawn("taskkill", ["/pid", String(pid), "/t", "/f"], { windowsHide: true, stdio: "ignore" });
  } else {
    try { process.kill(-pid, "SIGTERM"); } catch { try { process.kill(pid, "SIGTERM"); } catch {} }
  }
}

function startServer() {
  return spawn(npmCmd, ["run", "start", "--", "--port", String(port), "--strictPort"], {
    cwd: process.cwd(),
    stdio: ["ignore", fs.openSync(serverOut, "w"), fs.openSync(serverErr, "w")],
    windowsHide: true,
    shell: process.platform === "win32",
    detached: process.platform !== "win32",
  });
}

async function waitForReady(child) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) {
      const code = child.exitCode ?? child.signalCode ?? 1;
      console.error(\`Background service exited before readiness. PID=\${child.pid} ExitCode=\${code}\\nStderr tail:\\n\${tail(serverErr)}\\nStdout tail:\\n\${tail(serverOut)}\`);
      killProcessTree(child.pid);
      process.exit(typeof code === "number" ? code || 1 : 1);
    }
    try {
      const response = await fetch(\`http://127.0.0.1:\${port}\`);
      if (response.ok) return;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  console.error(\`Background service did not become ready before timeout. PID=\${child.pid}\\nStderr tail:\\n\${tail(serverErr)}\\nStdout tail:\\n\${tail(serverOut)}\`);
  killProcessTree(child.pid);
  process.exit(1);
}

function runCommand() {
  const child = spawn(commandArgs[0], commandArgs.slice(1), {
    cwd: process.cwd(),
    env: { ...process.env, PORT: String(port) },
    stdio: "inherit",
    shell: process.platform === "win32",
    windowsHide: true,
  });
  return new Promise((resolve) => {
    child.on("exit", (code, signal) => resolve({ code, signal }));
    child.on("error", (error) => {
      console.error(error.message);
      resolve({ code: 1, signal: null });
    });
  });
}

const server = startServer();
server.on("error", (error) => {
  console.error(\`Failed to start background service: \${error.message}\`);
  process.exit(1);
});

try {
  await waitForReady(server);
  const result = await runCommand();
  process.exitCode = result.code ?? (result.signal ? 1 : 0);
} finally {
  killProcessTree(server.pid);
}
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
const serverOut = path.join(outDir, "vite.stdout.log");
const serverErr = path.join(outDir, "vite.stderr.log");

function killProcessTree(pid) {
  if (!pid) return;
  if (process.platform === "win32") {
    spawn("taskkill", ["/pid", String(pid), "/t", "/f"], { windowsHide: true });
  } else {
    try { process.kill(-pid, "SIGTERM"); } catch { try { process.kill(pid, "SIGTERM"); } catch {} }
  }
}

function startServer() {
  return spawn(npmCmd, ["run", "start", "--", "--port", String(port), "--strictPort"], {
    cwd: process.cwd(),
    stdio: ["ignore", fs.openSync(serverOut, "w"), fs.openSync(serverErr, "w")],
    windowsHide: true,
    shell: process.platform === "win32",
    detached: process.platform !== "win32",
  });
}

async function waitForServer(child) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) {
      const tail = fs.existsSync(serverErr) ? fs.readFileSync(serverErr, "utf8").slice(-4000) : "";
      throw new Error(\`Vite exited before readiness with code \${child.exitCode}; stderr tail:\\n\${tail}\`);
    }
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
    killProcessTree(child.pid);
  } catch {}
}

const server = startServer();
try {
  await waitForServer(server);
  const browser = await chromium.launch({ headless: false });
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
const browser = await chromium.launch({ headless: false });
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
const browser = await chromium.launch({ headless: false });
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
const browser = await chromium.launch({ headless: false });
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

function programbenchVerifyScript() {
  return `import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve("programbench-mini");
const exeName = process.platform === "win32" ? "pb-rebuild.exe" : "pb-rebuild";
const exePath = path.join(root, "target", "release", exeName);

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || root,
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 120_000,
    maxBuffer: 64 * 1024 * 1024,
    shell: process.platform === "win32",
    windowsHide: true,
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.status !== 0) {
    throw new Error(command + " " + args.join(" ") + " failed with " + result.status);
  }
  return result;
}

run("cargo", ["test"], { timeoutMs: 180_000 });
run("cargo", ["build", "--release"], { timeoutMs: 240_000 });
const selfCheck = run(exePath, ["--self-check"], { cwd: root, timeoutMs: 30_000 });
if (!selfCheck.stdout.includes("PB_REBUILD_SELF_CHECK ok")) {
  throw new Error("--self-check did not print expected marker");
}
for (const [a, op, b, expected] of [["2", "+", "3", "5"], ["10", "-", "3", "7"], ["4", "*", "3", "12"]]) {
  const calc = run(exePath, [a, op, b], { cwd: root, timeoutMs: 30_000 });
  if (calc.stdout.trim() !== expected) throw new Error("calculator behavior mismatch for " + [a, op, b].join(" "));
}
const reportPath = path.join(root, "artifacts", "cli-report.md");
const cli = run(exePath, ["--manifest", "benches/programbench-mini.manifest", "--out", "artifacts/cli-report.md"], { cwd: root, timeoutMs: 30_000 });
if (!cli.stdout.includes("PB_REBUILD_OK")) throw new Error("CLI did not print PB_REBUILD_OK");
const report = fs.readFileSync(reportPath, "utf8");
for (const expected of ["ProgramBench Mini Report", "facebookresearch/programbench", "testorg__calculator.abc1234", "addition", "subtraction", "multiplication", "submission", "step-2 barrier"]) {
  if (!report.includes(expected)) throw new Error("report missing " + expected);
}
const runDir = path.join(root, "programbench-run", "testorg__calculator.abc1234");
if (!fs.existsSync(path.join(runDir, "submission.tar.gz"))) throw new Error("missing ProgramBench submission.tar.gz");
const evalPath = path.join(runDir, "testorg__calculator.abc1234.eval.json");
const evalJson = JSON.parse(fs.readFileSync(evalPath, "utf8"));
if (evalJson.error_code !== null || !evalJson.test_results?.every((test) => test.status === "passed")) {
  throw new Error("eval JSON does not show all calculator tests passed");
}
for (const file of ["docs/REBUILD.md", "docs/ARCHITECTURE.md"]) {
  const content = fs.readFileSync(path.join(root, file), "utf8");
  if (!/ProgramBench|benchmark/i.test(content) || !/command_run|file-lock|file lock|queue/i.test(content)) {
    throw new Error(file + " missing ProgramBench/command_run architecture details");
  }
}
console.log(JSON.stringify({ ok: true, exe: exePath, report: reportPath }, null, 2));
`
}

function hiddenEvaluator() {
  return `import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
const workspace = process.argv[2];
const port = Number(process.argv[3]);
const require = createRequire(workspace + "/package.json");
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: false });
const failures = [];
const score = { visual: 0, interaction: 0, phase2: 0, programbench: 0 };

function run(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: options.cwd || workspace,
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 120_000,
    maxBuffer: 64 * 1024 * 1024,
    shell: process.platform === "win32",
    windowsHide: true,
  });
}

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
  h1Size: document.querySelector("h1") ? parseFloat(getComputedStyle(document.querySelector("h1")).fontSize) : 0,
}));
if (!mobile.overflow) score.visual += 1; else failures.push("mobile horizontal overflow");
if (mobile.navHeight <= 130) score.visual += 1; else failures.push("mobile nav too tall");
if (mobile.h1Size > 0 && mobile.h1Size <= 46) score.visual += 1; else failures.push(mobile.h1Size > 0 ? "mobile heading too large" : "mobile heading missing");
await page.close();

page = await pageFor({ width: 1280, height: 900 });
try {
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
} catch (error) {
  failures.push("interaction probe failed: " + String(error?.message || error).split("\\n")[0]);
}
await page.close();

const source = fs.readFileSync(\`\${workspace}/src/App.jsx\`, "utf8") + "\\n" + fs.readFileSync(\`\${workspace}/src/styles.css\`, "utf8");
if (/bulk|select all|selected/i.test(source)) score.phase2 += 1; else failures.push("phase2 bulk selection not implemented");
if (/export|download|csv/i.test(source)) score.phase2 += 1; else failures.push("phase2 export not implemented");
if (/prefers-reduced-motion|animation/i.test(source)) score.phase2 += 1; else failures.push("phase2 animation/reduced motion missing");

page = await pageFor({ width: 1280, height: 900 });
const body = await page.textContent("body");
if (/bulk|selected|export|csv/i.test(body || "")) score.phase2 += 1; else failures.push("phase2 UI text missing");
await page.close();

const pbRoot = path.join(workspace, "programbench-mini");
const exeName = process.platform === "win32" ? "pb-rebuild.exe" : "pb-rebuild";
const exePath = path.join(pbRoot, "target", "release", exeName);
const cargoTest = run("cargo", ["test"], { cwd: pbRoot, timeoutMs: 180_000 });
if (cargoTest.status === 0) score.programbench += 1; else failures.push("programbench cargo test failed: " + cargoTest.stderr.slice(-800));
const cargoBuild = run("cargo", ["build", "--release"], { cwd: pbRoot, timeoutMs: 240_000 });
if (cargoBuild.status === 0 && fs.existsSync(exePath) && fs.statSync(exePath).size > 1000) score.programbench += 1; else failures.push("programbench release exe missing");
const selfCheck = run(exePath, ["--self-check"], { cwd: pbRoot, timeoutMs: 30_000 });
if (selfCheck.status === 0 && /PB_REBUILD_SELF_CHECK ok/.test(selfCheck.stdout)) score.programbench += 1; else failures.push("programbench self-check failed");
let calculatorOk = true;
for (const [a, op, b, expected] of [["2", "+", "3", "5"], ["10", "-", "3", "7"], ["4", "*", "3", "12"]]) {
  const calc = run(exePath, [a, op, b], { cwd: pbRoot, timeoutMs: 30_000 });
  calculatorOk &&= calc.status === 0 && calc.stdout.trim() === expected;
}
if (calculatorOk) score.programbench += 1; else failures.push("programbench calculator behavior failed");
const cliRun = run(exePath, ["--manifest", "benches/programbench-mini.manifest", "--out", "artifacts/cli-report.md"], { cwd: pbRoot, timeoutMs: 30_000 });
const reportPath = path.join(pbRoot, "artifacts", "cli-report.md");
const report = fs.existsSync(reportPath) ? fs.readFileSync(reportPath, "utf8") : "";
if (cliRun.status === 0 && /PB_REBUILD_OK/.test(cliRun.stdout) && /ProgramBench Mini Report/.test(report) && /testorg__calculator\.abc1234/.test(report) && /step-2 barrier/.test(report)) score.programbench += 1; else failures.push("programbench report generation failed");
const runDir = path.join(pbRoot, "programbench-run", "testorg__calculator.abc1234");
if (fs.existsSync(path.join(runDir, "submission.tar.gz"))) score.programbench += 1; else failures.push("programbench submission.tar.gz missing");
const evalPath = path.join(runDir, "testorg__calculator.abc1234.eval.json");
try {
  const evalJson = JSON.parse(fs.readFileSync(evalPath, "utf8"));
  if (evalJson.error_code === null && evalJson.test_results?.every((test) => test.status === "passed")) score.programbench += 1;
  else failures.push("programbench eval json not all passed");
} catch {
  failures.push("programbench eval json missing or invalid");
}
for (const file of ["docs/REBUILD.md", "docs/ARCHITECTURE.md"]) {
  const docPath = path.join(pbRoot, file);
  const content = fs.existsSync(docPath) ? fs.readFileSync(docPath, "utf8") : "";
  if (/ProgramBench|benchmark/i.test(content) && /command_run|file-lock|file lock|queue/i.test(content)) score.programbench += 1;
  else failures.push("programbench doc incomplete: " + file);
}
await browser.close();

const total = score.visual + score.interaction + score.phase2;
const result = { pass: failures.length === 0, total: total + score.programbench, max: 25, score, failures };
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
  writeFile(path.join(template, "tools", "make_reference.mjs"), `import { chromium } from "playwright";\nimport path from "node:path";\nconst browser = await chromium.launch({ headless: false });\nfor (const kind of ["desktop","mobile","modal"]) {\n  const page = await browser.newPage({ viewport: kind === "mobile" ? { width: 390, height: 844 } : { width: 1440, height: 980 } });\n  await page.goto("file://" + path.resolve("reference", kind + ".html"));\n  await page.screenshot({ path: path.resolve("reference", kind + ".png"), fullPage: true });\n  await page.close();\n}\nawait browser.close();\n`)
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

function promptPhase1(port, shellSurface = "shell_command") {
  return `Repair the React task board in this current workspace only so it matches the provided desktop, mobile, and modal references in visual quality and behavior. Do not read from, copy from, diff against, or inspect any other benchmark run directory, sibling agent workspace, old target artifact, previous solution, or path outside this workspace; all implementation work must be based only on the files and reference assets already present under the current working directory. Fix filtering, search, task creation, completion, details, accessibility names, overflow, and responsive layout. The create dialog must have an accessible name like "Create task" or "Create work item", its submit button must be named "Create", and the success status must include "Task added". Keep the desktop hero between 150px and 340px tall, the mobile navigation no taller than 130px, the mobile heading no larger than 46px, and the page free of horizontal overflow. Also complete the programbench-mini rebuild: implement the Rust calculator CLI, produce the required report, docs, submission archive, and eval JSON artifacts.`
}

function promptSmoke(port, shellSurface = "shell_command") {
  return `Smoke test only. Confirm the local frontend task board can run and report one observed page detail.`
}

function promptPhase2(port, shellSurface = "shell_command") {
  return `Extend the repaired task board in this current workspace only with row selection, select all, bulk complete with audit logging, CSV export or preview for the filtered tasks, smooth card/create-task animation with reduced-motion support, keyboard-accessible labels, and no mobile horizontal overflow. Do not read from, copy from, diff against, or inspect any other benchmark run directory, sibling agent workspace, old target artifact, previous solution, or path outside this workspace; all implementation work must be based only on the files and reference assets already present under the current working directory. Preserve the desktop hero height, compact mobile navigation, and modest mobile heading from the repaired layout. Preserve the completed programbench-mini executable, docs, report, submission archive, and eval JSON artifacts.`
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

function addUsage(...items) {
  const usage = { input: 0, cached: 0, output: 0, reasoning: 0, total: 0 }
  for (const item of items) {
    usage.input += Number(item?.input || 0)
    usage.cached += Number(item?.cached || 0)
    usage.output += Number(item?.output || 0)
    usage.reasoning += Number(item?.reasoning || 0)
    usage.total += Number(item?.total || 0)
  }
  return usage
}

function sameUsage(left, right) {
  return ["input", "cached", "output", "reasoning", "total"].every((key) => Number(left?.[key] || 0) === Number(right?.[key] || 0))
}

function usageFromRuns(agentId, result) {
  const first = usageFromEvents(parseJsonl(result.first.stdout))
  const second = usageFromEvents(parseJsonl(result.second.stdout))
  return addUsage(first, second)
}

function usageDiagnostics(result) {
  const phase1 = usageFromEvents(parseJsonl(result.first.stdout))
  const phase2 = usageFromEvents(parseJsonl(result.second.stdout))
  const duplicate = sameUsage(phase1, phase2) && phase1.total > 0
  return {
    phase1,
    phase2,
    summed: addUsage(phase1, phase2),
    phase2_equals_phase1: duplicate,
    warning: duplicate ? "phase2 usage exactly matches phase1; upstream usage event may be stale or cumulative" : null,
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
  const child = spawn(npmCmd, ["run", "start", "--", "--port", String(port), "--strictPort"], isolatedProcessOptions({
    cwd: workspace,
    stdio: ["ignore", "ignore", "ignore"],
    env: process.env,
    shell: process.platform === "win32",
    windowsHide: true,
  }))
  child.once("error", () => {})
  return child
}

function stopServer(child) {
  if (!child || child.killed) return
  try {
    killProcessTree(child.pid)
  } catch {}
}

async function waitForServer(port, child = null) {
  const deadline = Date.now() + 30_000
  while (Date.now() < deadline) {
    if (child?.exitCode !== null || child?.signalCode !== null) return false
    try {
      const response = await fetch(`http://127.0.0.1:${port}`)
      if (response.ok) return true
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
  return false
}

async function runCurrentLike(agentId, exe, workspace, agentDir, agentPort, shellSurface = "shell_command") {
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
    ...serviceTierConfigArgs(),
  ]
  const env = envForShellSurface(shellSurface)
  const first = await runLive(exe, [...common, smokeOnly ? promptSmoke(agentPort, shellSurface) : promptPhase1(agentPort, shellSurface)], {
    cwd: workspace,
    env,
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
          ...serviceTierConfigArgs(),
          threadId,
          promptPhase2(agentPort, shellSurface),
        ],
        {
          cwd: workspace,
          env,
          timeoutMs,
          stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
          stderrPath: path.join(agentDir, "phase2.stderr.log"),
          statusPath: path.join(agentDir, "phase2.status.json"),
        },
      )
    : emptyRun(smokeOnly ? "smoke mode skipped phase2" : `${agentId} did not emit thread.started`)
  return { first, second, threadId, error: first.error || second.error || null }
}

async function runTura(workspace, agentDir, agentPort, agentPrompt = "fast", shellSurface = "shell_command") {
  runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_exec"], { cwd: repoRoot, timeoutMs: 240_000 })
  const sessionId = `frontend-${Date.now()}`
  const common = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    sessionId,
    "--agent-id",
    agentPrompt,
    "-m",
    turaModel,
    ...turaServiceTierConfigArgs(),
    "--model-reasoning-effort",
    reasoning,
    "--cwd",
    workspace,
  ]
  const env = {
    OPENAI_LOGIN: process.env.OPENAI_LOGIN || "oauth",
    TURA_ENV_PATH: process.env.TURA_ENV_PATH || path.join(repoRoot, ".env"),
    TURA_COMMAND_RUN_SHELL: shellSurface,
    TURA_COMMAND_RUN_STRICT_JSON: "0",
    COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    ...envForShellSurface(shellSurface),
  }
  const first = await runLive(turaExe, [...common, smokeOnly ? promptSmoke(agentPort, shellSurface) : promptPhase1(agentPort, shellSurface)], {
    cwd: workspace,
    env,
    timeoutMs,
    stdoutPath: path.join(agentDir, "phase1.stdout.jsonl"),
    stderrPath: path.join(agentDir, "phase1.stderr.log"),
    statusPath: path.join(agentDir, "phase1.status.json"),
  })
  const second = smokeOnly
    ? emptyRun("smoke mode skipped phase2")
    : await runLive(turaExe, [...common, promptPhase2(agentPort, shellSurface)], {
        cwd: workspace,
        env,
        timeoutMs,
        stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
        stderrPath: path.join(agentDir, "phase2.stderr.log"),
        statusPath: path.join(agentDir, "phase2.status.json"),
      })
  return { first, second, threadId: sessionId, error: first.error || second.error || null }
}

function realTuiBridgeHtml() {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Real TUI bridge</title>
  <style>
    * { box-sizing: border-box; }
    body { margin: 0; background: #0b0d10; color: #f5f2e8; font: 14px/1.35 ui-monospace, SFMono-Regular, Consolas, monospace; overflow: hidden; }
    main { height: 100vh; display: grid; grid-template-rows: auto minmax(0, 1fr) auto; gap: 10px; padding: 14px; }
    h1 { margin: 0; font: 700 18px/1.2 system-ui, sans-serif; letter-spacing: 0; }
    .terminal { margin: 0; white-space: pre-wrap; overflow: hidden; border: 1px solid #303743; border-radius: 8px; background: #050608; padding: 14px; color: #f7f4ec; }
    form { display: grid; grid-template-columns: 1fr auto; gap: 8px; }
    textarea { min-height: 86px; resize: vertical; border: 1px solid #48515d; border-radius: 6px; background: #171b22; color: inherit; padding: 10px; font: inherit; }
    button { min-height: 38px; border: 1px solid #d7cda9; border-radius: 6px; background: #d7cda9; color: #15120b; padding: 0 18px; font-weight: 700; }
    button:disabled { opacity: .55; }
    .status { color: #9bd4ff; }
  </style>
</head>
<body>
  <main>
    <h1>Real apps/tui terminal <span id="status" class="status" role="status">booting</span></h1>
    <pre id="screen" class="terminal" aria-label="TUI screen"></pre>
    <form id="form">
      <textarea id="input" aria-label="TUI input" spellcheck="false"></textarea>
      <button id="send" type="submit">Send to TUI</button>
    </form>
  </main>
  <script>
    const screen = document.querySelector("#screen");
    const input = document.querySelector("#input");
    const form = document.querySelector("#form");
    const status = document.querySelector("#status");
    const send = document.querySelector("#send");
    async function refresh() {
      try {
        const response = await fetch("/screen");
        const result = await response.json();
        screen.textContent = result.screen || "";
        status.textContent = result.alive ? "running" : "closed";
        screen.scrollTop = 0;
      } catch {}
    }
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      send.disabled = true;
      await fetch("/input", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ text: input.value, submit: true, slow: true }),
      });
      input.value = "";
      await refresh();
      send.disabled = false;
    });
    setInterval(refresh, 500);
    refresh();
  </script>
</body>
</html>`
}

function stripAnsi(text) {
  return String(text)
    .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/\x1b\][^\x07]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b[()][A-Za-z0-9]/g, "")
}

function latestTuiScreen(raw) {
  const marker = "\x1b[2J\x1b[H"
  const index = raw.lastIndexOf(marker)
  const current = index >= 0 ? raw.slice(index + marker.length) : raw
  const text = stripAnsi(current).replace(/\r/g, "")
  const lines = text.split("\n")
  return lines.slice(Math.max(0, lines.length - 38)).join("\n")
}

function startGatewayProcess(port, agentDir, env = {}) {
  const stdoutPath = path.join(agentDir, "gateway.stdout.log")
  const stderrPath = path.join(agentDir, "gateway.stderr.log")
  const gatewayExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway")
  mkdirp(agentDir)
  const child = spawn(gatewayExe, [], isolatedProcessOptions({
    cwd: repoRoot,
    env: { ...process.env, PORT: String(port), ...env },
    stdio: ["ignore", fs.openSync(stdoutPath, "w"), fs.openSync(stderrPath, "w")],
    windowsHide: true,
    shell: false,
  }))
  child.once("error", () => {})
  return { child, stdoutPath, stderrPath }
}

async function waitForGateway(port, child) {
  const deadline = Date.now() + 120_000
  while (Date.now() < deadline) {
    if (child?.exitCode !== null || child?.signalCode !== null) return false
    try {
      const response = await fetch(`http://127.0.0.1:${port}/global/health`)
      if (response.ok) return true
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
  return false
}

function startRealTuiBridge(workspace, gatewayUrl, agentDir, env) {
  const pty = tuiRequire("node-pty")
  const tuiBin = path.join(repoRoot, "apps", "tui", "dist", "index.js")
  let raw = ""
  let alive = true
  const term = pty.spawn(process.execPath, [tuiBin, "--gateway-url", gatewayUrl, "--cwd", workspace, "--color", "always"], {
    name: "xterm-256color",
    cols: 120,
    rows: 38,
    cwd: repoRoot,
    env: { ...process.env, ...env },
  })
  term.onData((data) => {
    raw += data
    if (raw.length > 2_000_000) raw = raw.slice(-1_000_000)
    writeFile(path.join(agentDir, "tui.raw.log"), raw)
    writeFile(path.join(agentDir, "tui.screen.txt"), latestTuiScreen(raw))
  })
  term.onExit(() => {
    alive = false
  })
  const writePty = async (text, slow = false) => {
    if (!slow) {
      term.write(text)
      return
    }
    for (const char of text) {
      term.write(char)
      await new Promise((resolve) => setTimeout(resolve, 1))
    }
  }
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1")
    if (req.method === "GET" && url.pathname === "/") {
      const body = realTuiBridgeHtml()
      res.writeHead(200, { "content-type": "text/html; charset=utf-8", "content-length": Buffer.byteLength(body) })
      res.end(body)
      return
    }
    if (req.method === "GET" && url.pathname === "/screen") {
      res.writeHead(200, { "content-type": "application/json" })
      res.end(JSON.stringify({ screen: latestTuiScreen(raw), alive }))
      return
    }
    if (req.method === "POST" && url.pathname === "/input") {
      try {
        const payload = await new Promise((resolve) => {
          let body = ""
          req.on("data", (chunk) => { body += chunk.toString() })
          req.on("end", () => resolve(body.trim() ? JSON.parse(body) : {}))
        })
        await writePty(String(payload.text || ""), Boolean(payload.slow))
        if (payload.submit) {
          await new Promise((resolve) => setTimeout(resolve, 100))
          term.write("\r")
        }
        res.writeHead(200, { "content-type": "application/json" })
        res.end(JSON.stringify({ ok: true }))
      } catch (error) {
        res.writeHead(500, { "content-type": "application/json" })
        res.end(JSON.stringify({ ok: false, error: String(error?.stack || error?.message || error) }))
      }
      return
    }
    res.writeHead(404, { "content-type": "application/json" })
    res.end(JSON.stringify({ error: `unhandled ${req.method} ${url.pathname}` }))
  })
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      resolve({ server, term, isAlive: () => alive, url: `http://127.0.0.1:${address.port}` })
    })
  })
}

async function sendRealTuiInput(page, text, screenshotPath) {
  mkdirp(path.dirname(screenshotPath))
  const ext = path.extname(screenshotPath)
  const stem = screenshotPath.slice(0, -ext.length)
  const typed = String(text).replace(/\r?\n/g, " ")
  await page.screenshot({ path: `${stem}-01-before-input${ext}` })
  await page.getByLabel("TUI input").fill(typed)
  await page.screenshot({ path: `${stem}-02-after-fill${ext}` })
  await page.evaluate(async (value) => {
    const response = await fetch("/input", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ text: value, submit: true, slow: true }),
    })
    if (!response.ok) throw new Error(await response.text())
    const input = document.querySelector("#input")
    if (input) input.value = ""
  }, typed)
  await page.waitForTimeout(800)
  await page.screenshot({ path: `${stem}-03-after-send${ext}` })
}

async function screenshotTui(page, screenshotPath) {
  await page.getByLabel("TUI screen").screenshot({ path: screenshotPath }).catch(async () => {
    await page.screenshot({ path: screenshotPath })
  })
}

async function fetchJson(url) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url} returned ${response.status}`)
  return response.json()
}

async function latestGatewaySession(gatewayUrl) {
  const sessions = await fetchJson(`${gatewayUrl}/session`)
  sessions.sort((left, right) => Number(right.updated_at || right.created_at || 0) - Number(left.updated_at || left.created_at || 0))
  const session = sessions[0]
  const messages = session ? await fetchJson(`${gatewayUrl}/session/${encodeURIComponent(session.id)}/message`).catch(() => []) : []
  return { session, messages }
}

async function gatewaySessionStatus(gatewayUrl, sessionID) {
  const statuses = await fetchJson(`${gatewayUrl}/session/status`).catch(() => ({}))
  const value = statuses?.[sessionID]
  if (typeof value === "string") return value
  return value?.status?.type || value?.status || "idle"
}

async function waitForTuiGatewayCompletion(page, screenshotPath, gatewayUrl, sessionID, initialMessageCount) {
  const started = performance.now()
  await page.waitForTimeout(2500)
  await screenshotTui(page, screenshotPath.replace(/\.png$/, "-04-running.png"))
  const deadline = Date.now() + timeoutMs + 30_000
  while (Date.now() < deadline) {
    const [messages, status] = await Promise.all([
      fetchJson(`${gatewayUrl}/session/${encodeURIComponent(sessionID)}/message`).catch(() => []),
      gatewaySessionStatus(gatewayUrl, sessionID),
    ])
    const newMessages = messages.slice(initialMessageCount)
    const hasUser = newMessages.some((message) => message.role === "user")
    const hasAssistant = newMessages.some((message) => message.role === "assistant")
    if (hasUser && hasAssistant && status !== "busy") {
      await page.waitForTimeout(800)
      await page.screenshot({ path: screenshotPath.replace(/\.png$/, "-05-completed.png") })
      await page.screenshot({ path: screenshotPath })
      return Math.round(performance.now() - started)
    }
    await new Promise((resolve) => setTimeout(resolve, 1000))
  }
  throw new Error(`timed out waiting for gateway session ${sessionID} to complete`)
}

async function waitForTuiScreenCompletion(page, screenshotPath, expectedText) {
  const started = performance.now()
  await page.waitForTimeout(2500)
  await page.screenshot({ path: screenshotPath.replace(/\.png$/, "-04-running.png") })
  const failurePatterns = [
    "provider runtime failed",
    "all providers failed",
    "you didn't provide an api key",
    "http status 401",
    "模型调用失败",
    "runtime failed",
  ]
  const deadline = Date.now() + timeoutMs + 30_000
  const watchdogMs = Number(process.env.COMMAND_RUN_AGENT_TUI_WATCHDOG_MS || 180_000)
  let lastText = ""
  let lastChangeAt = Date.now()
  let nextScreenshotAt = Date.now() + 60_000
  let tick = 0
  let result = null
  while (Date.now() < deadline) {
    const text = await page.getByLabel("TUI screen").innerText().catch(() => "")
    const lower = text.toLowerCase()
    if (text !== lastText) {
      lastText = text
      lastChangeAt = Date.now()
    }
    if (Date.now() >= nextScreenshotAt) {
      tick += 1
      await screenshotTui(page, screenshotPath.replace(/\.png$/, `-watchdog-${String(tick).padStart(2, "0")}.png`))
      nextScreenshotAt = Date.now() + 60_000
    }
    if (failurePatterns.some((pattern) => lower.includes(pattern))) {
      result = { ok: false, text }
      break
    }
    if (expectedText.every((item) => lower.includes(String(item).toLowerCase()))) {
      result = { ok: true, text }
      break
    }
    if (lower.includes("assistant:") && text.includes("✓") && !lower.includes("busy")) {
      result = { ok: true, text }
      break
    }
    if (Date.now() - lastChangeAt > watchdogMs) {
      await screenshotTui(page, screenshotPath.replace(/\.png$/, "-watchdog-stalled.png"))
      throw new Error(`TUI screen stalled for ${watchdogMs}ms before completion:\n${String(text || "").slice(-3000)}`)
    }
    await page.waitForTimeout(2000)
  }
  if (!result) {
    const text = await page.getByLabel("TUI screen").innerText().catch(() => "")
    throw new Error(`timed out waiting for TUI completion:\n${String(text || "").slice(-3000)}`)
  }
  await page.waitForTimeout(800)
  await screenshotTui(page, screenshotPath.replace(/\.png$/, "-05-completed.png"))
  await screenshotTui(page, screenshotPath)
  if (!result.ok) throw new Error(`TUI task failed before completion:\n${String(result.text || "").slice(-3000)}`)
  return Math.round(performance.now() - started)
}

function startTuiWatchdog(page, bridge, gateway, agentDir) {
  mkdirp(agentDir)
  let tick = 0
  let stopped = false
  const interval = setInterval(async () => {
    if (stopped) return
    tick += 1
    const stamp = String(tick).padStart(2, "0")
    const screen = await page.getByLabel("TUI screen").innerText().catch((error) => `screen read failed: ${error?.message || error}`)
    const status = {
      tick,
      at: new Date().toISOString(),
      bridge_alive: bridge.isAlive(),
      gateway_exit_code: gateway.child.exitCode,
      gateway_signal: gateway.child.signalCode,
      screen_tail: String(screen).slice(-2000),
    }
    writeFile(path.join(agentDir, `tui-watchdog-${stamp}.json`), JSON.stringify(status, null, 2))
    await screenshotTui(page, path.join(agentDir, `tui-watchdog-${stamp}.png`)).catch(() => {})
  }, 60_000)
  return () => {
    stopped = true
    clearInterval(interval)
  }
}

async function runTuraViaWebTerminal(workspace, agentDir, agentPort, agentPrompt = "fast", shellSurface = "shell_command") {
  const gatewayExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway")
  if (!fs.existsSync(gatewayExe)) {
    runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_gateway"], { cwd: repoRoot, timeoutMs: 240_000 })
  }
  runOk(npmCmd, ["run", "build"], { cwd: path.join(repoRoot, "apps", "tui"), timeoutMs: 120_000, shell: process.platform === "win32" })
  const sessionId = `frontend-tui-${Date.now()}`
  const env = {
    OPENAI_LOGIN: process.env.OPENAI_LOGIN || "oauth",
    TURA_ENV_PATH: process.env.TURA_ENV_PATH || path.join(repoRoot, ".env"),
    TURA_COMMAND_RUN_SHELL: shellSurface,
    TURA_COMMAND_RUN_STRICT_JSON: "0",
    COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    ...envForShellSurface(shellSurface),
  }
  const gatewayPort = 45900 + Math.floor(Math.random() * 1000)
  const gatewayUrl = `http://127.0.0.1:${gatewayPort}`
  const gateway = startGatewayProcess(gatewayPort, agentDir, env)
  const ready = await waitForGateway(gatewayPort, gateway.child)
  if (!ready) {
    stopServer(gateway.child)
    throw new Error(`gateway did not become ready on ${gatewayPort}`)
  }
  const bridge = await startRealTuiBridge(workspace, gatewayUrl, agentDir, env)
  const { chromium } = tuiRequire("playwright")
  const browser = await chromium.launch({ headless: false })
  const page = await browser.newPage({ viewport: { width: 1200, height: 860 } })
  let stopWatchdog = () => {}
  try {
    await page.goto(bridge.url, { waitUntil: "domcontentloaded" })
    stopWatchdog = startTuiWatchdog(page, bridge, gateway, agentDir)
    await screenshotTui(page, path.join(agentDir, "tui-terminal-00-loaded.png"))
    await page.waitForFunction(
      () => {
        const text = document.querySelector("#screen")?.textContent || ""
        return text.includes("panel:") && text.includes("gateway:") && text.includes("Enter send")
      },
      undefined,
      { timeout: 60_000 },
    )
    await sendRealTuiInput(page, "/new", path.join(agentDir, "tui-setup-new-session.png"))
    await sendRealTuiInput(page, `/agent ${agentPrompt}`, path.join(agentDir, "tui-setup-agent.png"))
    await sendRealTuiInput(page, `/model ${turaModel}`, path.join(agentDir, "tui-setup-model.png"))
    await sendRealTuiInput(page, `/config set model_variant=${reasoning} model_acceleration_enabled=${serviceTier === "priority"}`, path.join(agentDir, "tui-setup-config.png"))

    const firstStarted = performance.now()
    await sendRealTuiInput(page, smokeOnly ? promptSmoke(agentPort, shellSurface) : promptPhase1(agentPort, shellSurface), path.join(agentDir, "tui-phase1-terminal.png"))
    const firstExpected = smokeOnly ? ["Smoke passed", "desktop.png"] : ["artifacts/desktop.png", "artifacts/mobile.png", "artifacts/modal.png"]
    const firstMs = await waitForTuiScreenCompletion(page, path.join(agentDir, "tui-phase1-terminal.png"), firstExpected)
    const firstScreen = await page.getByLabel("TUI screen").innerText()
    writeFile(path.join(agentDir, "phase1.stdout.jsonl"), JSON.stringify({ type: "tui.screen", phase: 1, text: firstScreen }) + "\n")
    const first = {
      command: "apps/tui",
      args: ["interactive", "phase1"],
      status: 0,
      signal: null,
      stdout: firstScreen,
      stderr: "",
      duration_ms: firstMs || Math.round(performance.now() - firstStarted),
      error: null,
    }
    const second = smokeOnly
      ? emptyRun("smoke mode skipped phase2")
      : await (async () => {
          const secondStarted = performance.now()
          await sendRealTuiInput(page, promptPhase2(agentPort, shellSurface), path.join(agentDir, "tui-phase2-terminal.png"))
          const secondMs = await waitForTuiScreenCompletion(page, path.join(agentDir, "tui-phase2-terminal.png"), ["Bulk complete", "CSV"])
          const secondScreen = await page.getByLabel("TUI screen").innerText()
          writeFile(path.join(agentDir, "phase2.stdout.jsonl"), JSON.stringify({ type: "tui.screen", phase: 2, text: secondScreen }) + "\n")
          return {
            command: "apps/tui",
            args: ["interactive", "phase2"],
            status: 0,
            signal: null,
            stdout: secondScreen,
            stderr: "",
            duration_ms: secondMs || Math.round(performance.now() - secondStarted),
            error: null,
          }
        })()
    await sendRealTuiInput(page, "/quit", path.join(agentDir, "tui-quit.png"))
    await page.waitForTimeout(1500)
    return { first, second, threadId: sessionId, error: first.error || second.error || null }
  } finally {
    stopWatchdog()
    await browser.close()
    try {
      if (bridge.isAlive?.()) bridge.term.kill()
    } catch {}
    await new Promise((resolve) => bridge.server.close(resolve))
    stopServer(gateway.child)
  }
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

async function runPiAgent(workspace, agentDir, agentPort) {
  const common = ["--mode", "json"]
  const first = await runLive(piExe, [...common, smokeOnly ? promptSmoke(agentPort) : promptPhase1(agentPort)], {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "phase1.stdout.jsonl"),
    stderrPath: path.join(agentDir, "phase1.stderr.log"),
    statusPath: path.join(agentDir, "phase1.status.json"),
  })
  const second = smokeOnly
    ? emptyRun("smoke mode skipped phase2")
    : await runLive(piExe, [...common, promptPhase2(agentPort)], {
        cwd: workspace,
        timeoutMs,
        stdoutPath: path.join(agentDir, "phase2.stdout.jsonl"),
        stderrPath: path.join(agentDir, "phase2.stderr.log"),
        statusPath: path.join(agentDir, "phase2.status.json"),
      })
  return { first, second, threadId: null, error: first.error || second.error || null }
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
    const ready = await waitForServer(port, server)
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
  const kind = agentKind(agentId)
  const shellSurface = shellSurfaceForAgent(agentId)
  try {
    if (kind === "current-shll" || kind === "current-bash") {
      result = await runCurrentLike(agentId, codexCurrentExe, workspace, agentDir, agentPort, shellSurface)
    } else if (kind === "codex-main") {
      result = await runCurrentLike(agentId, codexMainExe, workspace, agentDir, agentPort, shellSurface)
    } else if (kind === "codex-main-bash") {
      result = await runCurrentLike(agentId, codexMainExe, workspace, agentDir, agentPort, shellSurface)
    } else if (kind === "tura-fast-shll" || kind === "tura-fast-bash") {
      result = await runTura(workspace, agentDir, agentPort, "fast", shellSurface)
    } else if (kind === "tura-shll") {
      result = await runTura(workspace, agentDir, agentPort, "coding_agent", shellSurface)
    } else if (kind === "tura-bash") {
      result = await runTura(workspace, agentDir, agentPort, "coding_agent", shellSurface)
    } else if (kind === "tui-fast-shll") {
      result = await runTuraViaWebTerminal(workspace, agentDir, agentPort, "fast", shellSurface)
    } else if (kind === "tui-shll") {
      result = await runTuraViaWebTerminal(workspace, agentDir, agentPort, "coding_agent", shellSurface)
    } else if (kind === "claude-code") {
      result = await runClaudeCode(workspace, agentDir, agentPort)
    } else if (kind === "pi-agent") {
      result = await runPiAgent(workspace, agentDir, agentPort)
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
    shell_surface: shellSurface,
    thread_id: result.threadId,
    elapsed_ms: Math.round(performance.now() - started),
    phase1_ms: result.first.duration_ms,
    phase2_ms: result.second.duration_ms,
    phase1_status: result.first.status,
    phase2_status: result.second.status,
    error: runError || result.error || null,
    usage: usageFromRuns(agentId, result),
    usage_by_phase: usageDiagnostics(result),
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
    const summary = normalizeBusinessSummary({
      ok: true,
      prep_only: true,
      template,
      evaluator,
    }, runPaths)
    writeFile(summaryPath, JSON.stringify(summary, null, 2))
    console.log(JSON.stringify(summary, null, 2))
    return
  }
  if (requireCodexCurrentExe()) {
    assert(fs.existsSync(codexCurrentExe), `missing current exe ${codexCurrentExe}`)
  }
  if (requireCodexMainExe()) {
    assert(fs.existsSync(codexMainExe), `missing main exe ${codexMainExe}`)
  }
  if (requirePiExe() && fs.existsSync(piExe) === false && !["pi", "pi.cmd"].includes(piExe)) {
    assert(fs.existsSync(piExe), `missing pi exe ${piExe}`)
  }
  const results = await Promise.all(agents.map((agent, index) => {
    console.log(`[frontend-playwright-e2e] running ${agent}`)
    return runAgent(agent, template, evaluator, index)
  }))
  const summary = normalizeBusinessSummary({
    ok: results.every((result) => result.validation?.pass),
    model,
    claude_model: claudeModel,
    reasoning,
    timeout_ms: timeoutMs,
    smoke_only: smokeOnly,
    agents,
    results,
  }, runPaths)
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
  if (!summary.ok && process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE !== "1") {
    process.exitCode = 1
  }
}

await main()
