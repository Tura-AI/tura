import type { AppState } from "./reducer.js";
import { renderChatFrameParts, renderFrame } from "./render.js";
import { clear as terminalClear } from "./render-terminal.js";
import type { TerminalCapabilities } from "./capabilities.js";

let lastDrawSurface = "";
let lastDrawSessionID = "";
let lastChatCacheLineCount = 0;
let lastChatCommittedLiveLineCount = 0;
let lastChatReservationLineCount = 0;
let lastChatLiveFrame = "";
let lastChatChromeFrame = "";
let lastChatLiveStreamKey = "";
let lastChatRenderCols = 0;
let hasSavedChatScrollbackCursor = false;

export function resetDrawState(): void {
  lastDrawSurface = "";
  lastDrawSessionID = "";
  lastChatCacheLineCount = 0;
  lastChatCommittedLiveLineCount = 0;
  lastChatReservationLineCount = 0;
  lastChatLiveFrame = "";
  lastChatChromeFrame = "";
  lastChatLiveStreamKey = "";
  lastChatRenderCols = 0;
  hasSavedChatScrollbackCursor = false;
}

export function clearTerminalForSurfaceTransition(): void {
  if (!process.stdout.isTTY) return;
  process.stdout.write(terminalSurfaceClear());
}

export function draw(
  state: AppState,
  capabilities: TerminalCapabilities,
  previousFrame = "",
  options: { forceReset?: boolean } = {},
): string {
  if (!process.stdout.isTTY) return previousFrame;
  const surface = drawSurface(state);
  const rendered =
    surface === "chat"
      ? renderChatFrameParts(state, capabilities)
      : renderFrame(state, capabilities);
  const frame = rendered.frame;
  const sessionID = state.session?.id ?? "";
  const previousSurface = lastDrawSurface;
  const previousSessionID = lastDrawSessionID;
  const surfaceChanged = Boolean(previousSurface) && previousSurface !== surface;
  const shouldClearForSurface =
    options.forceReset || previousSessionID !== sessionID || surfaceChanged;
  lastDrawSurface = surface;
  lastDrawSessionID = sessionID;

  if (surface === "chat") {
    return drawChatFrame(
      rendered as ReturnType<typeof renderChatFrameParts>,
      previousFrame,
      shouldClearForSurface,
    );
  }

  let output = "\x1b[?25l";
  output += terminalSurfaceClear();
  output += terminalAppendFrame(frame);
  output += cursorOutputFromFrameEnd(frame, rendered.cursor);
  process.stdout.write(output);
  return frame;
}

function drawChatFrame(
  rendered: ReturnType<typeof renderChatFrameParts>,
  _previousFrame: string,
  forceReset: boolean,
): string {
  const frame = rendered.frame;
  const renderWidthChanged = lastChatRenderCols !== 0 && rendered.renderCols !== lastChatRenderCols;
  const liveStreamChanged = lastChatLiveStreamKey !== rendered.liveStreamKey;
  const target = chatScrollbackTarget(rendered, liveStreamChanged || renderWidthChanged);
  const previousBodyLineCount = lastChatCacheLineCount + lastChatCommittedLiveLineCount;
  const previousTotalLineCount = previousBodyLineCount + lastChatReservationLineCount;
  const bodyShrank = previousBodyLineCount !== 0 && target.bodyLines.length < previousBodyLineCount;
  const firstChatDraw = lastChatRenderCols === 0;
  const rewriteAllRegions = forceReset || renderWidthChanged || bodyShrank || firstChatDraw;
  const liveChanged = lastChatLiveFrame !== rendered.liveFrame;
  const chromeChanged = lastChatChromeFrame !== rendered.chromeFrame;
  const bodyChanged = target.bodyLines.length !== previousBodyLineCount;
  const reservationChanged = target.mutableLines.length !== lastChatReservationLineCount;
  const rewriteMutableRegion = liveChanged || chromeChanged || reservationChanged;

  if (!rewriteAllRegions && !bodyChanged && !rewriteMutableRegion) {
    return frame;
  }

  let output = "";
  let effectiveReservationLineCount = target.mutableLines.length;
  if (rewriteAllRegions) {
    const scrollbackLines = [...target.bodyLines, ...blankLines(effectiveReservationLineCount)];
    output += "\x1b[?25l";
    output += terminalSurfaceClear();
    output += terminalAppendLines(scrollbackLines);
    output += terminalSaveCursor();
    hasSavedChatScrollbackCursor = true;
  } else {
    const newBodyLines = target.bodyLines.slice(previousBodyLineCount);
    const materializedReservationLineCount = Math.min(
      newBodyLines.length,
      lastChatReservationLineCount,
    );
    const materializedLines = newBodyLines.slice(0, materializedReservationLineCount);
    const appendedBodyLines = newBodyLines.slice(materializedReservationLineCount);
    const residualReservationLineCount =
      lastChatReservationLineCount - materializedReservationLineCount;
    const appendBlankLineCount = appendedBodyLines.length
      ? target.mutableLines.length
      : Math.max(0, target.mutableLines.length - residualReservationLineCount);
    effectiveReservationLineCount = appendedBodyLines.length
      ? target.mutableLines.length
      : residualReservationLineCount + appendBlankLineCount;

    output += "\x1b[?25l";
    output += clearChatReservationRegion(
      previousBodyLineCount,
      lastChatReservationLineCount,
      previousTotalLineCount,
    );
    output += terminalWriteLogicalLines(
      materializedLines,
      previousBodyLineCount + 1,
      previousTotalLineCount,
    );
    output += terminalAppendScrollbackLines(
      [...appendedBodyLines, ...blankLines(appendBlankLineCount)],
      previousTotalLineCount,
    );
  }
  const mutableLayout = terminalMutableLayout(
    target.mutableLines,
    target.pendingLiveLines.length,
    rendered.chromeCursor,
    target.bodyLines.length,
    effectiveReservationLineCount,
  );
  output += terminalWriteOverlayFrame(mutableLayout.frame, mutableLayout.startRow);
  output += cursorOutputFromAbsoluteCursor(mutableLayout.cursor);
  if (output) process.stdout.write(output);
  lastChatCacheLineCount = target.cacheLines.length;
  lastChatCommittedLiveLineCount = target.committedLiveLines.length;
  lastChatReservationLineCount = effectiveReservationLineCount;
  lastChatLiveFrame = rendered.liveFrame;
  lastChatChromeFrame = rendered.chromeFrame;
  lastChatLiveStreamKey = rendered.liveStreamKey;
  lastChatRenderCols = rendered.renderCols;
  return frame;
}

function drawSurface(state: AppState): string {
  if (state.help) return "help";
  if (state.sessionsOpen) return "sessions";
  if (state.authOpen) return "auth";
  if (state.settingsOpen) return "settings";
  if (state.personasOpen) return "personas";
  if (state.modelsOpen) return "models";
  return "chat";
}

function terminalAppendFrame(frame: string): string {
  if (!frame) return "";
  return frame.replace(/\n/g, "\r\n");
}

function cursorOutputFromFrameEnd(frame: string, cursor?: { row: number; column: number }): string {
  if (!cursor) return "";
  return `${cursorPositionFromFrameEnd(frame, cursor)}\x1b[?25h`;
}

function cursorOutputFromAbsoluteCursor(cursor?: { row: number; column: number }): string {
  if (!cursor) return "";
  return `${absoluteCursor(cursor.row, cursor.column)}\x1b[?25h`;
}

function cursorPositionFromFrameEnd(
  frame: string,
  cursor: { row: number; column: number },
): string {
  const frameRows = frameLineCount(frame);
  const cursorRow = Math.max(1, Math.min(frameRows, cursor.row));
  const rowsBelowCursor = frameRows - cursorRow;
  const column = Math.max(1, cursor.column);
  return `${rowsBelowCursor > 0 ? `\x1b[${rowsBelowCursor}A` : ""}\x1b[${column}G`;
}

function frameLineCount(frame: string): number {
  return frame ? frame.split("\n").length : 1;
}

function terminalSurfaceClear(): string {
  return `\x1b[0m${terminalClear}`;
}

function terminalRows(): number {
  return Math.max(1, process.stdout.rows || 1);
}

function terminalSaveCursor(): string {
  return "\x1b[s";
}

function terminalRestoreCursor(): string {
  return hasSavedChatScrollbackCursor ? "\x1b[u" : "";
}

function terminalAppendLines(lines: string[]): string {
  if (!lines.length) return "";
  return terminalAppendFrame(lines.join("\n"));
}

function terminalAppendScrollbackLines(lines: string[], previousTotalLineCount: number): string {
  if (!lines.length) return "";
  const prefix = previousTotalLineCount > 0 ? "\r\n" : "";
  return `${terminalRestoreCursor()}${prefix}${terminalAppendLines(lines)}${terminalSaveCursor()}`;
}

type MutableLayout = {
  frame: string;
  startRow: number;
  skippedRows: number;
  cursor?: { row: number; column: number };
};

type ChatScrollbackTarget = {
  cacheLines: string[];
  committedLiveLines: string[];
  pendingLiveLines: string[];
  mutableLines: string[];
  bodyLines: string[];
};

function chatScrollbackTarget(
  rendered: ReturnType<typeof renderChatFrameParts>,
  resetCommittedLiveCount: boolean,
): ChatScrollbackTarget {
  const cacheLines = frameLines(rendered.cacheFrame);
  const liveLines = frameLines(rendered.liveFrame);
  const chromeLines = frameLines(rendered.chromeFrame);
  const liveRows = rendered.liveRows.slice(0, liveLines.length);
  const previousCommittedLiveLineCount = resetCommittedLiveCount
    ? 0
    : lastChatCommittedLiveLineCount;
  const committedLiveLineCount = Math.min(
    liveLines.length,
    Math.max(previousCommittedLiveLineCount, completeLiveLineCount(liveRows)),
  );
  const committedLiveLines = liveLines.slice(0, committedLiveLineCount);
  const pendingLiveLines = liveLines.slice(committedLiveLineCount);
  const mutableLines = [...pendingLiveLines, ...chromeLines];
  return {
    cacheLines,
    committedLiveLines,
    pendingLiveLines,
    mutableLines,
    bodyLines: [...cacheLines, ...committedLiveLines],
  };
}

function completeLiveLineCount(lines: ReturnType<typeof renderChatFrameParts>["liveRows"]): number {
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    if (lines[index]?.kind !== "gap") return Math.max(0, index);
  }
  return 0;
}

function terminalMutableLayout(
  mutableLines: string[],
  pendingLiveLineCount: number,
  chromeCursor: { row: number; column: number } | undefined,
  bodyLineCount: number,
  reservationLineCount: number,
): MutableLayout {
  const totalLineCount = bodyLineCount + reservationLineCount;
  const reservationStartLogicalLine = bodyLineCount + 1;
  const visibleFirstLogicalLine = visibleFirstLogicalLineForTotal(totalLineCount);
  const visibleStartLogicalLine = Math.max(reservationStartLogicalLine, visibleFirstLogicalLine);
  const visibleReservationLineCount = Math.max(0, totalLineCount - visibleStartLogicalLine + 1);
  const skippedRows = Math.max(0, visibleStartLogicalLine - reservationStartLogicalLine);
  const visibleMutableLineCount = Math.max(
    0,
    Math.min(mutableLines.length - skippedRows, visibleReservationLineCount),
  );
  const visibleLines =
    visibleMutableLineCount > 0
      ? mutableLines.slice(skippedRows, skippedRows + visibleMutableLineCount)
      : [];
  const startRow = Math.max(1, visibleStartLogicalLine - visibleFirstLogicalLine + 1);
  return {
    frame: visibleLines.join("\n"),
    startRow,
    skippedRows,
    cursor: adjustMutableCursor(chromeCursor, pendingLiveLineCount, skippedRows, startRow),
  };
}

function frameLines(frame: string): string[] {
  return frame ? frame.split("\n") : [];
}

function blankLines(count: number): string[] {
  return Array.from({ length: Math.max(0, count) }, () => "");
}

function clearChatReservationRegion(
  bodyLineCount: number,
  reservationLineCount: number,
  totalLineCount: number,
): string {
  if (reservationLineCount <= 0 || totalLineCount <= 0) return "";
  const reservationStartLogicalLine = bodyLineCount + 1;
  const visibleFirstLogicalLine = visibleFirstLogicalLineForTotal(totalLineCount);
  const visibleStartLogicalLine = Math.max(reservationStartLogicalLine, visibleFirstLogicalLine);
  if (visibleStartLogicalLine > totalLineCount) return "";
  const startRow = visibleStartLogicalLine - visibleFirstLogicalLine + 1;
  return `${absoluteCursor(startRow, 1)}\x1b[J`;
}

function terminalWriteLogicalLines(
  lines: string[],
  startLogicalLine: number,
  totalLineCount: number,
): string {
  if (!lines.length || totalLineCount <= 0) return "";
  const visibleFirstLogicalLine = visibleFirstLogicalLineForTotal(totalLineCount);
  return lines
    .map((line, index) => {
      const logicalLine = startLogicalLine + index;
      if (logicalLine < visibleFirstLogicalLine || logicalLine > totalLineCount) return "";
      const row = logicalLine - visibleFirstLogicalLine + 1;
      return `${absoluteCursor(row, 1)}\x1b[2K${line}`;
    })
    .join("");
}

function visibleFirstLogicalLineForTotal(totalLineCount: number): number {
  return Math.max(1, totalLineCount - terminalRows() + 1);
}

function terminalWriteOverlayFrame(frame: string, startRow: number): string {
  if (!frame) return "";
  return frame
    .split("\n")
    .map((line, index) => `${absoluteCursor(startRow + index, 1)}${line}`)
    .join("");
}

function adjustMutableCursor(
  cursor: { row: number; column: number } | undefined,
  pendingLiveLineCount: number,
  skippedRows: number,
  startRow: number,
): { row: number; column: number } | undefined {
  if (!cursor) return undefined;
  const mutableRow = pendingLiveLineCount + cursor.row;
  const row = mutableRow - skippedRows;
  if (row < 1) return undefined;
  return { row: startRow + row - 1, column: cursor.column };
}

function absoluteCursor(row: number, column: number): string {
  return `\x1b[${Math.max(1, row)};${Math.max(1, column)}H`;
}
