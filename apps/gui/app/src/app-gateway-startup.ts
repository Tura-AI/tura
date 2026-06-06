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
  return error instanceof TypeError && error.message.toLowerCase() === "failed to fetch";
}

export async function tryStartGateway(
  baseUrl: string,
  setState: Setter<AppState>,
): Promise<boolean> {
  return (
    (await tryStartGatewayFromDevServer(baseUrl, setState)) ||
    (await tryStartGatewayFromTauri(baseUrl, setState))
  );
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
    const response = await fetch("/__tura/start-gateway", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ gatewayUrl: baseUrl }),
    });
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
      | { status?: string }
      | undefined;
    const notice = payload?.status === "connected" ? t("gatewayWaiting") : t("gatewayWaiting");
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
      const response = await fetch(`${baseUrl.replace(/\/+$/u, "")}/global/health`);
      if (response.ok) return;
    } catch {
      // Keep the loading overlay alive while the dev server starts Gateway.
    }
    await new Promise((resolve) => window.setTimeout(resolve, 500));
  }
}
