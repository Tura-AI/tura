import type { Setter } from "solid-js";
import { t } from "./i18n";
import type { AppState } from "./state/global-store";

export const GATEWAY_CONNECT_TIMEOUT_MS = 5_000;

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
  setState: Setter<AppState>,
): Promise<boolean> {
  if (isTauriRuntime()) {
    return (
      (await tryStartGatewayFromTauri(baseUrl, setState)) ||
      (await tryStartGatewayFromDevServer(baseUrl, setState))
    );
  }
  return (
    (await tryStartGatewayFromDevServer(baseUrl, setState)) ||
    (await tryStartGatewayFromTauri(baseUrl, setState))
  );
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function tryStartGatewayFromDevServer(
  baseUrl: string,
  setState: Setter<AppState>,
): Promise<boolean> {
  setState((previous) => ({
    ...previous,
    loading: true,
    connection: "connecting",
    error: undefined,
    settingsNotice: t("gatewayStarting"),
    gatewayStartupNotice: t("gatewayStarting"),
  }));
  try {
    const controller = new AbortController();
    const timer = window.setTimeout(() => controller.abort(), 1_500);
    const response = await fetch("/__tura/start-gateway", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ gatewayUrl: baseUrl }),
      signal: controller.signal,
    }).finally(() => window.clearTimeout(timer));
    if (!response.ok) return false;
    const payload = (await response.json().catch(() => undefined)) as
      | { status?: string; message?: string }
      | undefined;
    const notice = payload?.status === "building" ? t("gatewayBuilding") : t("gatewayWaiting");
    setState((previous) => ({
      ...previous,
      settingsNotice: notice,
      gatewayStartupNotice: notice,
    }));
    return true;
  } catch {
    return false;
  }
}

async function tryStartGatewayFromTauri(
  baseUrl: string,
  setState: Setter<AppState>,
): Promise<boolean> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const payload = (await invoke("start_gateway", { gatewayUrl: baseUrl })) as
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
      const timer = window.setTimeout(() => controller.abort(), 1_200);
      const response = await fetch(`${baseUrl.replace(/\/+$/u, "")}/global/health`, {
        signal: controller.signal,
      }).finally(() => window.clearTimeout(timer));
      if (response.ok) {
        const body = (await response
          .clone()
          .json()
          .catch(() => undefined)) as { dev_log_path?: string } | undefined;
        const devPath = body?.dev_log_path;
        if (devPath) {
          setState((previous) => ({
            ...previous,
            settingsNotice: `${t("devModeActive")}${devPath}`,
            gatewayStartupNotice: `${t("devModeActive")}${devPath}`,
          }));
        }
        return;
      }
    } catch {
      // Keep the loading overlay alive while the dev server starts Gateway.
    }
    await new Promise((resolve) => window.setTimeout(resolve, 500));
  }
}
