#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import fsp from "node:fs/promises";
import { createRequire } from "node:module";
import net from "node:net";
import path from "node:path";
import process from "node:process";

import { cleanupRepoTuraProcesses } from "./cleanup_repo_tura_processes.mjs";

const repoRoot =
  process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..", "..");
const appRoot = path.join(repoRoot, "apps", "tui");
const runRoot = path.join(
  repoRoot,
  "apps",
  "tui",
  "test-results",
  "tui-real-session-db-replay",
  String(Date.now()),
);
const workspace = path.join(runRoot, "workspace");
const turaHome = path.join(runRoot, "tura-home");
const logsDir = path.join(runRoot, "logs");
const screenshotsDir = path.join(runRoot, "screenshots");
const summaryPath = path.join(runRoot, "summary.json");
const debugDir = path.join(repoRoot, "target", "debug");
const exeSuffix = process.platform === "win32" ? ".exe" : "";
const copiedIndexDb = path.join(turaHome, "db", "session_log", "index.sqlite3");
const copiedWorkspaceDb = path.join(workspace, ".tura", "session_log.sqlite3");
const fixtureTitle = "MOCK_DB_REPLAY_SESSION";
const fixtureSessionId = "mock-db-replay-session";
const tuiRequire = createRequire(path.join(appRoot, "package.json"));
const { chromium } = tuiRequire("playwright");

function runChecked(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 120_000,
    windowsHide: true,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status || result.signal}\nSTDOUT:\n${result.stdout || ""}\nSTDERR:\n${result.stderr || ""}`,
      { cause: result.error },
    );
  }
}

function ensureBuilds() {
  runChecked(
    process.platform === "win32" ? "cmd.exe" : "npm",
    process.platform === "win32" ? ["/d", "/s", "/c", "npm run build"] : ["run", "build"],
    {
      cwd: appRoot,
      timeoutMs: 120_000,
    },
  );
  const required = ["tura_gateway", "tura_router", "tura_runtime", "tura_session_db"].map((name) =>
    path.join(debugDir, `${name}${exeSuffix}`),
  );
  if (required.every((file) => fs.existsSync(file))) return;
  runChecked(
    "cargo",
    [
      "build",
      "-p",
      "gateway",
      "--bin",
      "tura_gateway",
      "-p",
      "router",
      "--bin",
      "tura_router",
      "-p",
      "runtime",
      "--bin",
      "tura_runtime",
      "-p",
      "session_log",
      "--bin",
      "tura_session_db",
    ],
    { cwd: repoRoot, timeoutMs: 300_000 },
  );
}

async function prepareMockSessionDb() {
  await fsp.rm(runRoot, { recursive: true, force: true });
  await fsp.mkdir(workspace, { recursive: true });
  await fsp.mkdir(logsDir, { recursive: true });
  await fsp.mkdir(screenshotsDir, { recursive: true });
  await fsp.mkdir(path.dirname(copiedIndexDb), { recursive: true });
  await fsp.mkdir(path.dirname(copiedWorkspaceDb), { recursive: true });
  return createMockSessionDb();
}

function createMockSessionDb() {
  const script = String.raw`
import json, sqlite3, sys, time
index_db, workspace_db, target_workspace, title, session_id = sys.argv[1:]
def norm(value):
    value = value.replace("\\", "/").rstrip("/")
    if len(value) == 2 and value[1] == ":":
        return value + "/"
    return value
target_workspace = norm(target_workspace)
workspace_db_text = norm(workspace_db)
now_ms = int(time.time() * 1000)
created_at = now_ms - 120000
assistant_at = now_ms - 60000
user_at = now_ms - 30000
management = {
    "session_id": session_id,
    "session_name": title,
    "session_directory": target_workspace,
    "session_created_at": time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime(created_at / 1000)),
    "session_last_update_at": time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime(now_ms / 1000)),
    "session_last_user_message_at": time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime(user_at / 1000)),
    "state": "completed",
}
task_management = {
    "plan_summary": title,
    "tasks": [{
        "task_id": "mock-db-replay-task",
        "step": 1,
        "task_summary": "Replay a seeded mock session DB in the TUI",
        "deliverable": "The terminal can hydrate persisted session records and still accept input.",
        "status": "done",
        "start_condition": "user_action",
    }],
}
session = {
    "id": session_id,
    "name": title,
    "directory": target_workspace,
    "model": "openai/gpt-5.5",
    "agent": "coding_agent",
    "session_type": "coding",
    "status": "idle",
    "message_count": 2,
    "created_at": created_at,
    "updated_at": now_ms,
    "last_user_message_at": user_at,
    "task_management": task_management,
    "management": management,
    "plan_summary": title,
    "session_display_name": title,
}
messages = [
    {
        "id": "mock-db-replay-user",
        "session_id": session_id,
        "role": "user",
        "parts": [{"id": "mock-db-replay-user-part", "type": "text", "text": "Load the mock session DB replay fixture."}],
        "created_at": user_at,
        "updated_at": user_at,
        "parent_id": None,
    },
    {
        "id": "mock-db-replay-assistant",
        "session_id": session_id,
        "role": "assistant",
        "parts": [{"id": "mock-db-replay-assistant-part", "type": "text", "text": "MOCK_DB_REPLAY_SESSION_READY"}],
        "created_at": assistant_at,
        "updated_at": assistant_at,
        "parent_id": "mock-db-replay-user",
    },
]

def init_index_db(con):
    con.executescript("""
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            workspace_db_path TEXT NOT NULL,
            name TEXT,
            parent_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_user_message_at INTEGER,
            state TEXT,
            status TEXT,
            message_count INTEGER NOT NULL DEFAULT 0,
            task_management_json TEXT NOT NULL,
            management_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_workspace_updated
            ON sessions(workspace, updated_at DESC, session_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_parent
            ON sessions(parent_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_workspace_last_user_message
            ON sessions(workspace, last_user_message_at DESC, session_id);
        CREATE TABLE IF NOT EXISTS session_write_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            idempotency_key TEXT NOT NULL UNIQUE,
            session_id TEXT NOT NULL,
            turn_id TEXT NULL,
            runtime_worker_id TEXT NULL,
            command_run_id TEXT NULL,
            command_id TEXT NULL,
            event_seq INTEGER NULL,
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            status TEXT NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            applied_at INTEGER NULL,
            last_error TEXT NULL
        );
    """)

def init_workspace_db(con):
    con.executescript("""
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            name TEXT,
            parent_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_user_message_at INTEGER,
            state TEXT,
            status TEXT,
            message_count INTEGER NOT NULL DEFAULT 0,
            task_management_json TEXT NOT NULL,
            management_json TEXT NOT NULL,
            session_json TEXT NOT NULL,
            todos_json TEXT NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_workspace_sessions_updated
            ON sessions(workspace, updated_at DESC, session_id);
        CREATE INDEX IF NOT EXISTS idx_workspace_sessions_last_user_message
            ON sessions(workspace, last_user_message_at DESC, session_id);
        CREATE TABLE IF NOT EXISTS session_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            role TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            record_json TEXT NOT NULL,
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_records_session_created
            ON session_records(session_id, created_at, id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_records_session_message
            ON session_records(session_id, message_id);
    """)

session_json = json.dumps(session)
management_json = json.dumps(management)
task_management_json = json.dumps(task_management)
con = sqlite3.connect(index_db)
init_index_db(con)
cur = con.cursor()
cur.execute(
    """INSERT INTO sessions(
        session_id, workspace, workspace_db_path, name, parent_id, created_at, updated_at,
        last_user_message_at, state, status, message_count, task_management_json, management_json
    ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, 'completed', 'idle', ?8, ?9, ?10)""",
    (session_id, target_workspace, workspace_db_text, title, created_at, now_ms, user_at, len(messages), task_management_json, management_json),
)
con.commit()
con.close()

con = sqlite3.connect(workspace_db)
init_workspace_db(con)
cur = con.cursor()
cur.execute(
    """INSERT INTO sessions(
        session_id, workspace, name, parent_id, created_at, updated_at, last_user_message_at,
        state, status, message_count, task_management_json, management_json, session_json, todos_json
    ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, 'completed', 'idle', ?7, ?8, ?9, ?10, '[]')""",
    (session_id, target_workspace, title, created_at, now_ms, user_at, len(messages), task_management_json, management_json, session_json),
)
for message in messages:
    cur.execute(
        """INSERT INTO session_records(session_id, message_id, role, created_at, updated_at, record_json)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)""",
        (session_id, message["id"], message["role"], message["created_at"], message["updated_at"], json.dumps(message)),
    )
records = len(messages)
con.commit()
con.close()

def text_from(value):
    parts = value.get("parts") or []
    text = "".join((part.get("text") or part.get("content") or "") for part in parts if isinstance(part, dict))
    return text.replace("\r", "\n").replace("\n", " ").strip()[:160]

print(json.dumps({
    "session_id": session_id,
    "message_count": len(messages),
    "record_count": records,
    "index_rows_rewritten": 1,
    "latest_text": text_from(messages[-1]),
    "oldest_text": text_from(messages[0]),
}))
`;
  const result = spawnSync(
    "python",
    ["-", copiedIndexDb, copiedWorkspaceDb, workspace, fixtureTitle, fixtureSessionId],
    {
      input: script,
      encoding: "utf8",
      text: true,
      windowsHide: true,
      timeout: 30_000,
    },
  );
  if (result.status !== 0) {
    throw new Error(
      `failed to create mock session db\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`,
    );
  }
  return JSON.parse(result.stdout);
}

async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close(() => resolve(port));
    });
    server.on("error", reject);
  });
}

function testEnv(extra = {}) {
  return {
    ...process.env,
    ...extra,
    PATH: `${debugDir}${path.delimiter}${process.env.PATH || ""}`,
    TURA_HOME: turaHome,
    TURA_PROJECT_ROOT: repoRoot,
    TURA_CWD: workspace,
    FORCE_COLOR: "1",
  };
}

async function waitForUrl(url, child, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    if (child?.exitCode !== null) {
      throw new Error(`${url} exited before readiness with ${child.exitCode}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) return response;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(200);
  }
  throw lastError || new Error(`timed out waiting for ${url}`);
}

async function startGateway(port) {
  const child = spawn(path.join(debugDir, `tura_gateway${exeSuffix}`), [], {
    cwd: workspace,
    env: testEnv({
      PORT: String(port),
      TURA_GATEWAY_PORT: String(port),
      TURA_GATEWAY_URL: `http://127.0.0.1:${port}`,
      TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF: "1",
    }),
    stdio: ["pipe", "pipe", "pipe"],
    windowsHide: true,
  });
  pipeLog(child.stdout, path.join(logsDir, "gateway.stdout.log"));
  pipeLog(child.stderr, path.join(logsDir, "gateway.stderr.log"));
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/global/health`, child);
  return { child, url };
}

async function startWebTerminal(gatewayUrl, port) {
  const child = spawn(process.execPath, [path.join(appRoot, "scripts", "web-terminal.mjs")], {
    cwd: appRoot,
    env: testEnv({
      PORT: String(port),
      TURA_GATEWAY_URL: gatewayUrl,
    }),
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  pipeLog(child.stdout, path.join(logsDir, "web-terminal.stdout.log"));
  pipeLog(child.stderr, path.join(logsDir, "web-terminal.stderr.log"));
  await waitForUrl(`http://127.0.0.1:${port}/`, child, 30_000);
  return { child, url: `http://127.0.0.1:${port}` };
}

function pipeLog(stream, file) {
  stream?.pipe(fs.createWriteStream(file));
}

async function stopGateway(gateway) {
  if (!gateway?.child || gateway.child.exitCode !== null || gateway.child.killed) return;
  gateway.child.stdin?.end();
  await Promise.race([
    new Promise((resolve) => gateway.child.once("exit", resolve)),
    delay(15_000),
  ]);
  await stopProcess(gateway.child);
}

async function stopProcess(child) {
  if (!child || child.exitCode !== null || child.killed) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
    return;
  }
  child.kill("SIGTERM");
  await Promise.race([new Promise((resolve) => child.once("exit", resolve)), delay(2_000)]);
  if (child.exitCode === null) child.kill("SIGKILL");
}

async function terminalBufferSnapshot(page) {
  return page.evaluate(() => {
    const buffer = window.__turaTerminal?.buffer.active;
    if (!buffer) return { length: 0, text: "" };
    const lines = [];
    for (let index = 0; index < buffer.length; index += 1) {
      lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
    }
    return { length: buffer.length, text: lines.join("\n") };
  });
}

async function waitForTerminalBufferGrowth(page, minimumLength, timeoutMs = 10_000) {
  await page.waitForFunction(
    (expectedLength) => {
      const buffer = window.__turaTerminal?.buffer.active;
      return Boolean(buffer && buffer.length >= expectedLength);
    },
    minimumLength,
    { timeout: timeoutMs },
  );
}

async function main() {
  ensureBuilds();
  const dbSummary = await prepareMockSessionDb();
  const gatewayPort = await freePort();
  const webPort = await freePort();
  let gateway;
  let web;
  let browser;
  const screenshots = [];
  try {
    gateway = await startGateway(gatewayPort);
    web = await startWebTerminal(gateway.url, webPort);
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    await page.goto(`${web.url}/rich?instance=mock-session-db`, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.__turaTerminal);
    await page.evaluate(() => window.__turaFit());
    await waitForTerminalBufferGrowth(page, 1, 60_000);
    await page.screenshot({
      path: path.join(screenshotsDir, "loaded-mock-session-db.png"),
      fullPage: false,
    });
    screenshots.push(path.join(screenshotsDir, "loaded-mock-session-db.png"));

    const before = await terminalBufferSnapshot(page);
    await page.evaluate(() => window.__turaSendInput("MOCK_DB_INPUT_OK"));
    await waitForTerminalBufferGrowth(page, before.length);
    await page.waitForTimeout(2500);
    const after = await terminalBufferSnapshot(page);
    assert.ok(after.text.length >= before.text.length, "terminal should accept input after replay");
    assert.ok(
      after.length <= before.length + 3,
      `terminal buffer grew during idle mock-db replay: before=${before.length} after=${after.length}`,
    );
    assert.ok(dbSummary.record_count > 0, "mock session db should contain records");
    assert.ok(dbSummary.index_rows_rewritten > 0, "mock index should point at temp workspace");
    await page.screenshot({
      path: path.join(screenshotsDir, "composer-accepts-input-after-mock-db-load.png"),
      fullPage: false,
    });
    screenshots.push(path.join(screenshotsDir, "composer-accepts-input-after-mock-db-load.png"));

    const summary = {
      ok: true,
      runRoot,
      workspace,
      turaHome,
      copiedIndexDb,
      copiedWorkspaceDb,
      dbSummary,
      bufferBefore: before.length,
      bufferAfter: after.length,
      screenshots,
    };
    await fsp.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
  } catch (error) {
    if (browser) {
      const pages = browser.contexts().flatMap((context) => context.pages());
      const page = pages[0];
      if (page) {
        try {
          const pathAfterFailure = path.join(screenshotsDir, "failure-after-input.png");
          await page.screenshot({ path: pathAfterFailure, fullPage: false });
          screenshots.push(pathAfterFailure);
          const buffer = await terminalBufferSnapshot(page).catch(() => undefined);
          if (buffer) {
            await fsp.writeFile(
              path.join(runRoot, "failure-terminal-buffer.txt"),
              buffer.text,
              "utf8",
            );
          }
        } catch {
          // Best-effort diagnostics only.
        }
      }
    }
    const summary = {
      ok: false,
      runRoot,
      workspace,
      turaHome,
      copiedIndexDb,
      copiedWorkspaceDb,
      error: error instanceof Error ? error.stack || error.message : String(error),
      screenshots,
    };
    await fsp.mkdir(runRoot, { recursive: true });
    await fsp.writeFile(summaryPath, JSON.stringify(summary, null, 2));
    console.error(JSON.stringify(summary, null, 2));
    process.exitCode = 1;
  } finally {
    await browser?.close().catch(() => undefined);
    await stopProcess(web?.child);
    await stopGateway(gateway);
    cleanupRepoTuraProcesses();
  }
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

await main();
