export function mediaSource(path: string, workspaceDirectory?: string): string {
  if (/^(https?:|data:)/iu.test(path) || path.startsWith("/assets/")) return path;
  const query = new URLSearchParams({ path: localPathQueryValue(path) });
  if (workspaceDirectory) query.set("directory", workspaceDirectory);
  return `${gatewayBaseUrl()}/file/media?${query.toString()}`;
}

export function localPathQueryValue(value: string): string {
  const trimmed = value.trim();
  if (!/^file:\/\//iu.test(trimmed)) return trimmed;
  try {
    const url = new URL(trimmed);
    const pathname = decodeURIComponent(url.pathname || "");
    if (url.hostname && url.hostname !== "localhost") return `//${url.hostname}${pathname}`;
    return pathname.length >= 4 && pathname[0] === "/" && /^[A-Za-z]:[\\/]/u.test(pathname.slice(1))
      ? pathname.slice(1)
      : pathname;
  } catch {
    return trimmed;
  }
}

export function gatewayBaseUrl(): string {
  if (typeof window === "undefined") return "";
  const configured = new URLSearchParams(window.location.search).get("gatewayUrl")?.trim();
  if (configured) return configured.replace(/\/+$/u, "");
  return window.location.origin.replace(/\/+$/u, "");
}
