import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { TerminalCapabilities } from "../tui/capabilities.js";
import { TUI_DRAW_INTERVAL_MS } from "../tui/frame-rate.js";
import { iconAnimationFrame } from "../tui/render/busy-animation.js";
import { t } from "../i18n.js";
import {
  currentBuildMode,
  defaultGatewayUrl,
  readActiveGatewayUrl,
  writeActiveGatewayUrl,
} from "./active-url.js";

type StartupStep = "checking";

const HEALTH_POLL_INTERVAL_MS = 500;
const GATEWAY_START_TIMEOUT_MS = 20_000;

interface GatewayIdentity {
  root: string;
  home?: string;
  version?: string;
}

interface GatewayLaunchRequest {
  targetUrl: string;
  instanceHome: string;
  projectRoot: string;
  dev: boolean;
}

type GatewayLauncher = (request: GatewayLaunchRequest) => Promise<string>;

let gatewayLauncher: GatewayLauncher = launchGatewayProcess;

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
  await runWithSpinner({
    step: "checking",
    text: t("gatewayWaiting"),
    capabilities,
    run: async (tick) => {
      for (const candidate of candidates) {
        tick();
        const identity = await gatewayIdentityWithProbeTimeout(candidate);
        if (identity && gatewayMatchesInstance(identity, instanceHome, projectRoot, Boolean(explicit))) {
          connectedUrl = candidate;
          return;
        }
      }
    },
  });

  if (connectedUrl) {
    writeActiveGatewayUrl(connectedUrl, instanceHome);
    return connectedUrl;
  }

  if (!explicit) {
    const startedUrl = await gatewayLauncher({
      targetUrl,
      instanceHome,
      projectRoot,
      dev: Boolean(dev),
    });
    const identity = await waitForSameHomeGateway(
      startedUrl,
      instanceHome,
      projectRoot,
      GATEWAY_START_TIMEOUT_MS,
    );
    if (identity) {
      writeActiveGatewayUrl(startedUrl, instanceHome);
      return startedUrl;
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
  const child = spawn(executable, [], {
    detached: true,
    env: gatewayProcessEnv(request),
    stdio: "ignore",
    windowsHide: true,
  });
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
    if (identity && gatewayMatchesInstance(identity, request.instanceHome, request.projectRoot, false)) {
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
    return realpathSync(value);
  } catch {
    return resolve(value);
  }
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

function stripTrailingSlash(value: string): string {
  return value.replace(/\/+$/u, "");
}
