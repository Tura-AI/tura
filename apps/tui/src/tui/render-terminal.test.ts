import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import test from "node:test";
import { plainCapabilities, richCapabilities } from "./capabilities.js";
import {
  fit,
  pad,
  rule,
  setActiveCapabilities,
  stripAnsi,
  truncate,
  truncateAnsi,
  visibleTextWidth,
  wrap,
  wrapAnsi,
} from "./render-terminal.js";

test("visibleTextWidth treats ANSI, CJK, emoji, and combining marks correctly", () => {
  assert.equal(visibleTextWidth("abc"), 3);
  assert.equal(visibleTextWidth("\x1b[31mred\x1b[0m"), 3);
  assert.equal(visibleTextWidth("中文"), 4);
  assert.equal(visibleTextWidth("e\u0301"), 1);
  assert.equal(visibleTextWidth("👨‍💻"), 2);
});

test("plain truncation uses ASCII ellipsis and rich truncation uses Unicode ellipsis", () => {
  setActiveCapabilities(plainCapabilities());
  assert.equal(truncate("abcdefghij", 6), "abc...");
  assert.equal(rule(4), "----");

  setActiveCapabilities(richCapabilities());
  assert.equal(truncate("abcdefghij", 6), "abcde…");
  assert.match(rule(4), /─{4}/u);
});

test("truncateAnsi preserves control sequences and terminates styling", () => {
  setActiveCapabilities(richCapabilities());
  const rendered = truncateAnsi("\x1b[31mabcdef\x1b[0m", 4);
  assert.equal(stripAnsi(rendered), "abc…");
  assert.match(rendered, /\x1b\[0m$/u);
});

test("wrap and fit keep line widths bounded at small terminal sizes", () => {
  setActiveCapabilities(richCapabilities());
  const wrapped = wrap("abcdefghij 中文 emoji 👨‍💻", 10);
  assert.ok(wrapped.length > 1);
  assert.ok(wrapped.every((line) => visibleTextWidth(line) <= 20));

  const fitted = fit(["1234567890", "abcdefghij"], 1, 5);
  assert.equal(fitted.length, 1);
  assert.ok(visibleTextWidth(stripAnsi(fitted[0])) <= 5);
});

test("pad respects visible ANSI width", () => {
  setActiveCapabilities(richCapabilities());
  const value = pad("\x1b[32mok\x1b[0m", 5);
  assert.equal(visibleTextWidth(value), 5);
});

test("wrapAnsi handles large streamed output within a bounded performance budget", () => {
  setActiveCapabilities(richCapabilities());
  const source = Array.from(
    { length: 2_000 },
    (_item, index) =>
      `\x1b[36m${index.toString().padStart(4, "0")} ${"long-output ".repeat(8)}中文👨‍💻\x1b[0m`,
  ).join("\n");
  const started = performance.now();
  const lines = wrapAnsi(source, 80);
  const elapsed = performance.now() - started;
  assert.ok(lines.length >= 2_000);
  assert.ok(elapsed < 1_500, `wrapAnsi took ${elapsed.toFixed(1)}ms`);
});
