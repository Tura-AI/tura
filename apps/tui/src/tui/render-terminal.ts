import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";

export const clear = "\x1b[3J\x1b[2J\x1b[H";
export const reset = "\x1b[0m";
export const bold = "\x1b[1m";
export const italic = "\x1b[3m";
export const underline = "\x1b[4m";
export const strike = "\x1b[9m";
export const inverse = "\x1b[7m";
export const dim = "\x1b[2m";
export const gray = "\x1b[90m";
export const richInlineBg = "\x1b[48;5;236m";
export const richBlockBg = "\x1b[48;5;235m";
export const opencodePrimary = "\x1b[38;2;250;178;131m";
export const opencodeText = "\x1b[38;2;238;238;238m";
export const opencodeTextWeak = "\x1b[38;2;128;128;128m";
export const opencodeBorder = "\x1b[38;2;58;58;58m";
export const opencodeLine = "\x1b[38;2;58;58;58m";
export const opencodePanelBg = "\x1b[48;2;32;32;34m";
export const opencodeElementBg = "\x1b[48;2;38;38;40m";

const ansiControlPattern = /^(?:\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\)/;
const ansiControlGlobalPattern = /(?:\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\)/g;
const segmenter =
  typeof Intl !== "undefined" && "Segmenter" in Intl
    ? new Intl.Segmenter(undefined, { granularity: "grapheme" })
    : undefined;

export let activeCapabilities: TerminalCapabilities = detectTerminalCapabilities();

export function setActiveCapabilities(capabilities: TerminalCapabilities) {
  activeCapabilities = capabilities;
}

export function wrap(text: string, cols: number): string[] {
  const width = Math.max(20, cols - 2);
  const result: string[] = [];
  for (const inputLine of text.split(/\r?\n/)) {
    let line = "";
    let visible = 0;
    for (const segment of graphemes(inputLine)) {
      const next = graphemeWidth(segment);
      if (visible + next > width) {
        result.push(line);
        line = "";
        visible = 0;
      }
      line += segment;
      visible += next;
    }
    result.push(line);
  }
  return result;
}

export function wrapAnsi(text: string, cols: number): string[] {
  const width = Math.max(20, cols - 2);
  const result: string[] = [];
  for (const inputLine of text.split(/\r?\n/)) {
    let line = "";
    let visible = 0;
    for (let index = 0; index < inputLine.length; index += 1) {
      const char = inputLine[index];
      if (char === "\x1b") {
        const match = inputLine.slice(index).match(ansiControlPattern);
        if (match) {
          line += match[0];
          index += match[0].length - 1;
          continue;
        }
      }
      const segment = firstGrapheme(inputLine.slice(index));
      const next = graphemeWidth(segment);
      if (visible > 0 && visible + next > width) {
        result.push(line + reset);
        line = "";
        visible = 0;
      }
      line += segment;
      visible += next;
      index += segment.length - 1;
    }
    result.push(line);
  }
  return result;
}

export function fit(lines: string[], rows: number, cols: number): string[] {
  return lines.slice(0, rows).map((line) => truncateAnsi(line, cols));
}

export function truncate(text: string, width: number): string {
  const ellipsis = activeCapabilities.unicode ? "…" : "...";
  if (visibleTextWidth(text) <= width) return text;
  const limit = Math.max(0, width - visibleTextWidth(ellipsis));
  let visible = 0;
  let output = "";
  for (const segment of graphemes(text)) {
    const next = graphemeWidth(segment);
    if (visible + next > limit) return `${output}${ellipsis}`;
    output += segment;
    visible += next;
  }
  return output;
}

export function truncateAnsi(text: string, width: number): string {
  if (visibleTextWidth(text) <= width) return text;
  let visible = 0;
  let output = "";
  const ellipsis = activeCapabilities.unicode ? "…" : "...";
  const limit = Math.max(0, width - visibleTextWidth(ellipsis));
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (char === "\x1b") {
      const match = text.slice(index).match(ansiControlPattern);
      if (match) {
        output += match[0];
        index += match[0].length - 1;
        continue;
      }
    }
    const segment = firstGrapheme(text.slice(index));
    const next = graphemeWidth(segment);
    if (visible + next > limit) return `${output}${ellipsis}${reset}`;
    output += segment;
    visible += next;
    index += segment.length - 1;
  }
  return output;
}

export function rule(cols: number): string {
  const line = (activeCapabilities.unicode ? "─" : "-").repeat(cols);
  if (activeCapabilities.level === "rich") return `${opencodeBorder}${line}${reset}`;
  if (activeCapabilities.level === "ansi") return `${gray}${line}${reset}`;
  return line;
}

export function pad(text: string, width: number): string {
  const visible = visibleTextWidth(text);
  return visible >= width ? truncate(text, width) : `${text}${" ".repeat(width - visible)}`;
}

export function padVisible(text: string, width: number): string {
  const visible = visibleTextWidth(text);
  if (visible > width) return truncateAnsi(text, width);
  return `${text}${" ".repeat(width - visible)}`;
}

export function visibleTextWidth(text: string): number {
  let width = 0;
  for (const segment of graphemes(text.replace(ansiControlGlobalPattern, "")))
    width += graphemeWidth(segment);
  return width;
}

export function stripAnsi(text: string): string {
  return text.replace(ansiControlGlobalPattern, "");
}

function graphemes(text: string): string[] {
  if (!text) return [];
  return segmenter ? [...segmenter.segment(text)].map((item) => item.segment) : Array.from(text);
}

function firstGrapheme(text: string): string {
  return graphemes(text)[0] ?? "";
}

function graphemeWidth(segment: string): number {
  if (!segment) return 0;
  if (isZeroWidthSegment(segment)) return 0;
  if (isEmojiSegment(segment)) return 2;
  const base = Array.from(segment).find((char) => !isZeroWidthCode(char.codePointAt(0) ?? 0));
  const code = base?.codePointAt(0) ?? 0;
  if (code === 0) return 0;
  if (code < 32 || (code >= 0x7f && code < 0xa0)) return 0;
  if (
    (code >= 0x1100 && code <= 0x115f) ||
    code === 0x2329 ||
    code === 0x232a ||
    (code >= 0x2e80 && code <= 0xa4cf && code !== 0x303f) ||
    (code >= 0xac00 && code <= 0xd7a3) ||
    (code >= 0xf900 && code <= 0xfaff) ||
    (code >= 0xfe10 && code <= 0xfe19) ||
    (code >= 0xfe30 && code <= 0xfe6f) ||
    (code >= 0xff00 && code <= 0xff60) ||
    (code >= 0xffe0 && code <= 0xffe6) ||
    (code >= 0x1f300 && code <= 0x1faff)
  ) {
    return 2;
  }
  return 1;
}

function isEmojiSegment(segment: string): boolean {
  if (segment.includes("\u200d") || segment.includes("\ufe0f")) return true;
  for (const char of segment) {
    const code = char.codePointAt(0) ?? 0;
    if (
      (code >= 0x1f000 && code <= 0x1faff) ||
      (code >= 0x2600 && code <= 0x27bf) ||
      (code >= 0x2b50 && code <= 0x2b55)
    )
      return true;
  }
  return false;
}

function isZeroWidthSegment(segment: string): boolean {
  return Array.from(segment).every((char) => isZeroWidthCode(char.codePointAt(0) ?? 0));
}

function isZeroWidthCode(code: number): boolean {
  return (
    code === 0 ||
    code === 0x200d ||
    (code >= 0x300 && code <= 0x36f) ||
    (code >= 0x1ab0 && code <= 0x1aff) ||
    (code >= 0x1dc0 && code <= 0x1dff) ||
    (code >= 0x20d0 && code <= 0x20ff) ||
    (code >= 0xfe00 && code <= 0xfe0f)
  );
}
