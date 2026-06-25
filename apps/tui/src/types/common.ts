export type JsonObject = Record<string, unknown>;

export interface CliContext {
  gatewayUrl: string;
  /** True when the gateway URL was explicitly chosen (flag or TURA_GATEWAY_URL),
   * so a reachable Tura gateway there is reused without the root identity check. */
  gatewayUrlExplicit: boolean;
  cwd: string;
  json: boolean;
  color: ColorMode;
  display: DisplayMode;
  language?: "zh-CN" | "en";
  verbose: boolean;
  mock: boolean;
  dev: boolean;
}

export type ColorMode = "auto" | "always" | "never";
export type DisplayMode = "auto" | "plain" | "rich";

export type OutputMode = "text" | "json" | "ndjson";

export class CliUsageError extends Error {
  exitCode = 2;
}

export class GatewayUnavailableError extends Error {
  exitCode = 5;
}

export class PermissionDeniedError extends Error {
  exitCode = 3;
}

export class TimeoutError extends Error {
  exitCode = 4;
}
