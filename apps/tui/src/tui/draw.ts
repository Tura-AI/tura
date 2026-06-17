import type { AppState } from "./reducer.js";
import { renderChatFrameParts, renderFrame, type RenderedChatCache } from "./render.js";
import { clear as terminalClear, padVisible } from "./render-terminal.js";
import type { TerminalCapabilities } from "./capabilities.js";

let lastDrawSurface = "";
let lastDrawSessionID = "";
let lastChatCacheLineCount = 0;
let lastChatSpilledLiveLineCount = 0;
let lastChatReservationLineCount = 0;
let lastChatLiveFrame = "";
let lastChatSpilledLiveFrame = "";
let lastChatChromeFrame = "";
let lastChatTailCacheMessageCount = 0;
let lastChatActiveLiveMessageCount = 0;
let lastChatRenderCols = 0;
let hasSavedChatScrollbackCursor = false;
let lastRenderedChatCache: RenderedChatCache | undefined;

export function resetDrawState(): void {
  lastDrawSurface = "";
  lastDrawSessionID = "";
  lastChatCacheLineCount = 0;
  lastChatSpilledLiveLineCount = 0;
  lastChatReservationLineCount = 0;
  lastChatLiveFrame = "";
  lastChatSpilledLiveFrame = "";
  lastChatChromeFrame = "";
  lastChatTailCacheMessageCount = 0;
  lastChatActiveLiveMessageCount = 0;
  lastChatRenderCols = 0;
  hasSavedChatScrollbackCursor = false;
  lastRenderedChatCache = undefined;
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
  const sessionID = state.session?.id ?? "";
  const previousSurface = lastDrawSurface;
  const previousSessionID = lastDrawSessionID;
  const surfaceChanged = Boolean(previousSurface) && previousSurface !== surface;
  const shouldClearForSurface =
    options.forceReset || previousSessionID !== sessionID || surfaceChanged;
  const rendered =
    surface === "chat"
      ? renderChatFrameParts(state, capabilities, { cache: lastRenderedChatCache })
      : renderFrame(state, capabilities);
  const frame = rendered.frame;
  lastDrawSurface = surface;
  lastDrawSessionID = sessionID;

  if (surface === "chat") {
    lastRenderedChatCache = (rendered as ReturnType<typeof renderChatFrameParts>).cache;
    return drawChatFrame(
      rendered as ReturnType<typeof renderChatFrameParts>,
      previousFrame,
      shouldClearForSurface,
    );
  }

  let output = terminalSurfaceClear();
  output += "\x1b[?25l";
  output += terminalAppendFrame(frame);
  process.stdout.write(output);
  return frame;
}

export function drawChatChromeOverlay(
  state: AppState,
  capabilities: TerminalCapabilities,
  previousFrame = "",
): string {
  if (!process.stdout.isTTY) return previousFrame;
  if (drawSurface(state) !== "chat" || lastDrawSurface !== "chat") return previousFrame;
  const rendered = renderChatFrameParts(state, capabilities, { cache: lastRenderedChatCache });
  const sessionID = state.session?.id ?? "";
  if (lastDrawSessionID !== sessionID || lastChatRenderCols !== rendered.renderCols) {
    return previousFrame;
  }
  const bodyLineCount = lastChatCacheLineCount + lastChatSpilledLiveLineCount;
  let target = chatScrollbackTarget(rendered);
  if (rendered.activeLiveMessageCount === 0 && rendered.tailCacheMessageCount > 0) {
    target = promoteLiveFrameToScrollbackBody(target);
  }
  target = preserveSpilledLivePrefix(target, bodyLineCount);
  if (target.bodyLines.length !== bodyLineCount || lastChatReservationLineCount <= 0) {
    return previousFrame;
  }
  const chromeLayout = terminalChromeLayout(
    frameLines(rendered.chromeFrame),
    rendered.chromeCursor,
    bodyLineCount,
    target.pendingLiveLines.length,
    lastChatReservationLineCount,
  );
  let output = "\x1b[?25l";
  output += terminalWriteOverlayFrame(
    chromeLayout.frame,
    chromeLayout.startRow,
    chromeLayout.lineCount,
  );
  output += cursorOutputFromAbsoluteCursor(chromeLayout.cursor);
  if (output) process.stdout.write(output);
  lastChatChromeFrame = rendered.chromeFrame;
  lastRenderedChatCache = rendered.cache;
  return rendered.frame;
}

function drawChatFrame(
  rendered: ReturnType<typeof renderChatFrameParts>,
  _previousFrame: string,
  forceReset: boolean,
): string {
  const frame = rendered.frame;
  const renderWidthChanged = lastChatRenderCols !== 0 && rendered.renderCols !== lastChatRenderCols;
  let target = chatScrollbackTarget(rendered);
  const previousBodyLineCount = lastChatCacheLineCount + lastChatSpilledLiveLineCount;
  const previousTotalLineCount = previousBodyLineCount + lastChatReservationLineCount;
  const stableTailCacheToBody =
    rendered.activeLiveMessageCount === 0 && rendered.tailCacheMessageCount > 0;
  const finalizingLiveToCache =
    lastChatActiveLiveMessageCount > 0 &&
    stableTailCacheToBody;
  if (stableTailCacheToBody) target = promoteLiveFrameToScrollbackBody(target);
  target = preserveSpilledLivePrefix(target, previousBodyLineCount);
  const bodyShrank = previousBodyLineCount !== 0 && target.bodyLines.length < previousBodyLineCount;
  const firstChatDraw = lastChatRenderCols === 0;
  const spilledLiveFrame = target.spilledLiveLines.join("\n");
  const spilledLiveChanged =
    lastChatSpilledLiveLineCount > 0 &&
    target.spilledLiveLines.length >= lastChatSpilledLiveLineCount &&
    target.spilledLiveLines.slice(0, lastChatSpilledLiveLineCount).join("\n") !==
      lastChatSpilledLiveFrame;
  const rewriteAllRegions =
    forceReset || renderWidthChanged || bodyShrank || firstChatDraw || spilledLiveChanged;
  const liveChanged = lastChatLiveFrame !== rendered.liveFrame;
  const tailCacheChanged = lastChatTailCacheMessageCount !== rendered.tailCacheMessageCount;
  const chromeChanged = lastChatChromeFrame !== rendered.chromeFrame;
  const bodyChanged = target.bodyLines.length !== previousBodyLineCount;
  const reservationChanged = target.mutableLines.length !== lastChatReservationLineCount;
  const rewriteMutableRegion = liveChanged || tailCacheChanged || chromeChanged || reservationChanged;

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
    output += "\x1b[?25l";
    if (finalizingLiveToCache && linesEqual(newBodyLines, previousPendingLiveLines())) {
      // The finalized live rows are already present in the reservation tail from
      // the previous frame. Promote them to body/scrollback ownership without an
      // absolute rewrite, which avoids a one-frame blink on stream completion.
      const residualReservationLineCount = Math.max(
        0,
        lastChatReservationLineCount - newBodyLines.length,
      );
      const appendBlankLineCount = Math.max(
        0,
        target.mutableLines.length - residualReservationLineCount,
      );
      effectiveReservationLineCount = residualReservationLineCount + appendBlankLineCount;
      output += terminalAppendScrollbackLines(
        blankLines(appendBlankLineCount),
        previousTotalLineCount,
      );
    } else {
      const replacedReservationLineCount = Math.min(
        newBodyLines.length,
        lastChatReservationLineCount,
      );
      const replacementBodyLines = newBodyLines.slice(0, replacedReservationLineCount);
      const appendedBodyLines = newBodyLines.slice(replacedReservationLineCount);
      const residualReservationLineCount =
        lastChatReservationLineCount - replacedReservationLineCount;
      const appendBlankLineCount = appendedBodyLines.length
        ? target.mutableLines.length
        : Math.max(0, target.mutableLines.length - residualReservationLineCount);
      effectiveReservationLineCount = appendedBodyLines.length
        ? target.mutableLines.length
        : residualReservationLineCount + appendBlankLineCount;

      output += terminalWriteLogicalLines(
        replacementBodyLines,
        previousBodyLineCount + 1,
        previousTotalLineCount,
      );
      output += terminalAppendScrollbackLines(
        [...appendedBodyLines, ...blankLines(appendBlankLineCount)],
        previousTotalLineCount,
      );
    }
  }
  const mutableLayout = terminalMutableLayout(
    target.mutableLines,
    target.pendingLiveLines.length,
    rendered.chromeCursor,
    target.bodyLines.length,
    effectiveReservationLineCount,
  );
  output += terminalWriteOverlayFrame(
    mutableLayout.frame,
    mutableLayout.startRow,
    mutableLayout.lineCount,
  );
  output += cursorOutputFromAbsoluteCursor(mutableLayout.cursor);
  if (output) process.stdout.write(output);
  lastChatCacheLineCount = target.cacheLines.length;
  lastChatSpilledLiveLineCount = target.spilledLiveLines.length;
  lastChatReservationLineCount = effectiveReservationLineCount;
  lastChatLiveFrame = rendered.liveFrame;
  lastChatSpilledLiveFrame = spilledLiveFrame;
  lastChatChromeFrame = rendered.chromeFrame;
  lastChatTailCacheMessageCount = rendered.tailCacheMessageCount;
  lastChatActiveLiveMessageCount = rendered.activeLiveMessageCount;
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

function cursorOutputFromAbsoluteCursor(cursor?: { row: number; column: number }): string {
  if (!cursor) return "";
  return `${absoluteCursor(cursor.row, cursor.column)}\x1b[?25h`;
}

function terminalSurfaceClear(): string {
  return `\x1b[0m${terminalClear}`;
}

function terminalRows(): number {
  return Math.max(1, process.stdout.rows || 1);
}

function terminalColumns(): number {
  return Math.max(1, process.stdout.columns || 1);
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
  lineCount: number;
  startRow: number;
  skippedRows: number;
  cursor?: { row: number; column: number };
};

type ChatScrollbackTarget = {
  cacheLines: string[];
  spilledLiveLines: string[];
  pendingLiveLines: string[];
  mutableLines: string[];
  bodyLines: string[];
};

function chatScrollbackTarget(
  rendered: ReturnType<typeof renderChatFrameParts>,
): ChatScrollbackTarget {
  const cacheLines = rendered.cacheLines;
  const liveLines = frameLines(rendered.liveFrame);
  const chromeLines = frameLines(rendered.chromeFrame);
  const liveTailLineBudget = Math.max(0, terminalRows() - chromeLines.length);
  const spilledLiveLineCount = Math.max(0, liveLines.length - liveTailLineBudget);
  const spilledLiveLines = liveLines.slice(0, spilledLiveLineCount);
  const pendingLiveLines = liveLines.slice(spilledLiveLineCount);
  const mutableLines = [...pendingLiveLines, ...chromeLines];
  return {
    cacheLines,
    spilledLiveLines,
    pendingLiveLines,
    mutableLines,
    bodyLines: [...cacheLines, ...spilledLiveLines],
  };
}

function preserveSpilledLivePrefix(
  target: ChatScrollbackTarget,
  previousBodyLineCount: number,
): ChatScrollbackTarget {
  const missingBodyLines = previousBodyLineCount - target.bodyLines.length;
  if (missingBodyLines <= 0 || lastChatSpilledLiveLineCount <= 0) return target;
  if (target.cacheLines.length !== lastChatCacheLineCount) return target;
  if (!lastChatSpilledLiveFrame) return target;

  const previousSpilledLines = lastChatSpilledLiveFrame.split("\n");
  const liveLines = [...target.spilledLiveLines, ...target.pendingLiveLines];
  const preservedSpilledLineCount = Math.min(
    liveLines.length,
    target.spilledLiveLines.length + missingBodyLines,
    lastChatSpilledLiveLineCount,
  );
  if (preservedSpilledLineCount <= target.spilledLiveLines.length) return target;

  const spilledLiveLines = liveLines.slice(0, preservedSpilledLineCount);
  if (
    spilledLiveLines.join("\n") !==
    previousSpilledLines.slice(0, preservedSpilledLineCount).join("\n")
  ) {
    return target;
  }

  const chromeLines = target.mutableLines.slice(target.pendingLiveLines.length);
  const pendingLiveLines = liveLines.slice(preservedSpilledLineCount);
  const mutableLines = [...pendingLiveLines, ...chromeLines];
  return {
    cacheLines: target.cacheLines,
    spilledLiveLines,
    pendingLiveLines,
    mutableLines,
    bodyLines: [...target.cacheLines, ...spilledLiveLines],
  };
}

function promoteLiveFrameToScrollbackBody(target: ChatScrollbackTarget): ChatScrollbackTarget {
  const liveLines = [...target.spilledLiveLines, ...target.pendingLiveLines];
  if (!liveLines.length) return target;
  const chromeLines = target.mutableLines.slice(target.pendingLiveLines.length);
  return {
    cacheLines: target.cacheLines,
    spilledLiveLines: liveLines,
    pendingLiveLines: [],
    mutableLines: chromeLines,
    bodyLines: [...target.cacheLines, ...liveLines],
  };
}

function previousPendingLiveLines(): string[] {
  return frameLines(lastChatLiveFrame).slice(lastChatSpilledLiveLineCount);
}

function linesEqual(left: string[], right: string[]): boolean {
  if (left.length !== right.length) return false;
  return left.every((line, index) => line === right[index]);
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
  const visibleMutableLineCount = visibleReservationLineCount;
  const visibleLines =
    visibleMutableLineCount > 0
      ? padLines(
          mutableLines.slice(skippedRows, skippedRows + visibleMutableLineCount),
          visibleMutableLineCount,
        )
      : [];
  const startRow = Math.max(1, visibleStartLogicalLine - visibleFirstLogicalLine + 1);
  return {
    frame: visibleLines.join("\n"),
    lineCount: visibleLines.length,
    startRow,
    skippedRows,
    cursor: adjustMutableCursor(chromeCursor, pendingLiveLineCount, skippedRows, startRow),
  };
}

function terminalChromeLayout(
  chromeLines: string[],
  chromeCursor: { row: number; column: number } | undefined,
  bodyLineCount: number,
  pendingLiveLineCount: number,
  reservationLineCount: number,
): MutableLayout {
  const totalLineCount = bodyLineCount + reservationLineCount;
  const chromeStartLogicalLine = bodyLineCount + pendingLiveLineCount + 1;
  const visibleFirstLogicalLine = visibleFirstLogicalLineForTotal(totalLineCount);
  const visibleStartLogicalLine = Math.max(chromeStartLogicalLine, visibleFirstLogicalLine);
  const visibleChromeLineCount = Math.max(0, totalLineCount - visibleStartLogicalLine + 1);
  const skippedRows = Math.max(0, visibleStartLogicalLine - chromeStartLogicalLine);
  const visibleLines =
    visibleChromeLineCount > 0
      ? padLines(
          chromeLines.slice(skippedRows, skippedRows + visibleChromeLineCount),
          visibleChromeLineCount,
        )
      : [];
  const startRow = Math.max(1, visibleStartLogicalLine - visibleFirstLogicalLine + 1);
  return {
    frame: visibleLines.join("\n"),
    lineCount: visibleLines.length,
    startRow,
    skippedRows,
    cursor: adjustChromeCursor(chromeCursor, skippedRows, startRow),
  };
}

function frameLines(frame: string): string[] {
  return frame ? frame.split("\n") : [];
}

function blankLines(count: number): string[] {
  return Array.from({ length: Math.max(0, count) }, () => "");
}

function padLines(lines: string[], count: number): string[] {
  return lines.length >= count ? lines : [...lines, ...blankLines(count - lines.length)];
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
      return `${absoluteCursor(row, 1)}${lineRewriteText(line)}`;
    })
    .join("");
}

function visibleFirstLogicalLineForTotal(totalLineCount: number): number {
  return Math.max(1, totalLineCount - terminalRows() + 1);
}

function terminalWriteOverlayFrame(frame: string, startRow: number, lineCount: number): string {
  if (lineCount <= 0) return "";
  const lines = frame ? frame.split("\n") : blankLines(lineCount);
  return lines
    .map((line, index) => `${absoluteCursor(startRow + index, 1)}${lineRewriteText(line)}`)
    .join("");
}

function lineRewriteText(line: string): string {
  return line.includes("\x1b[K") ? line : padVisible(line, terminalColumns());
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

function adjustChromeCursor(
  cursor: { row: number; column: number } | undefined,
  skippedRows: number,
  startRow: number,
): { row: number; column: number } | undefined {
  if (!cursor) return undefined;
  const row = cursor.row - skippedRows;
  if (row < 1) return undefined;
  return { row: startRow + row - 1, column: cursor.column };
}

function absoluteCursor(row: number, column: number): string {
  return `\x1b[${Math.max(1, row)};${Math.max(1, column)}H`;
}
