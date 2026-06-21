import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const richContentCss = readFileSync(
  resolve(import.meta.dir, "../../../app/src/styles/parts/layout/rich-content.css"),
  "utf8",
);

function cssBlock(selector: string): string {
  const start = richContentCss.indexOf(selector);
  expect(start).toBeGreaterThanOrEqual(0);
  const open = richContentCss.indexOf("{", start);
  const close = richContentCss.indexOf("}", open);
  expect(open).toBeGreaterThan(start);
  expect(close).toBeGreaterThan(open);
  return richContentCss.slice(open + 1, close);
}

describe("rich content table scrollbars", () => {
  test("hides native table scrollbars and keeps the simplified overflow bars", () => {
    expect(cssBlock(".rich-table-scroll")).toContain("overflow: auto;");
    expect(cssBlock(".rich-table-scroll")).toContain("scrollbar-width: none;");
    expect(cssBlock(".rich-table-scroll")).not.toContain("scrollbar-color");
    expect(cssBlock(".rich-table-scroll::-webkit-scrollbar")).toContain("display: none;");
    expect(richContentCss).toContain(".rich-table-overflow-x");
    expect(richContentCss).toContain(".rich-table-overflow-y");
  });
});
