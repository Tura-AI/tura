import assert from "node:assert/strict";
import test from "node:test";
import {
  backspaceAtCursor,
  clampCursor,
  deleteAtCursor,
  insertAtCursor,
  moveCursorByCharacter,
  moveCursorByWord,
  moveCursorToLineBoundary,
} from "../../../src/tui/composer-editor.js";

test("composer edits insert and delete at the active cursor", () => {
  assert.deepEqual(insertAtCursor("helo", 3, "l"), { value: "hello", cursor: 4 });
  assert.deepEqual(backspaceAtCursor("hello", 3), { value: "helo", cursor: 2 });
  assert.deepEqual(deleteAtCursor("hello", 2), { value: "helo", cursor: 2 });
});

test("composer character navigation keeps grapheme clusters intact", () => {
  const value = "a😀b";
  assert.equal(moveCursorByCharacter(value, value.length, -1), 3);
  assert.equal(moveCursorByCharacter(value, 3, -1), 1);
  assert.equal(moveCursorByCharacter(value, 1, 1), 3);
  assert.deepEqual(backspaceAtCursor(value, 3), { value: "ab", cursor: 1 });
  assert.deepEqual(deleteAtCursor(value, 1), { value: "ab", cursor: 1 });

  for (const grapheme of ["e\u0301", "👩🏽‍💻"]) {
    const clustered = `a${grapheme}b`;
    const afterGrapheme = 1 + grapheme.length;
    assert.equal(moveCursorByCharacter(clustered, afterGrapheme, -1), 1);
    assert.equal(moveCursorByCharacter(clustered, 1, 1), afterGrapheme);
    assert.equal(clampCursor(clustered, 2), 1);
    assert.deepEqual(backspaceAtCursor(clustered, afterGrapheme), { value: "ab", cursor: 1 });
    assert.deepEqual(deleteAtCursor(clustered, 1), { value: "ab", cursor: 1 });
  }
});

test("composer supports shell-style word and multiline boundary navigation", () => {
  const value = "first word\nsecond line";
  assert.equal(moveCursorByWord(value, 10, -1), 6);
  assert.equal(moveCursorByWord(value, 6, 1), 11);
  assert.equal(moveCursorToLineBoundary(value, 17, "start"), 11);
  assert.equal(moveCursorToLineBoundary(value, 17, "end"), value.length);
});
