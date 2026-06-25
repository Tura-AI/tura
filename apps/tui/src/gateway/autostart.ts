import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { TerminalCapabilities } from "../tui/capabilities.js";
import { TUI_DRAW_INTERVAL_MS } from "../tui/frame-rate.js";
import { iconAnimationFrame } from "../tui/render/busy-animation.js";
import { t } from "../i18n.js";

// Tracks the gateway spawned by this TUI instance. Undefined when reusing an
// already-running gateway that belongs to another session.
let ownedGatewayProcess: ChildProcess | undefined;
let ownedGatewayShutdownMode: "stdin-eof" | "kill" | undefined;

/**
 * Kill the gateway that this TUI instance started, if any.
 * Safe to call multiple times; a no-op when no gateway is owned.
 * The shared backend is owned by the detached router, not this gateway front.
 */
export function killOwnedGateway(): void {
  const proc = ownedGatewayProcess;
  if (proc) {
    ownedGatewayProcess = undefined;
    const shutdownMode = ownedGatewayShutdownMode;
    ownedGatewayShutdownMode = undefined;
    try {
      if (shutdownMode === "stdin-eof") {
        proc.stdin?.end();
        const timer = setTimeout(() => {
          if (proc.exitCode === null && !proc.killed) {
            proc.kill();
          }
        }, 5_000);
        timer.unref();
      } else {
        proc.kill();
      }
    } catch {
      // Already dead — nothing to do.
    }
  }
}

/** For testing only: inject a process as the owned gateway. */
export function _setOwnedGatewayForTest(
  child: ChildProcess | undefined,
  shutdownMode: "stdin-eof" | "kill" = "kill",
): void {
  ownedGatewayProcess = child;
  ownedGatewayShutdownMode = child ? shutdownMode : undefined;
}

type StartupStep = "checking" | "starting" | "waiting";

const HEALTH_TIMEOUT_MS = 20_000;
const HEALTH_POLL_INTERVAL_MS = 500;

const gatewayBinaryName = process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway";

interface GatewayIdentity {
  root: string;
  version?: string;
}

interface GatewayStartupState {
  exitCode?: number | null;
  exitSignal?: string | null;
  spawnError?: unknown;
}

/**
 * Ensure a gateway this package can use is running, and return the URL to talk
 * to it on.
 *
 * Behaviour:
 *  - If the fixed (per-package) port already serves *our own* directory's
 *    gateway, reuse it.
 *  - If that port is occupied by a foreign gateway or process, fail clearly.
 *  - If nothing is listening, start our own gateway on the fixed port.
 */
export async function ensureGatewayAvailable(
  gatewayUrl: string,
  capabilities: TerminalCapabilities,
  _dev?: boolean,
  explicit?: boolean,
): Promise<string> {
  const desiredUrl = stripTrailingSlash(gatewayUrl);
  const myRoot = packageRoot();
  const resolved = resolveGatewayBinary(myRoot);
  const binary = resolved.binary;
  const instanceHome = process.env.TURA_HOME?.trim() ? canonical(process.env.TURA_HOME) : myRoot;
  const mode = gatewayMode(binary);
  let identity = await gatewayIdentityWithProbeTimeout(desiredUrl);
  // An explicitly chosen URL (flag / TURA_GATEWAY_URL) is trusted as-is: if a
  // Tura gateway answers there, reuse it regardless of its reported root. The
  // root identity check only guards reuse of the default auto-discovered port.
  if (identity && (explicit || isOwnGateway(identity, myRoot))) {
    return desiredUrl;
  }

  if (!identity && !(await canBindGatewayUrl(desiredUrl))) {
    identity = await waitForGatewayIdentity(desiredUrl, HEALTH_TIMEOUT_MS);
    if (identity && (explicit || isOwnGateway(identity, myRoot))) return desiredUrl;
    await terminateGatewayFromLock(instanceHome, mode, desiredUrl);
    identity = await waitForGatewayIdentity(desiredUrl, HEALTH_POLL_INTERVAL_MS);
    if (identity && (explicit || isOwnGateway(identity, myRoot))) return desiredUrl;
  }

  if (identity || !(await canBindGatewayUrl(desiredUrl))) {
    throw new Error(
      `gateway URL ${desiredUrl} is occupied by a foreign process; set --gateway-url/TURA_GATEWAY_URL to an explicit Tura gateway or stop the foreign process`,
    );
  }

  if (!binary) {
    throw new Error(t("gatewayMissingBinary"));
  }

  const launchBinary = binary;
  const port = portOf(desiredUrl);
  const spawnPort = port || process.env.PORT;
  const startupState: GatewayStartupState = {};
  for (let attempt = 0; attempt < 2; attempt += 1) {
    startupState.exitCode = undefined;
    startupState.exitSignal = undefined;
    startupState.spawnError = undefined;
    await runWithSpinner({
      step: "starting",
      text: t("gatewayStarting"),
      capabilities,
      run: async () => {
        const child = spawn(launchBinary, [], {
          cwd: myRoot,
          // stdin is a front lifetime lease: gateway exits when the owning TUI
          // closes this pipe or dies; router observes heartbeat expiry and
          // performs backend idle shutdown itself.
          stdio: ["pipe", "ignore", "ignore"],
          windowsHide: true,
          env: {
            ...process.env,
            ...(spawnPort ? { PORT: spawnPort } : {}),
            TURA_HOME: instanceHome,
            TURA_PROJECT_ROOT: myRoot,
            TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF: "1",
          },
        });
        child.on("error", (error) => {
          startupState.spawnError = error;
        });
        child.on("exit", (code, signal) => {
          startupState.exitCode = code;
          startupState.exitSignal = signal;
        });
        // unref so the event loop can exit normally even while gateway is alive.
        child.unref();
        (child.stdin as (NodeJS.WritableStream & { unref?: () => void }) | null)?.unref?.();
        ownedGatewayProcess = child;
        ownedGatewayShutdownMode = "stdin-eof";
      },
    });

    if (await waitForGateway(desiredUrl, capabilities, startupState, instanceHome, mode)) {
      return desiredUrl;
    }
  }
  throw new Error(t("gatewayStartTimeout"));
}

interface ResolvedGateway {
  /** Path to an existing gateway binary, when one was found. */
  binary?: string;
}

function resolveGatewayBinary(myRoot: string): ResolvedGateway {
  const candidates: string[] = [];
  const override = process.env.TURA_GATEWAY_BIN;
  if (override) candidates.push(override);
  const profile = currentBuildMode();
  const execDir = dirname(process.execPath);
  const repo = findRepoRoot();
  candidates.push(join(execDir, gatewayBinaryName));
  candidates.push(join(myRoot, gatewayBinaryName));
  candidates.push(
    join(repo, "target", profile === "release" ? "release" : "debug", gatewayBinaryName),
  );

  for (const candidate of candidates) {
    if (candidate && existsSync(candidate)) return { binary: candidate };
  }
  return {};
}

async function waitForGatewayIdentity(
  gatewayUrl: string,
  timeoutMs: number,
): Promise<GatewayIdentity | null> {
  const deadline = Date.now() + timeoutMs;
  do {
    const identity = await gatewayIdentityWithProbeTimeout(gatewayUrl);
    if (identity) return identity;
    await delay(HEALTH_POLL_INTERVAL_MS);
  } while (Date.now() < deadline);
  return null;
}

async function gatewayIdentityWithProbeTimeout(
  gatewayUrl: string,
): Promise<GatewayIdentity | null> {
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), HEALTH_POLL_INTERVAL_MS);
    const response = await fetch(`${stripTrailingSlash(gatewayUrl)}/global/health`, {
      signal: controller.signal,
    });
    clearTimeout(timer);
    if (!response.ok) return null;
    const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;
    return {
      root: typeof body.root === "string" ? body.root : "",
      version: typeof body.version === "string" ? body.version : undefined,
    };
  } catch {
    return null;
  }
}

function isOwnGateway(identity: GatewayIdentity, myRoot: string): boolean {
  // Older gateways report no root; assume the fixed port belongs to us.
  if (!identity.root) return true;
  return sameRoot(identity.root, myRoot);
}

function packageRoot(): string {
  const fromEnv = process.env.TURA_PROJECT_ROOT;
  if (fromEnv && existsSync(fromEnv)) return canonical(fromEnv);
  return canonical(findRepoRoot());
}

function sameRoot(left: string, right: string): boolean {
  return normalizeRoot(left) === normalizeRoot(right);
}

function normalizeRoot(value: string): string {
  // Strip the Windows verbatim prefix the gateway's canonicalize() may emit.
  let normalized = canonical(value).replace(/^\\\\\?\\(UNC\\)?/u, (_m, unc) => (unc ? "\\\\" : ""));
  normalized = normalized.replace(/[\\/]+$/u, "");
  if (process.platform === "win32") normalized = normalized.toLowerCase();
  return normalized;
}

function canonical(value: string): string {
  try {
    return realpathSync(value);
  } catch {
    return resolve(value);
  }
}

async function waitForGateway(
  gatewayUrl: string,
  capabilities: TerminalCapabilities,
  startupState?: GatewayStartupState,
  instanceHome?: string,
  mode?: "dev" | "release",
): Promise<boolean> {
  let becameHealthy = false;
  await runWithSpinner({
    step: "waiting",
    text: t("gatewayWaiting"),
    capabilities,
    run: async (tick) => {
      const deadline = Date.now() + HEALTH_TIMEOUT_MS;
      while (Date.now() < deadline) {
        if (startupState?.spawnError) {
          throw startupState.spawnError instanceof Error
            ? startupState.spawnError
            : new Error(String(startupState.spawnError));
        }
        if (startupState && startupState.exitCode !== undefined) {
          const detail = startupState.exitSignal
            ? `by signal ${startupState.exitSignal}`
            : `exit code ${startupState.exitCode ?? "unknown"}`;
          throw new Error(`${t("gatewayStartTimeout")} (${detail})`);
        }
        if (await healthOk(gatewayUrl)) {
          becameHealthy = true;
          return;
        }
        tick();
        await delay(TUI_DRAW_INTERVAL_MS);
      }
      const killed =
        instanceHome && mode
          ? await terminateGatewayFromLock(instanceHome, mode, gatewayUrl)
          : false;
      const killedOwned = await forceKillOwnedGateway();
      if (!killed && !killedOwned) {
        throw new Error(`${t("gatewayStartTimeout")} (failed to kill unhealthy gateway)`);
      }
    },
  });
  return becameHealthy;
}

async function healthOk(gatewayUrl: string): Promise<boolean> {
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), HEALTH_POLL_INTERVAL_MS);
    const response = await fetch(`${gatewayUrl.replace(/\/+$/u, "")}/global/health`, {
      signal: controller.signal,
    });
    clearTimeout(timer);
    return response.ok;
  } catch {
    return false;
  }
}

interface GatewayLockRecord {
  pid?: number;
  processStartTime?: number;
  kind?: string;
  mode?: string;
  port?: string;
  root?: string;
}

async function terminateGatewayFromLock(
  instanceHome: string,
  mode: "dev" | "release",
  gatewayUrl: string,
): Promise<boolean> {
  const port = portOf(gatewayUrl);
  if (!port) return false;
  const record = readGatewayLock(instanceHome, mode);
  if (!record?.pid || !record.processStartTime) return false;
  if (record.kind !== "gateway" || record.mode !== mode || record.port !== port) return false;
  if (!record.root || !sameRoot(record.root, instanceHome)) return false;
  const currentStart = await processStartTime(record.pid);
  if (currentStart === undefined || currentStart !== record.processStartTime) return false;
  killProcessTree(record.pid);
  await waitForProcessExit(record.pid, 5_000);
  return !isProcessAlive(record.pid);
}

function readGatewayLock(
  instanceHome: string,
  mode: "dev" | "release",
): GatewayLockRecord | undefined {
  const path = join(instanceHome, ".tura", "locks", `gateway-${mode}.lock`);
  let raw = "";
  try {
    raw = readFileSync(path, "utf8");
  } catch {
    return undefined;
  }
  const record: GatewayLockRecord = {};
  for (const line of raw.split(/\r?\n/u)) {
    const index = line.indexOf("=");
    if (index < 0) continue;
    const key = line.slice(0, index).trim();
    const value = line.slice(index + 1).trim();
    if (key === "pid") record.pid = Number(value);
    else if (key === "process_start_time") record.processStartTime = Number(value);
    else if (key === "kind") record.kind = value;
    else if (key === "mode") record.mode = value;
    else if (key === "port") record.port = value;
    else if (key === "root") record.root = value;
  }
  return record;
}

function killProcessTree(pid: number): void {
  if (process.platform === "win32") {
    spawn("taskkill", ["/pid", String(pid), "/t", "/f"], { stdio: "ignore", windowsHide: true });
    return;
  }
  try {
    process.kill(pid, "SIGTERM");
  } catch {
    return;
  }
  setTimeout(() => {
    if (isProcessAlive(pid)) {
      try {
        process.kill(pid, "SIGKILL");
      } catch {
        // Already exited.
      }
    }
  }, 2_000).unref();
}

async function forceKillOwnedGateway(): Promise<boolean> {
  const proc = ownedGatewayProcess;
  const pid = proc?.pid;
  ownedGatewayProcess = undefined;
  ownedGatewayShutdownMode = undefined;
  if (!pid) return false;
  killProcessTree(pid);
  await waitForProcessExit(pid, 5_000);
  return !isProcessAlive(pid);
}

async function waitForProcessExit(pid: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline && isProcessAlive(pid)) {
    await delay(100);
  }
}

function isProcessAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function processStartTime(pid: number): Promise<number | undefined> {
  if (process.platform === "win32") {
    return processStartTimeWindows(pid);
  }
  return processStartTimeUnix(pid);
}

async function processStartTimeWindows(pid: number): Promise<number | undefined> {
  const script = `$p = Get-CimInstance Win32_Process -Filter "ProcessId=${pid}"; if ($p) { ([DateTimeOffset]$p.CreationDate).ToUnixTimeSeconds() }`;
  const output = await collectProcessOutput("powershell", ["-NoProfile", "-Command", script]);
  const startTime = Number(output.trim());
  return Number.isFinite(startTime) ? startTime : undefined;
}

async function processStartTimeUnix(pid: number): Promise<number | undefined> {
  const output = await collectProcessOutput("ps", ["-o", "lstart=", "-p", String(pid)]);
  const date = Date.parse(output.trim());
  return Number.isFinite(date) ? Math.floor(date / 1000) : undefined;
}

function collectProcessOutput(command: string, args: string[]): Promise<string> {
  return new Promise((resolveOutput) => {
    const child = spawn(command, args, { stdio: ["ignore", "pipe", "ignore"], windowsHide: true });
    let output = "";
    child.stdout?.on("data", (chunk: Buffer) => {
      output += chunk.toString("utf8");
    });
    child.on("error", () => resolveOutput(""));
    child.on("exit", () => resolveOutput(output));
  });
}

function gatewayMode(binary: string | undefined): "dev" | "release" {
  if (!binary) return currentBuildMode();
  const normalized = binary.replace(/\\/g, "/").toLowerCase();
  if (normalized.includes("/target/debug/")) return "dev";
  if (normalized.includes("/target/release/")) return "release";
  return currentBuildMode();
}

function currentBuildMode(): "dev" | "release" {
  if (process.env.TURA_BUILD_KIND === "release") return "release";
  const normalized = process.execPath.replace(/\\/g, "/").toLowerCase();
  return normalized.includes("/target/release/") ? "release" : "dev";
}

async function canBindGatewayUrl(gatewayUrl: string): Promise<boolean> {
  const port = Number(portOf(gatewayUrl));
  if (!Number.isInteger(port) || port <= 0) return true;
  return new Promise((resolveBind) => {
    const server = createServer();
    server.unref();
    server.once("error", () => resolveBind(false));
    server.listen(port, "127.0.0.1", () => {
      server.close(() => resolveBind(true));
    });
  });
}

export const _canBindGatewayUrlForTest = canBindGatewayUrl;

async function runWithSpinner(options: {
  step: StartupStep;
  text: string;
  capabilities: TerminalCapabilities;
  run: (tick: () => void) => Promise<void>;
}): Promise<void> {
  let frame = 0;
  const frames = options.capabilities.unicode
    ? ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "⠋", "⠙", "⠹", "⠸", "⠼"]
    : ["|", "/", "-", "\\", "-", "/"];
  const draw = () => {
    if (!process.stdout.isTTY) return;
    const iconFrame = iconAnimationFrame(frame);
    const prefix = frames[iconFrame % frames.length];
    frame += 1;
    const line = `${prefix} ${options.text}`;
    process.stdout.write(options.capabilities.cursorControl ? `\r\x1b[2K${line}` : `${line}\n`);
  };
  draw();
  const timer = setInterval(draw, TUI_DRAW_INTERVAL_MS);
  try {
    await options.run(draw);
    if (process.stdout.isTTY) {
      const done = `✓ ${options.text}`;
      process.stdout.write(
        options.capabilities.cursorControl && options.capabilities.unicode
          ? `\r\x1b[2K${done}\n`
          : "\n",
      );
    }
  } finally {
    clearInterval(timer);
  }
}

function findRepoRoot(): string {
  const starts = [
    process.cwd(),
    dirname(process.execPath),
    dirname(fileURLToPath(import.meta.url)),
  ];
  for (const start of starts) {
    let current = resolve(start);
    for (let depth = 0; depth < 8; depth += 1) {
      if (isRuntimeRoot(current)) return current;
      const parent = dirname(current);
      if (parent === current) break;
      current = parent;
    }
  }
  return process.cwd();
}

function isRuntimeRoot(candidate: string): boolean {
  return (
    (existsSync(join(candidate, "Cargo.toml")) &&
      existsSync(join(candidate, "crates", "gateway"))) ||
    (existsSync(join(candidate, "agents", "src")) &&
      existsSync(join(candidate, "personas", "src"))) ||
    existsSync(join(candidate, "config", "provider_config.json"))
  );
}

function portOf(gatewayUrl: string): string | undefined {
  try {
    return new URL(gatewayUrl).port || undefined;
  } catch {
    return undefined;
  }
}

function stripTrailingSlash(value: string): string {
  return value.replace(/\/+$/u, "");
}

function delay(ms: number): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, ms));
}
