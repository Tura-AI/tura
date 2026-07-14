import assert from "node:assert/strict";
import { stripAnsi, visibleTextWidth } from "../../../../src/tui/render-terminal.js";

export const providerEnums = {
  domains: [],
  capabilities: [],
  api_styles: [],
  auth_methods: [],
  statuses: [],
};

export function withTerminalSize<T>(cols: number, rows: number, fn: () => T): T {
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const stdoutRows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: cols });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: rows });
  try {
    return fn();
  } finally {
    if (columns) Object.defineProperty(process.stdout, "columns", columns);
    else Reflect.deleteProperty(process.stdout, "columns");
    if (stdoutRows) Object.defineProperty(process.stdout, "rows", stdoutRows);
    else Reflect.deleteProperty(process.stdout, "rows");
  }
}

export function withNow<T>(now: number, fn: () => T): T {
  const original = Date.now;
  Date.now = () => now;
  try {
    return fn();
  } finally {
    Date.now = original;
  }
}

export function assertFitsTerminal(output: string, cols: number, rows: number): void {
  const lines = output.split("\n");
  assert.ok(lines.length <= rows, `expected at most ${rows} rows, got ${lines.length}`);
  assertLineWidths(output, cols);
}

export function assertLineWidths(output: string, cols: number): void {
  const lines = output.split("\n");
  for (const [index, line] of lines.entries()) {
    assert.ok(
      visibleTextWidth(line) <= cols,
      `line ${index + 1} overflows ${cols} cols: ${visibleTextWidth(line)} ${stripAnsi(line)}`,
    );
  }
}

export function assertOpencodePalette(output: string): void {
  assert.doesNotMatch(output, /\x1b\[(?:3[1-6]|9[1-6])m/u);
  assert.doesNotMatch(
    output,
    /\x1b\[38;2;(?!64;224;208m|70;199;190m|75;174;172m|81;149;154m|86;124;136m|244;247;235m|217;222;205m|151;160;153m|103;116;111m|54;63;61m|61;70;68m)/u,
  );
  assert.doesNotMatch(output, /\x1b\[48;2;(?!16;19;20m|20;23;24m|24;27;28m)/u);
}

export function assertWideMenuGap(
  line: string,
  label: string,
  description: string,
  minimumGap = 20,
): void {
  const text = stripAnsi(line);
  const labelIndex = text.indexOf(label);
  const descriptionIndex = text.indexOf(description);
  assert.ok(labelIndex >= 0, `missing label ${label}: ${text}`);
  assert.ok(descriptionIndex >= 0, `missing description ${description}: ${text}`);
  const gap = descriptionIndex - labelIndex - label.length;
  assert.ok(gap >= minimumGap, `expected wide menu label gap, got ${gap}: ${text}`);
}
