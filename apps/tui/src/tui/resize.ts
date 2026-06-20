import { TUI_RESIZE_DRAW_PAUSE_MS } from "./frame-rate.js";
import type { AppAction, AppState } from "./reducer.js";

export function createResizeDrawGate(options: {
  drawNow: () => void;
  clearPendingDraw: () => void;
  resizePauseMs?: number;
  setTimeoutFn?: (callback: () => void, ms: number) => ReturnType<typeof setTimeout>;
  clearTimeoutFn?: (timer: ReturnType<typeof setTimeout>) => void;
}): {
  isFrozen: () => boolean;
  enterResize: () => void;
  dispose: () => void;
} {
  const resizePauseMs = options.resizePauseMs ?? TUI_RESIZE_DRAW_PAUSE_MS;
  const setTimeoutFn = options.setTimeoutFn ?? setTimeout;
  const clearTimeoutFn = options.clearTimeoutFn ?? clearTimeout;
  let frozen = false;
  let resizeEndTimer: ReturnType<typeof setTimeout> | undefined;

  const clearResizeEndTimer = () => {
    if (!resizeEndTimer) return;
    clearTimeoutFn(resizeEndTimer);
    resizeEndTimer = undefined;
  };
  const finishResize = () => {
    resizeEndTimer = undefined;
    if (!frozen) return;
    frozen = false;
    options.drawNow();
  };

  return {
    isFrozen: () => frozen,
    enterResize: () => {
      if (!frozen) {
        options.drawNow();
        frozen = true;
      }
      options.clearPendingDraw();
      clearResizeEndTimer();
      resizeEndTimer = setTimeoutFn(finishResize, resizePauseMs);
    },
    dispose: () => {
      clearResizeEndTimer();
      frozen = false;
    },
  };
}

export function createTerminalResizeHandler(
  getState: () => AppState,
  dispatch: (action: AppAction) => void,
  options: { onResize?: () => void; onHeightResize?: () => void } = {},
): () => void {
  let lastResizeSize = terminalSize();
  return () => {
    const size = terminalSize();
    if (size.columns === lastResizeSize.columns && size.rows === lastResizeSize.rows) return;
    const columnsChanged = size.columns !== lastResizeSize.columns;
    const rowsChanged = size.rows !== lastResizeSize.rows;
    lastResizeSize = size;
    options.onResize?.();
    if (columnsChanged) {
      dispatch({ type: "notice", value: getState().notice });
      return;
    }
    if (rowsChanged) options.onHeightResize?.();
  };
}

function terminalSize(): { columns: number; rows: number } {
  return {
    columns: process.stdout.columns || 0,
    rows: process.stdout.rows || 0,
  };
}
