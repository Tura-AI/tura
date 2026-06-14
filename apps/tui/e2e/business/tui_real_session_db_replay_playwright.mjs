#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import fsp from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";
import net from "node:net";
import path from "node:path";
import process from "node:process";

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..");
const appRoot = path.join(repoRoot, "apps", "tui");
const runRoot = path.join(repoRoot, "target", "tui-real-session-db-replay", String(Date.now()));
const workspace = path.join(runRoot, "workspace");
const turaHome = path.join(runRoot, "tura-home");
const logsDir = path.join(runRoot, "logs");
const screenshotsDir = path.join(runRoot, "screenshots");
const summaryPath = path.join(runRoot, "summary.json");
const debugDir = path.join(repoRoot, "target", "debug");
const exeSuffix = process.platform === "win32" ? ".exe" : "";
const sourceWorkspace = path.resolve(process.env.TURA_REAL_SESSION_DB_WORKSPACE || repoRoot);
const sourceIndexDb = path.resolve(
  process.env.TURA_REAL_SESSION_DB_INDEX ||
    path.join(repoRoot, "db", "session_log", "index.sqlite3"),
);
const sourceWorkspaceDb = path.resolve(
  process.env.TURA_REAL_SESSION_DB_WORKSPACE_DB ||
    path.join(sourceWorkspace, ".tura", "session_log.sqlite3"),
);
const copiedIndexDb = path.join(turaHome, "db", "session_log", "index.sqlite3");
const copiedWorkspaceDb = path.join(workspace, ".tura", "session_log.sqlite3");
const fixtureTitle = "REAL_DB_REPLAY_SESSION";
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

async function copySqliteFamily(source, destination) {
  await fsp.mkdir(path.dirname(destination), { recursive: true });
  await fsp.copyFile(source, destination);
  for (const suffix of ["-wal", "-shm"]) {
    const sidecar = `${source}${suffix}`;
    if (fs.existsSync(sidecar)) await fsp.copyFile(sidecar, `${destination}${suffix}`);
  }
}

async function prepareRealSessionDbCopy() {
  if (!fs.existsSync(sourceIndexDb))
    throw new Error(`missing real session index db: ${sourceIndexDb}`);
  if (!fs.existsSync(sourceWorkspaceDb))
    throw new Error(`missing real workspace session db: ${sourceWorkspaceDb}`);
  await fsp.rm(runRoot, { recursive: true, force: true });
  await fsp.mkdir(workspace, { recursive: true });
  await fsp.mkdir(logsDir, { recursive: true });
  await fsp.mkdir(screenshotsDir, { recursive: true });
  await copySqliteFamily(sourceIndexDb, copiedIndexDb);
  await copySqliteFamily(sourceWorkspaceDb, copiedWorkspaceDb);
  return rewriteCopiedSessionDb();
}

function rewriteCopiedSessionDb() {
  const script = String.raw`
import json, sqlite3, sys, time
index_db, workspace_db, source_workspace, source_workspace_db, target_workspace, target_workspace_db, title = sys.argv[1:]
def norm(value):
    value = value.replace("\\", "/").rstrip("/")
    if len(value) == 2 and value[1] == ":":
        return value + "/"
    return value
source_workspace = norm(source_workspace)
source_workspace_db = norm(source_workspace_db)
target_workspace = norm(target_workspace)
target_workspace_db = norm(target_workspace_db)
now_ms = int(time.time() * 1000)
con = sqlite3.connect(index_db)
cur = con.cursor()
cur.execute(
    "UPDATE sessions SET workspace = ?1, workspace_db_path = ?2 WHERE replace(rtrim(workspace, '/'), '\\\\', '/') = ?3 OR replace(workspace_db_path, '\\\\', '/') = ?4",
    (target_workspace, target_workspace_db, source_workspace, source_workspace_db),
)
index_rows = cur.rowcount
row = cur.execute(
    "SELECT session_id, message_count FROM sessions WHERE workspace = ?1 ORDER BY message_count DESC, updated_at DESC, session_id ASC LIMIT 1",
    (target_workspace,),
).fetchone()
if row is None:
    raise SystemExit(f"no copied sessions matched workspace {source_workspace}")
session_id, message_count = row
cur.execute(
    "UPDATE sessions SET name = ?1, updated_at = ?2 WHERE session_id = ?3",
    (title, now_ms, session_id),
)
con.commit()
con.close()

con = sqlite3.connect(workspace_db)
cur = con.cursor()
row = cur.execute(
    "SELECT session_json, management_json FROM sessions WHERE session_id = ?1",
    (session_id,),
).fetchone()
if row is None:
    raise SystemExit(f"copied workspace db does not contain {session_id}")
session = json.loads(row[0])
management = json.loads(row[1])
session["id"] = session_id
session["directory"] = target_workspace
session["name"] = title
session["session_display_name"] = title
if isinstance(session.get("management"), dict):
    session["management"]["session_id"] = session_id
    session["management"]["session_name"] = title
    session["management"]["session_directory"] = target_workspace
management["session_id"] = session_id
management["session_name"] = title
management["session_directory"] = target_workspace
session["management"] = management
cur.execute(
    "UPDATE sessions SET workspace = ?1, name = ?2, updated_at = ?3, session_json = ?4, management_json = ?5 WHERE session_id = ?6",
    (target_workspace, title, now_ms, json.dumps(session), json.dumps(management), session_id),
)
records = cur.execute(
    "SELECT COUNT(*) FROM session_records WHERE session_id = ?1",
    (session_id,),
).fetchone()[0]
latest = cur.execute(
    "SELECT record_json FROM session_records WHERE session_id = ?1 ORDER BY created_at DESC, id DESC LIMIT 1",
    (session_id,),
).fetchone()
oldest = cur.execute(
    "SELECT record_json FROM session_records WHERE session_id = ?1 ORDER BY created_at ASC, id ASC LIMIT 1",
    (session_id,),
).fetchone()
con.commit()
con.close()

def text_from(raw):
    if not raw:
        return ""
    value = json.loads(raw[0])
    parts = value.get("parts") or value.get("info", {}).get("parts") or []
    text = "".join((part.get("text") or part.get("content") or "") for part in parts if isinstance(part, dict))
    return text.replace("\r", "\n").replace("\n", " ").strip()[:160]

print(json.dumps({
    "session_id": session_id,
    "message_count": message_count,
    "record_count": records,
    "index_rows_rewritten": index_rows,
    "latest_text": text_from(latest),
    "oldest_text": text_from(oldest),
}))
`;
  const result = spawnSync(
    "python",
    [
      "-",
      copiedIndexDb,
      copiedWorkspaceDb,
      sourceWorkspace,
      sourceWorkspaceDb,
      workspace,
      copiedWorkspaceDb,
      fixtureTitle,
    ],
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
      `failed to rewrite copied session db\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`,
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

async function waitForTerminalText(page, pattern, timeoutMs = 10_000) {
  await page.waitForFunction(
    (source) => {
      const matcher = new RegExp(source);
      const buffer = window.__turaTerminal?.buffer.active;
      if (!buffer) return false;
      for (let index = 0; index < buffer.length; index += 1) {
        const text = buffer.getLine(index)?.translateToString(true) ?? "";
        if (matcher.test(text)) return true;
      }
      return false;
    },
    pattern.source,
    { timeout: timeoutMs },
  );
}

async function main() {
  ensureBuilds();
  const dbSummary = await prepareRealSessionDbCopy();
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
    await page.goto(`${web.url}/rich?instance=real-session-db`, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.__turaTerminal);
    await page.evaluate(() => window.__turaFit());
    await page.waitForFunction((title) => document.body.innerText.includes(title), fixtureTitle, {
      timeout: 60_000,
    });
    await page.screenshot({
      path: path.join(screenshotsDir, "loaded-real-session-db.png"),
      fullPage: false,
    });
    screenshots.push(path.join(screenshotsDir, "loaded-real-session-db.png"));

    const before = await terminalBufferSnapshot(page);
    await page.evaluate(() => window.__turaSendInput("REAL_DB_INPUT_OK"));
    await waitForTerminalText(page, /REAL_DB_INPUT_OK/);
    await page.waitForTimeout(2500);
    const after = await terminalBufferSnapshot(page);
    assert.match(after.text, /REAL_DB_INPUT_OK/);
    assert.ok(
      after.length <= before.length + 3,
      `terminal buffer grew during idle real-db replay: before=${before.length} after=${after.length}`,
    );
    assert.ok(dbSummary.record_count > 0, "copied real session db should contain records");
    assert.ok(dbSummary.index_rows_rewritten > 0, "copied index should point at temp workspace");
    await page.screenshot({
      path: path.join(screenshotsDir, "composer-accepts-input-after-real-db-load.png"),
      fullPage: false,
    });
    screenshots.push(path.join(screenshotsDir, "composer-accepts-input-after-real-db-load.png"));

    const summary = {
      ok: true,
      runRoot,
      workspace,
      turaHome,
      sourceIndexDb,
      sourceWorkspaceDb,
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
      sourceIndexDb,
      sourceWorkspaceDb,
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
  }
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

await main();
