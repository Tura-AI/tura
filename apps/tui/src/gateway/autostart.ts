import { existsSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { TerminalCapabilities } from "../tui/capabilities.js";
import { TUI_DRAW_INTERVAL_MS } from "../tui/frame-rate.js";
import { iconAnimationFrame } from "../tui/render/busy-animation.js";
import { t } from "../i18n.js";
import { defaultGatewayUrl, readActiveGatewayUrl, writeActiveGatewayUrl } from "./active-url.js";

type StartupStep = "checking";

const HEALTH_POLL_INTERVAL_MS = 500;

interface GatewayIdentity {
  root: string;
  version?: string;
}

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

/**
 * Ensure a gateway this package can use is already running, and return its URL.
 * TUI is never the gateway owner: it only probes existing gateway candidates.
 */
export async function ensureGatewayAvailable(
  gatewayUrl: string,
  capabilities: TerminalCapabilities,
  _dev?: boolean,
  explicit?: boolean,
): Promise<string> {
  const desiredUrl = stripTrailingSlash(gatewayUrl);
  const instanceHome = process.env.TURA_HOME?.trim()
    ? canonical(process.env.TURA_HOME)
    : packageRoot();
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
        if (await gatewayIdentityWithProbeTimeout(candidate)) {
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

  throw new Error(
    `Gateway is not running at ${candidates.join(", ")}. Start tura_gateway first; TUI only connects to an existing gateway.`,
  );
}

export async function _gatewayProbeForTest(gatewayUrl: string): Promise<boolean> {
  return Boolean(await gatewayIdentityWithProbeTimeout(stripTrailingSlash(gatewayUrl)));
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
        version: typeof body.version === "string" ? body.version : undefined,
      };
    } finally {
      clearTimeout(timer);
    }
  } catch {
    return null;
  }
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
