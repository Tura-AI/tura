import { execFile, spawn, type ChildProcess } from "node:child_process";
import { closeSync, existsSync, mkdirSync, openSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import type { TerminalCapabilities } from "../tui/capabilities.js";
import { TUI_ANIMATION_INTERVAL_MS } from "../tui/frame-rate.js";
import { iconAnimationFrame } from "../tui/render/busy-animation.js";
import { t } from "../i18n.js";
import {
  currentBuildMode,
  defaultGatewayUrl,
  readActiveGatewayRecord,
  readActiveGatewayUrl,
  writeActiveGatewayUrl,
  type ActiveGatewayRecord,
} from "./active-url.js";

type StartupStep = "checking";

const HEALTH_POLL_INTERVAL_MS = 500;
const GATEWAY_START_TIMEOUT_MS = 20_000;

interface GatewayIdentity {
  root: string;
  home?: string;
  version?: string;
  pid?: number;
  processStartTime?: number;
}

interface GatewayLaunchRequest {
  targetUrl: string;
  instanceHome: string;
  projectRoot: string;
  dev: boolean;
}

type GatewayLauncher = (request: GatewayLaunchRequest) => Promise<string>;
type GatewayProcessTerminator = (
  record: ActiveGatewayRecord,
  instanceHome: string,
) => Promise<boolean>;

let gatewayLauncher: GatewayLauncher = launchGatewayProcess;
let gatewayProcessTerminator: GatewayProcessTerminator = terminateGatewayProcess;
let gatewayStartTimeoutMs = GATEWAY_START_TIMEOUT_MS;
const execFileAsync = promisify(execFile);

function gatewayCandidates(
  requestedUrl: string,
  defaultUrl: string,
  instanceHome: string,
  explicit: boolean,
): string[] {
  if (explicit) return [requestedUrl];
  const candidates = [readActiveGatewayUrl(instanceHome), requestedUrl, defaultUrl].filter(
    (value): value is string => Boolean(value),
  );
  return [...new Set(candidates.map(stripTrailingSlash))];
}

/** Ensure a same-home gateway is running, starting one when non-explicit lookup misses. */
export async function ensureGatewayAvailable(
  gatewayUrl: string,
  capabilities: TerminalCapabilities,
  dev?: boolean,
  explicit?: boolean,
): Promise<string> {
  const desiredUrl = stripTrailingSlash(gatewayUrl);
  const instanceHome = process.env.TURA_HOME?.trim()
    ? canonical(process.env.TURA_HOME)
    : packageRoot();
  const projectRoot = packageRoot();
  const targetUrl = explicit ? desiredUrl : stripTrailingSlash(defaultGatewayUrl());
  const candidates = gatewayCandidates(desiredUrl, targetUrl, instanceHome, Boolean(explicit));

  let connectedUrl: string | undefined;
  let connectedIdentity: GatewayIdentity | undefined;
  await runWithSpinner({
    step: "checking",
    text: t("gatewayWaiting"),
    capabilities,
    run: async (tick) => {
      for (const candidate of candidates) {
        tick();
        const identity = await gatewayIdentityWithProbeTimeout(candidate);
        if (
          identity &&
          gatewayMatchesInstance(identity, instanceHome, projectRoot, Boolean(explicit))
        ) {
          connectedUrl = candidate;
          connectedIdentity = identity;
          return;
        }
      }
    },
  });

  if (connectedUrl) {
    writeActiveGatewayUrl(connectedUrl, instanceHome, connectedIdentity);
    return connectedUrl;
  }

  if (!explicit) {
    const request = {
      targetUrl,
      instanceHome,
      projectRoot,
      dev: Boolean(dev),
    };
    const activeRecord = readActiveGatewayRecord(instanceHome);
    if (activeRecord) {
      const activeProbe = await gatewayIdentityWithProbeTimeout(activeRecord.url);
      if (activeProbe && !gatewayMatchesInstance(activeProbe, instanceHome, projectRoot, false)) {
        const first = await launchAndConfirmGateway(request);
        if (first) {
          writeActiveGatewayUrl(first.url, instanceHome, first.identity);
          return first.url;
        }
        throw new Error(t("gatewayStartTimeout"));
      }
      const activeIdentity = await waitForSameHomeGateway(
        activeRecord.url,
        instanceHome,
        projectRoot,
        gatewayStartTimeoutMs,
      );
      if (activeIdentity) {
        writeActiveGatewayUrl(activeRecord.url, instanceHome, activeIdentity);
        return activeRecord.url;
      }
      if (await gatewayProcessTerminator(activeRecord, instanceHome)) {
        const restarted = await launchAndConfirmGateway(request);
        if (restarted) {
          writeActiveGatewayUrl(restarted.url, instanceHome, restarted.identity);
          return restarted.url;
        }
        throw new Error(t("gatewayStartTimeout"));
      }
    }
    const first = await launchAndConfirmGateway(request);
    if (first) {
      writeActiveGatewayUrl(first.url, instanceHome, first.identity);
      return first.url;
    }
    if (activeRecord && (await gatewayProcessTerminator(activeRecord, instanceHome))) {
      const restarted = await launchAndConfirmGateway(request);
      if (restarted) {
        writeActiveGatewayUrl(restarted.url, instanceHome, restarted.identity);
        return restarted.url;
      }
    }
    throw new Error(t("gatewayStartTimeout"));
  }

  throw new Error(
    `Gateway is not running at ${candidates.join(", ")}. Explicit gateway URLs are only connected, not auto-started.`,
  );
}

export async function _gatewayProbeForTest(gatewayUrl: string): Promise<boolean> {
  return Boolean(await gatewayIdentityWithProbeTimeout(stripTrailingSlash(gatewayUrl)));
}

export function _setGatewayLauncherForTest(launcher: GatewayLauncher): () => void {
  const previous = gatewayLauncher;
  gatewayLauncher = launcher;
  return () => {
    gatewayLauncher = previous;
  };
}

export function _setGatewayProcessTerminatorForTest(
  terminator: GatewayProcessTerminator,
): () => void {
  const previous = gatewayProcessTerminator;
  gatewayProcessTerminator = terminator;
  return () => {
    gatewayProcessTerminator = previous;
  };
}

export function _setGatewayStartTimeoutMsForTest(timeoutMs: number): () => void {
  const previous = gatewayStartTimeoutMs;
  gatewayStartTimeoutMs = timeoutMs;
  return () => {
    gatewayStartTimeoutMs = previous;
  };
}

async function launchAndConfirmGateway(
  request: GatewayLaunchRequest,
): Promise<{ url: string; identity: GatewayIdentity } | null> {
  let startedUrl: string;
  try {
    startedUrl = await gatewayLauncher(request);
  } catch (error) {
    if (!isGatewayStartupTimeout(error)) throw error;
    return null;
  }
  const identity = await waitForSameHomeGateway(
    startedUrl,
    request.instanceHome,
    request.projectRoot,
    gatewayStartTimeoutMs,
  );
  return identity ? { url: startedUrl, identity } : null;
}

function isGatewayStartupTimeout(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return (
    message.includes(t("gatewayStartTimeout")) ||
    message.includes("did not become healthy") ||
    message.includes("exited before becoming healthy")
  );
}

async function gatewayIdentityWithProbeTimeout(
  gatewayUrl: string,
): Promise<GatewayIdentity | null> {
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), HEALTH_POLL_INTERVAL_MS);
    try {
      const response = await fetch(`${stripTrailingSlash(gatewayUrl)}/global/health`, {
        signal: controller.signal,
      });
      if (!response.ok) return null;
      const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;
      if (body.healthy !== true) return null;
      return {
        root: typeof body.root === "string" ? body.root : "",
        home: typeof body.home === "string" ? body.home : undefined,
        version: typeof body.version === "string" ? body.version : undefined,
        pid: numberField(body.pid),
        processStartTime: numberField(body.process_start_time),
      };
    } finally {
      clearTimeout(timer);
    }
  } catch {
    return null;
  }
}

async function waitForSameHomeGateway(
  gatewayUrl: string,
  instanceHome: string,
  projectRoot: string,
  timeoutMs: number,
): Promise<GatewayIdentity | null> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const identity = await gatewayIdentityWithProbeTimeout(gatewayUrl);
    if (identity && gatewayMatchesInstance(identity, instanceHome, projectRoot, false)) {
      return identity;
    }
    await delay(HEALTH_POLL_INTERVAL_MS);
  }
  return null;
}

async function launchGatewayProcess(request: GatewayLaunchRequest): Promise<string> {
  const executable = resolveGatewayBinary(request.dev);
  if (!executable) throw new Error(t("gatewayMissingBinary"));
  const targetUrl = stripTrailingSlash(request.targetUrl);
  let exited: { code: number | null; signal: NodeJS.Signals | null } | undefined;
  const stdio = gatewayLogStdio(request.instanceHome);
  const child = spawn(executable, [], {
    detached: true,
    env: gatewayProcessEnv(request),
    stdio,
    windowsHide: true,
  });
  closeGatewayLogStdio(stdio);
  child.once("exit", (code, signal) => {
    exited = { code, signal };
  });
  child.unref();
  const deadline = Date.now() + GATEWAY_START_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (exited) {
      throw new Error(`Gateway exited before becoming healthy (${exitDescription(exited)}).`);
    }
    const activeUrl = readActiveGatewayUrl(request.instanceHome);
    const candidateUrl = activeUrl ? stripTrailingSlash(activeUrl) : targetUrl;
    const identity = await gatewayIdentityWithProbeTimeout(candidateUrl);
    if (
      identity &&
      gatewayMatchesInstance(identity, request.instanceHome, request.projectRoot, false)
    ) {
      return candidateUrl;
    }
    await delay(HEALTH_POLL_INTERVAL_MS);
  }
  stopUnreadyChild(child);
  throw new Error(t("gatewayStartTimeout"));
}

function gatewayProcessEnv(request: GatewayLaunchRequest): NodeJS.ProcessEnv {
  const port = portFromGatewayUrl(request.targetUrl);
  return {
    ...process.env,
    TURA_HOME: request.instanceHome,
    TURA_PROJECT_ROOT: request.projectRoot,
    TURA_GATEWAY_PORT: port ?? process.env.TURA_GATEWAY_PORT,
  };
}

function resolveGatewayBinary(dev: boolean): string | undefined {
  const executable = process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway";
  const fromEnv = process.env.TURA_GATEWAY_BIN || process.env.TURA_GATEWAY_EXE;
  const repo = packageRoot();
  const mode = dev || currentBuildMode() === "dev" ? "debug" : "release";
  const candidates = [
    fromEnv,
    join(dirname(process.execPath), executable),
    join(repo, "target", mode, executable),
    join(repo, "target", "release", executable),
    join(repo, "target", "debug", executable),
    join(repo, "bin", executable),
  ].filter((value): value is string => Boolean(value));
  return candidates.find((candidate) => existsSync(candidate));
}

function portFromGatewayUrl(gatewayUrl: string): string | undefined {
  try {
    const parsed = new URL(gatewayUrl);
    return parsed.port || undefined;
  } catch {
    return undefined;
  }
}

function exitDescription(exit: { code: number | null; signal: NodeJS.Signals | null }): string {
  return exit.signal ? `signal ${exit.signal}` : `exit code ${exit.code ?? 1}`;
}

function stopUnreadyChild(child: ChildProcess): void {
  try {
    if (child.exitCode === null && child.signalCode === null) child.kill();
  } catch {
    // Best effort: the gateway may have already detached or exited.
  }
}

function gatewayLogStdio(instanceHome: string): ["ignore", number, number] {
  const logDir = join(instanceHome, ".tura", "logs");
  mkdirSync(logDir, { recursive: true });
  return [
    "ignore",
    openSync(join(logDir, "gateway-autostart.stdout.log"), "a"),
    openSync(join(logDir, "gateway-autostart.stderr.log"), "a"),
  ];
}

function closeGatewayLogStdio(stdio: ["ignore", number, number]): void {
  for (const fd of [stdio[1], stdio[2]]) {
    try {
      closeSync(fd);
    } catch {
      // Best effort: the child process may already own or close the handle.
    }
  }
}

async function terminateGatewayProcess(
  record: ActiveGatewayRecord,
  _instanceHome: string,
): Promise<boolean> {
  if (!record.pid || !record.processStartTime) return false;
  const info = await gatewayProcessInfo(record.pid).catch(() => undefined);
  if (!info || !gatewayProcessNameMatches(info.name)) return false;
  if (Math.abs(info.processStartTime - record.processStartTime) > 2) return false;
  try {
    process.kill(record.pid);
    return true;
  } catch {
    return false;
  }
}

async function gatewayProcessInfo(
  pid: number,
): Promise<{ name: string; processStartTime: number } | undefined> {
  if (process.platform === "win32") return windowsProcessInfo(pid);
  return posixProcessInfo(pid);
}

async function windowsProcessInfo(
  pid: number,
): Promise<{ name: string; processStartTime: number } | undefined> {
  const command = [
    "$p = Get-CimInstance Win32_Process -Filter 'ProcessId = ",
    String(pid),
    "'; if ($p) {",
    " $d = [Management.ManagementDateTimeConverter]::ToDateTime($p.CreationDate).ToUniversalTime();",
    " $s = ([DateTimeOffset]$d).ToUnixTimeSeconds();",
    " Write-Output ($p.Name + '|' + $s)",
    " }",
  ].join("");
  const { stdout } = await execFileAsync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", command],
    { windowsHide: true, timeout: 3_000 },
  );
  return parseProcessInfo(stdout);
}

async function posixProcessInfo(
  pid: number,
): Promise<{ name: string; processStartTime: number } | undefined> {
  const { stdout } = await execFileAsync(
    "ps",
    ["-p", String(pid), "-o", "comm=", "-o", "lstart="],
    {
      timeout: 3_000,
    },
  );
  const line = stdout.trim();
  if (!line) return undefined;
  const parts = line.split(/\s+/u);
  const name = parts.shift() ?? "";
  const processStartTime = Math.floor(Date.parse(parts.join(" ")) / 1000);
  return name && Number.isFinite(processStartTime) ? { name, processStartTime } : undefined;
}

function parseProcessInfo(raw: string): { name: string; processStartTime: number } | undefined {
  const [name, start] = raw.trim().split("|", 2);
  const processStartTime = Number(start);
  return name && Number.isFinite(processStartTime) ? { name, processStartTime } : undefined;
}

function gatewayProcessNameMatches(name: string): boolean {
  return name.replace(/\.exe$/iu, "").toLowerCase() === "tura_gateway";
}

function numberField(value: unknown): number | undefined {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 ? value : undefined;
}

function gatewayMatchesInstance(
  identity: GatewayIdentity,
  instanceHome: string,
  projectRoot: string,
  explicit: boolean,
): boolean {
  if (explicit) return true;
  if (identity.home) return samePath(identity.home, instanceHome);
  return samePath(identity.root, projectRoot);
}

function samePath(left: string, right: string): boolean {
  if (!left.trim() || !right.trim()) return false;
  return comparablePath(left) === comparablePath(right);
}

function comparablePath(value: string): string {
  const normalized = canonical(value).replace(/[\\/]+$/u, "");
  return process.platform === "win32" ? normalized.toLowerCase() : normalized;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, ms));
}

function packageRoot(): string {
  const fromEnv = process.env.TURA_PROJECT_ROOT;
  if (fromEnv && existsSync(fromEnv)) return canonical(fromEnv);
  return canonical(findRepoRoot());
}

function canonical(value: string): string {
  try {
    return stripVerbatimPrefix(realpathSync.native(value));
  } catch {
    return stripVerbatimPrefix(resolve(value));
  }
}

function stripVerbatimPrefix(value: string): string {
  if (value.startsWith("\\\\?\\UNC\\")) return `\\\\${value.slice("\\\\?\\UNC\\".length)}`;
  if (value.startsWith("\\\\?\\")) return value.slice("\\\\?\\".length);
  return value;
}

async function runWithSpinner(options: {
  step: StartupStep;
  text: string;
  capabilities: TerminalCapabilities;
  run: (tick: () => void) => Promise<void>;
}): Promise<void> {
  let frame = 0;
  const frames = options.capabilities.unicode
    ? [".", "..", "..."]
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
  const timer = setInterval(draw, TUI_ANIMATION_INTERVAL_MS);
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
    const sourceRoot = findAncestor(start, isSourceCheckoutRoot);
    if (sourceRoot) return sourceRoot;
  }
  for (const start of starts) {
    const runtimeRoot = findAncestor(start, isRuntimeRoot);
    if (runtimeRoot) return runtimeRoot;
  }
  return process.cwd();
}

function findAncestor(
  start: string,
  predicate: (candidate: string) => boolean,
): string | undefined {
  let current = resolve(start);
  for (let depth = 0; depth < 8; depth += 1) {
    if (predicate(current)) return current;
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return undefined;
}

function isSourceCheckoutRoot(candidate: string): boolean {
  return (
    existsSync(join(candidate, "Cargo.toml")) && existsSync(join(candidate, "crates", "gateway"))
  );
}

function isRuntimeRoot(candidate: string): boolean {
  return (
    isSourceCheckoutRoot(candidate) ||
    (existsSync(join(candidate, "agents", "src")) &&
      existsSync(join(candidate, "personas", "src"))) ||
    existsSync(join(candidate, "config", "provider_config.json"))
  );
}

function stripTrailingSlash(value: string): string {
  return value.replace(/\/+$/u, "");
}
