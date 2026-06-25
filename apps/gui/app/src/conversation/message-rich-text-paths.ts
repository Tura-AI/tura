export type LocalTextReference =
  | { kind: "text"; text: string }
  | { kind: "local-path"; path: string; label?: string }
  | { kind: "web-link"; href: string; label: string };

const MARKDOWN_LINK_PATTERN = /\[([^\]\n]+)\]\(([^)\n]+)\)/gu;
const LOCAL_PATH_PATTERN =
  /(?:[A-Za-z]:[\\/][^\r\n<>"'`]+|\\\\[^\\/\r\n<>"'`]+\\[^\\/\r\n<>"'`]+(?:\\[^\r\n<>"'`]+)*|\/[A-Za-z0-9_. -]+(?:\/[A-Za-z0-9_. -]+)+|\.{1,2}[\\/][^\r\n<>"'`]+|(?:[A-Za-z0-9_.-]+[\\/])+(?:[A-Za-z0-9_. -]+))/gu;
const TRAILING_PATH_PUNCTUATION = /[),.;:!?]+$/u;
const KNOWN_FILE_EXTENSION_PATTERN =
  /\.(?:png|jpe?g|gif|webp|svg|bmp|mp4|mov|webm|m4v|mp3|wav|ogg|flac|pdf|md|markdown|txt|tsx?|jsx?|json|ya?ml|toml|html?|css|scss|rs|py|go|java|kt|swift|c|cc|cpp|h|hpp)(?=$|[\s),.;:!?])/iu;

export function parseLocalTextReferences(text: string): LocalTextReference[] {
  const nodes: LocalTextReference[] = [];
  let cursor = 0;
  for (const match of text.matchAll(MARKDOWN_LINK_PATTERN)) {
    const index = match.index ?? 0;
    if (index > cursor) {
      nodes.push(...splitLocalPathText(text.slice(cursor, index)));
    }
    const raw = match[0];
    const label = match[1] ?? "";
    const target = markdownLinkTarget(match[2] ?? "");
    if (isLocalLinkTarget(target)) {
      nodes.push({ kind: "local-path", path: localPathQueryValue(target), label });
    } else if (isSafeUrl(target)) {
      nodes.push({ kind: "web-link", href: target, label });
    } else {
      nodes.push({ kind: "text", text: raw });
    }
    cursor = index + raw.length;
  }
  if (cursor < text.length) {
    nodes.push(...splitLocalPathText(text.slice(cursor)));
  }
  return nodes.length > 0 ? nodes : [{ kind: "text", text }];
}

function splitLocalPathText(text: string): LocalTextReference[] {
  const nodes: LocalTextReference[] = [];
  let cursor = 0;
  for (const match of text.matchAll(LOCAL_PATH_PATTERN)) {
    const raw = match[0];
    const index = match.index ?? 0;
    const path = normalizeMatchedPath(raw);
    if (!path || !isLocalPathReference(path)) continue;
    if (index > cursor) nodes.push({ kind: "text", text: text.slice(cursor, index) });
    nodes.push({ kind: "local-path", path });
    const trailing = raw.slice(path.length);
    if (trailing) nodes.push({ kind: "text", text: trailing });
    cursor = index + raw.length;
  }
  if (cursor < text.length) nodes.push({ kind: "text", text: text.slice(cursor) });
  return nodes.length > 0 ? nodes : [{ kind: "text", text }];
}

function isLocalPath(value: string): boolean {
  return /^(?:[A-Za-z]:[\\/]|\\\\|\/|\.{1,2}[\\/])/u.test(value);
}

function isRelativePath(value: string): boolean {
  return (
    !/^[A-Za-z][A-Za-z0-9+.-]*:/u.test(value) && !value.startsWith("#") && /[\\/]/u.test(value)
  );
}

function isLocalPathReference(value: string): boolean {
  return isLocalPath(value) || isRelativePath(value);
}

export function isLocalLinkTarget(value: string): boolean {
  return /^file:\/\//iu.test(value) || isLocalPathReference(value);
}

function normalizeMatchedPath(raw: string): string {
  const trimmed = raw.replace(TRAILING_PATH_PUNCTUATION, "");
  if (!/\s/u.test(trimmed)) return trimmed;
  const extension = trimmed.match(KNOWN_FILE_EXTENSION_PATTERN);
  if (extension?.index !== undefined) {
    return trimmed.slice(0, extension.index + extension[0].trimEnd().length);
  }
  return trimmed.trimEnd();
}

function markdownLinkTarget(value: string): string {
  const trimmed = decodeHtml(value.trim());
  if (!trimmed) return "";
  if (trimmed.startsWith("<")) {
    const close = trimmed.indexOf(">");
    if (close > 0) return trimmed.slice(1, close).trim();
  }
  const withoutTitle = trimmed.match(/^(\S+)(?:\s+(?:"[^"]*"|'[^']*'|\([^)]*\)))?\s*$/u);
  return (withoutTitle?.[1] ?? trimmed).trim();
}

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

export function localOpenPathQueryValue(value: string): string {
  return stripSourceLineSuffix(normalizeWindowsFilePath(localPathQueryValue(value)));
}

function normalizeWindowsFilePath(value: string): string {
  return value.length >= 4 && value[0] === "/" && /^[A-Za-z]:[\\/]/u.test(value.slice(1))
    ? value.slice(1)
    : value;
}

function stripSourceLineSuffix(value: string): string {
  const match = value.match(
    /^(.+\.(?:tsx?|jsx?|json|ya?ml|toml|html?|css|scss|rs|py|go|java|kt|swift|c|cc|cpp|h|hpp|md|markdown|txt)):\d+(?::\d+)?$/iu,
  );
  return match?.[1] ?? value;
}

export function gatewayBaseUrl(): string {
  if (typeof window === "undefined") return "";
  const configured = new URLSearchParams(window.location.search).get("gatewayUrl")?.trim();
  if (configured) return configured.replace(/\/+$/u, "");
  return window.location.origin.replace(/\/+$/u, "");
}

function isSafeUrl(value: string): boolean {
  return /^https?:\/\//iu.test(value);
}

function decodeHtml(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");
}
