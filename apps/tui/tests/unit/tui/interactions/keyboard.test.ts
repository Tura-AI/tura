import assert from "node:assert/strict";
import test from "node:test";
import { keySequence, printableSequence } from "../../../../src/tui/interactions/keyboard.js";

test("printableSequence accepts exactly one printable character", () => {
  assert.equal(printableSequence("a"), "a");
  assert.equal(printableSequence(" "), " ");
  assert.equal(printableSequence("é"), "é");
  assert.equal(printableSequence(""), undefined);
  assert.equal(printableSequence("ab"), undefined);
});

test("printableSequence rejects control and delete characters", () => {
  assert.equal(printableSequence("\n"), undefined);
  assert.equal(printableSequence("\r"), undefined);
  assert.equal(printableSequence("\t"), undefined);
  assert.equal(printableSequence("\x1b"), undefined);
  assert.equal(printableSequence("\x7f"), undefined);
});

test("keySequence returns only string key sequences", () => {
  assert.equal(keySequence({ sequence: "\u001b[A", name: "up" }), "\u001b[A");
  assert.equal(keySequence({ sequence: Buffer.from("a") }), undefined);
  assert.equal(keySequence({ sequence: 1 }), undefined);
  assert.equal(keySequence(undefined), undefined);
});
