import { t } from "../i18n.js";
import { isInternalTaskStatusPayload } from "../types/session.js";
import {
  activeCapabilities,
  bold,
  dim,
  gray,
  inverse,
  italic,
  opencodeTextWeak,
  pad,
  reset,
  richBlockBg,
  richInlineBg,
  strike,
  stripAnsi,
  truncate,
  underline,
  visibleTextWidth,
} from "./render-terminal.js";

const osc8FullPattern = /\x1b\]8;;[^\x1b]*\x1b\\[\s\S]*?\x1b\]8;;\x1b\\/g;
const ansiSequencePattern = /\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\/g;
const rawAnsiControlPattern =
  /\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x1b]*(?:\x07|\x1b\\)|[PX^_][\s\S]*?\x1b\\|.)/gu;

// Keep whole assistant answers (lists, multi-step plans) intact. The transcript
// window bounds the visible height on its own, so this is only a sanity cap that
// prevents a pathological multi-thousand-line dump from being materialized.
const MAX_ASSISTANT_LINES = 200;

export function displayMessageText(role: string, value: string): string {
  const text = cleanMessageText(value);
  if (!text) return "";
  if (/completed without a user-facing message/i.test(text)) return "";
  if (role === "user") {
    const first = text.split(/\r?\n/).find((line) => line.trim()) ?? text;
    return truncate(first.trim(), 140);
  }
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim())
    .slice(0, MAX_ASSISTANT_LINES);
  return lines.join("\n");
}

function cleanMessageText(value: string): string {
  if (stringIsTaskStatusPayload(value)) return "";
  const text = sanitizeRawTerminalText(value)
    .replace(/<br\s*\/?>/g, "\n")
    .replace(/^\s*\[command_run:\s*[^\r\n\]]*\]\s*$/gimu, "")
    .replace(/\[command_run:\s*[^\r\n\]]*\]/giu, "")
    .replace(/data:image\/[a-z0-9.+-]+;base64,[A-Za-z0-9+/=]+/gi, `[${t("imageData")}]`)
    .replace(/[A-Za-z0-9+/]{180,}={0,2}/g, `[${t("encodedData")}]`);
  return compactInlinePayloads(text).trim();
}

export function sanitizeRawTerminalText(value: string): string {
  return value.replace(/\r\n/g, "\n").replace(/\r/g, "\n").replace(rawAnsiControlPattern, "");
}

export function compactInlinePayloads(value: string): string {
  const compactKnown = value.replace(
    /\[([a-z_][a-z0-9_-]*):\s*([\s\S]*?)\]/gu,
    (match, tool, payload) => {
      const summary =
        compactPayloadField(String(payload)) ??
        compactCommandJson(normalizePayloadText(String(payload))) ??
        summarizePayloadText(String(payload));
      return summary ? `[${String(tool)}: ${summary}]` : String(match);
    },
  );
  const inline = compactKnown.replace(
    /\[([a-z_][a-z0-9_-]*):\s*(\{[\s\S]*?\})\]/giu,
    (match, tool, payload) => {
      const summary =
        compactPayloadField(String(payload)) ??
        compactCommandJson(normalizePayloadText(String(payload))) ??
        summarizePayloadText(String(payload));
      return summary ? `[${String(tool)}: ${summary}]` : String(match);
    },
  );
  return inline
    .split(/\r?\n/)
    .map((line) => {
      const match = line.trim().match(/^\[([a-z_][a-z0-9_-]*):\s*([\s\S]+)\]$/iu);
      if (!match) return line;
      const tool = match[1] ?? t("tool");
      const payload = match[2] ?? "";
      const summary =
        compactPayloadField(payload) ??
        compactCommandJson(normalizePayloadText(payload)) ??
        summarizePayloadText(payload);
      return summary ? `[${tool}: ${summary}]` : line;
    })
    .join("\n");
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
      (body) => `${bold}${renderHtmlSubset(body)}${reset}`,
    ],
    [/<(?:i|em)>([\s\S]*?)<\/(?:i|em)>/giu, (body) => `${italic}${renderHtmlSubset(body)}${reset}`],
    [/<u>([\s\S]*?)<\/u>/giu, (body) => `${underline}${renderHtmlSubset(body)}${reset}`],
    [
      /<(?:s|del)>([\s\S]*?)<\/(?:s|del)>/giu,
      (body) => `${strike}${renderHtmlSubset(body)}${reset}`,
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
        `${bold}${heading[1]} ${stripAnsi(renderInlineMarkdown(heading[2] ?? ""))}${reset}`,
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
  return `${richBlockBg}${opencodeTextWeak}${value.replaceAll(reset, `${reset}${richBlockBg}${opencodeTextWeak}`)}${reset}`;
}

function quoteRegion(value: string): string {
  if (activeCapabilities.level !== "rich") return value;
  return `${richBlockBg}${value.replaceAll(reset, `${reset}${richBlockBg}`)}${reset}`;
}

function inlineRegion(value: string): string {
  if (activeCapabilities.level !== "rich") return value;
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (!normalized) return "";
  return `${richInlineBg}${opencodeTextWeak} ${normalized.replaceAll(reset, `${reset}${richInlineBg}${opencodeTextWeak}`)} ${reset}`;
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
    return `${gray}◇${reset} ${opencodeTextWeak}${cells.join("  ")}${reset}`;
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
    .replace(/(?<!\\)\*\*([^*\n]+)\*\*/gu, (_match, body) => `${bold}${body}${reset}`)
    .replace(/(?<!\\)__([^_\n]+)__/gu, (_match, body) => `${bold}${body}${reset}`)
    .replace(/(?<!\\)`([^`\n]+)`/gu, (_match, body) => inlineRegion(String(body)));
}

function renderMediaToken(path: string): string {
  const label = path;
  return isLinkTarget(path)
    ? terminalLink(linkTargetUrl(path), `${opencodeTextWeak}${label}${reset}`)
    : `${opencodeTextWeak}${label}${reset}`;
}

function renderLinkTarget(target: string, label: string): string {
  if (!isLinkTarget(target)) return `${label} (${target})`;
  const visible = `${stripAnsi(label)} ${opencodeTextWeak}(${target})${reset}`;
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
      return `${terminalLink(linkTargetUrl(path), `${opencodeTextWeak}${path}${reset}`)}${trailing}`;
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

function summarizePayloadText(value: string): string | undefined {
  if (
    !/[{[]/.test(value) ||
    !/(command_run|apply_patch|image_url|command_type|tool_result|results)/i.test(value)
  )
    return undefined;
  const normalized = normalizePayloadText(value);
  const snippets: string[] = [];
  for (const match of normalized.matchAll(/"path"\s*:\s*"([^"]+)"/g))
    snippets.push(`[${t("read")}: ${match[1]}]`);
  for (const match of normalized.matchAll(/"command_line"\s*:\s*"([^"]+)"/g)) {
    const command = decodeJsonString(match[1] ?? "");
    if (looksLikeCommand(command)) snippets.push(`[${t("bash")}: ${firstCommandLine(command)}]`);
  }
  for (const match of normalized.matchAll(/"command"\s*:\s*"([^"]+)"/g)) {
    const command = decodeJsonString(match[1] ?? "");
    if (looksLikeCommand(command)) snippets.push(`[${t("bash")}: ${firstCommandLine(command)}]`);
  }
  for (const match of normalized.matchAll(/"command_type"\s*:\s*"([^"]+)"/g))
    snippets.push(`[${t("tool")}: ${match[1]}]`);
  for (const match of normalized.matchAll(/"label"\s*:\s*"([^"]+)"/g)) {
    if (/img|image/i.test(match[1])) snippets.push(`[${t("media")}: ${match[1]}]`);
  }
  const unique = Array.from(new Set(snippets)).slice(0, 8);
  if (unique.length) return unique.join("\n");
  return `[${t("toolResult")}]`;
}

export function extractCommandsFromText(value: string): string[] {
  const text = sanitizeRawTerminalText(value).trim();
  if (!text) return [];
  const commands: string[] = [];
  for (const pattern of [
    /"command_line"\s*:\s*"([^"]+)"/g,
    /"command"\s*:\s*"([^"]+)"/g,
    /`([^`\n]*(?:npm|pnpm|yarn|node|python|cargo|git|rg|powershell|pwsh|cmd)\s+[^`\n]*)`/gi,
  ]) {
    for (const match of text.matchAll(pattern)) {
      const command = decodeJsonString(match[1] ?? "");
      if (looksLikeCommand(command)) commands.push(command);
    }
  }
  const compact = compactCommandJson(text);
  if (compact) {
    for (const command of extractCommandsFromText(compact)) commands.push(command);
  }
  return commands;
}

export function extractCommandsFromUnknown(value: unknown): string[] {
  if (!value) return [];
  if (typeof value === "string") return extractCommandsFromText(value);
  if (Array.isArray(value)) return value.flatMap(extractCommandsFromUnknown);
  if (typeof value !== "object") return [];
  const object = value as Record<string, unknown>;
  if (isTaskStatusPayload(object)) return [];
  const commands: string[] = [];
  for (const key of ["command_line", "command"]) {
    const command = object[key];
    if (typeof command === "string" && looksLikeCommand(command))
      commands.push(firstCommandLine(command));
  }
  for (const key of ["commands", "results", "steps", "input", "output"]) {
    commands.push(...extractCommandsFromUnknown(object[key]));
  }
  return commands;
}

export function firstCommandLine(value: string): string {
  return sanitizeRawTerminalText(value).trim().split(/\n/)[0]?.trim() ?? "";
}

function looksLikeCommand(value: string): boolean {
  const command = firstCommandLine(value);
  return (
    Boolean(command) &&
    !/^https?:\/\//i.test(command) &&
    (/^(?:npm|pnpm|yarn|node|python|py|cargo|git|rg|powershell|pwsh|cmd|npx|tsx|tsc|ls|dir|cd|cat|type|echo|mkdir|copy|xcopy|robocopy|del|rm|cp|mv)\b/i.test(
      command,
    ) ||
      /^[A-Z][A-Za-z]+-[A-Z][A-Za-z]+\b/u.test(command))
  );
}

function decodeJsonString(value: string): string {
  try {
    return sanitizeRawTerminalText(JSON.parse(`"${value.replace(/"/g, '\\"')}"`) as string);
  } catch {
    return sanitizeRawTerminalText(value.replace(/\\"/g, '"').replace(/\\\\/g, "\\"));
  }
}

export function toolSummary(state: Record<string, unknown>): string {
  const output = state.output;
  if (isTaskStatusPayload(output)) return "";
  if (typeof output === "string") {
    if (stringIsTaskStatusPayload(output)) return "";
    const clean = cleanMessageText(output);
    return (
      compactPayloadField(clean) ??
      summarizePayloadText(clean) ??
      compactCommandJson(clean) ??
      clean
    );
  }
  if (output && typeof output === "object") {
    const object = output as Record<string, unknown>;
    for (const key of ["reply_message", "text", "summary", "stdout", "stderr"]) {
      const value = object[key];
      if (typeof value === "string" && value.trim()) {
        const clean = cleanMessageText(value);
        return (
          compactPayloadField(clean) ??
          summarizePayloadText(clean) ??
          compactCommandJson(clean) ??
          clean
        );
      }
    }
  }
  const input = state.input;
  if (isTaskStatusPayload(input)) return "";
  if (input && typeof input === "object") {
    const object = input as Record<string, unknown>;
    for (const key of ["step_summary", "task_detail", "summary", "status", "label"]) {
      const value = object[key];
      if (typeof value === "string" && value.trim()) return sanitizeRawTerminalText(value).trim();
    }
    for (const key of ["command_line", "command"]) {
      const value = object[key];
      if (typeof value === "string" && looksLikeCommand(value)) return firstCommandLine(value);
    }
    const commands = object.commands;
    if (Array.isArray(commands)) {
      const first = commands.find((item) => item && typeof item === "object") as
        | Record<string, unknown>
        | undefined;
      const command = first?.command_line ?? first?.command ?? first?.command_type;
      if (typeof command === "string" && command.trim())
        return sanitizeRawTerminalText(command).trim();
    }
  }
  return "";
}

export function compactPayloadField(value: string): string | undefined {
  if (stringIsTaskStatusPayload(value)) return undefined;
  const normalized = normalizePayloadText(value);
  for (const key of ["task_detail", "summary", "status", "label"]) {
    const index = normalized.indexOf(key);
    if (index < 0) continue;
    const colon = normalized.indexOf(":", index + key.length);
    const open = colon >= 0 ? normalized.indexOf('"', colon + 1) : -1;
    const close = open >= 0 ? normalized.indexOf('"', open + 1) : -1;
    if (open >= 0 && close > open)
      return truncate(
        normalized
          .slice(open + 1, close)
          .trim()
          .replace(/\s+/g, " "),
        90,
      );
  }
  for (const key of ["task_detail", "summary", "status", "label"]) {
    const direct = normalized.match(new RegExp(`${key}\\\\*"?\\s*:\\s*\\\\*"([^"\\\\]+)`, "u"));
    if (direct?.[1]?.trim()) return truncate(direct[1].trim().replace(/\s+/g, " "), 90);
  }
  const start = normalized.indexOf("{");
  const end = normalized.lastIndexOf("}");
  if (start >= 0 && end > start) {
    const compact = compactCommandJson(normalized.slice(start, end + 1));
    if (compact) return compact;
  }
  for (const key of ["task_detail", "summary", "status", "label"]) {
    const match = normalized.match(new RegExp(`\\\\*"${key}\\\\*"\\s*:\\s*\\\\*"([^"\\\\]+)`, "u"));
    if (match?.[1]?.trim()) return truncate(match[1].trim().replace(/\s+/g, " "), 90);
  }
  const loose = normalized.replace(/[\\"]/g, "");
  for (const key of ["task_detail", "summary", "status", "label"]) {
    const index = loose.indexOf(`${key}:`);
    if (index < 0) continue;
    const rest = loose.slice(index + key.length + 1);
    const stop = rest.search(/[}\],]/u);
    const field = (stop >= 0 ? rest.slice(0, stop) : rest).trim();
    if (field) return truncate(field.replace(/\s+/g, " "), 90);
  }
  return undefined;
}

function normalizePayloadText(value: string): string {
  let normalized = sanitizeRawTerminalText(value);
  for (let index = 0; index < 5; index += 1) {
    const next = normalized.replace(/\\\\/g, "\\").replace(/\\"/g, '"');
    if (next === normalized) return normalized;
    normalized = next;
  }
  return normalized;
}

function compactCommandJson(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) return undefined;
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    return compactCommandValue(parsed);
  } catch {
    return undefined;
  }
}

function compactCommandValue(value: unknown): string | undefined {
  if (Array.isArray(value)) {
    const nested = value.map(compactCommandValue).filter(Boolean).slice(0, 4);
    return nested.length ? nested.join("  ") : undefined;
  }
  if (!value || typeof value !== "object") return undefined;
  const object = value as Record<string, unknown>;
  if (isTaskStatusPayload(object)) return undefined;
  const command = object.command ?? object.command_line;
  if (typeof command === "string" && command.trim()) return `[${t("bash")}: ${command.trim()}]`;
  const commandType = object.command_type;
  if (typeof commandType === "string" && commandType.trim())
    return `[${t("tool")}: ${commandType.trim()}]`;
  for (const key of ["task_detail", "summary", "status", "label"]) {
    const value = object[key];
    if (typeof value === "string" && value.trim())
      return truncate(value.trim().replace(/\s+/g, " "), 90);
  }
  const output = object.output;
  if (typeof output === "string" && output.trim())
    return truncate(output.trim().replace(/\s+/g, " "), 90);
  if (output && typeof output === "object") {
    const nested = compactCommandValue(output);
    if (nested) return nested;
  }
  for (const key of ["results", "commands", "changes"]) {
    const nested = compactCommandValue(object[key]);
    if (nested) return nested;
  }
  return undefined;
}

export function isTaskStatusPayload(value: unknown): boolean {
  if (isInternalTaskStatusPayload(value)) return true;
  return false;
}

function stringIsTaskStatusPayload(value: string): boolean {
  return isInternalTaskStatusPayload(sanitizeRawTerminalText(value));
}
