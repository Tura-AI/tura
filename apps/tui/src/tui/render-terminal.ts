import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import { borderColor, reset, textSecondary } from "./styles/colors.js";
export {
  bold,
  borderColor,
  dim,
  inverse,
  italic,
  reset,
  richBlockBg,
  richHighlight,
  richInlineBg,
  strike,
  surfaceBackground,
  textAgentRich,
  textAuxiliary,
  textBackground,
  textPrimary,
  textSecondary,
  thinkingWaveBaseBlend,
  thinkingWaveGlow,
  thinkingWaveLow,
  thinkingWaveMid,
  underline,
} from "./styles/colors.js";

export const clear = "\x1bc\x1b[3J\x1b[2J\x1b[H\x1b[3J";

const ansiControlPattern = /^(?:\x1b\[[0-9;]*[Km]|\x1b\]8;;[^\x1b]*\x1b\\)/;
const ansiControlGlobalPattern = /(?:\x1b\[[0-9;]*[Km]|\x1b\]8;;[^\x1b]*\x1b\\)/g;
const sgrControlPattern = /^\x1b\[([0-9;]*)m/;
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
    let activeSgr = "";
    for (const token of ansiTokens(inputLine)) {
      if (token.control) {
        line += token.value;
        activeSgr = nextActiveSgr(activeSgr, token.value);
        continue;
      }
      const segment = token.value;
      const next = graphemeWidth(segment);
      if (visible > 0 && visible + next > width) {
        result.push(line + reset);
        line = activeSgr;
        visible = 0;
      }
      line += segment;
      visible += next;
    }
    result.push(line);
  }
  return result;
}

function nextActiveSgr(current: string, sequence: string): string {
  const match = sequence.match(sgrControlPattern);
  if (!match) return current;
  const codes = (match[1] || "0").split(";").filter(Boolean);
  if (!codes.length || codes.includes("0")) return "";
  return `${current}${sequence}`;
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
  for (const token of ansiTokens(text)) {
    if (token.control) {
      output += token.value;
      continue;
    }
    const segment = token.value;
    const next = graphemeWidth(segment);
    if (visible + next > limit) return `${output}${ellipsis}${reset}`;
    output += segment;
    visible += next;
  }
  return output;
}

export function rule(cols: number): string {
  const line = (activeCapabilities.unicode ? "─" : "-").repeat(cols);
  if (activeCapabilities.level === "rich") return `${borderColor}${line}${reset}`;
  if (activeCapabilities.level === "ansi") return `${textSecondary}${line}${reset}`;
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

function* ansiTokens(text: string): Generator<{ value: string; control: boolean }> {
  let plain = "";
  for (let index = 0; index < text.length; index += 1) {
    if (text[index] === "\x1b") {
      const match = text.slice(index).match(ansiControlPattern);
      if (match) {
        if (plain) {
          for (const segment of graphemes(plain)) yield { value: segment, control: false };
          plain = "";
        }
        yield { value: match[0], control: true };
        index += match[0].length - 1;
        continue;
      }
    }
    plain += text[index];
  }
  if (plain) {
    for (const segment of graphemes(plain)) yield { value: segment, control: false };
  }
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
    (code >= 0x16fe0 && code <= 0x16fff) ||
    (code >= 0x17000 && code <= 0x187ff) ||
    (code >= 0x18800 && code <= 0x18aff) ||
    (code >= 0x18b00 && code <= 0x18cff) ||
    (code >= 0x18d00 && code <= 0x18d8f) ||
    (code >= 0x1aff0 && code <= 0x1afff) ||
    (code >= 0x20000 && code <= 0x2fffd) ||
    (code >= 0x30000 && code <= 0x3fffd) ||
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
