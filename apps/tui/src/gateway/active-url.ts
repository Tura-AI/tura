import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const DEV_GATEWAY_PORT = "4125";
export const RELEASE_GATEWAY_PORT = "4126";
const ACTIVE_GATEWAY_FILE = "gateway-active.env";
const TURA_GATEWAY_URL = "TURA_GATEWAY_URL";
const TURA_GATEWAY_PID = "TURA_GATEWAY_PID";
const TURA_GATEWAY_PROCESS_START_TIME = "TURA_GATEWAY_PROCESS_START_TIME";

export interface ActiveGatewayRecord {
  url: string;
  pid?: number;
  processStartTime?: number;
}

export function defaultGatewayUrl(): string {
  return `http://127.0.0.1:${defaultGatewayPort()}`;
}

export function defaultGatewayPort(): string {
  const fromEnv = process.env.TURA_GATEWAY_PORT?.trim();
  if (fromEnv && /^\d+$/u.test(fromEnv)) return fromEnv;
  return currentBuildMode() === "release" ? RELEASE_GATEWAY_PORT : DEV_GATEWAY_PORT;
}

export function currentBuildMode(): "dev" | "release" {
  if (process.env.TURA_BUILD_KIND === "release") return "release";
  const normalized = process.execPath.replace(/\\/g, "/").toLowerCase();
  return normalized.includes("/target/release/") ? "release" : "dev";
}

export function readActiveGatewayUrl(home = instanceHome()): string | undefined {
  return readActiveGatewayRecord(home)?.url;
}

export function readActiveGatewayRecord(home = instanceHome()): ActiveGatewayRecord | undefined {
  try {
    return parseActiveGatewayRecord(readFileSync(activeGatewayEnvPath(home), "utf8"));
  } catch {
    return undefined;
  }
}

export function writeActiveGatewayUrl(
  gatewayUrl: string,
  home = instanceHome(),
  identity?: { pid?: number; processStartTime?: number },
): void {
  const path = activeGatewayEnvPath(home);
  mkdirSync(dirname(path), { recursive: true });
  let content = `${TURA_GATEWAY_URL}=${stripTrailingSlash(gatewayUrl)}\n`;
  if (identity?.pid) content += `${TURA_GATEWAY_PID}=${identity.pid}\n`;
  if (identity?.processStartTime) {
    content += `${TURA_GATEWAY_PROCESS_START_TIME}=${identity.processStartTime}\n`;
  }
  writeFileSync(path, content, "utf8");
  process.env.TURA_GATEWAY_URL = stripTrailingSlash(gatewayUrl);
}

export function activeGatewayEnvPath(home = instanceHome()): string {
  return join(home, ".tura", ACTIVE_GATEWAY_FILE);
}

export function instanceHome(root?: string): string {
  const fromEnv = process.env.TURA_HOME?.trim();
  return canonical(fromEnv || root || findRuntimeRoot());
}

function parseActiveGatewayRecord(raw: string): ActiveGatewayRecord | undefined {
  let url: string | undefined;
  let pid: number | undefined;
  let processStartTime: number | undefined;
  for (const line of raw.split(/\r?\n/u)) {
    const trimmed = line.trim();
    const [key, rawValue] = trimmed.split("=", 2);
    if (!key || rawValue === undefined) continue;
    const value = rawValue.trim().replace(/^['"]|['"]$/gu, "");
    if (key === TURA_GATEWAY_URL && value) url = stripTrailingSlash(value);
    if (key === TURA_GATEWAY_PID) pid = parsePositiveInteger(value);
    if (key === TURA_GATEWAY_PROCESS_START_TIME) {
      processStartTime = parsePositiveInteger(value);
    }
  }
  return url ? { url, pid, processStartTime } : undefined;
}

function parsePositiveInteger(value: string): number | undefined {
  if (!/^\d+$/u.test(value)) return undefined;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined;
}

function findRuntimeRoot(): string {
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

function stripTrailingSlash(value: string): string {
  return value.replace(/\/+$/u, "");
}
