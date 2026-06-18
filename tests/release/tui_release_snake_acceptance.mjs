#!/usr/bin/env node
import { createRequire } from "node:module";
import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..", "..");
const tuiRequire = createRequire(
  path.join(repoRoot, "apps", "tui", "package.json"),
);
const pty = tuiRequire("node-pty");
const exeSuffix = process.platform === "win32" ? ".exe" : "";
const binaryProfile = process.env.TURA_BUSINESS_BINARY_PROFILE || "release";
const binaryDir = path.join(repoRoot, "target", binaryProfile);
const turaExe = path.join(binaryDir, `tura${exeSuffix}`);
const runId =
  process.env.TURA_BUSINESS_RUN_ID || `tui-snake-normal-${timestamp()}`;
const targetRoot =
  process.env.TURA_BUSINESS_TARGET_ROOT ||
  process.env.COMMAND_RUN_BUSINESS_TARGET_ROOT ||
  path.join(repoRoot, "apps", "tui", "test-results", "release");
const runRoot = path.join(
  targetRoot,
  binaryProfile,
  "tui",
  "snake-normal",
  runId,
);
const workspace = path.join(runRoot, "workspace");
const logs = path.join(runRoot, "logs");
const summaryPath = path.join(runRoot, "summary.json");
const turaHome = path.join(runRoot, "tura-home");
const timeoutMs = Number(
  process.env.TURA_BUSINESS_SNAKE_TIMEOUT_MS ||
    process.env.TURA_BUSINESS_TIMEOUT_MS ||
    240_000,
);
const sentinel = `TURA_TUI_SNAKE_NORMAL_${runId}`;

const prompt = [
  "Create and verify a minimal browser Snake game in this empty workspace.",
  "Create snake.html with a playable canvas snake game.",
  "Create tools/snake_playwright.mjs that opens snake.html with Playwright.",
  "The verifier must press ArrowRight and ArrowDown, check movement, score UI, restart, and no horizontal overflow.",
  "Run the verifier with node and make it save desktop.png and mobile.png in the workspace root.",
  "If the verifier fails, fix the game or verifier and rerun it until it passes.",
  "Do not include the final marker until snake.html, tools/snake_playwright.mjs, desktop.png, and mobile.png all exist and the verifier exits 0.",
  `Final answer must include exactly this marker: ${sentinel}.`,
  "Final answer must mention snake.html, tools/snake_playwright.mjs, desktop.png, mobile.png, ArrowRight, ArrowDown, score, restart, and no horizontal overflow.",
].join(" ");

const checks = [];
const interactiveEvents = [];

await main();

async function main() {
  assertReleaseArtifacts();
  await fsp.rm(runRoot, { recursive: true, force: true });
  await fsp.mkdir(workspace, { recursive: true });
  await fsp.mkdir(logs, { recursive: true });
  await exposeWorkspaceNodeModules();
  await writeWorkspaceConfig();
  await fsp.writeFile(
    path.join(workspace, "BUSINESS_TEST_CONTEXT.md"),
    [
      "# TUI release normal startup snake acceptance",
      "",
      "This workspace belongs to a release test that starts Tura TUI with no CLI arguments.",
      "Do not ask follow-up questions.",
      "",
    ].join("\n"),
  );

  const result = await runInteractiveTui();
  const cleanup = await shutdownBackendDaemons();
  await writeSummary(result, cleanup);
  if (!checks.every((check) => check.ok)) process.exitCode = 1;
}

function assertReleaseArtifacts() {
  const required = [
    "tura",
    "tura_gateway",
    "tura_router",
    "tura_runtime",
    "tura_session_db",
    "tura_exec",
  ];
  const missing = required
    .map((name) => path.join(binaryDir, `${name}${exeSuffix}`))
    .filter((candidate) => !fs.existsSync(candidate));
  if (missing.length) {
    throw new Error(
      `Missing release artifacts:\n${missing.map((item) => `- ${item}`).join("\n")}`,
    );
  }
}

async function exposeWorkspaceNodeModules() {
  const candidates = [
    path.join(repoRoot, "apps", "tui", "node_modules"),
    path.join(repoRoot, "apps", "gui", "node_modules"),
    path.join(repoRoot, "node_modules"),
  ];
  const source = candidates.find((candidate) =>
    fs.existsSync(path.join(candidate, "playwright")),
  );
  if (!source) return;
  const destination = path.join(workspace, "node_modules");
  try {
    await fsp.symlink(
      source,
      destination,
      process.platform === "win32" ? "junction" : "dir",
    );
  } catch {
    await fsp.cp(source, destination, { recursive: true });
  }
}

async function writeWorkspaceConfig() {
  const configDir = path.join(workspace, ".tura");
  await fsp.mkdir(configDir, { recursive: true });
  await fsp.writeFile(
    path.join(configDir, "config.conf"),
    [
      `model=${process.env.TURA_BUSINESS_MODEL || "codex/gpt-5.5"}`,
      "active_provider=codex",
      "active_model=gpt-5.5",
      `active_agent=${process.env.TURA_BUSINESS_AGENT || "fast"}`,
      "session_type=coding",
      `model_variant=${process.env.TURA_BUSINESS_MODEL_VARIANT || "low"}`,
      "model_acceleration_enabled=true",
      "kill_processes_on_start=false",
      "validator_enabled=false",
      "force_planning=false",
      "",
    ].join("\n"),
  );
}

async function runInteractiveTui() {
  const stdoutPath = path.join(logs, "tura-tui.stdout.log");
  let output = "";
  const child = pty.spawn(turaExe, [], {
    name: "xterm-256color",
    cols: 120,
    rows: 40,
    cwd: workspace,
    env: {
      ...process.env,
      TURA_HOME: turaHome,
      TURA_PROJECT_ROOT: repoRoot,
      FORCE_COLOR: "0",
      NO_COLOR: "1",
    },
  });
  interactiveEvents.push({
    name: "spawn-no-args",
    command: turaExe,
    args: [],
    cwd: workspace,
  });
  child.onData((chunk) => {
    output += chunk;
  });

  await waitForOutput(
    () => output,
    /tura|回车输入|Enter|目录:|Directory:/i,
    45_000,
  );
  interactiveEvents.push({ name: "initial-tui-rendered" });
  await typeText(child, prompt);
  interactiveEvents.push({ name: "typed-prompt", chars: prompt.length });
  child.write("\r");
  interactiveEvents.push({ name: "pressed-enter" });

  const deadline = Date.now() + timeoutMs;
  let observedMarker = false;
  let observedFiles = false;
  while (Date.now() < deadline) {
    observedMarker ||= output.includes(sentinel);
    observedFiles = requiredFilesReady();
    if (observedMarker && observedFiles) break;
    await delay(1_000);
  }

  child.write("\x03");
  await waitForExit(child, 10_000).catch(() => {
    try {
      child.kill();
    } catch {
      // already gone
    }
  });
  await fsp.writeFile(stdoutPath, output);
  record("started release TUI with no arguments", true, {
    command: turaExe,
    args: [],
  });
  record("final marker observed in TUI output", observedMarker);
  validateRequiredFiles();
  return {
    command: turaExe,
    args: [],
    status: observedMarker && observedFiles ? 0 : 1,
    stdoutPath,
    outputTail: tail(output),
  };
}

function requiredFilesReady() {
  return (
    fileSize(path.join(workspace, "snake.html")) >= 500 &&
    fileSize(path.join(workspace, "tools", "snake_playwright.mjs")) >= 500 &&
    fileSize(path.join(workspace, "desktop.png")) >= 1_000 &&
    fileSize(path.join(workspace, "mobile.png")) >= 1_000
  );
}

function validateRequiredFiles() {
  for (const [relative, minSize] of [
    ["snake.html", 500],
    [path.join("tools", "snake_playwright.mjs"), 500],
    ["desktop.png", 1_000],
    ["mobile.png", 1_000],
  ]) {
    const file = path.join(workspace, relative);
    record(`${relative} exists and is non-empty`, fileSize(file) >= minSize, {
      size: fileSize(file),
    });
  }
}

async function shutdownBackendDaemons() {
  const routerAddrPath = path.join(
    turaHome,
    "db",
    "session_log",
    "router.addr",
  );
  const serviceAddrPath = path.join(
    turaHome,
    "db",
    "session_log",
    "service.addr",
  );
  const result = {
    router_addr_path: routerAddrPath,
    service_addr_path: serviceAddrPath,
    requested: false,
    ok: false,
  };
  const endpoint = await readJson(routerAddrPath);
  if (endpoint?.addr) {
    result.requested = true;
    try {
      await callRouter(endpoint.addr, {
        request_id: "release-tui-normal-cleanup",
        kind: "call",
        method: "execution.shutdown",
        payload: {},
      });
    } catch (error) {
      result.error = String(error?.stack || error?.message || error);
    }
  }
  await delay(1_000);
  result.ok = !fs.existsSync(routerAddrPath) && !fs.existsSync(serviceAddrPath);
  record("backend daemons cleaned up", result.ok, result);
  return result;
}

async function callRouter(addr, payload) {
  const net = await import("node:net");
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

function parseHostPort(addr) {
  const parsed = new URL(`tcp://${addr}`);
  return { host: parsed.hostname, port: Number(parsed.port) };
}

async function writeSummary(result, cleanup) {
  const summary = {
    schema: "tura.business.release-entry.v1",
    ok: checks.every((check) => check.ok),
    surface: "tui",
    case_name: "snake-normal-startup",
    run_id: runId,
    run_root: runRoot,
    workspace,
    summary_path: summaryPath,
    binary_profile: binaryProfile,
    binary_dir: binaryDir,
    sentinel,
    command: result.command,
    args: result.args,
    status: result.status,
    stdout_path: result.stdoutPath,
    validation: checks,
    interactive_events: interactiveEvents,
    cleanup,
    output_tail: result.outputTail,
  };
  await fsp.writeFile(summaryPath, JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
}

function record(name, ok, details = {}) {
  checks.push({ name, ok, ...details });
}

function fileSize(file) {
  try {
    return fs.statSync(file).size;
  } catch {
    return 0;
  }
}

async function readJson(file) {
  try {
    return JSON.parse(await fsp.readFile(file, "utf8"));
  } catch {
    return undefined;
  }
}

function waitForOutput(read, pattern, timeout) {
  const deadline = Date.now() + timeout;
  return new Promise((resolve, reject) => {
    const timer = setInterval(() => {
      if (pattern.test(read())) {
        clearInterval(timer);
        resolve();
      } else if (Date.now() > deadline) {
        clearInterval(timer);
        reject(
          new Error(`timed out waiting for TUI output matching ${pattern}`),
        );
      }
    }, 100);
  });
}

function waitForExit(child, timeout) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("timed out waiting for TUI exit")),
      timeout,
    );
    child.onExit((event) => {
      clearTimeout(timer);
      resolve(event);
    });
  });
}

async function typeText(child, text) {
  for (const char of Array.from(text)) {
    child.write(char);
    await delay(2);
  }
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function tail(value) {
  return value.replace(/\x1b\[[0-?]*[ -/]*[@-~]/gu, "").slice(-4000);
}

function timestamp() {
  return new Date()
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\..+$/u, "")
    .replace("T", "-");
}
