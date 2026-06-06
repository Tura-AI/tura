import { join, resolve } from "node:path";

export function resolveGatewayUrl(flagValue?: string): string {
  return (flagValue || process.env.TURA_GATEWAY_URL || "http://127.0.0.1:4096").replace(/\/+$/, "");
}

export function resolveCwd(flagValue?: string): string {
  return resolve(flagValue || process.env.TURA_CWD || defaultWorkspaceDirectory());
}

function defaultWorkspaceDirectory(): string {
  const home = process.env.USERPROFILE || process.env.HOME;
  if (!home) return process.cwd();
  return join(home, "Documents", "tura workspace");
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
