import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import fsp from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
export const appRoot = path.resolve(here, "..", "..", "..");
export const repoRoot = path.resolve(appRoot, "..", "..");
const exeSuffix = process.platform === "win32" ? ".exe" : "";
const debugDir = path.join(repoRoot, "target", "debug");
const tuiBin = path.join(appRoot, "dist", "index.js");
const defaultModel = process.env.TURA_BUSINESS_MODEL || "codex/gpt-5.5";
const defaultAgent = process.env.TURA_BUSINESS_AGENT || "direct";
const defaultVariant = process.env.TURA_BUSINESS_MODEL_VARIANT || "medium";
const defaultTimeoutMsByCase = {
  "single-request": 180_000,
  snake: 240_000,
  "password-zip": 600_000,
};

export const caseNames = ["single-request", "snake", "password-zip"];

export async function runTuiLocalCase(caseName) {
  ensureLocalTuiBuild();
  ensureDebugBackend();
  const timeoutMs = caseTimeoutMs(caseName);
  const ctx = await prepareContext(caseName);
  const definition = caseDefinition(caseName, ctx);
  let gateway;
  let result;
  let cleanup;
  try {
    gateway = await startDebugGateway(ctx);
    const args = [
      "--gateway-url",
      gateway.url,
      "--cwd",
      ctx.workspace,
      "run",
      "-m",
      defaultModel,
      "-a",
      defaultAgent,
      "--model-reasoning-effort",
      defaultVariant,
      "-p",
      "--timeout",
      String(Math.ceil(timeoutMs / 1000)),
      "--last-message-file",
      ctx.lastMessagePath,
      definition.prompt,
    ];
    result = await runLoggedProcess(process.execPath, [tuiBin, ...args], ctx, {
      timeoutMs: timeoutMs + 30_000,
      env: {
        TURA_GATEWAY_PORT: String(gateway.port),
        TURA_GATEWAY_URL: gateway.url,
      },
    });
  } finally {
    cleanup = await shutdownBackendDaemons(ctx);
    await stopProcess(gateway?.child);
  }
  return finishCase(ctx, definition, result, { cleanup });
}

function ensureLocalTuiBuild() {
  runChecked(process.platform === "win32" ? "npm.cmd" : "npm", ["run", "build"], {
    cwd: appRoot,
    timeoutMs: 120_000,
  });
}

function ensureDebugBackend() {
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

function runChecked(command, args, options = {}) {
  const shell = process.platform === "win32" && command.endsWith(".cmd");
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 120_000,
    windowsHide: true,
    shell,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status || result.signal || result.error}\nSTDOUT:\n${result.stdout || ""}\nSTDERR:\n${result.stderr || ""}`,
    );
  }
}

async function prepareContext(caseName) {
  if (!caseNames.includes(caseName)) {
    throw new Error(`Unknown TUI business case ${caseName}. Expected: ${caseNames.join(", ")}`);
  }
  const runId = process.env.TURA_BUSINESS_RUN_ID || `tui-${caseName}-${timestamp()}`;
  const runRoot = path.join(
    repoRoot,
    "apps",
    "tui",
    "test-results",
    "business",
    "local",
    caseName,
    runId,
  );
  const ctx = {
    caseName,
    runId,
    runRoot,
    workspace: path.join(runRoot, "workspace"),
    logs: path.join(runRoot, "logs"),
    summaryPath: path.join(runRoot, "summary.json"),
    lastMessagePath: path.join(runRoot, "last-message.txt"),
    providerLogRoot: path.join(runRoot, "logs", "provider"),
    turaHome: path.join(runRoot, "tura-home"),
  };
  await fsp.rm(ctx.runRoot, { recursive: true, force: true });
  await fsp.mkdir(ctx.workspace, { recursive: true });
  await fsp.mkdir(ctx.logs, { recursive: true });
  await writeWorkspaceConfig(ctx);
  if (caseName === "snake") await exposeWorkspaceNodeModules(ctx);
  if (caseName === "password-zip") await writePasswordZipSeed(ctx);
  await fsp.writeFile(
    path.join(ctx.workspace, "BUSINESS_TEST_CONTEXT.md"),
    [
      `# TUI ${caseName} local build business test`,
      "",
      "This workspace is disposable and belongs to an app-local TUI test.",
      "Use the current workspace for all generated files.",
      "Do not ask follow-up questions.",
      "",
    ].join("\n"),
  );
  return ctx;
}

async function writeWorkspaceConfig(ctx) {
  const configDir = path.join(ctx.workspace, ".tura");
  await fsp.mkdir(configDir, { recursive: true });
  await fsp.writeFile(
    path.join(configDir, "config.conf"),
    [
      `model=${defaultModel}`,
      "active_provider=codex",
      "active_model=gpt-5.5",
      `active_agent=${defaultAgent}`,
      "session_type=coding",
      `model_variant=${defaultVariant}`,
      `model_acceleration_enabled=${process.env.TUI_PROVIDER_BUSINESS_PRIORITY === "1"}`,
      "kill_processes_on_start=false",
      "validator_enabled=false",
      "",
    ].join("\n"),
  );
}

async function exposeWorkspaceNodeModules(ctx) {
  const source = [path.join(appRoot, "node_modules"), path.join(repoRoot, "node_modules")].find(
    (candidate) => fs.existsSync(path.join(candidate, "playwright")),
  );
  if (!source) return;
  const destination = path.join(ctx.workspace, "node_modules");
  await fsp.rm(destination, { recursive: true, force: true });
  try {
    await fsp.symlink(source, destination, process.platform === "win32" ? "junction" : "dir");
  } catch {
    await fsp.cp(source, destination, { recursive: true });
  }
}

async function writePasswordZipSeed(ctx) {
  const salt = "tura-password-zip-20260611";
  const dictionaryPassword = "tura-zip-5519";
  const bruteForcePassword = "cab";
  const dictionaryTarget = createHash("sha256")
    .update(`${salt}:${dictionaryPassword}`, "utf8")
    .digest("hex");
  const bruteForceTarget = createHash("sha256")
    .update(`${salt}:${bruteForcePassword}`, "utf8")
    .digest("hex");
  const legacyDir = path.join(ctx.workspace, "legacy_zip_password_cli");
  const fixtureDir = path.join(ctx.workspace, "fixtures");
  const acceptanceDir = path.join(ctx.workspace, "acceptance");
  await fsp.mkdir(legacyDir, { recursive: true });
  await fsp.mkdir(fixtureDir, { recursive: true });
  await fsp.mkdir(acceptanceDir, { recursive: true });
  await fsp.writeFile(
    path.join(legacyDir, "legacy_zip_password_finder.mjs"),
    [
      "#!/usr/bin/env node",
      "import crypto from 'node:crypto'",
      "import fs from 'node:fs'",
      "const args = process.argv.slice(2)",
      "let input = ''",
      "let wordlist = ''",
      "for (let i = 0; i < args.length; i += 1) {",
      "  if (args[i] === '-i' || args[i] === '--input') input = args[++i] || ''",
      "  else if (args[i] === '-w' || args[i] === '--wordlist') wordlist = args[++i] || ''",
      "}",
      "if (!input || !wordlist) { console.error('usage: legacy_zip_password_finder --input fixture.json --wordlist candidates.txt'); process.exit(2) }",
      "const fixture = JSON.parse(fs.readFileSync(input, 'utf8'))",
      "const candidates = fs.readFileSync(wordlist, 'utf8').split(/\\r?\\n/u).map((line) => line.trim()).filter(Boolean)",
      "for (const candidate of candidates) {",
      "  const digest = crypto.createHash('sha256').update(`${fixture.salt}:${candidate}`, 'utf8').digest('hex')",
      "  if (digest === fixture.target) { console.log(`Password found: ${candidate}`); process.exit(0) }",
      "}",
      "console.error('Password not found')",
      "process.exit(1)",
      "",
    ].join("\n"),
  );
  await fsp.writeFile(
    path.join(fixtureDir, "secret.zip.fixture.json"),
    JSON.stringify(
      { kind: "tura.sha256.zip-password-fixture.v1", salt, target: dictionaryTarget },
      null,
      2,
    ),
  );
  await fsp.writeFile(
    path.join(fixtureDir, "bruteforce.zip.fixture.json"),
    JSON.stringify(
      { kind: "tura.sha256.zip-password-fixture.v1", salt, target: bruteForceTarget },
      null,
      2,
    ),
  );
  await fsp.writeFile(
    path.join(fixtureDir, "candidates.txt"),
    ["winter-2024", "password", "letmein", "tura-zip-5519", "archive-open"].join("\n"),
  );
  await fsp.writeFile(
    path.join(acceptanceDir, "zip_password_cli_acceptance.mjs"),
    [
      "#!/usr/bin/env node",
      "import assert from 'node:assert/strict'",
      "import fs from 'node:fs'",
      "import path from 'node:path'",
      "import { spawnSync } from 'node:child_process'",
      "const root = process.cwd()",
      "const cli = path.join(root, 'zip_password_refactor', 'bin', 'zip-password-finder.mjs')",
      "const legacyCli = path.join(root, 'legacy_zip_password_cli', 'legacy_zip_password_finder.mjs')",
      "const reportPath = path.join(root, 'zip_password_refactor', 'acceptance-report.json')",
      "function run(command, args, expectedStatus = 0) {",
      "  const result = spawnSync(process.execPath, [command, ...args], { cwd: root, encoding: 'utf8', windowsHide: true, timeout: 30_000 })",
      "  assert.equal(result.status, expectedStatus, `${command} exited with ${result.status}: ${result.stderr || result.stdout}`)",
      "  return result",
      "}",
      "function password(text) { return String(text || '').match(/Password found:\\s*([^\\r\\n]+)/i)?.[1]?.trim() || '' }",
      "assert.ok(fs.existsSync(cli), `missing CLI ${cli}`)",
      "assert.ok(fs.statSync(cli).size > 700, 'CLI implementation is too small')",
      "assert.match(run(cli, ['--help']).stdout, /zip-password-finder|--input|--wordlist|--json/is)",
      "const oracle = password(run(legacyCli, ['--input', 'fixtures/secret.zip.fixture.json', '--wordlist', 'fixtures/candidates.txt']).stdout)",
      "assert.equal(oracle, 'tura-zip-5519')",
      "assert.equal(password(run(cli, ['--input', 'fixtures/secret.zip.fixture.json', '--wordlist', 'fixtures/candidates.txt']).stdout), oracle)",
      "const parsed = JSON.parse(run(cli, ['--input', 'fixtures/secret.zip.fixture.json', '--wordlist', 'fixtures/candidates.txt', '--json']).stdout)",
      "assert.equal(parsed.password, oracle)",
      "assert.match(run(cli, ['--input', 'fixtures/bruteforce.zip.fixture.json', '--charset', 'abc', '--max-len', '3']).stdout, /cab/)",
      "assert.match(`${run(cli, ['--wordlist', 'fixtures/candidates.txt'], 2).stderr}`, /input/i)",
      "fs.mkdirSync(path.dirname(reportPath), { recursive: true })",
      "fs.writeFileSync(reportPath, JSON.stringify({ ok: true, oracle: { dictionary_password: oracle } }, null, 2))",
      "console.log('ZIP_PASSWORD_REFACTOR_ACCEPTANCE_OK')",
      "",
    ].join("\n"),
  );
}

function caseDefinition(caseName, ctx) {
  const sentinel = `TURA_TUI_LOCAL_${caseName.replaceAll("-", "_").toUpperCase()}_${ctx.runId}`;
  if (caseName === "single-request") {
    return {
      sentinel,
      prompt: [
        "Use command_run to create single_request_result.txt in the current workspace.",
        `The file must contain this marker: ${sentinel}`,
        "Run a shell command to verify the file exists and contains the marker.",
        `Final answer must include this marker: ${sentinel}`,
      ].join("\n"),
      validate: async () => {
        const file = path.join(ctx.workspace, "single_request_result.txt");
        const text = await readText(file);
        return [
          check("single_request_result.txt exists", fs.existsSync(file)),
          check("single request marker written", text.includes(sentinel)),
        ];
      },
    };
  }
  if (caseName === "snake") {
    return {
      sentinel,
      prompt: [
        "Create and verify a minimal browser Snake game in this empty workspace.",
        "Create snake.html and tools/snake_playwright.mjs.",
        "The verifier must press ArrowRight and ArrowDown, check movement, score UI, restart, and no horizontal overflow.",
        "Run the verifier with node and save desktop.png and mobile.png in the workspace root.",
        "Fix and rerun until the verifier passes.",
        `Final answer must include exactly this marker: ${sentinel}`,
      ].join("\n"),
      validate: async () =>
        [
          ["snake.html", 500],
          [path.join("tools", "snake_playwright.mjs"), 500],
          ["desktop.png", 1_000],
          ["mobile.png", 1_000],
        ].map(([relative, minSize]) => {
          const file = path.join(ctx.workspace, relative);
          return check(`${relative} exists and is non-empty`, fileSize(file) >= minSize, {
            size: fileSize(file),
          });
        }),
    };
  }
  return {
    sentinel,
    prompt: [
      "Long CLI refactor task: rebuild the provided legacy zip-password-finder CLI.",
      "You are given legacy source under legacy_zip_password_cli/ and deterministic ZIP-password fixtures under fixtures/.",
      "Create zip_password_refactor/bin/zip-password-finder.mjs with dictionary mode, brute-force mode, --json, --help, and clear validation.",
      "Run node acceptance/zip_password_cli_acceptance.mjs until it prints ZIP_PASSWORD_REFACTOR_ACCEPTANCE_OK.",
      `Final answer must include exactly this marker: ${sentinel}`,
    ].join("\n"),
    validate: async () => {
      const cli = path.join(
        ctx.workspace,
        "zip_password_refactor",
        "bin",
        "zip-password-finder.mjs",
      );
      const acceptance = path.join(ctx.workspace, "acceptance", "zip_password_cli_acceptance.mjs");
      const report = path.join(ctx.workspace, "zip_password_refactor", "acceptance-report.json");
      const result = fs.existsSync(acceptance)
        ? spawnSync(process.execPath, [acceptance], {
            cwd: ctx.workspace,
            encoding: "utf8",
            timeout: 45_000,
            windowsHide: true,
          })
        : {
            status: 1,
            signal: "missing-acceptance",
            stdout: "",
            stderr: "missing acceptance script",
          };
      const reportValue = await readJson(report);
      return [
        check("refactored CLI exists", fileSize(cli) > 700, { size: fileSize(cli) }),
        check("acceptance script exists", fileSize(acceptance) > 1_000, {
          size: fileSize(acceptance),
        }),
        check("acceptance rerun exited 0", result.status === 0, {
          status: result.status,
          signal: result.signal,
          stdout: trimForSummary(result.stdout),
          stderr: trimForSummary(result.stderr),
        }),
        check("acceptance report ok", reportValue?.ok === true, { report }),
      ];
    },
  };
}

async function startDebugGateway(ctx) {
  const port = await freePort();
  const child = spawn(path.join(debugDir, `tura_gateway${exeSuffix}`), [], {
    cwd: ctx.workspace,
    env: {
      ...caseEnv(ctx),
      PORT: String(port),
      TURA_GATEWAY_PORT: String(port),
      TURA_GATEWAY_URL: `http://127.0.0.1:${port}`,
      TURA_ROUTER_STDERR_LOG: path.join(ctx.logs, "router.stderr.log"),
      TURA_RUNTIME_WORKER_STDERR_LOG: path.join(ctx.logs, "runtime-worker.stderr.log"),
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout?.pipe(fs.createWriteStream(path.join(ctx.logs, "gateway.stdout.log")));
  child.stderr?.pipe(fs.createWriteStream(path.join(ctx.logs, "gateway.stderr.log")));
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/global/health`, child, 60_000);
  return { child, url, port };
}

function caseEnv(ctx) {
  return {
    ...process.env,
    PATH: `${debugDir}${path.delimiter}${process.env.PATH || ""}`,
    TURA_HOME: ctx.turaHome,
    TURA_PROJECT_ROOT: repoRoot,
    TURA_PROVIDER_CONFIG:
      process.env.TURA_PROVIDER_CONFIG ||
      path.join(repoRoot, "crates", "provider", "config", "provider_config.json"),
    LOG_PATH: ctx.providerLogRoot,
    TURA_CWD: ctx.workspace,
    TURA_DEBUG_RUNTIME: process.env.TURA_DEBUG_RUNTIME || "1",
    FORCE_COLOR: process.env.FORCE_COLOR || "0",
  };
}

async function runLoggedProcess(command, args, ctx, options = {}) {
  const stdoutPath = path.join(
    ctx.logs,
    `${path.basename(command).replace(/[^\w.-]/g, "_")}.stdout.log`,
  );
  const stderrPath = path.join(
    ctx.logs,
    `${path.basename(command).replace(/[^\w.-]/g, "_")}.stderr.log`,
  );
  const started = Date.now();
  const child = spawn(command, args, {
    cwd: repoRoot,
    env: { ...caseEnv(ctx), ...(options.env || {}) },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    shell: false,
  });
  let stdout = "";
  let stderr = "";
  child.stdout?.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr?.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const timeoutMs = options.timeoutMs || caseTimeoutMs(ctx.caseName);
  const result = await new Promise((resolve) => {
    const timer = setTimeout(async () => {
      await stopProcess(child);
      resolve({ status: null, signal: "timeout" });
    }, timeoutMs);
    child.on("error", (error) => {
      clearTimeout(timer);
      stderr += `\n${error.stack || error.message || error}`;
      resolve({ status: 1, signal: "error" });
    });
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      resolve({ status, signal });
    });
  });
  await fsp.writeFile(stdoutPath, stdout);
  await fsp.writeFile(stderrPath, stderr);
  return {
    command,
    args,
    status: result.status,
    signal: result.signal,
    durationMs: Date.now() - started,
    stdout,
    stderr,
    stdoutPath,
    stderrPath,
  };
}

async function finishCase(ctx, definition, result, extras = {}) {
  const lastMessage = await readText(ctx.lastMessagePath);
  const finalAnswerText = [result?.stdout || "", result?.stderr || "", lastMessage].join("\n");
  const validation = await definition.validate();
  validation.push(
    check("process exited 0", result?.status === 0, {
      status: result?.status,
      signal: result?.signal,
    }),
  );
  validation.push(check("final marker observed", finalAnswerText.includes(definition.sentinel)));
  if (extras.cleanup)
    validation.push(check("backend daemons cleaned up", extras.cleanup.ok, extras.cleanup));
  const summary = {
    schema: "tura.business.tui-local.v1",
    ok: validation.every((item) => item.ok),
    surface: "tui",
    case_name: ctx.caseName,
    run_id: ctx.runId,
    run_root: ctx.runRoot,
    workspace: ctx.workspace,
    summary_path: ctx.summaryPath,
    entry: "node apps/tui/dist/index.js",
    debug_dir: debugDir,
    model: defaultModel,
    agent: defaultAgent,
    model_variant: defaultVariant,
    timeout_ms: caseTimeoutMs(ctx.caseName),
    sentinel: definition.sentinel,
    command: result?.command,
    args: result?.args,
    status: result?.status,
    signal: result?.signal,
    duration_ms: result?.durationMs,
    stdout_path: result?.stdoutPath,
    stderr_path: result?.stderrPath,
    last_message_path: ctx.lastMessagePath,
    validation,
    ...extras,
  };
  await fsp.writeFile(ctx.summaryPath, JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  if (!summary.ok) process.exitCode = 1;
  return summary;
}

async function shutdownBackendDaemons(ctx) {
  const routerAddrPath = path.join(ctx.turaHome, "db", "session_log", "router.addr");
  const serviceAddrPath = path.join(ctx.turaHome, "db", "session_log", "service.addr");
  const result = {
    router_addr_path: routerAddrPath,
    service_addr_path: serviceAddrPath,
    requested: false,
    ok: true,
  };
  const endpoint = await readJson(routerAddrPath);
  const serviceEndpoint = await readJson(serviceAddrPath);
  if (endpoint?.addr) {
    result.requested = true;
    try {
      await callRouter(endpoint.addr, {
        request_id: "tui-local-business-cleanup",
        kind: "call",
        method: "execution.shutdown",
        payload: {},
      });
    } catch (error) {
      result.error = String(error?.stack || error?.message || error);
    }
  }
  result.ok = await waitForBackendAddrFilesGone(routerAddrPath, serviceAddrPath, 15_000);
  if (!result.ok) {
    const liveAddrs = [];
    for (const addr of [endpoint?.addr, serviceEndpoint?.addr].filter(Boolean)) {
      if (await canConnect(addr)) liveAddrs.push(addr);
    }
    result.live_addrs = liveAddrs;
    if (!liveAddrs.length) {
      result.stale_addr_files_removed = true;
      await fsp.rm(routerAddrPath, { force: true });
      await fsp.rm(serviceAddrPath, { force: true });
      result.ok = true;
    }
  }
  return result;
}

async function waitForBackendAddrFilesGone(routerAddrPath, serviceAddrPath, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!fs.existsSync(routerAddrPath) && !fs.existsSync(serviceAddrPath)) return true;
    await delay(250);
  }
  return !fs.existsSync(routerAddrPath) && !fs.existsSync(serviceAddrPath);
}

async function canConnect(addr) {
  const parsed = parseHostPort(addr);
  return new Promise((resolve) => {
    const socket = net.createConnection(parsed.port, parsed.host);
    const timer = setTimeout(() => {
      socket.destroy();
      resolve(false);
    }, 500);
    socket.on("connect", () => {
      clearTimeout(timer);
      socket.end();
      resolve(true);
    });
    socket.on("error", () => {
      clearTimeout(timer);
      resolve(false);
    });
  });
}

async function callRouter(addr, payload) {
  const parsed = parseHostPort(addr);
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(parsed.port, parsed.host);
    let raw = "";
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("router cleanup timeout"));
    }, 10_000);
    socket.on("connect", () => socket.write(`${JSON.stringify(payload)}\n`));
    socket.on("data", (chunk) => {
      raw += chunk.toString();
      if (raw.includes("\n")) {
        clearTimeout(timer);
        socket.end();
        resolve(JSON.parse(raw.trim()));
      }
    });
    socket.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

async function waitForUrl(url, child, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    if (child?.exitCode !== null)
      throw new Error(`${url} exited before readiness with ${child.exitCode}`);
    try {
      const response = await fetch(url);
      if (response.ok) return response;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw lastError || new Error(`Timed out waiting for ${url}`);
}

async function stopProcess(child) {
  if (!child || child.killed || child.exitCode !== null) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
    return;
  }
  try {
    child.kill("SIGTERM");
  } catch {
    // Best-effort process cleanup.
  }
  await Promise.race([new Promise((resolve) => child.once("exit", resolve)), delay(2_000)]);
  if (child.exitCode === null) {
    try {
      child.kill("SIGKILL");
    } catch {
      // Best-effort process cleanup.
    }
  }
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

function caseTimeoutMs(caseName) {
  return Number(
    process.env.TURA_BUSINESS_TIMEOUT_MS || defaultTimeoutMsByCase[caseName] || 180_000,
  );
}

function check(name, ok, details = {}) {
  return { name, ok, ...details };
}

function fileSize(file) {
  try {
    return fs.statSync(file).size;
  } catch {
    return 0;
  }
}

async function readText(file) {
  try {
    return await fsp.readFile(file, "utf8");
  } catch {
    return "";
  }
}

async function readJson(file) {
  try {
    return JSON.parse(await fsp.readFile(file, "utf8"));
  } catch {
    return undefined;
  }
}

function parseHostPort(addr) {
  const parsed = new URL(`tcp://${addr}`);
  return { host: parsed.hostname, port: Number(parsed.port) };
}

function trimForSummary(value) {
  return String(value || "").slice(-2000);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function timestamp() {
  return new Date().toISOString().replace(/[-:]/g, "").replace(/\..+$/u, "").replace("T", "-");
}
