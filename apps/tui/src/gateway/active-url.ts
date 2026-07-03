import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const DEV_GATEWAY_PORT = "4125";
export const RELEASE_GATEWAY_PORT = "4126";
const ACTIVE_GATEWAY_FILE = "gateway-active.env";
const TURA_GATEWAY_URL = "TURA_GATEWAY_URL";

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
  try {
    return parseActiveGatewayUrl(readFileSync(activeGatewayEnvPath(home), "utf8"));
  } catch {
    return undefined;
  }
}

export function writeActiveGatewayUrl(gatewayUrl: string, home = instanceHome()): void {
  const path = activeGatewayEnvPath(home);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${TURA_GATEWAY_URL}=${stripTrailingSlash(gatewayUrl)}\n`, "utf8");
  process.env.TURA_GATEWAY_URL = stripTrailingSlash(gatewayUrl);
}

export function activeGatewayEnvPath(home = instanceHome()): string {
  return join(home, ".tura", ACTIVE_GATEWAY_FILE);
}

export function instanceHome(root?: string): string {
  const fromEnv = process.env.TURA_HOME?.trim();
  return canonical(fromEnv || root || findRuntimeRoot());
}

function parseActiveGatewayUrl(raw: string): string | undefined {
  for (const line of raw.split(/\r?\n/u)) {
    const trimmed = line.trim();
    if (!trimmed.startsWith(`${TURA_GATEWAY_URL}=`)) continue;
    const value = trimmed
      .slice(TURA_GATEWAY_URL.length + 1)
      .trim()
      .replace(/^['"]|['"]$/gu, "");
    if (value) return stripTrailingSlash(value);
  }
  return undefined;
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

function findAncestor(start: string, predicate: (candidate: string) => boolean): string | undefined {
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
  return existsSync(join(candidate, "Cargo.toml")) && existsSync(join(candidate, "crates", "gateway"));
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
    return realpathSync(value);
  } catch {
    return resolve(value);
  }
}

function stripTrailingSlash(value: string): string {
  return value.replace(/\/+$/u, "");
}
