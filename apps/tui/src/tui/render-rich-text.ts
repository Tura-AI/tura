import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { t } from "../i18n.js";
import {
  activeCapabilities,
  bold,
  dim,
  inverse,
  italic,
  padVisible,
  reset,
  richBlockBg,
  richInlineBg,
  strike,
  stripAnsi,
  textAgentRich,
  textAuxiliary,
  underline,
  visibleTextWidth,
} from "./render-terminal.js";
import { compactInlinePayloads, sanitizeRawTerminalText } from "./render-payload.js";

export {
  compactInlinePayloads,
  compactPayloadField,
  extractCommandsFromText,
  extractCommandsFromUnknown,
  firstCommandLine,
  looksLikeCommand,
  sanitizeRawTerminalText,
  toolSummary,
} from "./render-payload.js";

const osc8FullPattern = /\x1b\]8;;[^\x1b]*\x1b\\[\s\S]*?\x1b\]8;;\x1b\\/g;
const ansiSequencePattern = /\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\/g;
const protectedRichRegionPattern =
  /\x1b\[48;5;(?:234|236)m\x1b\[38;2;217;222;205m[\s\S]*?\x1b\[0m/g;

// Keep whole assistant answers (lists, multi-step plans) intact. The transcript
// window bounds the visible height on its own, so this is only a sanity cap that
// prevents a pathological multi-thousand-line dump from being materialized.
const MAX_ASSISTANT_LINES = 200;

type RenderRichTextOptions = {
  tableWidth?: number;
  workspaceDirectory?: string;
};

export function displayMessageText(role: string, value: string): string {
  const text = cleanMessageText(value);
  if (!text) return "";
  if (/completed without a user-facing message/i.test(text)) return "";
  if (role === "user") {
    return text
      .split(/\r?\n/)
      .map((line) => line.trimEnd())
      .join("\n")
      .trim();
  }
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .slice(0, MAX_ASSISTANT_LINES);
  while (lines.length && !lines[0]?.trim()) lines.shift();
  while (lines.length && !lines.at(-1)?.trim()) lines.pop();
  return lines.join("\n");
}

function cleanMessageText(value: string): string {
  const text = sanitizeRawTerminalText(value)
    .replace(/<br\s*\/?>/g, "\n")
    .replace(/^\s*\[command_run:\s*[^\r\n\]]*\]\s*$/gimu, "")
    .replace(/\[command_run:\s*[^\r\n\]]*\]/giu, "")
    .replace(/data:image\/[a-z0-9.+-]+;base64,[A-Za-z0-9+/=]+/gi, `[${t("imageData")}]`)
    .replace(/[A-Za-z0-9+/]{180,}={0,2}/g, `[${t("encodedData")}]`);
  return compactInlinePayloads(text).trim();
}

export function renderRichText(source: string, options: RenderRichTextOptions = {}): string {
  if (!source) return "";
  source = compactInlinePayloads(source);
  if (activeCapabilities.richText === "none") return plainRichText(source, options);
  if (activeCapabilities.richText === "basicMarkdown") return basicRichText(source, options);
  const tokenized = source.replace(
    /\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu,
    (_match, media, path, mode, emoji) => {
      if (media) return renderMediaToken(String(path).trim(), options);
      return mode === "react" ? `${dim}${String(emoji).trim()}${reset}` : String(emoji).trim();
    },
  );
  return renderInlineMarkdown(
    renderMarkdownRegions(
      renderMarkdownTables(renderHtmlSubset(tokenized, options), options.tableWidth, options),
    ),
    options,
  );
}

function plainRichText(source: string, options: RenderRichTextOptions): string {
  return renderMarkdownTables(
    decodeHtml(
      stripUnsupportedHtml(
        source
          .replace(/<a\s+href=['"]([^'"]+)['"][^>]*>([\s\S]*?)<\/a>/giu, (_match, _href, body) =>
            stripHtml(String(body)),
          )
          .replace(markdownLinkPattern, "$1")
          .replace(/\[MEDIA:([\s\S]*?):MEDIA\]/gu, "[MEDIA:$1:MEDIA]")
          .replace(/\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu, (_match, _mode, emoji) =>
            String(emoji).trim(),
          ),
      ),
    ),
    options.tableWidth,
    options,
  );
}

function basicRichText(source: string, options: RenderRichTextOptions): string {
  const tokenized = source.replace(
    /\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu,
    (_match, media, path, _mode, emoji) => {
      if (media) return renderMediaToken(String(path).trim(), options);
      return String(emoji).trim();
    },
  );
  return renderInlineMarkdown(
    renderMarkdownTables(renderHtmlSubset(tokenized, options), options.tableWidth, options),
    options,
  );
}

function renderHtmlSubset(source: string, options: RenderRichTextOptions = {}): string {
  let output = source;
  output = output.replace(
    /<pre(?:\s[^>]*)?>\s*<code(?:\s[^>]*)?>([\s\S]*?)<\/code>\s*<\/pre>/giu,
    (_match, body) => renderCodeFence(decodeHtml(body)),
  );
  output = output.replace(/<blockquote>([\s\S]*?)<\/blockquote>/giu, (_match, body) =>
    decodeHtml(stripHtml(body))
      .split(/\r?\n/)
      .map((line) => quoteRegion(line))
      .join("\n"),
  );
  const replacements: Array<[RegExp, (body: string, attr?: string) => string]> = [
    [
      /<(?:b|strong)>([\s\S]*?)<\/(?:b|strong)>/giu,
      (body) => `${textAgentRich}${bold}${renderHtmlSubset(body, options)}${reset}`,
    ],
    [
      /<(?:i|em)>([\s\S]*?)<\/(?:i|em)>/giu,
      (body) => `${textAgentRich}${italic}${renderHtmlSubset(body, options)}${reset}`,
    ],
    [
      /<u>([\s\S]*?)<\/u>/giu,
      (body) => `${textAgentRich}${underline}${renderHtmlSubset(body, options)}${reset}`,
    ],
    [
      /<(?:s|del)>([\s\S]*?)<\/(?:s|del)>/giu,
      (body) => `${textAgentRich}${strike}${renderHtmlSubset(body, options)}${reset}`,
    ],
    [
      /<code(?:\s[^>]*)?>([\s\S]*?)<\/code>/giu,
      (body) => inlineRegion(decodeHtml(stripHtml(body))),
    ],
    [
      /<span\s+class=['"]tg-spoiler['"]>([\s\S]*?)<\/span>/giu,
      (body) => `${inverse}${decodeHtml(stripHtml(body))}${reset}`,
    ],
    [/<mark>([\s\S]*?)<\/mark>/giu, (body) => `${inverse}${decodeHtml(stripHtml(body))}${reset}`],
    [
      /<a\s+href=['"]([^'"]+)['"][^>]*>([\s\S]*?)<\/a>/giu,
      (body, href) => renderLinkTarget(href ?? "", renderHtmlSubset(body, options), options),
    ],
  ];
  let changed = true;
  while (changed) {
    changed = false;
    for (const [pattern, format] of replacements) {
      output = output.replace(pattern, (match, first, second) => {
        changed = true;
        if (pattern.source.startsWith("<a")) return format(second, decodeHtml(first));
        return format(first);
      });
    }
  }
  return decodeHtml(stripUnsupportedHtml(output));
}

function renderCodeFence(value: string): string {
  return codeBlockRegionLines(codeBlockLines(value)).join("\n");
}

function codeBlockLines(value: string): string[] {
  return value.replace(/\r\n/g, "\n").replace(/\n$/u, "").split("\n");
}

function codeBlockRegionLines(lines: string[]): string[] {
  return [blockRegion(""), ...lines.map(blockRegion), blockRegion("")];
}

function renderMarkdownRegions(source: string): string {
  if (activeCapabilities.level !== "rich") return source;
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const output: string[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    const fence = lines[index].match(/^\s*(`{3,}|~{3,})[^`~]*$/u);
    if (fence) {
      const fenceMarker = fence[1] ?? "```";
      const fenceChar = fenceMarker[0] ?? "`";
      const closingFence = new RegExp(
        `^\\s*${escapeRegExp(fenceChar).repeat(fenceMarker.length)}${escapeRegExp(fenceChar)}*\\s*$`,
        "u",
      );
      pushBlankBeforeBlock(output);
      index += 1;
      const codeLines: string[] = [];
      while (index < lines.length && !closingFence.test(lines[index])) {
        codeLines.push(lines[index]);
        index += 1;
      }
      output.push(...codeBlockRegionLines(codeLines));
      pushBlankAfterBlock(output, lines, index + 1);
      continue;
    }
    const heading = lines[index].match(/^\s{0,3}(#{1,6})\s+(.+)$/u);
    if (heading) {
      output.push(
        `${textAgentRich}${bold}${heading[1]} ${stripAnsi(renderInlineMarkdown(heading[2] ?? ""))}${reset}`,
      );
      continue;
    }
    const quote = lines[index].match(/^\s{0,3}>\s?(.*)$/u);
    if (quote) {
      output.push(quoteRegion(quote[1] ?? ""));
      continue;
    }
    output.push(lines[index]);
  }
  return output.join("\n");
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function blockRegion(value: string): string {
  if (activeCapabilities.level !== "rich") return value;
  return `${richBlockBg}${textAgentRich}${value.replaceAll(reset, `${reset}${richBlockBg}${textAgentRich}`)}${reset}`;
}

function quoteRegion(value: string): string {
  if (activeCapabilities.level !== "rich") return value;
  return `${richBlockBg}${textAgentRich}${value.replaceAll(reset, `${reset}${richBlockBg}${textAgentRich}`)}${reset}`;
}

function inlineRegion(value: string): string {
  if (activeCapabilities.level !== "rich") return value;
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (!normalized) return "";
  return `${richInlineBg}${textAgentRich} ${normalized.replaceAll(reset, `${reset}${richInlineBg}${textAgentRich}`)} ${reset}`;
}

function renderMarkdownTables(
  source: string,
  tableWidth?: number,
  options: RenderRichTextOptions = {},
): string {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const output: string[] = [];
  for (let index = 0; index < lines.length; ) {
    if (isMarkdownTableStart(lines, index)) {
      pushBlankBeforeBlock(output);
      const table: string[][] = [tableCells(lines[index])];
      index += 2;
      while (index < lines.length && /^\s*\|.*\|\s*$/u.test(lines[index])) {
        table.push(tableCells(lines[index]));
        index += 1;
      }
      output.push(...formatMarkdownTable(table, tableWidth, options));
      pushBlankAfterBlock(output, lines, index);
      continue;
    }
    output.push(lines[index]);
    index += 1;
  }
  return output.join("\n");
}

function pushBlankBeforeBlock(output: string[]): void {
  if (output.length > 0 && output.at(-1)?.trim()) output.push("");
}

function pushBlankAfterBlock(output: string[], lines: string[], nextIndex: number): void {
  if (nextIndex >= lines.length || lines[nextIndex]?.trim()) output.push("");
}

function isMarkdownTableStart(lines: string[], index: number): boolean {
  return (
    index + 1 < lines.length &&
    /^\s*\|.*\|\s*$/u.test(lines[index]) &&
    /^\s*\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?\s*$/u.test(lines[index + 1])
  );
}

function tableCells(line: string): string[] {
  return line
    .trim()
    .replace(/^\|/u, "")
    .replace(/\|$/u, "")
    .split("|")
    .map((cell) => cell.trim());
}

function formatMarkdownTable(
  rows: string[][],
  tableWidth?: number,
  options: RenderRichTextOptions = {},
): string[] {
  const width = Math.max(...rows.map((row) => row.length));
  let normalized = rows.map((row) =>
    Array.from({ length: width }, (_item, index) => row[index] ?? ""),
  );
  if (activeCapabilities.level === "rich") {
    normalized = normalized.map((row) => row.map((cell) => renderInlineMarkdown(cell, options)));
  }
  const separator =
    activeCapabilities.level === "rich" && activeCapabilities.unicode
      ? ` ${textAuxiliary}│${textAgentRich} `
      : activeCapabilities.unicode
        ? " │ "
        : "  ";
  const widths = tableColumnWidths(normalized, separator, tableWidth);
  return normalized.map((row, index) => {
    const cells = row.map((cell, column) => tableCell(cell, widths[column]));
    const text = ` ${cells.join(separator)} `;
    if (activeCapabilities.level === "rich")
      return index === 0
        ? `${textAgentRich}${bold}${text}${reset}`
        : `${textAgentRich}${text}${reset}`;
    return index === 0 ? `${bold}${text}${reset}` : text;
  });
}

function tableColumnWidths(rows: string[][], separator: string, tableWidth?: number): number[] {
  const width = Math.max(...rows.map((row) => row.length), 0);
  const desired = Array.from({ length: width }, (_item, column) =>
    Math.min(48, Math.max(3, ...rows.map((row) => visibleTextWidth(row[column])))),
  );
  if (!tableWidth || width <= 0) return desired;
  const separatorWidth = visibleTextWidth(separator);
  const available = Math.max(1, tableWidth - 2 - separatorWidth * Math.max(0, width - 1));
  const widths = [...desired];
  const minimum = widths.map(() => 1);
  let total = widths.reduce((sum, item) => sum + item, 0);
  while (total > available) {
    let target = -1;
    for (const [index, value] of widths.entries()) {
      if (value <= minimum[index]) continue;
      if (target < 0 || value > widths[target]) target = index;
    }
    if (target < 0) break;
    widths[target] -= 1;
    total -= 1;
  }
  return widths;
}

function tableCell(value: string, width: number): string {
  const truncated = truncateAnsiForTable(value, width);
  return padVisible(truncated, width);
}

function truncateAnsiForTable(value: string, width: number): string {
  if (width <= 0) return "";
  if (visibleTextWidth(value) <= width) return value;
  const ellipsis = tableEllipsis(width);
  const limit = Math.max(0, width - visibleTextWidth(ellipsis));
  let visible = 0;
  let output = "";
  for (const token of ansiTokensForTable(value)) {
    if (token.control) {
      output += token.value;
      continue;
    }
    const segmentWidth = visibleTextWidth(token.value);
    if (visible + segmentWidth > limit) {
      return `${output}${ellipsis}${value.includes("\x1b]8;;") ? "\x1b]8;;\x1b\\" : ""}${reset}`;
    }
    output += token.value;
    visible += segmentWidth;
  }
  return output;
}

function tableEllipsis(width: number): string {
  if (width >= 3) return "...";
  return ".".repeat(width);
}

function* ansiTokensForTable(value: string): Generator<{ value: string; control: boolean }> {
  let plain = "";
  for (let index = 0; index < value.length; index += 1) {
    if (value[index] === "\x1b") {
      const match = value.slice(index).match(/^(?:\x1b\[[0-9;]*[Km]|\x1b\]8;;[^\x1b]*\x1b\\)/u);
      if (match) {
        if (plain) {
          for (const segment of graphemesForTable(plain)) yield { value: segment, control: false };
          plain = "";
        }
        yield { value: match[0], control: true };
        index += match[0].length - 1;
        continue;
      }
    }
    plain += value[index];
  }
  if (plain) {
    for (const segment of graphemesForTable(plain)) yield { value: segment, control: false };
  }
}

function graphemesForTable(value: string): string[] {
  const segmenter =
    typeof Intl !== "undefined" && "Segmenter" in Intl
      ? new Intl.Segmenter(undefined, { granularity: "grapheme" })
      : undefined;
  return segmenter ? [...segmenter.segment(value)].map((item) => item.segment) : Array.from(value);
}

const markdownLinkPattern = /\[([^\]\n]+)\]\(((?:<[^>\n]+>|[^()\n]+|\([^()\n]*\))+)\)/gu;

function renderInlineMarkdown(source: string, options: RenderRichTextOptions = {}): string {
  const linked = source.replace(markdownLinkPattern, (_match, label, href) =>
    renderLinkTarget(markdownLinkTarget(String(href)), String(label), options),
  );
  const preserved = preserveAnsiSequences(linked);
  const localLinked = linkLocalPathsPreservingOsc(preserved.text, options);
  return restoreAnsiSequences(renderInlineDecorations(localLinked), preserved.tokens);
}

function markdownLinkTarget(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  if (trimmed.startsWith("<")) {
    const close = trimmed.indexOf(">");
    if (close > 0) return decodeHtml(trimmed.slice(1, close).trim());
  }
  const withoutTitle = trimmed.match(/^(\S+)(?:\s+(?:"[^"]*"|'[^']*'|\([^)]*\)))?\s*$/u);
  return decodeHtml((withoutTitle?.[1] ?? trimmed).trim());
}

function preserveAnsiSequences(source: string): { text: string; tokens: string[] } {
  const tokens: string[] = [];
  const withLinksPreserved = source.replace(osc8FullPattern, (match) => {
    const index = tokens.push(match) - 1;
    return `\u0000ANSI${index}\u0000`;
  });
  const withRichRegionsPreserved = withLinksPreserved.replace(
    protectedRichRegionPattern,
    (match) => {
      const index = tokens.push(match) - 1;
      return `\u0000ANSI${index}\u0000`;
    },
  );
  const text = withRichRegionsPreserved.replace(ansiSequencePattern, (match) => {
    const index = tokens.push(match) - 1;
    return `\u0000ANSI${index}\u0000`;
  });
  return { text, tokens };
}

function restoreAnsiSequences(source: string, tokens: string[]): string {
  return source.replace(/\u0000ANSI(\d+)\u0000/gu, (_match, index) => tokens[Number(index)] ?? "");
}

function renderInlineDecorations(source: string): string {
  return source
    .replace(
      /(?<!\\)\*\*([^*\n]+)\*\*/gu,
      (_match, body) => `${textAgentRich}${bold}${body}${reset}`,
    )
    .replace(
      /(?<![A-Za-z0-9_\\])__(?![_\s])([^_\n]+?)(?<![\s_])__(?![A-Za-z0-9_])/gu,
      (_match, body) => `${textAgentRich}${bold}${body}${reset}`,
    )
    .replace(/(?<!\\)~~([^~\n]+)~~/gu, (_match, body) => `${textAgentRich}${strike}${body}${reset}`)
    .replace(
      /(?<!\\)==([^=\n]+)==/gu,
      (_match, body) => `${inverse}${decodeHtml(String(body))}${reset}`,
    )
    .replace(
      /(?<![*\\])\*(?!\*)([^*\n]+?)(?<!\*)\*(?!\*)/gu,
      (_match, body) => `${textAgentRich}${italic}${body}${reset}`,
    )
    .replace(
      /(?<![A-Za-z0-9_\\])_(?![_\s])([^_\n]+?)(?<![\s_])_(?![A-Za-z0-9_])/gu,
      (_match, body) => `${textAgentRich}${italic}${body}${reset}`,
    )
    .replace(/(?<!\\)`([^`\n]+)`/gu, (_match, body) => inlineRegion(String(body)));
}

function renderMediaToken(path: string, options: RenderRichTextOptions = {}): string {
  const label = path;
  return isLinkTarget(path)
    ? terminalLink(linkTargetUrl(path, options.workspaceDirectory), linkLabel(label, path))
    : `${textAgentRich}${label}${reset}`;
}

function renderLinkTarget(
  target: string,
  label: string,
  options: RenderRichTextOptions = {},
): string {
  const visibleLabel = stripAnsi(label).trim() || stripAnsi(target).trim();
  if (!isLinkTarget(target)) return visibleLabel;
  const visible = linkLabel(visibleLabel, target);
  return terminalLink(linkTargetUrl(target, options.workspaceDirectory), visible);
}

function linkLabel(label: string, target: string): string {
  const style = isLocalLinkTarget(target) ? `${textAgentRich}${underline}` : textAgentRich;
  return `${style}${label}${reset}`;
}

const LOCAL_PATH_PATTERN =
  /(?:[A-Za-z]:[\\/][^\r\n<>"'`]+|\\\\[^\\/\r\n<>"'`]+\\[^\\/\r\n<>"'`]+(?:\\[^\r\n<>"'`]+)*|\/[A-Za-z0-9_. -]+(?:\/[A-Za-z0-9_. -]+)+|\.{1,2}[\\/][^\r\n<>"'`]+|(?:[A-Za-z0-9_.-]+[\\/])+(?:[A-Za-z0-9_. -]+))/gu;
const FILE_URL_PATTERN = /file:\/\/[^\s\r\n<>"'`]+/giu;
const TRAILING_PATH_PUNCTUATION = /[),.;:!?]+$/u;
const KNOWN_FILE_EXTENSION_PATTERN =
  /\.(?:png|jpe?g|gif|webp|svg|bmp|mp4|mov|webm|m4v|mp3|wav|ogg|flac|pdf|md|markdown|txt|tsx?|jsx?|json|ya?ml|toml|html?|css|scss|rs|py|go|java|kt|swift|c|cc|cpp|h|hpp|cs)(?=$|[\s),.;:!?])/iu;

function linkLocalPaths(source: string, options: RenderRichTextOptions = {}): string {
  const fileLinks: string[] = [];
  const protectedFileLinks = source.replace(FILE_URL_PATTERN, (raw) => {
    const path = normalizeMatchedPath(raw, options);
    const trailing = raw.slice(path.length);
    const label = localFilesystemPath(path, options.workspaceDirectory) ?? path;
    const index =
      fileLinks.push(
        terminalLink(linkTargetUrl(path, options.workspaceDirectory), linkLabel(label, path)),
      ) - 1;
    return `\u0000FILELINK${index}\u0000${trailing}`;
  });
  const linked = protectedFileLinks.replace(LOCAL_PATH_PATTERN, (raw, offset: number) => {
    if (protectedFileLinks.slice(Math.max(0, offset - 10), offset).includes("\u0000FILELINK"))
      return raw;
    if (protectedFileLinks.slice(Math.max(0, offset - 8), offset).includes("[MEDIA:")) return raw;
    if (
      offset > 1 &&
      protectedFileLinks[offset - 2] === ":" &&
      protectedFileLinks[offset - 1] === "/"
    )
      return raw;
    if (
      /^[A-Za-z]:[\\/]/u.test(raw) &&
      offset > 0 &&
      /[A-Za-z0-9]/u.test(protectedFileLinks[offset - 1])
    )
      return raw;
    const path = normalizeMatchedPath(raw, options);
    const trailing = raw.slice(path.length);
    if (!isLocalPathReference(path)) return raw;
    if (activeCapabilities.level === "rich" || activeCapabilities.level === "ansi")
      return `${terminalLink(linkTargetUrl(path, options.workspaceDirectory), linkLabel(path, path))}${trailing}`;
    return `${path}${trailing}`;
  });
  return linked.replace(
    /\u0000FILELINK(\d+)\u0000/gu,
    (_match, index) => fileLinks[Number(index)] ?? "",
  );
}

function linkLocalPathsPreservingOsc(source: string, options: RenderRichTextOptions = {}): string {
  if (stripAnsi(source).trimStart().startsWith("◇")) return source;
  let cursor = 0;
  let output = "";
  for (const match of source.matchAll(osc8FullPattern)) {
    const index = match.index ?? 0;
    output += linkLocalPaths(source.slice(cursor, index), options);
    output += match[0];
    cursor = index + match[0].length;
  }
  output += linkLocalPaths(source.slice(cursor), options);
  return output;
}

function normalizeMatchedPath(raw: string, options: RenderRichTextOptions = {}): string {
  const existing = longestExistingMatchedPath(raw, options.workspaceDirectory);
  if (existing) return existing;
  const trimmed = raw.replace(TRAILING_PATH_PUNCTUATION, "");
  if (!/\s/u.test(trimmed)) return trimmed;
  const extension = trimmed.match(KNOWN_FILE_EXTENSION_PATTERN);
  if (extension?.index !== undefined) {
    return trimmed.slice(0, extension.index + extension[0].trimEnd().length);
  }
  return trimmed.trimEnd();
}

function longestExistingMatchedPath(raw: string, workspaceDirectory?: string): string | undefined {
  const candidates = matchedPathCandidates(raw);
  for (const candidate of candidates) {
    if (localTargetExists(candidate, workspaceDirectory)) return candidate;
  }
  return undefined;
}

function matchedPathCandidates(raw: string): string[] {
  const endings = new Set<number>([raw.length]);
  for (let end = raw.length; end > 0; end -= 1) {
    const previous = raw[end - 1] ?? "";
    if (!/[),.;:!?]/u.test(previous)) break;
    endings.add(end - 1);
  }
  for (let index = 0; index < raw.length; index += 1) {
    if (/[\s,.;:!?]/u.test(raw[index] ?? "")) endings.add(index);
    if (/[\])}]/u.test(raw[index] ?? "")) {
      endings.add(index);
      endings.add(index + 1);
    }
  }
  return [...endings]
    .map((end) => raw.slice(0, end).trimEnd())
    .filter((candidate) => candidate && isLocalLinkTarget(candidate))
    .sort((left, right) => right.length - left.length);
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

function terminalLink(url: string, label: string): string {
  return isLinkTarget(url) && activeCapabilities.osc8 && activeCapabilities.level !== "plain"
    ? `\x1b]8;;${url}\x1b\\${label}\x1b]8;;\x1b\\`
    : label;
}

function stripHtml(value: string): string {
  return stripUnsupportedHtml(value).replace(supportedHtmlTagPattern, "");
}

const supportedHtmlTags = new Set([
  "a",
  "address",
  "article",
  "aside",
  "b",
  "blockquote",
  "br",
  "caption",
  "code",
  "del",
  "details",
  "div",
  "em",
  "figcaption",
  "figure",
  "footer",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "header",
  "hr",
  "i",
  "li",
  "main",
  "mark",
  "nav",
  "ol",
  "p",
  "pre",
  "s",
  "section",
  "span",
  "strong",
  "summary",
  "table",
  "tbody",
  "td",
  "tfoot",
  "th",
  "thead",
  "tr",
  "u",
  "ul",
]);
const supportedHtmlTagPattern = /<\/?([A-Za-z][A-Za-z0-9_-]*)(?:\s[^<>]*)?\/?>/gu;

function stripUnsupportedHtml(value: string): string {
  const preserved = preserveMarkdownCodeFences(value);
  const stripped = preserved.text
    .replace(/<br\s*\/?>/giu, "\n")
    .replace(/<li(?:\s[^>]*)?>/giu, "\n")
    .replace(/<\/li>/giu, "\n")
    .replace(
      /<\/?(?:address|article|aside|blockquote|details|div|figcaption|figure|footer|h[1-6]|header|hr|main|nav|ol|p|pre|section|summary|table|tbody|td|tfoot|th|thead|tr|ul)(?:\s[^>]*)?>/giu,
      "\n",
    )
    .replace(supportedHtmlTagPattern, (match, tagName) =>
      supportedHtmlTags.has(String(tagName).toLowerCase()) ? "" : match,
    );
  return restoreMarkdownCodeFences(stripped, preserved.tokens);
}

function preserveMarkdownCodeFences(source: string): { text: string; tokens: string[] } {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const output: string[] = [];
  const tokens: string[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    const fence = lines[index].match(/^\s*(`{3,}|~{3,})[^`~]*$/u);
    if (!fence) {
      output.push(lines[index]);
      continue;
    }
    const fenceMarker = fence[1] ?? "```";
    const fenceChar = fenceMarker[0] ?? "`";
    const closingFence = new RegExp(
      `^\\s*${escapeRegExp(fenceChar).repeat(fenceMarker.length)}${escapeRegExp(fenceChar)}*\\s*$`,
      "u",
    );
    const block = [lines[index]];
    index += 1;
    while (index < lines.length) {
      block.push(lines[index]);
      if (closingFence.test(lines[index])) break;
      index += 1;
    }
    const token = `\u0000FENCE${tokens.length}\u0000`;
    tokens.push(block.join("\n"));
    output.push(token);
  }
  return { text: output.join("\n"), tokens };
}

function restoreMarkdownCodeFences(source: string, tokens: string[]): string {
  return source.replace(/\u0000FENCE(\d+)\u0000/gu, (_match, index) => tokens[Number(index)] ?? "");
}

function isLinkTarget(value: string): boolean {
  return /^(?:https?:\/\/|file:\/\/)/iu.test(value) || isLocalPathReference(value);
}

function isLocalLinkTarget(value: string): boolean {
  return /^file:\/\//iu.test(value) || isLocalPathReference(value);
}

function linkTargetUrl(value: string, workspaceDirectory?: string): string {
  if (/^(?:https?:\/\/|file:\/\/)/iu.test(value)) return value;
  return localPathUrl(value, workspaceDirectory);
}

function localPathUrl(value: string, workspaceDirectory?: string): string {
  const resolved = localFilesystemPath(value, workspaceDirectory) ?? value;
  const normalized = resolved.replace(/\\/g, "/");
  const withSlash = /^[A-Za-z]:\//u.test(normalized) ? `/${normalized}` : normalized;
  return `file://${encodeURI(withSlash)}`;
}

function localTargetExists(value: string, workspaceDirectory?: string): boolean {
  const resolved = localFilesystemPath(value, workspaceDirectory);
  return Boolean(resolved && existsSync(resolved));
}

function localFilesystemPath(value: string, workspaceDirectory?: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  if (/^file:\/\//iu.test(trimmed)) {
    try {
      return fileURLToPath(trimmed);
    } catch {
      return undefined;
    }
  }
  if (!isLocalPathReference(trimmed)) return undefined;
  if (path.isAbsolute(trimmed)) return trimmed;
  return path.resolve(workspaceDirectory || process.cwd(), trimmed);
}

function decodeHtml(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");
}
