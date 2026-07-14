import { stripAnsi, truncateAnsi, visibleTextWidth } from "../render-terminal.js";

export const COMPOSER_CURSOR_MARKER = "\x01\x02\x01";

export type RenderedFrame = {
  frame: string;
  cursor?: { row: number; column: number };
};

export function finalizeFrame(lines: string[], _rows: number, cols: number): RenderedFrame {
  const renderedLines = lines.map((line) => truncateAnsi(line, cols));
  const cursor = findComposerCursor(renderedLines);
  return {
    frame: renderedLines.map((line) => line.replace(COMPOSER_CURSOR_MARKER, "")).join("\n"),
    cursor,
  };
}

export function terminalRenderCols(cols: number): number {
  return Math.max(20, cols - 1);
}

export function plainFrame(frame: RenderedFrame): RenderedFrame {
  return { ...frame, frame: stripAnsi(frame.frame) };
}

function findComposerCursor(lines: string[]): RenderedFrame["cursor"] {
  for (const [rowIndex, line] of lines.entries()) {
    const markerIndex = line.indexOf(COMPOSER_CURSOR_MARKER);
    if (markerIndex < 0) continue;
    return {
      row: rowIndex + 1,
      column: Math.max(1, visibleTextWidth(line.slice(0, markerIndex)) + 1),
    };
  }
  return undefined;
}
