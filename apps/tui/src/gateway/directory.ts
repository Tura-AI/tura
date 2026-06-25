import { resolve } from "node:path";

const DEV_GATEWAY_PORT = "4126";
const RELEASE_GATEWAY_PORT = "4156";

export function resolveGatewayUrl(flagValue?: string): string {
  const raw = (flagValue || process.env.TURA_GATEWAY_URL || defaultGatewayUrl()).trim();
  return normalizeGatewayUrl(raw).replace(/\/+$/, "");
}

/** Whether the gateway URL was explicitly provided (flag or env) rather than
 * falling back to the default port. An explicit URL is trusted as-is. */
export function gatewayUrlIsExplicit(flagValue?: string): boolean {
  return Boolean((flagValue || process.env.TURA_GATEWAY_URL || "").trim());
}

function normalizeGatewayUrl(input: string): string {
  try {
    const parsed = new URL(input);
    const host = parsed.hostname.toLowerCase();
    const isLoopback =
      host === "127.0.0.1" || host === "::1" || host === "localhost" || host === "[::1]";
    if (isLoopback && !parsed.port) {
      parsed.port = defaultGatewayPort();
    }
    return parsed.toString();
  } catch {
    return input.replace(/\/+$/, "");
  }
}

function defaultGatewayUrl(): string {
  return `http://127.0.0.1:${defaultGatewayPort()}`;
}

function defaultGatewayPort(): string {
  return currentBuildMode() === "release" ? RELEASE_GATEWAY_PORT : DEV_GATEWAY_PORT;
}

function currentBuildMode(): "dev" | "release" {
  if (process.env.TURA_BUILD_KIND === "release") return "release";
  const normalized = process.execPath.replace(/\\/g, "/").toLowerCase();
  return normalized.includes("/target/release/") ? "release" : "dev";
}

export function resolveCwd(flagValue?: string): string {
  return resolve(flagValue || process.env.TURA_CWD || process.cwd());
}

export function directoryHeader(directory: string): string {
  return encodeURIComponent(directory);
}

export function sameDirectory(left?: string, right?: string): boolean {
  if (!left || !right) return false;
  return normalizeDirectory(left) === normalizeDirectory(right);
}

function normalizeDirectory(value: string): string {
  const normalized = value.trim().replace(/\\/g, "/");
  if (/^[A-Za-z]:\/$/.test(normalized)) return normalized;
  if (/^\/+$/.test(normalized)) return "/";
  return normalized.replace(/\/+$/, "");
}
