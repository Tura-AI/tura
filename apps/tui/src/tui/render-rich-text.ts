import { t } from "../i18n.js";
import {
  activeCapabilities,
  bold,
  dim,
  inverse,
  italic,
  pad,
  reset,
  richBlockBg,
  richInlineBg,
  strike,
  stripAnsi,
  textAgentRich,
  textSecondary,
  truncate,
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
    .filter((line) => line.trim())
    .slice(0, MAX_ASSISTANT_LINES);
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

export function renderRichText(source: string): string {
  if (!source) return "";
  source = compactInlinePayloads(source);
  if (activeCapabilities.richText === "none") return plainRichText(source);
  if (activeCapabilities.richText === "basicMarkdown") return basicRichText(source);
  const tokenized = source.replace(
    /\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu,
    (_match, media, path, mode, emoji) => {
      if (media) return renderMediaToken(String(path).trim());
      return mode === "react" ? `${dim}${String(emoji).trim()}${reset}` : String(emoji).trim();
    },
  );
  return renderInlineMarkdown(
    renderMarkdownRegions(renderMarkdownTables(renderHtmlSubset(tokenized))),
  );
}

function plainRichText(source: string): string {
  return renderMarkdownTables(
    decodeHtml(
      stripUnsupportedHtml(
        source
          .replace(
            /<a\s+href=['"]((?:https?:\/\/|file:\/\/)[^'"]+)['"][^>]*>([\s\S]*?)<\/a>/giu,
            (_match, href, body) => `${stripHtml(String(body))} (${href})`,
          )
          .replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/gu, "$1 ($2)")
          .replace(/\[MEDIA:([\s\S]*?):MEDIA\]/gu, "[MEDIA:$1:MEDIA]")
          .replace(/\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu, (_match, _mode, emoji) =>
            String(emoji).trim(),
          ),
      ),
    ),
  );
}

function basicRichText(source: string): string {
  const tokenized = source.replace(
    /\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu,
    (_match, media, path, _mode, emoji) => {
      if (media) return renderMediaToken(String(path).trim());
      return String(emoji).trim();
    },
  );
  return renderInlineMarkdown(renderMarkdownTables(renderHtmlSubset(tokenized)));
}

function renderHtmlSubset(source: string): string {
  let output = source;
  output = output.replace(
    /<pre(?:\s[^>]*)?>\s*<code(?:\s+class=['"]language-([^'"]+)['"])?>([\s\S]*?)<\/code>\s*<\/pre>/giu,
    (_match, language, body) => {
      return renderCodeFence(decodeHtml(body), language ? decodeHtml(language) : undefined);
    },
  );
  output = output.replace(/<blockquote>([\s\S]*?)<\/blockquote>/giu, (_match, body) =>
    decodeHtml(stripHtml(body))
      .split(/\r?\n/)
      .map((line) => quoteRegion(`${activeCapabilities.unicode ? "│" : ">"} ${line}`))
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
    [
      /<a\s+href=['"]((?:https?:\/\/|file:\/\/)[^'"]+)['"][^>]*>([\s\S]*?)<\/a>/giu,
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

function renderCodeFence(value: string, language?: string): string {
  const fence = `\`\`\`${language ?? ""}`;
  return [fence, ...codeBlockLines(value), "```"].map(blockRegion).join("\n");
}

function codeBlockLines(value: string): string[] {
  return value.replace(/\r\n/g, "\n").replace(/\n$/u, "").split("\n");
}

function renderMarkdownRegions(source: string): string {
  if (activeCapabilities.level !== "rich") return source;
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const output: string[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    const fence = lines[index].match(/^\s*```([A-Za-z0-9_-]+)?\s*$/u);
    if (fence) {
      output.push(blockRegion(lines[index]));
      index += 1;
      while (index < lines.length && !/^\s*```\s*$/u.test(lines[index])) {
        output.push(blockRegion(lines[index]));
        index += 1;
      }
      if (index < lines.length) output.push(blockRegion(lines[index]));
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
      output.push(quoteRegion(`${activeCapabilities.unicode ? "│" : ">"} ${quote[1] ?? ""}`));
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

function renderMarkdownTables(source: string): string {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const output: string[] = [];
  for (let index = 0; index < lines.length; ) {
    if (isMarkdownTableStart(lines, index)) {
      const table: string[][] = [tableCells(lines[index])];
      index += 2;
      while (index < lines.length && /^\s*\|.*\|\s*$/u.test(lines[index])) {
        table.push(tableCells(lines[index]));
        index += 1;
      }
      output.push(...formatMarkdownTable(table));
      continue;
    }
    output.push(lines[index]);
    index += 1;
  }
  return output.join("\n");
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

function formatMarkdownTable(rows: string[][]): string[] {
  const width = Math.max(...rows.map((row) => row.length));
  let normalized = rows.map((row) =>
    Array.from({ length: width }, (_item, index) => row[index] ?? ""),
  );
  if (activeCapabilities.level === "rich") {
    normalized = normalized.map((row) => row.map((cell) => renderInlineMarkdown(cell)));
  }
  const widths = Array.from({ length: width }, (_item, column) =>
    Math.min(48, Math.max(3, ...normalized.map((row) => visibleTextWidth(row[column])))),
  );
  if (activeCapabilities.level === "rich") {
    return compactMarkdownTable(normalized);
  }
  return normalized.map((row, index) => {
    const cells = row.map((cell, column) => pad(truncate(cell, widths[column]), widths[column]));
    const text = ` ${cells.join("  ")} `;
    return index === 0 ? `${bold}${text}${reset}` : text;
  });
}

function compactMarkdownTable(rows: string[][]): string[] {
  const headers = rows[0] ?? [];
  return rows.slice(1).map((row) => {
    const cells = row
      .map((cell, index) => {
        const header = stripAnsi(headers[index] ?? "").trim();
        const value = cell.trim();
        if (!value) return "";
        return header ? `${header}: ${value}` : value;
      })
      .filter(Boolean);
    return `${textSecondary}◇${reset} ${textAgentRich}${cells.join("  ")}${reset}`;
  });
}

function renderInlineMarkdown(source: string): string {
  const linked = source.replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/gu, (_match, label, href) =>
    renderLinkTarget(String(href), String(label)),
  );
  const localLinked = linkLocalPathsPreservingOsc(linked);
  const preserved = preserveAnsiSequences(localLinked);
  return restoreAnsiSequences(renderInlineDecorations(preserved.text), preserved.tokens);
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
    .replace(/(?<!\\)__([^_\n]+)__/gu, (_match, body) => `${textAgentRich}${bold}${body}${reset}`)
    .replace(/(?<!\\)`([^`\n]+)`/gu, (_match, body) => inlineRegion(String(body)));
}

function renderMediaToken(path: string): string {
  const label = path;
  return isLinkTarget(path)
    ? terminalLink(linkTargetUrl(path), `${textAgentRich}${label}${reset}`)
    : `${textAgentRich}${label}${reset}`;
}

function renderLinkTarget(target: string, label: string): string {
  if (!isLinkTarget(target)) return `${label} (${target})`;
  const visible = `${stripAnsi(label)} ${textAgentRich}(${target})${reset}`;
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
