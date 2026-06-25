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
  test("expands tables vertically and keeps only horizontal overflow controls", () => {
    expect(cssBlock(".rich-table-scroll")).not.toContain("max-height");
    expect(cssBlock(".rich-table-scroll")).toContain("overflow-x: auto;");
    expect(cssBlock(".rich-table-scroll")).toContain("overflow-y: visible;");
    expect(cssBlock(".rich-table-scroll")).toContain("scrollbar-width: none;");
    expect(cssBlock(".rich-table-scroll")).not.toContain("scrollbar-color");
    expect(cssBlock(".rich-table-scroll::-webkit-scrollbar")).toContain("display: none;");
    expect(richContentCss).toContain(".rich-table-overflow-x");
    expect(richContentCss).not.toContain(".rich-table-overflow-y");
  });

  test("keeps table columns unfrozen and rich cell text visible", () => {
    expect(cssBlock(".rich-table-scroll table")).toContain("min-width: 100%;");
    expect(cssBlock(".rich-table-scroll table")).toContain("width: max-content;");
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "text-align: left;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "white-space: normal;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "overflow: visible;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "text-overflow: clip;",
    );
    expect(cssBlock(".rich-table-scroll th:first-child,\n.rich-table-scroll td:first-child"))
      .not.toContain("position: sticky;");
    expect(cssBlock(".rich-table-scroll th")).not.toContain("position: sticky;");
  });
});
