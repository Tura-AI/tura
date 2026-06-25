import { invoke, isTauri } from "@tauri-apps/api/core";

export function isWebUrl(value: string | null | undefined): value is string {
  return typeof value === "string" && /^https?:\/\//iu.test(value.trim());
}

export function isSystemOpenUrl(value: string | null | undefined): value is string {
  return typeof value === "string" && /^(?:https?|file):\/\//iu.test(value.trim());
}

export async function openExternalUrl(url: string): Promise<void> {
  const target = normalizeExternalUrl(url);
  if (!target) {
    return;
  }

  if (await openWithTauri(target)) {
    return;
  }

  openWithBrowser(target);
}

async function openWithTauri(target: string): Promise<boolean> {
  try {
    await invoke("open_external_url", { url: target });
    return true;
  } catch (error) {
    if (isTauri()) {
      throw error;
    }
    return false;
  }
}

function openWithBrowser(target: string): void {
  const opened = window.open(target, "_blank", "noopener,noreferrer");
  if (opened || typeof document === "undefined") {
    return;
  }

  const link = document.createElement("a");
  link.href = target;
  link.target = "_blank";
  link.rel = "noopener noreferrer";
  link.click();
}

export function installExternalLinkInterceptor(): void {
  document.addEventListener("click", handleExternalLinkClick);
  document.addEventListener("auxclick", handleExternalLinkClick);
}

function handleExternalLinkClick(event: MouseEvent): void {
  if (event.defaultPrevented || ![0, 1].includes(event.button)) {
    return;
  }
  const target = event.target instanceof Element ? event.target.closest("a[href]") : null;
  if (!(target instanceof HTMLAnchorElement) || !isSystemOpenUrl(target.href)) {
    return;
  }
  event.preventDefault();
  void openExternalUrl(target.href).catch((error) => {
    console.error("Failed to open external URL", error);
  });
}

function normalizeExternalUrl(url: string): string | undefined {
  const trimmed = url.trim();
  if (!isSystemOpenUrl(trimmed)) {
    return undefined;
  }
  return trimmed;
}
