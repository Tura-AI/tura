import { graphemeBoundaries } from "./text-graphemes.js";

export interface ComposerEdit {
  value: string;
  cursor: number;
}

export function insertAtCursor(value: string, cursor: number, text: string): ComposerEdit {
  const boundaries = graphemeBoundaries(value);
  const at = boundaries[boundaryIndex(value, cursor, boundaries)] ?? 0;
  return { value: `${value.slice(0, at)}${text}${value.slice(at)}`, cursor: at + text.length };
}

export function backspaceAtCursor(value: string, cursor: number): ComposerEdit {
  const boundaries = graphemeBoundaries(value);
  const index = boundaryIndex(value, cursor, boundaries);
  const at = boundaries[index] ?? 0;
  const previous = boundaries[Math.max(0, index - 1)] ?? 0;
  if (previous === at) return { value, cursor: at };
  return { value: `${value.slice(0, previous)}${value.slice(at)}`, cursor: previous };
}

export function deleteAtCursor(value: string, cursor: number): ComposerEdit {
  const boundaries = graphemeBoundaries(value);
  const index = boundaryIndex(value, cursor, boundaries);
  const at = boundaries[index] ?? 0;
  const next = boundaries[Math.min(boundaries.length - 1, index + 1)] ?? value.length;
  if (next === at) return { value, cursor: at };
  return { value: `${value.slice(0, at)}${value.slice(next)}`, cursor: at };
}

export function moveCursorByCharacter(value: string, cursor: number, direction: -1 | 1): number {
  const boundaries = graphemeBoundaries(value);
  const index = boundaryIndex(value, cursor, boundaries);
  const nextIndex = Math.max(0, Math.min(boundaries.length - 1, index + direction));
  return boundaries[nextIndex] ?? 0;
}

export function moveCursorByWord(value: string, cursor: number, direction: -1 | 1): number {
  const boundaries = graphemeBoundaries(value);
  let index = boundaryIndex(value, cursor, boundaries);
  if (direction < 0) {
    while (index > 0 && isWhitespace(value, boundaries, index - 1)) index -= 1;
    while (index > 0 && !isWhitespace(value, boundaries, index - 1)) index -= 1;
    return boundaries[index] ?? 0;
  }
  while (index < boundaries.length - 1 && !isWhitespace(value, boundaries, index)) index += 1;
  while (index < boundaries.length - 1 && isWhitespace(value, boundaries, index)) index += 1;
  return boundaries[index] ?? value.length;
}

export function moveCursorToLineBoundary(
  value: string,
  cursor: number,
  boundary: "start" | "end",
): number {
  const at = clampCursor(value, cursor);
  if (boundary === "start") return value.lastIndexOf("\n", Math.max(0, at - 1)) + 1;
  const newline = value.indexOf("\n", at);
  return newline < 0 ? value.length : newline;
}

export function clampCursor(value: string, cursor: number): number {
  const boundaries = graphemeBoundaries(value);
  return boundaries[boundaryIndex(value, cursor, boundaries)] ?? 0;
}

function boundaryIndex(value: string, cursor: number, boundaries: number[]): number {
  if (!Number.isFinite(cursor)) return boundaries.length - 1;
  const safe = Math.max(0, Math.min(Math.trunc(cursor), value.length));
  for (let index = 0; index < boundaries.length; index += 1) {
    const boundary = boundaries[index] ?? 0;
    if (boundary === safe) return index;
    if (boundary > safe) return Math.max(0, index - 1);
  }
  return boundaries.length - 1;
}

function isWhitespace(value: string, boundaries: number[], index: number): boolean {
  return /\s/u.test(value.slice(boundaries[index], boundaries[index + 1]));
}
