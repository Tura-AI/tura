import assert from "node:assert/strict";
import { setLanguage } from "../../../../src/i18n.js";
import type { Session } from "../../../../src/types/session.js";
import { reducer, type AppAction, type AppState } from "../../../../src/tui/reducer.js";
import { resetDrawState } from "../../../../src/tui/draw.js";

setLanguage("en");

export const activeSession: Session = {
  id: "sess-1",
  name: "Active",
  directory: "C:/repo",
  status: "idle",
  updated_at: 1,
  message_count: 2,
};

export const otherSession: Session = {
  id: "sess-2",
  name: "Other",
  directory: "C:/repo",
  status: "idle",
  updated_at: 2,
  message_count: 3,
};

export function lastAbsoluteCursorBefore(
  output: string,
  index: number,
): { row: number; column: number } | undefined {
  if (index < 0) return undefined;
  let cursor: { row: number; column: number } | undefined;
  for (const match of output.slice(0, index).matchAll(/\x1b\[(\d+);(\d+)H/gu)) {
    cursor = { row: Number(match[1]), column: Number(match[2]) };
  }
  return cursor;
}

export function assertMutableRegionClearedBefore(
  output: string,
  _cursorRow: number,
  marker: string,
): void {
  const clearMatch = output.match(/\x1b\[\d+;1H\x1b\[J/u);
  assert.ok(clearMatch?.index !== undefined, "live/chrome rewrite must clear mutable region first");
  const markerIndex = output.indexOf(marker);
  assert.ok(markerIndex >= 0, `expected rewritten output to include ${marker}`);
  assert.ok(
    clearMatch.index < markerIndex,
    "live/chrome content must be written only after clearing the old mutable region",
  );
}

export function captureDrawWrites(fn: (writes: string[]) => void): string[] {
  resetDrawState();
  const writes: string[] = [];
  const isTTY = Object.getOwnPropertyDescriptor(process.stdout, "isTTY");
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  const write = Object.getOwnPropertyDescriptor(process.stdout, "write");
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: 80 });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 20 });
  Object.defineProperty(process.stdout, "write", {
    configurable: true,
    value: (chunk: string | Uint8Array): boolean => {
      writes.push(typeof chunk === "string" ? chunk : chunk.toString());
      return true;
    },
  });
  try {
    fn(writes);
    return writes;
  } finally {
    resetDrawState();
    restoreProperty(process.stdout, "isTTY", isTTY);
    restoreProperty(process.stdout, "columns", columns);
    restoreProperty(process.stdout, "rows", rows);
    restoreProperty(process.stdout, "write", write);
  }
}

export async function captureDrawWritesAsync(
  fn: (writes: string[]) => Promise<void>,
): Promise<string[]> {
  resetDrawState();
  const writes: string[] = [];
  const isTTY = Object.getOwnPropertyDescriptor(process.stdout, "isTTY");
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rows = Object.getOwnPropertyDescriptor(process.stdout, "rows");
  const write = Object.getOwnPropertyDescriptor(process.stdout, "write");
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: 80 });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: 20 });
  Object.defineProperty(process.stdout, "write", {
    configurable: true,
    value: (chunk: string | Uint8Array): boolean => {
      writes.push(typeof chunk === "string" ? chunk : chunk.toString());
      return true;
    },
  });
  try {
    await fn(writes);
    return writes;
  } finally {
    resetDrawState();
    restoreProperty(process.stdout, "isTTY", isTTY);
    restoreProperty(process.stdout, "columns", columns);
    restoreProperty(process.stdout, "rows", rows);
    restoreProperty(process.stdout, "write", write);
  }
}

export function regexCount(text: string, pattern: RegExp): number {
  return Array.from(text.matchAll(pattern)).length;
}

export function stateHarness(initial: AppState): {
  getState: () => AppState;
  dispatch: (action: AppAction) => void;
} {
  let state = initial;
  return {
    getState: () => state,
    dispatch: (action) => {
      state = reducer(state, action);
    },
  };
}

export function restoreProperty<T extends object>(
  target: T,
  key: keyof T,
  descriptor: PropertyDescriptor | undefined,
): void {
  if (descriptor) Object.defineProperty(target, key, descriptor);
  else Reflect.deleteProperty(target, key);
}
