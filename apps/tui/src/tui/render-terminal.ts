import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";

export const clear = "\x1b[2J\x1b[H";
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
export const richTableBg = "\x1b[48;5;234m";
export const cyan = "\x1b[36m";
export const magenta = "\x1b[35m";
export const green = "\x1b[32m";
export const yellow = "\x1b[33m";
export const red = "\x1b[31m";

const ansiControlPattern = /^(?:\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\)/;
const ansiControlGlobalPattern = /(?:\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\)/g;

export let activeCapabilities: TerminalCapabilities = detectTerminalCapabilities();

export function setActiveCapabilities(capabilities: TerminalCapabilities) {
  activeCapabilities = capabilities;
}

export function wrap(text: string, cols: number): string[] {
  const width = Math.max(20, cols - 2);
  const result: string[] = [];
  for (const inputLine of text.split(/\r?\n/)) {
    let line = inputLine;
    while (line.length > width) {
      result.push(line.slice(0, width));
      line = line.slice(width);
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
      if (visible >= width) {
        result.push(line + reset);
        line = "";
        visible = 0;
      }
      line += char;
      visible += 1;
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
  return text.length > width
    ? `${text.slice(0, Math.max(0, width - ellipsis.length))}${ellipsis}`
    : text;
}

export function truncateAnsi(text: string, width: number): string {
  let visible = 0;
  let output = "";
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
    if (visible >= Math.max(0, width - 1))
      return `${output}${activeCapabilities.unicode ? "…" : "..."}${reset}`;
    output += char;
    visible += 1;
  }
  return output;
}

export function rule(cols: number): string {
  return (activeCapabilities.unicode ? "─" : "-").repeat(cols);
}

export function pad(text: string, width: number): string {
  return text.length >= width ? truncate(text, width) : `${text}${" ".repeat(width - text.length)}`;
}

export function padVisible(text: string, width: number): string {
  const visible = visibleTextWidth(text);
  if (visible > width) return truncateAnsi(text, width);
  return `${text}${" ".repeat(width - visible)}`;
}

export function visibleTextWidth(text: string): number {
  return text.replace(ansiControlGlobalPattern, "").length;
}

export function stripAnsi(text: string): string {
  return text.replace(ansiControlGlobalPattern, "");
}
