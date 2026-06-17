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
import {
  compactInlinePayloads,
  isTaskStatusPayload,
  sanitizeRawTerminalText,
} from "./render-payload.js";

export {
  compactInlinePayloads,
  compactPayloadField,
  extractCommandsFromText,
  extractCommandsFromUnknown,
  firstCommandLine,
  isTaskStatusPayload,
  looksLikeCommand,
  sanitizeRawTerminalText,
  toolSummary,
} from "./render-payload.js";

const osc8FullPattern = /\x1b\]8;;[^\x1b]*\x1b\\[\s\S]*?\x1b\]8;;\x1b\\/g;
const ansiSequencePattern = /\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\/g;

// Keep whole assistant answers (lists, multi-step plans) intact. The transcript
// window bounds the visible height on its own, so this is only a sanity cap that
// prevents a pathological multi-thousand-line dump from being materialized.
const MAX_ASSISTANT_LINES = 200;

type RenderRichTextOptions = {
  tableWidth?: number;
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
  if (isTaskStatusPayload(value)) return "";
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
      if (media) return renderMediaToken(String(path).trim());
      return mode === "react" ? `${dim}${String(emoji).trim()}${reset}` : String(emoji).trim();
    },
  );
  return renderInlineMarkdown(
    renderMarkdownRegions(renderMarkdownTables(renderHtmlSubset(tokenized), options.tableWidth)),
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
  );
}

function basicRichText(source: string, options: RenderRichTextOptions): string {
  const tokenized = source.replace(
    /\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu,
    (_match, media, path, _mode, emoji) => {
      if (media) return renderMediaToken(String(path).trim());
      return String(emoji).trim();
    },
  );
  return renderInlineMarkdown(
    renderMarkdownTables(renderHtmlSubset(tokenized), options.tableWidth),
  );
}

function renderHtmlSubset(source: string): string {
  let output = source;
  output = output.replace(
    /<pre(?:\s[^>]*)?>\s*<code(?:\s+class=['"]language-([^'"]+)['"])?>([\s\S]*?)<\/code>\s*<\/pre>/giu,
    (_match, _language, body) => {
      return renderCodeFence(decodeHtml(body));
    },
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
      (body) => `${textAgentRich}${bold}${renderHtmlSubset(body)}${reset}`,
    ],
    [
      /<(?:i|em)>([\s\S]*?)<\/(?:i|em)>/giu,
      (body) => `${textAgentRich}${italic}${renderHtmlSubset(body)}${reset}`,
    ],
    [
      /<u>([\s\S]*?)<\/u>/giu,
      (body) => `${textAgentRich}${underline}${renderHtmlSubset(body)}${reset}`,
    ],
    [
      /<(?:s|del)>([\s\S]*?)<\/(?:s|del)>/giu,
      (body) => `${textAgentRich}${strike}${renderHtmlSubset(body)}${reset}`,
    ],
    [/<code>([\s\S]*?)<\/code>/giu, (body) => inlineRegion(decodeHtml(stripHtml(body)))],
    [
      /<span\s+class=['"]tg-spoiler['"]>([\s\S]*?)<\/span>/giu,
      (body) => `${inverse}${decodeHtml(stripHtml(body))}${reset}`,
    ],
    [/<mark>([\s\S]*?)<\/mark>/giu, (body) => `${inverse}${decodeHtml(stripHtml(body))}${reset}`],
    [
      /<a\s+href=['"]([^'"]+)['"][^>]*>([\s\S]*?)<\/a>/giu,
      (body, href) => renderLinkTarget(href ?? "", renderHtmlSubset(body)),
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
    const fence = lines[index].match(/^\s*```([A-Za-z0-9_-]+)?\s*$/u);
    if (fence) {
      pushBlankBeforeBlock(output);
      index += 1;
      const codeLines: string[] = [];
      while (index < lines.length && !/^\s*```\s*$/u.test(lines[index])) {
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

function renderMarkdownTables(source: string, tableWidth?: number): string {
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
      output.push(...formatMarkdownTable(table, tableWidth));
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

function formatMarkdownTable(rows: string[][], tableWidth?: number): string[] {
  const width = Math.max(...rows.map((row) => row.length));
  let normalized = rows.map((row) =>
    Array.from({ length: width }, (_item, index) => row[index] ?? ""),
  );
  if (activeCapabilities.level === "rich") {
    normalized = normalized.map((row) => row.map((cell) => renderInlineMarkdown(cell)));
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

const markdownLinkPattern = /\[([^\]\n]+)\]\(([^)\n]+)\)/gu;

function renderInlineMarkdown(source: string): string {
  const linked = source.replace(markdownLinkPattern, (_match, label, href) =>
    renderLinkTarget(markdownLinkTarget(String(href)), String(label)),
  );
  const localLinked = linkLocalPathsPreservingOsc(linked);
  const preserved = preserveAnsiSequences(localLinked);
  return restoreAnsiSequences(renderInlineDecorations(preserved.text), preserved.tokens);
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
  const text = source.replace(ansiSequencePattern, (match) => {
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

function renderMediaToken(path: string): string {
  const label = path;
  return isLinkTarget(path)
    ? terminalLink(linkTargetUrl(path), `${textAgentRich}${label}${reset}`)
    : `${textAgentRich}${label}${reset}`;
}

function renderLinkTarget(target: string, label: string): string {
  const visibleLabel = stripAnsi(label).trim() || stripAnsi(target).trim();
  if (!isLinkTarget(target)) return visibleLabel;
  const visible = `${textAgentRich}${visibleLabel}${reset}`;
  return terminalLink(linkTargetUrl(target), visible);
}

const LOCAL_PATH_PATTERN =
  /(?:[A-Za-z]:[\\/][^\s<>"'`]+|\\\\[^\\/\s<>"'`]+\\[^\\/\s<>"'`]+(?:\\[^\s<>"'`]+)*|\/[A-Za-z0-9_.-]+(?:\/[A-Za-z0-9_.-]+)+|\.{1,2}[\\/][^\s<>"'`]+)/gu;
const TRAILING_PATH_PUNCTUATION = /[),.;:!?]+$/u;

function linkLocalPaths(source: string): string {
  return source.replace(LOCAL_PATH_PATTERN, (raw, offset: number) => {
    if (source.slice(Math.max(0, offset - 8), offset).includes("[MEDIA:")) return raw;
    if (offset > 1 && source[offset - 2] === ":" && source[offset - 1] === "/") return raw;
    if (/^[A-Za-z]:[\\/]/u.test(raw) && offset > 0 && /[A-Za-z0-9]/u.test(source[offset - 1]))
      return raw;
    const path = raw.replace(TRAILING_PATH_PUNCTUATION, "");
    const trailing = raw.slice(path.length);
    if (!isLocalPath(path)) return raw;
    if (activeCapabilities.level === "rich" || activeCapabilities.level === "ansi")
      return `${terminalLink(linkTargetUrl(path), `${textAgentRich}${path}${reset}`)}${trailing}`;
    return `${path}${trailing}`;
  });
}

function linkLocalPathsPreservingOsc(source: string): string {
  if (stripAnsi(source).trimStart().startsWith("◇")) return source;
  let cursor = 0;
  let output = "";
  for (const match of source.matchAll(osc8FullPattern)) {
    const index = match.index ?? 0;
    output += linkLocalPaths(source.slice(cursor, index));
    output += match[0];
    cursor = index + match[0].length;
  }
  output += linkLocalPaths(source.slice(cursor));
  return output;
}

function isLocalPath(value: string): boolean {
  return /^(?:[A-Za-z]:[\\/]|\\\\|\/|\.{1,2}[\\/])/u.test(value);
}

function terminalLink(url: string, label: string): string {
  return isLinkTarget(url) && activeCapabilities.osc8 && activeCapabilities.level !== "plain"
    ? `\x1b]8;;${url}\x1b\\${label}\x1b]8;;\x1b\\`
    : label;
}

function stripHtml(value: string): string {
  return stripUnsupportedHtml(value).replace(/<[^>]+>/gu, "");
}

function stripUnsupportedHtml(value: string): string {
  return value
    .replace(/<br\s*\/?>/giu, "\n")
    .replace(/<\/?(?:p|div)>/giu, "\n")
    .replace(/<\/?[^>]+>/gu, "");
}

function isLinkTarget(value: string): boolean {
  return /^(?:https?:\/\/|file:\/\/)/iu.test(value) || isLocalPath(value);
}

function linkTargetUrl(value: string): string {
  if (/^(?:https?:\/\/|file:\/\/)/iu.test(value)) return value;
  return localPathUrl(value);
}

function localPathUrl(value: string): string {
  const normalized = value.replace(/\\/g, "/");
  const withSlash = /^[A-Za-z]:\//u.test(normalized) ? `/${normalized}` : normalized;
  return `file://${encodeURI(withSlash)}`;
}

function decodeHtml(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");
}
