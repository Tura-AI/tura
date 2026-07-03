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
    if (!response.ok) return false;
    const body = (await response
      .clone()
      .json()
      .catch(() => undefined)) as { healthy?: unknown } | undefined;
    if (body?.healthy !== true) return false;
    setState((previous) => ({
      ...previous,
      settingsNotice: t("gatewayWaiting"),
      gatewayStartupNotice: t("gatewayWaiting"),
    }));
    return true;
  } catch {
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
      | { status?: string; gatewayUrl?: string; gateway_url?: string }
      | undefined;
    const nextGatewayUrl = payload?.gatewayUrl ?? payload?.gateway_url;
    const notice = payload?.status === "connected" ? t("gatewayWaiting") : t("gatewayWaiting");
    setState((previous) => ({
      ...previous,
      gatewayUrl: nextGatewayUrl ?? previous.gatewayUrl,
      settingsNotice: notice,
      gatewayStartupNotice: notice,
    }));
    return true;
  } catch {
    return false;
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
      if (response.ok) {
        const body = (await response
          .clone()
          .json()
          .catch(() => undefined)) as { dev_log_path?: string; healthy?: unknown } | undefined;
        if (body?.healthy !== true) {
          await new Promise((resolve) => window.setTimeout(resolve, 500));
          continue;
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
      }
    } catch {
      // Keep the loading overlay alive while waiting for Gateway to appear.
    }
    await new Promise((resolve) => window.setTimeout(resolve, 500));
  }
  throw new DOMException("Gateway did not become healthy within 20 seconds.", "TimeoutError");
}
