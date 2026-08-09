import type { Setter } from "solid-js";
import { t } from "./i18n";
import type { AppState } from "./state/global-store";

export const GATEWAY_CONNECT_TIMEOUT_MS = 20_000;
export const GATEWAY_HEALTH_TIMEOUT_MS = 20_000;

export function isGatewayTimeoutError(error: unknown): boolean {
  if (
    error instanceof DOMException &&
    (error.name === "AbortError" || error.name === "TimeoutError")
  ) {
    return true;
  }
  if (error instanceof TypeError) {
    const message = error.message.toLowerCase();
    return (
      message.includes("failed to fetch") ||
      message.includes("fetch failed") ||
      message.includes("networkerror") ||
      message.includes("network error") ||
      message.includes("load failed")
    );
  }
  return false;
}

export async function tryStartGateway(
  baseUrl: string,
  gatewayUrlExplicit: boolean,
  setState: Setter<AppState>,
): Promise<boolean> {
  setState((previous) => ({
    ...previous,
    loading: true,
    connection: "connecting",
    error: undefined,
    settingsNotice: t("gatewayWaiting"),
    gatewayStartupNotice: t("gatewayWaiting"),
  }));
  if (isTauriRuntime()) {
    return tryConnectGatewayFromTauri(baseUrl, gatewayUrlExplicit, setState);
  }
  return tryConnectGatewayByHealth(baseUrl, setState);
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function tryConnectGatewayByHealth(
  baseUrl: string,
  setState: Setter<AppState>,
): Promise<boolean> {
  try {
    const controller = new AbortController();
    const timer = window.setTimeout(() => controller.abort(), GATEWAY_CONNECT_TIMEOUT_MS);
    const response = await fetch(`${baseUrl.replace(/\/+$/u, "")}/global/health`, {
      signal: controller.signal,
    }).finally(() => window.clearTimeout(timer));
    if (!response.ok) {
      if (response.status >= 500 && response.status <= 599) return false;
      throw new GatewayHealthContractError(
        `Gateway health endpoint returned HTTP ${response.status}.`,
      );
    }
    const body = (await response
      .clone()
      .json()
      .catch(() => undefined)) as { healthy?: unknown } | undefined;
    if (body?.healthy === false) return false;
    if (body?.healthy !== true) {
      throw new GatewayHealthContractError(
        "Gateway health response is incompatible: expected healthy=true.",
      );
    }
    setState((previous) => ({
      ...previous,
      settingsNotice: t("gatewayWaiting"),
      gatewayStartupNotice: t("gatewayWaiting"),
    }));
    return true;
  } catch (error) {
    if (error instanceof GatewayHealthContractError) throw error;
    return false;
  }
}

async function tryConnectGatewayFromTauri(
  baseUrl: string,
  gatewayUrlExplicit: boolean,
  setState: Setter<AppState>,
): Promise<boolean> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const payload = (await invoke("start_gateway", { gatewayUrl: baseUrl, gatewayUrlExplicit })) as
      | { ok?: boolean; status?: string; gatewayUrl?: string; gateway_url?: string }
      | undefined;
    const nextGatewayUrl = payload?.gatewayUrl ?? payload?.gateway_url;
    if (payload?.ok !== true || payload.status !== "connected" || !nextGatewayUrl) {
      throw new Error("Gateway startup returned an incompatible response.");
    }
    setState((previous) => ({
      ...previous,
      gatewayUrl: nextGatewayUrl ?? previous.gatewayUrl,
      settingsNotice: t("gatewayWaiting"),
      gatewayStartupNotice: t("gatewayWaiting"),
    }));
    return true;
  } catch (error) {
    if (error instanceof Error) throw error;
    throw new Error(String(error));
  }
}

export async function waitForGatewayHealth(
  baseUrl: string,
  timeoutMs: number,
  setState: Setter<AppState>,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  setState((previous) => ({
    ...previous,
    settingsNotice: t("gatewayWaiting"),
    gatewayStartupNotice: t("gatewayWaiting"),
  }));
  while (Date.now() < deadline) {
    try {
      const controller = new AbortController();
      const timer = window.setTimeout(() => controller.abort(), 500);
      const response = await fetch(`${baseUrl.replace(/\/+$/u, "")}/global/health`, {
        signal: controller.signal,
      }).finally(() => window.clearTimeout(timer));
      if (!response.ok) {
        if (response.status >= 500 && response.status <= 599) {
          await new Promise((resolve) => window.setTimeout(resolve, 500));
          continue;
        }
        throw new GatewayHealthContractError(
          `Gateway health endpoint returned HTTP ${response.status}.`,
        );
      }
      const body = (await response
        .clone()
        .json()
        .catch(() => undefined)) as { dev_log_path?: string; healthy?: unknown } | undefined;
      if (body?.healthy === false) {
        await new Promise((resolve) => window.setTimeout(resolve, 500));
        continue;
      }
      if (body?.healthy !== true) {
        throw new GatewayHealthContractError(
          "Gateway health response is incompatible: expected healthy=true.",
        );
      }
      const devPath = body?.dev_log_path;
      if (devPath) {
        setState((previous) => ({
          ...previous,
          settingsNotice: `${t("devModeActive")}${devPath}`,
          gatewayStartupNotice: `${t("devModeActive")}${devPath}`,
        }));
      } else {
        setState((previous) => ({
          ...previous,
          settingsNotice: undefined,
          gatewayStartupNotice: undefined,
        }));
      }
      return;
    } catch (error) {
      if (error instanceof GatewayHealthContractError) throw error;
      // Keep the loading overlay alive while waiting for Gateway to appear.
    }
    await new Promise((resolve) => window.setTimeout(resolve, 500));
  }
  throw new DOMException("Gateway did not become healthy within 20 seconds.", "TimeoutError");
}

export class GatewayHealthContractError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "GatewayHealthContractError";
  }
}
