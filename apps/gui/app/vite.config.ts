import { spawn } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [turaGatewayStartupPlugin(), solid()],
  server: {
    host: "127.0.0.1",
    port: 5174,
    strictPort: true,
  },
});

// The GUI dev server starts the repo-local target/debug/tura_gateway on port 4126.
const isWindows = process.platform === "win32";
const gatewayBinaryName = isWindows ? "tura_gateway.exe" : "tura_gateway";
const DEV_GATEWAY_URL = "http://127.0.0.1:4126";
const HEALTH_TIMEOUT_MS = 20_000;
const HEALTH_POLL_INTERVAL_MS = 500;
let gatewayStartupPromise: Promise<void> | undefined;
let ownedGatewayChild: ReturnType<typeof spawn> | undefined;
let ownedGatewayShutdownMode: "stdin-eof" | "kill" | undefined;

function turaGatewayStartupPlugin(): Plugin {
  return {
    name: "tura-gateway-startup",
    configureServer(server) {
      server.httpServer?.once("close", () => {
        killOwnedGateway();
      });
      server.middlewares.use("/__tura/start-gateway", async (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405;
          res.end("method not allowed");
          return;
        }
        try {
          const body = await readJsonBody(req);
          const gatewayUrl =
            typeof body.gatewayUrl === "string" ? body.gatewayUrl : DEV_GATEWAY_URL;
          if (
            !(await canBindGatewayUrl(gatewayUrl)) &&
            (await waitForHealth(gatewayUrl, HEALTH_TIMEOUT_MS))
          ) {
            writeJson(res, { ok: true, status: "connected" });
            return;
          }
          const root = repoRoot();
          const status = resolveGatewayBinary(root) ? "starting" : "building";
          gatewayStartupPromise ??= startGatewayTask(root, gatewayUrl).finally(() => {
            gatewayStartupPromise = undefined;
          });
          writeJson(res, { ok: true, status });
        } catch (error) {
          res.statusCode = 500;
          writeJson(res, {
            ok: false,
            error: error instanceof Error ? error.message : String(error),
          });
        }
      });
    },
  };
}

function resolveGatewayBinary(root: string): string | undefined {
  const candidates = [join(root, "target", "debug", gatewayBinaryName)];
  return candidates.find((candidate) => existsSync(candidate));
}

async function startGatewayTask(root: string, gatewayUrl: string): Promise<void> {
  if (!(await canBindGatewayUrl(gatewayUrl))) {
    if (await waitForHealth(gatewayUrl, HEALTH_TIMEOUT_MS)) return;
    await terminateGatewayFromLock(instanceHome(root), "dev", gatewayUrl);
  }
  if (await waitForHealth(gatewayUrl, HEALTH_POLL_INTERVAL_MS)) return;
  if (!(await canBindGatewayUrl(gatewayUrl))) {
    throw new Error(
      `gateway port ${gatewayPort(gatewayUrl) ?? "unknown"} is occupied by an unknown or foreign process`,
    );
  }
  let binary = resolveGatewayBinary(root);
  if (!binary) {
    // No dev gateway yet: build target/debug/tura_gateway.
    if (isWindows) {
      await runProcess(
        "powershell",
        [
          "-NoProfile",
          "-ExecutionPolicy",
          "Bypass",
          "-File",
          join(root, "scripts", "build-debug.ps1"),
          "-SkipTui",
        ],
        root,
      );
    } else {
      await runProcess("sh", [join(root, "scripts", "build-debug.sh"), "--skip-tui"], root);
    }
    binary = resolveGatewayBinary(root);
  }
  if (!binary) throw new Error("tura_gateway binary not found after build");
  if (await waitForHealth(gatewayUrl, HEALTH_TIMEOUT_MS)) return;
  if (!(await canBindGatewayUrl(gatewayUrl))) {
    await terminateGatewayFromLock(instanceHome(root), "dev", gatewayUrl);
  }
  if (!(await canBindGatewayUrl(gatewayUrl))) {
    throw new Error(
      `gateway port ${gatewayPort(gatewayUrl) ?? "unknown"} is occupied by an unknown or foreign process`,
    );
  }
  for (let attempt = 0; attempt < 2; attempt += 1) {
    const child = spawnGateway(binary, root, gatewayUrl);
    if (await waitForHealth(gatewayUrl, HEALTH_TIMEOUT_MS)) return;
    const killed = await terminateGatewayFromLock(instanceHome(root), "dev", gatewayUrl);
    const killedOwned = await forceKillGatewayChild(child);
    if (ownedGatewayChild === child) {
      ownedGatewayChild = undefined;
      ownedGatewayShutdownMode = undefined;
    }
    if (!killed && !killedOwned) {
      throw new Error(
        `gateway did not become healthy after ${HEALTH_TIMEOUT_MS}ms and could not be killed`,
      );
    }
  }
  throw new Error(`gateway did not become healthy after ${HEALTH_TIMEOUT_MS}ms`);
}

function spawnGateway(binary: string, root: string, gatewayUrl: string): ReturnType<typeof spawn> {
  const port = gatewayPort(gatewayUrl);
  const child = spawn(binary, [], {
    cwd: root,
    stdio: ["pipe", "ignore", "ignore"],
    windowsHide: true,
    env: {
      ...process.env,
      ...(port ? { PORT: port } : {}),
      TURA_HOME: instanceHome(root),
      TURA_PROJECT_ROOT: root,
      TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF: "1",
    },
  });
  ownedGatewayChild = child;
  ownedGatewayShutdownMode = "stdin-eof";
  (child.stdin as (NodeJS.WritableStream & { unref?: () => void }) | null)?.unref?.();
  child.once("exit", () => {
    if (ownedGatewayChild === child) ownedGatewayChild = undefined;
    if (ownedGatewayChild === undefined) ownedGatewayShutdownMode = undefined;
  });
  return child;
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

async function waitForHealth(gatewayUrl: string, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  do {
    if (await healthOk(gatewayUrl)) return true;
    await delay(HEALTH_POLL_INTERVAL_MS);
  } while (Date.now() < deadline);
  return false;
}

function instanceHome(root: string): string {
  return canonical(process.env.TURA_HOME?.trim() || root);
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
  instanceHomePath: string,
  mode: "dev" | "release",
  gatewayUrl: string,
): Promise<boolean> {
  const port = gatewayPort(gatewayUrl);
  if (!port) return false;
  const record = readGatewayLock(instanceHomePath, mode);
  if (!record?.pid || !record.processStartTime) return false;
  if (record.kind !== "gateway" || record.mode !== mode || record.port !== port) return false;
  if (!record.root || normalizeRoot(record.root) !== normalizeRoot(instanceHomePath)) return false;
  const startTime = await processStartTime(record.pid);
  if (startTime === undefined || startTime !== record.processStartTime) return false;
  killProcessTree(record.pid);
  await waitForProcessExit(record.pid, 5_000);
  return !isProcessAlive(record.pid);
}

function readGatewayLock(
  instanceHomePath: string,
  mode: "dev" | "release",
): GatewayLockRecord | undefined {
  let raw = "";
  try {
    raw = readFileSync(join(instanceHomePath, ".tura", "locks", `gateway-${mode}.lock`), "utf8");
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

function normalizeRoot(value: string): string {
  const normalized = canonical(value).replace(/[\\/]+$/u, "");
  return process.platform === "win32" ? normalized.toLowerCase() : normalized;
}

function canonical(value: string): string {
  try {
    return realpathSync(value).replace(/^\\\\\?\\(UNC\\)?/u, (_match, unc) => (unc ? "\\\\" : ""));
  } catch {
    return resolve(value).replace(/^\\\\\?\\(UNC\\)?/u, (_match, unc) => (unc ? "\\\\" : ""));
  }
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
    try {
      process.kill(pid, "SIGKILL");
    } catch {
      // Already exited.
    }
  }, 2_000).unref();
}

async function forceKillGatewayChild(child: ReturnType<typeof spawn>): Promise<boolean> {
  const pid = child.pid;
  if (!pid) return false;
  killProcessTree(pid);
  await waitForProcessExit(pid, 5_000);
  return !isProcessAlive(pid);
}

async function waitForProcessExit(pid: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline && isProcessAlive(pid)) await delay(100);
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
    const script = `$p = Get-CimInstance Win32_Process -Filter "ProcessId=${pid}"; if ($p) { ([DateTimeOffset]$p.CreationDate).ToUnixTimeSeconds() }`;
    const output = await collectProcessOutput("powershell", ["-NoProfile", "-Command", script]);
    const startTime = Number(output.trim());
    return Number.isFinite(startTime) ? startTime : undefined;
  }
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

function delay(ms: number): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, ms));
}

function runProcess(command: string, args: string[], cwd: string): Promise<void> {
  return new Promise((resolveProcess, reject) => {
    const child = spawn(command, args, { cwd, stdio: "ignore", windowsHide: true });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolveProcess();
      else reject(new Error(`${command} ${args.join(" ")} exited with ${code ?? "signal"}`));
    });
  });
}

function repoRoot(): string {
  let current = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
  for (let depth = 0; depth < 4; depth += 1) {
    if (existsSync(join(current, "Cargo.toml")) && existsSync(join(current, "crates", "gateway")))
      return current;
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
}

function canBindGatewayUrl(gatewayUrl: string): Promise<boolean> {
  const port = Number(gatewayPort(gatewayUrl));
  if (!Number.isInteger(port) || port <= 0) return Promise.resolve(false);
  return new Promise((resolveBind) => {
    const server = createServer();
    server.once("error", () => resolveBind(false));
    server.listen(port, "127.0.0.1", () => {
      server.close(() => resolveBind(true));
    });
  });
}
function gatewayPort(gatewayUrl: string): string | undefined {
  try {
    return new URL(gatewayUrl).port || undefined;
  } catch {
    return undefined;
  }
}

function readJsonBody(req: import("node:http").IncomingMessage): Promise<Record<string, unknown>> {
  return new Promise((resolveBody, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk) => chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)));
    req.on("error", reject);
    req.on("end", () => {
      const text = Buffer.concat(chunks).toString("utf8").trim();
      if (!text) {
        resolveBody({});
        return;
      }
      try {
        resolveBody(JSON.parse(text) as Record<string, unknown>);
      } catch (error) {
        reject(error);
      }
    });
  });
}

function writeJson(res: import("node:http").ServerResponse, payload: unknown): void {
  res.setHeader("content-type", "application/json");
  res.end(JSON.stringify(payload));
}

function killOwnedGateway(): void {
  const child = ownedGatewayChild;
  if (!child) return;
  ownedGatewayChild = undefined;
  const shutdownMode = ownedGatewayShutdownMode;
  ownedGatewayShutdownMode = undefined;
  try {
    if (shutdownMode === "stdin-eof") {
      child.stdin?.end();
      const timer = setTimeout(() => {
        if (child.exitCode === null && !child.killed) child.kill();
      }, 5_000);
      timer.unref();
    } else {
      child.kill();
    }
  } catch {
    // Already exited.
  }
}
