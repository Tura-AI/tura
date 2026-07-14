import { existsSync, statSync } from "node:fs";
import { basename } from "node:path";

export function richPromptFromInput(value: string): string {
  const trimmed = value.trim();
  if (!trimmed || /\[(?:MEDIA|EMOJI):/u.test(trimmed) || /\[[^\]]+\]\([^)]+\)/u.test(trimmed))
    return value;
  const paths = draggedPaths(trimmed);
  if (!paths.length) return value;
  return paths.map(richTokenForPath).join("\n");
}

function draggedPaths(value: string): string[] {
  const matches = Array.from(value.matchAll(/"([^"]+)"|'([^']+)'|(\S+)/gu))
    .map((match) => match[1] ?? match[2] ?? match[3])
    .filter(Boolean);
  if (!matches.length) return [];
  return matches.every((item) => isExistingLocalPath(item)) ? matches : [];
}

function richTokenForPath(path: string): string {
  if (isMediaPath(path)) return `[MEDIA:${path}:MEDIA]`;
  const label = basename(path.replace(/[\\/]+$/u, "")) || path;
  return `[${label}](${fileUrl(path)})`;
}

function isExistingLocalPath(path: string): boolean {
  try {
    return existsSync(path);
  } catch {
    return false;
  }
}

function isMediaPath(path: string): boolean {
  try {
    const stat = statSync(path);
    if (stat.isDirectory()) return false;
  } catch {
    return false;
  }
  return /\.(?:png|jpe?g|gif|webp|svg|bmp|mp4|mov|webm|mp3|wav|ogg)$/iu.test(path);
}

function fileUrl(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const withSlash = /^[A-Za-z]:\//u.test(normalized) ? `/${normalized}` : normalized;
  return `file://${encodeURI(withSlash)}`;
}
